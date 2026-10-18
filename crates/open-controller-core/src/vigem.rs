//! A client for ViGEmBus, the bus driver that creates virtual Xbox 360 controllers, and
//! DualShock 4s for controllers set to show themselves as one.
//!
//! It speaks the driver's IOCTL interface directly (`BusShared.h` in ViGEmClient) instead of
//! loading ViGEmClient.dll. Every virtual controller belongs to the handle that plugged it in:
//! when the process exits or crashes, Windows closes the handle and the bus unplugs them, so a
//! crash never leaves phantom controllers behind.

use crate::ds4::{self, Calibration, Ds4Report};
use crate::mapping::XusbReport;
pub use crate::win::Overlapped;
use crate::win::{Handle, bytes_of, bytes_of_mut, ctl_code, interface_paths, wide};
use crossbeam_channel::Sender;
use std::ptr::null;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_DEVICE_HARDWARE_ERROR, ERROR_DEVICE_NOT_CONNECTED, ERROR_DEVICE_REMOVED, ERROR_FILE_NOT_FOUND,
    ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER, ERROR_NO_SUCH_DEVICE, ERROR_OPERATION_ABORTED, GENERIC_READ, GENERIC_WRITE,
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
const IOCTL_DS4_SUBMIT_REPORT: u32 = ioctl(0x202, WRITE);
const IOCTL_DS4_REQUEST_NOTIFICATION: u32 = ioctl(0x203, WRITE);

const VIGEM_COMMON_VERSION: u32 = 0x0001;
const TARGET_XBOX360_WIRED: u32 = 0;
const TARGET_DUALSHOCK4_WIRED: u32 = 2;
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

/// `DS4_SUBMIT_REPORT_EX`: the full 63-byte report after its id, motion and touchpad included,
/// which drivers from 1.17 take. Packed, as the header declares it.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Ds4SubmitReportEx {
    size: u32,
    serial: u32,
    report: [u8; 63],
}

/// `DS4_REQUEST_NOTIFICATION`: rumble and the light bar colour a game asks for.
#[repr(C)]
#[derive(Clone, Copy)]
struct Ds4Notification {
    size: u32,
    serial: u32,
    small_motor: u8,
    large_motor: u8,
    lightbar: [u8; 3],
}

/// The 63 bytes of a DualShock 4 report after its id (`DS4_REPORT_EX`), with the sensors scaled
/// to ViGEmBus's calibration report. `timestamp` counts in units of 16/3 µs, as the controller's.
fn ds4_report(r: &Ds4Report, cal: &Calibration, timestamp: u16) -> [u8; 63] {
    let mut b = [0u8; 63];
    b[0] = r.lx;
    b[1] = r.ly;
    b[2] = r.rx;
    b[3] = r.ry;
    b[4..6].copy_from_slice(&(r.buttons | u16::from(ds4::hat(r.dpad))).to_le_bytes());
    b[6] = u8::from(r.ps) | u8::from(r.touchpad) << 1;
    b[7] = r.l2;
    b[8] = r.r2;
    b[9..11].copy_from_slice(&timestamp.to_le_bytes());
    let (gyro, accel) = cal.encode(&r.motion);
    for i in 0..3 {
        b[12 + 2 * i..14 + 2 * i].copy_from_slice(&gyro[i].to_le_bytes());
        b[18 + 2 * i..20 + 2 * i].copy_from_slice(&accel[i].to_le_bytes());
    }
    b[29] = r.battery;
    // No finger on the touchpad: both touches marked up.
    b[34] = 0x80;
    b[38] = 0x80;
    b
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

/// Whether a request's error says the bus device behind the handle is gone: removed, restarted
/// or updated while the handle was open. The handle is of no use any more; a new one is needed.
pub fn gone(code: u32) -> bool {
    matches!(code, ERROR_INVALID_HANDLE | ERROR_DEVICE_NOT_CONNECTED | ERROR_DEVICE_REMOVED | ERROR_NO_SUCH_DEVICE)
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
    /// What the DualShock 4s' timestamps count from, and how they scale their sensors.
    started: Instant,
    calibration: Calibration,
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
                Ok(_) => {
                    let calibration = Calibration::from_usb_report(&ds4::VIGEM_CALIBRATION);
                    return Ok(Bus { handle, path, started: Instant::now(), calibration });
                }
                Err(_) => last = BusError::VersionMismatch,
            }
        }
        Err(last)
    }

    /// Plugs in a virtual Xbox 360 controller and waits until it accepts reports. Returns its
    /// serial number on the bus.
    pub fn plug_x360(&self, io: &mut Overlapped) -> Result<u32, BusError> {
        self.plug(io, TARGET_XBOX360_WIRED, 0x045E, 0x028E)
    }

    /// Plugs in a virtual DualShock 4 (Sony's first model, as ViGEmBus makes it).
    pub fn plug_ds4(&self, io: &mut Overlapped) -> Result<u32, BusError> {
        self.plug(io, TARGET_DUALSHOCK4_WIRED, 0x054C, 0x05C4)
    }

    fn plug(&self, io: &mut Overlapped, target_type: u32, vendor_id: u16, product_id: u16) -> Result<u32, BusError> {
        let mut last = BusError::Os(ERROR_ACCESS_DENIED);
        // Serials in use, by this or another program, are refused; the first free one is taken.
        for serial in 1..=MAX_TARGETS {
            let plug = PluginTarget { size: size_of::<PluginTarget>() as u32, serial, target_type, vendor_id, product_id };
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

    pub fn submit_ds4(&self, io: &mut Overlapped, serial: u32, report: &Ds4Report) -> Result<(), u32> {
        let timestamp = (self.started.elapsed().as_micros() * 3 / 16) as u16;
        let req = Ds4SubmitReportEx {
            size: size_of::<Ds4SubmitReportEx>() as u32,
            serial,
            report: ds4_report(report, &self.calibration, timestamp),
        };
        io.ioctl(self.handle.0, IOCTL_DS4_SUBMIT_REPORT, bytes_of(&req), &mut []).map(drop)
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
        self.wait_for_feedback(serial, tx, move |bus, io| {
            let mut req =
                XusbNotification { size: size_of::<XusbNotification>() as u32, serial, large_motor: 0, small_motor: 0, led_number: 0 };
            let input = req;
            io.ioctl(bus.handle.0, IOCTL_XUSB_REQUEST_NOTIFICATION, bytes_of(&input), bytes_of_mut(&mut req))?;
            Ok((req.large_motor, req.small_motor))
        })
    }

    /// The same for a DualShock 4, whose requests also carry the light bar colour, not used.
    pub fn listen_ds4(self: &Arc<Bus>, serial: u32, tx: Sender<Feedback>) -> JoinHandle<()> {
        self.wait_for_feedback(serial, tx, move |bus, io| {
            let mut req =
                Ds4Notification { size: size_of::<Ds4Notification>() as u32, serial, small_motor: 0, large_motor: 0, lightbar: [0; 3] };
            let input = req;
            io.ioctl(bus.handle.0, IOCTL_DS4_REQUEST_NOTIFICATION, bytes_of(&input), bytes_of_mut(&mut req))?;
            Ok((req.large_motor, req.small_motor))
        })
    }

    /// Asks `next` for the next rumble request (large motor, small motor) until the controller
    /// goes, passing each on.
    fn wait_for_feedback(
        self: &Arc<Bus>,
        serial: u32,
        tx: Sender<Feedback>,
        next: impl Fn(&Bus, &mut Overlapped) -> Result<(u8, u8), u32> + Send + 'static,
    ) -> JoinHandle<()> {
        let bus = self.clone();
        std::thread::Builder::new()
            .name(format!("vigem-feedback-{serial}"))
            .spawn(move || {
                let mut io = Overlapped::new();
                let mut failures = 0;
                loop {
                    match next(&bus, &mut io) {
                        Ok((large_motor, small_motor)) => {
                            failures = 0;
                            if tx.send(Feedback { serial, large_motor, small_motor }).is_err() {
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
        assert_eq!(size_of::<Ds4SubmitReportEx>(), 71);
        assert_eq!(size_of::<Ds4Notification>(), 16);
    }

    #[test]
    fn ds4_reports_have_the_controllers_layout() {
        use crate::ds4::{Motion, dpad};
        let r = Ds4Report {
            lx: 1,
            ly: 2,
            rx: 3,
            ry: 4,
            buttons: ds4::button::CROSS | ds4::button::R3,
            dpad: dpad::DOWN | dpad::LEFT,
            ps: true,
            touchpad: true,
            l2: 9,
            r2: 10,
            battery: 0x15,
            motion: Motion::REST,
        };
        let cal = Calibration::VIIPER;
        let b = ds4_report(&r, &cal, 0x1234);
        assert_eq!(&b[0..4], &[1, 2, 3, 4]);
        assert_eq!(u16::from_le_bytes([b[4], b[5]]), 0x8020 | 5, "buttons, with the hat down-left");
        assert_eq!((b[6], b[7], b[8]), (3, 9, 10));
        assert_eq!(&b[9..11], &[0x34, 0x12]);
        // At rest flat: no rotation, and 1 g up, which is Y.
        assert_eq!(&b[12..18], &[0; 6]);
        assert_eq!(i16::from_le_bytes([b[20], b[21]]), 8192);
        assert_eq!(b[29], 0x15);
        assert_eq!((b[34], b[38]), (0x80, 0x80), "no finger on the touchpad");
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
        assert_eq!(IOCTL_DS4_SUBMIT_REPORT, 0x2AA80C);
        assert_eq!(IOCTL_DS4_REQUEST_NOTIFICATION, 0x2AA810);
    }
}
