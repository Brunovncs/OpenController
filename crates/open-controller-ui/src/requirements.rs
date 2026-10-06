//! What OpenController needs from the system, shown in the settings and warned about on the
//! home screen: drivers on Windows, a device rule on Linux, a permission on macOS. Each system's
//! module says what there is and how to get it; the cards look the same everywhere.

use crate::theme::{icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{caption, chip, icon, row, strong};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Div, Hsla, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::i18n::Text;

#[cfg_attr(windows, path = "requirements/windows.rs")]
#[cfg_attr(target_os = "linux", path = "requirements/linux.rs")]
#[cfg_attr(target_os = "macos", path = "requirements/macos.rs")]
mod sys;

pub use sys::{Component, Outcome, attention, hint, install, installed};

#[derive(Clone, Copy)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Need {
    Required,
    Recommended,
    Optional,
}

/// One thing to have: what it is, whether it works and what can be done about it.
pub struct Card {
    pub id: usize,
    pub name: &'static str,
    pub need: Need,
    pub desc: &'static str,
    pub works: bool,
    pub status: String,
    pub note: Option<(String, Hsla)>,
    pub action: Option<AnyElement>,
    pub home: Option<&'static str>,
}

impl MainView {
    pub fn requirement(&self, c: Card, text: &Text) -> Div {
        let t = self.theme;
        let (tag, tag_fg, tag_bg) = match c.need {
            Need::Required => (text.required, t.accent, t.accent_soft),
            Need::Recommended => (text.recommended, t.text2, t.control),
            Need::Optional => (text.optional, t.text3, t.control),
        };
        let link = c.home.map(|home| {
            div()
                .id(("home", c.id))
                .flex()
                .items_center()
                .gap(px(4.))
                .pt(px(2.))
                .cursor_pointer()
                .text_color(t.accent)
                .on_click(move |_, _, cx| cx.open_url(home))
                .child(caption(text.project_page, t.accent))
                .child(icon(glyph::OPEN, 10., t.accent))
        });
        row(&t)
            .items_start()
            .py(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded(px(radius::CONTROL))
                    .bg(match (c.works, c.need) {
                        (true, _) => t.success.opacity(0.14),
                        (false, Need::Optional) => t.control,
                        (false, _) => t.caution.opacity(0.14),
                    })
                    .child(match (c.works, c.need) {
                        (true, _) => icon(glyph::CHECK, 16., t.success),
                        (false, Need::Optional) => icon(glyph::ADD, 14., t.text2),
                        (false, _) => icon(glyph::WARNING, 16., t.caution),
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(4.))
                    .child(div().flex().items_center().gap(px(8.)).child(strong(c.name, t.text)).child(chip(tag, tag_fg, tag_bg)))
                    .child(caption(c.desc, t.text2))
                    .child(caption(c.status, if c.works { t.text2 } else { t.text3 }))
                    .when_some(c.note, |d, (n, color)| d.child(caption(n, color)))
                    .children(link),
            )
            .children(c.action)
    }
}
