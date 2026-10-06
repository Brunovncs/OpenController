//! The building blocks every screen uses: text styles, cards, buttons, chips, the toggle switch,
//! a row of choices, keycaps, the player indicator and the battery.

use crate::theme::{DISPLAY_FONT, FONT, Theme, radius, text};
use gpui::prelude::FluentBuilder;
use gpui::{
    Context, Div, ElementId, FontWeight, Hsla, InteractiveElement, ParentElement, SharedString, Stateful, StatefulInteractiveElement,
    Styled, div, linear_color_stop, linear_gradient, px,
};

pub fn caption(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_size(px(text::CAPTION.0)).line_height(px(text::CAPTION.1)).text_color(color).child(s.into())
}

pub fn body(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_size(px(text::BODY.0)).line_height(px(text::BODY.1)).text_color(color).child(s.into())
}

pub fn strong(s: impl Into<SharedString>, color: Hsla) -> Div {
    body(s, color).font_weight(FontWeight::SEMIBOLD)
}

pub fn title(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_size(px(text::TITLE.0)).line_height(px(text::TITLE.1)).font_weight(FontWeight::SEMIBOLD).text_color(color).child(s.into())
}

pub fn display(s: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .font_family(DISPLAY_FONT)
        .text_size(px(text::DISPLAY.0))
        .line_height(px(text::DISPLAY.1))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(s.into())
}

pub fn icon(name: &'static str, size: f32, color: Hsla) -> Div {
    div().flex_none().size(px(size)).child(gpui::img(crate::icons::image(name, color)).size(px(size)))
}

/// A section title above a group of cards.
pub fn section(s: impl Into<SharedString>, t: &Theme) -> Div {
    title(s, t.text).pt(px(28.)).pb(px(10.))
}

/// A card: a raised surface with a hairline edge.
pub fn card(t: &Theme) -> Div {
    div().rounded(px(radius::CARD)).bg(t.layer).border_1().border_color(t.stroke)
}

/// One row of a group: a card holding a line of content.
pub fn row(t: &Theme) -> Div {
    card(t).flex().items_center().gap(px(16.)).px(px(16.)).py(px(12.)).min_h(px(56.))
}

/// Rows stacked a little apart, so a group reads as one block.
pub fn group() -> Div {
    div().flex().flex_col().gap(px(4.))
}

/// Where a drawn controller stands: a trace of the accent fading into the card below it.
pub fn stage(t: &Theme) -> Div {
    div().bg(linear_gradient(180., linear_color_stop(t.stage_top, 0.), linear_color_stop(t.stage_bottom, 1.)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Standard,
    Subtle,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Kind, t: &Theme) -> Stateful<Div> {
    button_base(id, kind, t).child(label.into())
}

/// A button with an icon before its label.
pub fn icon_label_button(
    id: impl Into<ElementId>,
    glyph: &'static str,
    label: impl Into<SharedString>,
    kind: Kind,
    t: &Theme,
) -> Stateful<Div> {
    let fg = if kind == Kind::Primary { t.on_accent } else { t.text };
    button_base(id, kind, t).pl(px(12.)).child(icon(glyph, 14., fg)).child(label.into())
}

fn button_base(id: impl Into<ElementId>, kind: Kind, t: &Theme) -> Stateful<Div> {
    let (bg, hover, fg, border) = match kind {
        Kind::Primary => (t.accent, t.accent.opacity(0.88), t.on_accent, t.accent),
        Kind::Standard => (t.control, t.control_hover, t.text, t.stroke),
        Kind::Subtle => (gpui::transparent_black(), t.control, t.text, gpui::transparent_black()),
    };
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .h(px(34.))
        .px(px(14.))
        .rounded(px(radius::CONTROL))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(text::BODY.0))
        .font_weight(if kind == Kind::Primary { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
        .text_color(fg)
        .font_family(FONT)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

/// A button holding only an icon.
pub fn icon_button(id: impl Into<ElementId>, glyph: &'static str, t: &Theme) -> Stateful<Div> {
    let hover = t.control;
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(icon(glyph, 16., t.text))
}

/// A small label with a tinted background: a connection, a state.
pub fn chip(s: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(22.))
        .px(px(8.))
        .rounded(px(radius::CHIP))
        .bg(bg)
        .text_size(px(text::CAPTION.0))
        .text_color(fg)
        .child(s.into())
}

/// The Windows toggle switch, with its state written next to it as Settings does.
pub fn switch(on: bool, label_on: &'static str, label_off: &'static str, t: &Theme) -> Div {
    let track = div()
        .flex_none()
        .flex()
        .items_center()
        .w(px(40.))
        .h(px(20.))
        .rounded(px(10.))
        .px(px(4.))
        .when(on, |d| d.justify_end().bg(t.accent))
        .when(!on, |d| d.border_1().border_color(t.text2))
        .child(div().size(px(12.)).rounded(px(6.)).bg(if on { t.on_accent } else { t.text2 }));
    div().flex().flex_none().items_center().gap(px(12.)).child(body(if on { label_on } else { label_off }, t.text2)).child(track)
}

/// A row of choices, one of them picked.
pub fn segmented<T: Copy + PartialEq + 'static, V: 'static>(
    id: &'static str,
    choices: Vec<(T, SharedString)>,
    picked: T,
    t: &Theme,
    cx: &mut Context<V>,
    on_pick: impl Fn(&mut V, T, &mut Context<V>) + 'static,
) -> Div {
    let on_pick = std::rc::Rc::new(on_pick);
    let mut d = div().flex().flex_wrap().gap(px(4.)).p(px(3.)).rounded(px(radius::CONTROL)).bg(t.control);
    for (i, (value, label)) in choices.into_iter().enumerate() {
        let on = value == picked;
        let on_pick = on_pick.clone();
        d = d.child(
            div()
                .id((id, i))
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(56.))
                .h(px(30.))
                .px(px(10.))
                .rounded(px(radius::CHIP))
                .cursor_pointer()
                .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke_strong))
                .child(caption(label, if on { t.text } else { t.text2 }))
                .on_click(cx.listener(move |this, _, _, cx| on_pick(this, value, cx))),
        );
    }
    d
}

pub fn keycap(s: impl Into<SharedString>, t: &Theme) -> Div {
    div()
        .flex_none()
        .px(px(7.))
        .min_w(px(24.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CHIP))
        .bg(t.control)
        .border_1()
        .border_color(t.stroke_strong)
        .text_size(px(text::CAPTION.0))
        .text_color(t.text)
        .child(s.into())
}

/// Four quadrants, like the ring on an Xbox controller, with the player's one lit.
pub fn player_mark(player: Option<u8>, t: &Theme) -> Div {
    let cell = |i: u8| {
        let lit = player == Some(i);
        div().size(px(5.)).rounded(px(1.)).when(lit, |d| d.bg(t.accent)).when(!lit, |d| d.border_1().border_color(t.text3))
    };
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().flex().gap(px(2.)).child(cell(0)).child(cell(1)))
        .child(div().flex().gap(px(2.)).child(cell(2)).child(cell(3)))
}

/// A small battery outline filled to `level` percent.
pub fn battery(level: u8, low: bool, t: &Theme) -> Div {
    let color = if low { t.critical } else { t.text2 };
    div()
        .flex_none()
        .flex()
        .items_center()
        .child(
            div()
                .w(px(18.))
                .h(px(9.))
                .p(px(1.5))
                .rounded(px(2.))
                .border_1()
                .border_color(color)
                .child(div().h_full().w(px(13. * level.min(100) as f32 / 100.)).rounded(px(1.)).bg(color)),
        )
        .child(div().w(px(1.5)).h(px(4.)).bg(color))
}
