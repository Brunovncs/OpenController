//! Optional controller tools. All defaults preserve direct gamepad input.
use crate::mapping::{PadState, button};
use serde::{Deserialize, Serialize};
use std::time::Instant;

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

#[derive(Default)]
pub struct Runtime {
    pub output: PadState,
    pub active: bool,
}
impl Runtime {
    pub fn update(&mut self, raw: PadState, _cfg: Config, _mapped: bool, _rate: Option<[f32; 3]>, _now: Instant) -> Mouse {
        self.output = raw;
        Mouse::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn defaults_leave_input_and_mouse_untouched() {
        let mut r = Runtime::default();
        let mut p = PadState { buttons: 7, ..Default::default() };
        p.axes[2] = 12345;
        let m = r.update(p, Config::default(), true, None, Instant::now());
        assert_eq!(r.output, p);
        assert_eq!((m.dx, m.dy, m.buttons, m.tap), (0., 0., 0, false));
    }
}
