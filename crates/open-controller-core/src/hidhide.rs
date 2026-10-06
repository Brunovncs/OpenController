//! Hiding the physical controllers from games with HidHide, so a game that understands a
//! DualSense does not see it twice (once as itself, once as the virtual Xbox controller).
//!
//! HidHide is a filter driver with a global configuration: a list of hidden devices (by
//! instance id), a list of programs that still see them (by NT image path) and a switch that
//! turns hiding on. Other programs, DS4Windows among them, keep entries there too. So this
//! module only ever adds and removes its own entries, by read-modify-write, and keeps a journal
//! of them on disk before touching the driver. If Open Controller crashes, the next start reads
//! the journal and undoes what the crash left behind.
//!
//! The driver's control device admits one handle at a time, so it is opened for each request
//! and closed right after; while another program holds it, the request is retried briefly.

use crate::win::{Handle, from_multi_sz, from_wide, ioctl_sync, to_multi_sz, wide};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, GENERIC_READ, GENERIC_WRITE, GetLastError};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_NAME_NORMALIZED, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFinalPathNameByHandleW,
    OPEN_EXISTING, VOLUME_NAME_NT,
};

const fn ioctl(function: u32) -> u32 {
    crate::win::ctl_code(0x8001, function, 0, 1)
}
const IOCTL_GET_WHITELIST: u32 = ioctl(0x800);
const IOCTL_SET_WHITELIST: u32 = ioctl(0x801);
const IOCTL_GET_BLACKLIST: u32 = ioctl(0x802);
const IOCTL_SET_BLACKLIST: u32 = ioctl(0x803);
const IOCTL_GET_ACTIVE: u32 = ioctl(0x804);
const IOCTL_SET_ACTIVE: u32 = ioctl(0x805);
const IOCTL_GET_WLINVERSE: u32 = ioctl(0x806);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloakError {
    NotInstalled,
    /// Another program kept HidHide's control device open.
    Busy,
    /// In inverse mode the program list means the opposite; Open Controller leaves it alone.
    InverseMode,
    /// The driver accepted a list but reads back something else.
    NotApplied,
    /// This program's own path could not be determined, so it could not keep seeing what it hides.
    Unregistered,
    /// The journal could not be written, so nothing was hidden.
    Journal(String),
    Os(u32),
}

impl std::fmt::Display for CloakError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CloakError::NotInstalled => f.write_str("HidHide is not installed"),
            CloakError::Busy => f.write_str("HidHide is in use by another program"),
            CloakError::InverseMode => f.write_str("HidHide is in inverse mode"),
            CloakError::NotApplied => f.write_str("HidHide did not apply the change"),
            CloakError::Unregistered => f.write_str("could not register with HidHide"),
            CloakError::Journal(e) => write!(f, "could not write the HidHide journal: {e}"),
            CloakError::Os(e) => write!(f, "HidHide error {e}"),
        }
    }
}

/// The raw driver interface: one open handle on HidHide's control device. The device admits one
/// handle at a time, so holding it is a lock; every read-modify-write runs on a single session,
/// and no other program can change the lists between the read and the write.
pub struct HidHide {
    handle: Handle,
}

/// Attempts at opening the device while another program holds it, 20 ms apart.
const OPEN_ATTEMPTS: u32 = 25;
/// At shutdown, worth waiting longer than that: giving up leaves controllers hidden.
const OPEN_ATTEMPTS_AT_EXIT: u32 = 250;

impl HidHide {
    /// True when the driver is installed (its control device exists).
    pub fn installed() -> bool {
        !matches!(Self::open(), Err(CloakError::NotInstalled))
    }

    pub fn open() -> Result<HidHide, CloakError> {
        Self::open_patiently(OPEN_ATTEMPTS)
    }

    fn open_patiently(attempts: u32) -> Result<HidHide, CloakError> {
        for _ in 0..attempts {
            let raw = unsafe {
                CreateFileW(
                    wide(r"\\.\HidHide").as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    0,
                    null_mut(),
                )
            };
            if let Some(handle) = Handle::valid(raw) {
                return Ok(HidHide { handle });
            }
            match unsafe { GetLastError() } {
                ERROR_FILE_NOT_FOUND => return Err(CloakError::NotInstalled),
                // Held open by someone else; it is usually a matter of milliseconds.
                ERROR_ACCESS_DENIED => std::thread::sleep(std::time::Duration::from_millis(20)),
                e => return Err(CloakError::Os(e)),
            }
        }
        Err(CloakError::Busy)
    }

    fn call(&self, code: u32, input: &[u8], output: &mut [u8]) -> Result<u32, CloakError> {
        ioctl_sync(self.handle.0, code, input, output).map_err(CloakError::Os)
    }

    fn get_bool(&self, code: u32) -> Result<bool, CloakError> {
        let mut b = [0u8; 1];
        self.call(code, &[], &mut b)?;
        Ok(b[0] != 0)
    }

    fn get_list(&self, code: u32) -> Result<Vec<String>, CloakError> {
        // A first call without a buffer returns the size needed. The driver refuses buffers far
        // larger than that, so a guess at a big one is not an option.
        let needed = self.call(code, &[], &mut [])? as usize;
        let mut buf = vec![0u8; needed.max(4) + 4];
        let n = self.call(code, &[], &mut buf)? as usize;
        let words: Vec<u16> = buf[..n.min(buf.len())].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(from_multi_sz(&words))
    }

    fn set_list(&self, code: u32, items: &[String]) -> Result<(), CloakError> {
        let bytes: Vec<u8> = to_multi_sz(items).iter().flat_map(|w| w.to_le_bytes()).collect();
        self.call(code, &bytes, &mut []).map(drop)
    }

    pub fn active(&self) -> Result<bool, CloakError> {
        self.get_bool(IOCTL_GET_ACTIVE)
    }

    pub fn set_active(&self, on: bool) -> Result<(), CloakError> {
        self.call(IOCTL_SET_ACTIVE, &[on as u8], &mut []).map(drop)
    }

    pub fn inverse(&self) -> Result<bool, CloakError> {
        self.get_bool(IOCTL_GET_WLINVERSE)
    }

    pub fn whitelist(&self) -> Result<Vec<String>, CloakError> {
        self.get_list(IOCTL_GET_WHITELIST)
    }

    pub fn set_whitelist(&self, items: &[String]) -> Result<(), CloakError> {
        self.set_list(IOCTL_SET_WHITELIST, items)
    }

    pub fn blacklist(&self) -> Result<Vec<String>, CloakError> {
        self.get_list(IOCTL_GET_BLACKLIST)
    }

    pub fn set_blacklist(&self, items: &[String]) -> Result<(), CloakError> {
        self.set_list(IOCTL_SET_BLACKLIST, items)
    }
}

/// What Open Controller changed in HidHide's configuration, kept on disk while it is in effect.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    /// Device instance ids this program added to the hidden list.
    pub hidden: Vec<String>,
    /// Hiding was off and this program turned it on.
    pub enabled: bool,
}

fn contains(list: &[String], id: &str) -> bool {
    list.iter().any(|x| x.eq_ignore_ascii_case(id))
}

/// `list` plus every id in `add` it does not have yet, compared without case.
pub fn merged(list: &[String], add: &[String]) -> Vec<String> {
    let mut out = list.to_vec();
    for id in add {
        if !contains(&out, id) {
            out.push(id.clone());
        }
    }
    out
}

/// `list` without the ids in `remove`, compared without case.
pub fn without(list: &[String], remove: &[String]) -> Vec<String> {
    list.iter().filter(|x| !contains(remove, x)).cloned().collect()
}

/// This executable's path as the kernel sees it (`\Device\HarddiskVolume3\...\app.exe`), the
/// form HidHide matches programs by. Junctions and substituted drives are resolved.
pub fn own_image_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let h = unsafe {
        CreateFileW(
            wide(&exe).as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            null_mut(),
        )
    };
    let h = Handle::valid(h)?;
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetFinalPathNameByHandleW(h.0, buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_NT) };
    (n > 0 && (n as usize) < buf.len()).then(|| from_wide(&buf))
}

fn read_journal(path: &Path) -> Option<Journal> {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice::<Journal>(&b).ok())
}

/// Writes the journal so that a crash at any point leaves either the old file or the new one:
/// to a temporary file first, then renamed over it.
fn write_journal(path: &Path, journal: &Journal) -> std::io::Result<()> {
    if *journal == Journal::default() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(journal).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, path)
}

/// Removes this program's entries (`journal.hidden` ∩ `ids`) from the hidden list, and turns
/// hiding off again once none is left if this program had turned it on.
fn reveal_on(driver: &HidHide, journal: &mut Journal, ids: &[String]) -> Result<(), CloakError> {
    let mine: Vec<String> = journal.hidden.iter().filter(|x| contains(ids, x)).cloned().collect();
    if !mine.is_empty() {
        let list = driver.blacklist()?;
        let kept = without(&list, &mine);
        if kept.len() != list.len() {
            driver.set_blacklist(&kept)?;
        }
        journal.hidden = without(&journal.hidden, &mine);
    }
    if journal.hidden.is_empty() && journal.enabled {
        driver.set_active(false)?;
        journal.enabled = false;
    }
    Ok(())
}

/// HidHide, managed safely: own entries only, journaled, verified by reading back.
pub struct Cloak {
    journal_path: PathBuf,
    journal: Journal,
    /// Ids known to be hidden, including ones another program had already hidden.
    covered: Vec<String>,
}

impl Cloak {
    /// Opens HidHide, undoes anything a previous run left behind and lets this program see
    /// hidden devices.
    pub fn start(journal_path: &Path) -> Result<Cloak, CloakError> {
        let driver = HidHide::open()?;
        if driver.inverse()? {
            return Err(CloakError::InverseMode);
        }
        // Without its own path in the program list, hiding a controller would hide it from
        // this program too, the next time it reconnects.
        let me = own_image_path().ok_or(CloakError::Unregistered)?;
        let mut cloak = Cloak { journal_path: journal_path.to_path_buf(), journal: Journal::default(), covered: Vec::new() };
        if let Some(stale) = read_journal(journal_path) {
            cloak.journal = stale;
            let all = cloak.journal.hidden.clone();
            reveal_on(&driver, &mut cloak.journal, &all)?;
            cloak.save()?;
        }
        let list = driver.whitelist()?;
        if !contains(&list, &me) {
            driver.set_whitelist(&merged(&list, &[me]))?;
        }
        Ok(cloak)
    }

    fn save(&self) -> Result<(), CloakError> {
        write_journal(&self.journal_path, &self.journal).map_err(|e| CloakError::Journal(e.to_string()))
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        contains(&self.covered, id)
    }

    /// Hides the devices from every program but the whitelisted ones.
    pub fn hide(&mut self, ids: &[String]) -> Result<(), CloakError> {
        let new: Vec<String> = ids.iter().filter(|id| !self.is_hidden(id)).cloned().collect();
        if new.is_empty() {
            return Ok(());
        }
        let driver = HidHide::open()?;
        let list = driver.blacklist()?;
        // Entries another program put there stay theirs: they are neither journaled nor removed.
        let added: Vec<String> = new.iter().filter(|id| !contains(&list, id)).cloned().collect();
        let was_active = driver.active()?;
        // Journal first: a crash after this line is undone on the next start. If the journal
        // cannot be written, nothing is hidden.
        let before = self.journal.clone();
        self.journal.hidden = merged(&self.journal.hidden, &added);
        self.journal.enabled |= !was_active;
        if let Err(e) = self.save() {
            self.journal = before;
            return Err(e);
        }
        if !added.is_empty() {
            driver.set_blacklist(&merged(&list, &added))?;
        }
        if !was_active {
            driver.set_active(true)?;
        }
        let now = driver.blacklist()?;
        if !new.iter().all(|id| contains(&now, id)) || !driver.active()? {
            return Err(CloakError::NotApplied);
        }
        self.covered = merged(&self.covered, &new);
        Ok(())
    }

    /// Shows the devices again, if this program hid them.
    pub fn reveal(&mut self, ids: &[String]) -> Result<(), CloakError> {
        self.covered = without(&self.covered, ids);
        if !self.journal.hidden.iter().any(|x| contains(ids, x)) {
            return Ok(());
        }
        let driver = HidHide::open()?;
        reveal_on(&driver, &mut self.journal, ids)?;
        self.save()
    }

    /// Removes every entry this program added and turns hiding off again if it turned it on.
    /// Waits for HidHide longer than other calls do: failing here leaves controllers hidden
    /// until the next start.
    pub fn reveal_all(&mut self) -> Result<(), CloakError> {
        self.covered.clear();
        if self.journal == Journal::default() {
            return Ok(());
        }
        let driver = HidHide::open_patiently(OPEN_ATTEMPTS_AT_EXIT)?;
        let all = self.journal.hidden.clone();
        reveal_on(&driver, &mut self.journal, &all)?;
        self.save()
    }
}

/// For uninstalling: undoes what a run that was killed left hidden and removes this program
/// from HidHide's list of programs that see hidden devices. Nothing to do without HidHide.
pub fn restore(journal_path: &Path) -> Result<(), CloakError> {
    let driver = match HidHide::open_patiently(OPEN_ATTEMPTS_AT_EXIT) {
        Err(CloakError::NotInstalled) => return Ok(()),
        other => other?,
    };
    if let Some(mut stale) = read_journal(journal_path) {
        let all = stale.hidden.clone();
        reveal_on(&driver, &mut stale, &all)?;
        write_journal(journal_path, &stale).map_err(|e| CloakError::Journal(e.to_string()))?;
    }
    if let Some(me) = own_image_path() {
        let list = driver.whitelist()?;
        if contains(&list, &me) {
            driver.set_whitelist(&without(&list, &[me]))?;
        }
    }
    Ok(())
}

/// Where the engine keeps the journal, in the app data folder.
pub const JOURNAL_FILE: &str = "hidhide-journal.json";

impl Drop for Cloak {
    fn drop(&mut self) {
        let _ = self.reveal_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn ioctl_codes_match_the_driver() {
        assert_eq!(IOCTL_GET_WHITELIST, 0x8001_6000);
        assert_eq!(IOCTL_SET_WHITELIST, 0x8001_6004);
        assert_eq!(IOCTL_GET_BLACKLIST, 0x8001_6008);
        assert_eq!(IOCTL_SET_BLACKLIST, 0x8001_600C);
        assert_eq!(IOCTL_GET_ACTIVE, 0x8001_6010);
        assert_eq!(IOCTL_SET_ACTIVE, 0x8001_6014);
        assert_eq!(IOCTL_GET_WLINVERSE, 0x8001_6018);
    }

    #[test]
    fn other_programs_entries_survive() {
        let theirs = s(&[r"HID\VID_054C&PID_05C4\1", r"HID\VID_057E&PID_2009\2"]);
        let mine = s(&[r"hid\vid_054c&pid_0ce6\3", r"HID\VID_054C&PID_05C4\1"]);
        let added = merged(&theirs, &mine);
        assert_eq!(added.len(), 3, "an entry already there is not doubled, whatever its case");
        // Only what was not there before is journaled, so only that is removed later.
        let journaled: Vec<String> = mine.iter().filter(|id| !contains(&theirs, id)).cloned().collect();
        assert_eq!(journaled, s(&[r"hid\vid_054c&pid_0ce6\3"]));
        assert_eq!(without(&added, &journaled), theirs);
        assert_eq!(without(&theirs, &[]), theirs);
    }

    /// Hides a made-up device on the real driver and checks that everything is put back,
    /// including after a simulated crash. Needs HidHide:
    /// `cargo test -p open-controller-core -- --ignored hidhide`.
    #[test]
    #[ignore]
    fn hidhide_round_trip_on_the_driver() {
        let dir = std::env::temp_dir().join(format!("oc-hidhide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("journal.json");
        let probe = r"HID\VID_0000&PID_0000\OPEN-CONTROLLER-TEST".to_string();
        assert!(HidHide::installed(), "HidHide is not installed");
        // Each look opens and closes the device: holding it would lock the code under test out.
        let state = || {
            let d = HidHide::open().unwrap();
            (d.blacklist().unwrap(), d.active().unwrap(), d.whitelist().unwrap())
        };
        let (list0, active0, white0) = state();

        let mut cloak = Cloak::start(&journal).unwrap();
        cloak.hide(std::slice::from_ref(&probe)).unwrap();
        let (list, active, _) = state();
        assert!(contains(&list, &probe) && active && journal.exists());
        cloak.reveal(std::slice::from_ref(&probe)).unwrap();
        assert_eq!((state().0, state().1), (list0.clone(), active0), "revealing one device puts it back");
        cloak.hide(std::slice::from_ref(&probe)).unwrap();
        // A crash: the journal stays, nothing is reverted.
        std::mem::forget(cloak);
        assert!(contains(&state().0, &probe));
        // The next start reverts it.
        let cloak = Cloak::start(&journal).unwrap();
        assert_eq!((state().0, state().1), (list0, active0));
        assert!(!journal.exists());
        drop(cloak);

        // Leave the whitelist as it was: this test binary is not a program that needs it.
        HidHide::open().unwrap().set_whitelist(&white0).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn journal_is_written_whole_and_removed_when_empty() {
        let path = std::env::temp_dir().join(format!("oc-journal-{}.json", std::process::id()));
        let j = Journal { hidden: s(&["A"]), enabled: true };
        write_journal(&path, &j).unwrap();
        assert_eq!(read_journal(&path), Some(j));
        assert!(!path.with_extension("json.tmp").exists());
        write_journal(&path, &Journal::default()).unwrap();
        assert!(!path.exists());
        write_journal(&path, &Journal::default()).unwrap();
    }
}
