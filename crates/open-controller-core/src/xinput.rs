//! XInput, read the way games read it. Used to find which XInput slot a virtual controller got:
//! ViGEmBus can be asked, but it answers 0 for a new controller while another XInput controller
//! already holds slot 0, so the slot is found by sending a marker report and looking for it.

use crate::mapping::XusbReport;
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

pub const SLOTS: u8 = 4;
/// `XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE`; the right one is larger.
const DEADZONE: i16 = 7849;

/// The sticks of the controller in `slot`, or `None` if the slot is free.
fn sticks(slot: u8) -> Option<[i16; 4]> {
    let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
    (unsafe { XInputGetState(slot as u32, &mut s) } == 0).then(|| {
        let g = s.Gamepad;
        [g.sThumbLX, g.sThumbLY, g.sThumbRX, g.sThumbRY]
    })
}

/// A report no person produces, different for every bus serial: four exact stick positions,
/// all well inside the deadzone every game applies, so nothing moves while it is shown.
pub fn marker(serial: u32) -> XusbReport {
    let a = 1000 + (serial % 64) as i16 * 61;
    let b = 1500 + (serial % 64) as i16 * 47;
    XusbReport { thumb_lx: a, thumb_ly: -b, thumb_rx: -a, thumb_ry: b, ..XusbReport::default() }
}

/// Whether games read a report as a controller nobody touches: no button or trigger, and the
/// sticks inside the deadzone, as a marker is.
pub fn at_rest(r: &XusbReport) -> bool {
    r.buttons == 0
        && r.left_trigger == 0
        && r.right_trigger == 0
        && [r.thumb_lx, r.thumb_ly, r.thumb_rx, r.thumb_ry].iter().all(|v| v.unsigned_abs() < DEADZONE as u16)
}

/// The slot whose controller currently shows `marker`.
pub fn find(marker: &XusbReport) -> Option<u8> {
    let want = [marker.thumb_lx, marker.thumb_ly, marker.thumb_rx, marker.thumb_ry];
    (0..SLOTS).find(|&i| sticks(i) == Some(want))
}

/// Whether some XInput slot is free: a controller without one may still get it.
pub fn has_free_slot() -> bool {
    (0..SLOTS).any(|i| sticks(i).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_are_distinct_and_inside_the_deadzone() {
        let all: Vec<XusbReport> = (1..=16).map(marker).collect();
        for (i, m) in all.iter().enumerate() {
            assert!([m.thumb_lx, m.thumb_ly, m.thumb_rx, m.thumb_ry].iter().all(|v| v.abs() < DEADZONE && *v != 0));
            assert_eq!((m.buttons, m.left_trigger, m.right_trigger), (0, 0, 0));
            assert!(all[i + 1..].iter().all(|o| o != m));
            assert!(at_rest(m));
        }
    }

    #[test]
    fn only_an_untouched_controller_is_at_rest() {
        assert!(at_rest(&XusbReport::default()));
        assert!(at_rest(&XusbReport { thumb_lx: -1200, thumb_ry: 3000, ..XusbReport::default() }), "stick drift");
        assert!(!at_rest(&XusbReport { buttons: 0x1000, ..XusbReport::default() }));
        assert!(!at_rest(&XusbReport { right_trigger: 1, ..XusbReport::default() }));
        assert!(!at_rest(&XusbReport { thumb_lx: -32768, ..XusbReport::default() }));
        assert!(!at_rest(&XusbReport { thumb_ry: DEADZONE, ..XusbReport::default() }));
    }
}
