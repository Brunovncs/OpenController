//! The DualShock 4 games see when a controller is set to show itself as one instead of as an Xbox
//! controller. It is built from the Xbox report every controller already gets, plus the motion
//! sensors, which an Xbox controller has no place for: games and Steam Input that read a
//! DualShock 4 then get the gyro and the accelerometer, as with DS4Windows. Windows only, through
//! ViGEmBus or VIIPER, which lay the same report out each their own way.

use crate::device::Power;
use crate::mapping::{XusbReport, xusb};

pub const STANDARD_GRAVITY: f32 = 9.80665;

/// The motion sensors in SDL's frame: the gyro in radians per second (pitch, yaw, roll) and the
/// accelerometer in m/s² (x to the right, y up, z towards the player). A DualShock 4's own frame
/// is the same, so the values go straight across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub gyro: [f32; 3],
    pub accel: [f32; 3],
}

impl Motion {
    /// A controller lying still and flat, for one without sensors.
    pub const REST: Motion = Motion { gyro: [0.; 3], accel: [0., STANDARD_GRAVITY, 0.] };
}

/// DualShock 4 button bits, as its report and both drivers have them.
pub mod button {
    pub const SQUARE: u16 = 0x0010;
    pub const CROSS: u16 = 0x0020;
    pub const CIRCLE: u16 = 0x0040;
    pub const TRIANGLE: u16 = 0x0080;
    pub const L1: u16 = 0x0100;
    pub const R1: u16 = 0x0200;
    pub const L2: u16 = 0x0400;
    pub const R2: u16 = 0x0800;
    pub const SHARE: u16 = 0x1000;
    pub const OPTIONS: u16 = 0x2000;
    pub const L3: u16 = 0x4000;
    pub const R3: u16 = 0x8000;
}

/// The d-pad as directions held, which each driver turns into its own form.
pub mod dpad {
    pub const UP: u8 = 1;
    pub const DOWN: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
}

/// What a virtual DualShock 4 shows. Sticks are 0..=255 with 128 at rest and Y growing
/// downwards, as on the controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ds4Report {
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    /// [`button`] bits.
    pub buttons: u16,
    /// [`dpad`] bits.
    pub dpad: u8,
    pub ps: bool,
    pub touchpad: bool,
    pub l2: u8,
    pub r2: u8,
    /// The charge in tenths (0 to 10, 11 full on a cable) and 0x10 while on a cable, as byte 30
    /// of the report has it.
    pub battery: u8,
    pub motion: Motion,
}

impl Default for Ds4Report {
    fn default() -> Self {
        Ds4Report {
            lx: 128,
            ly: 128,
            rx: 128,
            ry: 128,
            buttons: 0,
            dpad: 0,
            ps: false,
            touchpad: false,
            l2: 0,
            r2: 0,
            battery: CABLE | 11,
            motion: Motion::REST,
        }
    }
}

const CABLE: u8 = 0x10;

/// An Xbox stick axis (Y up) to a DualShock 4 one.
fn stick(v: i16) -> u8 {
    ((i32::from(v) + 32768) >> 8) as u8
}

fn stick_y(v: i16) -> u8 {
    (((-i32::from(v)).min(32767) + 32768) >> 8) as u8
}

/// The DualShock 4 version of an Xbox report: same buttons by position (A is Cross), Back and
/// Start as Share and Options, the Xbox button as PS.
pub fn from_xusb(r: &XusbReport, touchpad: bool, battery: u8, motion: Motion) -> Ds4Report {
    const BUTTONS: [(u16, u16); 10] = [
        (xusb::A, button::CROSS),
        (xusb::B, button::CIRCLE),
        (xusb::X, button::SQUARE),
        (xusb::Y, button::TRIANGLE),
        (xusb::LEFT_SHOULDER, button::L1),
        (xusb::RIGHT_SHOULDER, button::R1),
        (xusb::BACK, button::SHARE),
        (xusb::START, button::OPTIONS),
        (xusb::LEFT_THUMB, button::L3),
        (xusb::RIGHT_THUMB, button::R3),
    ];
    const DPAD: [(u16, u8); 4] =
        [(xusb::DPAD_UP, dpad::UP), (xusb::DPAD_DOWN, dpad::DOWN), (xusb::DPAD_LEFT, dpad::LEFT), (xusb::DPAD_RIGHT, dpad::RIGHT)];
    let mut buttons = BUTTONS.iter().filter(|(x, _)| r.buttons & x != 0).fold(0, |b, (_, d)| b | d);
    // The controller reports a trigger as a button too, once it moves.
    if r.left_trigger > 0 {
        buttons |= button::L2;
    }
    if r.right_trigger > 0 {
        buttons |= button::R2;
    }
    Ds4Report {
        lx: stick(r.thumb_lx),
        ly: stick_y(r.thumb_ly),
        rx: stick(r.thumb_rx),
        ry: stick_y(r.thumb_ry),
        buttons,
        dpad: DPAD.iter().filter(|(x, _)| r.buttons & x != 0).fold(0, |b, (_, d)| b | d),
        ps: r.buttons & xusb::GUIDE != 0,
        touchpad,
        l2: r.left_trigger,
        r2: r.right_trigger,
        battery,
        motion,
    }
}

/// The d-pad as the report's hat: 0 up, clockwise to 7 up-left, 8 at rest. Opposite directions
/// held together cancel out.
pub fn hat(d: u8) -> u8 {
    let up = d & dpad::UP != 0 && d & dpad::DOWN == 0;
    let down = d & dpad::DOWN != 0 && d & dpad::UP == 0;
    let left = d & dpad::LEFT != 0 && d & dpad::RIGHT == 0;
    let right = d & dpad::RIGHT != 0 && d & dpad::LEFT == 0;
    match (up, right, down, left) {
        (true, false, _, false) => 0,
        (true, true, _, _) => 1,
        (false, true, false, _) => 2,
        (_, true, true, _) => 3,
        (_, false, true, false) => 4,
        (_, _, true, true) => 5,
        (false, _, false, true) => 6,
        (true, _, _, true) => 7,
        _ => 8,
    }
}

/// What byte 30 of the report says for a controller's battery: the charge in tenths, and the
/// cable bit while it charges or has no battery. Unknown reads as full on a cable, so a game warns
/// of nothing it cannot know.
pub fn battery(p: Power) -> u8 {
    let tenths = |pct: Option<u8>| pct.map_or(8, |p| (p / 10).min(10));
    match p {
        Power::Battery(pct) => tenths(pct),
        Power::Charging(pct) => CABLE | tenths(pct),
        _ => CABLE | 11,
    }
}

/// How a virtual DualShock 4 scales its sensors: what its calibration report (feature 0x02) says,
/// which is what games read it with. Per axis, the raw value at rest and the raw counts per unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    /// Per degree per second.
    pub gyro: [(f32, f32); 3],
    /// Per g.
    pub accel: [(f32, f32); 3],
}

impl Calibration {
    /// VIIPER's (`device/dualshock4`): no bias, 16 counts per °/s and 8192 per g.
    pub const VIIPER: Calibration = Calibration { gyro: [(0., 16.); 3], accel: [(0., 8192.); 3] };

    /// From a calibration report as a USB DualShock 4 sends it, read as SDL reads it
    /// (`HIDAPI_DriverPS4_LoadOfficialCalibrationData`).
    pub fn from_usb_report(r: &[u8; 37]) -> Calibration {
        let w = |i: usize| i32::from(i16::from_le_bytes([r[i], r[i + 1]]));
        let speed = (w(19) + w(21)) as f32;
        // A report without a speed or a range would scale by infinity or by nothing: such an
        // axis takes the counts a real controller has, VIIPER's.
        let or = |per: f32, real: f32| if per.is_finite() && per > 0. { per } else { real };
        let gyro = |bias: i32, plus: i32, minus: i32| {
            (bias as f32, or(((plus - bias).abs() + (minus - bias).abs()) as f32 / speed, Calibration::VIIPER.gyro[0].1))
        };
        let accel = |plus: i32, minus: i32| {
            let range = plus - minus;
            ((plus - range / 2) as f32, or(range as f32 / 2., Calibration::VIIPER.accel[0].1))
        };
        Calibration {
            gyro: [gyro(w(1), w(7), w(9)), gyro(w(3), w(11), w(13)), gyro(w(5), w(15), w(17))],
            accel: [accel(w(23), w(25)), accel(w(27), w(29)), accel(w(31), w(33))],
        }
    }

    /// The raw gyro and accelerometer values that read as `m` with this calibration.
    pub fn encode(&self, m: &Motion) -> ([i16; 3], [i16; 3]) {
        let raw = |(bias, per): (f32, f32), v: f32| (bias + v * per).round().clamp(-32768., 32767.) as i16;
        let mut gyro = [0; 3];
        let mut accel = [0; 3];
        for i in 0..3 {
            gyro[i] = raw(self.gyro[i], m.gyro[i].to_degrees());
            accel[i] = raw(self.accel[i], m.accel[i] / STANDARD_GRAVITY);
        }
        (gyro, accel)
    }
}

/// The calibration report ViGEmBus's DualShock 4 answers with (`Ds4Pdo.cpp`, from a real one).
pub const VIGEM_CALIBRATION: [u8; 37] = [
    0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0x22, 0x7B, 0xDD, 0xB2, 0x22, 0x47, 0xDD, 0xBD, 0x22, 0x43, 0xDD, 0x1C, 0x02, 0x1C,
    0x02, 0x7F, 0x1E, 0x2E, 0xDF, 0x60, 0x1F, 0x4C, 0xE0, 0x3A, 0x1D, 0xC6, 0xDE, 0x08, 0x00,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_is_rest() {
        let r = from_xusb(&XusbReport::default(), false, battery(Power::Wired), Motion::REST);
        assert_eq!(r, Ds4Report::default());
        assert_eq!(hat(r.dpad), 8);
    }

    #[test]
    fn buttons_follow_position() {
        let x = XusbReport { buttons: xusb::A | xusb::Y | xusb::BACK | xusb::GUIDE | xusb::RIGHT_THUMB, ..XusbReport::default() };
        let r = from_xusb(&x, true, 0, Motion::REST);
        assert_eq!(r.buttons, button::CROSS | button::TRIANGLE | button::SHARE | button::R3);
        assert!(r.ps && r.touchpad);
        let r = from_xusb(&XusbReport { left_trigger: 1, right_trigger: 0, ..XusbReport::default() }, false, 0, Motion::REST);
        assert_eq!((r.buttons, r.l2), (button::L2, 1), "a trigger that moved is pressed");
    }

    #[test]
    fn sticks_keep_their_ends_and_flip_y() {
        let x = XusbReport { thumb_lx: i16::MIN, thumb_ly: i16::MAX, thumb_rx: i16::MAX, thumb_ry: i16::MIN, ..XusbReport::default() };
        let r = from_xusb(&x, false, 0, Motion::REST);
        assert_eq!((r.lx, r.ly, r.rx, r.ry), (0, 0, 255, 255), "left: full left and up; right: full right and down");
    }

    #[test]
    fn the_hat_goes_clockwise() {
        use dpad::*;
        let cases = [(UP, 0), (UP | RIGHT, 1), (RIGHT, 2), (DOWN | RIGHT, 3), (DOWN, 4), (DOWN | LEFT, 5), (LEFT, 6), (UP | LEFT, 7)];
        for (d, h) in cases {
            assert_eq!(hat(d), h, "{d:#x}");
        }
        assert_eq!(hat(UP | DOWN), 8, "opposites cancel");
        assert_eq!(hat(UP | DOWN | LEFT), 6);
    }

    #[test]
    fn battery_byte() {
        assert_eq!(battery(Power::Battery(Some(73))), 7);
        assert_eq!(battery(Power::Charging(Some(100))), 0x10 | 10);
        assert_eq!(battery(Power::Charged), 0x1B);
        assert_eq!(battery(Power::Unknown), 0x1B);
    }

    #[test]
    fn vigem_calibration_reads_as_sdl_reads_it() {
        let c = Calibration::from_usb_report(&VIGEM_CALIBRATION);
        // Pitch: bias 1, plus 8839, minus -8837, 540 + 540 °/s between them.
        assert_eq!(c.gyro[0].0, 1.);
        assert!((c.gyro[0].1 - 17676. / 1080.).abs() < 1e-4, "{:?}", c.gyro[0]);
        // X: plus 7807, minus -8402, so 8104 counts per g around -297.
        assert_eq!(c.accel[0], (7807. - 8104., 8104.5));
        let blank = Calibration::from_usb_report(&[0; 37]);
        assert_eq!(blank, Calibration::VIIPER, "a report without a speed or a range scales by finite counts");
    }

    /// What SDL makes of a raw value, with its own formulas, to check `encode` against.
    fn sdl_reads(c: &Calibration, gyro: [i16; 3], accel: [i16; 3]) -> Motion {
        let mut m = Motion::REST;
        for i in 0..3 {
            m.gyro[i] = ((f32::from(gyro[i]) - c.gyro[i].0) / c.gyro[i].1).to_radians();
            m.accel[i] = (f32::from(accel[i]) - c.accel[i].0) / c.accel[i].1 * STANDARD_GRAVITY;
        }
        m
    }

    #[test]
    fn motion_survives_the_round_trip() {
        let m = Motion { gyro: [1.5, -0.25, 3.], accel: [-4.2, 9.81, 0.7] };
        for c in [Calibration::from_usb_report(&VIGEM_CALIBRATION), Calibration::VIIPER] {
            let (g, a) = c.encode(&m);
            let back = sdl_reads(&c, g, a);
            for i in 0..3 {
                assert!((back.gyro[i] - m.gyro[i]).abs() < 0.002, "{back:?}");
                assert!((back.accel[i] - m.accel[i]).abs() < 0.01, "{back:?}");
            }
        }
        let (g, _) = Calibration::VIIPER.encode(&Motion { gyro: [100.; 3], ..Motion::REST });
        assert_eq!(g, [i16::MAX; 3], "out of range is clamped");
    }
}
