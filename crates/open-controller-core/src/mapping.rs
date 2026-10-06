//! Turns the state of any gamepad, as SDL normalises it, into an Xbox 360 report.
//!
//! SDL names buttons by position (south, east, west, north), so the bottom face button is A on
//! every controller whatever is printed on it: Cross on a PlayStation pad, B on a Nintendo one.
//! Sticks and triggers are passed through without a deadzone; games apply their own, as they do
//! for a real Xbox controller.

/// SDL's gamepad button indices (`SDL_GamepadButton`), as bit positions in [`PadState::buttons`].
pub mod button {
    pub const SOUTH: u32 = 0;
    pub const EAST: u32 = 1;
    pub const WEST: u32 = 2;
    pub const NORTH: u32 = 3;
    pub const BACK: u32 = 4;
    pub const GUIDE: u32 = 5;
    pub const START: u32 = 6;
    pub const LEFT_STICK: u32 = 7;
    pub const RIGHT_STICK: u32 = 8;
    pub const LEFT_SHOULDER: u32 = 9;
    pub const RIGHT_SHOULDER: u32 = 10;
    pub const DPAD_UP: u32 = 11;
    pub const DPAD_DOWN: u32 = 12;
    pub const DPAD_LEFT: u32 = 13;
    pub const DPAD_RIGHT: u32 = 14;
    pub const MISC1: u32 = 15;
    pub const TOUCHPAD: u32 = 20;
    pub const COUNT: u32 = 26;
}

/// SDL's gamepad axis indices (`SDL_GamepadAxis`), as positions in [`PadState::axes`].
pub mod axis {
    pub const LEFT_X: usize = 0;
    pub const LEFT_Y: usize = 1;
    pub const RIGHT_X: usize = 2;
    pub const RIGHT_Y: usize = 3;
    pub const LEFT_TRIGGER: usize = 4;
    pub const RIGHT_TRIGGER: usize = 5;
    pub const COUNT: usize = 6;
}

/// XUSB button bits (`XINPUT_GAMEPAD_*`).
pub mod xusb {
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    pub const GUIDE: u16 = 0x0400;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;
}

/// One gamepad's input as SDL reports it: a bit per button, sticks in -32768..=32767 with Y
/// growing downwards, triggers in 0..=32767.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PadState {
    pub buttons: u32,
    pub axes: [i16; axis::COUNT],
}

impl PadState {
    pub fn pressed(&self, b: u32) -> bool {
        self.buttons & (1 << b) != 0
    }
}

/// `XUSB_REPORT`, the payload ViGEmBus takes for a virtual Xbox 360 controller. Y grows upwards.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XusbReport {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

const BUTTONS: [(u32, u16); 15] = [
    (button::SOUTH, xusb::A),
    (button::EAST, xusb::B),
    (button::WEST, xusb::X),
    (button::NORTH, xusb::Y),
    (button::BACK, xusb::BACK),
    (button::GUIDE, xusb::GUIDE),
    (button::START, xusb::START),
    (button::LEFT_STICK, xusb::LEFT_THUMB),
    (button::RIGHT_STICK, xusb::RIGHT_THUMB),
    (button::LEFT_SHOULDER, xusb::LEFT_SHOULDER),
    (button::RIGHT_SHOULDER, xusb::RIGHT_SHOULDER),
    (button::DPAD_UP, xusb::DPAD_UP),
    (button::DPAD_DOWN, xusb::DPAD_DOWN),
    (button::DPAD_LEFT, xusb::DPAD_LEFT),
    (button::DPAD_RIGHT, xusb::DPAD_RIGHT),
];

/// SDL's Y axis grows downwards and XInput's upwards. Negating maps -32768 to 32768, one past
/// the range, so the top end is clamped: full deflection stays full and centre stays centre.
fn flip(v: i16) -> i16 {
    (-(v as i32)).min(i16::MAX as i32) as i16
}

/// 0..=32767 to 0..=255, rounded, so a fully pressed trigger is exactly 255.
fn trigger(v: i16) -> u8 {
    let v = v.max(0) as u32;
    ((v * 255 + 16383) / 32767) as u8
}

pub fn to_xusb(s: &PadState) -> XusbReport {
    let mut buttons = 0u16;
    for (from, to) in BUTTONS {
        if s.pressed(from) {
            buttons |= to;
        }
    }
    XusbReport {
        buttons,
        left_trigger: trigger(s.axes[axis::LEFT_TRIGGER]),
        right_trigger: trigger(s.axes[axis::RIGHT_TRIGGER]),
        thumb_lx: s.axes[axis::LEFT_X],
        thumb_ly: flip(s.axes[axis::LEFT_Y]),
        thumb_rx: s.axes[axis::RIGHT_X],
        thumb_ry: flip(s.axes[axis::RIGHT_Y]),
    }
}

/// Axis movement below this is noise, as far as choosing between two connections goes.
const NOISE: i32 = 2048;

/// A change a person made: a button, or an axis moved further than stick noise.
pub fn significant(before: &PadState, after: &PadState) -> bool {
    before.buttons != after.buttons || before.axes.iter().zip(after.axes).any(|(&a, b)| (a as i32 - b as i32).abs() > NOISE)
}

/// XInput motor speed (0..=255) to SDL's rumble intensity (0..=65535).
pub fn motor(v: u8) -> u16 {
    u16::from(v) * 257
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(buttons: &[u32], axes: [i16; 6]) -> PadState {
        PadState { buttons: buttons.iter().fold(0, |b, &i| b | 1 << i), axes }
    }

    #[test]
    fn neutral_is_neutral() {
        assert_eq!(to_xusb(&PadState::default()), XusbReport::default());
    }

    #[test]
    fn face_buttons_follow_position() {
        let r = to_xusb(&state(&[button::SOUTH, button::NORTH], [0; 6]));
        assert_eq!(r.buttons, xusb::A | xusb::Y);
        let r = to_xusb(&state(&[button::EAST, button::WEST], [0; 6]));
        assert_eq!(r.buttons, xusb::B | xusb::X);
    }

    #[test]
    fn every_mapped_button_has_its_own_bit() {
        let all: Vec<u32> = BUTTONS.iter().map(|b| b.0).collect();
        let r = to_xusb(&state(&all, [0; 6]));
        assert_eq!(r.buttons.count_ones(), 15);
        assert_eq!(r.buttons & 0x0800, 0, "bit 11 is unused in XUSB");
    }

    #[test]
    fn unmapped_buttons_are_ignored() {
        let r = to_xusb(&state(&[button::TOUCHPAD, button::MISC1, 16, 17, 25], [0; 6]));
        assert_eq!(r.buttons, 0);
    }

    #[test]
    fn y_axes_are_flipped_without_losing_the_ends() {
        let r = to_xusb(&state(&[], [0, i16::MIN, 0, i16::MAX, 0, 0]));
        assert_eq!(r.thumb_ly, i16::MAX, "stick fully up");
        assert_eq!(r.thumb_ry, -i16::MAX, "stick fully down");
        let r = to_xusb(&state(&[], [0, 1000, 0, -1000, 0, 0]));
        assert_eq!((r.thumb_ly, r.thumb_ry), (-1000, 1000));
    }

    #[test]
    fn x_axes_pass_through() {
        let r = to_xusb(&state(&[], [i16::MIN, 0, i16::MAX, 0, 0, 0]));
        assert_eq!((r.thumb_lx, r.thumb_rx), (i16::MIN, i16::MAX));
    }

    #[test]
    fn triggers_scale_to_a_byte() {
        let t = |v| to_xusb(&state(&[], [0, 0, 0, 0, v, v])).left_trigger;
        assert_eq!(t(0), 0);
        assert_eq!(t(i16::MAX), 255);
        assert_eq!(t(16384), 128);
        assert_eq!(t(-5), 0, "SDL never sends this, but it must not wrap");
        let r = to_xusb(&state(&[], [0, 0, 0, 0, 0, i16::MAX]));
        assert_eq!((r.left_trigger, r.right_trigger), (0, 255));
    }

    #[test]
    fn report_has_the_xusb_layout() {
        assert_eq!(std::mem::size_of::<XusbReport>(), 12);
    }

    #[test]
    fn noise_is_not_a_change() {
        let rest = state(&[], [100, -80, 0, 0, 0, 0]);
        assert!(!significant(&rest, &state(&[], [140, -20, 30, 5, 900, 0])));
        assert!(significant(&rest, &state(&[], [9000, -80, 0, 0, 0, 0])));
        assert!(significant(&rest, &state(&[button::START], [100, -80, 0, 0, 0, 0])));
    }

    #[test]
    fn motors_span_the_full_range() {
        assert_eq!(motor(0), 0);
        assert_eq!(motor(255), u16::MAX);
    }
}
