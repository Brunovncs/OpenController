//! The building blocks every screen uses: text styles, cards, buttons, chips, the toggle switch,
//! keycaps, the player indicator and the battery.

use crate::theme::{DISPLAY_FONT, FONT, ICONS, Theme, radius, text};
use gpui::prelude::FluentBuilder;
use gpui::{
    Div, ElementId, FontWeight, Hsla, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div, linear_color_stop,
    linear_gradient, px,
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

pub fn icon(glyph: &'static str, size: f32, color: Hsla) -> Div {
    div().flex_none().font_family(ICONS).text_size(px(size)).line_height(px(size)).text_color(color).child(glyph)
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
        .child(label.into())
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
