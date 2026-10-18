//! The DualShock 3's motion sensors, which SDL leaves half read. Behind DsHidMini in its SXS mode
//! SDL reads the controller from feature report 0, as with Sony's own driver, but takes the
//! accelerometer's bytes the wrong way round and leaves out the gyro, which turns only around one
//! axis (yaw). OpenController reads the same report itself for both, while they are wanted.
//!
//! The words are written one of two ways. DsHidMini 3 writes them as Sony's driver did:
//! calibrated to 113 counts per g around 512, little-endian, X mirrored and the gyro's sign
//! flipped. Older DsHidMini copied the controller's own report, big-endian and uncalibrated.
//! Which one is found from the accelerometer, which reads about 1 g at rest only the right way.
//! The signs follow PadForge's SDL fork and DS4Windows, which were checked on real controllers;
//! OpenController's own were not, so the feature is experimental and reports carry the raw words.

use crate::ds4::STANDARD_GRAVITY;
use sdl3_sys::everything::*;
use std::ffi::CString;
use std::time::{Duration, Instant};

/// Where the four motion words start in feature report 0, counting its id byte: accelerometer
/// X, Y and Z, then the gyro.
const MOTION_AT: usize = 42;
/// The report with its id.
const FEATURE_LEN: usize = 50;
/// The driver answers from the last report it got, so reading faster only repeats it.
const READ_EVERY: Duration = Duration::from_millis(2);
/// Readings in a row that must agree on the byte order before it is taken.
const AGREE: u8 = 8;
/// Accelerometer counts per g, and the gyro's counts per 90 °/s (DS4Windows).
const COUNTS_PER_G: f32 = 113.;
const GYRO_SCALE: f32 = 90. / 123.;

/// The four motion words, as they come.
pub type Words = [[u8; 2]; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Little-endian and calibrated, X mirrored and the gyro flipped: DsHidMini 3, Sony's driver.
    Driver,
    /// Big-endian and raw, as the controller sends them: older DsHidMini.
    Controller,
}

/// The motion words of a feature report 0 read with its id byte first.
pub fn words(report: &[u8]) -> Option<Words> {
    let w = report.get(MOTION_AT..MOTION_AT + 8)?;
    Some([[w[0], w[1]], [w[2], w[3]], [w[4], w[5]], [w[6], w[7]]])
}

fn value(b: [u8; 2], o: Order) -> f32 {
    f32::from(match o {
        Order::Driver => u16::from_le_bytes(b),
        Order::Controller => u16::from_be_bytes(b),
    }) - 512.
}

/// The accelerometer in SDL's frame, m/s²: x to the right, y up, z towards the player. The
/// controller's own Z is up and its Y points away.
pub fn accel(w: &Words, o: Order) -> [f32; 3] {
    let x = value(w[0], o);
    let x = if o == Order::Driver { -x } else { x };
    let k = STANDARD_GRAVITY / COUNTS_PER_G;
    [x * k, -value(w[2], o) * k, -value(w[1], o) * k]
}

/// The gyro, turning around the controller's up axis, in radians per second; positive turns left
/// as SDL has it.
pub fn yaw(w: &Words, o: Order) -> f32 {
    let dps = value(w[3], o) * GYRO_SCALE;
    (if o == Order::Driver { -dps } else { dps }).to_radians()
}

fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// The order a reading makes sense in: the one whose accelerometer reads near 1 g. Read the wrong
/// way, a word near 512 turns into one near 2 or 130 000, many g off.
pub fn guess(w: &Words) -> Option<Order> {
    let near = |o| (0.5..1.6).contains(&(length(accel(w, o)) / STANDARD_GRAVITY));
    match (near(Order::Driver), near(Order::Controller)) {
        (true, false) => Some(Order::Driver),
        (false, true) => Some(Order::Controller),
        _ => None,
    }
}

/// Settles the byte order once enough readings in a row agree, so a shake while it is worked out
/// cannot pick the wrong one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Detector {
    candidate: Option<Order>,
    agreeing: u8,
    pub order: Option<Order>,
}

impl Detector {
    pub fn feed(&mut self, w: &Words) -> Option<Order> {
        if self.order.is_none() {
            match guess(w) {
                Some(o) if Some(o) == self.candidate => self.agreeing += 1,
                g => {
                    self.candidate = g;
                    self.agreeing = u8::from(g.is_some());
                }
            }
            if self.agreeing >= AGREE {
                self.order = self.candidate;
            }
        }
        self.order
    }
}

/// The gyro's reading at rest, learnt while the controller lies still and taken off what it
/// reads. DsHidMini already zeroes a genuine controller; this catches the rest, such as a clone or
/// an older DsHidMini, without taking a slow, deliberate turn for drift.
///
/// Turning around gravity leaves the accelerometer as it was, so a turn lying on a table looks
/// still to it: only a yaw this close to the bias is taken for drift.
#[derive(Clone, Copy, Debug, Default)]
pub struct RestBias {
    pub bias: f32,
    still: f32,
    /// The yaw averaged over a fraction of a second, so the gyro's noise does not read as turns.
    mean: Option<f32>,
    /// The accelerometer when the controller last moved, which it must stay near while still.
    anchor: Option<[f32; 3]>,
}

impl RestBias {
    /// Within this of the bias the gyro may be resting, in radians per second: about 1.1 °/s,
    /// a count and a half of the gyro, where a deliberate turn is slower than anyone aims.
    const NEAR: f32 = 0.02;
    /// How long the yaw is averaged over, in seconds.
    const AVERAGE: f32 = 0.25;
    /// How long it must look still before the bias follows it, and how slowly it follows.
    const SETTLE: f32 = 1.0;
    const FOLLOW: f32 = 2.0;

    /// `yaw` as read, `accel` in m/s², `dt` in seconds since the last reading. Returns the yaw
    /// with the bias taken off.
    pub fn update(&mut self, yaw: f32, accel: [f32; 3], dt: f32) -> f32 {
        let blend = |tau: f32| if dt > 0. { 1. - (-dt / tau).exp() } else { 0. };
        let mean = self.mean.map_or(yaw, |m| m + (yaw - m) * blend(Self::AVERAGE));
        self.mean = Some(mean);
        let g = length(accel) / STANDARD_GRAVITY;
        let steady = self.anchor.is_some_and(|a| length([accel[0] - a[0], accel[1] - a[1], accel[2] - a[2]]) < 0.3);
        if !steady {
            self.anchor = Some(accel);
        }
        if steady && (0.85..1.15).contains(&g) && (mean - self.bias).abs() < Self::NEAR {
            self.still += dt;
        } else {
            self.still = 0.;
        }
        if self.still > Self::SETTLE {
            self.bias += (mean - self.bias) * blend(Self::FOLLOW);
        }
        yaw - self.bias
    }
}

/// One reading: the accelerometer in m/s² and the yaw in radians per second, bias taken off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub accel: [f32; 3],
    pub yaw: f32,
}

/// A DualShock 3 opened for its motion, alongside SDL. Lives on the input thread.
pub struct Sixaxis {
    dev: *mut SDL_hid_device,
    detector: Detector,
    bias: RestBias,
    read_at: Option<Instant>,
    last: Option<Reading>,
    /// The words last read and how many reads failed, for reports.
    pub words: Option<Words>,
    pub failures: u32,
}

impl Sixaxis {
    /// Opens the controller at SDL's device path for it.
    pub fn open(path: &str) -> Result<Sixaxis, String> {
        let path = CString::new(path).map_err(|e| e.to_string())?;
        unsafe {
            if SDL_hid_init() != 0 {
                return Err(crate::sdl::error());
            }
            let dev = SDL_hid_open_path(path.as_ptr());
            if dev.is_null() {
                let e = crate::sdl::error();
                SDL_hid_exit();
                return Err(e);
            }
            Ok(Sixaxis {
                dev,
                detector: Detector::default(),
                bias: RestBias::default(),
                read_at: None,
                last: None,
                words: None,
                failures: 0,
            })
        }
    }

    /// Whether a new reading is worth asking for.
    pub fn due(&self, now: Instant) -> bool {
        self.read_at.is_none_or(|t| now.duration_since(t) >= READ_EVERY)
    }

    /// The motion in the controller's last report; `None` while the byte order is not known yet.
    pub fn read(&mut self, now: Instant) -> Option<Reading> {
        if !self.due(now) {
            return self.last;
        }
        let dt = self.read_at.map_or(0., |t| now.duration_since(t).as_secs_f32());
        self.read_at = Some(now);
        let mut buf = [0u8; 64];
        let n = unsafe { SDL_hid_get_feature_report(self.dev, buf.as_mut_ptr(), buf.len()) };
        let Some(w) = (n >= FEATURE_LEN as i32).then(|| words(&buf)).flatten() else {
            self.failures = self.failures.saturating_add(1);
            return self.last;
        };
        self.words = Some(w);
        let order = self.detector.feed(&w)?;
        let accel = accel(&w, order);
        let reading = Reading { accel, yaw: self.bias.update(yaw(&w, order), accel, dt) };
        self.last = Some(reading);
        self.last
    }

    pub fn order(&self) -> Option<Order> {
        self.detector.order
    }

    pub fn bias(&self) -> f32 {
        self.bias.bias
    }
}

impl Drop for Sixaxis {
    fn drop(&mut self) {
        unsafe {
            SDL_hid_close(self.dev);
            SDL_hid_exit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words for a reading in counts from 512, written in `o`.
    fn written(counts: [i32; 4], o: Order) -> Words {
        counts.map(|c| {
            let v = (512 + c) as u16;
            match o {
                Order::Driver => v.to_le_bytes(),
                Order::Controller => v.to_be_bytes(),
            }
        })
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.05)
    }

    #[test]
    fn the_words_sit_where_sdl_reads_them() {
        // SDL takes the report from its second byte and the words from 41 on.
        let mut r = [0u8; 50];
        r[42..50].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(words(&r), Some([[1, 2], [3, 4], [5, 6], [7, 8]]));
        assert_eq!(words(&r[..49]), None);
    }

    #[test]
    fn flat_reads_one_g_up_either_way() {
        // Lying flat, the controller's Z reads -1 g (113 counts under 512).
        for o in [Order::Driver, Order::Controller] {
            let w = written([0, 0, -113, 0], o);
            assert!(close(accel(&w, o), [0., STANDARD_GRAVITY, 0.]), "{o:?}: {:?}", accel(&w, o));
            assert_eq!(guess(&w), Some(o));
        }
    }

    #[test]
    fn the_driver_mirrors_x_and_flips_the_gyro() {
        let w = written([56, 0, -113, -41], Order::Driver);
        let c = written([-56, 0, -113, 41], Order::Controller);
        assert!(close(accel(&w, Order::Driver), accel(&c, Order::Controller)));
        assert!((yaw(&w, Order::Driver) - yaw(&c, Order::Controller)).abs() < 1e-6);
        // 41 counts is 30 °/s.
        assert!((yaw(&c, Order::Controller).to_degrees() - 30.).abs() < 0.01);
    }

    #[test]
    fn the_order_is_settled_by_agreement() {
        let mut d = Detector::default();
        let flat = written([0, 0, -113, 0], Order::Driver);
        let shaken = written([300, 300, 300, 0], Order::Driver);
        for _ in 0..AGREE - 1 {
            assert_eq!(d.feed(&flat), None);
        }
        assert_eq!(d.feed(&shaken), None, "a shake that fits neither starts the count again");
        for _ in 0..AGREE - 1 {
            assert_eq!(d.feed(&flat), None);
        }
        assert_eq!(d.feed(&flat), Some(Order::Driver));
        assert_eq!(d.feed(&written([0, 0, -113, 0], Order::Controller)), Some(Order::Driver), "and then kept");
    }

    #[test]
    fn rest_bias_is_learnt_when_still_only() {
        let flat = [0., STANDARD_GRAVITY, 0.];
        let mut b = RestBias::default();
        // A count of the gyro off, with a count of noise either way.
        let count = (90f32 / 123.).to_radians();
        let drift = count;
        for i in 0..2000 {
            b.update(drift + count * ((i % 3) as f32 - 1.), flat, 0.004);
        }
        assert!((b.update(drift, flat, 0.004)).abs() < 0.005, "eight seconds still: the drift is gone ({})", b.bias);
        let mut b = RestBias::default();
        for _ in 0..1250 {
            // Turned slowly on a table: the accelerometer does not change.
            b.update(0.035, flat, 0.004);
        }
        assert_eq!(b.bias, 0., "five seconds of a slow turn are not drift");
        let mut b = RestBias::default();
        for i in 0..1000 {
            // Held in a hand that moves: the accelerometer never settles.
            let a = [2. * (i as f32 * 1.3).sin(), STANDARD_GRAVITY, 0.];
            b.update(drift, a, 0.004);
        }
        assert_eq!(b.bias, 0.);
        let mut b = RestBias::default();
        for _ in 0..1000 {
            b.update(0.2, flat, 0.004);
        }
        assert_eq!(b.bias, 0., "a turn is not drift");
    }
}
