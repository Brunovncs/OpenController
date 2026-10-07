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
use open_controller_core::device::Brand;
use open_controller_core::extras::{self, Art, Family};
use open_controller_core::mapping::{PadState, axis, button};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const W: f32 = 400.;
const H: f32 = 280.;
/// Callers size drawings as for a 320-wide one.
const UNIT: f32 = 320. / W;

static PADS: &str = include_str!("../../../assets/pads.json");

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
    Ultimate2,
    Ultimate2C,
    Ultimate,
    Sn30Pro,
    Retro,
    Handheld,
    Generic,
}

impl Look {
    #[cfg(test)]
    pub const ALL: [Look; 14] = [
        Look::DualSense,
        Look::DualSenseEdge,
        Look::DualShock4,
        Look::DualShock3,
        Look::Xbox,
        Look::SwitchPro,
        Look::JoyCons,
        Look::Ultimate2,
        Look::Ultimate2C,
        Look::Ultimate,
        Look::Sn30Pro,
        Look::Retro,
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
            Look::Ultimate2 => "ultimate2",
            Look::Ultimate2C => "ultimate2c",
            Look::Ultimate => "ultimate",
            Look::Sn30Pro => "sn30pro",
            Look::Retro => "retro",
            Look::Handheld => "handheld",
            Look::Generic => "generic",
        }
    }
}

/// How the face buttons are labelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphs {
    Xbox,
    PlayStation,
    Nintendo,
}

pub fn look(pad: &PadView) -> Look {
    use Family::*;
    let name = pad.name.to_ascii_lowercase();
    let ultimate = name.contains("ultimate");
    match pad.family {
        DualSenseEdge => Look::DualSenseEdge,
        DualSense => Look::DualSense,
        DualShock4 => Look::DualShock4,
        DualShock3 | Ps2Adapter => Look::DualShock3,
        Xbox | XboxElite => Look::Xbox,
        SwitchPro => Look::SwitchPro,
        JoyCons => Look::JoyCons,
        Switch2 if pad.name.contains("Joy-Con") => Look::JoyCons,
        Switch2 if pad.name.contains("Pro Controller") => Look::SwitchPro,
        EightBitDoUltimate2C => Look::Ultimate2C,
        EightBitDoUltimate => Look::Ultimate,
        EightBitDoFour | EightBitDoPro2 if pad.art == Art::Offset || ultimate => Look::Ultimate2,
        EightBitDoFour | EightBitDoPro2 => Look::Sn30Pro,
        EightBitDo if pad.art == Art::Retro => Look::Retro,
        EightBitDo if pad.art == Art::Symmetric => Look::Sn30Pro,
        EightBitDo if name.contains("ultimate 2c") => Look::Ultimate2C,
        EightBitDo if ultimate => Look::Ultimate,
        _ => match pad.art {
            Art::Retro => Look::Retro,
            Art::Handheld => Look::Handheld,
            Art::JoyCons => Look::JoyCons,
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

pub fn glyphs(pad: &PadView, look: Look) -> Glyphs {
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
}

#[derive(Deserialize)]
struct RawButton {
    input: String,
    circle: Option<[f32; 3]>,
    d: Option<String>,
    icon: Option<String>,
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
    sticks: Vec<RawStick>,
    #[serde(default)]
    buttons: Vec<RawButton>,
    #[serde(default)]
    tint: bool,
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

/// The highest point of a shape, for filling a trigger from the bottom up.
fn top(ops: &Ops) -> (f32, f32) {
    let ys = ops.iter().flat_map(|op| match *op {
        Op::M(_, y) | Op::L(_, y) => vec![y],
        Op::C(_, a, _, b, _, c) => vec![a, b, c],
        Op::Z => vec![],
    });
    ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)))
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
    input: Option<u32>,
    /// Which face button it is, south, east, west, north, for its label.
    face: Option<usize>,
    shape: Shape,
    icon: Option<String>,
}

struct Stick {
    x: f32,
    y: f32,
    cap: f32,
    well: f32,
    ring: Option<[f32; 2]>,
}

struct Pad {
    body: Ops,
    bumpers: Option<[Ops; 2]>,
    triggers: Option<[Ops; 2]>,
    panels: Vec<(String, Ops)>,
    touchpad: Option<Ops>,
    dots: Vec<[f32; 3]>,
    sticks: Vec<Stick>,
    buttons: Vec<Btn>,
    tint: bool,
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
                        face: ["south", "east", "west", "north"].iter().position(|f| *f == b.input),
                        input: input(&b.input),
                        shape: match (b.circle, &b.d) {
                            (Some([x, y, r]), _) => Shape::Circle(x, y, r),
                            (None, Some(d)) => Shape::Path(parse(d)),
                            (None, None) => Shape::Path(Vec::new()),
                        },
                        icon: b.icon,
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
                    sticks: r
                        .sticks
                        .into_iter()
                        .map(|s| {
                            let [x, y, cap] = s.circle;
                            Stick { x, y, cap, well: s.well.unwrap_or(cap * 1.18), ring: s.ring }
                        })
                        .collect(),
                    buttons,
                    tint: r.tint,
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

    let painted = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut pen = Pen { window, o: bounds.origin, k };
            let on = |b: u32| state.pressed(b);
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

            // Sticks: the light ring round them, the well, and the cap following the stick.
            for (i, st) in g.sticks.iter().enumerate().take(2) {
                let (ax, ay, click) = if i == 0 {
                    (axis::LEFT_X, axis::LEFT_Y, button::LEFT_STICK)
                } else {
                    (axis::RIGHT_X, axis::RIGHT_Y, button::RIGHT_STICK)
                };
                let (mx, my) = (state.axes[ax] as f32 / 32768., state.axes[ay] as f32 / 32768.);
                let moved = mx.hypot(my) > 0.2;
                let active = moved || on(click);
                if let Some([rr, w]) = st.ring {
                    pen.stroke(&circle(st.x, st.y, rr), (w * k).max(1.), if active { ink.lit.opacity(0.75) } else { ink.soft });
                }
                pen.shape(&circle(st.x, st.y, st.well), ink.shade, if active { ink.lit.opacity(0.6) } else { ink.soft });
                // The cap leans as far as the opening lets it.
                let reach = (st.well - st.cap).max(st.cap * 0.25) * 0.8;
                let (cx, cy) = (st.x + mx * reach, st.y + my * reach);
                pen.shape(&circle(cx, cy, st.cap), if on(click) { ink.lit } else { ink.body_top }, if active { ink.lit } else { ink.edge });
                pen.stroke(&circle(cx, cy, st.cap * 0.62), 1., if on(click) { ink.on_lit.opacity(0.3) } else { ink.soft });
            }

            // Buttons: their shape, and the symbol on the small ones.
            for b in &g.buttons {
                let fill = lit_or(b.input, ink.part);
                let edge = lit_or(b.input, ink.edge);
                let (ops, centre) = match &b.shape {
                    &Shape::Circle(x, y, r) => (circle(x, y, r), Some((x, y, r))),
                    Shape::Path(ops) => (ops.clone(), None),
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
        },
    )
    .absolute()
    .size_full();

    let mut root = div().relative().flex_none().w(px(W * k)).h(px(H * k)).child(painted);
    if glyphs != Glyphs::PlayStation {
        let letters = match glyphs {
            Glyphs::Nintendo => ["B", "A", "Y", "X"],
            _ => ["A", "B", "X", "Y"],
        };
        for b in &g.buttons {
            let (Some(i), Shape::Circle(x, y, r)) = (b.face, &b.shape) else { continue };
            let pressed = b.input.is_some_and(|x| state.pressed(x));
            let color: Hsla = if pressed {
                ink.on_lit
            } else if g.tint && glyphs == Glyphs::Xbox {
                rgb(XBOX_TINT[i]).into()
            } else {
                ink.label
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
                    .child(letters[i]),
            );
        }
    }
    root.into_any_element()
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
    fn every_look_is_drawn() {
        let all = pads();
        for l in Look::ALL {
            let p = all.get(l.key()).unwrap_or_else(|| panic!("{l:?} has no drawing in pads.json"));
            assert!(p.body.len() > 4 && p.body.iter().any(|o| matches!(o, Op::Z)), "{l:?}: a closed outline");
            assert_eq!(p.sticks.len(), if l == Look::Retro { 0 } else { 2 }, "{l:?}: sticks");
            for input in ["south", "east", "west", "north", "up", "down", "left", "right"] {
                let i = super::input(input);
                assert!(p.buttons.iter().any(|b| b.input == i), "{l:?}: {input}");
            }
        }
    }
}
