//! Rumble written straight to the event node of a controller another program makes through
//! uinput (a userspace driver, a remapper). Its force feedback goes through that program, which
//! may take up to 30 s to answer, and SDL holds its lock on every controller while it waits. Done
//! here, on the controller's own rumble thread, only that controller waits.

use super::uinput::{event, write_events};
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const EVIOCSFF: libc::c_ulong = super::ioc::iow::<libc::ff_effect>(b'E', 0x80);
const EV_FF: u16 = 0x15;
const FF_RUMBLE: u16 = 0x50;

/// An event node of a device made through uinput: those live under `/sys/devices/virtual/input`,
/// while a kernel driver's, Bluetooth ones included, hang under their bus or `uhid`.
pub fn is_userspace(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/dev/input/").filter(|n| n.starts_with("event")) else { return false };
    std::fs::canonicalize(format!("/sys/class/input/{name}/device")).is_ok_and(|d| d.starts_with("/sys/devices/virtual/input"))
}

pub struct Rumble {
    file: File,
    /// The effect uploaded to the device, once there is one.
    id: Option<i16>,
}

impl Rumble {
    pub fn open(path: &str) -> Option<Rumble> {
        let file = OpenOptions::new().read(true).write(true).custom_flags(libc::O_CLOEXEC).open(Path::new(path)).ok()?;
        Some(Rumble { file, id: None })
    }

    /// Same meaning as SDL's: `low` is the large motor, `high` the small one, for `ms`.
    pub fn set(&mut self, low: u16, high: u16, ms: u32) -> std::io::Result<()> {
        if low == 0 && high == 0 {
            return match self.id {
                Some(id) => self.play(id, false),
                None => Ok(()),
            };
        }
        let mut effect: libc::ff_effect = unsafe { std::mem::zeroed() };
        effect.type_ = FF_RUMBLE;
        effect.id = self.id.unwrap_or(-1);
        effect.replay.length = u16::try_from(ms).unwrap_or(u16::MAX);
        // The union starts with `ff_rumble_effect`: strong, then weak magnitude.
        let magnitudes = effect.u.as_mut_ptr().cast::<u16>();
        unsafe {
            *magnitudes = low;
            *magnitudes.add(1) = high;
        }
        if unsafe { libc::ioctl(self.file.as_raw_fd(), EVIOCSFF, &mut effect) } < 0 {
            self.id = None;
            return Err(std::io::Error::last_os_error());
        }
        self.id = Some(effect.id);
        self.play(effect.id, true)
    }

    fn play(&self, id: i16, on: bool) -> std::io::Result<()> {
        write_events(&self.file, &[event(EV_FF, id as u16, i32::from(on))])
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn upload_request_number() {
        assert_eq!(super::EVIOCSFF, 0x4030_4580);
    }

    #[test]
    fn only_event_nodes_can_be_userspace() {
        assert!(!super::is_userspace("/dev/hidraw0"));
        assert!(!super::is_userspace("/dev/input/js0"));
    }
}
