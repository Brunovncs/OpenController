//! A controller's Sticks section: each stick's deadzone, anti-deadzone, full tilt and curve, one
//! set for both or one each, from a choice or typed in; a live picture of what games get;
//! measuring a stick to set its centre, deadzone and full tilt; and a stick as the d-pad.

use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::{Theme, radius};
use crate::ui::MainView;
use crate::widgets::{self, Kind, button, caption, card, strong};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, Context, Div, Entity, Focusable, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, canvas, div, point, px,
};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::mapping::{PadState, axis};
use open_controller_core::motion::{self, EDGE_SECTORS, Edge, Rest};
use open_controller_core::profile::{DpadStick, Edit, Shape, Sticks};
use open_controller_core::{PadKey, PadView};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

/// One setting of a stick's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    Deadzone,
    AntiDeadzone,
    FullTilt,
    Curve,
}

/// Picked in a row of choices when the value is none of them.
const OTHER: u16 = u16::MAX;

impl Knob {
    fn id(self) -> &'static str {
        match self {
            Knob::Deadzone => "deadzone",
            Knob::AntiDeadzone => "anti-deadzone",
            Knob::FullTilt => "full-tilt",
            Knob::Curve => "curve",
        }
    }

    fn get(self, s: Shape) -> u16 {
        match self {
            Knob::Deadzone => u16::from(s.deadzone),
            Knob::AntiDeadzone => u16::from(s.anti_deadzone),
            Knob::FullTilt => u16::from(s.full_tilt),
            Knob::Curve => s.curve,
        }
    }

    /// `v` within the knob's range.
    fn set(self, s: Shape, v: u16) -> Shape {
        let (lo, hi) = self.range();
        let v = v.clamp(lo, hi);
        match self {
            Knob::Deadzone => Shape { deadzone: v as u8, ..s },
            Knob::AntiDeadzone => Shape { anti_deadzone: v as u8, ..s },
            Knob::FullTilt => Shape { full_tilt: v as u8, ..s },
            Knob::Curve => Shape { curve: v, ..s },
        }
    }

    fn range(self) -> (u16, u16) {
        match self {
            Knob::Deadzone => (0, u16::from(Shape::MAX_DEADZONE)),
            Knob::AntiDeadzone => (0, u16::from(Shape::MAX_ANTI_DEADZONE)),
            Knob::FullTilt => (u16::from(Shape::MIN_FULL_TILT), 100),
            Knob::Curve => (Shape::MIN_CURVE, Shape::MAX_CURVE),
        }
    }

    fn presets(self) -> &'static [u16] {
        match self {
            Knob::Deadzone => &[0, 5, 10, 15, 20, 25],
            Knob::AntiDeadzone => &[0, 10, 15, 20, 30],
            Knob::FullTilt => &[100, 95, 90, 85, 80],
            Knob::Curve => &[Shape::LINEAR, 150, 200, 70],
        }
    }

    /// A value as typed in the box: percent as a whole number, the curve's power with decimals.
    fn typed(self, v: u16, text: &Text) -> String {
        match self {
            Knob::Curve => power(v, text),
            _ => v.to_string(),
        }
    }

    /// A value as a choice shows it.
    fn label(self, v: u16, text: &Text) -> String {
        match (self, v) {
            (Knob::Curve, Shape::LINEAR) => text.curve_linear.into(),
            (Knob::Curve, 150) => text.curve_precise.into(),
            (Knob::Curve, 200) => text.curve_very_precise.into(),
            (Knob::Curve, 70) => text.curve_quick.into(),
            (Knob::Curve, _) => power(v, text),
            _ => format!("{v} %"),
        }
    }

    /// What was typed, read either way round for decimals and with or without the percent sign;
    /// not yet held to the range.
    fn parse(self, s: &str) -> Option<u16> {
        let s = s.trim().trim_end_matches('%').trim().replace(',', ".");
        let n: f32 = s.parse().ok().filter(|n: &f32| n.is_finite() && *n >= 0.)?;
        let v = if self == Knob::Curve { n * 100. } else { n };
        (v <= f32::from(u16::MAX - 1)).then(|| v.round() as u16)
    }
}

/// A curve's power in hundredths as a person writes it: 150 is 1.5, 200 is 2.
fn power(v: u16, text: &Text) -> String {
    let s = format!("{}.{:02}", v / 100, v % 100);
    s.trim_end_matches('0').trim_end_matches('.').replace('.', text.decimal)
}

/// A share of a stick's travel in tenths of a percent, as a person reads it.
fn tenths(v: u16, text: &Text) -> String {
    format!("{}.{}", v / 10, v % 10).replace('.', text.decimal)
}

/// The box a value is typed into, open under one of the settings.
pub struct Custom {
    knob: Knob,
    right: bool,
    field: Entity<TextField>,
    _events: Subscription,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// The sticks left alone, for where they rest.
    Rest,
    /// One of the sticks, by its place in `Measuring::sticks`, turned around its edge.
    Edge(usize),
    Done,
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Moved,
    Incomplete,
    NoInput,
}

const REST_TIME: Duration = Duration::from_secs(2);
/// After the circle fills, the turn goes on this long so each direction gets its farthest reach.
const EDGE_EXTRA: Duration = Duration::from_millis(1200);
const EDGE_TIMEOUT: Duration = Duration::from_secs(20);

/// Measuring a controller's sticks: where they rest, then how far each reaches.
pub struct Measuring {
    pub key: PadKey,
    /// The sticks measured, `false` for the left one: both when they share their settings.
    sticks: Vec<bool>,
    step: Step,
    since: Instant,
    rest: [Vec<[i16; 2]>; 2],
    rests: [Option<Rest>; 2],
    edges: [Edge; 2],
    filled: Option<Instant>,
}

fn stick_of(input: &PadState, right: bool) -> [i16; 2] {
    if right { [input.axes[axis::RIGHT_X], input.axes[axis::RIGHT_Y]] } else { [input.axes[axis::LEFT_X], input.axes[axis::LEFT_Y]] }
}

impl Measuring {
    /// Takes in the controller's sticks as they are now.
    pub fn sample(&mut self, input: &PadState) {
        match self.step {
            Step::Rest => {
                for &right in &self.sticks {
                    self.rest[usize::from(right)].push(stick_of(input, right));
                }
            }
            Step::Edge(i) => {
                let right = self.sticks[i];
                let [x, y] = stick_of(input, right);
                let centre = self.rests[usize::from(right)].map_or([0, 0], |r| r.centre);
                self.edges[usize::from(right)].add(x, y, centre);
            }
            Step::Done | Step::Failed(_) => {}
        }
    }

    fn running(&self) -> bool {
        matches!(self.step, Step::Rest | Step::Edge(_))
    }

    /// Moves on when the time for a step is up; whether to keep ticking.
    fn tick(&mut self, now: Instant) -> bool {
        match self.step {
            Step::Rest if now >= self.since + REST_TIME => {
                for &right in &self.sticks {
                    let r = motion::measure_rest(&self.rest[usize::from(right)]);
                    if r.is_none() {
                        self.step = Step::Failed(Failure::Moved);
                        return false;
                    }
                    self.rests[usize::from(right)] = r;
                }
                self.step = Step::Edge(0);
                self.since = now;
            }
            Step::Edge(i) => {
                let edge = self.edges[usize::from(self.sticks[i])];
                if edge.covered() == EDGE_SECTORS {
                    let filled = *self.filled.get_or_insert(now);
                    if now >= filled + EDGE_EXTRA {
                        self.filled = None;
                        self.since = now;
                        self.step = if i + 1 < self.sticks.len() { Step::Edge(i + 1) } else { Step::Done };
                    }
                } else if now >= self.since + EDGE_TIMEOUT {
                    self.step = Step::Failed(if edge.covered() == 0 { Failure::NoInput } else { Failure::Incomplete });
                }
            }
            _ => {}
        }
        self.running()
    }

    /// The sticks' settings with what was measured: each stick's centre, and for the shape they
    /// use, the larger deadzone and the smaller full tilt the sticks measured need.
    fn apply(&self, s: Sticks) -> Sticks {
        let mut s = s;
        let mut deadzone = 0;
        let mut full_tilt = 100;
        for &right in &self.sticks {
            let i = usize::from(right);
            if let Some(r) = self.rests[i] {
                s.centre[i] = r.centre;
                deadzone = deadzone.max(r.deadzone);
            }
            if let Some(f) = self.edges[i].full_tilt() {
                full_tilt = full_tilt.min(f);
            }
        }
        let right = self.sticks[0];
        s.with_shape(right, Shape { deadzone, full_tilt, ..s.shape(right) })
    }
}

/// A choice styled as the others in a row, for the value typed in.
fn other_choice(id: &'static str, label: impl Into<SharedString>, on: bool, t: &Theme) -> gpui::Stateful<Div> {
    div()
        .id((id, 99usize))
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(56.))
        .h(px(30.))
        .px(px(10.))
        .rounded(px(radius::CHIP))
        .cursor_pointer()
        .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke_strong))
        .child(caption(label.into(), if on { t.text } else { t.text2 }))
}

fn setting(label: &'static str, desc: &'static str, t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().flex().flex_col().gap(px(2.)).child(strong(label, t.text)).child(caption(desc, t.text2)))
}

/// A circle as a path of short lines.
fn circle(b: &mut PathBuilder, c: Point<Pixels>, r: f32) {
    let n = 48;
    for i in 0..=n {
        let a = i as f32 / n as f32 * 2. * PI;
        let p = c + point(px(a.cos() * r), px(a.sin() * r));
        if i == 0 { b.move_to(p) } else { b.line_to(p) }
    }
}

fn paint(b: PathBuilder, color: Hsla, window: &mut Window) {
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

fn disc(c: Point<Pixels>, r: f32, color: Hsla, window: &mut Window) {
    let mut b = PathBuilder::fill();
    circle(&mut b, c, r);
    b.close();
    paint(b, color, window);
}

fn ring(c: Point<Pixels>, r: f32, width: f32, color: Hsla, window: &mut Window) {
    let mut b = PathBuilder::stroke(px(width));
    circle(&mut b, c, r);
    paint(b, color, window);
}

fn line(a: Point<Pixels>, z: Point<Pixels>, width: f32, color: Hsla, window: &mut Window) {
    let mut b = PathBuilder::stroke(px(width));
    b.move_to(a);
    b.line_to(z);
    paint(b, color, window);
}

const PLOT: f32 = 168.;
const INSET: f32 = 14.;

/// A stick's travel from its centre, 0 to 1.
fn travel(at: [i16; 2], centre: [i16; 2]) -> f32 {
    ((f32::from(at[0]) - f32::from(centre[0])) / 32767.).hypot((f32::from(at[1]) - f32::from(centre[1])) / 32767.)
}

/// The curve of a shape, what games get for each distance the stick is pushed, with the sticks
/// using it as dots on it.
fn curve_plot(shape: Shape, dots: Vec<(f32, Hsla)>, t: &Theme) -> impl IntoElement {
    let t = *t;
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let side = PLOT - 2. * INSET;
            let o = bounds.origin + point(px(INSET), px(INSET));
            let at = |x: f32, y: f32| o + point(px(x.min(1.) * side), px((1. - y) * side));
            let dz = f32::from(shape.deadzone) / 100.;
            if dz > 0. {
                let mut b = PathBuilder::fill();
                b.move_to(at(0., 0.));
                b.line_to(at(dz, 0.));
                b.line_to(at(dz, 1.));
                b.line_to(at(0., 1.));
                b.close();
                paint(b, t.text3.opacity(0.18), window);
            }
            let mut frame = PathBuilder::stroke(px(1.));
            frame.move_to(at(0., 1.));
            frame.line_to(at(0., 0.));
            frame.line_to(at(1., 0.));
            paint(frame, t.stroke_strong, window);
            line(at(0., 0.), at(1., 1.), 1., t.stroke, window);
            if shape.full_tilt < 100 {
                let f = f32::from(shape.full_tilt) / 100.;
                line(at(f, 0.), at(f, 1.), 1., t.text3, window);
            }
            let mut b = PathBuilder::stroke(px(2.));
            let n = 96;
            for i in 0..=n {
                let x = i as f32 / n as f32;
                let p = at(x, motion::response(x, shape));
                if i == 0 { b.move_to(p) } else { b.line_to(p) }
            }
            paint(b, t.accent, window);
            for (x, color) in &dots {
                disc(at(*x, motion::response(*x, shape)), 4.5, *color, window);
            }
        },
    )
    .w(px(PLOT))
    .h(px(PLOT))
}

/// A stick seen from above: the ring where it is, the dot where games see it, its deadzone and
/// full tilt, and while it is measured, the directions its edge was reached in.
fn stick_view(at: [i16; 2], shape: Shape, centre: [i16; 2], reached: Option<Edge>, t: &Theme) -> impl IntoElement {
    let t = *t;
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let r = PLOT / 2. - INSET;
            let c = bounds.origin + point(px(PLOT / 2.), px(PLOT / 2.));
            let pos = |v: [f32; 2]| {
                let len = v[0].hypot(v[1]);
                let k = if len > 1.08 { 1.08 / len } else { 1. };
                c + point(px(v[0] * k * r), px(v[1] * k * r))
            };
            disc(c, r, t.control, window);
            ring(c, r, 1., t.stroke_strong, window);
            if shape.deadzone > 0 {
                disc(c, f32::from(shape.deadzone) / 100. * r, t.text3.opacity(0.25), window);
            }
            if shape.full_tilt < 100 {
                ring(c, f32::from(shape.full_tilt) / 100. * r, 1., t.text3, window);
            }
            line(c - point(px(5.), px(0.)), c + point(px(5.), px(0.)), 1., t.stroke_strong, window);
            line(c - point(px(0.), px(5.)), c + point(px(0.), px(5.)), 1., t.stroke_strong, window);
            if let Some(e) = reached {
                for i in 0..EDGE_SECTORS {
                    let a0 = i as f32 / EDGE_SECTORS as f32 * 2. * PI - PI;
                    let a1 = a0 + 2. * PI / EDGE_SECTORS as f32 * 0.85;
                    let mut b = PathBuilder::stroke(px(5.));
                    for k in 0..=6 {
                        let a = a0 + (a1 - a0) * k as f32 / 6.;
                        let p = c + point(px(a.cos() * (r + 1.)), px(a.sin() * (r + 1.)));
                        if k == 0 { b.move_to(p) } else { b.line_to(p) }
                    }
                    paint(b, if e.sector(i) { t.accent } else { t.stroke_strong }, window);
                }
            }
            let raw = [f32::from(at[0]) / 32767., f32::from(at[1]) / 32767.];
            let (x, y) = motion::shape_stick(at[0], at[1], shape, centre);
            let out = [f32::from(x) / 32767., f32::from(y) / 32767.];
            ring(pos(raw), 7., 1.5, t.text2, window);
            disc(pos(out), 5., t.accent, window);
        },
    )
    .w(px(PLOT))
    .h(px(PLOT))
}

impl MainView {
    /// Changes one setting of the shape a stick uses.
    fn set_knob(&mut self, key: PadKey, knob: Knob, right: bool, v: u16, cx: &mut Context<Self>) {
        let Some(pad) = self.pad(key, cx) else { return };
        let s = pad.profiles.active().sticks;
        let shape = knob.set(s.shape(right), v);
        self.edit_now(key, Edit::Sticks(s.with_shape(right, shape)), cx);
    }

    fn open_custom(&mut self, key: PadKey, knob: Knob, right: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pad) = self.pad(key, cx) else { return };
        let text = self.model.read(cx).text;
        let now = knob.get(pad.profiles.active().sticks.shape(right));
        let field = cx.new(|cx| TextField::new(cx, false, 6));
        field.update(cx, |f, cx| f.set_text_selected(&knob.typed(now, text), cx));
        let events = cx.subscribe_in(&field, window, move |this, field, ev, window, cx| {
            let typed = knob.parse(&field.read(cx).text());
            let (lo, hi) = knob.range();
            match ev {
                // As it is typed, a value in range applies right away; one out of it waits.
                TextFieldEvent::Changed => {
                    if let Some(v) = typed.filter(|v| (lo..=hi).contains(v)) {
                        this.set_knob(key, knob, right, v, cx);
                    }
                }
                TextFieldEvent::Submit => {
                    if let Some(v) = typed {
                        this.set_knob(key, knob, right, v, cx);
                    }
                    this.close_custom(window, cx);
                }
            }
        });
        field.focus_handle(cx).focus(window, cx);
        self.custom = Some(Custom { knob, right, field, _events: events });
        cx.notify();
    }

    pub fn close_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.custom = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// The choices for one setting, the value typed in among them, and the box when it is open.
    fn knob_row(&self, key: PadKey, knob: Knob, right: bool, shape: Shape, text: &'static Text, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let v = knob.get(shape);
        let presets = knob.presets();
        let open = self.custom.as_ref().filter(|c| c.knob == knob && c.right == right);
        let other = !presets.contains(&v);
        let picked = if other || open.is_some() { OTHER } else { v };
        let choices = presets.iter().map(|&p| (p, knob.label(p, text).into())).collect();
        let label = if other { fill(text.custom_value_is, knob.label(v, text)) } else { text.custom_value.to_string() };
        let row = widgets::segmented(knob.id(), choices, picked, &t, cx, move |this, v, cx| {
            this.custom = None;
            this.set_knob(key, knob, right, v, cx);
        })
        .child(
            other_choice(knob.id(), label, picked == OTHER, &t)
                .on_click(cx.listener(move |this, _, window, cx| this.open_custom(key, knob, right, window, cx))),
        );
        let (lo, hi) = knob.range();
        let unit = |v: u16| if knob == Knob::Curve { power(v, text) } else { format!("{v} %") };
        let hint = fill(&fill(text.custom_range, unit(lo)), unit(hi));
        div().flex().flex_col().gap(px(8.)).child(row).when_some(open, |d, c| {
            d.child(div().flex().items_center().gap(px(12.)).child(div().w(px(96.)).child(c.field.clone())).child(caption(hint, t.text2)))
        })
    }

    fn start_measure(&mut self, key: PadKey, sticks: Vec<bool>, window: &mut Window, cx: &mut Context<Self>) {
        if self.custom.is_some() {
            self.close_custom(window, cx);
        }
        let mut m = Measuring {
            key,
            sticks,
            step: Step::Rest,
            since: Instant::now(),
            rest: [Vec::new(), Vec::new()],
            rests: [None; 2],
            edges: [Edge::default(); 2],
            filled: None,
        };
        if let Some(pad) = self.pad(key, cx) {
            m.sample(&pad.input);
        }
        self.measuring = Some(m);
        cx.notify();
        // Snapshots come as the sticks move; these ticks also catch a stick held still.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                let going = this.update(cx, |this, cx| this.measure_tick(key, cx)).unwrap_or(false);
                if !going {
                    break;
                }
            }
        })
        .detach();
    }

    fn measure_tick(&mut self, key: PadKey, cx: &mut Context<Self>) -> bool {
        let pad = self.pad(key, cx);
        let Some(m) = self.measuring.as_mut().filter(|m| m.key == key && m.running()) else { return false };
        if let Some(p) = &pad {
            m.sample(&p.input);
        }
        let going = m.tick(Instant::now());
        if m.step == Step::Done
            && let Some(p) = pad
        {
            let s = m.apply(p.profiles.active().sticks);
            self.edit_now(key, Edit::Sticks(s), cx);
        }
        cx.notify();
        going
    }

    /// Measuring: what to do now, what came out, and the buttons to start, stop or go again.
    fn measure_block(&self, pad: &PadView, sticks: Vec<bool>, text: &'static Text, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let key = pad.key;
        let m = self.measuring.as_ref().filter(|m| m.key == key);
        let mut lines: Vec<(SharedString, Hsla)> = Vec::new();
        match m.map(|m| m.step) {
            Some(Step::Rest) => {
                let both = m.is_some_and(|m| m.sticks.len() == 2);
                lines.push(((if both { text.measure_rest_both } else { text.measure_rest_one }).into(), t.text));
            }
            Some(Step::Edge(i)) => {
                let right = m.is_some_and(|m| m.sticks[i]);
                lines.push(((if right { text.measure_edge_right } else { text.measure_edge_left }).into(), t.text));
            }
            Some(Step::Done) => {
                if let Some(m) = m {
                    for &right in &m.sticks {
                        let i = usize::from(right);
                        let (Some(r), Some(least)) = (m.rests[i], m.edges[i].least()) else { continue };
                        let off = (travel(r.centre, [0, 0]) * 1000.).round() as u16;
                        let name = if right { text.stick_right } else { text.stick_left };
                        let line = fill(
                            &fill(&fill(&fill(text.measure_result, name), tenths(off, text)), tenths(r.wander, text)),
                            tenths(least.min(1000), text),
                        );
                        lines.push((line.into(), t.text));
                    }
                }
                lines.push((text.measure_applied.into(), t.success));
            }
            Some(Step::Failed(f)) => {
                let why = match f {
                    Failure::Moved => text.measure_moved,
                    Failure::Incomplete => text.measure_incomplete,
                    Failure::NoInput => text.measure_no_input,
                };
                lines.push((why.into(), t.caution));
            }
            None => {}
        }
        let running = m.is_some_and(Measuring::running);
        let title = if sticks.len() == 2 { text.measure_both } else { text.measure };
        let action = if running {
            button("measure-cancel", text.measure_cancel, Kind::Standard, &t).on_click(cx.listener(|this, _, _, cx| {
                this.measuring = None;
                cx.notify();
            }))
        } else {
            let label = if m.is_some() { text.measure_again } else { text.measure_start };
            button("measure", label, Kind::Standard, &t)
                .on_click(cx.listener(move |this, _, window, cx| this.start_measure(key, sticks.clone(), window, cx)))
        };
        setting(title, text.measure_desc, &t)
            .children(lines.into_iter().map(|(l, color)| caption(l, color)))
            .child(div().flex().child(action))
    }

    pub fn sticks_section(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let s = pad.profiles.active().sticks;
        let shared = s.right.is_none();
        let right = !shared && self.stick_right;
        let shape = s.shape(right);
        // The sticks these settings are for.
        let sticks: Vec<bool> = if shared { vec![false, true] } else { vec![right] };

        let same = self.toggle(
            "sticks-same",
            (text.sticks_same, text.sticks_same_desc),
            shared,
            Edit::Sticks(Sticks { right: if shared { Some(s.left) } else { None }, ..s }),
            key,
            cx,
        );
        let which = (!shared).then(|| {
            let choice = widgets::segmented(
                "sticks-which",
                vec![(false, text.stick_left.into()), (true, text.stick_right.into())],
                right,
                &t,
                cx,
                |this, v, cx| {
                    this.stick_right = v;
                    this.custom = None;
                    cx.notify();
                },
            );
            div().flex().flex_col().gap(px(8.)).child(strong(text.sticks_editing, t.text)).child(choice)
        });

        // What games get, live, for the sticks these settings are for.
        let measuring = self.measuring.as_ref().filter(|m| m.key == key);
        let edge_of = |r: bool| match measuring.map(|m| (m.step, m)) {
            Some((Step::Edge(i), m)) if m.sticks[i] == r => Some(m.edges[usize::from(r)]),
            _ => None,
        };
        let dots = sticks.iter().map(|&r| (travel(stick_of(&pad.input, r), s.centre[usize::from(r)]), t.accent)).collect();
        let mut views = div().flex().flex_wrap().gap(px(12.)).child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .child(div().rounded(px(radius::CONTROL)).bg(t.control).child(curve_plot(shape, dots, &t)))
                .child(caption(text.curve, t.text2)),
        );
        for &r in &sticks {
            let name = if r { text.stick_right } else { text.stick_left };
            views = views.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(stick_view(stick_of(&pad.input, r), s.shape(r), s.centre[usize::from(r)], edge_of(r), &t))
                    .child(caption(name, t.text2)),
            );
        }
        let preview = div().flex().flex_col().gap(px(8.)).child(views).child(caption(text.sticks_preview_desc, t.text2));

        let off = sticks.iter().map(|&r| travel(s.centre[usize::from(r)], [0, 0])).fold(0., f32::max);
        let centre_note = (off > 0.).then(|| {
            let pct = tenths((off * 1000.).round() as u16, text);
            let cleared = sticks.iter().fold(s, |mut s, &r| {
                s.centre[usize::from(r)] = [0, 0];
                s
            });
            div().flex().items_center().gap(px(12.)).child(caption(fill(text.centre_corrected, pct), t.text2)).child(
                button("centre-reset", text.centre_reset, Kind::Subtle, &t)
                    .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Sticks(cleared), cx))),
            )
        });

        let knob = |label, desc, k| (label, desc, k);
        let knobs = [
            knob(text.deadzone, text.deadzone_desc, Knob::Deadzone),
            knob(text.anti_deadzone, text.anti_deadzone_desc, Knob::AntiDeadzone),
            knob(text.full_tilt, text.full_tilt_desc, Knob::FullTilt),
            knob(text.curve, text.curve_desc, Knob::Curve),
        ];
        let rows: Vec<Div> = knobs
            .into_iter()
            .map(|(label, desc, k)| setting(label, desc, &t).child(self.knob_row(key, k, right, shape, text, cx)))
            .collect();

        let dpad = widgets::segmented(
            "dpad-stick",
            vec![(DpadStick::Off, text.off.into()), (DpadStick::Left, text.stick_left.into()), (DpadStick::Right, text.stick_right.into())],
            s.dpad,
            &t,
            cx,
            move |this, v, cx| this.edit_now(key, Edit::Sticks(Sticks { dpad: v, ..s }), cx),
        );
        card(&t)
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(20.))
            .max_w(px(760.))
            .child(caption(text.sticks_desc, t.text2))
            .child(same)
            .children(which)
            .child(preview)
            .child(self.measure_block(pad, sticks, text, cx))
            .children(centre_note)
            .children(rows)
            .child(setting(text.dpad_stick, text.dpad_stick_desc, &t).child(dpad))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_controller_core::i18n::{EN, PT};

    #[test]
    fn typed_values_read_either_way() {
        assert_eq!(Knob::Deadzone.parse(" 12 % "), Some(12));
        assert_eq!(Knob::Deadzone.parse("7.6"), Some(8));
        assert_eq!(Knob::Curve.parse("1,5"), Some(150));
        assert_eq!(Knob::Curve.parse("2"), Some(200));
        assert_eq!(Knob::Curve.parse("0.333"), Some(33));
        for bad in ["", "-3", "abc", "inf", "1e9"] {
            assert_eq!(Knob::Deadzone.parse(bad), None, "{bad}");
        }
        assert_eq!(Knob::FullTilt.set(Shape::default(), 20).full_tilt, Shape::MIN_FULL_TILT, "held to the range");
        assert_eq!(Knob::Curve.set(Shape::default(), 9000).curve, Shape::MAX_CURVE);
    }

    #[test]
    fn values_as_people_write_them() {
        assert_eq!([power(150, &EN), power(200, &EN), power(75, &PT), power(125, &PT)], ["1.5", "2", "0,75", "1,25"]);
        assert_eq!(Knob::Curve.label(150, &EN), EN.curve_precise);
        assert_eq!(Knob::Curve.label(180, &PT), "1,8");
        assert_eq!(Knob::FullTilt.label(92, &EN), "92 %");
        assert_eq!(Knob::Curve.typed(Shape::LINEAR, &EN), "1");
        assert_eq!(tenths(34, &PT), "3,4");
    }

    #[test]
    fn every_preset_is_within_its_range() {
        for k in [Knob::Deadzone, Knob::AntiDeadzone, Knob::FullTilt, Knob::Curve] {
            let (lo, hi) = k.range();
            assert!(k.presets().iter().all(|p| (lo..=hi).contains(p)), "{k:?}");
            assert_eq!(k.get(k.set(Shape::default(), k.presets()[1])), k.presets()[1]);
        }
    }

    fn input(left: [i16; 2], right: [i16; 2]) -> PadState {
        let mut s = PadState::default();
        s.axes[axis::LEFT_X] = left[0];
        s.axes[axis::LEFT_Y] = left[1];
        s.axes[axis::RIGHT_X] = right[0];
        s.axes[axis::RIGHT_Y] = right[1];
        s
    }

    fn measuring(sticks: Vec<bool>) -> (Measuring, Instant) {
        let now = Instant::now();
        let m = Measuring {
            key: PadKey::Slot(1),
            sticks,
            step: Step::Rest,
            since: now,
            rest: [Vec::new(), Vec::new()],
            rests: [None; 2],
            edges: [Edge::default(); 2],
            filled: None,
        };
        (m, now)
    }

    /// Rolls a stick once around at `reach` of its travel from `centre`.
    fn roll(m: &mut Measuring, right: bool, reach: f32, centre: [i16; 2]) {
        for i in 0..64 {
            let a = i as f32 / 64. * 2. * PI;
            let at = [
                ((a.cos() * reach * 32767.) as i16).saturating_add(centre[0]),
                ((a.sin() * reach * 32767.) as i16).saturating_add(centre[1]),
            ];
            m.sample(&if right { input([0, 0], at) } else { input(at, [0, 0]) });
        }
    }

    #[test]
    fn measuring_both_sticks_sets_what_both_need() {
        let (mut m, t0) = measuring(vec![false, true]);
        for _ in 0..10 {
            m.sample(&input([1500, -800], [0, 300]));
        }
        assert!(m.tick(t0 + REST_TIME));
        assert_eq!(m.step, Step::Edge(0));
        roll(&mut m, false, 0.905, [1500, -800]);
        let t1 = t0 + REST_TIME + Duration::from_secs(1);
        assert!(m.tick(t1) && m.step == Step::Edge(0), "the turn goes on a moment once the circle fills");
        assert!(m.tick(t1 + EDGE_EXTRA) && m.step == Step::Edge(1));
        roll(&mut m, true, 1., [0, 300]);
        m.tick(t1 + EDGE_EXTRA * 2);
        assert!(!m.tick(t1 + EDGE_EXTRA * 3) && m.step == Step::Done);
        let s = m.apply(Sticks { left: Shape { curve: 150, ..Shape::default() }, ..Sticks::default() });
        assert_eq!(s.centre, [[1500, -800], [0, 300]]);
        assert_eq!(s.right, None, "still one shape for both");
        assert_eq!(s.left, Shape { deadzone: 2, full_tilt: 89, curve: 150, ..Shape::default() }, "the shorter reach of the two");
    }

    #[test]
    fn measuring_one_stick_leaves_the_other() {
        let (mut m, t0) = measuring(vec![true]);
        m.sample(&input([9000, 9000], [0, 0]));
        m.tick(t0 + REST_TIME);
        roll(&mut m, true, 1., [0, 0]);
        m.tick(t0 + REST_TIME);
        m.tick(t0 + REST_TIME + EDGE_EXTRA);
        assert_eq!(m.step, Step::Done, "the left stick held over does not matter");
        let other = Shape { deadzone: 9, ..Shape::default() };
        let s = m.apply(Sticks { left: other, right: Some(Shape::default()), centre: [[100, 100], [0, 0]], ..Sticks::default() });
        assert_eq!((s.left, s.centre[0]), (other, [100, 100]));
        assert_eq!(s.right, Some(Shape { deadzone: 2, ..Shape::default() }));
    }

    #[test]
    fn a_measurement_that_goes_wrong_says_why() {
        let (mut m, t0) = measuring(vec![false]);
        m.sample(&input([0, 0], [0, 0]));
        m.sample(&input([12000, 0], [0, 0]));
        assert!(!m.tick(t0 + REST_TIME));
        assert_eq!(m.step, Step::Failed(Failure::Moved));
        let (mut m, t0) = measuring(vec![false]);
        m.sample(&input([0, 0], [0, 0]));
        m.tick(t0 + REST_TIME);
        assert!(!m.tick(t0 + REST_TIME + EDGE_TIMEOUT));
        assert_eq!(m.step, Step::Failed(Failure::NoInput));
        let (mut m, t0) = measuring(vec![false]);
        m.sample(&input([0, 0], [0, 0]));
        m.tick(t0 + REST_TIME);
        m.sample(&input([32767, 0], [0, 0]));
        m.tick(t0 + REST_TIME + EDGE_TIMEOUT);
        assert_eq!(m.step, Step::Failed(Failure::Incomplete));
    }
}
