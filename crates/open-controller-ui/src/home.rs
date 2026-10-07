//! The first screen: every controller as a tile with its drawing, live, and a tile to connect
//! another one. Whatever keeps controllers from working shows above them.

use crate::art;
use crate::theme::{Theme, icon as glyph, layout, radius};
use crate::ui::MainView;
use crate::widgets::{Kind, battery, body, button, caption, chip, display, icon, icon_label_button, player_mark, title};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Div, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use open_controller_core::device::{Link, Power};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::ipc::ToTray;
use open_controller_core::{PadKey, PadView, Role};

const TILE_MIN_W: f32 = 248.;
const TILE_H: f32 = 280.;
const GAP: f32 = 16.;

/// Where pairing a controller is done, on systems that have one page for it.
const BLUETOOTH_SETTINGS: Option<&str> = if cfg!(windows) {
    Some("ms-settings:bluetooth")
} else if cfg!(target_os = "macos") {
    Some("x-apple.systempreferences:com.apple.BluetoothSettings")
} else {
    None
};
const ART_SCALE: f32 = 0.74;

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
        Role::KeptNative => text.kept_native.to_string(),
    }
}

/// The battery as a glyph and a percentage, with a bolt while it charges, or `None` when the
/// controller does not say.
pub fn power(pad: &PadView, text: &Text, t: &Theme) -> Option<Div> {
    let (level, charging) = match pad.power {
        Power::Battery(Some(p)) => (Some(p), false),
        Power::Charging(p) => (p, true),
        Power::Charged => (Some(100), true),
        _ => return None,
    };
    let low = !charging && level.is_some_and(|l| l <= 15);
    let color = if low { t.critical } else { t.text2 };
    let label = match level {
        Some(p) => format!("{p}%"),
        None => text.charging.to_string(),
    };
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .when(charging, |d| d.child(icon(glyph::BOLT, 12., t.success)))
            .when_some(level, |d, l| d.child(battery(l, low, t)))
            .child(caption(label, color)),
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

/// A controller being dragged onto another, to trade players with it; also its drag image.
#[derive(Clone)]
pub struct DraggedPad {
    key: PadKey,
    name: String,
    theme: Theme,
}

impl Render for DraggedPad {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .h(px(36.))
            .rounded(px(radius::CONTROL))
            .bg(t.layer)
            .border_1()
            .border_color(t.accent)
            .shadow_lg()
            .text_color(t.text)
            .child(icon(glyph::STICKS, 14., t.accent))
            .child(caption(self.name.clone(), t.text))
    }
}

impl MainView {
    fn tile(&self, i: usize, pad: &PadView, width: f32, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
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
            // Clear of the player badge in the corner.
            .pt(px(22.))
            .rounded_t(px(radius::TILE))
            .relative()
            .child(div().when(away, |d| d.opacity(0.45)).child(art::controller(pad, ART_SCALE, &t)))
            .child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(px(12.))
                    .max_w(px(width - 24.))
                    .overflow_hidden()
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
                    .child(caption(status(pad, text), if away { t.text3 } else { t.text2 }).min_w(px(0.)).truncate()),
            );
        // Controllers with a virtual controller can trade players by dragging one onto another.
        let swappable = matches!(key, PadKey::Slot(_)) && pad.role != Role::KeptNative;
        let dragged = DraggedPad { key, name: pad.name.clone(), theme: t };
        div()
            .id(("tile", i))
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .h(px(TILE_H))
            .overflow_hidden()
            .rounded(px(radius::TILE))
            .bg(t.layer)
            .border_1()
            .border_color(t.stroke)
            .cursor_pointer()
            .hover(move |s| s.bg(t.layer_hover).border_color(t.stroke_strong))
            .on_click(cx.listener(move |this, _, _, cx| this.open(key, cx)))
            .when(swappable, |d| {
                d.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<DraggedPad>(move |s, d, _, _| if d.key != key { s.border_color(t.accent).bg(t.accent_soft) } else { s })
                    .on_drop(cx.listener(move |this, d: &DraggedPad, _, cx| {
                        if d.key != key {
                            this.send(ToTray::SwapPlayers(d.key, key), cx);
                        }
                    }))
            })
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
                            .child(caption(link_line(pad, text), t.text3).min_w(px(0.)).truncate())
                            .children(power(pad, text, &t)),
                    ),
            )
            .into_any_element()
    }

    /// Nothing connected: what to do about it, in the middle of the page.
    fn empty_state(&self, text: &'static Text) -> AnyElement {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .py(px(56.))
            .px(px(24.))
            .rounded(px(radius::TILE))
            .border_1()
            .border_dashed()
            .border_color(t.stroke_strong)
            .child(div().flex().items_center().justify_center().size(px(56.)).rounded_full().bg(t.control).child(icon(
                glyph::BUTTONS,
                24.,
                t.text2,
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .max_w(px(440.))
                    .child(title(text.empty_title, t.text))
                    .child(body(text.empty_body, t.text2).text_center()),
            )
            .children(BLUETOOTH_SETTINGS.map(|url| {
                icon_label_button("bluetooth-empty", glyph::OPEN, text.bluetooth_settings, Kind::Standard, &t)
                    .on_click(move |_, _, cx| cx.open_url(url))
            }))
            .into_any_element()
    }

    pub fn render_home(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let model = self.model.read(cx);
        let text = model.text;
        let snap = model.snapshot.clone();
        let connected = model.connected;

        let problem = if !connected {
            Some(text.tray_off.to_string())
        } else if let Some(e) = &snap.sdl_error {
            Some(format!("SDL: {e}"))
        } else {
            crate::requirements::attention(&snap, text).map(str::to_string)
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
        // As many columns as fit, the tiles stretched to fill the row.
        let width = (f32::from(window.viewport_size().width) - 2. * layout::GUTTER).min(layout::MAX_W - 2. * layout::GUTTER);
        let cols = ((width + GAP) / (TILE_MIN_W + GAP)).floor().max(1.);
        let tile_w = ((width - GAP * (cols - 1.)) / cols).floor();
        let mut grid = div().flex().flex_wrap().gap(px(GAP));
        for (i, pad) in snap.pads.iter().enumerate() {
            grid = grid.child(self.tile(i, pad, tile_w, text, cx));
        }
        let connect = BLUETOOTH_SETTINGS.filter(|_| !snap.pads.is_empty()).map(|url| {
            icon_label_button("connect", glyph::ADD, text.connect_tile, Kind::Standard, &t).on_click(move |_, _, cx| cx.open_url(url))
        });
        let swap_hint = snap.pads.iter().filter(|p| matches!(p.key, PadKey::Slot(_)) && p.role != Role::KeptNative).count() >= 2;

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .pb(if swap_hint { px(4.) } else { px(20.) })
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(12.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(display(text.controllers, t.text))
                            .child(caption(count, t.text3).pb(px(6.))),
                    )
                    .children(connect),
            )
            .when(swap_hint, |d| d.child(caption(text.swap_hint, t.text3).pb(px(16.))))
            .children(banner)
            .when(snap.pads.is_empty(), |d| d.child(self.empty_state(text)))
            .when(!snap.pads.is_empty(), |d| d.child(grid))
            .into_any_element()
    }
}
