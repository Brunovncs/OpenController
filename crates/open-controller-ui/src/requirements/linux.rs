//! Linux: one udev rule, installed as root through polkit when the user asks.

use super::{Card, Need};
use crate::theme::icon as glyph;
use crate::ui::{Install, MainView};
use crate::widgets::{Kind, body, button, group, icon};
use gpui::{AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::i18n::{Text, fill};
pub use open_controller_core::linux::setup::Outcome;
use open_controller_core::linux::setup::{self, RuleState};
use open_controller_core::{Driver, Snapshot};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Component {
    Rule,
}

pub fn hint(text: &Text) -> &'static str {
    text.requirements_hint_linux
}

pub fn installed() -> Vec<(Component, Option<String>)> {
    let state = match setup::rule_state() {
        RuleState::Missing => None,
        RuleState::Outdated => Some("outdated".to_string()),
        RuleState::Current => Some("current".to_string()),
    };
    vec![(Component::Rule, state)]
}

pub fn install(_: Component, dir: &Path, _: bool) -> Result<Outcome, String> {
    setup::install_rule(dir)
}

pub fn attention(snap: &Snapshot, text: &'static Text) -> Option<&'static str> {
    (!matches!(snap.vigem, Driver::Ready { .. })).then_some(text.device_access_missing)
}

impl MainView {
    pub fn render_requirements(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let model = self.model.read(cx);
        let text = model.text;
        let live = matches!(model.snapshot.vigem, Driver::Ready { .. });
        let state = self.installed.get(&Component::Rule).cloned().flatten();
        let outdated = state.as_deref() == Some("outdated");
        let works = live || state.is_some();
        let status = if works { text.installed.to_string() } else { text.not_installed.to_string() };
        let progress = self.installs.get(&Component::Rule).cloned();
        let note = match &progress {
            Some(Install::Done(Outcome::Installed)) => Some((text.reconnect_controllers.to_string(), t.text2)),
            Some(Install::Done(Outcome::Cancelled)) => Some((text.install_cancelled.to_string(), t.text2)),
            Some(Install::Failed(e)) => Some((fill(text.install_failed, e), t.critical)),
            _ if outdated => Some((text.device_access_outdated.to_string(), t.text2)),
            _ => None,
        };
        let action: Option<AnyElement> = match progress {
            Some(Install::Running) => Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(glyph::CLOCK, 14., t.text2))
                    .child(body(text.installing, t.text2))
                    .into_any_element(),
            ),
            _ if !works || outdated => Some(
                button("set-up", if outdated { text.update } else { text.set_up }, Kind::Primary, &t)
                    .on_click(cx.listener(|this, _, _, cx| this.install(Component::Rule, true, cx)))
                    .into_any_element(),
            ),
            _ => None,
        };
        let card = Card {
            id: 0,
            name: text.device_access,
            need: Need::Required,
            desc: text.device_access_desc,
            works,
            status,
            note,
            action,
            home: Some("https://github.com/Brunovncs/OpenController#linux"),
        };
        group().child(self.requirement(card, text)).into_any_element()
    }
}
