//! A controller's own HID reports, read alongside SDL while a report is open on it. Where SDL
//! leaves a part out, such as triggers a driver looks for in the wrong place, the bytes that moved
//! show where the controller sends it.

use crate::mapping::HidBytes;
use sdl3_sys::everything::*;
use std::ffi::CString;

/// The most reports taken in one look, so a chatty controller cannot hold the engine up.
const MOST_PER_LOOK: usize = 64;

pub struct HidTap {
    dev: *mut SDL_hid_device,
}

impl HidTap {
    /// Opens the device at SDL's path for the controller, beside SDL's own handle: each handle
    /// gets every report. `None` for a controller with no HID device of its own, such as one
    /// read through XInput.
    pub fn open(path: &str) -> Option<HidTap> {
        let path = CString::new(path).ok()?;
        unsafe {
            if SDL_hid_init() != 0 {
                return None;
            }
            let dev = SDL_hid_open_path(path.as_ptr());
            if dev.is_null() {
                SDL_hid_exit();
                return None;
            }
            Some(HidTap { dev })
        }
    }

    /// Takes in what the controller sent since the last look, without waiting. Whether `out`
    /// shows anything new.
    pub fn read(&mut self, out: &mut HidBytes) -> bool {
        let mut buf = [0u8; 128];
        let mut changed = false;
        for _ in 0..MOST_PER_LOOK {
            let n = unsafe { SDL_hid_read_timeout(self.dev, buf.as_mut_ptr(), buf.len(), 0) };
            if n <= 0 {
                break;
            }
            changed |= out.feed(&buf[..n as usize]);
        }
        changed
    }
}

impl Drop for HidTap {
    fn drop(&mut self) {
        unsafe {
            SDL_hid_close(self.dev);
            SDL_hid_exit();
        }
    }
}
