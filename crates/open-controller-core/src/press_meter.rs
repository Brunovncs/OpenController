//! Observed input hold times, collected only during an explicit controller recording session.
//! These are report-level observations, not hardware latency or a device minimum specification.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub input: u64,
    pub held_us: u64,
    pub minimum_us: u64,
    pub samples: u32,
}

pub struct Meter {
    ready: bool,
    held: u64,
    since: Instant,
    peak: u64,
    longest: Duration,
    stats: BTreeMap<u64, (u64, u32)>,
    pub latest: Option<Reading>,
}

impl Meter {
    pub fn new(held: u64, now: Instant) -> Self {
        Self { ready: held == 0, held, since: now, peak: 0, longest: Duration::ZERO, stats: BTreeMap::new(), latest: None }
    }

    pub fn feed(&mut self, held: u64, now: Instant) {
        if !self.ready {
            self.ready = held == 0;
            self.held = held;
            self.since = now;
            return;
        }
        if held == self.held {
            return;
        }
        if self.held != 0 {
            let span = now.saturating_duration_since(self.since);
            if self.held.count_ones() > self.peak.count_ones() || self.held.count_ones() == self.peak.count_ones() && span > self.longest {
                self.peak = self.held;
                self.longest = span;
            }
        }
        self.held = held;
        self.since = now;
        if held == 0 && self.peak != 0 {
            let us = self.longest.as_micros().min(u128::from(u64::MAX)) as u64;
            if self.stats.len() >= 128 && !self.stats.contains_key(&self.peak) {
                self.stats.clear();
            }
            let stats = self.stats.entry(self.peak).or_insert((us, 0));
            stats.0 = stats.0.min(us);
            stats.1 = stats.1.saturating_add(1);
            self.latest = Some(Reading { input: self.peak, held_us: us, minimum_us: stats.0, samples: stats.1 });
            self.peak = 0;
            self.longest = Duration::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_button_observed_minimum_is_per_input_not_a_hardware_claim() {
        let start = Instant::now();
        let mut meter = Meter::new(0, start);
        meter.feed(1, start);
        meter.feed(0, start + Duration::from_millis(180));
        meter.feed(1, start + Duration::from_millis(200));
        meter.feed(0, start + Duration::from_millis(250));
        assert_eq!(meter.latest, Some(Reading { input: 1, held_us: 50_000, minimum_us: 50_000, samples: 2 }));
        meter.feed(2, start + Duration::from_millis(300));
        meter.feed(0, start + Duration::from_millis(390));
        assert_eq!(meter.latest.as_ref().unwrap().minimum_us, 90_000);
    }
    #[test]
    fn combination_reports_the_simultaneous_hold_not_the_total_gesture() {
        let start = Instant::now();
        let mut meter = Meter::new(0, start);
        meter.feed(1, start);
        meter.feed(3, start + Duration::from_millis(120));
        meter.feed(1, start + Duration::from_millis(150));
        meter.feed(0, start + Duration::from_millis(400));
        assert_eq!(meter.latest, Some(Reading { input: 3, held_us: 30_000, minimum_us: 30_000, samples: 1 }));
    }
    #[test]
    fn preheld_buttons_and_incomplete_gestures_do_not_create_measurements() {
        let start = Instant::now();
        let mut meter = Meter::new(1, start);
        meter.feed(0, start + Duration::from_secs(1));
        assert!(meter.latest.is_none());
        meter.feed(1, start + Duration::from_secs(2));
        assert!(meter.latest.is_none());
        meter.feed(0, start + Duration::from_secs(3));
        assert_eq!(meter.latest.unwrap().held_us, 1_000_000);
    }
}
