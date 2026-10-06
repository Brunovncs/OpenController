//! Hiding a controller from games on Linux: OpenController holds an exclusive grab on every
//! event node the kernel made for it, so programs reading event devices (Wine, Proton, most
//! games) get nothing from them. SDL reads the controller through its `hidraw` node, which a
//! grab leaves alone. Grabs end with the process, so nothing stays hidden after a crash.

use super::devnode::event_nodes;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const EVIOCGRAB: libc::c_ulong = super::ioc::iow::<libc::c_int>(b'E', 0x90);

/// Kept for the engine's sake: Windows keeps a journal of what it hid, Linux has nothing to
/// undo.
pub const JOURNAL_FILE: &str = "hidden.json";

#[derive(Debug)]
pub enum CloakError {
    NotInstalled,
    Os(String),
}

impl std::fmt::Display for CloakError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CloakError::NotInstalled => f.write_str("not installed"),
            CloakError::Os(e) => f.write_str(e),
        }
    }
}

pub struct Cloak {
    /// The grabbed event nodes of each hidden device.
    held: HashMap<String, Vec<File>>,
}

fn grab(node: &Path) -> std::io::Result<File> {
    let f = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(node)?;
    if unsafe { libc::ioctl(f.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

impl Cloak {
    pub fn start(_journal: &Path) -> Result<Cloak, CloakError> {
        Ok(Cloak { held: HashMap::new() })
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        self.held.get(id).is_some_and(|f| !f.is_empty())
    }

    /// Grabs what can be grabbed. A node this user may not open (some systems keep motion
    /// sensors to themselves) is left as it is.
    pub fn hide(&mut self, ids: &[String]) -> Result<(), CloakError> {
        for id in ids {
            if self.held.contains_key(id) {
                continue;
            }
            let files: Vec<File> = event_nodes(id).iter().filter_map(|n| grab(n).ok()).collect();
            self.held.insert(id.clone(), files);
        }
        Ok(())
    }

    /// Closing a grabbing handle ends its grab.
    pub fn reveal(&mut self, ids: &[String]) -> Result<(), CloakError> {
        for id in ids {
            self.held.remove(id);
        }
        Ok(())
    }

    pub fn reveal_all(&mut self) -> Result<(), CloakError> {
        self.held.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn grab_request_number() {
        assert_eq!(super::EVIOCGRAB, 0x4004_4590);
    }
}
