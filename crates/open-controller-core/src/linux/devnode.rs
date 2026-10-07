//! What sysfs says about a device SDL opened: the HID device behind a `hidraw` node, its event
//! nodes, its bus, and whether it is one of OpenController's own virtual controllers.

use std::path::{Path, PathBuf};

/// `/dev/hidraw3` -> `hidraw3`, `/dev/input/event7` -> `event7`.
fn node(path: &str, prefix: &str) -> Option<String> {
    let name = path.strip_prefix("/dev/")?.trim_start_matches("input/");
    name.starts_with(prefix).then(|| name.to_string())
}

/// The HID device a `hidraw` node belongs to (`/sys/devices/.../0005:054C:0CE6.0009`), which is
/// what hiding works on. `None` for a controller SDL reads through its event node: grabbing that
/// node would take it from SDL too.
pub fn instance_id(path: &str) -> Option<String> {
    let n = node(path, "hidraw")?;
    let dir = std::fs::canonicalize(format!("/sys/class/hidraw/{n}/device")).ok()?;
    Some(dir.to_string_lossy().into_owned())
}

/// The event nodes the kernel made for a HID device: the gamepad, and for PlayStation and
/// Nintendo controllers their motion sensors and touchpad.
pub fn event_nodes(hid_device: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(inputs) = std::fs::read_dir(Path::new(hid_device).join("input")) else { return out };
    for input in inputs.flatten() {
        let Ok(entries) = std::fs::read_dir(input.path()) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("event") {
                out.push(PathBuf::from("/dev/input").join(name));
            }
        }
    }
    out.sort();
    out
}

fn read(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// One of OpenController's own virtual controllers, read back through its event node.
pub fn is_virtual(path: &str) -> bool {
    node(path, "event").and_then(|n| read(format!("/sys/class/input/{n}/device/phys"))).is_some_and(|p| p.starts_with(super::uinput::PHYS))
}

/// Connected over Bluetooth: bus 5 in the HID id or the input device's id.
pub fn is_bluetooth(path: &str) -> bool {
    const BUS_BLUETOOTH: &str = "0005";
    if let Some(n) = node(path, "hidraw") {
        return read(format!("/sys/class/hidraw/{n}/device/uevent"))
            .is_some_and(|u| u.lines().any(|l| l.strip_prefix("HID_ID=").is_some_and(|id| id.starts_with(BUS_BLUETOOTH))));
    }
    node(path, "event").and_then(|n| read(format!("/sys/class/input/{n}/device/id/bustype"))).is_some_and(|b| b == BUS_BLUETOOTH)
}

/// An Xbox controller the kernel's own driver reads, which games already see as one.
pub fn is_native(path: &str, xbox: bool) -> bool {
    xbox && path.starts_with("/dev/input/") && !is_virtual(path)
}

pub fn driver_version(_: &str) -> Option<String> {
    None
}

pub fn usb_product_name(_: u16, _: u16) -> Option<String> {
    None
}

pub fn ancestry(_: &str) -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes() {
        assert_eq!(node("/dev/hidraw3", "hidraw").as_deref(), Some("hidraw3"));
        assert_eq!(node("/dev/input/event12", "event").as_deref(), Some("event12"));
        assert_eq!(node("/dev/input/event12", "hidraw"), None);
        assert_eq!(instance_id("/dev/input/event12"), None);
    }
}
