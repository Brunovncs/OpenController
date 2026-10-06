//! macOS: the Accessibility permission, which only the user can grant, in System Settings.

use super::{Card, Need};
use crate::ui::MainView;
use crate::widgets::{Kind, button, caption, group};
use gpui::{AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::Snapshot;
use open_controller_core::i18n::Text;
use open_controller_core::macos::accessibility;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Component {
    Accessibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Opened,
}

pub fn hint(text: &Text) -> &'static str {
    text.requirements_hint_mac
}

pub fn installed() -> Vec<(Component, Option<String>)> {
    vec![(Component::Accessibility, accessibility::granted().then(String::new))]
}

pub fn install(_: Component, _: &Path, _: bool) -> Result<Outcome, String> {
    accessibility::open_settings();
    Ok(Outcome::Opened)
}

pub fn attention(_: &Snapshot, text: &'static Text) -> Option<&'static str> {
    (!accessibility::granted()).then_some(text.accessibility_missing)
}

impl MainView {
    pub fn render_requirements(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let text = self.model.read(cx).text;
        // Asked again on every look: the user grants it in System Settings, outside this window.
        let works = accessibility::granted();
        let action = (!works).then(|| {
            button("open-settings", text.open_system_settings, Kind::Primary, &t)
                .on_click(cx.listener(|this, _, _, cx| this.install(Component::Accessibility, true, cx)))
                .into_any_element()
        });
        let card = Card {
            id: 0,
            name: text.accessibility,
            need: Need::Required,
            desc: text.accessibility_desc,
            works,
            status: if works { text.allowed } else { text.not_allowed }.to_string(),
            note: None,
            action,
            home: None,
        };
        div()
            .flex()
            .flex_col()
            .child(group().child(self.requirement(card, text)))
            .child(caption(text.mac_note, t.text3).pt(px(10.)))
            .into_any_element()
    }
}
