//! Small Win32 helpers shared by the driver clients.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::IO::{DeviceIoControl, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::CreateEventW;

pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// UTF-16 up to the first NUL.
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// A `REG_MULTI_SZ`-style list: NUL-separated strings ending in an empty one.
pub fn from_multi_sz(buf: &[u16]) -> Vec<String> {
    buf.split(|&c| c == 0).take_while(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
}

pub fn to_multi_sz(items: &[String]) -> Vec<u16> {
    let mut out: Vec<u16> = items.iter().flat_map(|s| s.encode_utf16().chain(Some(0))).collect();
    out.push(0);
    if items.is_empty() {
        out.push(0);
    }
    out
}

pub const fn ctl_code(device: u32, function: u32, method: u32, access: u32) -> u32 {
    (device << 16) | (access << 14) | (function << 2) | method
}

/// An owned kernel handle.
pub struct Handle(pub HANDLE);

// Kernel handles may be used from any thread; the drivers serialise what they need to.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    pub fn valid(h: HANDLE) -> Option<Handle> {
        (!h.is_null() && h != INVALID_HANDLE_VALUE).then_some(Handle(h))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// An `OVERLAPPED` with its own event, for one caller at a time. Every request waits for its
/// completion before returning, so the structure never moves while the driver holds it.
pub struct Overlapped {
    ov: OVERLAPPED,
    event: Handle,
}

unsafe impl Send for Overlapped {}

impl Overlapped {
    pub fn new() -> Overlapped {
        let event = unsafe { CreateEventW(null(), 0, 0, null()) };
        Overlapped { ov: unsafe { std::mem::zeroed() }, event: Handle::valid(event).expect("CreateEventW failed") }
    }

    /// `DeviceIoControl` on a handle opened with `FILE_FLAG_OVERLAPPED`, waiting for the result.
    /// Returns the bytes written to `output`, or the Win32 error.
    pub(crate) fn ioctl(&mut self, device: HANDLE, code: u32, input: &[u8], output: &mut [u8]) -> Result<u32, u32> {
        self.ov = unsafe { std::mem::zeroed() };
        self.ov.hEvent = self.event.0;
        let (out_ptr, out_len) = if output.is_empty() { (null_mut(), 0) } else { (output.as_mut_ptr(), output.len() as u32) };
        let (in_ptr, in_len) = if input.is_empty() { (null(), 0) } else { (input.as_ptr(), input.len() as u32) };
        unsafe {
            if DeviceIoControl(device, code, in_ptr.cast(), in_len, out_ptr.cast(), out_len, null_mut(), &mut self.ov) == 0 {
                let e = GetLastError();
                if e != ERROR_IO_PENDING {
                    return Err(e);
                }
            }
            let mut n = 0u32;
            if GetOverlappedResult(device, &self.ov, &mut n, 1) == 0 {
                return Err(GetLastError());
            }
            Ok(n)
        }
    }
}

impl Default for Overlapped {
    fn default() -> Self {
        Self::new()
    }
}

/// Synchronous `DeviceIoControl`, for handles opened without `FILE_FLAG_OVERLAPPED`.
pub(crate) fn ioctl_sync(device: HANDLE, code: u32, input: &[u8], output: &mut [u8]) -> Result<u32, u32> {
    let (out_ptr, out_len) = if output.is_empty() { (null_mut(), 0) } else { (output.as_mut_ptr(), output.len() as u32) };
    let (in_ptr, in_len) = if input.is_empty() { (null(), 0) } else { (input.as_ptr(), input.len() as u32) };
    let mut n = 0u32;
    if unsafe { DeviceIoControl(device, code, in_ptr.cast(), in_len, out_ptr.cast(), out_len, &mut n, null_mut()) } == 0 {
        return Err(unsafe { GetLastError() });
    }
    Ok(n)
}

/// The bytes of a `#[repr(C)]` request structure.
pub fn bytes_of<T: Copy>(v: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((v as *const T).cast(), std::mem::size_of::<T>()) }
}

pub fn bytes_of_mut<T: Copy>(v: &mut T) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((v as *mut T).cast(), std::mem::size_of::<T>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_sz_round_trip() {
        let items = vec!["a".to_string(), r"HID\VID_054C&PID_09CC\7&1".to_string()];
        let buf = to_multi_sz(&items);
        assert_eq!(buf.iter().rev().take(2).collect::<Vec<_>>(), [&0, &0]);
        assert_eq!(from_multi_sz(&buf), items);
        assert_eq!(from_multi_sz(&to_multi_sz(&[])), Vec::<String>::new());
        // The driver pads its answer with NULs.
        let mut padded = to_multi_sz(&items);
        padded.extend([0u16; 64]);
        assert_eq!(from_multi_sz(&padded), items);
    }

    #[test]
    fn ioctl_codes() {
        // Values from ViGEmBus's BusShared.h and DS4Windows's HidHide client.
        assert_eq!(ctl_code(0x2A, 0x801, 0, 2), 0x2AA004);
        assert_eq!(ctl_code(0x2A, 0xA01, 0, 3), 0x2AE804);
        assert_eq!(ctl_code(0x8001, 0x800, 0, 1), 0x8001_6000);
        assert_eq!(ctl_code(0x8001, 0x807, 0, 1), 0x8001_601C);
    }
}
