//! "Start with Windows": a value in the per-user `Run` key, so no administrator rights are
//! needed. It starts the app in the notification area, without the window.

use crate::win::wide;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "OpenController";
/// The name up to 0.4.1, moved to [`VALUE`] at the next start.
const OLD_VALUE: &str = "Open Controller";

fn command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\" --minimized", exe.display()))
}

fn current() -> Option<String> {
    read(VALUE)
}

fn read(value: &str) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(KEY).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    (ok == ERROR_SUCCESS).then(|| crate::win::from_wide(&buf))
}

pub fn enabled() -> bool {
    current().is_some()
}

pub fn set(on: bool) -> bool {
    unsafe {
        if on {
            let Some(cmd) = command() else { return false };
            let data = wide(&cmd);
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                wide(KEY).as_ptr(),
                wide(VALUE).as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            ) == ERROR_SUCCESS
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), wide(VALUE).as_ptr()) == ERROR_SUCCESS || current().is_none()
        }
    }
}

/// Keeps the entry pointing at this executable if it moved (a reinstall to another folder), and
/// under its current name.
pub fn refresh() {
    if read(OLD_VALUE).is_some() {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), wide(OLD_VALUE).as_ptr()) };
        set(true);
        return;
    }
    if let (Some(now), Some(want)) = (current(), command())
        && now != want
    {
        set(true);
    }
}
