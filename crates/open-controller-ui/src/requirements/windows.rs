//! Windows: the drivers Open Controller and some controllers need, each installable from here
//! when the user asks.

use super::{Card, Need};
use crate::theme::icon as glyph;
use crate::ui::{Install, MainView};
use crate::widgets::{Kind, body, button, caption, card, group, icon, title};
use gpui::{AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::drivers::{self, PACKAGES};
pub use open_controller_core::drivers::{Component, Outcome};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::{Driver, Snapshot};
use std::path::Path;

pub fn hint(text: &Text) -> &'static str {
    text.requirements_hint
}

/// What Programs and Features says is installed.
pub fn installed() -> Vec<(Component, Option<String>)> {
    PACKAGES.iter().map(|p| (p.component, drivers::installed(p.component))).collect()
}

pub fn install(c: Component, dir: &Path, silent: bool) -> Result<Outcome, String> {
    drivers::install(c, dir, silent)
}

/// What keeps controllers from working fully, for the banner on the home screen.
pub fn attention(snap: &Snapshot, text: &'static Text) -> Option<&'static str> {
    if !matches!(snap.vigem, Driver::Ready { .. }) {
        Some(text.vigem_missing)
    } else if snap.hiding && !matches!(snap.hidhide, Driver::Ready { .. }) {
        Some(text.hidhide_missing)
    } else {
        None
    }
}

/// Whether a driver works, as far as can be told: the engine's own view for the two it uses,
/// Programs and Features for the rest.
fn state(c: Component, snap: &Snapshot, installed: Option<&String>) -> (bool, Option<String>) {
    let live = match c {
        Component::ViGEmBus => Some(&snap.vigem),
        Component::HidHide => Some(&snap.hidhide),
        _ => None,
    };
    // The product version, which is what releases are numbered by: ViGEmBus 1.22.0 carries a
    // driver file of version 1.21.442.0.
    match live {
        Some(Driver::Ready { version }) => (true, installed.cloned().or_else(|| version.clone())),
        Some(_) => (false, installed.cloned()),
        None => (installed.is_some(), installed.cloned()),
    }
}

impl MainView {
    fn driver_card(&self, c: Component, need: Need, desc: &'static str, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let model = self.model.read(cx);
        let text = model.text;
        let pkg = drivers::package(c);
        let (works, version) = state(c, &model.snapshot, self.installed.get(&c).and_then(|v| v.as_ref()));
        let outdated = version.as_deref().is_some_and(|v| drivers::outdated(c, v));
        let status = match (&version, works) {
            (Some(v), _) if !v.is_empty() => fill(text.installed_version, v),
            (_, true) => text.installed.to_string(),
            _ => text.not_installed.to_string(),
        };
        let progress = self.installs.get(&c).cloned();
        let note = match &progress {
            Some(Install::Done(Outcome::RestartNeeded)) => Some((text.restart_needed.to_string(), t.caution)),
            // HidHide's driver starts only after a restart, whatever its installer says.
            Some(Install::Done(Outcome::Installed)) if c == Component::HidHide && !works => {
                Some((text.restart_needed.to_string(), t.caution))
            }
            Some(Install::Done(Outcome::Cancelled)) => Some((text.install_cancelled.to_string(), t.text2)),
            Some(Install::Failed(e)) => Some((fill(text.install_failed, e), t.critical)),
            _ if outdated => Some((fill(text.update_available, pkg.version), t.text2)),
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
            _ if !works || version.is_none() => Some(
                button(("install", c as usize), text.install, Kind::Primary, &t)
                    .on_click(cx.listener(move |this, _, _, cx| this.install(c, true, cx)))
                    .into_any_element(),
            ),
            // An update runs the installer with its own windows: HidHide's may ask to restart
            // halfway.
            _ if outdated => Some(
                button(("update", c as usize), text.update, Kind::Standard, &t)
                    .on_click(cx.listener(move |this, _, _, cx| this.install(c, false, cx)))
                    .into_any_element(),
            ),
            _ => None,
        };
        let card = Card { id: c as usize, name: pkg.name, need, desc, works, status, note, action, home: Some(pkg.home) };
        self.requirement(card, text)
    }

    pub fn render_requirements(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let text = self.model.read(cx).text;
        let required = group().child(self.driver_card(Component::ViGEmBus, Need::Required, text.vigem_desc, cx)).child(self.driver_card(
            Component::HidHide,
            Need::Recommended,
            text.hidhide_desc,
            cx,
        ));
        let ps3 = card(&t)
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(12.))
                    .pt(px(12.))
                    .pb(px(8.))
                    .child(title(text.ps3_title, t.text))
                    .child(caption(text.ps3_desc, t.text2)),
            )
            .child(self.driver_card(Component::DsHidMini, Need::Optional, text.dshidmini_desc, cx).border_0())
            .child(self.driver_card(Component::BthPs3, Need::Optional, text.bthps3_desc, cx).border_0());
        div()
            .flex()
            .flex_col()
            .child(required)
            .child(div().h(px(12.)))
            .child(ps3)
            .child(caption(text.install_note, t.text3).pt(px(10.)))
            .into_any_element()
    }
}
