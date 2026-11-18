//! Optional controller tools. All defaults preserve direct gamepad input.
use crate::mapping::{PadState, axis, button};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchMode {
    #[default]
    Off,
    Mouse,
    Trackball,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TouchMouse {
    pub mode: TouchMode,
    pub sensitivity: u16,
    pub friction_ms: u16,
    pub tap: bool,
    pub scroll: bool,
}
impl Default for TouchMouse {
    fn default() -> Self {
        Self { mode: TouchMode::Off, sensitivity: 100, friction_ms: 180, tap: true, scroll: true }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flick {
    pub enabled: bool,
    pub counts_per_turn: u16,
    pub duration_ms: u16,
    pub threshold: u8,
    pub gyro_mouse: bool,
    pub gyro_sensitivity: u16,
}
impl Default for Flick {
    fn default() -> Self {
        Self { enabled: false, counts_per_turn: 3600, duration_ms: 80, threshold: 85, gyro_mouse: true, gyro_sensitivity: 10 }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    #[default]
    Normal,
    Hair,
    TwoStage,
    HipFire,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TriggerInput {
    pub mode: TriggerMode,
    pub soft: u8,
    pub full: u8,
    pub hysteresis: u8,
    pub delay_ms: u16,
    pub soft_button: u8,
    pub full_button: u8,
    pub exclusive: bool,
}
impl Default for TriggerInput {
    fn default() -> Self {
        Self {
            mode: TriggerMode::Normal,
            soft: 25,
            full: 90,
            hysteresis: 3,
            delay_ms: 100,
            soft_button: button::LEFT_SHOULDER as u8,
            full_button: button::SOUTH as u8,
            exclusive: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectMode {
    #[default]
    Off,
    Resistance,
    Vibration,
    Click,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TriggerEffect {
    pub mode: EffectMode,
    pub start: u8,
    pub end: u8,
    pub strength: u8,
    pub frequency: u8,
}
impl Default for TriggerEffect {
    fn default() -> Self {
        Self { mode: EffectMode::Off, start: 3, end: 7, strength: 3, frequency: 20 }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightMode {
    #[default]
    Off,
    Rainbow,
    Breathe,
    Rumble,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DynamicLight {
    pub mode: LightMode,
    pub period_ms: u16,
    pub idle_seconds: u16,
}
impl Default for DynamicLight {
    fn default() -> Self {
        Self { mode: LightMode::Off, period_ms: 4000, idle_seconds: 0 }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub touch: TouchMouse,
    pub flick: Flick,
    pub triggers: [TriggerInput; 2],
    pub debounce_ms: u16,
    pub dsu: bool,
    pub dynamic: DynamicLight,
    pub adaptive_enabled: bool,
    pub adaptive: [TriggerEffect; 2],
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    pub testing: bool,
    pub trigger_error: Option<String>,
    pub dsu: crate::dsu::Status,
    pub output: PadState,
    pub led: Option<[u8; 3]>,
}
impl Config {
    pub fn clean(mut self) -> Self {
        self.touch.sensitivity = self.touch.sensitivity.clamp(10, 400);
        self.touch.friction_ms = self.touch.friction_ms.clamp(40, 1000);
        self.flick.counts_per_turn = self.flick.counts_per_turn.clamp(100, 60000);
        self.flick.duration_ms = self.flick.duration_ms.clamp(20, 300);
        self.flick.threshold = self.flick.threshold.clamp(50, 99);
        self.flick.gyro_sensitivity = self.flick.gyro_sensitivity.clamp(1, 100);
        for t in &mut self.triggers {
            t.soft = t.soft.clamp(1, 90);
            t.full = t.full.clamp(t.soft + 1, 100);
            t.hysteresis = t.hysteresis.clamp(1, 20);
            t.delay_ms = t.delay_ms.clamp(10, 1000);
            t.soft_button = t.soft_button.min(63);
            t.full_button = t.full_button.min(63);
        }
        self.debounce_ms = self.debounce_ms.min(100);
        self.dynamic.period_ms = self.dynamic.period_ms.clamp(500, 30000);
        self.dynamic.idle_seconds = self.dynamic.idle_seconds.min(600);
        self.adaptive = self.adaptive.map(TriggerEffect::clean);
        self
    }
}
impl TriggerEffect {
    pub fn clean(mut self) -> Self {
        self.start = self.start.min(9);
        self.end = self.end.clamp(1, 9);
        self.strength = self.strength.min(8);
        self.frequency = self.frequency.clamp(1, 100);
        if self.mode == EffectMode::Click {
            self.start = self.start.clamp(2, 7);
            self.end = self.end.clamp(self.start + 1, 8);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Mouse {
    pub dx: f32,
    pub dy: f32,
    pub wheel: f32,
    pub buttons: u8,
    pub tap: bool,
}

struct Debounce {
    stable: u64,
    pending: u64,
    since: [Option<Instant>; 64],
}
impl Default for Debounce {
    fn default() -> Self {
        Self { stable: 0, pending: 0, since: [None; 64] }
    }
}
impl Debounce {
    fn update(&mut self, raw: u64, ms: u16, now: Instant) -> u64 {
        if ms == 0 {
            self.stable = raw;
            self.pending = raw;
            self.since.fill(None);
            return raw;
        }
        for bit in 0..64 {
            let mask = 1u64 << bit;
            if (raw ^ self.stable) & mask == 0 {
                self.since[bit] = None;
                continue;
            }
            if (raw ^ self.pending) & mask != 0 || self.since[bit].is_none() {
                self.pending = (self.pending & !mask) | (raw & mask);
                self.since[bit] = Some(now);
            }
            if self.since[bit].is_some_and(|t| now.duration_since(t) >= Duration::from_millis(u64::from(ms))) {
                self.stable = (self.stable & !mask) | (raw & mask);
                self.since[bit] = None;
            }
        }
        self.stable
    }
}

#[derive(Default)]
struct TriggerState {
    hair: bool,
    extreme: f32,
    soft_since: Option<Instant>,
}
impl TriggerState {
    fn apply(&mut self, state: &mut PadState, side: usize, cfg: TriggerInput, now: Instant) {
        let a = if side == 0 { axis::LEFT_TRIGGER } else { axis::RIGHT_TRIGGER };
        let v = f32::from(state.axes[a].max(0)) / 32767. * 100.;
        match cfg.mode {
            TriggerMode::Normal => *self = Self::default(),
            TriggerMode::Hair => {
                if v < 1. {
                    self.hair = false;
                    self.extreme = 0.;
                } else if self.hair {
                    self.extreme = self.extreme.max(v);
                    if v <= self.extreme - f32::from(cfg.hysteresis) {
                        self.hair = false;
                        self.extreme = v;
                    }
                } else {
                    self.extreme = self.extreme.min(v);
                    if v >= f32::from(cfg.soft) && v >= self.extreme + f32::from(cfg.hysteresis) {
                        self.hair = true;
                        self.extreme = v;
                    }
                }
                state.axes[a] = if self.hair { 32767 } else { 0 };
            }
            TriggerMode::TwoStage | TriggerMode::HipFire => {
                let soft = v >= f32::from(cfg.soft);
                let full = v >= f32::from(cfg.full);
                if soft {
                    self.soft_since.get_or_insert(now);
                } else {
                    self.soft_since = None;
                }
                let ready = cfg.mode != TriggerMode::HipFire
                    || self.soft_since.is_some_and(|t| now.duration_since(t) >= Duration::from_millis(u64::from(cfg.delay_ms)));
                if soft && ready && !(cfg.exclusive && full) {
                    state.buttons |= 1u64 << cfg.soft_button;
                }
                if full {
                    state.buttons |= 1u64 << cfg.full_button;
                }
                state.axes[a] = 0;
            }
        }
    }
}

#[derive(Default)]
struct TouchState {
    finger: Option<usize>,
    last: Option<(f32, f32)>,
    velocity: (f32, f32),
    moved: Option<Instant>,
    start: Option<Instant>,
    travel: f32,
    clicked: bool,
    click_button: u8,
    last_scroll: Option<f32>,
}
impl TouchState {
    fn update(&mut self, pad: &PadState, cfg: TouchMouse, now: Instant, dt: f32) -> Mouse {
        if cfg.mode == TouchMode::Off {
            *self = Self::default();
            return Mouse::default();
        }
        let mut out = Mouse::default();
        let selected = self.finger.filter(|&i| pad.touches[i].down).or_else(|| pad.touches.iter().position(|t| t.down));
        let switched = self.finger.is_some() && selected.is_some() && self.finger != selected;
        if switched {
            self.last = None;
            self.velocity = (0., 0.);
        }
        self.finger = selected;
        let finger = selected.map(|i| pad.touches[i]).unwrap_or_default();
        let two = pad.touches.iter().all(|t| t.down);
        let current = finger.down.then_some((f32::from(finger.x) / 65535., f32::from(finger.y) / 65535.));
        let clicked = pad.pressed(button::TOUCHPAD);
        if clicked && self.click_button == 0 {
            self.click_button = if two { 2 } else { 1 };
            self.clicked = true;
        }
        if !clicked {
            self.click_button = 0;
        }
        out.buttons = self.click_button;
        if two && cfg.scroll {
            let y = pad.touches.iter().map(|t| f32::from(t.y) / 65535.).sum::<f32>() / 2.;
            if let Some(before) = self.last_scroll {
                out.wheel = (before - y) * 1200. * f32::from(cfg.sensitivity) / 100.;
            }
            self.last_scroll = Some(y);
            self.last = current;
            self.velocity = (0., 0.);
            self.clicked = true;
            return out;
        }
        self.last_scroll = None;
        match (self.last, current) {
            (None, Some(_)) => {
                self.start = Some(now);
                self.travel = 0.;
                self.clicked = clicked || switched;
                self.velocity = (0., 0.);
                self.moved = Some(now);
            }
            (Some(before), Some(at)) => {
                let delta = (at.0 - before.0, at.1 - before.1);
                self.travel += delta.0.hypot(delta.1);
                let scale = 1900. * f32::from(cfg.sensitivity) / 100.;
                (out.dx, out.dy) = (delta.0 * scale, delta.1 * scale);
                if delta.0.hypot(delta.1) > 0.0001 && dt > 0. {
                    let sample_dt = self.moved.map_or(dt, |at| now.duration_since(at).as_secs_f32().clamp(0.001, 0.05));
                    self.velocity = ((out.dx / sample_dt).clamp(-12000., 12000.), (out.dy / sample_dt).clamp(-12000., 12000.));
                    self.moved = Some(now);
                }
            }
            (Some(_), None) => {
                out.tap = cfg.tap
                    && !self.clicked
                    && self.travel < 0.015
                    && self.start.is_some_and(|t| now.duration_since(t) <= Duration::from_millis(180));
                if !self.moved.is_some_and(|t| now.duration_since(t) < Duration::from_millis(40)) {
                    self.velocity = (0., 0.);
                }
                self.start = None;
            }
            _ => {}
        }
        if current.is_none() && cfg.mode == TouchMode::Trackball {
            let decay = (-dt * 1000. / f32::from(cfg.friction_ms)).exp();
            let integral = f32::from(cfg.friction_ms) / 1000. * (1. - decay);
            out.dx += self.velocity.0 * integral;
            out.dy += self.velocity.1 * integral;
            self.velocity.0 *= decay;
            self.velocity.1 *= decay;
            if self.velocity.0.hypot(self.velocity.1) < 1. {
                self.velocity = (0., 0.);
            }
        }
        self.last = current;
        out
    }
}

#[derive(Default)]
struct FlickState {
    angle: Option<f32>,
    remaining: f32,
    total: f32,
    progress: f32,
}
fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}
impl FlickState {
    fn update(&mut self, pad: &PadState, cfg: Flick, rate: Option<[f32; 3]>, dt: f32) -> (f32, f32) {
        if !cfg.enabled {
            *self = Self::default();
            return (0., 0.);
        }
        let x = f32::from(pad.axes[axis::RIGHT_X]) / 32768.;
        let y = f32::from(pad.axes[axis::RIGHT_Y]) / 32768.;
        let mut dx = 0.;
        let mut dy = 0.;
        if x.hypot(y) >= f32::from(cfg.threshold) / 100. {
            let angle = x.atan2(-y);
            if let Some(before) = self.angle {
                dx += wrap(angle - before) / TAU * f32::from(cfg.counts_per_turn);
            } else {
                self.total = self.remaining + angle / TAU * f32::from(cfg.counts_per_turn);
                self.remaining = self.total;
                self.progress = 0.;
            }
            self.angle = Some(angle);
        } else if x.hypot(y) < f32::from(cfg.threshold.saturating_sub(10)) / 100. {
            self.angle = None;
        }
        if self.remaining.abs() > 0.001 {
            let before = self.progress;
            self.progress = (self.progress + dt * 1000. / f32::from(cfg.duration_ms)).min(1.);
            let ease = |t: f32| t * t * (3. - 2. * t);
            let step = self.total * (ease(self.progress) - ease(before));
            dx += step;
            self.remaining -= step;
            if self.progress >= 1. {
                self.remaining = 0.;
            }
        }
        if cfg.gyro_mouse
            && let Some([pitch, yaw, _]) = rate
            && pitch.is_finite()
            && yaw.is_finite()
        {
            let quiet = |rate: f32| {
                let degrees = rate.to_degrees();
                if degrees.abs() < 0.05 { 0. } else { degrees }
            };
            dx -= quiet(yaw) * dt * f32::from(cfg.gyro_sensitivity);
            dy -= quiet(pitch) * dt * f32::from(cfg.gyro_sensitivity);
        }
        (dx, dy)
    }
}

#[derive(Default)]
pub struct Runtime {
    pub output: PadState,
    pub active: bool,
    config: Config,
    last: Option<Instant>,
    debounce: Debounce,
    triggers: [TriggerState; 2],
    touch: TouchState,
    flick: FlickState,
}
impl Runtime {
    pub fn update(&mut self, raw: PadState, cfg: Config, mapped: bool, rate: Option<[f32; 3]>, now: Instant) -> Mouse {
        let cfg = cfg.clean();
        if self.config.touch != cfg.touch {
            self.touch = TouchState::default();
        }
        if self.config.flick != cfg.flick {
            self.flick = FlickState::default();
        }
        if self.config.triggers != cfg.triggers {
            self.triggers = Default::default();
        }
        self.config = cfg;
        let dt = self.last.map_or(0.004, |last| now.duration_since(last).as_secs_f32().min(0.05));
        self.last = Some(now);
        let mut output = raw;
        output.buttons = self.debounce.update(raw.buttons, if mapped { cfg.debounce_ms } else { 0 }, now);
        let mut mouse = self.touch.update(&raw, cfg.touch, now, dt);
        if mapped {
            for (side, trigger) in self.triggers.iter_mut().enumerate() {
                trigger.apply(&mut output, side, cfg.triggers[side], now);
            }
            let flick = if cfg!(windows) { cfg.flick } else { Flick::default() };
            let (dx, dy) = self.flick.update(&raw, flick, rate, dt);
            mouse.dx += dx;
            mouse.dy += dy;
            if flick.enabled {
                output.axes[axis::RIGHT_X] = 0;
                output.axes[axis::RIGHT_Y] = 0;
            }
        } else {
            self.flick = FlickState::default();
            self.triggers = Default::default();
        }
        self.output = output;
        mouse
    }
}

pub fn dynamic_color(cfg: DynamicLight, base: [u8; 3], brightness: u8, elapsed: Duration, idle: Duration, rumble: u8) -> [u8; 3] {
    let phase = elapsed.as_secs_f32() * 1000. / f32::from(cfg.period_ms.clamp(500, 30000));
    let mut out = match cfg.mode {
        LightMode::Off => base,
        LightMode::Rainbow => {
            let h = phase.fract() * 6.;
            let x = (1. - (h % 2. - 1.).abs()) * 255.;
            let rgb = match h as u8 {
                0 => [255., x, 0.],
                1 => [x, 255., 0.],
                2 => [0., 255., x],
                3 => [0., x, 255.],
                4 => [x, 0., 255.],
                _ => [255., 0., x],
            };
            rgb.map(|v| (v * f32::from(brightness.min(100)) / 100.) as u8)
        }
        LightMode::Breathe => base.map(|v| (f32::from(v) * (0.15 + 0.85 * (0.5 - (phase * TAU).cos() * 0.5))) as u8),
        LightMode::Rumble => {
            let v = f32::from(rumble) / 255.;
            [f32::from(base[0]) * (1. - v) + 255. * v, f32::from(base[1]) * (1. - v), f32::from(base[2]) * (1. - v)].map(|c| c as u8)
        }
    };
    if cfg.idle_seconds > 0 {
        let fade = (1. - (idle.as_secs_f32() - f32::from(cfg.idle_seconds)).max(0.) / 5.).clamp(0., 1.);
        out = out.map(|c| (f32::from(c) * fade) as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::Touch;

    #[test]
    fn touch_mouse_taps_scrolls_and_hands_off_fingers_without_a_jump() {
        let now = Instant::now();
        let cfg = TouchMouse { mode: TouchMode::Mouse, ..Default::default() };
        let mut touch = TouchState::default();
        let mut p = PadState::default();
        p.touches[0] = Touch::from_position(true, 0.2, 0.3);
        touch.update(&p, cfg, now, 0.004);
        p.touches[0] = Touch::default();
        assert!(touch.update(&p, cfg, now + Duration::from_millis(70), 0.004).tap);
        p.touches = [Touch::from_position(true, 0.2, 0.5), Touch::from_position(true, 0.8, 0.5)];
        touch.update(&p, cfg, now + Duration::from_millis(80), 0.004);
        p.touches[0].y = 20000;
        p.touches[1].y = 20000;
        let scroll = touch.update(&p, cfg, now + Duration::from_millis(90), 0.01);
        assert!(scroll.wheel > 0.);
        assert_eq!((scroll.dx, scroll.dy), (0., 0.));
        p.touches[0] = Touch::default();
        let handoff = touch.update(&p, cfg, now + Duration::from_millis(100), 0.01);
        assert_eq!((handoff.dx, handoff.dy), (0., 0.));
        p.touches[1].x = 60000;
        assert!(touch.update(&p, cfg, now + Duration::from_millis(110), 0.01).dx > 0.);
        p.touches[1] = Touch::default();
        assert!(!touch.update(&p, cfg, now + Duration::from_millis(120), 0.01).tap);
    }

    #[test]
    fn trackball_coasts_then_stops_and_retouched_fingers_cancel_inertia() {
        let now = Instant::now();
        let cfg = TouchMouse { mode: TouchMode::Trackball, ..Default::default() };
        let mut touch = TouchState::default();
        let mut p = PadState::default();
        p.touches[0] = Touch::from_position(true, 0.2, 0.5);
        touch.update(&p, cfg, now, 0.004);
        p.touches[0].x = 20000;
        touch.update(&p, cfg, now + Duration::from_millis(20), 0.02);
        p.touches[0] = Touch::default();
        let first = touch.update(&p, cfg, now + Duration::from_millis(24), 0.004);
        let later = touch.update(&p, cfg, now + Duration::from_millis(40), 0.004);
        assert!(first.dx > 0. && later.dx > 0. && later.dx < first.dx);
        p.touches[0] = Touch::from_position(true, 0.8, 0.5);
        let again = touch.update(&p, cfg, now + Duration::from_millis(44), 0.004);
        assert_eq!((again.dx, again.dy), (0., 0.));
        p.touches[0] = Touch::default();
        assert_eq!(touch.update(&p, cfg, now + Duration::from_millis(100), 0.004).dx, 0.);
    }

    #[test]
    fn two_stage_and_hip_fire_actions_do_not_leave_buttons_held() {
        let now = Instant::now();
        let cfg = TriggerInput { mode: TriggerMode::TwoStage, ..Default::default() };
        let mut t = TriggerState::default();
        let sample = |value| {
            let mut p = PadState::default();
            p.axes[axis::LEFT_TRIGGER] = value;
            p
        };
        let mut soft = sample(17000);
        t.apply(&mut soft, 0, cfg, now);
        assert!(soft.pressed(u32::from(cfg.soft_button)));
        assert!(!soft.pressed(u32::from(cfg.full_button)));
        assert_eq!(soft.axes[axis::LEFT_TRIGGER], 0);
        let mut full = sample(32767);
        t.apply(&mut full, 0, cfg, now);
        assert!(!full.pressed(u32::from(cfg.soft_button)));
        assert!(full.pressed(u32::from(cfg.full_button)));
        let mut released = sample(0);
        t.apply(&mut released, 0, cfg, now);
        assert_eq!(released.buttons, 0);
        let mut hip = TriggerState::default();
        let cfg = TriggerInput { mode: TriggerMode::HipFire, ..cfg };
        let mut pending = sample(17000);
        hip.apply(&mut pending, 0, cfg, now);
        assert_eq!(pending.buttons, 0);
        let mut held = sample(17000);
        hip.apply(&mut held, 0, cfg, now + Duration::from_millis(110));
        assert!(held.pressed(u32::from(cfg.soft_button)));
        let mut quick = TriggerState::default();
        let mut full = sample(32767);
        quick.apply(&mut full, 0, cfg, now);
        assert!(full.pressed(u32::from(cfg.full_button)));
        assert!(!full.pressed(u32::from(cfg.soft_button)));
    }

    #[test]
    fn native_mode_keeps_input_even_with_mapping_tools_configured() {
        let mut r = Runtime::default();
        let mut p = PadState { buttons: 3, ..Default::default() };
        p.axes[axis::LEFT_TRIGGER] = 18000;
        p.axes[axis::RIGHT_X] = 32767;
        let cfg = Config {
            flick: Flick { enabled: true, ..Default::default() },
            debounce_ms: 100,
            triggers: [TriggerInput { mode: TriggerMode::TwoStage, ..Default::default() }; 2],
            ..Default::default()
        };
        let mouse = r.update(p, cfg, false, Some([1.; 3]), Instant::now());
        assert_eq!(r.output, p);
        assert_eq!((mouse.dx, mouse.dy), (0., 0.));
    }

    #[test]
    fn rapid_flicks_keep_the_unfinished_turn() {
        let mut f = FlickState::default();
        let cfg = Flick { enabled: true, gyro_mouse: false, ..Default::default() };
        let mut p = PadState::default();
        p.axes[axis::RIGHT_X] = 32767;
        let mut sum = f.update(&p, cfg, None, 0.004).0;
        let rest = PadState::default();
        sum += f.update(&rest, cfg, None, 0.004).0;
        for _ in 0..20 {
            sum += f.update(&p, cfg, None, 0.004).0;
        }
        assert!((sum - 1800.).abs() < 0.1);
    }

    #[test]
    fn gyro_mouse_ignores_sensor_noise_and_invalid_samples() {
        let mut f = FlickState::default();
        let cfg = Flick { enabled: true, ..Default::default() };
        let p = PadState::default();
        assert_eq!(f.update(&p, cfg, Some([0.0001, -0.0001, 0.]), 0.01), (0., 0.));
        assert_eq!(f.update(&p, cfg, Some([f32::NAN, 1., 0.]), 0.01), (0., 0.));
        let motion = f.update(&p, cfg, Some([1., 0., 0.]), 0.01);
        assert!(motion.1 < -5.);
    }

    #[test]
    fn lab_settings_survive_profiles_and_old_profiles_keep_every_tool_off() {
        let mut profiles = crate::profile::Profiles::default();
        let cfg = Config { dsu: true, debounce_ms: 15, ..Default::default() };
        profiles.apply(crate::profile::Edit::Lab(cfg));
        assert!(!profiles.is_default());
        let loaded: crate::profile::Profiles = serde_json::from_str(&serde_json::to_string(&profiles).unwrap()).unwrap();
        assert_eq!(loaded.active().lab, cfg);
        let old: crate::profile::Profile = serde_json::from_str(r#"{"name":"old"}"#).unwrap();
        assert_eq!(old.lab, Config::default());
    }
    #[test]
    fn debounce_settles_without_another_hid_report() {
        let mut d = Debounce::default();
        let n = Instant::now();
        assert_eq!(d.update(1, 20, n), 0);
        assert_eq!(d.update(0, 20, n + Duration::from_millis(5)), 0);
        assert_eq!(d.update(1, 20, n + Duration::from_millis(10)), 0);
        assert_eq!(d.update(1, 20, n + Duration::from_millis(31)), 1);
        assert_eq!(d.update(0, 20, n + Duration::from_millis(32)), 1);
        assert_eq!(d.update(0, 20, n + Duration::from_millis(53)), 0);
    }
    #[test]
    fn hair_releases_and_rearms_without_returning_to_zero() {
        let mut t = TriggerState::default();
        let n = Instant::now();
        let cfg = TriggerInput { mode: TriggerMode::Hair, ..Default::default() };
        let mut state = PadState::default();
        for (v, expected) in [(16000, 32767), (15000, 0), (17000, 32767), (0, 0)] {
            state.axes[axis::LEFT_TRIGGER] = v;
            t.apply(&mut state, 0, cfg, n);
            assert_eq!(state.axes[axis::LEFT_TRIGGER], expected);
        }
    }
    #[test]
    fn flick_wraps_at_back_and_finishes_a_quarter_turn() {
        assert!((wrap(-PI + 0.01 - (PI - 0.01)) - 0.02).abs() < 0.001);
        let mut f = FlickState::default();
        let mut p = PadState::default();
        p.axes[axis::RIGHT_X] = 32767;
        let cfg = Flick { enabled: true, gyro_mouse: false, ..Default::default() };
        let sum: f32 = (0..20).map(|_| f.update(&p, cfg, None, 0.004).0).sum();
        assert!((sum - 900.).abs() < 0.1);
    }
    #[test]
    fn defaults_leave_input_and_mouse_untouched() {
        let mut r = Runtime::default();
        let mut p = PadState { buttons: 7, ..Default::default() };
        p.axes[2] = 12345;
        let m = r.update(p, Config::default(), true, None, Instant::now());
        assert_eq!(r.output, p);
        assert_eq!((m.dx, m.dy, m.buttons, m.tap), (0., 0., 0, false));
    }
    #[test]
    fn config_bounds_and_idle_fade() {
        let cfg = Config { debounce_ms: u16::MAX, ..Default::default() }.clean();
        assert_eq!(cfg.debounce_ms, 100);
        let light = DynamicLight { idle_seconds: 10, ..Default::default() };
        assert_eq!(dynamic_color(light, [20, 40, 60], 100, Duration::ZERO, Duration::from_secs(16), 0), [0; 3]);
    }
}
