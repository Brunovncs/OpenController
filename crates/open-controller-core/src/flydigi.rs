//! The Flydigi Apex 2 in its Android mode, wired (04b4:2412). SDL 3.4 reads it with the layout of
//! the Vader pads that share its protocol, and on the Apex 2 that layout is wrong in two ways:
//!
//! - Bytes 23 and 24, the Vader's analog triggers, are where the Apex 2 says how far its ABXY
//!   wheel is dragged. They rest near the middle (0x81, 0x85), so SDL holds both triggers about
//!   half pressed for as long as the pad is connected, and they never move.
//! - Its triggers are buttons, bits 0x10 and 0x20 of byte 10, which SDL does not read; nor C, Z,
//!   M5 and M6 (byte 7), Return or ))) (byte 8), as SDL gives only the Vader pads their extra
//!   buttons.
//!
//! So OpenController opens the pad's own report, as SDL does, and takes those from it. The layout
//! is Flydigi's own (ApexSpace's `getDeviceOperateEventFrom32`), with the Apex 2's buttons as
//! ExtendInput's `FlydigiController` names them, and matches a report from an Apex 2.
//!
//! On its 2.4G receiver (04b4:2411) SDL reads the pad as a plain gamepad, without M1 to M4 either.
//! The same report comes on the receiver's interface 2, as Flydigi's app reads it, so
//! OpenController opens that, asks which pad is on the receiver and, for an Apex 2, takes the
//! whole pad from it: SDL's plain gamepad has no paddles or Home, and falls silent while Flydigi's
//! app is open, which turns the pad over to its own report (an owner's report from 0.9.9).
//!
//! Two things SDL does not give for any of these pads come from the same report: the battery, in
//! the reply to the question SDL asks only once to learn the model, and the motion the pad's air
//! mouse reads, which Flydigi's app moves the mouse by. The battery is read the same way for every
//! pad of this generation (Vader 2 to 4 Pro, Apex 2 to 4, Direwolf), on the cable or the receiver.
//!
//! The Apex 3, 4 and 5 have triggers that push back: [`trigger`] sends them the Lab's effects and
//! those of game mods made for Flydigi's app.

pub mod trigger;

use crate::device::Power;
use crate::extras::{LEFT_PADDLE1, LEFT_PADDLE2, MISC1, MISC2, MISC3, MISC4, MISC5, MISC6, RIGHT_PADDLE1, RIGHT_PADDLE2};
use crate::hidtap;
use crate::mapping::{PadState, axis, button};
use crate::platform::devnode;
use sdl3_sys::everything::*;
use std::ffi::CString;
use std::time::{Duration, Instant};

pub const VENDOR: u16 = 0x04B4;
pub const PRODUCT: u16 = 0x2412;
/// The 2.4G receiver of the older pads (ExtendInput's "two dot" dongle, the Apex 2's).
pub const RECEIVER: u16 = 0x2411;
/// The newer pads' vendor id, and the Apex 5's product (`USB_PRODUCT_FLYDIGI_V2_APEX`).
pub const V2_VENDOR: u16 = 0x37D7;
pub const APEX5: u16 = 0x2501;

/// SDL's mark for each model in the last byte of the GUID (`SDL_FlyDigiControllerType` in SDL's
/// `SDL_hidapi_flydigi.h`), from the model number the pad gives when asked. 0 is unknown.
pub mod sdl_type {
    pub const APEX2: u8 = 1;
    pub const APEX3: u8 = 2;
    pub const APEX4: u8 = 3;
    pub const APEX5: u8 = 4;
}

/// The model number an Apex 2 gives when asked.
pub const APEX2_MODEL: u8 = 19;

/// The Flydigi pads OpenController does more with than SDL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Apex2,
    Apex3,
    Apex4,
    Apex5,
    /// Any other: its battery only, on the older generation.
    Other,
}

impl Kind {
    pub fn from_sdl(t: u8) -> Kind {
        match t {
            sdl_type::APEX2 => Kind::Apex2,
            sdl_type::APEX3 => Kind::Apex3,
            sdl_type::APEX4 => Kind::Apex4,
            sdl_type::APEX5 => Kind::Apex5,
            _ => Kind::Other,
        }
    }

    /// From the model number in the pad's reply. SDL's list, with the Apex 4's limited editions
    /// it lacks (Flydigi's own list, as ApexSenseBridge has it).
    pub fn from_model(id: u8) -> Kind {
        match id {
            APEX2_MODEL => Kind::Apex2,
            24 | 26 | 29 => Kind::Apex3,
            84 | 86 | 87 | 92 | 93 | 102 | 103 | 104 => Kind::Apex4,
            _ => Kind::Other,
        }
    }

    /// Whether its triggers push back. The Apex 6 is left out: its triggers are voice coils with
    /// commands of their own, and the sources disagree on whether it takes these.
    pub fn force_triggers(self) -> bool {
        matches!(self, Kind::Apex3 | Kind::Apex4 | Kind::Apex5)
    }

    pub fn name(self) -> Option<&'static str> {
        match self {
            Kind::Apex2 => Some("Flydigi Apex 2"),
            Kind::Apex3 => Some("Flydigi Apex 3"),
            Kind::Apex4 => Some("Flydigi Apex 4"),
            Kind::Apex5 => Some("Flydigi Apex 5"),
            Kind::Other => None,
        }
    }
}
/// What asks a pad its model and battery, as SDL and Flydigi's app ask it (`getDeviceInfo`). The
/// reply comes in the input report with 0xEC in byte 15 (see [`info`]).
const ASK_MODEL: [u8; 12] = [0x05, 0xEC, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// How often a pad is asked again until it answers: on the receiver it may be off or out of range.
const ASK_EVERY: Duration = Duration::from_secs(2);
/// How often it is asked once it answered, for its battery: ExtendInput's slow poll.
const BATTERY_EVERY: Duration = Duration::from_secs(30);
/// The battery reading of an empty and of a full pad (FlydigiPcSpace's `apex2MinBY` and
/// `apex2MaxBY`, ExtendInput's 0x62 and 0x72). 0 means the pad does not know.
const BATTERY_RANGE: (u8, u8) = (98, 114);
/// What one unit of the air mouse's motion is taken to be, in degrees a second. Nothing of
/// Flydigi's says; a gyro reading ±2000 degrees a second in 16 bits, cut to these 12, gives about
/// 1, and the gyro sensitivity setting makes up for the rest.
const AIR_DEGREES: f32 = 1.0;

/// What tells a Flydigi pad in its 360 mode to go to its Android mode: rumble through XInput,
/// left and right motor, one step every [`KNOCK_STEP`]. Flydigi's own app sends it
/// (`DongleCommand::SwitchXInput2Android` in FlydigiPcSpace 2.5.7.8); no driver of theirs is
/// needed, as the pad's firmware reads it. The pad then leaves and comes back in Android mode.
pub const ANDROID_KNOCK: [(u16, u16); 4] = [(0xAA00, 0xBB00), (0xBB00, 0xAA00), (0xAA00, 0xBB00), (0, 0)];
pub const KNOCK_STEP: std::time::Duration = std::time::Duration::from_millis(100);

/// Whether Flydigi's PC app for its older pads is running (FlydigiPcSpace, the Apex 2's), which
/// switches a pad to its 360 mode whenever a setting is applied there. Its newer Space Station
/// has no Apex 2.
#[cfg(windows)]
pub fn app_running() -> bool {
    running("FlydigiPcSpace.exe")
}

/// Whether a program with this file name is running, whoever runs it.
#[cfg(windows)]
fn running(exe: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snap, &mut e) != 0;
        while more && !found {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            found = String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe);
            more = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(not(windows))]
pub fn app_running() -> bool {
    false
}

/// Whether Flydigi's current app runs: its service starts with Windows and owns the adaptive
/// triggers' port for mods. Windows lets another program take 127.0.0.1 on a port the service holds
/// for every address, and the mods' packets would then go to it alone, so OpenController stays
/// off the port while it runs.
#[cfg(windows)]
pub fn station_running() -> bool {
    running("SpaceStationService.exe") || running("Flydigi Space Station.exe")
}

#[cfg(not(windows))]
pub fn station_running() -> bool {
    false
}

/// Whether a controller is one of the older Flydigi pads that app drives: wired or on its
/// receiver, in Android mode.
pub fn app_drives(vendor: u16) -> bool {
    vendor == VENDOR
}

/// Receivers whose one pad Windows lists as two gamepads, two interfaces of one USB device: the
/// Flydigi 2.4G receiver came twice, in two slots, in an Apex 2 owner's report. Receivers that
/// really carry two pads (PlayStation 2 adapters, the Joy-Con grip) must not be here.
pub const LISTED_TWICE: [(u16, u16); 1] = [(VENDOR, 0x2411)];

/// The extra buttons OpenController reads from the report: C and Z under ABXY, M6 and M5 on top
/// beside the left and right bumpers, Return under the right stick, and ))) left of Home (Pairing
/// in the manual), which also turns the pad's air mouse on and off.
pub const EXTRAS: [u8; 6] = [MISC1, MISC2, MISC3, MISC4, MISC5, MISC6];
/// M1 to M4, SDL's on the cable (`SDL_GAMEPAD_BUTTON_FLYDIGI_M1` and on), OpenController's on the
/// receiver.
pub const PADDLES: [u8; 4] = [RIGHT_PADDLE1, LEFT_PADDLE1, RIGHT_PADDLE2, LEFT_PADDLE2];
const fn b(button: u32) -> u8 {
    button as u8
}
const HOME: u8 = b(button::GUIDE);
const DPAD: [(u8, u8); 4] =
    [(0x01, b(button::DPAD_UP)), (0x02, b(button::DPAD_RIGHT)), (0x04, b(button::DPAD_DOWN)), (0x08, b(button::DPAD_LEFT))];
/// What is read from the report only on the receiver, besides [`EXTRAS`] and the triggers: every
/// other button, and the sticks.
pub const ON_RECEIVER: [u8; 19] = [
    b(button::SOUTH),
    b(button::EAST),
    b(button::WEST),
    b(button::NORTH),
    b(button::BACK),
    HOME,
    b(button::START),
    b(button::LEFT_STICK),
    b(button::RIGHT_STICK),
    b(button::LEFT_SHOULDER),
    b(button::RIGHT_SHOULDER),
    b(button::DPAD_UP),
    b(button::DPAD_DOWN),
    b(button::DPAD_LEFT),
    b(button::DPAD_RIGHT),
    RIGHT_PADDLE1,
    LEFT_PADDLE1,
    RIGHT_PADDLE2,
    LEFT_PADDLE2,
];

/// The extra buttons of an Apex 2 SDL gave `have`: SDL's and those read here.
pub fn extras(have: &[u8], receiver: bool) -> Vec<u8> {
    let ours = |b: &u8| EXTRAS.contains(b) || receiver && PADDLES.contains(b);
    crate::extras::ALL.iter().copied().filter(|b| have.contains(b) || ours(b)).collect()
}

/// What the report says that SDL leaves out or gets wrong.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Apex2 {
    pub lt: bool,
    pub rt: bool,
    /// [`EXTRAS`] held, as the bits of `PadState::buttons`.
    pub buttons: u64,
    /// [`ON_RECEIVER`] held, the same way.
    pub on_receiver: u64,
    /// Left x and y, right x and y, as SDL reads them on the cable.
    pub sticks: [i16; 4],
}

/// ))) is the bit SDL leaves out on purpose, as the Vader's gyro mouse button.
const BUTTON_BITS: [(usize, u8, u8); 6] =
    [(8, 0x01, MISC1), (7, 0x01, MISC2), (7, 0x02, MISC3), (7, 0x80, MISC4), (7, 0x40, MISC5), (8, 0x10, MISC6)];
/// As SDL reads them on the cable (`HIDAPI_DriverFlydigi_HandleStatePacketV1`), the d-pad aside.
const RECEIVER_BITS: [(usize, u8, u8); 15] = [
    (9, 0x10, b(button::SOUTH)),
    (9, 0x20, b(button::EAST)),
    (9, 0x40, b(button::BACK)),
    (9, 0x80, b(button::WEST)),
    (10, 0x01, b(button::NORTH)),
    (10, 0x02, b(button::START)),
    (10, 0x04, b(button::LEFT_SHOULDER)),
    (10, 0x08, b(button::RIGHT_SHOULDER)),
    (10, 0x40, b(button::LEFT_STICK)),
    (10, 0x80, b(button::RIGHT_STICK)),
    (7, 0x04, RIGHT_PADDLE1),
    (7, 0x08, LEFT_PADDLE1),
    (7, 0x10, RIGHT_PADDLE2),
    (7, 0x20, LEFT_PADDLE2),
    (8, 0x08, HOME),
];
/// Where the sticks are: left x and y, right x and y.
const STICK_BYTES: [usize; 4] = [17, 19, 21, 22];

/// A stick byte as SDL scales it, 0x7F as the middle.
fn stick(v: u8) -> i16 {
    if v == 0x7f { 0 } else { (-32768.0f32 + 65535.0 * f32::from(v) / 255.0) as i16 }
}

fn bits(r: &[u8], table: &[(usize, u8, u8)]) -> u64 {
    table.iter().filter(|&&(at, bit, _)| r[at] & bit != 0).fold(0, |b, &(.., to)| b | 1 << to)
}

fn mask(buttons: &[u8]) -> u64 {
    buttons.iter().fold(0, |m, &b| m | 1 << b)
}

/// Reads an input report (id 0x04, then 0xFE). Replies to commands come in the same report with
/// another second byte, and are not input.
pub fn parse(r: &[u8]) -> Option<Apex2> {
    if r.len() < 25 || r[0] != 0x04 || r[1] != 0xFE {
        return None;
    }
    let dpad = DPAD.iter().filter(|&&(bit, _)| r[9] & bit != 0).fold(0, |m, &(_, to)| m | 1 << to);
    Some(Apex2 {
        lt: r[10] & 0x10 != 0,
        rt: r[10] & 0x20 != 0,
        buttons: bits(r, &BUTTON_BITS),
        on_receiver: bits(r, &RECEIVER_BITS) | dpad,
        sticks: STICK_BYTES.map(|at| stick(r[at])),
    })
}

/// What a pad says of itself in its reply to [`ASK_MODEL`], as FlydigiPcSpace's `handerDeviceInfo`
/// reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub model: u8,
    /// In percent; `None` when the pad does not know.
    pub battery: Option<u8>,
    /// Whether it is on the cable, from firmware 6.4 on (as SDL reads it); before, `None`.
    pub wired: Option<bool>,
}

/// Only a whole 32-byte report, as SDL takes it: a reply to another command that happens to hold
/// 0xEC there must not set the model, which is learnt once.
pub fn info(r: &[u8]) -> Option<Info> {
    if r.len() < 32 || r[0] != 0x04 || r[1] == 0xFE || r[15] != 0xEC {
        return None;
    }
    let (empty, full) = BATTERY_RANGE;
    let battery = (r[11] != 0).then(|| (u16::from(r[11].clamp(empty, full) - empty) * 100 / u16::from(full - empty)) as u8);
    // Firmware 6.0.4.9 says nothing there; SDL trusts it from 0x6400 on.
    let firmware = u16::from_le_bytes([r[9], r[10]]);
    let wired = match r[13] {
        0 if firmware >= 0x6400 => Some(false),
        1 if firmware >= 0x6400 => Some(true),
        _ => None,
    };
    Some(Info { model: r[3], battery, wired })
}

impl Info {
    /// The battery as OpenController shows it. A pad that does not say how it is connected is
    /// taken as on the cable at SDL's path: the newer receivers also show up as 2412, but their
    /// pads have firmware that says.
    pub fn power(self, on_receiver: bool) -> Option<Power> {
        let pct = self.battery?;
        Some(match (self.wired.unwrap_or(!on_receiver), pct) {
            (false, _) => Power::Battery(Some(pct)),
            (true, 100) => Power::Charged,
            (true, _) => Power::Charging(Some(pct)),
        })
    }
}

/// The air mouse's motion in an input report: two 12-bit numbers in bytes 4 to 6, how far to move
/// the mouse right and down (FlydigiPcSpace's `handleMotionData`, ExtendInput's air mouse).
pub fn air(r: &[u8]) -> Option<(i16, i16)> {
    if r.len() < 25 || r[0] != 0x04 || r[1] != 0xFE {
        return None;
    }
    let twelve = |v: u16| ((v << 4) as i16) >> 4;
    let x = u16::from(r[4]) | u16::from(r[5] & 0x0F) << 8;
    let y = u16::from(r[5] >> 4) | u16::from(r[6]) << 4;
    Some((twelve(x), twelve(y)))
}

/// The air mouse's motion as SDL's gyro reading: radians a second, pitch, yaw and roll. Moving the
/// mouse right is turning right, a negative yaw; moving it down is tilting the top down, a
/// negative pitch.
pub fn air_rate((x, y): (i16, i16)) -> [f32; 3] {
    let k = AIR_DEGREES.to_radians();
    [-f32::from(y) * k, -f32::from(x) * k, 0.]
}

impl Apex2 {
    /// What the pad holds once `report` came: what it says, or as before for a report that is not
    /// input.
    pub fn after(self, report: &[u8]) -> Apex2 {
        parse(report).unwrap_or(self)
    }

    /// Puts what SDL got wrong right in a state SDL just gave: the triggers, all the way or not at
    /// all, and the extra buttons.
    pub fn apply(self, s: &mut PadState) {
        let full = |on: bool| if on { i16::MAX } else { 0 };
        s.axes[axis::LEFT_TRIGGER] = full(self.lt);
        s.axes[axis::RIGHT_TRIGGER] = full(self.rt);
        s.buttons = s.buttons & !mask(&EXTRAS) | self.buttons;
    }

    /// Puts the rest of the pad in a state SDL gave without M1 to M4 and Home, or with nothing at
    /// all while Flydigi's app is open.
    pub fn apply_on_receiver(self, s: &mut PadState) {
        s.buttons = s.buttons & !mask(&ON_RECEIVER) | self.on_receiver;
        s.axes[..4].copy_from_slice(&self.sticks);
    }
}

/// A pad's HID device, opened beside SDL's handle: each handle gets every report, and what is
/// written goes to the same pad.
pub struct Port(*mut SDL_hid_device);

impl Port {
    pub fn open(path: &str) -> Option<Port> {
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
            Some(Port(dev))
        }
    }

    /// Sends an output report, its id first. Whether the pad took it.
    pub fn write(&self, report: &[u8]) -> bool {
        unsafe { SDL_hid_write(self.0, report.as_ptr(), report.len()) >= 0 }
    }

    fn read(&self, buf: &mut [u8]) -> i32 {
        unsafe { SDL_hid_read_timeout(self.0, buf.as_mut_ptr(), buf.len(), 0) }
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        unsafe {
            SDL_hid_close(self.0);
            SDL_hid_exit();
        }
    }
}

/// The pad's report, opened beside SDL's handle, and what it last said. Until a report comes the
/// triggers are released, rather than SDL's half press.
pub struct V1Reader {
    /// `None` once a read failed, until it opens again.
    port: Option<Port>,
    /// Where it was opened, to open it again.
    origin: Origin,
    /// When it was lost, or last tried again.
    lost_at: Option<Instant>,
    pub last: Apex2,
    /// Opened on the receiver rather than at SDL's path.
    pub receiver: bool,
    /// What the pad last said of itself, once it answered.
    pub info: Option<Info>,
    asked: Option<Instant>,
    /// What the air mouse last read, right and down, and when a report last came.
    pub air: (i16, i16),
    heard: Option<Instant>,
    /// Whether its report is coming, for the receiver.
    input: Heard,
}

/// How long without a report before the air mouse is taken as still: a pad that leaves its
/// receiver mid-turn would otherwise keep turning the camera.
const AIR_STALE: Duration = Duration::from_millis(250);

/// The air mouse is still once the pad has gone quiet.
fn settled(air: (i16, i16), heard: Option<Instant>, now: Instant) -> (i16, i16) {
    if heard.is_some_and(|at| now.duration_since(at) >= AIR_STALE) { (0, 0) } else { air }
}

/// What [`V1Reader::read`] took in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Read {
    /// Buttons or triggers changed.
    pub input: bool,
    /// The air mouse moved.
    pub motion: bool,
}

/// The most reports taken in one look, so the engine is never held up; the pad sends about 500 a
/// second over the cable.
const MOST_PER_LOOK: usize = 64;

/// How a reader opened its pad: at SDL's path for it, or on the receiver SDL reads at that path.
#[derive(Clone, Debug)]
enum Origin {
    Pad(String),
    Receiver(String),
}

impl Origin {
    fn open(&self) -> Option<Port> {
        match self {
            Origin::Pad(path) => Port::open(path),
            Origin::Receiver(sdl_path) => receiver_port(sdl_path),
        }
    }
}

/// How often a reader whose handle failed tries to open it again, as the handheld reader does: a
/// resume from sleep may leave SDL's handle working and this one dead.
const REOPEN_EVERY: Duration = Duration::from_secs(2);

/// Interface 2 of the receiver SDL reads as a gamepad at `sdl_path`.
fn receiver_port(sdl_path: &str) -> Option<Port> {
    unsafe {
        if SDL_hid_init() != 0 {
            return None;
        }
    }
    let own = devnode::usb_device(sdl_path);
    let all: Vec<_> = hidtap::interfaces(VENDOR, RECEIVER).into_iter().filter(|(_, s)| s.interface == 2).collect();
    unsafe { SDL_hid_exit() };
    let mut found: Vec<_> = match own {
        Some(_) => all.into_iter().filter(|(p, _)| devnode::usb_device(p) == own).collect(),
        // Where the USB device cannot be told (outside Windows), only a receiver on its own:
        // with two, each pad would read the other's. One receiver has one collection on
        // Flydigi's own page, whatever else its interface lists (macOS lists each collection).
        None => {
            let own_page: Vec<_> = all.iter().filter(|(_, s)| s.usage_page >= 0xFF00).cloned().collect();
            match (own_page.len(), all.len()) {
                (1, _) => own_page,
                (0, 1) => all,
                _ => Vec::new(),
            }
        }
    };
    // Flydigi's report has a page of its own; any other collection there comes after.
    found.sort_by_key(|(_, s)| s.usage_page < 0xFF00);
    Port::open(&found.first()?.0)
}

impl V1Reader {
    /// Opens the device at SDL's path for the pad: the interface with its own report.
    pub fn open(path: &str) -> Option<V1Reader> {
        let origin = Origin::Pad(path.to_string());
        origin.open().map(|port| Self::on(Some(port), origin, false))
    }

    /// Opens interface 2 of the receiver SDL reads as a gamepad at `sdl_path`, and asks which pad
    /// is on it. `info` says once it answers.
    pub fn on_receiver(sdl_path: &str, now: Instant) -> Option<V1Reader> {
        let origin = Origin::Receiver(sdl_path.to_string());
        let mut r = Self::on(Some(origin.open()?), origin, true);
        r.ask(now);
        Some(r)
    }

    fn on(port: Option<Port>, origin: Origin, receiver: bool) -> V1Reader {
        V1Reader {
            port,
            origin,
            lost_at: None,
            last: Apex2::default(),
            receiver,
            info: None,
            asked: None,
            air: (0, 0),
            heard: None,
            input: Heard::default(),
        }
    }

    /// Sends a command to the pad (output report 0x05). Not while its handle is lost.
    pub fn write(&self, report: &[u8]) -> bool {
        self.port.as_ref().is_some_and(|p| p.write(report))
    }

    fn ask(&mut self, now: Instant) {
        // It may fail now and then, as SDL finds: asked again later.
        self.write(&ASK_MODEL);
        self.asked = Some(now);
    }

    /// Its handle failed: what the pad held is let go, and SDL's reading of the receiver stands
    /// until it opens again. What the pad said of itself is kept.
    fn lose(&mut self, now: Instant) {
        self.port = None;
        self.lost_at = Some(now);
        self.last = Apex2::default();
        self.air = (0, 0);
        self.heard = None;
        self.input = Heard { was_live: self.input.was_live, ..Heard::default() };
    }

    /// Whether it is time to open a lost handle again.
    fn reopen_due(&self, now: Instant) -> bool {
        self.port.is_none() && self.lost_at.is_none_or(|at| now.duration_since(at) >= REOPEN_EVERY)
    }

    /// Asks the pad about itself when it is time: every few seconds until it answers, then now and
    /// then for its battery.
    pub fn ask_again(&mut self, now: Instant) {
        let every = if self.info.is_some() { BATTERY_EVERY } else { ASK_EVERY };
        if self.asked.is_none_or(|at| now.duration_since(at) >= every) {
            self.ask(now);
        }
    }

    /// Takes in what the pad sent since the last look, without waiting.
    pub fn read(&mut self, now: Instant) -> Read {
        let mut buf = [0u8; 64];
        let (before, air) = (self.last, self.air);
        if self.reopen_due(now) {
            self.port = self.origin.open();
            self.lost_at = Some(now);
        }
        if let Some(port) = self.port.take() {
            let mut lost = false;
            for _ in 0..MOST_PER_LOOK {
                let n = port.read(&mut buf);
                if n < 0 {
                    lost = true;
                    break;
                }
                if n == 0 {
                    break;
                }
                self.take(&buf[..n as usize], now);
            }
            if lost {
                drop(port);
                self.lose(now);
            } else {
                self.port = Some(port);
            }
        }
        self.settle(now);
        // The report starting or stopping changes what the pad holds even when it says nothing.
        let flipped = self.input.flipped(now);
        Read { input: self.last != before || flipped, motion: self.air != air }
    }

    fn take(&mut self, report: &[u8], now: Instant) {
        if let Some(i) = info(report) {
            self.info = Some(i);
        }
        if let Some(a) = air(report) {
            self.air = a;
            self.heard = Some(now);
            self.input.report(report, now);
        }
        self.last = self.last.after(report);
    }

    /// Whether the report started or stopped coming, as [`V1Reader::read`] tells the engine.
    #[cfg(test)]
    fn read_flips(&mut self, now: Instant) -> bool {
        self.input.flipped(now)
    }

    fn settle(&mut self, now: Instant) {
        self.air = settled(self.air, self.heard, now);
    }

    /// The battery as the pad last told it.
    pub fn power(&self) -> Option<Power> {
        self.info.and_then(|i| i.power(self.receiver))
    }

    /// Puts what the pad last said in a state SDL gave. On the receiver, the whole pad while its
    /// report comes; otherwise SDL's reading stands.
    pub fn apply(&self, s: &mut PadState, now: Instant) {
        put(self.last, self.receiver, self.input.live(now), s);
    }
}

fn put(last: Apex2, receiver: bool, live: bool, s: &mut PadState) {
    if !receiver {
        last.apply(s);
    } else if live {
        last.apply(s);
        last.apply_on_receiver(s);
    }
}

/// How long a pad that repeats its report may fall silent before it is taken as gone: switched
/// off, out of the receiver's range or flat, with whatever it held let go.
pub const GONE_AFTER: Duration = Duration::from_millis(500);

/// Whether a pad's report is coming, on the receiver. Until one comes, and once a pad that keeps
/// sending falls silent, SDL's reading of the receiver's plain gamepad stands: the report must
/// never blank a pad SDL reads, should Flydigi's interface say nothing (a handle lost to sleep, a
/// receiver that does not send it). A pad seen only sending changes is never taken as gone: its
/// silence may be a button held still.
#[derive(Clone, Copy, Debug, Default)]
struct Heard {
    /// When the last input report came, and what it said.
    last: Option<(Instant, [u8; 30])>,
    /// It sent the same report twice running: it sends all the time, not only on a change.
    repeats: bool,
    /// What [`Heard::flipped`] last found.
    was_live: bool,
}

impl Heard {
    fn report(&mut self, r: &[u8], now: Instant) {
        let Some(said) = r.get(2..32).and_then(|b| <[u8; 30]>::try_from(b).ok()) else { return };
        self.repeats |= self.last.is_some_and(|(_, before)| before == said);
        self.last = Some((now, said));
    }

    fn live(&self, now: Instant) -> bool {
        self.last.is_some_and(|(at, _)| !self.repeats || now.duration_since(at) < GONE_AFTER)
    }

    /// Whether it started or stopped coming since the last look.
    fn flipped(&mut self, now: Instant) -> bool {
        let live = self.live(now);
        std::mem::replace(&mut self.was_live, live) != live
    }
}

#[cfg(test)]
mod sim;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::button;

    /// An Apex 2 at rest, as its owner's report caught it (firmware 6.0.5.2, wired).
    const REST: [u8; 32] = [
        0x04, 0xfe, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84, 0x00, 0x7f, 0x00, 0x7e,
        0x7f, 0x81, 0x85, 0x00, 0x00, 0x00, 0x01, 0x04, 0x36, 0x00,
    ];

    fn with(changes: &[(usize, u8)]) -> [u8; 32] {
        let mut r = REST;
        for &(at, v) in changes {
            r[at] = v;
        }
        r
    }

    fn bit(b: u8) -> u64 {
        1 << b
    }

    #[cfg(windows)]
    #[test]
    fn a_running_program_is_found_by_its_file_name() {
        let me = std::env::current_exe().unwrap();
        let me = me.file_name().unwrap().to_string_lossy().to_uppercase();
        assert!(running(&me), "{me}");
        assert!(!running("no-such-program-here.exe"));
    }

    #[test]
    fn at_rest_nothing_is_held_whatever_the_wheel_says() {
        let rest = Apex2 { sticks: [stick(0x84), 0, stick(0x7e), 0], ..Apex2::default() };
        assert_eq!(parse(&REST), Some(rest));
        // The wheel dragged all the way: SDL would read full triggers.
        assert_eq!(parse(&with(&[(23, 0xff), (24, 0x00)])), Some(rest));
    }

    #[test]
    fn triggers_are_bits_of_byte_10() {
        let both = parse(&with(&[(10, 0x30)])).unwrap();
        assert!(both.lt && both.rt);
        let lt = parse(&with(&[(10, 0x10)])).unwrap();
        assert!(lt.lt && !lt.rt);
        // Y, Start, the bumpers and stick clicks share the byte and are SDL's.
        let others = parse(&with(&[(10, 0xcf)])).unwrap();
        assert!(!others.lt && !others.rt);
    }

    #[test]
    fn extra_buttons_by_their_bits() {
        let cases =
            [((8, 0x01), MISC1), ((7, 0x01), MISC2), ((7, 0x02), MISC3), ((7, 0x80), MISC4), ((7, 0x40), MISC5), ((8, 0x10), MISC6)];
        for ((at, v), b) in cases {
            assert_eq!(parse(&with(&[(at, v)])).unwrap().buttons, bit(b), "byte {at} bit {v:#04x}");
        }
        // M1 to M4 (byte 7, 0x04 to 0x20) and Home (byte 8) are SDL's, but on the receiver.
        assert_eq!(parse(&with(&[(7, 0x3c), (8, 0x08)])).unwrap().buttons, 0);
    }

    #[test]
    fn paddles_and_home_as_sdl_reads_them_on_the_cable() {
        let cases = [
            ((7, 0x04), RIGHT_PADDLE1),
            ((7, 0x08), LEFT_PADDLE1),
            ((7, 0x10), RIGHT_PADDLE2),
            ((7, 0x20), LEFT_PADDLE2),
            ((8, 0x08), HOME),
        ];
        for ((at, v), b) in cases {
            let a = parse(&with(&[(at, v)])).unwrap();
            assert_eq!((a.on_receiver, a.buttons), (bit(b), 0), "byte {at} bit {v:#04x}");
        }
        assert_eq!(parse(&with(&[(7, 0xc3), (8, 0x11)])).unwrap().on_receiver, 0, "C, Z, M5, M6, ))) and Return");
    }

    fn at(t: Instant, ms: u64) -> Instant {
        t + Duration::from_millis(ms)
    }

    /// A pad on the receiver: what it sends and when, and what games get on top of SDL's reading.
    struct OnReceiver {
        heard: Heard,
        last: Apex2,
    }

    impl OnReceiver {
        fn new() -> OnReceiver {
            OnReceiver { heard: Heard::default(), last: Apex2::default() }
        }

        fn send(&mut self, r: &[u8], now: Instant) {
            self.heard.report(r, now);
            self.last = self.last.after(r);
        }

        fn games_get(&self, sdl: PadState, now: Instant) -> PadState {
            let mut s = sdl;
            put(self.last, true, self.heard.live(now), &mut s);
            s
        }
    }

    /// What SDL reads of the receiver's plain gamepad with A held, the left stick right, LT pulled.
    fn sdl_holding_a() -> PadState {
        let mut s = PadState { buttons: 1 << button::SOUTH, ..PadState::default() };
        (s.axes[axis::LEFT_X], s.axes[axis::LEFT_TRIGGER]) = (20000, i16::MAX);
        s
    }

    #[test]
    fn with_flydigis_app_open_the_report_alone_is_the_pad() {
        // The app turned SDL's side off: SDL reads nothing at all.
        let (t, mut pad) = (Instant::now(), OnReceiver::new());
        pad.send(&with(&[(9, 0x10), (17, 0xff), (10, 0x10)]), t);
        let s = pad.games_get(PadState::default(), at(t, 5));
        assert_eq!(s.buttons, 1 << button::SOUTH);
        assert_eq!((s.axes[axis::LEFT_X], s.axes[axis::LEFT_TRIGGER]), (i16::MAX, i16::MAX));
    }

    #[test]
    fn a_silent_report_never_blanks_what_sdl_reads() {
        // Flydigi's interface opened but says nothing (a lost handle, another receiver): before,
        // the pad would have been taken as at rest whatever SDL read.
        let (t, pad) = (Instant::now(), OnReceiver::new());
        assert_eq!(pad.games_get(sdl_holding_a(), at(t, 10_000)), sdl_holding_a());
    }

    #[test]
    fn a_pad_that_keeps_sending_is_gone_once_silent_and_what_it_held_is_let_go() {
        // Switched off, flat or out of range with A and the stick held, Flydigi's app open.
        let (t, mut pad) = (Instant::now(), OnReceiver::new());
        let held = with(&[(9, 0x10), (17, 0xff)]);
        for ms in 0..20 {
            pad.send(&held, at(t, ms * 2));
        }
        assert!(pad.heard.repeats);
        assert_eq!(pad.games_get(PadState::default(), at(t, 300)).buttons, 1 << button::SOUTH, "a short gap is not gone");
        let gone = pad.games_get(PadState::default(), at(t, 38 + GONE_AFTER.as_millis() as u64));
        assert_eq!(gone, PadState::default(), "nothing held, the stick centred");
        // Switched back on.
        pad.send(&REST, at(t, 5000));
        assert!(pad.heard.live(at(t, 5000)));
    }

    #[test]
    fn a_pad_that_sends_only_changes_keeps_a_held_button() {
        // Never the same report twice: its silence may be A held still for a long while.
        let (t, mut pad) = (Instant::now(), OnReceiver::new());
        pad.send(&REST, t);
        pad.send(&with(&[(9, 0x10)]), at(t, 10));
        assert!(!pad.heard.repeats);
        assert_eq!(pad.games_get(PadState::default(), at(t, 60_000)).buttons, 1 << button::SOUTH);
    }

    #[test]
    fn motion_alone_does_not_count_as_repeating() {
        // The air mouse moving makes every report differ, so a held button with the air mouse on
        // never teaches that the pad repeats itself.
        let (t, mut pad) = (Instant::now(), OnReceiver::new());
        for i in 0..50u8 {
            pad.send(&with(&[(9, 0x10), (4, i)]), at(t, u64::from(i)));
        }
        assert!(!pad.heard.repeats);
    }

    #[test]
    fn replies_and_cut_reports_say_nothing_of_the_pad_being_there() {
        let (t, mut h) = (Instant::now(), Heard::default());
        h.report(&REST[..20], t);
        assert!(!h.live(t), "too short to be the pad");
    }

    #[test]
    fn the_engine_is_woken_when_the_report_starts_and_stops() {
        let (t, mut pad) = (Instant::now(), OnReceiver::new());
        assert!(!pad.heard.flipped(t));
        pad.send(&REST, t);
        pad.send(&REST, at(t, 2));
        assert!(pad.heard.flipped(at(t, 3)), "started");
        assert!(!pad.heard.flipped(at(t, 4)));
        assert!(pad.heard.flipped(at(t, 2 + GONE_AFTER.as_millis() as u64)), "stopped");
    }

    #[test]
    fn on_the_cable_sdl_reads_the_report_itself_and_nothing_changes() {
        let mut s = sdl_holding_a();
        put(Apex2::default(), false, false, &mut s);
        assert_eq!(
            (s.buttons, s.axes[axis::LEFT_X], s.axes[axis::LEFT_TRIGGER]),
            (1 << button::SOUTH, 20000, 0),
            "triggers are the report's"
        );
    }

    /// A reader on the receiver whose handle is not open, fed by hand.
    fn unopened() -> V1Reader {
        V1Reader::on(None, Origin::Receiver(String::new()), true)
    }

    #[test]
    fn a_lost_handle_lets_go_and_hands_the_pad_back_to_sdl() {
        // A resume from sleep: SDL's handle works, this one fails mid-press, Flydigi's app closed.
        let (t, mut r) = (Instant::now(), unopened());
        r.info = Some(Info { model: APEX2_MODEL, battery: Some(80), wired: Some(false) });
        for ms in 0..5 {
            r.take(&with(&[(9, 0x10), (4, 0x20), (10, 0x10)]), at(t, ms));
        }
        assert!(r.read_flips(at(t, 5)), "the report started");
        r.lose(at(t, 6));
        assert!(r.read_flips(at(t, 6)), "the engine is woken to let go");
        let mut s = sdl_holding_a();
        r.apply(&mut s, at(t, 6));
        assert_eq!(s, sdl_holding_a(), "SDL's reading stands, its trigger too");
        assert_eq!((r.air, r.last), ((0, 0), Apex2::default()), "no air mouse turning, nothing held");
        assert_eq!(r.info.map(|i| i.model), Some(APEX2_MODEL), "what it said of itself is kept");
        assert!(!r.write(&ASK_MODEL), "nothing goes out while lost");
    }

    #[test]
    fn a_lost_handle_is_tried_again_every_two_seconds() {
        let (t, mut r) = (Instant::now(), unopened());
        assert!(r.reopen_due(t), "never opened: tried at once");
        r.lose(t);
        assert!(!r.reopen_due(at(t, 1999)));
        assert!(r.reopen_due(at(t, 2000)));
    }

    #[test]
    fn on_the_cable_a_lost_handle_releases_what_sdl_gets_wrong() {
        // SDL holds the triggers half way on the cable: lost, they are released, not held.
        let (t, mut r) = (Instant::now(), V1Reader::on(None, Origin::Pad(String::new()), false));
        r.take(&with(&[(10, 0x10)]), t);
        r.lose(at(t, 1));
        let mut s = PadState { axes: [0, 0, 0, 0, 16000, 16000], ..PadState::default() };
        r.apply(&mut s, at(t, 1));
        assert_eq!(s.axes[axis::LEFT_TRIGGER..], [0, 0]);
    }

    #[test]
    fn the_sticks_end_to_end_with_0x7f_the_middle() {
        assert_eq!((stick(0x00), stick(0x7f), stick(0xff)), (i16::MIN, 0, i16::MAX));
        assert!(stick(0x80) > 0 && stick(0x7e) < 0);
    }

    #[test]
    fn on_the_receiver_the_whole_pad_comes_from_the_report() {
        // A, Select, Y, Start, the bumpers and the stick clicks, and the d-pad up and left.
        let a = parse(&with(&[(9, 0x59), (10, 0xcf), (17, 0x00), (22, 0xff)])).unwrap();
        let want = [button::SOUTH, button::BACK, button::NORTH, button::START, button::LEFT_SHOULDER, button::RIGHT_SHOULDER]
            .into_iter()
            .chain([button::LEFT_STICK, button::RIGHT_STICK, button::DPAD_UP, button::DPAD_LEFT]);
        assert_eq!(a.on_receiver, want.fold(0, |m, b| m | 1 << b));
        let mut s = PadState { buttons: 1 << button::EAST, axes: [100; 6], ..PadState::default() };
        a.apply_on_receiver(&mut s);
        assert_eq!(s.buttons & 1 << button::EAST, 0, "B is let go: SDL's view is not the pad's");
        assert_eq!(s.axes[..4], [i16::MIN, 0, stick(0x7e), i16::MAX]);
        assert_eq!(s.axes[4..], [100, 100], "the triggers are apply's");
    }

    #[test]
    fn paddles_and_home_are_ours_only_on_the_receiver() {
        let held = Apex2 { on_receiver: bit(RIGHT_PADDLE1) | bit(LEFT_PADDLE2) | bit(HOME), ..Apex2::default() };
        // On the cable SDL gives M1 to M4 and Home: what it says stays.
        let mut s = PadState { buttons: bit(LEFT_PADDLE1), ..PadState::default() };
        held.apply(&mut s);
        assert_eq!(s.buttons, bit(LEFT_PADDLE1));
        // On the receiver SDL's mapping has no Home.
        held.apply_on_receiver(&mut s);
        assert_eq!(s.buttons, bit(RIGHT_PADDLE1) | bit(LEFT_PADDLE2) | bit(HOME));
        let wired = extras(&PADDLES, false);
        let receiver = extras(&[], true);
        assert_eq!(wired, receiver, "the same buttons either way");
        assert!(PADDLES.iter().chain(&EXTRAS).all(|b| receiver.contains(b)));
        assert!(!extras(&[], false).iter().any(|b| PADDLES.contains(b)));
    }

    fn reply(battery: u8) -> [u8; 32] {
        // How SDL finds the Apex 2's model on the cable: byte 15 echoes the question, byte 3 is
        // the model, byte 11 the battery.
        let mut r = REST;
        (r[1], r[3], r[11], r[15]) = (0xff, APEX2_MODEL, battery, 0xec);
        r
    }

    #[test]
    fn the_model_and_battery_come_in_a_reply() {
        assert_eq!(info(&reply(114)), Some(Info { model: APEX2_MODEL, battery: Some(100), wired: None }));
        assert_eq!(parse(&reply(114)), None, "and is not input");
        assert_eq!(air(&reply(114)), None);
        assert_eq!(info(&REST), None);
        let mut input = REST;
        input[15] = 0xec;
        assert_eq!(info(&input), None, "input that happens to hold 0xEC there");
        assert_eq!(info(&reply(100)[..15]), None);
        assert_eq!(info(&reply(100)[..31]), None, "a whole report only, as SDL takes it");
    }

    #[test]
    fn battery_from_empty_to_full() {
        let pct = |raw| info(&reply(raw)).unwrap().battery;
        assert_eq!([pct(98), pct(106), pct(114)], [Some(0), Some(50), Some(100)]);
        assert_eq!([pct(60), pct(200)], [Some(0), Some(100)], "past either end");
        assert_eq!(pct(0), None, "the pad does not know");
    }

    #[test]
    fn the_connection_is_trusted_from_firmware_6_4_on() {
        let with_firmware = |lo: u8, hi: u8, link: u8| {
            let mut r = reply(106);
            (r[9], r[10], r[13]) = (lo, hi, link);
            info(&r).unwrap()
        };
        // The Apex 2 owner's 6.0.5.2 says 0 there whatever the link.
        let old = with_firmware(0x52, 0x60, 0);
        assert_eq!(old.wired, None);
        assert_eq!(old.power(false), Some(Power::Charging(Some(50))), "at SDL's path: taken as the cable");
        assert_eq!(old.power(true), Some(Power::Battery(Some(50))), "on the receiver");
        // A newer receiver shows up as 2412 too: its pad says it is wireless.
        let new = with_firmware(0x01, 0x64, 0);
        assert_eq!(new.wired, Some(false));
        assert_eq!(new.power(false), Some(Power::Battery(Some(50))));
        assert_eq!(with_firmware(0x01, 0x64, 1).power(true), Some(Power::Charging(Some(50))));
        assert_eq!(with_firmware(0x01, 0x64, 7).wired, None, "anything else says nothing");
        let mut full = reply(114);
        (full[10], full[13]) = (0x64, 1);
        assert_eq!(info(&full).unwrap().power(false), Some(Power::Charged));
        assert_eq!(info(&reply(0)).unwrap().power(false), None);
    }

    #[test]
    fn models_by_sdl_and_by_their_own_number() {
        assert_eq!(Kind::from_sdl(sdl_type::APEX2), Kind::Apex2);
        assert_eq!(Kind::from_sdl(sdl_type::APEX5), Kind::Apex5);
        assert_eq!(Kind::from_sdl(0), Kind::Other);
        assert_eq!(Kind::from_sdl(16), Kind::Other, "a Vader 2");
        assert_eq!([24, 26, 29].map(Kind::from_model), [Kind::Apex3; 3]);
        assert_eq!([84, 86, 104].map(Kind::from_model), [Kind::Apex4; 3], "limited editions SDL does not know");
        assert_eq!([19, 20, 85, 0].map(Kind::from_model), [Kind::Apex2, Kind::Other, Kind::Other, Kind::Other]);
        assert!(Kind::Apex3.force_triggers() && Kind::Apex4.force_triggers() && Kind::Apex5.force_triggers());
        assert!(!Kind::Apex2.force_triggers() && !Kind::Other.force_triggers());
    }

    #[test]
    fn the_air_mouse_is_two_signed_12_bit_numbers() {
        assert_eq!(air(&REST), Some((0, 0)));
        // Right 5, down 3: x low byte and low nibble, y high nibble and byte.
        assert_eq!(air(&with(&[(4, 0x05), (5, 0x30), (6, 0x00)])), Some((5, 3)));
        // Left 1, up 2.
        assert_eq!(air(&with(&[(4, 0xff), (5, 0xef), (6, 0xff)])), Some((-1, -2)));
        assert_eq!(air(&with(&[(4, 0xff), (5, 0x07)])), Some((2047, 0)));
        assert_eq!(air(&with(&[(4, 0x00), (5, 0x08)])), Some((-2048, 0)));
        assert_eq!(air(&with(&[(5, 0x00), (6, 0x80)])), Some((0, -2048)));
    }

    #[test]
    fn a_pad_gone_quiet_mid_turn_stops_turning() {
        let t = Instant::now();
        assert_eq!(settled((300, -20), Some(t), t + Duration::from_millis(100)), (300, -20));
        assert_eq!(settled((300, -20), Some(t), t + AIR_STALE), (0, 0), "off, or out of the receiver's range");
        assert_eq!(settled((0, 0), None, t), (0, 0));
    }

    #[test]
    fn the_air_mouse_turns_the_gyro_as_a_mouse_would_the_camera() {
        // Moved right (turning right): a negative yaw, which OpenController's gyro aim turns into
        // the camera going right.
        let [pitch, yaw, roll] = air_rate((90, 0));
        assert!(yaw < 0. && pitch == 0. && roll == 0.);
        assert!((yaw + 90f32.to_radians()).abs() < 1e-6, "one unit a degree a second");
        // Moved up (the top tilted up): a positive pitch.
        let [pitch, yaw, _] = air_rate((0, -10));
        assert!(pitch > 0. && yaw == 0.);
        let mut aim = crate::motion::GyroAim::default();
        let g = crate::profile::Gyro { mode: crate::profile::GyroMode::Always, ..Default::default() };
        let (x, y) = aim.update(air_rate((400, -400)), 0.01, g, false);
        assert!(x > 0 && y > 0, "right and up on the right stick: {x} {y}");
    }

    #[test]
    fn motion_and_replies_are_not_input() {
        // The air mouse's two 12-bit values in bytes 4 to 6 move whenever the pad does.
        assert_eq!(parse(&with(&[(4, 0xff), (5, 0xff), (6, 0xff)])), parse(&REST));
        // A reply to SDL's question about the model, in the same report.
        let mut reply = REST;
        (reply[1], reply[10], reply[15]) = (0xff, 0x30, 0xec);
        assert_eq!(parse(&reply), None);
        assert_eq!(parse(&REST[..20]), None);
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn apply_replaces_sdls_half_pressed_triggers() {
        // What SDL gives for the resting wheel: about half of each trigger.
        let mut s = PadState { axes: [0, 0, 0, 0, 16576, 16964], ..PadState::default() };
        s.buttons = 1 << button::SOUTH;
        Apex2::default().apply(&mut s);
        assert_eq!((s.axes[axis::LEFT_TRIGGER], s.axes[axis::RIGHT_TRIGGER]), (0, 0));
        assert_eq!(s.buttons, 1 << button::SOUTH, "SDL's buttons stay");
        Apex2 { lt: true, buttons: bit(MISC2) | bit(MISC6), ..Apex2::default() }.apply(&mut s);
        assert_eq!((s.axes[axis::LEFT_TRIGGER], s.axes[axis::RIGHT_TRIGGER]), (i16::MAX, 0));
        assert_eq!(s.buttons, 1 << button::SOUTH | bit(MISC2) | bit(MISC6));
        // Released again, whatever SDL thinks of the extras.
        s.buttons |= bit(MISC3);
        Apex2::default().apply(&mut s);
        assert_eq!(s.buttons, 1 << button::SOUTH);
    }

    #[test]
    fn a_full_trigger_reaches_games_as_255() {
        let mut s = PadState::default();
        Apex2 { lt: true, rt: true, ..Apex2::default() }.apply(&mut s);
        let x = crate::mapping::to_xusb(&s);
        assert_eq!((x.left_trigger, x.right_trigger), (255, 255));
        let mut rest = PadState { axes: [0, 0, 0, 0, 16576, 16964], ..PadState::default() };
        assert_eq!(crate::mapping::to_xusb(&rest).left_trigger, 129, "what games got before");
        Apex2::default().apply(&mut rest);
        assert_eq!(crate::mapping::to_xusb(&rest).left_trigger, 0);
    }
}
