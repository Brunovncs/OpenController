//! What happens to a controller's movement on its way to the virtual Xbox controller: a radial
//! deadzone and anti-deadzone for the sticks, the gyro turned into right-stick movement, and the
//! touchpad split into buttons. Pure functions, so they are tested without a controller.

use crate::profile::{Gyro, Sticks};
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
/// The smallest stick travel a turn past the deadzone gives, so games react to slow turns.
const GYRO_ANTI_DEADZONE: f32 = 0.12;

/// The gyro's contribution to the right stick, kept between reports: a filter per axis that
/// smooths slow, fine movement and lets fast turns through.
#[derive(Clone, Copy, Debug, Default)]
pub struct GyroAim {
    filters: [OneEuro; 2],
}

impl GyroAim {
    /// `rate` is SDL's gyro reading in radians per second (pitch, yaw, roll), `dt` the seconds
    /// since the last reading. Returns right-stick X and Y in Xbox convention (Y up).
    pub fn update(&mut self, rate: [f32; 3], dt: f32, g: Gyro) -> (i16, i16) {
        // Turning left is a positive yaw, and turns the camera left: stick X negative. Tilting
        // the top up is a positive pitch, and looks up: stick Y positive.
        let yaw = self.filters[0].filter(rate[1], dt);
        let pitch = self.filters[1].filter(rate[0], dt);
        // At 100 % a half turn a second (pi rad/s) is the stick all the way.
        let gain = f32::from(g.sensitivity) / 100. / PI;
        let axis = |v: f32| {
            let m = v.abs();
            if m < GYRO_DEADZONE {
                return 0.;
            }
            let travel = ((m - GYRO_DEADZONE) * gain).min(1.);
            (GYRO_ANTI_DEADZONE + (1. - GYRO_ANTI_DEADZONE) * travel).min(1.) * v.signum()
        };
        let y = axis(pitch) * if g.invert_y { -1. } else { 1. };
        (to_axis(-axis(yaw)), to_axis(y))
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
    let down: Vec<&Finger> = fingers.iter().filter(|f| f.down).collect();
    let two = down.len() >= 2;
    // A click with one finger is on the side that finger is on; with none (a click without a
    // finger SDL saw), it counts as neither half.
    let side = down.first().map(|f| f.x < 0.5);
    let left = clicked && !two && side == Some(true);
    let right = clicked && !two && side == Some(false);
    (left, right, two)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::GyroMode;

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
        let g = Gyro { mode: GyroMode::Always, sensitivity: 100, invert_y: false };
        let mut aim = GyroAim::default();
        assert_eq!(aim.update([0.01, -0.02, 0.5], 0.004, g), (0, 0), "noise and roll do nothing");
        let mut aim = GyroAim::default();
        let (x, y) = aim.update([0., 1.0, 0.], 0.004, g);
        assert!(x < -10000 && y == 0, "turning left looks left: {x}");
        let mut aim = GyroAim::default();
        let (_, y) = aim.update([1.0, 0., 0.], 0.004, g);
        assert!(y > 10000, "tilting up looks up");
        let mut aim = GyroAim::default();
        let (_, y) = aim.update([1.0, 0., 0.], 0.004, Gyro { invert_y: true, ..g });
        assert!(y < -10000, "inverted");
        let mut aim = GyroAim::default();
        assert_eq!(aim.update([0., -10., 0.], 0.004, g).0, 32767, "a fast turn is the stick all the way");
    }

    #[test]
    fn gyro_is_smoothed_when_slow() {
        let g = Gyro { mode: GyroMode::Always, sensitivity: 100, invert_y: false };
        let mut aim = GyroAim::default();
        aim.update([0., 0., 0.], 0.004, g);
        let (x, _) = aim.update([0., 0.2, 0.], 0.004, g);
        let mut raw = GyroAim::default();
        let (x_raw, _) = raw.update([0., 0.2, 0.], 0.004, g);
        assert!(x.abs() < x_raw.abs(), "a sudden small change is eased in: {x} vs {x_raw}");
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
