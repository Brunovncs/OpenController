//! A test bench for controllers OpenController reads from their own HID reports, beside or instead
//! of SDL. Each controller is a file in `pads/`: its report at rest, what each byte and bit means
//! (from SDL, the maker's app, other projects, a user's report), and what users' reports caught
//! moving. Each decoder of OpenController's that reads such a report goes through the same checks,
//! [`check`], on reports made from that file: no real controller needed.
//!
//! See README.md beside this file for how to add a controller.

use crate::extras;
use crate::mapping::{PadState, axis, button};
use serde::Deserialize;
use std::path::PathBuf;

/// A controller's report, as its file in `pads/` describes it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pad {
    pub name: String,
    /// `vvvv:pppp`, every id it comes with (cable, receiver...).
    pub ids: Vec<String>,
    #[serde(default)]
    pub connections: String,
    pub sources: Vec<String>,
    /// The report at rest, in hex, report id first.
    pub rest: String,
    /// How many leading bytes say which report this is: kept as they are when making reports.
    pub signature: usize,
    pub controls: Vec<Control>,
    #[serde(default)]
    pub axes: Vec<AxisByte>,
    /// Bytes that move with no button or stick behind them: motion, counters, battery.
    #[serde(default)]
    pub noise: Vec<Noise>,
    /// Reports that are not input, such as replies to commands.
    #[serde(default)]
    pub not_input: Vec<NotInput>,
    #[serde(default)]
    pub captures: Vec<Capture>,
    /// What is in the file but never seen on a real controller.
    #[serde(default)]
    pub unseen: Vec<String>,
    /// The decoders of OpenController's that read this report ([`decoder`]).
    #[serde(default)]
    pub decoders: Vec<String>,
    /// For a report OpenController reads motion from.
    #[serde(default)]
    pub motion: Option<MotionSpec>,
}

/// Motion read from the report: the decoder ([`motion_decoder`]) and reports whose motion is known.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionSpec {
    pub decoder: String,
    pub samples: Vec<MotionSample>,
}

/// A report and the motion in it: for pitch, yaw and roll, "0" for still, "+" or "-" for a turn
/// that way (SDL's: tilting the top up and turning left are positive), "any" when it does not
/// matter; and the accelerometer's length in g, for a decoder that reads one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionSample {
    pub what: String,
    pub report: String,
    pub gyro: [String; 3],
    #[serde(default)]
    pub accel_g: Option<f32>,
}

/// What a motion decoder reads: the gyro in radians a second (pitch, yaw, roll) and the
/// accelerometer in m/s², when there is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub gyro: [f32; 3],
    pub accel: Option<[f32; 3]>,
}

pub struct MotionDecoder {
    pub name: &'static str,
    pub read: fn(&[u8]) -> Option<Motion>,
}

pub fn motion_decoder(name: &str) -> Option<MotionDecoder> {
    match name {
        "flydigi-apex2-air" => Some(MotionDecoder {
            name: "flydigi-apex2-air",
            read: |r| crate::flydigi::air(r).map(|a| Motion { gyro: crate::flydigi::air_rate(a), accel: None }),
        }),
        // One report at a time: the byte order is taken from it alone, as the detector does once
        // enough readings agree.
        "ds3-sixaxis" => Some(MotionDecoder {
            name: "ds3-sixaxis",
            read: |r| {
                let w = crate::ds3::words(r)?;
                let o = crate::ds3::guess(&w)?;
                Some(Motion { gyro: [0., crate::ds3::yaw(&w, o), 0.], accel: Some(crate::ds3::accel(&w, o)) })
            },
        }),
        _ => None,
    }
}

/// A gyro below this is still: a count or two of noise, a degree or so a second.
const STILL: f32 = 0.05;

/// A button: held while `byte & mask == value` (`value` is `mask` when not given).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub name: String,
    pub byte: usize,
    pub mask: u8,
    #[serde(default)]
    pub value: Option<u8>,
    /// What it is to OpenController ([`target`]), or "none".
    pub to: String,
}

/// A whole byte for a stick or an analog trigger.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisByte {
    pub name: String,
    pub byte: usize,
    pub to: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    pub name: String,
    pub bytes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotInput {
    pub name: String,
    pub report: String,
}

/// What a user's report caught: its "bytes that changed" line, pasted as it is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub what: String,
    pub moved: String,
}

/// What a control or a byte is to OpenController.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Button(u8),
    /// A trigger as a button: all the way or not at all.
    Trigger(usize),
    Axis(usize),
    None,
}

pub fn target(name: &str) -> Option<Target> {
    let b = |v: u32| Some(Target::Button(v as u8));
    match name {
        "south" => b(button::SOUTH),
        "east" => b(button::EAST),
        "west" => b(button::WEST),
        "north" => b(button::NORTH),
        "back" => b(button::BACK),
        "guide" => b(button::GUIDE),
        "start" => b(button::START),
        "left_stick" => b(button::LEFT_STICK),
        "right_stick" => b(button::RIGHT_STICK),
        "left_shoulder" => b(button::LEFT_SHOULDER),
        "right_shoulder" => b(button::RIGHT_SHOULDER),
        "dpad_up" => b(button::DPAD_UP),
        "dpad_down" => b(button::DPAD_DOWN),
        "dpad_left" => b(button::DPAD_LEFT),
        "dpad_right" => b(button::DPAD_RIGHT),
        "misc1" => b(button::MISC1),
        "touchpad" => b(button::TOUCHPAD),
        "right_paddle1" => Some(Target::Button(extras::RIGHT_PADDLE1)),
        "left_paddle1" => Some(Target::Button(extras::LEFT_PADDLE1)),
        "right_paddle2" => Some(Target::Button(extras::RIGHT_PADDLE2)),
        "left_paddle2" => Some(Target::Button(extras::LEFT_PADDLE2)),
        "misc2" => Some(Target::Button(extras::MISC2)),
        "misc3" => Some(Target::Button(extras::MISC3)),
        "misc4" => Some(Target::Button(extras::MISC4)),
        "misc5" => Some(Target::Button(extras::MISC5)),
        "misc6" => Some(Target::Button(extras::MISC6)),
        "left_trigger" => Some(Target::Trigger(axis::LEFT_TRIGGER)),
        "right_trigger" => Some(Target::Trigger(axis::RIGHT_TRIGGER)),
        "left_x" => Some(Target::Axis(axis::LEFT_X)),
        "left_y" => Some(Target::Axis(axis::LEFT_Y)),
        "right_x" => Some(Target::Axis(axis::RIGHT_X)),
        "right_y" => Some(Target::Axis(axis::RIGHT_Y)),
        "none" => Some(Target::None),
        _ => None,
    }
}

/// One of OpenController's readers of a report: what it puts in a state SDL gave, and which parts
/// are its own. The rest is SDL's: the decoder must leave it as SDL had it.
pub struct Decoder {
    pub name: &'static str,
    pub owns: fn(Target) -> bool,
    pub apply: fn(&[u8], &mut PadState),
}

pub fn decoder(name: &str) -> Option<Decoder> {
    use crate::flydigi::{Apex2, EXTRAS, ON_RECEIVER};
    match name {
        "flydigi-apex2-cable" => Some(Decoder {
            name: "flydigi-apex2-cable",
            owns: |t| matches!(t, Target::Button(b) if EXTRAS.contains(&b)) || matches!(t, Target::Trigger(_)),
            apply: |r, s| Apex2::default().after(r).apply(s),
        }),
        "flydigi-apex2-receiver" => Some(Decoder {
            name: "flydigi-apex2-receiver",
            owns: |t| matches!(t, Target::Button(b) if EXTRAS.contains(&b) || ON_RECEIVER.contains(&b)) || matches!(t, Target::Trigger(_)),
            apply: |r, s| {
                let a = Apex2::default().after(r);
                a.apply(s);
                a.apply_on_receiver(s);
            },
        }),
        _ => None,
    }
}

pub fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap_or_else(|_| panic!("not a hex byte: {b}"))).collect()
}

/// The pads in `pads/`, by file name.
pub fn pads() -> Vec<(String, Pad)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/testbench/pads");
    let mut out: Vec<(String, Pad)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let pad = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p.file_name().unwrap().to_string_lossy().into_owned(), pad)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A state as a short text: what is held and where the axes are.
fn show(s: &PadState) -> String {
    let held: Vec<String> = (0..64).filter(|b| s.buttons & 1 << b != 0).map(|b| b.to_string()).collect();
    format!("buttons [{}] axes {:?}", held.join(" "), s.axes)
}

/// A small deterministic random source, so a failing case comes back the same.
struct Lcg(u64);

impl Lcg {
    fn byte(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u8
    }
}

/// What a decoder makes of a report, from a state where SDL holds nothing.
fn decode(d: &Decoder, report: &[u8]) -> PadState {
    let mut s = PadState::default();
    (d.apply)(report, &mut s);
    s
}

/// What the decoder should make of the rest report with `controls` held.
fn expected(d: &Decoder, held: &[Target]) -> PadState {
    let mut s = PadState::default();
    for &t in held.iter().filter(|&&t| (d.owns)(t)) {
        match t {
            Target::Button(b) => s.buttons |= 1 << b,
            Target::Trigger(a) => s.axes[a] = i16::MAX,
            Target::Axis(_) | Target::None => {}
        }
    }
    s
}

fn press(report: &mut [u8], c: &Control) {
    let v = c.value.unwrap_or(c.mask);
    report[c.byte] = report[c.byte] & !c.mask | v;
}

/// Every check of the bench, for one pad and one of its decoders. What went wrong, one line each.
pub fn check(pad: &Pad, d: &Decoder) -> Vec<String> {
    let mut bad = Vec::new();
    let rest = hex(&pad.rest);
    let len = rest.len();
    let targets: Vec<Target> = pad.controls.iter().map(|c| target(&c.to).unwrap_or(Target::None)).collect();

    // The file itself.
    for c in &pad.controls {
        if target(&c.to).is_none() {
            bad.push(format!("{}: unknown target {:?}", c.name, c.to));
        }
        if c.byte >= len || c.byte < pad.signature {
            bad.push(format!("{}: byte {} outside the report or in its signature", c.name, c.byte));
            return bad;
        }
        if rest[c.byte] & c.mask == c.value.unwrap_or(c.mask) {
            bad.push(format!("{}: held in the rest report", c.name));
        }
    }
    for a in &pad.axes {
        if target(&a.to).is_none() || a.byte >= len {
            bad.push(format!("{}: unknown target {:?} or byte {} outside", a.name, a.to, a.byte));
        }
    }
    for (i, a) in pad.controls.iter().enumerate() {
        for b in &pad.controls[i + 1..] {
            if a.byte == b.byte && a.value.is_none() && b.value.is_none() && a.mask & b.mask != 0 {
                bad.push(format!("{} and {} share bits of byte {}", a.name, b.name, a.byte));
            }
        }
    }

    // At rest nothing is held.
    let none = decode(d, &rest);
    if none != PadState::default() {
        bad.push(format!("at rest: {}", show(&none)));
    }

    // Each control alone, then all at once.
    let mut all = rest.clone();
    for (c, &t) in pad.controls.iter().zip(&targets) {
        let mut r = rest.clone();
        press(&mut r, c);
        press(&mut all, c);
        let (got, want) = (decode(d, &r), expected(d, &[t]));
        if got != want {
            bad.push(format!("{} alone: got {}, want {}", c.name, show(&got), show(&want)));
        }
    }
    let (got, want) = (decode(d, &all), expected(d, &targets));
    if got != want {
        bad.push(format!("everything held: got {}, want {}", show(&got), show(&want)));
    }

    // Sticks and the bytes that move on their own change nothing the decoder owns.
    let free: Vec<(String, usize)> = pad
        .axes
        .iter()
        .filter(|a| !target(&a.to).is_some_and(|t| (d.owns)(t)))
        .map(|a| (a.name.clone(), a.byte))
        .chain(pad.noise.iter().flat_map(|n| n.bytes.iter().map(move |&b| (n.name.clone(), b))))
        .collect();
    for (name, byte) in &free {
        for v in [0x00, 0x01, 0x7f, 0x80, 0xfe, 0xff] {
            let mut r = rest.clone();
            r[*byte] = v;
            let got = decode(d, &r);
            if got != none {
                bad.push(format!("{name} (byte {byte}) at {v:#04x}: {}", show(&got)));
                break;
            }
        }
    }
    let mut rng = Lcg(0x5eed);
    for _ in 0..500 {
        let mut r = all.clone();
        for (_, byte) in &free {
            r[*byte] = rng.byte();
        }
        let got = decode(d, &r);
        if got != want {
            bad.push(format!("everything held, sticks and noise random: got {}", show(&got)));
            break;
        }
    }

    // Replies and cut reports are not input.
    for n in &pad.not_input {
        let got = decode(d, &hex(&n.report));
        if got != PadState::default() {
            bad.push(format!("{}: read as input, {}", n.name, show(&got)));
        }
    }
    // A report too short to hold every byte the decoder reads is not read at all: half of it
    // would be taken for released. Longer cuts may be read, as SDL reads them.
    let needed = pad.controls.iter().zip(&targets).filter(|&(_, &t)| (d.owns)(t)).map(|(c, _)| c.byte + 1).max().unwrap_or(0);
    for cut in 0..len {
        let got = decode(d, &all[..cut]);
        if cut < needed && got != PadState::default() {
            bad.push(format!("cut to {cut} bytes, short of byte {}: read as input, {}", needed - 1, show(&got)));
            break;
        }
    }

    // Anything at all, never a panic.
    for _ in 0..2000 {
        let mut r: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        r[..pad.signature].copy_from_slice(&rest[..pad.signature]);
        decode(d, &r);
        let n = usize::from(rng.byte()) % (len + 8);
        let junk: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
        decode(d, &junk);
    }
    bad
}

/// Every motion check, for one pad and its motion decoder. What went wrong, one line each.
pub fn check_motion(pad: &Pad, spec: &MotionSpec, d: &MotionDecoder) -> Vec<String> {
    let mut bad = Vec::new();
    let g = crate::ds4::STANDARD_GRAVITY;
    for s in &spec.samples {
        let Some(m) = (d.read)(&hex(&s.report)) else {
            bad.push(format!("{}: no reading", s.what));
            continue;
        };
        for (axis, (want, got)) in ["pitch", "yaw", "roll"].iter().zip(s.gyro.iter().zip(m.gyro)) {
            let ok = match want.as_str() {
                "0" => got.abs() < STILL,
                "+" => got >= STILL,
                "-" => got <= -STILL,
                "any" => true,
                other => {
                    bad.push(format!("{}: {axis} {other:?} is none of 0, +, -, any", s.what));
                    true
                }
            };
            if !ok {
                bad.push(format!("{}: {axis} {got:.3} rad/s, want {want}", s.what));
            }
        }
        match (s.accel_g, m.accel) {
            (Some(want), Some(a)) => {
                let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() / g;
                if (length - want).abs() > 0.15 * want.max(0.1) {
                    bad.push(format!("{}: accelerometer at {length:.2} g, want {want}", s.what));
                }
            }
            (Some(_), None) => bad.push(format!("{}: no accelerometer reading", s.what)),
            _ => {}
        }
    }
    let rest = hex(&pad.rest);
    let still = (d.read)(&rest);
    // Buttons and sticks never move the gyro.
    for c in &pad.controls {
        let mut r = rest.clone();
        press(&mut r, c);
        if (d.read)(&r) != still {
            bad.push(format!("{} moves the gyro", c.name));
        }
    }
    for a in &pad.axes {
        let mut r = rest.clone();
        r[a.byte] = 0xff;
        if (d.read)(&r) != still {
            bad.push(format!("{} (byte {}) moves the gyro", a.name, a.byte));
        }
    }
    // Anything at all: never a panic, and a reading is a number.
    let mut rng = Lcg(0x6d6f74);
    let finite = |m: &Motion| m.gyro.iter().chain(m.accel.iter().flatten()).all(|v| v.is_finite());
    for i in 0..3000 {
        let len = if i % 2 == 0 { rest.len() } else { usize::from(rng.byte()) % (rest.len() + 8) };
        let mut r: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let keep = pad.signature.min(len);
        r[..keep].copy_from_slice(&rest[..keep]);
        if let Some(m) = (d.read)(&r)
            && !finite(&m)
        {
            bad.push(format!("not a number from {r:02x?}"));
            break;
        }
    }
    for cut in 0..rest.len() {
        (d.read)(&rest[..cut]);
    }
    bad
}

/// The bytes and bits a user's report caught moving that the file says nothing about.
pub fn unexplained(pad: &Pad, capture: &Capture) -> Vec<String> {
    let mut known = vec![0u8; 256];
    for c in &pad.controls {
        known[c.byte] |= c.mask;
    }
    for a in &pad.axes {
        known[a.byte] = 0xff;
    }
    for n in &pad.noise {
        for &b in &n.bytes {
            known[b] = 0xff;
        }
    }
    let mut out = Vec::new();
    for part in capture.moved.split(", ") {
        // "byte 7 0 to 32 (bits 3f)"
        let words: Vec<&str> = part.split_whitespace().collect();
        let (Some(byte), Some(bits)) = (
            words.get(1).and_then(|b| b.parse::<usize>().ok()),
            words.last().and_then(|b| u8::from_str_radix(b.trim_end_matches(')'), 16).ok()),
        ) else {
            out.push(format!("could not read {part:?}"));
            continue;
        };
        let left = bits & !known.get(byte).copied().unwrap_or(0);
        if left != 0 {
            out.push(format!("byte {byte} bits {left:#04x} moved but nothing in the file uses them"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pad_passes_every_check_of_each_of_its_decoders() {
        let pads = pads();
        assert!(!pads.is_empty());
        let mut bad = Vec::new();
        for (file, pad) in &pads {
            for name in &pad.decoders {
                let Some(d) = decoder(name) else {
                    bad.push(format!("{file}: no decoder {name:?}"));
                    continue;
                };
                let problems = check(pad, &d);
                println!(
                    "{file} ({}; {}) with {}: {} controls, {} problems; from {} sources, {} not seen on a real one",
                    pad.name,
                    pad.connections,
                    d.name,
                    pad.controls.len(),
                    problems.len(),
                    pad.sources.len(),
                    pad.unseen.len()
                );
                bad.extend(problems.into_iter().map(|p| format!("{file} with {}: {p}", d.name)));
            }
        }
        assert!(bad.is_empty(), "\n{}", bad.join("\n"));
    }

    #[test]
    fn every_motion_reading_passes_its_checks() {
        let mut bad = Vec::new();
        let mut seen = 0;
        for (file, pad) in pads() {
            let Some(spec) = &pad.motion else { continue };
            let Some(d) = motion_decoder(&spec.decoder) else {
                bad.push(format!("{file}: no motion decoder {:?}", spec.decoder));
                continue;
            };
            seen += 1;
            let problems = check_motion(&pad, spec, &d);
            println!("{file} with {}: {} samples, {} problems", d.name, spec.samples.len(), problems.len());
            bad.extend(problems.into_iter().map(|p| format!("{file} with {}: {p}", d.name)));
        }
        assert!(seen >= 2, "the DualShock 3 and the Apex 2");
        assert!(bad.is_empty(), "\n{}", bad.join("\n"));
    }

    /// A gyro read the wrong way round fails the samples.
    #[test]
    fn the_bench_catches_a_gyro_turned_the_wrong_way() {
        let (_, pad) = pads().into_iter().find(|(f, _)| f == "flydigi-apex2.json").unwrap();
        let backwards = MotionDecoder {
            name: "backwards",
            read: |r| crate::flydigi::air(r).map(|a| Motion { gyro: crate::flydigi::air_rate((-a.0, -a.1)), accel: None }),
        };
        let problems = check_motion(&pad, pad.motion.as_ref().unwrap(), &backwards);
        assert!(problems.iter().any(|p| p.contains("yaw")), "{problems:#?}");
        assert!(problems.iter().any(|p| p.contains("pitch")), "{problems:#?}");
    }

    #[test]
    fn what_users_reports_caught_is_all_explained() {
        let mut bad = Vec::new();
        for (file, pad) in pads() {
            for c in &pad.captures {
                bad.extend(unexplained(&pad, c).into_iter().map(|p| format!("{file}, {}: {p}", c.what)));
            }
        }
        assert!(bad.is_empty(), "\n{}", bad.join("\n"));
    }

    #[test]
    fn ids_are_vendor_and_product_in_hex() {
        for (file, pad) in pads() {
            for id in &pad.ids {
                let ok = id.len() == 9 && id.as_bytes()[4] == b':' && id.split(':').all(|h| u16::from_str_radix(h, 16).is_ok());
                assert!(ok, "{file}: {id}");
            }
        }
    }

    /// The bench finds what it is for: a decoder that reads the wheel as triggers, as SDL did on
    /// the Apex 2, and a byte the file forgot.
    #[test]
    fn the_bench_catches_the_apex_2_bug_and_a_gap_in_a_file() {
        let (_, pad) = pads().into_iter().find(|(f, _)| f == "flydigi-apex2.json").unwrap();
        let wheel_as_triggers = Decoder {
            name: "like SDL 3.4",
            owns: |t| matches!(t, Target::Trigger(_)),
            apply: |r, s| {
                if r.len() > 24 {
                    s.axes[axis::LEFT_TRIGGER] = i16::from(r[23]) * 128;
                    s.axes[axis::RIGHT_TRIGGER] = i16::from(r[24]) * 128;
                }
            },
        };
        let problems = check(&pad, &wheel_as_triggers);
        assert!(problems.iter().any(|p| p.starts_with("at rest")), "{problems:#?}");
        assert!(problems.iter().any(|p| p.starts_with("LT alone")), "{problems:#?}");
        let gap = Capture { what: "test".into(), moved: "byte 7 0 to 32 (bits 3f), byte 12 0 to 4 (bits 04)".into() };
        assert_eq!(unexplained(&pad, &gap), ["byte 12 bits 0x04 moved but nothing in the file uses them"]);
    }
}
