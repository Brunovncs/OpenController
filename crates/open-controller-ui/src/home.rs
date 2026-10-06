//! The first screen: every controller as a tile with its drawing, live, and a tile to connect
//! another one. Whatever keeps controllers from working shows above them.

use crate::art;
use crate::theme::{Theme, icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{Kind, battery, body, button, caption, chip, display, icon, player_mark, title};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, px};
use open_controller_core::device::{Link, Power};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::{Driver, PadView, Role};

const TILE_W: f32 = 248.;
const TILE_H: f32 = 296.;
const ART_SCALE: f32 = 0.66;

pub fn link_name(t: &Text, l: Link) -> &'static str {
    match l {
        Link::Usb => t.usb,
        Link::Bluetooth => t.bluetooth,
        Link::Dongle => t.dongle,
        Link::Wireless => t.wireless,
        Link::Unknown => "",
    }
}

/// The player number games give the controller, if any.
pub fn player(role: &Role) -> Option<u8> {
    match role {
        Role::Virtual { player } | Role::Waiting { player, .. } | Role::Native { player } => *player,
        _ => None,
    }
}

/// "Player 2", "Reconnecting, 4 s" and the like: what the controller is to games right now.
pub fn status(pad: &PadView, text: &Text) -> String {
    match &pad.role {
        Role::Virtual { player: Some(n) } | Role::Native { player: Some(n) } => fill(text.player, n + 1),
        Role::Virtual { player: None } => text.fifth.to_string(),
        Role::Native { player: None } => text.native.to_string(),
        Role::Waiting { remaining, .. } => fill(text.reconnecting, remaining.as_secs() + 1),
        Role::Unmapped => text.unmapped.to_string(),
        Role::Unavailable => text.unavailable.to_string(),
    }
}

/// The battery as a glyph and a percentage, or `None` when the controller does not say.
pub fn power(pad: &PadView, text: &Text, t: &Theme) -> Option<Div> {
    let (level, label) = match pad.power {
        Power::Battery(Some(p)) => (Some(p), format!("{p} %")),
        Power::Charging(Some(p)) => (Some(p), format!("{}, {p} %", text.charging)),
        Power::Charging(None) => (None, text.charging.to_string()),
        Power::Charged => (Some(100), text.charged.to_string()),
        _ => return None,
    };
    let low = level.is_some_and(|l| l <= 15) && matches!(pad.power, Power::Battery(_));
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .when_some(level, |d, l| d.child(battery(l, low, t)))
            .child(caption(label, if low { t.critical } else { t.text2 })),
    )
}

/// The connections in a line: "USB + Bluetooth".
pub fn link_line(pad: &PadView, text: &Text) -> String {
    pad.links.iter().map(|&l| link_name(text, l)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" + ")
}

/// The connections as chips: USB, Bluetooth, Receiver.
pub fn links(pad: &PadView, text: &Text, t: &Theme) -> Div {
    let names = pad.links.iter().map(|&l| link_name(text, l)).filter(|s| !s.is_empty());
    div().flex().flex_wrap().gap(px(6.)).children(names.map(|n| chip(n, t.text2, t.control)))
}

fn banner(message: String, action: Option<AnyElement>, t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .py(px(12.))
        .mb(px(20.))
        .rounded(px(radius::CARD))
        .bg(t.caution_layer)
        .border_1()
        .border_color(t.stroke)
        .child(icon(glyph::WARNING, 16., t.caution))
        .child(body(message, t.text).flex_1().min_w(px(0.)))
        .children(action)
}

impl MainView {
    fn tile(&self, i: usize, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let away = matches!(pad.role, Role::Waiting { .. } | Role::Unmapped | Role::Unavailable);
        let n = player(&pad.role);
        let stage = crate::widgets::stage(&t)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(184.))
            .rounded_t(px(radius::TILE))
            .relative()
            .child(div().when(away, |d| d.opacity(0.45)).child(art::controller(&pad.input, pad.art, pad.brand, ART_SCALE, &t)))
            .child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(8.))
                    .h(px(24.))
                    .rounded(px(radius::CHIP))
                    .bg(t.layer.opacity(0.85))
                    .border_1()
                    .border_color(t.stroke)
                    .when_some(n, |d, n| d.child(player_mark(Some(n), &t)))
                    .child(caption(status(pad, text), if away { t.text3 } else { t.text2 })),
            );
        div()
            .id(("tile", i))
            .flex()
            .flex_col()
            .flex_none()
            .w(px(TILE_W))
            .h(px(TILE_H))
            .rounded(px(radius::TILE))
            .bg(t.layer)
            .border_1()
            .border_color(t.stroke)
            .cursor_pointer()
            .hover(move |s| s.bg(t.layer_hover).border_color(t.stroke_strong))
            .on_click(cx.listener(move |this, _, _, cx| this.open(key, cx)))
            .child(stage)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .justify_between()
                    .px(px(16.))
                    .pt(px(12.))
                    .pb(px(14.))
                    .child(title(pad.name.clone(), if away { t.text2 } else { t.text }).line_clamp(2))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .child(caption(link_line(pad, text), t.text2).truncate())
                            .children(power(pad, text, &t)),
                    ),
            )
            .into_any_element()
    }

    fn connect_tile(&self, text: &'static Text) -> AnyElement {
        let t = self.theme;
        div()
            .id("connect")
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .w(px(TILE_W))
            .h(px(TILE_H))
            .rounded(px(radius::TILE))
            .border_1()
            .border_dashed()
            .border_color(t.stroke_strong)
            .cursor_pointer()
            .hover(move |s| s.bg(t.layer).border_color(t.text3))
            .on_click(|_, _, cx| cx.open_url("ms-settings:bluetooth"))
            .child(div().flex().items_center().justify_center().size(px(48.)).rounded_full().bg(t.control).child(icon(
                glyph::ADD,
                18.,
                t.text2,
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .child(title(text.connect_tile, t.text))
                    .child(caption(text.connect_tile_hint, t.text3)),
            )
            .into_any_element()
    }

    pub fn render_home(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let model = self.model.read(cx);
        let text = model.text;
        let snap = model.snapshot.clone();
        let connected = model.connected;

        let problem = if !connected {
            Some(text.tray_off.to_string())
        } else if let Some(e) = &snap.sdl_error {
            Some(format!("SDL: {e}"))
        } else if !matches!(snap.vigem, Driver::Ready { .. }) {
            Some(text.vigem_missing.to_string())
        } else if snap.hiding && !matches!(snap.hidhide, Driver::Ready { .. }) {
            Some(text.hidhide_missing.to_string())
        } else {
            None
        };
        let banner = problem.map(|message| {
            let action = connected.then(|| {
                button("requirements", text.see_requirements, Kind::Standard, &t)
                    .on_click(cx.listener(|this, _, _, cx| this.settings(cx)))
                    .into_any_element()
            });
            banner(message, action, &t)
        });

        let count = match snap.pads.len() {
            0 => String::new(),
            1 => text.controllers_one.to_string(),
            n => format!("{n} {}", text.controllers_many),
        };
        let mut grid = div().flex().flex_wrap().gap(px(16.));
        for (i, pad) in snap.pads.iter().enumerate() {
            grid = grid.child(self.tile(i, pad, text, cx));
        }
        grid = grid.child(self.connect_tile(text));

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(12.))
                    .pb(px(20.))
                    .child(display(text.controllers, t.text))
                    .child(caption(count, t.text3).pb(px(6.))),
            )
            .children(banner)
            .when(snap.pads.is_empty(), |d| d.child(body(text.empty_body, t.text2).pb(px(16.))))
            .child(grid)
            .into_any_element()
    }
}
