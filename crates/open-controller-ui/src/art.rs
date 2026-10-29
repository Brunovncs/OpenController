//! The controller, drawn. Each kind is traced from its maker's own front view (the drawing in
//! its manual, or a straight product shot) onto a 400 by 280 grid: the outline, the bumpers, the
//! sticks and every button where the real one has them. The shapes live in `assets/pads.json`,
//! which the website draws from too, and are lit here as buttons are pressed.

use crate::theme::Theme;
use gpui::{
    AnyElement, Background, Bounds, ContentMask, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point, Styled, Window, canvas, div,
    hsla, linear_color_stop, linear_gradient, point, px, rgb, size,
};
use open_controller_core::PadView;
use open_controller_core::device::{Brand, Form};
use open_controller_core::extras::{self, Art, Family};
use open_controller_core::mapping::{PadState, axis, button};
use open_controller_core::models;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const W: f32 = 400.;
const H: f32 = 280.;
/// Callers size drawings as for a 320-wide one.
const UNIT: f32 = 320. / W;

static PADS: &str = include_str!("../../../assets/pads.json");

/// The drawing every flight stick gets that has none of its own.
const FLIGHT_STICK: &str = "logitechExtreme3d";

/// Which drawing a controller gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    DualSense,
    DualSenseEdge,
    DualShock4,
    DualShock3,
    Xbox,
    SwitchPro,
    JoyCons,
    /// A Joy-Con on its own, held sideways.
    JoyConLeft,
    JoyConRight,
    Ultimate2,
    Ultimate2C,
    Ultimate,
    Sn30Pro,
    Retro,
    /// Redragon Darkflame G820, a PlayStation-style pad that passes itself off as a DualShock 4 or
    /// an Xbox 360 pad: told apart only by a name that says so.
    DarkflameG820,
    Handheld,
    Generic,
    /// A model with a drawing of its own, by its key in `pads.json` (`models::DRAWINGS`).
    Traced(&'static str),
}

impl Look {
    #[cfg(test)]
    pub const ALL: [Look; 17] = [
        Look::DualSense,
        Look::DualSenseEdge,
        Look::DualShock4,
        Look::DualShock3,
        Look::Xbox,
        Look::SwitchPro,
        Look::JoyCons,
        Look::JoyConLeft,
        Look::JoyConRight,
        Look::Ultimate2,
        Look::Ultimate2C,
        Look::Ultimate,
        Look::Sn30Pro,
        Look::Retro,
        Look::DarkflameG820,
        Look::Handheld,
        Look::Generic,
    ];

    /// Its name in `pads.json`.
    pub fn key(self) -> &'static str {
        match self {
            Look::DualSense => "dualsense",
            Look::DualSenseEdge => "dualsenseEdge",
            Look::DualShock4 => "ds4",
            Look::DualShock3 => "ds3",
            Look::Xbox => "xbox",
            Look::SwitchPro => "switchPro",
            Look::JoyCons => "joycons",
            Look::JoyConLeft => "joyconLeft",
            Look::JoyConRight => "joyconRight",
            Look::Ultimate2 => "ultimate2",
            Look::Ultimate2C => "ultimate2c",
            Look::Ultimate => "ultimate",
            Look::Sn30Pro => "sn30pro",
            Look::Retro => "retro",
            Look::DarkflameG820 => "g820",
            Look::Handheld => "handheld",
            Look::Generic => "generic",
            Look::Traced(key) => key,
        }
    }
}

/// How the face buttons are labelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphs {
    Xbox,
    PlayStation,
    Nintendo,
    /// 1 to 4, clockwise from the top, as many PC pads print them.
    Numbers,
}

pub fn look(pad: &PadView) -> Look {
    use Family::*;
    let name = pad.name.to_ascii_lowercase();
    let ultimate = name.contains("ultimate");
    if name.contains("g820") || name.contains("darkflame") {
        return Look::DarkflameG820;
    }
    if let Some(a) = models::alias(pad.vendor, pad.product, pad.profiles.model.as_deref()) {
        return Look::Traced(a.5);
    }
    if let Some(key) = models::drawing(pad.vendor, pad.product) {
        return Look::Traced(key);
    }
    if pad.form == Form::FlightStick {
        return Look::Traced(FLIGHT_STICK);
    }
    match pad.family {
        DualSenseEdge => Look::DualSenseEdge,
        DualSense => Look::DualSense,
        DualShock4 => Look::DualShock4,
        DualShock3 | Ps2Adapter => Look::DualShock3,
        Xbox | XboxElite => Look::Xbox,
        SwitchPro => Look::SwitchPro,
        JoyCons if pad.art == Art::JoyConLeft => Look::JoyConLeft,
        JoyCons if pad.art == Art::JoyConRight => Look::JoyConRight,
        JoyCons => Look::JoyCons,
        Switch2 if pad.name.contains("Joy-Con") => Look::JoyCons,
        Switch2 if pad.name.contains("Pro Controller") => Look::SwitchPro,
        EightBitDoUltimate2C => Look::Ultimate2C,
        EightBitDoUltimate => Look::Ultimate,
        // 2dc8:3106 is also the first Ultimate (2.4G, or Bluetooth on its receiver) in XInput mode.
        EightBitDoPro2 if ultimate => Look::Ultimate,
        EightBitDoFour if pad.art == Art::Offset || ultimate => Look::Ultimate2,
        EightBitDoFour | EightBitDoPro2 => Look::Sn30Pro,
        EightBitDo if pad.art == Art::Retro => Look::Retro,
        EightBitDo if pad.art == Art::Symmetric => Look::Sn30Pro,
        EightBitDo if name.contains("ultimate 2c") => Look::Ultimate2C,
        EightBitDo if ultimate => Look::Ultimate,
        _ => match pad.art {
            Art::Retro => Look::Retro,
            Art::Handheld => Look::Handheld,
            Art::JoyCons => Look::JoyCons,
            Art::JoyConLeft => Look::JoyConLeft,
            Art::JoyConRight => Look::JoyConRight,
            Art::PlayStation => Look::DualSense,
            Art::Symmetric => Look::DualShock3,
            Art::Offset => match pad.brand {
                Brand::Xbox => Look::Xbox,
                Brand::Nintendo => Look::SwitchPro,
                _ => Look::Generic,
            },
        },
    }
}

/// What a model's own drawing prints on a button that its family does not name: the Extreme 3D
/// Pro's numbers.
pub fn printed_label(pad: &PadView, b: u8) -> Option<&'static str> {
    models::drawing(pad.vendor, pad.product)?;
    let input = Some(u32::from(b));
    pad_for(look(pad)).buttons.iter().find(|x| x.input == input).and_then(|x| x.label.as_deref())
}

pub fn glyphs(pad: &PadView, look: Look) -> Glyphs {
    if look == Look::DarkflameG820 {
        return Glyphs::PlayStation;
    }
    if let Look::Traced(_) = look {
        // What its own drawing says is printed on it.
        return match pad_for(look).glyphs.as_deref() {
            Some("ps") => Glyphs::PlayStation,
            Some("nintendo") => Glyphs::Nintendo,
            Some("numbers") => Glyphs::Numbers,
            _ => Glyphs::Xbox,
        };
    }
    match pad.brand {
        Brand::PlayStation => Glyphs::PlayStation,
        Brand::Nintendo => Glyphs::Nintendo,
        // 8BitDo's Super Nintendo shapes print Nintendo's letters, except the ones made for Xbox.
        _ if matches!(look, Look::Sn30Pro | Look::Retro) && !pad.name.contains("for Xbox") => Glyphs::Nintendo,
        _ => Glyphs::Xbox,
    }
}

// The geometry file.

#[derive(Deserialize)]
struct RawStick {
    /// The cap, as seen from the front.
    circle: [f32; 3],
    /// The opening it moves in, when the drawing shows one.
    well: Option<f32>,
    /// A light ring round it: its radius and width.
    ring: Option<[f32; 2]>,
    /// A flight stick's grip, seen from above, drawn in place of the cap: it leans with the stick
    /// and turns about it with the right stick's x, the twist, by up to `twist` degrees.
    grip: Option<String>,
    twist: Option<f32>,
}

#[derive(Deserialize)]
struct RawButton {
    input: String,
    circle: Option<[f32; 3]>,
    d: Option<String>,
    icon: Option<String>,
    /// What is printed on it, when that is not the glyph set's letter.
    label: Option<String>,
    /// Set on the grip of a flight stick, so it moves with it.
    #[serde(default)]
    grip: bool,
}

/// A lever that stays where it is put, a throttle: its slot, filled from the back as far as the
/// trigger it stands for (`lt` or `rt`) is pulled.
#[derive(Deserialize)]
struct RawLever {
    input: String,
    d: String,
}

#[derive(Deserialize)]
struct RawPanel {
    style: String,
    d: String,
}

#[derive(Deserialize)]
struct RawPad {
    body: String,
    bumpers: Option<[String; 2]>,
    triggers: Option<[String; 2]>,
    #[serde(default)]
    panels: Vec<RawPanel>,
    touchpad: Option<String>,
    #[serde(default)]
    dots: Vec<[f32; 3]>,
    #[serde(default)]
    levers: Vec<RawLever>,
    #[serde(default)]
    sticks: Vec<RawStick>,
    #[serde(default)]
    buttons: Vec<RawButton>,
    #[serde(default)]
    tint: bool,
    glyphs: Option<String>,
}

#[derive(Clone, Copy)]
enum Op {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

type Ops = Vec<Op>;

/// Numbers and commands of an SVG path, in order.
fn tokens(d: &str) -> Vec<Result<f32, char>> {
    let mut out = Vec::new();
    let mut num = String::new();
    let end = |num: &mut String, out: &mut Vec<Result<f32, char>>| {
        if !num.is_empty() {
            out.push(Ok(num.parse().unwrap_or(0.)));
            num.clear();
        }
    };
    for ch in d.chars() {
        match ch {
            '0'..='9' => num.push(ch),
            '.' if num.contains('.') && !num.contains('e') => {
                end(&mut num, &mut out);
                num.push(ch);
            }
            '-' | '+' if !num.ends_with('e') => {
                end(&mut num, &mut out);
                num.push(ch);
            }
            '.' | '-' | '+' | 'e' => num.push(ch),
            c if c.is_ascii_alphabetic() => {
                end(&mut num, &mut out);
                out.push(Err(c));
            }
            _ => end(&mut num, &mut out),
        }
    }
    end(&mut num, &mut out);
    out
}

/// The SVG path syntax the geometry file uses: M, L, H, V, C and Z, absolute or relative.
fn parse(d: &str) -> Ops {
    let mut ops = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0f32, 0f32, 0f32, 0f32);
    let mut cmd = 'M';
    let mut args: Vec<f32> = Vec::new();
    let arity = |c: char| match c.to_ascii_uppercase() {
        'M' | 'L' => 2,
        'H' | 'V' => 1,
        'C' => 6,
        _ => 0,
    };
    let mut first = true;
    for t in tokens(d) {
        match t {
            Err(c) => {
                cmd = c;
                first = true;
                args.clear();
                if c.eq_ignore_ascii_case(&'z') {
                    ops.push(Op::Z);
                    (x, y) = (sx, sy);
                }
            }
            Ok(v) => {
                args.push(v);
                if args.len() < arity(cmd) {
                    continue;
                }
                let rel = cmd.is_ascii_lowercase();
                let (ox, oy) = if rel { (x, y) } else { (0., 0.) };
                match cmd.to_ascii_uppercase() {
                    'M' if first => {
                        (x, y) = (args[0] + ox, args[1] + oy);
                        (sx, sy) = (x, y);
                        ops.push(Op::M(x, y));
                    }
                    'M' | 'L' => {
                        (x, y) = (args[0] + ox, args[1] + oy);
                        ops.push(Op::L(x, y));
                    }
                    'H' => {
                        x = args[0] + ox;
                        ops.push(Op::L(x, y));
                    }
                    'V' => {
                        y = args[0] + oy;
                        ops.push(Op::L(x, y));
                    }
                    'C' => {
                        let a = &args;
                        ops.push(Op::C(a[0] + ox, a[1] + oy, a[2] + ox, a[3] + oy, a[4] + ox, a[5] + oy));
                        (x, y) = (a[4] + ox, a[5] + oy);
                    }
                    _ => {}
                }
                first = false;
                args.clear();
            }
        }
    }
    ops
}

const K: f32 = 0.552_284_8;

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Ops {
    let (kx, ky) = (rx * K, ry * K);
    vec![
        Op::M(cx + rx, cy),
        Op::C(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        Op::C(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        Op::C(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        Op::C(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        Op::Z,
    ]
}

fn circle(cx: f32, cy: f32, r: f32) -> Ops {
    ellipse(cx, cy, r, r)
}

fn rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Ops {
    let r = r.min(w / 2.).min(h / 2.);
    let k = r * (1. - K);
    vec![
        Op::M(x + r, y),
        Op::L(x + w - r, y),
        Op::C(x + w - k, y, x + w, y + k, x + w, y + r),
        Op::L(x + w, y + h - r),
        Op::C(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h),
        Op::L(x + r, y + h),
        Op::C(x + k, y + h, x, y + h - k, x, y + h - r),
        Op::L(x, y + r),
        Op::C(x, y + k, x + k, y, x + r, y),
        Op::Z,
    ]
}

fn poly(points: &[(f32, f32)]) -> Ops {
    let mut v: Ops = points.iter().enumerate().map(|(i, &(x, y))| if i == 0 { Op::M(x, y) } else { Op::L(x, y) }).collect();
    v.push(Op::Z);
    v
}

fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> Ops {
    vec![Op::M(x1, y1), Op::L(x2, y2)]
}

fn star(x: f32, y: f32, r: f32) -> Ops {
    let pts: Vec<(f32, f32)> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.;
            let rr = if i % 2 == 1 { r * 0.45 } else { r };
            (x + rr * a.cos(), y + rr * a.sin())
        })
        .collect();
    poly(&pts)
}

fn plus(x: f32, y: f32, a: f32, t: f32) -> Ops {
    poly(&[
        (x - t, y - a),
        (x + t, y - a),
        (x + t, y - t),
        (x + a, y - t),
        (x + a, y + t),
        (x + t, y + t),
        (x + t, y + a),
        (x - t, y + a),
        (x - t, y + t),
        (x - a, y + t),
        (x - a, y - t),
        (x - t, y - t),
    ])
}

/// The box round a shape: left, top, right, bottom.
fn extent(ops: &Ops) -> (f32, f32, f32, f32) {
    let pts = ops.iter().flat_map(|op| match *op {
        Op::M(x, y) | Op::L(x, y) => vec![(x, y)],
        Op::C(a, b, c, d, x, y) => vec![(a, b), (c, d), (x, y)],
        Op::Z => vec![],
    });
    pts.fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(l, t, r, b), (x, y)| (l.min(x), t.min(y), r.max(x), b.max(y)))
}

/// Where a flight stick's grip is: leant with the stick by `(dx, dy)` and turned by `angle`
/// radians about its pivot.
#[derive(Clone, Copy)]
struct Turn {
    px: f32,
    py: f32,
    dx: f32,
    dy: f32,
    cos: f32,
    sin: f32,
}

impl Turn {
    const NONE: Turn = Turn { px: 0., py: 0., dx: 0., dy: 0., cos: 1., sin: 0. };

    fn new(px: f32, py: f32, (dx, dy): (f32, f32), angle: f32) -> Turn {
        Turn { px, py, dx, dy, cos: angle.cos(), sin: angle.sin() }
    }

    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let (rx, ry) = (x - self.px, y - self.py);
        (self.px + rx * self.cos - ry * self.sin + self.dx, self.py + rx * self.sin + ry * self.cos + self.dy)
    }

    fn ops(&self, ops: &Ops) -> Ops {
        let at = |x, y| self.at(x, y);
        ops.iter()
            .map(|op| match *op {
                Op::M(x, y) => {
                    let (x, y) = at(x, y);
                    Op::M(x, y)
                }
                Op::L(x, y) => {
                    let (x, y) = at(x, y);
                    Op::L(x, y)
                }
                Op::C(a, b, c, d, x, y) => {
                    let ((a, b), (c, d), (x, y)) = (at(a, b), at(c, d), at(x, y));
                    Op::C(a, b, c, d, x, y)
                }
                Op::Z => Op::Z,
            })
            .collect()
    }
}

/// The highest point of a shape, for filling a trigger from the bottom up.
fn top(ops: &Ops) -> (f32, f32) {
    let ys = ops.iter().flat_map(|op| match *op {
        Op::M(_, y) | Op::L(_, y) => vec![y],
        Op::C(_, a, _, b, _, c) => vec![a, b, c],
        Op::Z => vec![],
    });
    ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)))
}

/// Triggers drawn as buttons (a Mega Drive pad's C and Z sit on them), past the 64 button bits.
const LEFT_TRIGGER_INPUT: u32 = 64;
const RIGHT_TRIGGER_INPUT: u32 = 65;

/// Whether a button's input is held; a trigger counts once pulled halfway.
fn held(state: &PadState, b: u32) -> bool {
    match b {
        LEFT_TRIGGER_INPUT => state.axes[axis::LEFT_TRIGGER] > i16::MAX / 2,
        RIGHT_TRIGGER_INPUT => state.axes[axis::RIGHT_TRIGGER] > i16::MAX / 2,
        _ => b < 64 && state.pressed(b),
    }
}

/// The input a button stands for, as an SDL button index.
fn input(name: &str) -> Option<u32> {
    Some(match name {
        "south" => button::SOUTH,
        "east" => button::EAST,
        "west" => button::WEST,
        "north" => button::NORTH,
        "back" => button::BACK,
        "guide" => button::GUIDE,
        "start" => button::START,
        "lstick" => button::LEFT_STICK,
        "rstick" => button::RIGHT_STICK,
        "up" => button::DPAD_UP,
        "down" => button::DPAD_DOWN,
        "left" => button::DPAD_LEFT,
        "right" => button::DPAD_RIGHT,
        "misc1" => button::MISC1,
        "misc2" => u32::from(extras::MISC2),
        "misc3" => u32::from(extras::MISC3),
        "misc4" => u32::from(extras::MISC4),
        "misc5" => u32::from(extras::MISC5),
        "misc6" => u32::from(extras::MISC6),
        // Shoulder buttons on the face: the original Xbox controller's white and black buttons.
        "lb" => button::LEFT_SHOULDER,
        "rb" => button::RIGHT_SHOULDER,
        "lt" => LEFT_TRIGGER_INPUT,
        "rt" => RIGHT_TRIGGER_INPUT,
        "touchpad" => button::TOUCHPAD,
        "rp1" => u32::from(extras::RIGHT_PADDLE1),
        "lp1" => u32::from(extras::LEFT_PADDLE1),
        "rp2" => u32::from(extras::RIGHT_PADDLE2),
        "lp2" => u32::from(extras::LEFT_PADDLE2),
        _ => return None,
    })
}

enum Shape {
    Circle(f32, f32, f32),
    Path(Ops),
}

struct Btn {
    /// Carried by a flight stick's grip.
    grip: bool,
    input: Option<u32>,
    /// Which face button it is, south, east, west, north, for its label.
    face: Option<usize>,
    shape: Shape,
    icon: Option<String>,
    label: Option<String>,
}

struct Stick {
    x: f32,
    y: f32,
    cap: f32,
    well: f32,
    ring: Option<[f32; 2]>,
    grip: Option<Ops>,
    /// How far the grip turns with the twist, in radians.
    twist: f32,
}

impl Stick {
    /// How far the cap leans at full tilt: as far as the opening lets it. A grip, much longer
    /// than the opening, leans less, so its tip stays in the picture.
    fn reach(&self) -> f32 {
        if self.grip.is_some() { self.cap * 0.4 } else { (self.well - self.cap).max(self.cap * 0.25) * 0.8 }
    }
}

struct Pad {
    body: Ops,
    bumpers: Option<[Ops; 2]>,
    triggers: Option<[Ops; 2]>,
    panels: Vec<(String, Ops)>,
    touchpad: Option<Ops>,
    dots: Vec<[f32; 3]>,
    /// The axis each lever follows, and its slot.
    levers: Vec<(usize, Ops)>,
    sticks: Vec<Stick>,
    buttons: Vec<Btn>,
    tint: bool,
    /// The face buttons' symbols as printed: `xbox`, `ps` or `nintendo`.
    glyphs: Option<String>,
}

fn pads() -> &'static HashMap<String, Pad> {
    static ALL: OnceLock<HashMap<String, Pad>> = OnceLock::new();
    ALL.get_or_init(|| {
        let raw: HashMap<String, RawPad> = serde_json::from_str(PADS).expect("assets/pads.json is malformed");
        raw.into_iter()
            .map(|(k, r)| {
                let buttons = r
                    .buttons
                    .into_iter()
                    .map(|b| Btn {
                        grip: b.grip,
                        face: ["south", "east", "west", "north"].iter().position(|f| *f == b.input),
                        input: input(&b.input),
                        shape: match (b.circle, &b.d) {
                            (Some([x, y, r]), _) => Shape::Circle(x, y, r),
                            (None, Some(d)) => Shape::Path(parse(d)),
                            (None, None) => Shape::Path(Vec::new()),
                        },
                        icon: b.icon,
                        label: b.label,
                    })
                    .collect();
                let pair = |p: Option<[String; 2]>| p.map(|[a, b]| [parse(&a), parse(&b)]);
                let pad = Pad {
                    body: parse(&r.body),
                    bumpers: pair(r.bumpers),
                    triggers: pair(r.triggers),
                    panels: r.panels.into_iter().map(|p| (p.style, parse(&p.d))).collect(),
                    touchpad: r.touchpad.as_deref().map(parse),
                    dots: r.dots,
                    levers: r
                        .levers
                        .into_iter()
                        .map(|l| (if l.input == "lt" { axis::LEFT_TRIGGER } else { axis::RIGHT_TRIGGER }, parse(&l.d)))
                        .collect(),
                    sticks: r
                        .sticks
                        .into_iter()
                        .map(|s| {
                            let [x, y, cap] = s.circle;
                            Stick {
                                x,
                                y,
                                cap,
                                well: s.well.unwrap_or(cap * 1.18),
                                ring: s.ring,
                                grip: s.grip.as_deref().map(parse),
                                twist: s.twist.unwrap_or(0.).to_radians(),
                            }
                        })
                        .collect(),
                    buttons,
                    tint: r.tint,
                    glyphs: r.glyphs,
                };
                (k, pad)
            })
            .collect()
    })
}

fn pad_for(look: Look) -> &'static Pad {
    let all = pads();
    all.get(look.key()).or_else(|| all.get(Look::Generic.key())).or_else(|| all.values().next()).expect("assets/pads.json has no pads")
}

// Painting.

struct Pen<'a> {
    window: &'a mut Window,
    o: Point<Pixels>,
    k: f32,
}

impl Pen<'_> {
    fn build(&self, ops: &Ops, stroke: Option<f32>) -> Option<gpui::Path<Pixels>> {
        let at = |x: f32, y: f32| self.o + point(px(x * self.k), px(y * self.k));
        let mut b = match stroke {
            Some(w) => PathBuilder::stroke(px(w)),
            None => PathBuilder::fill(),
        };
        for &op in ops {
            match op {
                Op::M(x, y) => b.move_to(at(x, y)),
                Op::L(x, y) => b.line_to(at(x, y)),
                Op::C(a, c, e, f, x, y) => b.cubic_bezier_to(at(x, y), at(a, c), at(e, f)),
                Op::Z => b.close(),
            }
        }
        b.build().ok()
    }

    fn fill(&mut self, ops: &Ops, color: impl Into<Background>) {
        if let Some(p) = self.build(ops, None) {
            self.window.paint_path(p, color);
        }
    }

    fn stroke(&mut self, ops: &Ops, width: f32, color: Hsla) {
        if let Some(p) = self.build(ops, Some(width)) {
            self.window.paint_path(p, color);
        }
    }

    fn shape(&mut self, ops: &Ops, fill: Hsla, edge: Hsla) {
        self.fill(ops, fill);
        self.stroke(ops, 1., edge);
    }

    /// Only what falls between `y0` and `y1` gets painted.
    fn band(&mut self, y0: f32, y1: f32, f: impl for<'w> FnOnce(&mut Pen<'w>)) {
        let bounds = Bounds::new(self.o + point(px(0.), px(y0 * self.k)), size(px(W * self.k), px((y1 - y0).max(0.) * self.k)));
        let (window, o, k) = (&mut *self.window, self.o, self.k);
        window.with_content_mask(Some(ContentMask { bounds }), |window| f(&mut Pen { window, o, k }));
    }
}

#[derive(Clone, Copy)]
struct Ink {
    body_top: Hsla,
    body_bottom: Hsla,
    edge: Hsla,
    soft: Hsla,
    part: Hsla,
    shade: Hsla,
    lit: Hsla,
    on_lit: Hsla,
    label: Hsla,
}

fn ink(t: &Theme) -> Ink {
    Ink {
        body_top: t.art_body_top,
        body_bottom: t.art_body_bottom,
        edge: t.stroke_strong,
        soft: t.stroke_strong.opacity(0.55),
        part: t.art_well,
        shade: hsla(0., 0., 0., 0.28),
        lit: t.accent,
        on_lit: t.on_accent,
        label: t.text2,
    }
}

const XBOX_TINT: [u32; 4] = [0x79dc95, 0xff8080, 0x76b3ff, 0xffd36e];
const PS_TINT: [u32; 4] = [0x8fb1ff, 0xff8a98, 0xf2a2d8, 0x6fe0c6];

/// A small button's symbol, as a filled shape or as lines, in the button's own size.
fn icon(name: &str, x: f32, y: f32, r: f32) -> Option<(Ops, Option<f32>)> {
    Some(match name {
        "minus" => (rect(x - r * 0.45, y - r * 0.11, r * 0.9, r * 0.22, r * 0.05), None),
        "plus" => (plus(x, y, r * 0.45, r * 0.11), None),
        "star" => (star(x, y, r * 0.55), None),
        "square" => (rect(x - r * 0.36, y - r * 0.36, r * 0.72, r * 0.72, r * 0.14), None),
        "ring" => (circle(x, y, r * 0.74), Some(1.)),
        "home" => (
            poly(&[
                (x, y - r * 0.5),
                (x + r * 0.5, y - r * 0.05),
                (x + r * 0.32, y - r * 0.05),
                (x + r * 0.32, y + r * 0.45),
                (x - r * 0.32, y + r * 0.45),
                (x - r * 0.32, y - r * 0.05),
                (x - r * 0.5, y - r * 0.05),
            ]),
            None,
        ),
        "capture" => (circle(x, y, r * 0.42), Some(1.)),
        "menu" => {
            let mut v = line(x - r * 0.42, y - r * 0.28, x + r * 0.42, y - r * 0.28);
            v.extend(line(x - r * 0.42, y, x + r * 0.42, y));
            v.extend(line(x - r * 0.42, y + r * 0.28, x + r * 0.42, y + r * 0.28));
            (v, Some(1.))
        }
        "view" => {
            let mut v = rect(x - r * 0.42, y - r * 0.12, r * 0.55, r * 0.44, r * 0.08);
            v.extend(rect(x - r * 0.13, y - r * 0.32, r * 0.55, r * 0.44, r * 0.08));
            (v, Some(1.))
        }
        "share" => {
            let mut v = line(x, y - r * 0.4, x, y + r * 0.12);
            v.extend(line(x - r * 0.2, y - r * 0.2, x, y - r * 0.4));
            v.extend(line(x + r * 0.2, y - r * 0.2, x, y - r * 0.4));
            v.extend(
                poly(&[(x - r * 0.42, y), (x - r * 0.42, y + r * 0.36), (x + r * 0.42, y + r * 0.36), (x + r * 0.42, y)])
                    .into_iter()
                    .filter(|o| !matches!(o, Op::Z)),
            );
            (v, Some(1.))
        }
        "xbox" => {
            let mut v = circle(x, y, r * 0.62);
            v.extend(line(x - r * 0.36, y - r * 0.36, x + r * 0.36, y + r * 0.36));
            v.extend(line(x + r * 0.36, y - r * 0.36, x - r * 0.36, y + r * 0.36));
            (v, Some(1.2))
        }
        "ps" => (circle(x, y, r * 0.5), Some(1.)),
        "mic" => (rect(x - r * 0.5, y - r * 0.12, r, r * 0.24, r * 0.12), None),
        _ => return None,
    })
}

/// The drawing of a controller in its current state, `scale` times the size of a 320-wide one.
pub fn controller(pad: &PadView, scale: f32, t: &Theme) -> AnyElement {
    let look = look(pad);
    let glyphs = glyphs(pad, look);
    let state: PadState = pad.input;
    let k = scale * UNIT;
    let ink = ink(t);
    let g = pad_for(look);
    let (turn, gripped) = grip_turn(g, &state);

    let painted = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut pen = Pen { window, o: bounds.origin, k };
            let on = |b: u32| held(&state, b);
            let lit_or = |b: Option<u32>, idle: Hsla| if b.is_some_and(on) { ink.lit } else { idle };
            let value = |a: usize| (state.axes[a].max(0) as f32 / 32767.).clamp(0., 1.);

            // Triggers behind the bumpers, filled from the bottom as they are pulled.
            if let Some(tr) = &g.triggers {
                for (ops, a) in [(&tr[0], axis::LEFT_TRIGGER), (&tr[1], axis::RIGHT_TRIGGER)] {
                    let v = value(a);
                    pen.shape(ops, ink.part, if v > 0.05 { ink.lit } else { ink.edge });
                    if v > 0.02 {
                        let (y0, y1) = top(ops);
                        pen.band(y1 - (y1 - y0) * v, H, |pen| pen.fill(ops, ink.lit.opacity(0.9)));
                    }
                }
            }
            if let Some(bm) = &g.bumpers {
                for (ops, b) in [(&bm[0], button::LEFT_SHOULDER), (&bm[1], button::RIGHT_SHOULDER)] {
                    pen.shape(ops, lit_or(Some(b), ink.part), lit_or(Some(b), ink.edge));
                }
            }

            // The body, then what is printed or set into it.
            pen.fill(&g.body, linear_gradient(180., linear_color_stop(ink.body_top, 0.), linear_color_stop(ink.body_bottom, 1.)));
            pen.stroke(&g.body, 1., ink.edge);
            for (style, ops) in &g.panels {
                match style.as_str() {
                    "line" => pen.stroke(ops, (1.2 * k).max(0.6), ink.soft),
                    "glow" => pen.stroke(ops, (2.2 * k).max(0.8), ink.lit.opacity(0.55)),
                    "light" => pen.shape(ops, ink.lit.opacity(0.28), ink.lit.opacity(0.5)),
                    "part" => pen.shape(ops, ink.part, ink.edge),
                    _ => pen.shape(ops, ink.shade, ink.soft),
                }
            }
            for &[x, y, r] in &g.dots {
                pen.fill(&circle(x, y, r), ink.soft);
            }
            if let Some(tp) = &g.touchpad {
                let b = Some(button::TOUCHPAD);
                pen.shape(tp, lit_or(b, ink.part), lit_or(b, ink.edge));
            }

            // Levers set into the body, filled from the back, with their handle where it is.
            for (a, ops) in &g.levers {
                let v = value(*a);
                let edge = if v > 0.05 { ink.lit } else { ink.edge };
                pen.shape(ops, ink.part, edge);
                let (l, t0, r, b) = extent(ops);
                if v > 0.02 {
                    pen.band(b - (b - t0) * v, H, |pen| pen.fill(ops, ink.lit.opacity(0.9)));
                }
                let y = b - 3.5 - (b - t0 - 7.) * v;
                pen.shape(&rect(l - 3., y - 3.5, r - l + 6., 7., 3.), ink.body_top, edge);
            }

            // Sticks: the light ring round them, the well, and the cap following the stick. A
            // flight stick's grip comes later, above the base's buttons.
            for (i, st) in g.sticks.iter().enumerate().take(2) {
                let (ax, ay, click) = if i == 0 {
                    (axis::LEFT_X, axis::LEFT_Y, button::LEFT_STICK)
                } else {
                    (axis::RIGHT_X, axis::RIGHT_Y, button::RIGHT_STICK)
                };
                let (mx, my) = (state.axes[ax] as f32 / 32768., state.axes[ay] as f32 / 32768.);
                let clicked = st.grip.is_none() && on(click);
                let active = if st.grip.is_some() { gripped } else { mx.hypot(my) > 0.2 || clicked };
                if let Some([rr, w]) = st.ring {
                    pen.stroke(&circle(st.x, st.y, rr), (w * k).max(1.), if active { ink.lit.opacity(0.75) } else { ink.soft });
                }
                pen.shape(&circle(st.x, st.y, st.well), ink.shade, if active { ink.lit.opacity(0.6) } else { ink.soft });
                if st.grip.is_some() {
                    continue;
                }
                let (cx, cy) = (st.x + mx * st.reach(), st.y + my * st.reach());
                pen.shape(&circle(cx, cy, st.cap), if clicked { ink.lit } else { ink.body_top }, if active { ink.lit } else { ink.edge });
                pen.stroke(&circle(cx, cy, st.cap * 0.62), 1., if clicked { ink.on_lit.opacity(0.3) } else { ink.soft });
            }

            // Buttons: their shape, and the symbol on the small ones. The grip and the buttons on
            // it go last, leant and turned with the stick.
            for on_grip in [false, true] {
                let turned = if on_grip { turn } else { Turn::NONE };
                if on_grip && let Some(grip) = g.sticks.first().and_then(|s| s.grip.as_ref()) {
                    let ops = turned.ops(grip);
                    pen.fill(&Turn { dx: turned.dx + 3., dy: turned.dy + 4., ..turned }.ops(grip), ink.shade);
                    pen.fill(&ops, linear_gradient(180., linear_color_stop(ink.body_top, 0.), linear_color_stop(ink.body_bottom, 1.)));
                    pen.stroke(&ops, 1., if gripped { ink.lit } else { ink.edge });
                }
                for b in g.buttons.iter().filter(|b| b.grip == on_grip) {
                    let fill = lit_or(b.input, ink.part);
                    let edge = lit_or(b.input, ink.edge);
                    let (ops, centre) = match &b.shape {
                        &Shape::Circle(x, y, r) => {
                            let (x, y) = turned.at(x, y);
                            (circle(x, y, r), Some((x, y, r)))
                        }
                        Shape::Path(ops) => (turned.ops(ops), None),
                    };
                    pen.shape(&ops, fill, edge);
                    let symbol = if b.input.is_some_and(on) { ink.on_lit } else { ink.label };
                    if let (Some(name), Some((x, y, r))) = (&b.icon, centre)
                        && let Some((ops, stroke)) = icon(name, x, y, r)
                    {
                        match stroke {
                            Some(w) => pen.stroke(&ops, (w * k).max(0.8), symbol),
                            None => pen.fill(&ops, symbol),
                        }
                    }
                    // PlayStation's symbols, drawn.
                    if let (Some(i), Some((x, y, r)), Glyphs::PlayStation) = (b.face, centre, glyphs) {
                        let c = if b.input.is_some_and(on) {
                            ink.on_lit
                        } else if g.tint {
                            rgb(PS_TINT[i]).into()
                        } else {
                            ink.label
                        };
                        let q = r / 10.5;
                        let sym = match i {
                            0 => {
                                let mut v = line(x - 3.6 * q, y - 3.6 * q, x + 3.6 * q, y + 3.6 * q);
                                v.extend(line(x + 3.6 * q, y - 3.6 * q, x - 3.6 * q, y + 3.6 * q));
                                v
                            }
                            1 => circle(x, y, 4. * q),
                            2 => rect(x - 3.6 * q, y - 3.6 * q, 7.2 * q, 7.2 * q, 0.6),
                            _ => poly(&[(x, y - 4.4 * q), (x + 4.4 * q, y + 3. * q), (x - 4.4 * q, y + 3. * q)]),
                        };
                        pen.stroke(&sym, (1.4 * k).max(1.), c);
                    }
                }
            }
        },
    )
    .absolute()
    .size_full();

    let mut root = div().relative().flex_none().w(px(W * k)).h(px(H * k)).child(painted);
    if glyphs != Glyphs::PlayStation {
        let letters = match glyphs {
            Glyphs::Nintendo => ["B", "A", "Y", "X"],
            Glyphs::Numbers => ["3", "2", "4", "1"],
            _ => ["A", "B", "X", "Y"],
        };
        for b in &g.buttons {
            let turned = if b.grip { turn } else { Turn::NONE };
            // A round button's letter fills it; a key of another shape gets its label in the
            // middle, smaller.
            let (x, y, r) = match &b.shape {
                &Shape::Circle(x, y, r) => {
                    let (x, y) = turned.at(x, y);
                    (x, y, r)
                }
                Shape::Path(ops) if b.label.is_some() => {
                    let (l, t0, r, bt) = extent(ops);
                    let (x, y) = turned.at((l + r) / 2., (t0 + bt) / 2.);
                    (x, y, (r - l).min(bt - t0) * 0.4)
                }
                Shape::Path(_) => continue,
            };
            let text = match (&b.label, b.face) {
                (Some(l), _) => l.as_str(),
                (None, Some(i)) => letters[i],
                (None, None) => continue,
            };
            let pressed = b.input.is_some_and(|x| held(&state, x));
            let color: Hsla = match b.face {
                _ if pressed => ink.on_lit,
                Some(i) if g.tint && matches!(glyphs, Glyphs::Xbox | Glyphs::Numbers) => rgb(XBOX_TINT[i]).into(),
                _ => ink.label,
            };
            root = root.child(
                div()
                    .absolute()
                    .left(px((x - r) * k))
                    .top(px((y - r) * k))
                    .size(px(2. * r * k))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(r * 1.05 * k))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(text.to_string()),
            );
        }
    }
    root.into_any_element()
}

/// Where a flight stick's grip is in this state, and whether it is moved or turned at all.
fn grip_turn(g: &Pad, state: &PadState) -> (Turn, bool) {
    let Some(st) = g.sticks.first().filter(|s| s.grip.is_some()) else { return (Turn::NONE, false) };
    let (mx, my) = (state.axes[axis::LEFT_X] as f32 / 32768., state.axes[axis::LEFT_Y] as f32 / 32768.);
    let twist = state.axes[axis::RIGHT_X] as f32 / 32768.;
    let turn = Turn::new(st.x, st.y, (mx * st.reach(), my * st.reach()), twist * st.twist);
    (turn, mx.hypot(my) > 0.2 || twist.abs() > 0.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_parse() {
        let ops = parse("M10 20L30 40H50V60C1 2 3 4 5 6Zm1 1l2 2z");
        assert_eq!(ops.len(), 9);
        assert!(matches!(ops[3], Op::L(50., 60.)));
        assert!(matches!(ops[6], Op::M(x, y) if x == 11. && y == 21.), "relative to the start of the closed path");
        assert!(matches!(parse("M1-2L-3.5.5")[1], Op::L(x, y) if x == -3.5 && y == 0.5));
    }

    #[test]
    fn darkflame_by_its_name() {
        use open_controller_core::PadKey;
        let mut p = crate::demo::pad(PadKey::Slot(0), "Redragon Darkflame G820", Brand::Xbox, Family::Xbox, (0x045E, 0x028E));
        assert_eq!(look(&p), Look::DarkflameG820);
        assert_eq!(glyphs(&p, Look::DarkflameG820), Glyphs::PlayStation, "it prints PlayStation's symbols");
        // Over Bluetooth it says it is a DualShock 4, and is drawn as one.
        p = crate::demo::pad(PadKey::Slot(0), "Wireless Controller", Brand::PlayStation, Family::DualShock4, (0x054C, 0x05C4));
        assert_eq!(look(&p), Look::DualShock4);
    }

    #[test]
    fn eightbitdo_3106_is_the_first_ultimate_unless_it_says_pro_2() {
        use open_controller_core::PadKey;
        let ids = (0x2DC8, 0x3106);
        let mut p = crate::demo::pad(PadKey::Slot(0), "Ultimate Wireless Controller", Brand::EightBitDo, Family::EightBitDoPro2, ids);
        assert_eq!(look(&p), Look::Ultimate);
        assert_eq!(glyphs(&p, Look::Ultimate), Glyphs::Xbox);
        p.profiles.apply(open_controller_core::profile::Edit::Model(Some("8bitdo-ultimate-bluetooth".into())));
        assert_eq!(look(&p), Look::Traced("ultimateBluetooth"));
        assert_eq!(glyphs(&p, look(&p)), Glyphs::Nintendo, "B at the bottom, as printed on it");
        p.profiles.apply(open_controller_core::profile::Edit::Model(None));
        p.name = "8BitDo Pro 2 Wired Controller".into();
        assert_eq!(look(&p), Look::Sn30Pro);
    }

    #[test]
    fn a_chosen_model_is_drawn_as_itself() {
        use open_controller_core::PadKey;
        use open_controller_core::profile::Edit;
        let mut p =
            crate::demo::pad(PadKey::Slot(0), "Nintendo Switch Pro Controller", Brand::Nintendo, Family::SwitchPro, (0x057E, 0x2009));
        assert_eq!(look(&p), Look::SwitchPro);
        p.profiles.apply(Edit::Model(Some("onikuma-c1".into())));
        assert_eq!(look(&p), Look::Traced("onikumaC1"));
        assert_eq!(glyphs(&p, look(&p)), Glyphs::Nintendo, "X on top, as printed on it");
    }

    #[test]
    fn a_flight_stick_is_drawn_as_one_and_its_grip_follows_the_stick() {
        use open_controller_core::PadKey;
        let mut p = crate::demo::pad(PadKey::Device(1), "T.16000M", Brand::Other, Family::Other, (0x044F, 0xB10A));
        assert_eq!(look(&p), Look::Generic);
        p.form = Form::FlightStick;
        assert_eq!(look(&p), Look::Traced(FLIGHT_STICK), "any flight stick, its own drawing or not");
        let g = pad_for(look(&p));
        let st = &g.sticks[0];
        let (rest, moved) = grip_turn(g, &PadState::default());
        assert!(!moved && rest.at(10., 20.) == (10., 20.));
        let mut state = PadState::default();
        state.axes[axis::RIGHT_X] = 32767;
        let (twisted, moved) = grip_turn(g, &state);
        let (x, y) = twisted.at(st.x, st.y);
        assert!(moved && (x - st.x).abs() < 1e-3 && (y - st.y).abs() < 1e-3, "it turns about its pivot");
        assert!(twisted.at(st.x, st.y - 100.).0 > st.x, "twisted right, its tip goes right");
        state.axes[axis::RIGHT_X] = 0;
        state.axes[axis::LEFT_Y] = -32768;
        assert!(grip_turn(g, &state).0.at(st.x, st.y).1 < st.y, "pushed forward, it leans up the picture");
        assert_eq!(g.levers.len(), 1, "the throttle");
    }

    #[test]
    fn every_look_is_drawn() {
        let all = pads();
        let traced = models::DRAWINGS.iter().map(|r| Look::Traced(r.2)).chain(models::ALIASES.iter().map(|a| Look::Traced(a.5)));
        for l in Look::ALL.into_iter().chain(traced) {
            let p = all.get(l.key()).unwrap_or_else(|| panic!("{l:?} has no drawing in pads.json"));
            assert!(p.body.len() > 4 && p.body.iter().any(|o| matches!(o, Op::Z)), "{l:?}: a closed outline");
            // A Joy-Con on its own has one stick, and its arrows are its face buttons.
            let single = matches!(l, Look::JoyConLeft | Look::JoyConRight);
            if matches!(l, Look::Traced(_)) {
                assert!(matches!(p.glyphs.as_deref(), Some("xbox" | "ps" | "nintendo" | "numbers")), "{l:?}: glyphs");
            } else {
                assert_eq!(
                    p.sticks.len(),
                    if l == Look::Retro {
                        0
                    } else if single {
                        1
                    } else {
                        2
                    },
                    "{l:?}: sticks"
                );
            }
            // A Genesis pad has three face buttons; every other look has all four.
            let faces: &[&str] = if l == Look::Traced("genesis3b") { &["south", "east"] } else { &["south", "east", "west", "north"] };
            let dpad: &[&str] = if single { &[] } else { &["up", "down", "left", "right"] };
            for &input in faces.iter().chain(dpad) {
                let i = super::input(input);
                assert!(p.buttons.iter().any(|b| b.input == i), "{l:?}: {input}");
            }
        }
    }
}
