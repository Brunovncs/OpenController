//! Virtual Xbox 360 controllers made with the kernel's uinput. Each is shaped like the device the
//! `xpad` driver makes for a real one (same ids, name, buttons and axes), so SDL, Wine and games
//! take it for an Xbox 360 controller. Rumble comes back as force-feedback effects the game
//! uploads and plays, read on a thread per controller.

use super::ioc::{io, iow, iowr};
use crate::engine::Driver;
use crate::mapping::{XusbReport, xusb};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const PATH: &str = "/dev/uinput";
/// What a virtual controller's `phys` starts with, to tell it from a real one.
pub const PHYS: &str = "open-controller/";

pub(crate) const UI_DEV_CREATE: libc::c_ulong = io(b'U', 1);
pub(crate) const UI_DEV_DESTROY: libc::c_ulong = io(b'U', 2);
pub(crate) const UI_DEV_SETUP: libc::c_ulong = iow::<libc::uinput_setup>(b'U', 3);
const UI_ABS_SETUP: libc::c_ulong = iow::<libc::uinput_abs_setup>(b'U', 4);
pub(crate) const UI_SET_EVBIT: libc::c_ulong = iow::<libc::c_int>(b'U', 100);
pub(crate) const UI_SET_KEYBIT: libc::c_ulong = iow::<libc::c_int>(b'U', 101);
const UI_SET_ABSBIT: libc::c_ulong = iow::<libc::c_int>(b'U', 103);
const UI_SET_FFBIT: libc::c_ulong = iow::<libc::c_int>(b'U', 107);
const UI_SET_PHYS: libc::c_ulong = iow::<*const libc::c_char>(b'U', 108);
const UI_BEGIN_FF_UPLOAD: libc::c_ulong = iowr::<libc::uinput_ff_upload>(b'U', 200);
const UI_END_FF_UPLOAD: libc::c_ulong = iow::<libc::uinput_ff_upload>(b'U', 201);
const UI_BEGIN_FF_ERASE: libc::c_ulong = iowr::<libc::uinput_ff_erase>(b'U', 202);
const UI_END_FF_ERASE: libc::c_ulong = iow::<libc::uinput_ff_erase>(b'U', 203);

pub(crate) const EV_SYN: u16 = 0x00;
pub(crate) const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const EV_FF: u16 = 0x15;
const EV_UINPUT: u16 = 0x0101;
const UI_FF_UPLOAD: u16 = 1;
const UI_FF_ERASE: u16 = 2;
const FF_RUMBLE: u16 = 0x50;
const BUS_USB: u16 = 0x03;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

/// The buttons, in `xpad`'s order, with the XUSB bit each stands for.
const KEYS: [(u16, u16); 11] = [
    (0x130, xusb::A),              // BTN_A
    (0x131, xusb::B),              // BTN_B
    (0x133, xusb::X),              // BTN_X
    (0x134, xusb::Y),              // BTN_Y
    (0x136, xusb::LEFT_SHOULDER),  // BTN_TL
    (0x137, xusb::RIGHT_SHOULDER), // BTN_TR
    (0x13A, xusb::BACK),           // BTN_SELECT
    (0x13B, xusb::START),          // BTN_START
    (0x13C, xusb::GUIDE),          // BTN_MODE
    (0x13D, xusb::LEFT_THUMB),     // BTN_THUMBL
    (0x13E, xusb::RIGHT_THUMB),    // BTN_THUMBR
];

/// (axis, minimum, maximum, fuzz, flat), as `xpad` sets them up.
const AXES: [(u16, i32, i32, i32, i32); 8] = [
    (ABS_X, -32768, 32767, 16, 128),
    (ABS_Y, -32768, 32767, 16, 128),
    (ABS_RX, -32768, 32767, 16, 128),
    (ABS_RY, -32768, 32767, 16, 128),
    (ABS_Z, 0, 255, 0, 0),
    (ABS_RZ, 0, 255, 0, 0),
    (ABS_HAT0X, -1, 1, 0, 0),
    (ABS_HAT0Y, -1, 1, 0, 0),
];

#[derive(Debug)]
pub enum BusError {
    /// The kernel has no uinput, built in or as a module (WSL, some custom kernels): nothing
    /// to set up.
    NoKernelSupport,
    /// uinput is a module that is not loaded yet; setting up device access loads it.
    NotLoaded,
    /// `/dev/uinput` is there but this user may not open it: the device rule is not installed.
    NoAccess,
    Os(std::io::Error),
}

impl BusError {
    /// How the settings show it.
    pub fn driver(&self) -> Driver {
        match self {
            BusError::NoKernelSupport => Driver::NoKernelSupport,
            BusError::NotLoaded | BusError::NoAccess => Driver::Missing,
            BusError::Os(_) => Driver::Failed(self.to_string()),
        }
    }
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BusError::NoKernelSupport => f.write_str("this kernel has no uinput"),
            BusError::NotLoaded => f.write_str("the uinput module is not loaded"),
            BusError::NoAccess => f.write_str("the device rule is not installed: no access to /dev/uinput"),
            BusError::Os(e) => write!(f, "uinput: {e}"),
        }
    }
}

/// Without `/dev/uinput`: whether the running kernel has uinput as a module that is not loaded,
/// or has none at all. When the module lists cannot be read, the module is assumed, so the user
/// is still offered the setup, which says so if loading fails.
fn absent() -> BusError {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let release = release.trim();
    let dirs = ["/lib/modules", "/usr/lib/modules", "/run/booted-system/kernel-modules/lib/modules"];
    let lists: Vec<String> = dirs
        .iter()
        .flat_map(|d| ["modules.dep", "modules.builtin"].map(|f| Path::new(d).join(release).join(f)))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect();
    absent_from(Path::new("/sys/module/uinput").exists(), &lists)
}

fn absent_from(loaded: bool, lists: &[String]) -> BusError {
    let listed = lists.iter().flat_map(|s| s.lines()).any(|l| {
        let module = l.split(':').next().unwrap_or_default();
        module.rsplit('/').next().unwrap_or_default().starts_with("uinput.ko")
    });
    if loaded || listed || lists.is_empty() { BusError::NotLoaded } else { BusError::NoKernelSupport }
}

/// Rumble the game asked of a virtual controller, as XInput motor speeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub serial: u32,
    pub large_motor: u8,
    pub small_motor: u8,
}

/// Nothing to keep between calls here; the engine passes one on every system.
#[derive(Default)]
pub struct Io;

impl Io {
    pub fn new() -> Io {
        Io
    }
}

/// One virtual controller. Dropping the last reference removes it.
struct Pad {
    file: File,
    stop: AtomicBool,
}

impl Drop for Pad {
    fn drop(&mut self) {
        unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
    }
}

pub struct Bus {
    pub path: String,
    pads: Mutex<HashMap<u32, Arc<Pad>>>,
    next: AtomicU32,
}

fn open() -> Result<File, BusError> {
    OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(PATH).map_err(|e| match e.kind() {
        ErrorKind::NotFound => absent(),
        ErrorKind::PermissionDenied => BusError::NoAccess,
        _ => BusError::Os(e),
    })
}

fn check(r: libc::c_int) -> Result<(), BusError> {
    if r < 0 { Err(BusError::Os(std::io::Error::last_os_error())) } else { Ok(()) }
}

fn set_bit(fd: i32, request: libc::c_ulong, bit: u16) -> Result<(), BusError> {
    check(unsafe { libc::ioctl(fd, request, libc::c_int::from(bit)) })
}

/// Declares an Xbox 360 controller on a fresh uinput handle and creates it.
fn create(file: &File, serial: u32) -> Result<(), BusError> {
    let fd = file.as_raw_fd();
    for ev in [EV_KEY, EV_ABS, EV_FF] {
        set_bit(fd, UI_SET_EVBIT, ev)?;
    }
    for (key, _) in KEYS {
        set_bit(fd, UI_SET_KEYBIT, key)?;
    }
    for (code, minimum, maximum, fuzz, flat) in AXES {
        set_bit(fd, UI_SET_ABSBIT, code)?;
        let setup = libc::uinput_abs_setup { code, absinfo: libc::input_absinfo { value: 0, minimum, maximum, fuzz, flat, resolution: 0 } };
        check(unsafe { libc::ioctl(fd, UI_ABS_SETUP, &setup) })?;
    }
    set_bit(fd, UI_SET_FFBIT, FF_RUMBLE)?;
    let phys = CString::new(format!("{PHYS}{serial}")).expect("no nul");
    check(unsafe { libc::ioctl(fd, UI_SET_PHYS, phys.as_ptr()) })?;
    let mut setup: libc::uinput_setup = unsafe { std::mem::zeroed() };
    setup.id = libc::input_id { bustype: BUS_USB, vendor: 0x045E, product: 0x028E, version: 0x0110 };
    for (dst, &src) in setup.name.iter_mut().zip(b"Microsoft X-Box 360 pad") {
        *dst = src as libc::c_char;
    }
    setup.ff_effects_max = 16;
    check(unsafe { libc::ioctl(fd, UI_DEV_SETUP, &setup) })?;
    check(unsafe { libc::ioctl(fd, UI_DEV_CREATE) })
}

pub(crate) fn event(kind: u16, code: u16, value: i32) -> libc::input_event {
    libc::input_event { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, type_: kind, code, value }
}

pub(crate) fn write_events(file: &File, events: &[libc::input_event]) -> std::io::Result<()> {
    let bytes = unsafe { std::slice::from_raw_parts(events.as_ptr().cast::<u8>(), size_of_val(events)) };
    (&*file).write_all(bytes)
}

/// The events that bring a virtual controller to `r`. Y grows downwards on Linux; `xpad` flips
/// it the same way (`~y`), which keeps -32768 and 32767 at the ends.
fn events_for(r: &XusbReport) -> Vec<libc::input_event> {
    let mut v = Vec::with_capacity(KEYS.len() + AXES.len() + 1);
    for (key, bit) in KEYS {
        v.push(event(EV_KEY, key, i32::from(r.buttons & bit != 0)));
    }
    let pressed = |bit| i32::from(r.buttons & bit != 0);
    v.push(event(EV_ABS, ABS_X, r.thumb_lx.into()));
    v.push(event(EV_ABS, ABS_Y, (!r.thumb_ly).into()));
    v.push(event(EV_ABS, ABS_RX, r.thumb_rx.into()));
    v.push(event(EV_ABS, ABS_RY, (!r.thumb_ry).into()));
    v.push(event(EV_ABS, ABS_Z, r.left_trigger.into()));
    v.push(event(EV_ABS, ABS_RZ, r.right_trigger.into()));
    v.push(event(EV_ABS, ABS_HAT0X, pressed(xusb::DPAD_RIGHT) - pressed(xusb::DPAD_LEFT)));
    v.push(event(EV_ABS, ABS_HAT0Y, pressed(xusb::DPAD_DOWN) - pressed(xusb::DPAD_UP)));
    v.push(event(EV_SYN, 0, 0));
    v
}

impl Bus {
    /// Checks that controllers can be made: uinput is there and this user may use it.
    pub fn connect() -> Result<Bus, BusError> {
        drop(open()?);
        Ok(Bus { path: PATH.into(), pads: Mutex::new(HashMap::new()), next: AtomicU32::new(1) })
    }

    pub fn plug_x360(&self, _: &mut Io) -> Result<u32, BusError> {
        let serial = self.next.fetch_add(1, Ordering::Relaxed);
        let file = open()?;
        create(&file, serial)?;
        let pad = Arc::new(Pad { file, stop: AtomicBool::new(false) });
        self.pads.lock().map_err(|_| BusError::Os(ErrorKind::Other.into()))?.insert(serial, pad);
        Ok(serial)
    }

    fn pad(&self, serial: u32) -> Option<Arc<Pad>> {
        self.pads.lock().ok()?.get(&serial).cloned()
    }

    pub fn unplug(&self, _: &mut Io, serial: u32) -> Result<(), u32> {
        let pad = self.pads.lock().ok().and_then(|mut p| p.remove(&serial)).ok_or(0u32)?;
        // Removed once its feedback thread lets go of it too.
        pad.stop.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn submit(&self, _: &mut Io, serial: u32, report: &XusbReport) -> Result<(), u32> {
        let pad = self.pad(serial).ok_or(0u32)?;
        write_events(&pad.file, &events_for(report)).map_err(|e| e.raw_os_error().unwrap_or(0) as u32)
    }

    /// Starts a thread that answers the game's force-feedback requests to one controller and
    /// passes its rumble on. It ends when the controller is unplugged.
    pub fn listen(self: &Arc<Bus>, serial: u32, tx: Sender<Feedback>) -> JoinHandle<()> {
        let pad = self.pad(serial);
        std::thread::Builder::new()
            .name(format!("uinput-feedback-{serial}"))
            .spawn(move || {
                if let Some(pad) = pad {
                    Rumble::default().run(&pad, serial, &tx);
                }
            })
            .expect("could not start the feedback thread")
    }

    /// Ends every feedback thread, which removes the controllers.
    pub fn cancel_all(&self) {
        if let Ok(mut pads) = self.pads.lock() {
            for p in pads.values() {
                p.stop.store(true, Ordering::Relaxed);
            }
            pads.clear();
        }
    }
}

/// The effects a game uploaded to one controller, and the one playing.
#[derive(Default)]
struct Rumble {
    effects: HashMap<i16, (u16, u16, u16)>,
    playing: Option<(i16, Option<Instant>)>,
}

impl Rumble {
    fn run(&mut self, pad: &Pad, serial: u32, tx: &Sender<Feedback>) {
        let fd = pad.file.as_raw_fd();
        let mut buf = [event(0, 0, 0); 16];
        let mut last = (0u8, 0u8);
        while !pad.stop.load(Ordering::Relaxed) {
            let wait = match self.playing {
                Some((_, Some(until))) => until.saturating_duration_since(Instant::now()).min(Duration::from_millis(100)),
                _ => Duration::from_millis(100),
            };
            let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            unsafe { libc::poll(&mut pfd, 1, wait.as_millis() as libc::c_int) };
            let bytes = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), size_of_val(&buf)) };
            let n = match (&pad.file).read(bytes) {
                Ok(n) => n / size_of::<libc::input_event>(),
                Err(e) if e.kind() == ErrorKind::WouldBlock => 0,
                Err(_) => return,
            };
            for ev in &buf[..n] {
                match (ev.type_, ev.code) {
                    (EV_UINPUT, UI_FF_UPLOAD) => self.upload(fd, ev.value as u32),
                    (EV_UINPUT, UI_FF_ERASE) => self.erase(fd, ev.value as u32),
                    (EV_FF, id) => {
                        let id = id as i16;
                        if ev.value > 0 {
                            let length = self.effects.get(&id).map_or(0, |e| e.2);
                            let until = (length > 0).then(|| Instant::now() + Duration::from_millis(length.into()));
                            self.playing = Some((id, until));
                        } else if self.playing.is_some_and(|(p, _)| p == id) {
                            self.playing = None;
                        }
                    }
                    _ => {}
                }
            }
            if let Some((_, Some(until))) = self.playing
                && Instant::now() >= until
            {
                self.playing = None;
            }
            let now = self.motors();
            if now != last {
                last = now;
                if tx.send(Feedback { serial, large_motor: now.0, small_motor: now.1 }).is_err() {
                    return;
                }
            }
        }
    }

    fn motors(&self) -> (u8, u8) {
        match self.playing.and_then(|(id, _)| self.effects.get(&id)) {
            Some(&(strong, weak, _)) => ((strong >> 8) as u8, (weak >> 8) as u8),
            None => (0, 0),
        }
    }

    fn upload(&mut self, fd: i32, request: u32) {
        let mut up: libc::uinput_ff_upload = unsafe { std::mem::zeroed() };
        up.request_id = request;
        if unsafe { libc::ioctl(fd, UI_BEGIN_FF_UPLOAD, &mut up) } < 0 {
            return;
        }
        if up.effect.type_ == FF_RUMBLE {
            // The effect's union starts with `ff_rumble_effect`: strong, then weak magnitude.
            let u = up.effect.u[0].to_ne_bytes();
            let strong = u16::from_ne_bytes([u[0], u[1]]);
            let weak = u16::from_ne_bytes([u[2], u[3]]);
            self.effects.insert(up.effect.id, (strong, weak, up.effect.replay.length));
            up.retval = 0;
        } else {
            up.retval = -libc::EINVAL;
        }
        unsafe { libc::ioctl(fd, UI_END_FF_UPLOAD, &up) };
    }

    fn erase(&mut self, fd: i32, request: u32) {
        let mut er: libc::uinput_ff_erase = unsafe { std::mem::zeroed() };
        er.request_id = request;
        if unsafe { libc::ioctl(fd, UI_BEGIN_FF_ERASE, &mut er) } < 0 {
            return;
        }
        let id = er.effect_id as i16;
        self.effects.remove(&id);
        if self.playing.is_some_and(|(p, _)| p == id) {
            self.playing = None;
        }
        er.retval = 0;
        unsafe { libc::ioctl(fd, UI_END_FF_ERASE, &er) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The request numbers `<linux/uinput.h>` gives on x86-64.
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn request_numbers_match_the_kernel_headers() {
        assert_eq!(UI_DEV_CREATE, 0x5501);
        assert_eq!(UI_DEV_DESTROY, 0x5502);
        assert_eq!(UI_DEV_SETUP, 0x405C_5503);
        assert_eq!(UI_ABS_SETUP, 0x401C_5504);
        assert_eq!(UI_SET_EVBIT, 0x4004_5564);
        assert_eq!(UI_SET_KEYBIT, 0x4004_5565);
        assert_eq!(UI_SET_ABSBIT, 0x4004_5567);
        assert_eq!(UI_SET_FFBIT, 0x4004_556B);
        assert_eq!(UI_SET_PHYS, 0x4008_556C);
        assert_eq!(UI_BEGIN_FF_UPLOAD, 0xC068_55C8);
        assert_eq!(UI_END_FF_UPLOAD, 0x4068_55C9);
        assert_eq!(UI_BEGIN_FF_ERASE, 0xC00C_55CA);
        assert_eq!(UI_END_FF_ERASE, 0x400C_55CB);
    }

    #[test]
    fn a_kernel_without_uinput_is_told_from_a_module_not_loaded() {
        let module = "kernel/drivers/input/misc/uinput.ko.zst:\nkernel/drivers/hid/uhid.ko.zst:\n".to_string();
        let builtin = "kernel/drivers/input/misc/uinput.ko\n".to_string();
        let other = "kernel/drivers/hid/uhid.ko:\nkernel/drivers/input/misc/uinput2.ko:\n".to_string();
        assert!(matches!(absent_from(false, &[module]), BusError::NotLoaded));
        assert!(matches!(absent_from(false, &[other.clone(), builtin]), BusError::NotLoaded));
        assert!(matches!(absent_from(false, std::slice::from_ref(&other)), BusError::NoKernelSupport));
        assert!(matches!(absent_from(true, &[other]), BusError::NotLoaded));
        assert!(matches!(absent_from(false, &[]), BusError::NotLoaded), "no module lists to go by");
        assert_eq!(BusError::NoKernelSupport.driver(), Driver::NoKernelSupport);
        assert_eq!(BusError::NoAccess.driver(), Driver::Missing);
    }

    #[test]
    fn reports_become_xpad_events() {
        let r = XusbReport {
            buttons: xusb::A | xusb::DPAD_LEFT | xusb::DPAD_DOWN,
            left_trigger: 255,
            right_trigger: 0,
            thumb_lx: -32768,
            thumb_ly: 32767,
            thumb_rx: 0,
            thumb_ry: -32768,
        };
        let v = events_for(&r);
        let get = |kind, code| v.iter().find(|e| e.type_ == kind && e.code == code).map(|e| e.value);
        assert_eq!(get(EV_KEY, 0x130), Some(1));
        assert_eq!(get(EV_KEY, 0x131), Some(0));
        assert_eq!(get(EV_ABS, ABS_X), Some(-32768));
        assert_eq!(get(EV_ABS, ABS_Y), Some(-32768), "up is negative");
        assert_eq!(get(EV_ABS, ABS_RY), Some(32767), "down is positive");
        assert_eq!(get(EV_ABS, ABS_Z), Some(255));
        assert_eq!(get(EV_ABS, ABS_HAT0X), Some(-1));
        assert_eq!(get(EV_ABS, ABS_HAT0Y), Some(1));
        assert_eq!(v.last().map(|e| e.type_), Some(EV_SYN));
    }
}
