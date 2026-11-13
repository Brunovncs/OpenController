//! Presentation-only touch smoothing. Raw controller input is never changed.

use open_controller_core::PadKey;
use open_controller_core::mapping::Touch;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

type Position = (f32, f32);
const TRAIL_LIFE: Duration = Duration::from_millis(240);
const RESPONSE: f32 = 0.014;
const MIN_STEP: f32 = 0.0007;
const MAX_SAMPLES: usize = 48;

#[derive(Default)]
pub struct Animation {
    key: Option<PadKey>,
    last: Option<Instant>,
    fingers: [Finger; 2],
}

#[derive(Default)]
struct Finger {
    down: bool,
    position: Position,
    sampled: Position,
    trail: VecDeque<(Instant, Position)>,
}

#[derive(Default)]
pub struct Frame {
    pub fingers: [FingerFrame; 2],
    pub animating: bool,
}

#[derive(Default)]
pub struct FingerFrame {
    pub head: Option<Position>,
    /// Oldest to newest; each point carries its remaining lifetime, from zero to one.
    pub trail: Vec<(Position, f32)>,
}

fn position(touch: Touch) -> Position {
    (f32::from(touch.x) / 65535., f32::from(touch.y) / 65535.)
}

fn distance(a: Position, b: Position) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

impl Frame {
    pub fn immediate(touches: [Touch; 2]) -> Self {
        Self {
            fingers: touches.map(|touch| FingerFrame { head: touch.down.then(|| position(touch)), trail: Vec::new() }),
            animating: false,
        }
    }
}

impl Animation {
    pub fn frame(&mut self, key: PadKey, touches: [Touch; 2], now: Instant) -> Frame {
        // A different page/controller or a long-hidden preview starts with fresh contacts.
        if self.key != Some(key) || self.last.is_some_and(|last| now.duration_since(last) > TRAIL_LIFE) {
            *self = Self { key: Some(key), ..Self::default() };
        }
        let dt = self.last.map_or(0., |last| now.duration_since(last).as_secs_f32());
        self.last = Some(now);
        let blend = 1. - (-dt / RESPONSE).exp();
        let mut frame = Frame::default();
        for ((finger, touch), painted) in self.fingers.iter_mut().zip(touches).zip(frame.fingers.iter_mut()) {
            while finger.trail.front().is_some_and(|(time, _)| now.duration_since(*time) >= TRAIL_LIFE) {
                finger.trail.pop_front();
            }
            if touch.down {
                let target = position(touch);
                if !finger.down {
                    // Never connect the end of one gesture to the start of the next.
                    finger.trail.clear();
                    finger.position = target;
                    finger.sampled = target;
                    finger.trail.push_back((now, target));
                } else {
                    finger.position.0 += (target.0 - finger.position.0) * blend;
                    finger.position.1 += (target.1 - finger.position.1) * blend;
                    if distance(finger.position, target) < MIN_STEP {
                        finger.position = target;
                    } else {
                        frame.animating = true;
                    }
                    if distance(finger.position, finger.sampled) >= MIN_STEP {
                        finger.trail.push_back((now, finger.position));
                        finger.sampled = finger.position;
                        while finger.trail.len() > MAX_SAMPLES {
                            finger.trail.pop_front();
                        }
                    }
                }
                painted.head = Some(finger.position);
            }
            finger.down = touch.down;
            painted.trail = finger
                .trail
                .iter()
                .map(|(time, point)| (*point, 1. - now.duration_since(*time).as_secs_f32() / TRAIL_LIFE.as_secs_f32()))
                .collect();
            frame.animating |= !painted.trail.is_empty();
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts_and_gestures_do_not_join_and_released_trails_stop_animating() {
        let mut animation = Animation::default();
        let key = PadKey::Slot(0);
        let now = Instant::now();
        let touch = |x| Touch { down: true, x, y: 32768 };
        animation.frame(key, [touch(1000), touch(60000)], now);
        let moved = animation.frame(key, [touch(30000), touch(40000)], now + Duration::from_millis(16));
        assert!(moved.fingers[0].head.unwrap().0 < moved.fingers[1].head.unwrap().0);
        let released = animation.frame(key, [Touch::default(); 2], now + Duration::from_millis(32));
        assert!(released.fingers.iter().all(|f| f.head.is_none()));
        assert!(released.animating);
        let new = animation.frame(key, [touch(65000), Touch::default()], now + Duration::from_millis(48));
        assert_eq!(new.fingers[0].trail.len(), 1);
        animation.frame(key, [Touch::default(); 2], now + Duration::from_millis(64));
        let faded = animation.frame(key, [Touch::default(); 2], now + Duration::from_millis(320));
        assert!(!faded.animating);
        assert!(faded.fingers.iter().all(|f| f.trail.is_empty()));
    }

    #[test]
    fn smoothing_uses_elapsed_time_instead_of_frame_count() {
        let finish = |steps: u32| {
            let mut animation = Animation::default();
            let key = PadKey::Slot(0);
            let now = Instant::now();
            let start = Touch { down: true, x: 0, y: 0 };
            let end = Touch { x: 65535, ..start };
            animation.frame(key, [start, Touch::default()], now);
            let mut frame = Frame::default();
            for step in 1..=steps {
                frame =
                    animation.frame(key, [end, Touch::default()], now + Duration::from_secs_f64(0.04 * f64::from(step) / f64::from(steps)));
            }
            frame.fingers[0].head.unwrap().0
        };
        assert!((finish(2) - finish(8)).abs() < 0.0001);
        assert!(finish(8) > 0.9 && finish(8) < 1.);
    }
}
