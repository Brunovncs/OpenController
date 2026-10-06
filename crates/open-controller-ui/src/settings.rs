//! The settings: the two options there are, the drivers Open Controller and some controllers
//! need, each installable from here when the user asks, and what is running.

use crate::theme::{icon as glyph, radius};
use crate::ui::{Install, MainView};
use crate::widgets::{Kind, body, button, caption, card, chip, display, group, icon, row, section, strong, switch, title};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, px};
use open_controller_core::drivers::{self, Component, Outcome};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::ipc::ToTray;
use open_controller_core::{Driver, Snapshot};

#[derive(Clone, Copy)]
enum Need {
    Required,
    Recommended,
    Optional,
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

fn drivers_line(snap: &Snapshot) -> String {
    let mut parts = vec![format!("Open Controller {}", env!("CARGO_PKG_VERSION"))];
    if !snap.sdl_version.is_empty() {
        parts.push(format!("SDL {}", snap.sdl_version));
    }
    parts.join("  ·  ")
}

impl MainView {
    fn driver_card(&self, c: Component, need: Need, desc: &'static str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let model = self.model.read(cx);
        let text = model.text;
        let pkg = drivers::package(c);
        let (works, version) = state(c, &model.snapshot, self.installed.get(&c).and_then(|v| v.as_ref()));
        let outdated = version.as_deref().is_some_and(|v| drivers::outdated(c, v));
        let (tag, tag_fg, tag_bg) = match need {
            Need::Required => (text.required, t.accent, t.accent_soft),
            Need::Recommended => (text.recommended, t.text2, t.control),
            Need::Optional => (text.optional, t.text3, t.control),
        };
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
        let home = pkg.home;
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
                    .bg(match (works, need) {
                        (true, _) => t.success.opacity(0.14),
                        (false, Need::Optional) => t.control,
                        (false, _) => t.caution.opacity(0.14),
                    })
                    .child(match (works, need) {
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
                    .child(div().flex().items_center().gap(px(8.)).child(strong(pkg.name, t.text)).child(chip(tag, tag_fg, tag_bg)))
                    .child(caption(desc, t.text2))
                    .child(caption(status, if works { t.text2 } else { t.text3 }))
                    .when_some(note, |d, (n, color)| d.child(caption(n, color)))
                    .child(
                        div()
                            .id(("home", c as usize))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .pt(px(2.))
                            .cursor_pointer()
                            .text_color(t.accent)
                            .on_click(move |_, _, cx| cx.open_url(home))
                            .child(caption(text.project_page, t.accent))
                            .child(icon(glyph::OPEN, 10., t.accent)),
                    ),
            )
            .children(action)
    }

    pub fn render_settings(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let model = self.model.read(cx);
        let text: &'static Text = model.text;
        let prefs = model.prefs;
        let connected = model.connected;
        let snap = model.snapshot.clone();

        let toggle =
            |id: &'static str, label: &'static str, hint: &'static str, on: bool, req: fn(bool) -> ToTray, cx: &mut Context<Self>| {
                row(&t)
                    .id(id)
                    .cursor_pointer()
                    .hover(move |s| s.bg(t.layer_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.send(req(!on), cx)))
                    .child(div().flex().flex_col().flex_1().gap(px(2.)).child(body(label, t.text)).child(caption(hint, t.text2)))
                    .child(switch(on, text.on, text.off, &t))
            };
        let options = group()
            .child(toggle("hide", text.hide_originals, text.hide_originals_hint, prefs.hide_originals, ToTray::SetHiding, cx))
            .child(toggle("autostart", text.start_with_windows, text.start_with_windows_hint, prefs.autostart, ToTray::SetAutostart, cx));

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

        let about = row(&t)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(2.))
                    .child(body(drivers_line(&snap), t.text))
                    .child(caption(text.quit_hint, t.text2)),
            )
            .when(connected, |d| {
                d.child(button("quit", text.quit, Kind::Standard, &t).on_click(cx.listener(|this, _, _, cx| this.send(ToTray::Quit, cx))))
            });

        div()
            .flex()
            .flex_col()
            .max_w(px(860.))
            .child(display(text.settings, t.text).pb(px(4.)))
            .child(section(text.options, &t).pt(px(16.)))
            .child(options)
            .child(section(text.requirements, &t))
            .child(caption(text.requirements_hint, t.text2).pb(px(12.)))
            .child(required)
            .child(div().h(px(12.)))
            .child(ps3)
            .child(caption(text.install_note, t.text3).pt(px(10.)))
            .child(section(text.about, &t))
            .child(about)
            .into_any_element()
    }
}
