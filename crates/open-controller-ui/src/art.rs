//! The controller, drawn: its body in one of six shapes, with sticks, triggers and buttons lit as
//! they are pressed. Face buttons carry the controller's own labels. Everything is laid out on a
//! 320 by 214 grid and drawn at any scale, so the same drawing serves the home tiles and the
//! controller's page.

use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Div, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point, Styled, canvas, div, linear_color_stop, linear_gradient,
    point, px,
};
use open_controller_core::device::Brand;
use open_controller_core::extras::Art;
use open_controller_core::mapping::{PadState, axis, button};

pub const WIDTH: f32 = 320.;
pub const HEIGHT: f32 = 214.;
/// Room above the body for the triggers.
const TOP: f32 = 14.;

/// The body of an Xbox-style pad, a cubic Bézier per stretch, clockwise from the top left.
const OFFSET_BODY: (f32, f32, [[f32; 6]; 10]) = (
    92.,
    34.,
    [
        [130., 28., 190., 28., 228., 34.],
        [262., 38., 284., 52., 296., 86.],
        [308., 122., 314., 160., 300., 182.],
        [290., 197., 262., 196., 250., 180.],
        [240., 168., 232., 154., 214., 150.],
        [192., 146., 128., 146., 106., 150.],
        [88., 154., 80., 168., 70., 180.],
        [58., 196., 30., 197., 20., 182.],
        [6., 160., 12., 122., 24., 86.],
        [36., 52., 58., 38., 92., 34.],
    ],
);

/// A PlayStation-style body: a flatter top and longer, straighter grips.
const PLAYSTATION_BODY: (f32, f32, [[f32; 6]; 10]) = (
    80.,
    30.,
    [
        [130., 26., 190., 26., 240., 30.],
        [275., 32., 296., 44., 302., 70.],
        [312., 110., 312., 160., 296., 190.],
        [284., 208., 252., 206., 244., 188.],
        [236., 170., 228., 150., 210., 146.],
        [190., 142., 130., 142., 110., 146.],
        [92., 150., 84., 170., 76., 188.],
        [68., 206., 36., 208., 24., 190.],
        [8., 160., 8., 110., 18., 70.],
        [24., 44., 45., 32., 80., 30.],
    ],
);

struct Layout {
    left_stick: Option<(f32, f32)>,
    right_stick: Option<(f32, f32)>,
    dpad: (f32, f32),
    face: (f32, f32),
    view: (f32, f32),
    menu: (f32, f32),
    guide: Option<(f32, f32)>,
    touchpad: Option<(f32, f32, f32, f32)>,
    shoulders: (f32, f32),
}

fn layout(art: Art) -> Layout {
    match art {
        Art::Offset => Layout {
            left_stick: Some((88., 82.)),
            right_stick: Some((196., 118.)),
            dpad: (124., 118.),
            face: (232., 82.),
            view: (140., 82.),
            menu: (180., 82.),
            guide: Some((160., 58.)),
            touchpad: None,
            shoulders: (84., 236.),
        },
        Art::PlayStation => Layout {
            left_stick: Some((122., 122.)),
            right_stick: Some((198., 122.)),
            dpad: (82., 80.),
            face: (238., 80.),
            view: (104., 50.),
            menu: (216., 50.),
            guide: Some((160., 114.)),
            touchpad: Some((160., 64., 72., 38.)),
            shoulders: (80., 240.),
        },
        Art::Symmetric => Layout {
            left_stick: Some((122., 122.)),
            right_stick: Some((198., 122.)),
            dpad: (82., 80.),
            face: (238., 80.),
            view: (140., 78.),
            menu: (180., 78.),
            guide: Some((160., 100.)),
            touchpad: None,
            shoulders: (80., 240.),
        },
        Art::Retro => Layout {
            left_stick: None,
            right_stick: None,
            dpad: (84., 106.),
            face: (236., 106.),
            view: (142., 118.),
            menu: (178., 118.),
            guide: None,
            touchpad: None,
            shoulders: (84., 236.),
        },
        Art::JoyCons => Layout {
            left_stick: Some((86., 72.)),
            right_stick: Some((234., 140.)),
            dpad: (86., 140.),
            face: (234., 72.),
            view: (114., 38.),
            menu: (206., 38.),
            guide: Some((214., 180.)),
            touchpad: None,
            shoulders: (86., 234.),
        },
        Art::Handheld => Layout {
            left_stick: Some((46., 70.)),
            right_stick: Some((274., 114.)),
            dpad: (46., 114.),
            face: (274., 70.),
            view: (78., 48.),
            menu: (242., 48.),
            guide: None,
            touchpad: Some((46., 160., 40., 34.)),
            shoulders: (52., 268.),
        },
    }
}

/// Labels north, east, south, west.
fn face_labels(brand: Brand) -> [&'static str; 4] {
    match brand {
        Brand::PlayStation => ["△", "○", "✕", "□"],
        Brand::Nintendo => ["X", "A", "B", "Y"],
        _ => ["Y", "B", "A", "X"],
    }
}

/// The colours of the drawing: a body a step above the surface it sits on, darker wells for the
/// controls, and the accent for whatever is pressed.
struct Ink {
    body_top: Hsla,
    body_bottom: Hsla,
    edge: Hsla,
    well: Hsla,
    lit: Hsla,
    on_lit: Hsla,
    label: Hsla,
}

fn ink(t: &Theme) -> Ink {
    Ink {
        body_top: t.art_body_top,
        body_bottom: t.art_body_bottom,
        edge: t.stroke_strong,
        well: t.art_well,
        lit: t.accent,
        on_lit: t.on_accent,
        label: t.text2,
    }
}

fn curved_body(shape: (f32, f32, [[f32; 6]; 10]), s: f32, ink: &Ink) -> impl IntoElement {
    let (top, bottom, edge) = (ink.body_top, ink.body_bottom, ink.edge);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let o = bounds.origin;
            let at = |x: f32, y: f32| -> Point<Pixels> { o + point(px(x * s), px((y + TOP) * s)) };
            let (x0, y0, segments) = shape;
            for stroke in [false, true] {
                let mut b = if stroke { PathBuilder::stroke(px(1.)) } else { PathBuilder::fill() };
                b.move_to(at(x0, y0));
                for [ax, ay, bx, by, x, y] in segments {
                    b.cubic_bezier_to(at(x, y), at(ax, ay), at(bx, by));
                }
                b.close();
                if let Ok(path) = b.build() {
                    if stroke {
                        window.paint_path(path, edge);
                    } else {
                        window.paint_path(path, linear_gradient(180., linear_color_stop(top, 0.), linear_color_stop(bottom, 1.)));
                    }
                }
            }
        },
    )
    .absolute()
    .size_full()
}

/// The drawing of a controller in its current state, `scale` times the base size.
pub fn controller(s: &PadState, art: Art, brand: Brand, scale: f32, t: &Theme) -> AnyElement {
    let k = scale;
    let ink = ink(t);
    let l = layout(art);
    let on = |b: u32| s.pressed(b);
    // An element centred on (x, y) of the grid.
    let at = |x: f32, y: f32, w: f32, h: f32| {
        div().absolute().left(px((x - w / 2.) * k)).top(px((y + TOP - h / 2.) * k)).w(px(w * k)).h(px(h * k))
    };
    let lit = |pressed: bool, d: Div| if pressed { d.bg(ink.lit).border_color(ink.lit) } else { d.bg(ink.well).border_color(ink.edge) };
    let body_block = |d: Div| {
        d.border_1().border_color(ink.edge).bg(linear_gradient(
            180.,
            linear_color_stop(ink.body_top, 0.),
            linear_color_stop(ink.body_bottom, 1.),
        ))
    };

    let mut root = div().relative().flex_none().w(px(WIDTH * k)).h(px(HEIGHT * k));

    // The shoulders, behind the body: bumper, and the trigger filling as it is pulled.
    let trigger = |x: f32, v: i16| {
        let fill = 44. * v.max(0) as f32 / 32767.;
        at(x, 4., 46., 10.)
            .rounded(px(4. * k))
            .border_1()
            .border_color(if v > 1000 { ink.lit } else { ink.edge })
            .bg(ink.well)
            .child(div().h_full().w(px(fill * k)).rounded(px(3. * k)).bg(ink.lit))
    };
    let bumper = |x: f32, b: u32| lit(on(b), at(x, 20., 58., 10.).rounded(px(5. * k)).border_1());
    let (sl, sr) = l.shoulders;
    root = root
        .child(trigger(sl, s.axes[axis::LEFT_TRIGGER]))
        .child(trigger(sr, s.axes[axis::RIGHT_TRIGGER]))
        .child(bumper(sl, button::LEFT_SHOULDER))
        .child(bumper(sr, button::RIGHT_SHOULDER));

    root = match art {
        Art::Offset => root.child(curved_body(OFFSET_BODY, k, &ink)),
        Art::PlayStation | Art::Symmetric => root.child(curved_body(PLAYSTATION_BODY, k, &ink)),
        // Two discs joined by a bar, like a Super Nintendo pad.
        Art::Retro => root.child(body_block(at(160., 104., 300., 132.).rounded(px(66. * k)))),
        Art::JoyCons => root
            .child(body_block(at(86., 108., 92., 196.).rounded_tl(px(40. * k)).rounded_bl(px(40. * k)).rounded_r(px(8. * k))))
            .child(body_block(at(234., 108., 92., 196.).rounded_tr(px(40. * k)).rounded_br(px(40. * k)).rounded_l(px(8. * k)))),
        Art::Handheld => root
            .child(body_block(at(160., 110., 316., 176.).rounded(px(44. * k))))
            .child(at(160., 108., 150., 116.).rounded(px(6. * k)).bg(ink.well).border_1().border_color(ink.edge)),
    };

    let stick = |(x, y): (f32, f32), ax: usize, ay: usize, click: u32| {
        let (dx, dy) = (s.axes[ax] as f32 / 32768. * 11., s.axes[ay] as f32 / 32768. * 11.);
        let moved = s.axes[ax].unsigned_abs() > 4000 || s.axes[ay].unsigned_abs() > 4000;
        div()
            .child(at(x, y, 42., 42.).rounded_full().bg(ink.well).border_1().border_color(if on(click) { ink.lit } else { ink.edge }))
            .child(
                at(x + dx, y + dy, 26., 26.)
                    .rounded_full()
                    .border_1()
                    .border_color(if moved || on(click) { ink.lit } else { ink.edge })
                    .bg(if on(click) { ink.lit } else { ink.body_top }),
            )
    };
    let dpad = |(x, y): (f32, f32)| {
        let arm = |dx: f32, dy: f32, b: u32| lit(on(b), at(x + dx, y + dy, 13., 13.).rounded(px(3. * k)).border_1());
        div()
            .child(at(x, y, 13., 13.).bg(ink.well))
            .child(arm(0., -13., button::DPAD_UP))
            .child(arm(13., 0., button::DPAD_RIGHT))
            .child(arm(0., 13., button::DPAD_DOWN))
            .child(arm(-13., 0., button::DPAD_LEFT))
    };
    let labels = face_labels(brand);
    let face = |(x, y): (f32, f32)| {
        let b = |dx: f32, dy: f32, idx: u32, label: &'static str| {
            let pressed = on(idx);
            lit(pressed, at(x + dx, y + dy, 20., 20.).rounded_full().border_1())
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10. * k))
                .text_color(if pressed { ink.on_lit } else { ink.label })
                .child(label)
        };
        div()
            .child(b(0., -19., button::NORTH, labels[0]))
            .child(b(19., 0., button::EAST, labels[1]))
            .child(b(0., 19., button::SOUTH, labels[2]))
            .child(b(-19., 0., button::WEST, labels[3]))
    };
    let small = |(x, y): (f32, f32), b: u32| lit(on(b), at(x, y, 14., 8.).rounded(px(4. * k)).border_1());

    if let Some(p) = l.left_stick {
        root = root.child(stick(p, axis::LEFT_X, axis::LEFT_Y, button::LEFT_STICK));
    }
    if let Some(p) = l.right_stick {
        root = root.child(stick(p, axis::RIGHT_X, axis::RIGHT_Y, button::RIGHT_STICK));
    }
    root.child(dpad(l.dpad))
        .child(face(l.face))
        .child(small(l.view, button::BACK))
        .child(small(l.menu, button::START))
        .when_some(l.guide, |d, (x, y)| d.child(lit(on(button::GUIDE), at(x, y, 16., 16.).rounded_full().border_1())))
        .when_some(l.touchpad, |d, (x, y, w, h)| d.child(lit(on(button::TOUCHPAD), at(x, y, w, h).rounded(px(6. * k)).border_1())))
        .when(art == Art::Handheld, |d| {
            let (x, y, w, h) = l.touchpad.unwrap_or_default();
            d.child(lit(on(21), at(320. - x, y, w, h).rounded(px(6. * k)).border_1()))
        })
        .into_any_element()
}
