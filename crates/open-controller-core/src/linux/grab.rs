//! Hiding a controller from games on Linux: OpenController holds an exclusive grab on every
//! event node the kernel made for it, so programs reading event devices (Wine, Proton, most
//! games) get nothing from them. SDL reads the controller through its `hidraw` node, which a
//! grab leaves alone. Grabs end with the process, so nothing stays hidden after a crash.
//!
//! The `hidraw` node can come before the event nodes: `hid-playstation` reads a few reports from
//! a DualSense before it makes them, which takes a moment over Bluetooth. A controller hidden
//! then is watched for a while, and each event node is grabbed as it appears.

use super::devnode::event_nodes;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const EVIOCGRAB: libc::c_ulong = super::ioc::iow::<libc::c_int>(b'E', 0x90);

/// How long a newly hidden controller is watched for event nodes still to come, and how often.
const WATCH: Duration = Duration::from_secs(5);
const WATCH_PERIOD: Duration = Duration::from_millis(50);

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

/// The event nodes of one hidden device that are grabbed.
struct Held {
    nodes: Vec<(PathBuf, File)>,
    /// Whether one of them is the gamepad itself, not its motion sensors or touchpad.
    gamepad: bool,
    since: Instant,
}

pub struct Cloak {
    held: HashMap<String, Held>,
}

fn grab(node: &Path) -> std::io::Result<File> {
    let f = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(node)?;
    if unsafe { libc::ioctl(f.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

/// A gamepad's event node, as opposed to the motion sensors (`INPUT_PROP_ACCELEROMETER`) or the
/// touchpad (`INPUT_PROP_POINTER`, `INPUT_PROP_BUTTONPAD`) the kernel makes beside it.
fn is_gamepad(node: &Path) -> bool {
    const SENSOR_OR_POINTER: u64 = 1 << 6 | 1 << 2 | 1 << 0;
    let Some(name) = node.file_name() else { return false };
    let props = std::fs::read_to_string(Path::new("/sys/class/input").join(name).join("device/properties")).unwrap_or_default();
    props.split_whitespace().last().and_then(|p| u64::from_str_radix(p, 16).ok()).is_none_or(|p| p & SENSOR_OR_POINTER == 0)
}

impl Held {
    /// Grabs the device's event nodes that are not held yet.
    fn catch_up(&mut self, id: &str) {
        for n in event_nodes(id) {
            if self.nodes.iter().any(|(p, _)| *p == n) {
                continue;
            }
            if let Ok(f) = grab(&n) {
                self.gamepad |= is_gamepad(&n);
                self.nodes.push((n, f));
            }
        }
    }
}

impl Cloak {
    pub fn start(_journal: &Path) -> Result<Cloak, CloakError> {
        Ok(Cloak { held: HashMap::new() })
    }

    /// Hidden once its gamepad node is grabbed.
    pub fn is_hidden(&self, id: &str) -> bool {
        self.held.get(id).is_some_and(|h| h.gamepad)
    }

    /// Grabs what can be grabbed, and keeps watching for the nodes still to come. A node this
    /// user may not open (some systems keep motion sensors to themselves) is left as it is.
    pub fn hide(&mut self, ids: &[String]) -> Result<(), CloakError> {
        for id in ids {
            let h = self.held.entry(id.clone()).or_insert_with(|| Held { nodes: Vec::new(), gamepad: false, since: Instant::now() });
            if !h.gamepad {
                h.since = Instant::now();
            }
            h.catch_up(id);
        }
        Ok(())
    }

    /// Grabs the event nodes that appeared since for the controllers still watched. Returns how
    /// long until the next look, or `None` when nothing is watched.
    pub fn follow_up(&mut self) -> Option<Duration> {
        let mut watching = false;
        for (id, h) in &mut self.held {
            if h.since.elapsed() < WATCH {
                h.catch_up(id);
                watching = true;
            }
        }
        watching.then_some(WATCH_PERIOD)
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
