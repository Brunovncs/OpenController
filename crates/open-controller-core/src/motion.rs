//! What happens to a controller's movement on its way to the virtual Xbox controller: a radial
//! deadzone and anti-deadzone for the sticks, the gyro turned into right-stick movement, and the
//! touchpad split into buttons. Pure functions, so they are tested without a controller.

use crate::mapping::{PadState, axis};
use crate::profile::{Gyro, GyroMode, Sticks};
use std::f32::consts::PI;

/// A stick, `x` and `y` in -32768..=32767, through a radial deadzone and anti-deadzone given in
/// percent. Inside the deadzone the stick is centred; past it, travel starts at the
/// anti-deadzone, so a game's own deadzone does not swallow the first millimetres.
pub fn shape_stick(x: i16, y: i16, s: Sticks) -> (i16, i16) {
    if s.deadzone == 0 && s.anti_deadzone == 0 {
        return (x, y);
    }
    let (fx, fy) = (f32::from(x) / 32767., f32::from(y) / 32767.);
    let mag = (fx * fx + fy * fy).sqrt();
    let dz = f32::from(s.deadzone) / 100.;
    let ad = f32::from(s.anti_deadzone) / 100.;
    if mag <= dz || mag == 0. {
        return (0, 0);
    }
    let travel = ((mag - dz) / (1. - dz)).min(1.);
    let out = ad + (1. - ad) * travel;
    let k = out / mag;
    (to_axis(fx * k), to_axis(fy * k))
}

fn to_axis(v: f32) -> i16 {
    (v.clamp(-1., 1.) * 32767.).round() as i16
}

/// Below this rotation the gyro is noise and drift, in radians per second (about 1.7 degrees).
const GYRO_DEADZONE: f32 = 0.03;

/// The left trigger past halfway: aiming, for the gyro.
pub const AIMING: i16 = 16384;

pub fn aiming(s: &PadState) -> bool {
    s.axes[axis::LEFT_TRIGGER] > AIMING
}

/// Whether the gyro moves the stick now. `before` is the previous report, to notice a press;
/// `toggled` is the toggle's state, kept between reports.
pub fn gyro_active(g: &Gyro, now: &PadState, before: &PadState, toggled: &mut bool) -> bool {
    let on = match g.mode {
        GyroMode::Off => false,
        GyroMode::Always => true,
        GyroMode::Aiming => aiming(now),
        GyroMode::Holding(b) if g.toggle => {
            let b = u32::from(b);
            if now.pressed(b) && !before.pressed(b) {
                *toggled = !*toggled;
            }
            *toggled
        }
        // Holding a button already is the way to stop: the off button is not offered here.
        GyroMode::Holding(b) => return now.pressed(u32::from(b)),
    };
    on && !g.off_button.is_some_and(|b| now.pressed(u32::from(b)))
}

/// The gyro's contribution to the right stick, kept between reports: a filter per axis that
/// smooths slow, fine movement and lets fast turns through.
#[derive(Clone, Copy, Debug, Default)]
pub struct GyroAim {
    filters: [OneEuro; 2],
}

impl GyroAim {
    /// `rate` is SDL's gyro reading in radians per second (pitch, yaw, roll), `dt` the seconds
    /// since the last reading, `aiming` whether the left trigger is pulled. Returns right-stick
    /// X and Y in Xbox convention (Y up).
    pub fn update(&mut self, rate: [f32; 3], dt: f32, g: Gyro, aiming: bool) -> (i16, i16) {
        // Turning left is a positive yaw, and turns the camera left: stick X negative. Tilting
        // the top up is a positive pitch, and looks up: stick Y positive.
        let yaw = self.filters[0].filter(rate[1], dt);
        let pitch = self.filters[1].filter(rate[0], dt);
        let speed = yaw.hypot(pitch);
        let base = match g.aim_sensitivity {
            Some(s) if aiming && g.mode != GyroMode::Aiming => s,
            _ => g.sensitivity,
        };
        let mut sens = f32::from(base);
        if let Some(a) = g.acceleration {
            let (from, to) = (f32::from(a.from).to_radians(), f32::from(a.to).to_radians());
            let k = ((speed - from) / (to - from)).clamp(0., 1.);
            sens *= 1. + (f32::from(a.factor) / 100. - 1.) * k;
        }
        // Tightening: below its speed, movement shrinks in proportion, so a tremble falls under
        // the deadzone and a slow, deliberate turn still gets through.
        let below = f32::from(g.tightening).to_radians();
        let tight = if speed < below { speed / below } else { 1. };
        // At 100 % a half turn a second (pi rad/s) is the stick all the way.
        let gain = sens / 100. / PI;
        let ad = f32::from(g.anti_deadzone) / 100.;
        let axis = |v: f32, gain: f32| {
            let m = v.abs();
            if m < GYRO_DEADZONE {
                return 0.;
            }
            let travel = ((m - GYRO_DEADZONE) * gain).min(1.);
            (ad + (1. - ad) * travel).min(1.) * v.signum()
        };
        let y = axis(pitch * tight, gain * f32::from(g.y_scale) / 100.) * if g.invert_y { -1. } else { 1. };
        (to_axis(-axis(yaw * tight, gain)), to_axis(y))
    }

    pub fn reset(&mut self) {
        *self = GyroAim::default();
    }
}

/// The one-euro filter (Casiez, Roussel and Vogel, 2012): a low-pass whose cutoff rises with
/// speed, so a still hand gives a still stick and a flick is not delayed.
#[derive(Clone, Copy, Debug, Default)]
struct OneEuro {
    value: Option<f32>,
    speed: f32,
}

impl OneEuro {
    const MIN_CUTOFF: f32 = 1.0;
    const BETA: f32 = 0.6;
    const SPEED_CUTOFF: f32 = 1.0;

    fn alpha(cutoff: f32, dt: f32) -> f32 {
        let tau = 1. / (2. * PI * cutoff);
        1. / (1. + tau / dt)
    }

    fn filter(&mut self, x: f32, dt: f32) -> f32 {
        let dt = dt.clamp(0.0005, 0.1);
        let Some(prev) = self.value else {
            self.value = Some(x);
            return x;
        };
        let speed = (x - prev) / dt;
        let a = Self::alpha(Self::SPEED_CUTOFF, dt);
        self.speed += a * (speed - self.speed);
        let cutoff = Self::MIN_CUTOFF + Self::BETA * self.speed.abs();
        let v = prev + Self::alpha(cutoff, dt) * (x - prev);
        self.value = Some(v);
        v
    }
}

/// A finger on the touchpad: whether it is down, and where, 0..1 from the left and from the top.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Finger {
    pub down: bool,
    pub x: f32,
    pub y: f32,
}

/// The touchpad's own buttons: clicking its left or right half, and touching it with two
/// fingers. Returned as (left, right, two).
pub fn touch_buttons(clicked: bool, fingers: &[Finger]) -> (bool, bool, bool) {
    let mut down = fingers.iter().filter(|f| f.down);
    let first = down.next();
    let two = down.next().is_some();
    // A click with one finger is on the side that finger is on; with none (a click without a
    // finger SDL saw), it counts as neither half.
    let side = first.map(|f| f.x < 0.5);
    let left = clicked && !two && side == Some(true);
    let right = clicked && !two && side == Some(false);
    (left, right, two)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Acceleration;

    #[test]
    fn sticks_pass_through_untouched_by_default() {
        assert_eq!(shape_stick(1200, -30000, Sticks::default()), (1200, -30000));
    }

    #[test]
    fn deadzone_centres_and_rescales() {
        let s = Sticks { deadzone: 10, anti_deadzone: 0 };
        assert_eq!(shape_stick(3000, 0, s), (0, 0), "inside 10 %");
        assert_eq!(shape_stick(32767, 0, s), (32767, 0), "full travel stays full");
        let (x, _) = shape_stick(16384, 0, s);
        assert!((x - 14563).abs() < 40, "half travel is rescaled past the deadzone: {x}");
    }

    #[test]
    fn anti_deadzone_jumps_past_the_games_own() {
        let s = Sticks { deadzone: 5, anti_deadzone: 20 };
        let (x, y) = shape_stick(2000, 0, s);
        assert!(x >= 6553 && y == 0, "just past the deadzone starts at 20 %: {x}");
        let (x, y) = shape_stick(-20000, -20000, s);
        assert!(x < 0 && y < 0 && x == y, "direction is kept");
    }

    #[test]
    fn gyro_turns_and_tilts_the_right_stick() {
        let g = Gyro { mode: GyroMode::Always, ..Gyro::default() };
        let mut aim = GyroAim::default();
        assert_eq!(aim.update([0.01, -0.02, 0.5], 0.004, g, false), (0, 0), "noise and roll do nothing");
        let mut aim = GyroAim::default();
        let (x, y) = aim.update([0., 1.0, 0.], 0.004, g, false);
        assert!(x < -10000 && y == 0, "turning left looks left: {x}");
        let mut aim = GyroAim::default();
        let (_, y) = aim.update([1.0, 0., 0.], 0.004, g, false);
        assert!(y > 10000, "tilting up looks up");
        let mut aim = GyroAim::default();
        let (_, y) = aim.update([1.0, 0., 0.], 0.004, Gyro { invert_y: true, ..g }, false);
        assert!(y < -10000, "inverted");
        let mut aim = GyroAim::default();
        assert_eq!(aim.update([0., -10., 0.], 0.004, g, false).0, 32767, "a fast turn is the stick all the way");
    }

    #[test]
    fn gyro_is_smoothed_when_slow() {
        let g = Gyro { mode: GyroMode::Always, ..Gyro::default() };
        let mut aim = GyroAim::default();
        aim.update([0., 0., 0.], 0.004, g, false);
        let (x, _) = aim.update([0., 0.2, 0.], 0.004, g, false);
        let mut raw = GyroAim::default();
        let (x_raw, _) = raw.update([0., 0.2, 0.], 0.004, g, false);
        assert!(x.abs() < x_raw.abs(), "a sudden small change is eased in: {x} vs {x_raw}");
    }

    /// One reading on a fresh filter, which lets it through as it is.
    fn once(rate: [f32; 3], g: Gyro, aiming: bool) -> (i16, i16) {
        GyroAim::default().update(rate, 0.004, g, aiming)
    }

    fn ratio(a: i16, b: i16) -> f32 {
        f32::from(a) / f32::from(b)
    }

    #[test]
    fn gyro_defaults_are_what_0_8_0_did() {
        // 0.8.0: a 0.03 rad/s deadzone and a 12 % anti-deadzone on each axis, one gain for both.
        let old = |v: f32, sensitivity: u16| {
            let m = v.abs();
            if m < 0.03 {
                return 0;
            }
            let travel = ((m - 0.03) * f32::from(sensitivity) / 100. / PI).min(1.);
            to_axis((0.12 + 0.88 * travel).min(1.) * v.signum())
        };
        for sensitivity in [25, 100, 300] {
            let g = Gyro { mode: GyroMode::Always, sensitivity, ..Gyro::default() };
            for (pitch, yaw) in [(0.02, 0.05), (0.1, -0.4), (-0.7, 1.3), (2.5, 0.2)] {
                assert_eq!(once([pitch, yaw, 0.], g, false), (-old(yaw, sensitivity), old(pitch, sensitivity)));
                assert_eq!(once([pitch, yaw, 0.], g, true), once([pitch, yaw, 0.], g, false), "no aiming sensitivity, no change");
            }
        }
    }

    #[test]
    fn acceleration_turns_fast_movement_further() {
        let plain = Gyro { mode: GyroMode::Always, sensitivity: 25, anti_deadzone: 0, ..Gyro::default() };
        let g = Gyro { acceleration: Some(Acceleration { factor: 200, from: 60, to: 180 }), ..plain };
        let at = |deg: f32| {
            let rate = [0., f32::to_radians(deg), 0.];
            ratio(once(rate, g, false).0, once(rate, plain, false).0)
        };
        assert!((at(30.) - 1.).abs() < 0.01, "slow turns keep the sensitivity: {}", at(30.));
        assert!((at(120.) - 1.5).abs() < 0.01, "halfway between, half the gain: {}", at(120.));
        assert!((at(200.) - 2.).abs() < 0.01, "fast turns get all of it: {}", at(200.));
    }

    #[test]
    fn vertical_scale_leaves_sideways_alone() {
        let g = Gyro { mode: GyroMode::Always, anti_deadzone: 0, ..Gyro::default() };
        let (x, y) = once([1., 1., 0.], g, false);
        let (x_half, y_half) = once([1., 1., 0.], Gyro { y_scale: 50, ..g }, false);
        assert_eq!(x, x_half);
        assert!((ratio(y_half, y) - 0.5).abs() < 0.001, "{y_half} vs {y}");
    }

    #[test]
    fn tightening_steadies_slow_movement_only() {
        let g = Gyro { mode: GyroMode::Always, anti_deadzone: 0, ..Gyro::default() };
        let tight = Gyro { tightening: 10, ..g };
        assert_ne!(once([0., 0.05, 0.], g, false).0, 0);
        assert_eq!(once([0., 0.05, 0.], tight, false).0, 0, "a tremble falls under the deadzone");
        let (slow, slow_plain) = (once([0., 0.1, 0.], tight, false).0, once([0., 0.1, 0.], g, false).0);
        assert!(slow < 0 && slow > slow_plain, "slow turns are scaled down: {slow} vs {slow_plain}");
        assert_eq!(once([0., 0.5, 0.], tight, false), once([0., 0.5, 0.], g, false), "past 10 degrees a second, nothing changes");
    }

    #[test]
    fn anti_deadzone_is_the_first_push() {
        let g = |anti_deadzone| Gyro { mode: GyroMode::Always, anti_deadzone, ..Gyro::default() };
        let none = once([0., 0.031, 0.], g(0), false).0;
        let more = once([0., 0.031, 0.], g(24), false).0;
        assert!(none.abs() < 100, "{none}");
        assert!((more + 7864).abs() < 50, "starts at 24 %: {more}");
        assert_eq!(once([0., 0.02, 0.], g(24), false).0, 0, "noise stays still");
    }

    #[test]
    fn aiming_sensitivity_only_while_aiming() {
        let g = Gyro { mode: GyroMode::Always, anti_deadzone: 0, aim_sensitivity: Some(50), ..Gyro::default() };
        let (hip, ads) = (once([0., 1., 0.], g, false).0, once([0., 1., 0.], g, true).0);
        assert!((ratio(ads, hip) - 0.5).abs() < 0.001, "{ads} vs {hip}");
        let only_aiming = Gyro { mode: GyroMode::Aiming, ..g };
        assert_eq!(once([0., 1., 0.], only_aiming, true).0, hip, "when the gyro only works while aiming, it has one sensitivity");
    }

    fn held(buttons: &[u8], lt: i16) -> PadState {
        let mut s = PadState { buttons: buttons.iter().fold(0, |b, &i| b | 1 << i), ..PadState::default() };
        s.axes[axis::LEFT_TRIGGER] = lt;
        s
    }

    #[test]
    fn gyro_on_by_mode() {
        let mut t = false;
        let none = held(&[], 0);
        let g = |mode| Gyro { mode, ..Gyro::default() };
        assert!(!gyro_active(&g(GyroMode::Off), &none, &none, &mut t));
        assert!(gyro_active(&g(GyroMode::Always), &none, &none, &mut t));
        assert!(!gyro_active(&g(GyroMode::Aiming), &held(&[], 9000), &none, &mut t));
        assert!(gyro_active(&g(GyroMode::Aiming), &held(&[], 30000), &none, &mut t));
        let hold = g(GyroMode::Holding(9));
        assert!(gyro_active(&hold, &held(&[9], 0), &none, &mut t));
        assert!(!gyro_active(&hold, &none, &held(&[9], 0), &mut t));
        assert!(!t, "holding leaves the toggle alone");
    }

    #[test]
    fn toggle_turns_on_and_off_with_presses() {
        let g = Gyro { mode: GyroMode::Holding(9), toggle: true, ..Gyro::default() };
        let (up, down) = (held(&[], 0), held(&[9], 0));
        let mut t = false;
        assert!(gyro_active(&g, &down, &up, &mut t), "a press turns it on");
        assert!(gyro_active(&g, &down, &down, &mut t), "keeping it pressed does not repeat");
        assert!(gyro_active(&g, &up, &down, &mut t), "letting go keeps it on");
        assert!(!gyro_active(&g, &down, &up, &mut t), "a second press turns it off");
        assert!(!gyro_active(&g, &up, &down, &mut t));
        assert!(!t, "two presses, back where it started");
    }

    #[test]
    fn the_off_button_wins() {
        let off = |mode| Gyro { mode, off_button: Some(10), toggle: true, ..Gyro::default() };
        let none = held(&[], 0);
        let mut t = false;
        assert!(!gyro_active(&off(GyroMode::Always), &held(&[10], 0), &none, &mut t));
        assert!(gyro_active(&off(GyroMode::Always), &none, &held(&[10], 0), &mut t), "letting go brings it back");
        assert!(!gyro_active(&off(GyroMode::Aiming), &held(&[10], 30000), &none, &mut t));
        let toggle = off(GyroMode::Holding(9));
        assert!(gyro_active(&toggle, &held(&[9], 0), &none, &mut t));
        assert!(!gyro_active(&toggle, &held(&[10], 0), &none, &mut t), "it stops a toggled gyro");
        assert!(gyro_active(&toggle, &none, &held(&[10], 0), &mut t), "which stays toggled on");
    }

    #[test]
    fn touchpad_halves_and_two_fingers() {
        let left = Finger { down: true, x: 0.2, y: 0.5 };
        let right = Finger { down: true, x: 0.8, y: 0.5 };
        assert_eq!(touch_buttons(true, &[left]), (true, false, false));
        assert_eq!(touch_buttons(true, &[right]), (false, true, false));
        assert_eq!(touch_buttons(false, &[left]), (false, false, false), "touching is not clicking");
        assert_eq!(touch_buttons(false, &[left, right]), (false, false, true));
        assert_eq!(touch_buttons(true, &[left, right]), (false, false, true), "a two-finger click is two fingers");
        assert_eq!(touch_buttons(true, &[]), (false, false, false));
    }
}
