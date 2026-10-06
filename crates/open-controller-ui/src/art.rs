//! The controller, drawn. Each kind has its own outline and its controls where the real one has
//! them: a DualSense with its touchpad and light strips, an 8BitDo Ultimate with its star and the
//! light rings round its sticks, a Switch Pro with its offset sticks, and so on. These are the
//! website's drawings, on its 400 by 280 grid, painted as paths and lit as buttons are pressed.

use crate::theme::Theme;
use gpui::{
    AnyElement, Background, Bounds, ContentMask, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point, Styled, Window, canvas, div,
    hsla, linear_color_stop, linear_gradient, point, px, rgb, size,
};
use open_controller_core::PadView;
use open_controller_core::device::Brand;
use open_controller_core::extras::{Art, Family};
use open_controller_core::mapping::{PadState, axis, button};
use std::sync::OnceLock;

const W: f32 = 400.;
const H: f32 = 280.;
/// Callers size drawings as for a 320-wide one.
const UNIT: f32 = 320. / W;

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
    Ultimate,
    Sn30Pro,
    Retro,
    Handheld,
    Generic,
}

const LOOKS: usize = 12;

/// How the face buttons are labelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphs {
    Xbox,
    PlayStation,
    Nintendo,
}

pub fn look(pad: &PadView) -> Look {
    use Family::*;
    let ultimate = pad.name.to_ascii_lowercase().contains("ultimate");
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
        EightBitDoUltimate | EightBitDoUltimate2C => Look::Ultimate,
        EightBitDoFour | EightBitDoPro2 if pad.art == Art::Offset || ultimate => Look::Ultimate,
        EightBitDoFour | EightBitDoPro2 => Look::Sn30Pro,
        EightBitDo if pad.art == Art::Retro => Look::Retro,
        EightBitDo if pad.art == Art::Symmetric => Look::Sn30Pro,
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

// Geometry, in grid units.

#[derive(Clone, Copy)]
enum Op {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

type Ops = Vec<Op>;
type Seg = [f32; 6];

/// A symmetric outline from its right half, top centre to bottom centre.
fn sym(start: (f32, f32), segs: &[Seg]) -> Ops {
    let mut v = vec![Op::M(start.0, start.1)];
    let mut ends = vec![start];
    for s in segs {
        v.push(Op::C(s[0], s[1], s[2], s[3], s[4], s[5]));
        ends.push((s[4], s[5]));
    }
    let m = |x: f32| 2. * 200. - x;
    for (i, s) in segs.iter().enumerate().rev() {
        let p = ends[i];
        v.push(Op::C(m(s[2]), s[3], m(s[0]), s[1], m(p.0), p.1));
    }
    v.push(Op::Z);
    v
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

/// A rectangle with each corner rounded on its own: top left, top right, bottom right, bottom left.
fn rect_r(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Ops {
    let [tl, tr, br, bl] = r.map(|r| r.min(w / 2.).min(h / 2.));
    let (x2, y2) = (x + w, y + h);
    vec![
        Op::M(x + tl, y),
        Op::L(x2 - tr, y),
        Op::C(x2 - tr + tr * K, y, x2, y + tr - tr * K, x2, y + tr),
        Op::L(x2, y2 - br),
        Op::C(x2, y2 - br + br * K, x2 - br + br * K, y2, x2 - br, y2),
        Op::L(x + bl, y2),
        Op::C(x + bl - bl * K, y2, x, y2 - bl + bl * K, x, y2 - bl),
        Op::L(x, y + tl),
        Op::C(x, y + tl - tl * K, x + tl - tl * K, y, x + tl, y),
        Op::Z,
    ]
}

fn rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Ops {
    rect_r(x, y, w, h, [r; 4])
}

fn poly(points: &[(f32, f32)]) -> Ops {
    let mut v: Ops = points.iter().enumerate().map(|(i, &(x, y))| if i == 0 { Op::M(x, y) } else { Op::L(x, y) }).collect();
    v.push(Op::Z);
    v
}

fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> Ops {
    vec![Op::M(x1, y1), Op::L(x2, y2)]
}

fn map(ops: &Ops, f: impl Fn(f32, f32) -> (f32, f32)) -> Ops {
    ops.iter()
        .map(|&op| match op {
            Op::M(x, y) => {
                let (x, y) = f(x, y);
                Op::M(x, y)
            }
            Op::L(x, y) => {
                let (x, y) = f(x, y);
                Op::L(x, y)
            }
            Op::C(a, b, c, d, x, y) => {
                let ((a, b), (c, d), (x, y)) = (f(a, b), f(c, d), f(x, y));
                Op::C(a, b, c, d, x, y)
            }
            Op::Z => Op::Z,
        })
        .collect()
}

fn rotate(ops: &Ops, cx: f32, cy: f32, deg: f32) -> Ops {
    let (s, c) = deg.to_radians().sin_cos();
    map(ops, |x, y| (cx + (x - cx) * c - (y - cy) * s, cy + (x - cx) * s + (y - cy) * c))
}

/// The outline as points, curves sampled, for finding its top edge.
fn sample(ops: &Ops) -> Vec<Vec<(f32, f32)>> {
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let (mut at, mut start) = ((0., 0.), (0., 0.));
    for &op in ops {
        match op {
            Op::M(x, y) => {
                out.push(vec![(x, y)]);
                at = (x, y);
                start = at;
            }
            Op::L(x, y) => {
                if let Some(p) = out.last_mut() {
                    p.push((x, y));
                }
                at = (x, y);
            }
            Op::C(a, b, c, d, x, y) => {
                if let Some(p) = out.last_mut() {
                    for i in 1..=24 {
                        let t = i as f32 / 24.;
                        let u = 1. - t;
                        let px = u * u * u * at.0 + 3. * u * u * t * a + 3. * u * t * t * c + t * t * t * x;
                        let py = u * u * u * at.1 + 3. * u * u * t * b + 3. * u * t * t * d + t * t * t * y;
                        p.push((px, py));
                    }
                }
                at = (x, y);
            }
            Op::Z => {
                if let Some(p) = out.last_mut() {
                    p.push(start);
                }
                at = start;
            }
        }
    }
    out
}

/// Where the outline is highest at `x`.
fn top_at(outline: &[Vec<(f32, f32)>], x: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for path in outline {
        for w in path.windows(2) {
            let ((x1, y1), (x2, y2)) = (w[0], w[1]);
            if (x1 <= x && x <= x2) || (x2 <= x && x <= x1) {
                let y = if (x2 - x1).abs() < 1e-4 { y1.min(y2) } else { y1 + (y2 - y1) * (x - x1) / (x2 - x1) };
                best = Some(best.map_or(y, |b: f32| b.min(y)));
            }
        }
    }
    best
}

/// A shoulder button: the body's top edge between `cx - rx` and `cx + rx`, raised by `up` in the
/// middle and less towards the ends, so it peeks out behind the body with rounded ends.
fn shoulder(outline: &[Vec<(f32, f32)>], cx: f32, rx: f32, up: f32) -> Ops {
    let n = 28;
    let xs: Vec<f32> = (0..=n).map(|i| cx - rx + 2. * rx * i as f32 / n as f32).collect();
    let mut top = Vec::new();
    let mut bottom = Vec::new();
    for &x in &xs {
        let Some(y) = top_at(outline, x) else { continue };
        let t = ((x - cx) / rx).abs().min(1.);
        let lift = up * (1. - t * t).max(0.).powf(0.3);
        top.push((x, y - lift));
        bottom.push((x, y + 6.));
    }
    bottom.reverse();
    top.extend(bottom);
    poly(&top)
}

#[derive(Clone, Copy)]
enum Dpad {
    Cross,
    Split,
    Buttons,
}

#[derive(Clone, Copy)]
enum Kind {
    Round,
    Square,
    Start,
    Plus,
    Pill,
}

#[derive(Clone, Copy)]
struct Btn {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rot: f32,
    kind: Kind,
}

const fn btn(x: f32, y: f32, w: f32, h: f32, kind: Kind) -> Btn {
    Btn { x, y, w, h, rot: 0., kind }
}

const fn tilted(x: f32, y: f32, w: f32, h: f32, rot: f32) -> Btn {
    Btn { x, y, w, h, rot, kind: Kind::Pill }
}

struct Spec {
    body: Ops,
    inner: Option<Ops>,
    wells: &'static [(f32, f32, f32)],
    /// Shoulder button: centre x, half width, how far it rises.
    lb: (f32, f32, f32),
    lt: Option<(f32, f32, f32)>,
    ls: Option<(f32, f32, f32)>,
    rs: Option<(f32, f32, f32)>,
    dpad: (f32, f32, f32, Dpad),
    face: (f32, f32, f32, f32),
    tint: bool,
    select: Option<Btn>,
    start: Option<Btn>,
    home: Option<Btn>,
    capture: Option<Btn>,
    touchpad: Option<Ops>,
    stick_rings: bool,
}

/// What does not light up: speaker grilles, light strips, logos, screens.
struct Deco {
    parts: Vec<Ops>,
    dark: Vec<Ops>,
    lines: Vec<(Ops, f32)>,
    glow: Vec<(Ops, f32)>,
    dots: Vec<(f32, f32)>,
}

struct Geometry {
    spec: Spec,
    deco: Deco,
    lb: [Ops; 2],
    lt: Option<[Ops; 2]>,
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

fn dots(x: f32, y: f32, n: usize, step: f32) -> Vec<(f32, f32)> {
    let start = x - (n as f32 - 1.) * step / 2.;
    (0..n).map(|i| (start + i as f32 * step, y)).collect()
}

fn spec(look: Look) -> (Spec, Deco) {
    let mut d = Deco { parts: vec![], dark: vec![], lines: vec![], glow: vec![], dots: vec![] };
    let base = Spec {
        body: vec![],
        inner: None,
        wells: &[],
        lb: (100., 74., 7.),
        lt: Some((116., 50., 17.)),
        ls: None,
        rs: None,
        dpad: (0., 0., 0., Dpad::Cross),
        face: (0., 0., 0., 0.),
        tint: false,
        select: None,
        start: None,
        home: None,
        capture: None,
        touchpad: None,
        stick_rings: false,
    };
    let s = match look {
        Look::Xbox => {
            d.parts.push(rect(194., 137., 12., 6., 3.));
            d.lines.push((circle(200., 86., 6.), 1.));
            Spec {
                body: sym(
                    (200., 70.),
                    &[
                        [236., 70., 262., 58., 300., 58.],
                        [334., 58., 356., 74., 366., 100.],
                        [378., 132., 386., 190., 380., 226.],
                        [376., 252., 352., 266., 330., 258.],
                        [310., 250., 296., 226., 282., 210.],
                        [266., 194., 236., 190., 200., 190.],
                    ],
                ),
                wells: &[(160., 170., 22.)],
                ls: Some((118., 112., 23.)),
                rs: Some((240., 170., 23.)),
                dpad: (160., 170., 17., Dpad::Cross),
                face: (282., 112., 20., 10.5),
                tint: true,
                select: Some(btn(172., 118., 11., 11., Kind::Round)),
                start: Some(btn(228., 118., 11., 11., Kind::Round)),
                home: Some(btn(200., 86., 21., 21., Kind::Round)),
                ..base
            }
        }
        Look::DualSense | Look::DualSenseEdge => {
            d.glow.push((line(139., 62., 147., 114.), 2.2));
            d.glow.push((line(261., 62., 253., 114.), 2.2));
            d.dots = dots(200., 128., 5, 6.);
            d.parts.push(rect(193., 166., 14., 5., 2.5));
            if look == Look::DualSenseEdge {
                d.parts.push(rect(144., 199., 12., 4.5, 2.25));
                d.parts.push(rect(244., 199., 12., 4.5, 2.25));
            }
            Spec {
                body: sym(
                    (200., 54.),
                    &[
                        [244., 54., 292., 50., 320., 52.],
                        [352., 54., 372., 74., 378., 106.],
                        [386., 148., 392., 208., 382., 240.],
                        [374., 266., 336., 272., 318., 254.],
                        [306., 242., 298., 222., 286., 210.],
                        [268., 196., 236., 207., 200., 207.],
                    ],
                ),
                inner: Some(sym(
                    (200., 70.),
                    &[
                        [250., 70., 300., 64., 328., 70.],
                        [360., 78., 366., 126., 342., 148.],
                        [324., 164., 302., 178., 292., 198.],
                        [282., 218., 250., 228., 200., 228.],
                    ],
                )),
                lb: (104., 74., 7.),
                lt: Some((110., 50., 16.)),
                ls: Some((150., 172., 21.)),
                rs: Some((250., 172., 21.)),
                dpad: (98., 114., 19., Dpad::Split),
                face: (302., 114., 21., 10.5),
                select: Some(tilted(130., 70., 5.5, 13., -12.)),
                start: Some(tilted(270., 70., 5.5, 13., 12.)),
                home: Some(btn(200., 150., 14., 14., Kind::Round)),
                touchpad: Some(vec![
                    Op::M(146., 54.),
                    Op::L(254., 54.),
                    Op::C(257., 54., 258., 56., 258., 59.),
                    Op::L(250., 112.),
                    Op::C(249., 116., 246., 118., 242., 118.),
                    Op::L(158., 118.),
                    Op::C(154., 118., 151., 116., 150., 112.),
                    Op::L(142., 59.),
                    Op::C(142., 56., 143., 54., 146., 54.),
                    Op::Z,
                ]),
                ..base
            }
        }
        Look::DualShock4 => {
            d.glow.push((line(158., 63.5, 242., 63.5), 2.));
            d.dots = dots(200., 128., 5, 5.);
            Spec {
                body: sym(
                    (200., 60.),
                    &[
                        [250., 60., 300., 56., 326., 58.],
                        [352., 60., 366., 74., 370., 96.],
                        [378., 138., 382., 196., 374., 234.],
                        [368., 262., 334., 270., 322., 246.],
                        [314., 226., 302., 202., 284., 190.],
                        [262., 178., 230., 182., 200., 184.],
                    ],
                ),
                wells: &[(104., 110., 34.), (296., 110., 34.)],
                lb: (104., 72., 7.),
                lt: Some((110., 48., 16.)),
                ls: Some((154., 158., 21.)),
                rs: Some((246., 158., 21.)),
                dpad: (104., 110., 18., Dpad::Split),
                face: (296., 110., 20., 10.),
                tint: true,
                select: Some(btn(138., 72., 5., 12., Kind::Pill)),
                start: Some(btn(262., 72., 5., 12., Kind::Pill)),
                home: Some(btn(200., 152., 13., 13., Kind::Round)),
                touchpad: Some(rect(150., 60., 100., 54., 7.)),
                ..base
            }
        }
        Look::DualShock3 => {
            d.dots = dots(200., 78., 4, 8.);
            Spec {
                body: sym(
                    (200., 70.),
                    &[
                        [252., 70., 302., 64., 330., 66.],
                        [358., 68., 374., 86., 374., 110.],
                        [376., 146., 376., 196., 366., 232.],
                        [358., 260., 326., 262., 320., 238.],
                        [312., 212., 298., 190., 278., 180.],
                        [254., 170., 228., 176., 200., 176.],
                    ],
                ),
                wells: &[(100., 112., 32.), (300., 112., 32.)],
                lb: (98., 72., 7.),
                lt: Some((108., 48., 16.)),
                ls: Some((156., 148., 20.)),
                rs: Some((244., 148., 20.)),
                dpad: (100., 112., 18., Dpad::Split),
                face: (300., 112., 20., 10.),
                tint: true,
                select: Some(btn(182., 108., 12., 6., Kind::Square)),
                start: Some(btn(218., 108., 11., 8., Kind::Start)),
                home: Some(btn(200., 128., 14., 14., Kind::Round)),
                ..base
            }
        }
        Look::SwitchPro => {
            d.dots = dots(200., 67., 4, 6.);
            Spec {
                body: sym(
                    (200., 62.),
                    &[
                        [254., 62., 304., 57., 334., 60.],
                        [364., 64., 382., 88., 386., 120.],
                        [392., 162., 384., 208., 364., 232.],
                        [346., 254., 312., 250., 302., 230.],
                        [294., 214., 284., 202., 262., 200.],
                        [242., 198., 222., 198., 200., 198.],
                    ],
                ),
                wells: &[(164., 162., 22.)],
                lb: (100., 76., 7.),
                lt: Some((112., 50., 16.)),
                ls: Some((126., 108., 23.)),
                rs: Some((246., 162., 23.)),
                dpad: (164., 162., 17., Dpad::Cross),
                face: (282., 108., 20., 10.5),
                select: Some(btn(170., 82., 12., 4., Kind::Pill)),
                start: Some(btn(230., 82., 12., 12., Kind::Plus)),
                home: Some(btn(220., 106., 14., 14., Kind::Round)),
                capture: Some(btn(180., 106., 10., 10., Kind::Square)),
                ..base
            }
        }
        Look::JoyCons => {
            d.lines.push((line(183., 46., 183., 250.), 4.));
            d.lines.push((line(217., 46., 217., 250.), 4.));
            let mut body = rect_r(106., 40., 80., 216., [40., 0., 0., 40.]);
            body.extend(rect_r(214., 40., 80., 216., [0., 40., 40., 0.]));
            Spec {
                body,
                lb: (132., 40., 8.),
                lt: Some((140., 38., 17.)),
                ls: Some((146., 96., 20.)),
                rs: Some((254., 162., 20.)),
                dpad: (146., 162., 17., Dpad::Buttons),
                face: (254., 96., 17., 8.5),
                select: Some(btn(170., 57., 10., 4., Kind::Pill)),
                start: Some(btn(230., 57., 10., 10., Kind::Plus)),
                home: Some(btn(236., 220., 13., 13., Kind::Round)),
                capture: Some(btn(164., 220., 9., 9., Kind::Square)),
                ..base
            }
        }
        Look::Ultimate => {
            d.lines.push((circle(200., 90., 12.5), 1.));
            d.parts.push(star(200., 132., 5.5));
            Spec {
                body: sym(
                    (200., 64.),
                    &[
                        [244., 64., 284., 52., 318., 56.],
                        [352., 60., 372., 82., 378., 114.],
                        [386., 158., 384., 212., 366., 238.],
                        [348., 260., 314., 258., 302., 236.],
                        [292., 218., 280., 200., 260., 196.],
                        [240., 192., 222., 192., 200., 192.],
                    ],
                ),
                wells: &[(162., 164., 21.)],
                lb: (102., 74., 7.),
                ls: Some((124., 110., 22.)),
                rs: Some((238., 164., 22.)),
                dpad: (162., 164., 16., Dpad::Cross),
                face: (280., 110., 19., 10.),
                select: Some(btn(176., 112., 11., 5., Kind::Pill)),
                start: Some(btn(224., 112., 11., 5., Kind::Pill)),
                home: Some(btn(200., 90., 17., 17., Kind::Round)),
                stick_rings: true,
                ..base
            }
        }
        Look::Sn30Pro => {
            d.parts.push(star(200., 98., 5.));
            Spec {
                body: sym(
                    (200., 70.),
                    &[
                        [256., 70., 296., 64., 316., 64.],
                        [352., 64., 378., 92., 378., 126.],
                        [378., 164., 372., 208., 356., 234.],
                        [342., 256., 312., 256., 304., 234.],
                        [298., 216., 292., 208., 276., 206.],
                        [254., 204., 228., 210., 200., 210.],
                    ],
                ),
                wells: &[(90., 122., 34.), (310., 122., 38.)],
                lb: (88., 74., 7.),
                lt: Some((100., 48., 16.)),
                ls: Some((160., 174., 19.)),
                rs: Some((240., 174., 19.)),
                dpad: (90., 122., 18., Dpad::Cross),
                face: (310., 122., 19., 10.),
                select: Some(tilted(181., 120., 13., 5., -28.)),
                start: Some(tilted(219., 120., 13., 5., -28.)),
                home: Some(btn(200., 142., 11., 11., Kind::Round)),
                ..base
            }
        }
        Look::Retro => {
            d.dark.push(rotate(&ellipse(304., 144., 52., 40.), 304., 144., -24.));
            d.dark.push(rotate(&rect(168., 140., 64., 24., 12.), 200., 152., -30.));
            Spec {
                body: rect(30., 78., 340., 132., 66.),
                wells: &[(96., 144., 38.)],
                lb: (84., 62., 7.),
                lt: None,
                dpad: (96., 144., 20., Dpad::Cross),
                face: (304., 144., 21., 11.),
                select: Some(tilted(183., 152., 16., 6., -30.)),
                start: Some(tilted(217., 152., 16., 6., -30.)),
                ..base
            }
        }
        Look::Handheld => {
            d.dark.push(rect(106., 86., 188., 128., 8.));
            Spec {
                body: rect(20., 74., 360., 152., 42.),
                lb: (70., 56., 6.),
                lt: Some((80., 44., 14.)),
                ls: Some((62., 112., 17.)),
                rs: Some((338., 182., 17.)),
                dpad: (62., 180., 15., Dpad::Cross),
                face: (338., 114., 15., 7.5),
                select: Some(btn(92., 86., 8., 4., Kind::Pill)),
                start: Some(btn(308., 86., 8., 4., Kind::Pill)),
                ..base
            }
        }
        Look::Generic => Spec {
            body: sym(
                (200., 64.),
                &[
                    [250., 64., 290., 60., 312., 62.],
                    [346., 66., 362., 86., 370., 118.],
                    [380., 160., 384., 214., 368., 236.],
                    [352., 256., 322., 254., 308., 234.],
                    [296., 214., 284., 200., 262., 198.],
                    [240., 196., 220., 196., 200., 196.],
                ],
            ),
            lb: (100., 72., 7.),
            ls: Some((122., 112., 22.)),
            rs: Some((240., 166., 22.)),
            dpad: (160., 166., 16., Dpad::Cross),
            face: (280., 112., 19., 10.),
            select: Some(btn(178., 112., 12., 6., Kind::Pill)),
            start: Some(btn(222., 112., 12., 6., Kind::Pill)),
            home: Some(btn(200., 90., 15., 15., Kind::Round)),
            ..base
        },
    };
    (s, d)
}

fn index(look: Look) -> usize {
    look as usize
}

/// Each look's shapes, worked out once.
fn geometry(look: Look) -> &'static Geometry {
    static ALL: OnceLock<Vec<Geometry>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        let looks = [
            Look::DualSense,
            Look::DualSenseEdge,
            Look::DualShock4,
            Look::DualShock3,
            Look::Xbox,
            Look::SwitchPro,
            Look::JoyCons,
            Look::Ultimate,
            Look::Sn30Pro,
            Look::Retro,
            Look::Handheld,
            Look::Generic,
        ];
        debug_assert_eq!(looks.len(), LOOKS);
        looks
            .iter()
            .map(|&l| {
                let (spec, deco) = spec(l);
                let outline = sample(&spec.body);
                let pair = |(x, rx, up): (f32, f32, f32)| [shoulder(&outline, x, rx, up), shoulder(&outline, W - x, rx, up)];
                Geometry { lb: pair(spec.lb), lt: spec.lt.map(pair), spec, deco }
            })
            .collect()
    });
    &all[index(look)]
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

fn small(b: &Btn) -> Ops {
    let (x, y, w, h) = (b.x, b.y, b.w, b.h);
    let ops = match b.kind {
        Kind::Round => circle(x, y, w / 2.),
        Kind::Square => rect(x - w / 2., y - h / 2., w, h, 2.),
        Kind::Start => poly(&[(x - w / 2., y - h / 2.), (x + w / 2., y), (x - w / 2., y + h / 2.)]),
        Kind::Plus => {
            let (a, t) = (w / 2., w * 0.17);
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
        Kind::Pill => rect(x - w / 2., y - h / 2., w, h, w.min(h) / 2.),
    };
    if b.rot != 0. { rotate(&ops, x, y, b.rot) } else { ops }
}

/// The drawing of a controller in its current state, `scale` times the size of a 320-wide one.
pub fn controller(pad: &PadView, scale: f32, t: &Theme) -> AnyElement {
    let look = look(pad);
    let glyphs = glyphs(pad, look);
    let state: PadState = pad.input;
    let k = scale * UNIT;
    let ink = ink(t);
    let g = geometry(look);
    let s = &g.spec;
    let (fx, fy, gap, fr) = s.face;
    // South, east, west, north.
    let faces = [(button::SOUTH, fx, fy + gap), (button::EAST, fx + gap, fy), (button::WEST, fx - gap, fy), (button::NORTH, fx, fy - gap)];

    let painted = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut pen = Pen { window, o: bounds.origin, k };
            let on = |b: u32| state.pressed(b);
            let lit_or = |b: u32, idle: Hsla| if on(b) { ink.lit } else { idle };
            let value = |a: usize| (state.axes[a].max(0) as f32 / 32767.).clamp(0., 1.);

            // Shoulders behind the body: triggers, then bumpers.
            if let Some(lt) = &g.lt {
                for (ops, a) in [(&lt[0], axis::LEFT_TRIGGER), (&lt[1], axis::RIGHT_TRIGGER)] {
                    let v = value(a);
                    pen.shape(ops, ink.part, if v > 0.05 { ink.lit } else { ink.edge });
                    if v > 0.02 {
                        let (_, _, up) = s.lt.unwrap_or_default();
                        let top = sample(ops).iter().flatten().fold(f32::MAX, |m, p| m.min(p.1));
                        let reach = top + up * (1. - v);
                        pen.band(reach, H, |pen| pen.fill(ops, ink.lit.opacity(0.9)));
                    }
                }
            }
            for (ops, b) in [(&g.lb[0], button::LEFT_SHOULDER), (&g.lb[1], button::RIGHT_SHOULDER)] {
                pen.shape(ops, lit_or(b, ink.part), lit_or(b, ink.edge));
            }

            // The body.
            pen.fill(&s.body, linear_gradient(180., linear_color_stop(ink.body_top, 0.), linear_color_stop(ink.body_bottom, 1.)));
            pen.stroke(&s.body, 1., ink.edge);
            if let Some(inner) = &s.inner {
                pen.shape(inner, ink.shade, ink.soft);
            }
            for &(x, y, r) in s.wells {
                pen.shape(&circle(x, y, r), ink.shade, ink.soft);
            }
            for ops in &g.deco.dark {
                pen.shape(ops, ink.shade, ink.soft);
            }
            for ops in &g.deco.parts {
                pen.shape(ops, ink.part, ink.edge);
            }
            for (ops, w) in &g.deco.lines {
                pen.stroke(ops, *w * k.max(0.5), ink.soft);
            }
            for (ops, w) in &g.deco.glow {
                pen.stroke(ops, *w * k.max(0.5), ink.lit.opacity(0.55));
            }
            for &(x, y) in &g.deco.dots {
                pen.fill(&circle(x, y, 1.2), ink.soft);
            }
            if let Some(tp) = &s.touchpad {
                pen.shape(tp, if on(button::TOUCHPAD) { ink.lit } else { ink.part }, lit_or(button::TOUCHPAD, ink.edge));
            }

            // Sticks.
            for (st, ax, ay, click) in
                [(s.ls, axis::LEFT_X, axis::LEFT_Y, button::LEFT_STICK), (s.rs, axis::RIGHT_X, axis::RIGHT_Y, button::RIGHT_STICK)]
            {
                let Some((x, y, r)) = st else { continue };
                let (mx, my) = (state.axes[ax] as f32 / 32768., state.axes[ay] as f32 / 32768.);
                let moved = mx.hypot(my) > 0.2;
                let active = moved || on(click);
                if s.stick_rings {
                    pen.stroke(&circle(x, y, r + 4.), 1.6, if active { ink.lit.opacity(0.75) } else { ink.soft });
                }
                pen.shape(&circle(x, y, r), ink.shade, if active { ink.lit.opacity(0.6) } else { ink.soft });
                let (cx, cy) = (x + mx * r * 0.34, y + my * r * 0.34);
                pen.shape(
                    &circle(cx, cy, r * 0.7),
                    if on(click) { ink.lit } else { ink.body_top },
                    if active { ink.lit } else { ink.edge },
                );
                pen.stroke(&circle(cx, cy, r * 0.43), 1., if on(click) { ink.on_lit.opacity(0.3) } else { ink.soft });
            }

            // The d-pad.
            let (dx, dy, ds, kind) = s.dpad;
            let arms = [(button::DPAD_UP, 0.), (button::DPAD_DOWN, 180.), (button::DPAD_LEFT, 270.), (button::DPAD_RIGHT, 90.)];
            match kind {
                Dpad::Buttons => {
                    for (b, (ox, oy)) in arms.iter().map(|a| a.0).zip([(0., -ds), (0., ds), (-ds, 0.), (ds, 0.)]) {
                        pen.shape(&circle(dx + ox, dy + oy, ds * 0.5), lit_or(b, ink.part), lit_or(b, ink.edge));
                    }
                }
                Dpad::Split => {
                    let (w, g) = (ds * 0.62, ds * 0.22);
                    let arrow = poly(&[
                        (dx - w / 2., dy - ds),
                        (dx + w / 2., dy - ds),
                        (dx + w / 2., dy - g - w * 0.42),
                        (dx, dy - g),
                        (dx - w / 2., dy - g - w * 0.42),
                    ]);
                    for (b, a) in arms {
                        pen.shape(&rotate(&arrow, dx, dy, a), lit_or(b, ink.part), lit_or(b, ink.edge));
                    }
                }
                Dpad::Cross => {
                    let h = ds * 0.34;
                    let plus = poly(&[
                        (dx - h, dy - ds),
                        (dx + h, dy - ds),
                        (dx + h, dy - h),
                        (dx + ds, dy - h),
                        (dx + ds, dy + h),
                        (dx + h, dy + h),
                        (dx + h, dy + ds),
                        (dx - h, dy + ds),
                        (dx - h, dy + h),
                        (dx - ds, dy + h),
                        (dx - ds, dy - h),
                        (dx - h, dy - h),
                    ]);
                    pen.shape(&plus, ink.part, ink.edge);
                    for (b, a) in arms {
                        if on(b) {
                            pen.fill(&rotate(&rect(dx - h + 1., dy - ds + 1., 2. * h - 2., ds - h, 1.5), dx, dy, a), ink.lit);
                        }
                    }
                }
            }

            // Face buttons, and PlayStation's symbols, drawn.
            for (i, &(b, x, y)) in faces.iter().enumerate() {
                pen.shape(&circle(x, y, fr), lit_or(b, ink.part), lit_or(b, ink.edge));
                if glyphs == Glyphs::PlayStation {
                    let c = if on(b) {
                        ink.on_lit
                    } else if s.tint {
                        rgb(PS_TINT[i]).into()
                    } else {
                        ink.label
                    };
                    let q = fr / 10.5;
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

            for (b, idx) in [(s.select, button::BACK), (s.start, button::START), (s.home, button::GUIDE), (s.capture, button::MISC1)] {
                if let Some(b) = b {
                    pen.shape(&small(&b), lit_or(idx, ink.part), lit_or(idx, ink.edge));
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
        for (i, &(b, x, y)) in faces.iter().enumerate() {
            let color: Hsla = if state.pressed(b) {
                ink.on_lit
            } else if s.tint && glyphs == Glyphs::Xbox {
                rgb(XBOX_TINT[i]).into()
            } else {
                ink.label
            };
            root = root.child(
                div()
                    .absolute()
                    .left(px((x - fr) * k))
                    .top(px((y - fr) * k))
                    .size(px(2. * fr * k))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(fr * 1.05 * k))
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
    fn every_look_has_shoulders_on_its_outline() {
        for l in [
            Look::DualSense,
            Look::DualSenseEdge,
            Look::DualShock4,
            Look::DualShock3,
            Look::Xbox,
            Look::SwitchPro,
            Look::JoyCons,
            Look::Ultimate,
            Look::Sn30Pro,
            Look::Retro,
            Look::Handheld,
            Look::Generic,
        ] {
            let g = geometry(l);
            assert_eq!(index(l), l as usize);
            for s in &g.lb {
                assert!(sample(s)[0].len() > 20, "{l:?}: a bumper needs the body's top edge");
            }
        }
    }
}
