//! A client for ViGEmBus, the bus driver that creates virtual Xbox 360 controllers.
//!
//! It speaks the driver's IOCTL interface directly (`BusShared.h` in ViGEmClient) instead of
//! loading ViGEmClient.dll. Every virtual controller belongs to the handle that plugged it in:
//! when the process exits or crashes, Windows closes the handle and the bus unplugs them, so a
//! crash never leaves phantom controllers behind.

use crate::mapping::XusbReport;
pub use crate::win::Overlapped;
use crate::win::{Handle, bytes_of, bytes_of_mut, ctl_code, interface_paths, wide};
use crossbeam_channel::Sender;
use std::ptr::null;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_DEVICE_HARDWARE_ERROR, ERROR_FILE_NOT_FOUND, ERROR_INVALID_PARAMETER, ERROR_OPERATION_ABORTED, GENERIC_READ,
    GENERIC_WRITE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_NO_BUFFERING, FILE_FLAG_OVERLAPPED, FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::CancelIoEx;
use windows_sys::core::GUID;

const GUID_DEVINTERFACE_BUSENUM_VIGEM: GUID = GUID::from_u128(0x96E42B22_F5E9_42F8_B043_ED0F932F014F);

const FILE_DEVICE_BUS_EXTENDER: u32 = 0x2A;
const WRITE: u32 = 2;
const READ_WRITE: u32 = 3;
const fn ioctl(index: u32, access: u32) -> u32 {
    ctl_code(FILE_DEVICE_BUS_EXTENDER, 0x801 + index, 0, access)
}
const IOCTL_VIGEM_PLUGIN_TARGET: u32 = ioctl(0x000, WRITE);
const IOCTL_VIGEM_UNPLUG_TARGET: u32 = ioctl(0x001, WRITE);
const IOCTL_VIGEM_CHECK_VERSION: u32 = ioctl(0x002, WRITE);
const IOCTL_VIGEM_WAIT_DEVICE_READY: u32 = ioctl(0x003, WRITE);
const IOCTL_XUSB_REQUEST_NOTIFICATION: u32 = ioctl(0x200, READ_WRITE);
const IOCTL_XUSB_SUBMIT_REPORT: u32 = ioctl(0x201, WRITE);
const IOCTL_XUSB_GET_USER_INDEX: u32 = ioctl(0x206, READ_WRITE);

const VIGEM_COMMON_VERSION: u32 = 0x0001;
const TARGET_XBOX360_WIRED: u32 = 0;
/// The bus numbers its children from 1; ViGEmClient tries up to this many.
const MAX_TARGETS: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy)]
struct PluginTarget {
    size: u32,
    serial: u32,
    target_type: u32,
    vendor_id: u16,
    product_id: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SerialRequest {
    size: u32,
    serial: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XusbSubmitReport {
    size: u32,
    serial: u32,
    report: XusbReport,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XusbNotification {
    size: u32,
    serial: u32,
    large_motor: u8,
    small_motor: u8,
    led_number: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XusbUserIndex {
    size: u32,
    serial: u32,
    user_index: u32,
}

fn serial_request(serial: u32) -> SerialRequest {
    SerialRequest { size: size_of::<SerialRequest>() as u32, serial }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    /// The driver is not installed, or its device is disabled.
    NotInstalled,
    /// The driver answered with an interface version this client does not speak.
    VersionMismatch,
    Os(u32),
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BusError::NotInstalled => f.write_str("ViGEmBus is not installed"),
            BusError::VersionMismatch => f.write_str("ViGEmBus speaks an unsupported interface version"),
            BusError::Os(e) => write!(f, "ViGEmBus error {e}"),
        }
    }
}

impl BusError {
    /// How the settings show it.
    pub fn driver(&self) -> crate::engine::Driver {
        match self {
            BusError::NotInstalled => crate::engine::Driver::Missing,
            _ => crate::engine::Driver::Failed(self.to_string()),
        }
    }
}

/// Rumble the game asked of a virtual controller, as XInput motor speeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub serial: u32,
    pub large_motor: u8,
    pub small_motor: u8,
}

pub struct Bus {
    handle: Handle,
    /// The device interface path, which also names the bus device for its driver version.
    pub path: String,
}

impl Bus {
    pub fn connect() -> Result<Bus, BusError> {
        let mut last = BusError::NotInstalled;
        for path in interface_paths(&GUID_DEVINTERFACE_BUSENUM_VIGEM) {
            let raw = unsafe {
                CreateFileW(
                    wide(&path).as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH | FILE_FLAG_OVERLAPPED,
                    std::ptr::null_mut(),
                )
            };
            let Some(handle) = Handle::valid(raw) else {
                let e = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                last = if e == ERROR_FILE_NOT_FOUND { BusError::NotInstalled } else { BusError::Os(e) };
                continue;
            };
            let check = SerialRequest { size: size_of::<SerialRequest>() as u32, serial: VIGEM_COMMON_VERSION };
            match Overlapped::new().ioctl(handle.0, IOCTL_VIGEM_CHECK_VERSION, bytes_of(&check), &mut []) {
                Ok(_) => return Ok(Bus { handle, path }),
                Err(_) => last = BusError::VersionMismatch,
            }
        }
        Err(last)
    }

    /// Plugs in a virtual Xbox 360 controller and waits until it accepts reports. Returns its
    /// serial number on the bus.
    pub fn plug_x360(&self, io: &mut Overlapped) -> Result<u32, BusError> {
        let mut last = BusError::Os(ERROR_ACCESS_DENIED);
        // Serials in use, by this or another program, are refused; the first free one is taken.
        for serial in 1..=MAX_TARGETS {
            let plug = PluginTarget {
                size: size_of::<PluginTarget>() as u32,
                serial,
                target_type: TARGET_XBOX360_WIRED,
                vendor_id: 0x045E,
                product_id: 0x028E,
            };
            if let Err(e) = io.ioctl(self.handle.0, IOCTL_VIGEM_PLUGIN_TARGET, bytes_of(&plug), &mut []) {
                last = BusError::Os(e);
                continue;
            }
            return match io.ioctl(self.handle.0, IOCTL_VIGEM_WAIT_DEVICE_READY, bytes_of(&serial_request(serial)), &mut []) {
                // Drivers before 1.17 have no wait request. The driver gives up waiting after
                // 1 s, which the first controller on a machine can exceed while Windows installs
                // the Xbox 360 driver for it; it finishes on its own, so it is kept either way.
                Ok(_) | Err(ERROR_INVALID_PARAMETER | ERROR_DEVICE_HARDWARE_ERROR) => Ok(serial),
                Err(e) => {
                    let _ = self.unplug(io, serial);
                    Err(BusError::Os(e))
                }
            };
        }
        Err(last)
    }

    pub fn unplug(&self, io: &mut Overlapped, serial: u32) -> Result<(), u32> {
        io.ioctl(self.handle.0, IOCTL_VIGEM_UNPLUG_TARGET, bytes_of(&serial_request(serial)), &mut []).map(drop)
    }

    pub fn submit(&self, io: &mut Overlapped, serial: u32, report: &XusbReport) -> Result<(), u32> {
        let req = XusbSubmitReport { size: size_of::<XusbSubmitReport>() as u32, serial, report: *report };
        io.ioctl(self.handle.0, IOCTL_XUSB_SUBMIT_REPORT, bytes_of(&req), &mut []).map(drop)
    }

    /// The XInput slot (0 to 3) the driver believes the controller has. Not to be trusted: with
    /// another XInput controller in slot 0, a new one in slot 1 is reported as 0. The engine
    /// finds the slot through XInput instead (see `xinput`).
    pub fn user_index(&self, io: &mut Overlapped, serial: u32) -> Result<u8, u32> {
        let mut req = XusbUserIndex { size: size_of::<XusbUserIndex>() as u32, serial, user_index: 0 };
        let input = req;
        io.ioctl(self.handle.0, IOCTL_XUSB_GET_USER_INDEX, bytes_of(&input), bytes_of_mut(&mut req))?;
        Ok(req.user_index as u8)
    }

    /// Starts a thread that waits for the game's rumble requests to one controller. It ends
    /// when the controller is unplugged or [`Bus::cancel_all`] is called.
    pub fn listen(self: &Arc<Bus>, serial: u32, tx: Sender<Feedback>) -> JoinHandle<()> {
        let bus = self.clone();
        std::thread::Builder::new()
            .name(format!("vigem-feedback-{serial}"))
            .spawn(move || {
                let mut io = Overlapped::new();
                let mut failures = 0;
                loop {
                    let mut req = XusbNotification {
                        size: size_of::<XusbNotification>() as u32,
                        serial,
                        large_motor: 0,
                        small_motor: 0,
                        led_number: 0,
                    };
                    let input = req;
                    match io.ioctl(bus.handle.0, IOCTL_XUSB_REQUEST_NOTIFICATION, bytes_of(&input), bytes_of_mut(&mut req)) {
                        Ok(_) => {
                            failures = 0;
                            let fb = Feedback { serial, large_motor: req.large_motor, small_motor: req.small_motor };
                            if tx.send(fb).is_err() {
                                return;
                            }
                        }
                        Err(ERROR_OPERATION_ABORTED | ERROR_ACCESS_DENIED) => return,
                        Err(_) => {
                            // The controller may still be starting; give up if it never does.
                            failures += 1;
                            if failures > 40 {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            })
            .expect("could not start the feedback thread")
    }

    /// Cancels every pending request on the bus handle, which ends the feedback threads.
    pub fn cancel_all(&self) {
        unsafe { CancelIoEx(self.handle.0, null()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layouts_match_the_driver() {
        assert_eq!(size_of::<PluginTarget>(), 16);
        assert_eq!(size_of::<SerialRequest>(), 8);
        assert_eq!(size_of::<XusbSubmitReport>(), 20);
        assert_eq!(size_of::<XusbNotification>(), 12);
        assert_eq!(size_of::<XusbUserIndex>(), 12);
    }

    #[test]
    fn ioctl_codes_match_bus_shared_h() {
        assert_eq!(IOCTL_VIGEM_PLUGIN_TARGET, 0x2AA004);
        assert_eq!(IOCTL_VIGEM_UNPLUG_TARGET, 0x2AA008);
        assert_eq!(IOCTL_VIGEM_CHECK_VERSION, 0x2AA00C);
        assert_eq!(IOCTL_VIGEM_WAIT_DEVICE_READY, 0x2AA010);
        assert_eq!(IOCTL_XUSB_REQUEST_NOTIFICATION, 0x2AE804);
        assert_eq!(IOCTL_XUSB_SUBMIT_REPORT, 0x2AA808);
        assert_eq!(IOCTL_XUSB_GET_USER_INDEX, 0x2AE81C);
    }
}
