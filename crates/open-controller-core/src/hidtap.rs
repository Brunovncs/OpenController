//! A controller's own HID reports, read alongside SDL while a report is open on it. Where SDL
//! leaves a part out, such as triggers a driver looks for in the wrong place, the bytes that moved
//! show where the controller sends it.
//!
//! On Windows every interface of the controller's USB device is read, not only the one SDL opened:
//! a receiver SDL reads as a plain gamepad (Flydigi's 2.4G receiver) sends its maker's full report
//! on another interface.

use crate::mapping::{HidBytes, HidSource};
use crate::platform::devnode;
use sdl3_sys::everything::*;
use std::ffi::{CStr, CString};

/// The most reports taken in one look, so a chatty controller cannot hold the engine up.
const MOST_PER_LOOK: usize = 64;
/// The most interfaces opened: a controller has a few, a receiver for several pads a few more.
const MOST_INTERFACES: usize = 6;

pub struct HidTap {
    devs: Vec<(Option<HidSource>, *mut SDL_hid_device)>,
}

impl HidTap {
    /// Opens the device at SDL's path for the controller, beside SDL's own handle (each handle gets
    /// every report), and the other interfaces of the same USB device. `None` for a controller with
    /// no HID device of its own, such as one read through XInput.
    pub fn open(path: &str, vendor: u16, product: u16) -> Option<HidTap> {
        unsafe {
            if SDL_hid_init() != 0 {
                return None;
            }
        }
        let mut devs = Vec::new();
        let own = devnode::usb_device(path);
        let mut opened_sdls = false;
        for (p, source) in interfaces(vendor, product) {
            let sdls = p.eq_ignore_ascii_case(path);
            let sibling = own.is_some() && devnode::usb_device(&p) == own;
            if (sdls || sibling)
                && devs.len() < MOST_INTERFACES
                && let Some(d) = open_path(&p)
            {
                opened_sdls |= sdls;
                devs.push((Some(source), d));
            }
        }
        // SDL's path may not be among HIDAPI's (a Raw Input collection): opened as it is.
        if !opened_sdls && let Some(d) = open_path(path) {
            devs.insert(0, (None, d));
        }
        if devs.is_empty() {
            unsafe { SDL_hid_exit() };
            return None;
        }
        Some(HidTap { devs })
    }

    /// Takes in what the controller sent since the last look, without waiting. Whether `out`
    /// shows anything new.
    pub fn read(&mut self, out: &mut HidBytes) -> bool {
        let mut buf = [0u8; 128];
        let mut changed = false;
        // Told apart by interface only when there is more than one.
        let many = self.devs.len() > 1;
        for &(source, dev) in &self.devs {
            for _ in 0..MOST_PER_LOOK {
                let n = unsafe { SDL_hid_read_timeout(dev, buf.as_mut_ptr(), buf.len(), 0) };
                if n <= 0 {
                    break;
                }
                changed |= out.feed_from(source.filter(|_| many), &buf[..n as usize]);
            }
        }
        changed
    }
}

fn open_path(path: &str) -> Option<*mut SDL_hid_device> {
    let path = CString::new(path).ok()?;
    let dev = unsafe { SDL_hid_open_path(path.as_ptr()) };
    (!dev.is_null()).then_some(dev)
}

/// Every HID interface with these ids, by path, with its number and usage page. HIDAPI must be
/// initialized.
pub(crate) fn interfaces(vendor: u16, product: u16) -> Vec<(String, HidSource)> {
    let mut out = Vec::new();
    unsafe {
        let list = SDL_hid_enumerate(vendor, product);
        let mut d = list;
        while !d.is_null() {
            let info = &*d;
            if !info.path.is_null() {
                let path = CStr::from_ptr(info.path).to_string_lossy().into_owned();
                out.push((path, HidSource { interface: info.interface_number, usage_page: info.usage_page }));
            }
            d = info.next;
        }
        if !list.is_null() {
            SDL_hid_free_enumeration(list);
        }
    }
    out
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// On this machine's own devices: every interface of a composite USB device (`&MI_`) is found
    /// under one USB device, the same for its siblings and never another unit's.
    #[test]
    fn interfaces_group_by_their_usb_device() {
        unsafe { SDL_hid_init() };
        let all = interfaces(0, 0);
        unsafe { SDL_hid_exit() };
        let mut groups: std::collections::BTreeMap<String, Vec<i32>> = Default::default();
        for (path, source) in &all {
            let usb = devnode::usb_device(path);
            if path.to_ascii_uppercase().contains("&MI_") {
                let usb = usb.unwrap_or_else(|| panic!("no USB device above {path}"));
                assert!(usb.to_ascii_uppercase().starts_with("USB\\VID_") && !usb.contains("&MI_"), "{usb}");
                groups.entry(usb).or_default().push(source.interface);
            }
        }
        println!("{groups:#?}");
        for (usb, ifs) in &groups {
            let ids = usb.to_ascii_uppercase();
            let ids = &ids[4..21];
            assert!(
                all.iter()
                    .filter(|(p, _)| devnode::usb_device(p).as_deref() == Some(usb.as_str()))
                    .all(|(p, _)| p.to_ascii_uppercase().contains(ids)),
                "{usb}: {ifs:?}"
            );
        }
    }
}

impl Drop for HidTap {
    fn drop(&mut self) {
        unsafe {
            for &(_, dev) in &self.devs {
                SDL_hid_close(dev);
            }
            SDL_hid_exit();
        }
    }
}
