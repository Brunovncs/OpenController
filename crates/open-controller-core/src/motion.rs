//! What happens to a controller's movement on its way to the virtual Xbox controller: the sticks'
//! centre, deadzones, full tilt and curve, and measuring a stick for them; a stick as the d-pad,
//! pressure-sensitive buttons on the triggers, the gyro turned into right-stick movement, the
//! rotation an accelerometer shows, and the touchpad split into buttons. Pure functions, so they
//! are tested without a controller.

use crate::mapping::{self, PadState, axis, button};
use crate::profile::{DpadStick, Gyro, GyroMode, Pressure, Shape, Sticks};
use std::f32::consts::PI;

/// How far games see a stick pushed `travel` of the way from its centre (0 to 1 and past it on
/// square gates): nothing inside the deadzone, then from the anti-deadzone up to all the way at
/// full tilt, along the curve.
pub fn response(travel: f32, s: Shape) -> f32 {
    let dz = f32::from(s.deadzone) / 100.;
    if travel <= dz || travel <= 0. {
        return 0.;
    }
    let ad = f32::from(s.anti_deadzone) / 100.;
    // A full tilt inside the deadzone would leave no travel to rise over.
    let full = (f32::from(s.full_tilt) / 100.).max(dz + 0.05);
    let past = ((travel - dz) / (full - dz)).min(1.);
    let curved = if s.curve == Shape::LINEAR { past } else { past.powf(f32::from(s.curve) / 100.) };
    ad + (1. - ad) * curved
}

/// A stick, `x` and `y` in -32768..=32767, taken from where it rests (`centre`) and through its
/// shape, radially: the direction is kept and only the distance changes. Inside the deadzone the
/// stick is centred; past it, travel starts at the anti-deadzone, so a game's own deadzone does
/// not swallow the first millimetres.
pub fn shape_stick(x: i16, y: i16, s: Shape, centre: [i16; 2]) -> (i16, i16) {
    if s == Shape::default() && centre == [0, 0] {
        return (x, y);
    }
    let fx = (f32::from(x) - f32::from(centre[0])) / 32767.;
    let fy = (f32::from(y) - f32::from(centre[1])) / 32767.;
    let mag = fx.hypot(fy);
    if mag == 0. {
        return (0, 0);
    }
    let k = response(mag, s) / mag;
    (to_axis(fx * k), to_axis(fy * k))
}

/// Both sticks of a state through their shapes.
pub fn shape_sticks(state: &mut PadState, s: &Sticks) {
    for (right, (ax, ay)) in [(false, (axis::LEFT_X, axis::LEFT_Y)), (true, (axis::RIGHT_X, axis::RIGHT_Y))] {
        let (x, y) = shape_stick(state.axes[ax], state.axes[ay], s.shape(right), s.centre[usize::from(right)]);
        state.axes[ax] = x;
        state.axes[ay] = y;
    }
}

/// A stick left alone, measured: where it rests, and the deadzone that hides how it wanders there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rest {
    pub centre: [i16; 2],
    /// How far it wandered from there, in tenths of a percent of its travel.
    pub wander: u16,
    pub deadzone: u8,
}

/// Wandering past this, in tenths of a percent, means the stick was being moved.
const MAX_WANDER: u16 = 120;

/// Where a stick rests, from its positions while left alone; `None` without positions, or when
/// it moved or rested too far off to be drifting.
pub fn measure_rest(samples: &[[i16; 2]]) -> Option<Rest> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len() as f32;
    let cx = samples.iter().map(|s| f32::from(s[0])).sum::<f32>() / n;
    let cy = samples.iter().map(|s| f32::from(s[1])).sum::<f32>() / n;
    let far = samples.iter().map(|s| (f32::from(s[0]) - cx).hypot(f32::from(s[1]) - cy)).fold(0., f32::max);
    let wander = (far / 32767. * 1000.).ceil() as u16;
    let centre = [cx.round() as i16, cy.round() as i16];
    if wander > MAX_WANDER || centre.iter().any(|c| c.unsigned_abs() > Sticks::MAX_CENTRE as u16) {
        return None;
    }
    // A little more than it wandered while measured, as it may wander further later on.
    let deadzone = (wander.div_ceil(10) + 2).min(u16::from(Shape::MAX_DEADZONE)) as u8;
    Some(Rest { centre, wander, deadzone })
}

/// Directions a turn around the edge is split into; each must be reached.
pub const EDGE_SECTORS: usize = 16;

/// Closer to the centre than this a position is not at the edge, however worn the stick.
const EDGE_MIN: f32 = 0.7;

/// A stick turned around its edge, measured: how far it reaches in each direction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edge {
    reach: [f32; EDGE_SECTORS],
}

impl Edge {
    pub fn add(&mut self, x: i16, y: i16, centre: [i16; 2]) {
        let fx = (f32::from(x) - f32::from(centre[0])) / 32767.;
        let fy = (f32::from(y) - f32::from(centre[1])) / 32767.;
        let r = fx.hypot(fy);
        if r < EDGE_MIN {
            return;
        }
        let sector = ((fy.atan2(fx) + PI) / (2. * PI) * EDGE_SECTORS as f32) as usize % EDGE_SECTORS;
        self.reach[sector] = self.reach[sector].max(r);
    }

    /// Whether the stick reached its edge in direction `i`, counted from the left, clockwise on
    /// the screen.
    pub fn sector(&self, i: usize) -> bool {
        self.reach.get(i).is_some_and(|&r| r > 0.)
    }

    /// How many of the directions the stick has reached its edge in.
    pub fn covered(&self) -> usize {
        self.reach.iter().filter(|&&r| r > 0.).count()
    }

    /// Once every direction was reached: the shortest reach, in tenths of a percent.
    pub fn least(&self) -> Option<u16> {
        (self.covered() == EDGE_SECTORS).then(|| (self.reach.iter().copied().fold(f32::MAX, f32::min) * 1000.).floor() as u16)
    }

    /// The full tilt every direction gets to: a hair inside the shortest reach, or the edge when
    /// the stick reaches it all round.
    pub fn full_tilt(&self) -> Option<u8> {
        self.least().map(|l| if l >= 980 { 100 } else { (l / 10).saturating_sub(1).max(u16::from(Shape::MIN_FULL_TILT)) as u8 })
    }
}

fn to_axis(v: f32) -> i16 {
    (v.clamp(-1., 1.) * 32767.).round() as i16
}

/// Past this much of its travel, a stick standing in for the d-pad presses a direction.
const DPAD_TRAVEL: f32 = 0.5;

/// A stick as d-pad directions, eight ways, as SDL button bits: past half its travel, the
/// direction it points in, each covering 45 degrees.
pub fn stick_dpad(x: i16, y: i16) -> u64 {
    let (fx, fy) = (f32::from(x) / 32767., f32::from(y) / 32767.);
    if fx.hypot(fy) < DPAD_TRAVEL {
        return 0;
    }
    let (up, down, left, right) = (1 << button::DPAD_UP, 1 << button::DPAD_DOWN, 1 << button::DPAD_LEFT, 1 << button::DPAD_RIGHT);
    // SDL's Y grows downwards; counted from the right, anticlockwise.
    let sector = ((-fy).atan2(fx) / (PI / 4.)).round() as i32;
    match sector.rem_euclid(8) {
        0 => right,
        1 => up | right,
        2 => up,
        3 => up | left,
        4 => left,
        5 => down | left,
        6 => down,
        _ => down | right,
    }
}

/// The chosen stick turned into the d-pad: its directions pressed, the stick itself at rest.
pub fn apply_dpad_stick(s: &mut PadState, which: DpadStick) {
    let (ax, ay) = match which {
        DpadStick::Off => return,
        DpadStick::Left => (axis::LEFT_X, axis::LEFT_Y),
        DpadStick::Right => (axis::RIGHT_X, axis::RIGHT_Y),
    };
    s.buttons |= stick_dpad(s.axes[ax], s.axes[ay]);
    s.axes[ax] = 0;
    s.axes[ay] = 0;
}

/// Pressure-sensitive buttons on the triggers: how hard the chosen button is pressed is how far
/// the trigger goes, as racing games on the PlayStation 2 and 3 used them. The button itself is
/// let go unless it is kept, so it does not press A at the same time.
pub fn apply_pressure(s: &mut PadState, p: Pressure) {
    for (b, ax) in [(p.left_trigger, axis::LEFT_TRIGGER), (p.right_trigger, axis::RIGHT_TRIGGER)] {
        let Some(i) = b.and_then(|b| mapping::pressure_index(u32::from(b))) else { continue };
        let v = (u32::from(s.pressure[i]) * 32767 / 255) as i16;
        s.axes[ax] = s.axes[ax].max(v);
        if !p.keep_buttons
            && let Some(b) = b
        {
            s.buttons &= !(1 << b);
        }
    }
}

/// The rotation an accelerometer can see, in radians per second in SDL's frame, for a controller
/// whose gyro misses axes: the DualShock 3's turns only around one. Held still, the accelerometer
/// reads gravity, which stays put in the world, so in the controller's frame it turns the other
/// way (dS/dt = -w x S). The turn between two readings of it is the part of the controller's turn
/// across gravity; a turn around gravity shows nothing, and that is what the real gyro gives.
///
/// The step between two smoothed directions is GamepadMotionHelpers' sensor fusion (Julian "Jibb"
/// Smart, MIT), as is waiting out readings the controller has not refreshed. Readings that are not
/// just gravity, a jolt or a swing, are left out: their length is far from 1 g or jumps from the
/// last one. The estimate reads still meanwhile, and starts over if they last.
#[derive(Clone, Copy, Debug, Default)]
pub struct TiltRate {
    /// Gravity's direction, smoothed; `None` until a clean reading.
    gravity: Option<V3>,
    /// The last reading and its length in g, to tell a repeat and a jolt.
    last: V3,
    last_g: f32,
    /// Seconds of repeated readings not yet accounted for, and of readings left out.
    repeated: f32,
    unclean: f32,
    pub rate: V3,
}

type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

fn scale(a: V3, k: f32) -> V3 {
    a.map(|v| v * k)
}

impl TiltRate {
    /// The half-life gravity's direction is smoothed with, in seconds: long enough to quiet the
    /// accelerometer's noise, short enough to aim with.
    const HALF_LIFE: f32 = 0.05;
    /// How far from 1 g gravity may read, for a controller that is not calibrated.
    const NEAR_G: f32 = 0.35;
    /// A jump in length this big from one reading to the next, in g, is a jolt.
    const JOLT: f32 = 0.15;
    /// After this long between readings or without a clean one, the estimate starts over rather
    /// than turn what it did not see into one burst.
    const STALE: f32 = 0.25;

    /// `a` is the accelerometer in m/s², `dt` the seconds since the last call.
    pub fn update(&mut self, a: V3, dt: f32) -> V3 {
        if !a.iter().chain([&dt]).all(|v| v.is_finite()) || dt < 0. || a == [0.; 3] {
            *self = TiltRate::default();
            return self.rate;
        }
        // The controller had nothing new: the time counts towards its next reading.
        if self.gravity.is_some() && a == self.last {
            self.repeated += dt;
            return self.rate;
        }
        let dt = dt + std::mem::take(&mut self.repeated);
        let g = length(a) / crate::ds4::STANDARD_GRAVITY;
        let clean = (g - 1.).abs() <= Self::NEAR_G && (g - self.last_g).abs() <= Self::JOLT;
        (self.last, self.last_g) = (a, g);
        let before = match self.gravity {
            Some(before) if dt > 0. && dt <= Self::STALE && self.unclean <= Self::STALE => before,
            _ => {
                self.start(a, g);
                return self.rate;
            }
        };
        if !clean {
            self.unclean += dt;
            self.rate = [0.; 3];
            return self.rate;
        }
        self.unclean = 0.;
        let keep = (-dt / Self::HALF_LIFE).exp2();
        let now: V3 = std::array::from_fn(|i| a[i] + (before[i] - a[i]) * keep);
        let (from, to) = (scale(before, 1. / length(before)), scale(now, 1. / length(now)));
        let axis = cross(to, from);
        let sine = length(axis);
        self.rate = if sine > 0. { scale(axis, sine.atan2(dot(to, from)) / (sine * dt)) } else { [0.; 3] };
        self.gravity = Some(now);
        self.rate
    }

    /// Starts over from this reading, if it can be gravity alone.
    fn start(&mut self, a: V3, g: f32) {
        *self = TiltRate { last: a, last_g: g, ..TiltRate::default() };
        if (g - 1.).abs() <= Self::NEAR_G {
            self.gravity = Some(a);
        }
    }
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
        // One bad sample would otherwise stay in the filter for good.
        if !x.is_finite() {
            return self.value.unwrap_or(0.);
        }
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
    fn a_bad_gyro_sample_does_not_stick_in_the_filter() {
        let mut f = OneEuro::default();
        f.filter(0.5, 0.004);
        assert_eq!(f.filter(f32::NAN, 0.004), 0.5);
        assert!(f.filter(0.6, 0.004).is_finite());
        assert_eq!(OneEuro::default().filter(f32::INFINITY, 0.004), 0.);
    }

    #[test]
    fn sticks_pass_through_untouched_by_default() {
        assert_eq!(shape_stick(1200, -30000, Shape::default(), [0, 0]), (1200, -30000));
        let mut s = PadState { axes: [1200, -30000, 32767, 32767, 0, 0], ..PadState::default() };
        shape_sticks(&mut s, &Sticks::default());
        assert_eq!(s.axes, [1200, -30000, 32767, 32767, 0, 0], "square gates keep their corners");
    }

    #[test]
    fn deadzone_centres_and_rescales() {
        let s = Shape { deadzone: 10, ..Shape::default() };
        assert_eq!(shape_stick(3000, 0, s, [0, 0]), (0, 0), "inside 10 %");
        assert_eq!(shape_stick(32767, 0, s, [0, 0]), (32767, 0), "full travel stays full");
        let (x, _) = shape_stick(16384, 0, s, [0, 0]);
        assert!((x - 14563).abs() < 40, "half travel is rescaled past the deadzone: {x}");
    }

    #[test]
    fn anti_deadzone_jumps_past_the_games_own() {
        let s = Shape { deadzone: 5, anti_deadzone: 20, ..Shape::default() };
        let (x, y) = shape_stick(2000, 0, s, [0, 0]);
        assert!(x >= 6553 && y == 0, "just past the deadzone starts at 20 %: {x}");
        let (x, y) = shape_stick(-20000, -20000, s, [0, 0]);
        assert!(x < 0 && y < 0 && x == y, "direction is kept");
    }

    #[test]
    fn full_tilt_reaches_the_edge_early() {
        let s = Shape { full_tilt: 80, ..Shape::default() };
        assert_eq!(shape_stick(26214, 0, s, [0, 0]), (32767, 0), "80 % of the travel is all the way");
        assert_eq!(shape_stick(0, -30000, s, [0, 0]), (0, -32767));
        let (x, _) = shape_stick(13107, 0, s, [0, 0]);
        assert!((x - 16384).abs() < 40, "40 % is half way: {x}");
        let (x, y) = shape_stick(30000, 30000, s, [0, 0]);
        assert!(x == y && (x - 23170).abs() < 3, "a corner stays on the circle: {x}");
    }

    #[test]
    fn curves_bend_the_middle_and_keep_the_ends() {
        let precise = Shape { curve: 200, ..Shape::default() };
        let quick = Shape { curve: 50, ..Shape::default() };
        assert!((response(0.5, precise) - 0.25).abs() < 1e-4, "squared, half travel is a quarter");
        assert!((response(0.25, quick) - 0.5).abs() < 1e-4);
        for s in [precise, quick] {
            assert_eq!((response(0., s), response(1., s), response(1.3, s)), (0., 1., 1.));
        }
        // The curve runs over what is left between the deadzone and full tilt, from the
        // anti-deadzone up.
        let s = Shape { deadzone: 10, anti_deadzone: 20, full_tilt: 90, curve: 200 };
        assert_eq!(response(0.1, s), 0.);
        assert!((response(0.5, s) - (0.2 + 0.8 * 0.25)).abs() < 1e-4);
        assert_eq!(response(0.9, s), 1.);
        let odd = Shape { deadzone: 40, full_tilt: 30, ..Shape::default() };
        assert!(response(0.42, odd) > 0. && response(0.46, odd) == 1., "a full tilt inside the deadzone still rises");
    }

    #[test]
    fn a_measured_centre_is_taken_away_first() {
        let s = Shape::default();
        assert_eq!(shape_stick(1500, -900, s, [1500, -900]), (0, 0), "resting where it leans is the centre");
        assert_eq!(shape_stick(11500, -900, s, [1500, -900]), (10000, 0));
        let mut state = PadState { axes: [1500, -900, 400, 0, 0, 0], ..PadState::default() };
        let sticks = Sticks { right: Some(Shape { deadzone: 5, ..s }), centre: [[1500, -900], [0, 0]], ..Sticks::default() };
        shape_sticks(&mut state, &sticks);
        assert_eq!(state.axes[..4], [0, 0, 0, 0], "each stick with its own centre and shape");
    }

    #[test]
    fn measuring_where_a_stick_rests() {
        assert_eq!(measure_rest(&[]), None);
        let leaning = [[2000, -1000], [2100, -1000], [1900, -1000], [2000, -1100], [2000, -900]];
        let r = measure_rest(&leaning).unwrap();
        assert_eq!(r.centre, [2000, -1000]);
        assert_eq!((r.wander, r.deadzone), (4, 3), "it wandered 0.3 %: a deadzone just past that");
        let still = measure_rest(&[[0, 0]; 3]).unwrap();
        assert_eq!((still.wander, still.deadzone), (0, 2));
        assert_eq!(measure_rest(&[[0, 0], [9000, 0]]), None, "it was moved");
        assert_eq!(measure_rest(&[[20000, 0]; 4]), None, "it was held over");
    }

    #[test]
    fn measuring_how_far_a_stick_reaches() {
        let turn = |reach: &dyn Fn(f32) -> f32, centre: [i16; 2], steps: usize| {
            let mut e = Edge::default();
            for i in 0..steps {
                let a = i as f32 / steps as f32 * 2. * PI;
                let r = reach(a) * 32767.;
                e.add((a.cos() * r) as i16 + centre[0], (a.sin() * r) as i16 + centre[1], centre);
            }
            e
        };
        let mut e = Edge::default();
        for i in 0..16 {
            let a = i as f32 / 32. * 2. * PI;
            e.add((a.cos() * 32767.) as i16, (a.sin() * 32767.) as i16, [0, 0]);
        }
        assert!(e.covered() < EDGE_SECTORS && e.full_tilt().is_none(), "half a turn is not enough");
        assert_eq!(turn(&|_| 1., [0, 0], 32).full_tilt(), Some(100));
        let worn = turn(&|a| if a > 4. && a < 5. { 0.865 } else { 0.95 }, [0, 0], 64);
        assert_eq!(worn.full_tilt(), Some(85), "the shortest reach, a hair inside");
        let square = turn(&|a| 1. / a.cos().abs().max(a.sin().abs()), [0, 0], 64);
        assert_eq!(square.full_tilt(), Some(100));
        let leaning = turn(&|_| 0.905, [3000, -3000], 64);
        assert_eq!(leaning.full_tilt(), Some(89), "measured from the stick's own centre");
        let mut e = Edge::default();
        e.add(16000, 0, [0, 0]);
        assert_eq!(e.covered(), 0, "the middle is not the edge");
        assert_eq!(turn(&|_| 0.725, [0, 0], 64).full_tilt(), Some(71));
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

    fn bits(buttons: &[u32]) -> u64 {
        buttons.iter().fold(0, |b, &i| b | 1 << i)
    }

    #[test]
    fn a_stick_presses_eight_directions() {
        use button::*;
        let m = 30000;
        assert_eq!(stick_dpad(0, -m), bits(&[DPAD_UP]), "SDL's Y grows downwards");
        assert_eq!(stick_dpad(m, -m), bits(&[DPAD_UP, DPAD_RIGHT]));
        assert_eq!(stick_dpad(m, 0), bits(&[DPAD_RIGHT]));
        assert_eq!(stick_dpad(m, m), bits(&[DPAD_DOWN, DPAD_RIGHT]));
        assert_eq!(stick_dpad(0, m), bits(&[DPAD_DOWN]));
        assert_eq!(stick_dpad(-m, m), bits(&[DPAD_DOWN, DPAD_LEFT]));
        assert_eq!(stick_dpad(-m, 0), bits(&[DPAD_LEFT]));
        assert_eq!(stick_dpad(-m, -m), bits(&[DPAD_UP, DPAD_LEFT]));
        assert_eq!(stick_dpad(30000, -9000), bits(&[DPAD_RIGHT]), "17 degrees up is still right");
        assert_eq!(stick_dpad(12000, -9000), 0, "under half its travel");
    }

    #[test]
    fn the_dpad_stick_rests_while_it_stands_in() {
        let mut s = PadState { axes: [-32768, 0, 5000, 7000, 0, 0], ..PadState::default() };
        apply_dpad_stick(&mut s, DpadStick::Left);
        assert_eq!(s.buttons, bits(&[button::DPAD_LEFT]));
        assert_eq!(s.axes, [0, 0, 5000, 7000, 0, 0], "the other stick is left alone");
        let before = s;
        apply_dpad_stick(&mut s, DpadStick::Off);
        assert_eq!(s, before);
    }

    #[test]
    fn pressure_moves_a_button_onto_a_trigger() {
        let mut pressure = [0u8; 10];
        pressure[mapping::pressure_index(button::SOUTH).unwrap()] = 128;
        pressure[mapping::pressure_index(button::WEST).unwrap()] = 255;
        let held = PadState { buttons: bits(&[button::SOUTH, button::WEST, button::NORTH]), pressure, ..PadState::default() };
        let p = Pressure { left_trigger: Some(button::WEST as u8), right_trigger: Some(button::SOUTH as u8), keep_buttons: false };
        let mut s = held;
        apply_pressure(&mut s, p);
        assert_eq!(s.axes[axis::LEFT_TRIGGER], 32767);
        assert!((s.axes[axis::RIGHT_TRIGGER] - 16448).abs() < 2, "half pressed, half the trigger: {}", s.axes[axis::RIGHT_TRIGGER]);
        assert_eq!(s.buttons, bits(&[button::NORTH]), "Cross and Square no longer press A and X");
        let mut s = held;
        apply_pressure(&mut s, Pressure { keep_buttons: true, ..p });
        assert_eq!(s.buttons, held.buttons);
        let mut s = PadState { axes: [0, 0, 0, 0, 0, 30000], ..held };
        apply_pressure(&mut s, p);
        assert_eq!(s.axes[axis::RIGHT_TRIGGER], 30000, "the real trigger still works");
        let mut s = held;
        apply_pressure(&mut s, Pressure { left_trigger: Some(button::START as u8), right_trigger: None, keep_buttons: false });
        assert_eq!(s, held, "a button without pressure does nothing");
    }

    const G: f32 = crate::ds4::STANDARD_GRAVITY;

    /// Feeds `TiltRate` the gravity a controller turning at `w` (rad/s, SDL's frame) would read,
    /// from lying flat, for `secs`; returns the last estimate.
    fn turn(w: [f32; 3], secs: f32) -> [f32; 3] {
        let dt = 0.004;
        let mut s = [0., G, 0.];
        let mut t = TiltRate::default();
        let mut rate = [0.; 3];
        for _ in 0..(secs / dt) as usize {
            rate = t.update(s, dt);
            // dS/dt = -w x S, in small steps.
            for _ in 0..10 {
                let d = cross(w, s);
                s = std::array::from_fn(|i| s[i] - d[i] * dt / 10.);
            }
        }
        rate
    }

    #[test]
    fn tilting_shows_as_pitch_and_roll() {
        let r = turn([1., 0., 0.], 0.8);
        assert!((r[0] - 1.).abs() < 0.03 && r[1].abs() < 0.03 && r[2].abs() < 0.03, "tilting the top up is a positive pitch: {r:?}");
        let r = turn([0., 0., -0.5], 0.8);
        assert!((r[2] + 0.5).abs() < 0.03 && r[0].abs() < 0.03, "{r:?}");
        let r = turn([0., 2., 0.], 0.8);
        assert!(r.iter().all(|v| v.abs() < 0.01), "turning around gravity shows nothing: {r:?}");
    }

    #[test]
    fn repeated_readings_do_not_slow_a_turn() {
        // Read every 4 ms from a controller that reports every 12 ms.
        let (dt, w) = (0.004, [0.8f32, 0., 0.]);
        let mut s = [0., G, 0.];
        let mut t = TiltRate::default();
        let mut rate = [0.; 3];
        for i in 0..300 {
            if i % 3 == 0 {
                for _ in 0..30 {
                    let d = cross(w, s);
                    s = std::array::from_fn(|k| s[k] - d[k] * 3. * dt / 30.);
                }
            }
            rate = t.update(s, dt);
        }
        assert!((rate[0] - 0.8).abs() < 0.03, "{rate:?}");
    }

    #[test]
    fn a_still_controller_and_a_shove_read_no_turn() {
        let mut t = TiltRate::default();
        for _ in 0..100 {
            t.update([0.3, G, -0.2], 0.004);
        }
        assert!(t.rate.iter().all(|v| v.abs() < 1e-3), "{:?}", t.rate);
        assert_eq!(t.update([0., 2. * G, 0.], 0.004), [0.; 3], "twice gravity is a shove");
        assert_eq!(TiltRate::default().update([0., 3. * G, 0.], 0.004), [0.; 3], "and cannot start it");
        assert_eq!(t.update([f32::NAN, G, 0.], 0.004), [0.; 3]);
    }
}
