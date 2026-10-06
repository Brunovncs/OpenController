//! The settings: the options there are, what OpenController needs from the system (see
//! [`crate::requirements`]), and what is running.

use crate::requirements;
use crate::theme::icon as glyph;
use crate::ui::MainView;
use crate::widgets::{Kind, body, button, caption, display, group, icon_button, row, section, switch};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, px};
use open_controller_core::Snapshot;
use open_controller_core::i18n::Text;
use open_controller_core::ipc::ToTray;
use open_controller_core::platform::VIRTUAL_PADS;

fn drivers_line(snap: &Snapshot) -> String {
    let mut parts = vec![format!("OpenController {}", env!("CARGO_PKG_VERSION"))];
    if !snap.sdl_version.is_empty() {
        parts.push(format!("SDL {}", snap.sdl_version));
    }
    parts.join("  ·  ")
}

impl MainView {
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
        let (hide_hint, start_label, start_hint) = if cfg!(windows) {
            (text.hide_originals_hint, text.start_with_windows, text.start_with_windows_hint)
        } else {
            (text.hide_originals_hint_linux, text.start_with_system, text.start_with_system_hint)
        };
        // Where no controller is made, none is hidden either.
        let options = group()
            .when(VIRTUAL_PADS, |g| g.child(toggle("hide", text.hide_originals, hide_hint, prefs.hide_originals, ToTray::SetHiding, cx)))
            .child(toggle("autostart", start_label, start_hint, prefs.autostart, ToTray::SetAutostart, cx))
            .child(toggle("updates", text.check_updates, text.check_updates_hint, prefs.check_updates, ToTray::SetCheckUpdates, cx));

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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .pb(px(4.))
                    .child(icon_button("back", glyph::BACK, &t).on_click(cx.listener(|this, _, _, cx| this.home(cx))))
                    .child(display(text.settings, t.text)),
            )
            .child(section(text.options, &t).pt(px(16.)))
            .child(options)
            .child(section(text.requirements, &t))
            .child(caption(requirements::hint(text), t.text2).pb(px(12.)))
            .child(self.render_requirements(cx))
            .child(section(text.about, &t))
            .child(about)
            .into_any_element()
    }
}
