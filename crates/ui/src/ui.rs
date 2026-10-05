//! The window: one card per controller with its player number, connection, battery and live
//! input, the two driver notices when something is missing, and the few preferences there are.

use crate::Model;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render, Rgba, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px, rgb, rgba,
};
use open_controller_core::device::{Brand, Link, Power};
use open_controller_core::i18n::Text;
use open_controller_core::ipc::ToTray;
use open_controller_core::mapping::{PadState, axis, button};
use open_controller_core::{Driver, PadView, Role, Snapshot};

const VIGEM_URL: &str = "https://github.com/nefarius/ViGEmBus/releases/latest";
const HIDHIDE_URL: &str = "https://github.com/nefarius/HidHide/releases/latest";

mod color {
    pub const BG: u32 = 0x0b0b0e;
    pub const SURFACE: u32 = 0x141419;
    pub const SURFACE_HI: u32 = 0x1c1c23;
    pub const BORDER: u32 = 0x26262f;
    pub const TEXT: u32 = 0xf2f2f5;
    pub const DIM: u32 = 0x9a9aa6;
    pub const FAINT: u32 = 0x5c5c68;
    pub const OK: u32 = 0x2fbf71;
    pub const WARN: u32 = 0xf0a020;
    /// Player colours, in the order a PlayStation console lights them.
    pub const PLAYERS: [u32; 4] = [0x4c7dff, 0xe5484d, 0x2fbf71, 0xff5fa2];
}

fn c(hex: u32) -> Rgba {
    rgb(hex)
}

fn player_color(p: Option<u8>) -> Rgba {
    p.map(|i| c(color::PLAYERS[i as usize % 4])).unwrap_or(c(color::FAINT))
}

pub struct MainView {
    model: Entity<Model>,
    _observe: Subscription,
}

impl MainView {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> MainView {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        MainView { model, _observe: observe }
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let t = model.text;
        let snap = &model.snapshot;
        let problems = !model.connected || matches!(snap.vigem, Driver::Missing | Driver::Failed(_)) || snap.sdl_error.is_some();

        let mut notices: Vec<AnyElement> = Vec::new();
        if !model.connected {
            notices.push(notice(t.tray_off.into(), None));
        }
        if let Some(e) = &snap.sdl_error {
            notices.push(notice(format!("SDL: {e}"), None));
        }
        match &snap.vigem {
            Driver::Missing if model.connected => notices.push(notice(t.vigem_missing.into(), Some(VIGEM_URL))),
            Driver::Missing => {}
            Driver::Failed(e) => notices.push(notice(format!("ViGEmBus: {e}"), Some(VIGEM_URL))),
            Driver::Ready { .. } => {}
        }
        if snap.hiding {
            match &snap.hidhide {
                Driver::Missing => notices.push(notice(t.hidhide_missing.into(), Some(HIDHIDE_URL))),
                Driver::Failed(e) => notices.push(notice(format!("HidHide: {e}"), Some(HIDHIDE_URL))),
                Driver::Ready { .. } => {}
            }
        }

        let count = match snap.pads.len() {
            1 => t.controllers_one.to_string(),
            n => format!("{n} {}", t.controllers_many),
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child("Open Controller"))
                    .child(div().text_size(px(13.)).text_color(c(color::DIM)).child(t.tagline)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(999.))
                    .bg(c(color::SURFACE))
                    .border_1()
                    .border_color(c(color::BORDER))
                    .child(div().size(px(8.)).rounded(px(4.)).bg(c(if problems { color::WARN } else { color::OK })))
                    .child(div().text_size(px(12.)).text_color(c(color::DIM)).child(if problems { t.problem } else { t.ready }))
                    .when(!snap.pads.is_empty(), |d| {
                        d.child(div().text_size(px(12.)).text_color(c(color::FAINT)).child(format!("· {count}")))
                    }),
            );

        let body: AnyElement = if snap.pads.is_empty() {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(t.empty_title))
                .child(div().text_size(px(13.)).text_color(c(color::DIM)).child(t.empty_body))
                .into_any_element()
        } else {
            div()
                .id("pads")
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(10.))
                .overflow_y_scroll()
                .children(snap.pads.iter().enumerate().map(|(i, p)| pad_card(i, p, t, &self.model)))
                .into_any_element()
        };

        let settings = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .pt(px(12.))
            .border_t_1()
            .border_color(c(color::BORDER))
            .child(toggle(
                "hide",
                t.hide_originals,
                Some(t.hide_originals_hint),
                model.prefs.hide_originals,
                &self.model,
                ToTray::SetHiding,
            ))
            .child(toggle("autostart", t.start_with_windows, None, model.prefs.autostart, &self.model, ToTray::SetAutostart))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .pt(px(8.))
                    .text_size(px(11.))
                    .text_color(c(color::FAINT))
                    .child(div().flex().flex_col().gap(px(2.)).child(t.quit_hint).child(drivers_line(snap)))
                    .when(model.connected, |d| {
                        let m = self.model.clone();
                        d.child(
                            div()
                                .id("quit")
                                .flex_none()
                                .px(px(10.))
                                .py(px(5.))
                                .rounded(px(8.))
                                .border_1()
                                .border_color(c(color::BORDER))
                                .text_color(c(color::DIM))
                                .cursor_pointer()
                                .hover(|s| s.bg(c(color::SURFACE_HI)).text_color(c(color::TEXT)))
                                .child(t.quit)
                                .on_click(move |_, _, cx| m.read(cx).send(ToTray::Quit)),
                        )
                    }),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(20.))
            .bg(c(color::BG))
            .text_color(c(color::TEXT))
            .font_family("Segoe UI")
            .child(header)
            .children(notices)
            .child(body)
            .child(settings)
    }
}

fn drivers_line(snap: &Snapshot) -> String {
    let vigem = match &snap.vigem {
        Driver::Ready { version: Some(v) } => format!("ViGEmBus {v}"),
        Driver::Ready { version: None } => "ViGEmBus".into(),
        _ => "ViGEmBus –".into(),
    };
    let hidhide = match &snap.hidhide {
        Driver::Ready { .. } => "HidHide",
        _ => "HidHide –",
    };
    let sdl = if snap.sdl_version.is_empty() { "SDL –".to_string() } else { format!("SDL {}", snap.sdl_version) };
    format!("{vigem}  ·  {hidhide}  ·  {sdl}  ·  v{}", env!("CARGO_PKG_VERSION"))
}

fn notice(message: String, url: Option<&'static str>) -> AnyElement {
    let t = crate::i18n::text();
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(10.))
        .bg(rgba(0xf0a0201a))
        .border_1()
        .border_color(rgba(0xf0a02055))
        .child(div().flex_1().text_size(px(13.)).child(message))
        .when_some(url, |d, url| {
            d.child(
                div()
                    .id(SharedString::from(url))
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .bg(c(color::WARN))
                    .text_color(c(color::BG))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(t.download)
                    .on_click(move |_, _, cx| cx.open_url(url)),
            )
        })
        .into_any_element()
}

fn toggle(
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    on: bool,
    model: &Entity<Model>,
    req: fn(bool) -> ToTray,
) -> AnyElement {
    let model = model.clone();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(6.))
        .cursor_pointer()
        .on_click(move |_, _, cx| model.read(cx).send(req(!on)))
        .child(
            div()
                .flex_none()
                .w(px(34.))
                .h(px(20.))
                .rounded(px(10.))
                .p(px(2.))
                .bg(c(if on { color::OK } else { color::SURFACE_HI }))
                .border_1()
                .border_color(c(if on { color::OK } else { color::BORDER }))
                .flex()
                .when(on, |d| d.justify_end())
                .child(div().size(px(14.)).rounded(px(7.)).bg(c(color::TEXT))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_size(px(13.)).child(label))
                .when_some(hint, |d, h| d.child(div().text_size(px(11.)).text_color(c(color::FAINT)).child(h))),
        )
        .into_any_element()
}

fn link_name(t: &Text, l: Link) -> &'static str {
    match l {
        Link::Usb => t.usb,
        Link::Bluetooth => t.bluetooth,
        Link::Dongle => t.dongle,
        Link::Wireless => t.wireless,
        Link::Unknown => "",
    }
}

fn brand_mark(b: Brand) -> &'static str {
    match b {
        Brand::PlayStation => "PS",
        Brand::Xbox => "XB",
        Brand::Nintendo => "NS",
        Brand::EightBitDo => "8B",
        Brand::Other => "··",
    }
}

fn pad_card(i: usize, p: &PadView, t: &'static Text, model: &Entity<Model>) -> AnyElement {
    let (player, status, active) = match &p.role {
        Role::Virtual { player: Some(n) } => (Some(*n), format!("{} {}", t.playing_as, n + 1), true),
        Role::Virtual { player: None } => (None, t.not_xinput.to_string(), true),
        Role::Waiting { player, remaining } => (*player, format!("{} · {} s", t.waiting, remaining.as_secs() + 1), false),
        Role::Native { player } => (*player, t.native.to_string(), true),
        Role::Unmapped => (None, t.unmapped.to_string(), false),
        Role::Unavailable => (None, t.unavailable.to_string(), false),
    };
    let virtualised = matches!(p.role, Role::Virtual { .. } | Role::Waiting { .. });
    let status = if virtualised { format!("{status} · {}", if p.hidden { t.hidden } else { t.visible }) } else { status };
    let accent = player_color(player);
    let badge_text = match (&p.role, player) {
        (Role::Native { .. }, Some(n)) => format!("X{}", n + 1),
        (_, Some(n)) => format!("P{}", n + 1),
        _ => brand_mark(p.brand).to_string(),
    };
    let key = p.key;
    let identify = model.clone();
    let power_off = model.clone();

    let links = p.links.iter().filter(|l| **l != Link::Unknown).enumerate().map(|(j, l)| {
        div()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(6.))
            .bg(c(if j == 0 { color::SURFACE_HI } else { color::SURFACE }))
            .border_1()
            .border_color(c(color::BORDER))
            .text_size(px(11.))
            .text_color(c(if j == 0 { color::TEXT } else { color::FAINT }))
            .child(link_name(t, *l))
    });

    div()
        .id(("pad", i))
        .flex()
        .gap(px(16.))
        .p(px(14.))
        .rounded(px(14.))
        .bg(c(color::SURFACE))
        .border_1()
        .border_color(c(color::BORDER))
        .when(!active, |d| d.opacity(0.6))
        .hover(|s| s.border_color(c(color::FAINT)))
        .cursor_pointer()
        .on_click(move |_, _, cx| identify.read(cx).send(ToTray::Identify(key)))
        .child(
            div()
                .flex_none()
                .size(px(52.))
                .rounded(px(12.))
                .bg(c(color::BG))
                .border_2()
                .border_color(accent)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(17.))
                .font_weight(FontWeight::BOLD)
                .text_color(accent)
                .child(badge_text),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(p.name.clone()))
                        .children(links)
                        .child(battery(p.power, t)),
                )
                .child(div().text_size(px(12.)).text_color(c(color::DIM)).child(status))
                .child(live_input(&p.input, accent)),
        )
        .when(p.can_power_off, |d| {
            d.child(
                div()
                    .id(("off", i))
                    .flex_none()
                    .self_center()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(color::BORDER))
                    .text_size(px(12.))
                    .text_color(c(color::DIM))
                    .hover(|s| s.bg(c(color::SURFACE_HI)).text_color(c(color::TEXT)))
                    .child(t.turn_off)
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        power_off.read(cx).send(ToTray::PowerOff(key));
                    }),
            )
        })
        .into_any_element()
}

fn battery(power: Power, t: &Text) -> AnyElement {
    let (level, label) = match power {
        Power::Battery(Some(p)) => (Some(p), format!("{p}%")),
        Power::Charging(Some(p)) => (Some(p), format!("{p}% · {}", t.charging)),
        Power::Charging(None) => (None, t.charging.to_string()),
        Power::Charged => (Some(100), t.charged.to_string()),
        Power::Wired => (None, t.wired_power.to_string()),
        Power::Battery(None) | Power::Unknown => return div().into_any_element(),
    };
    let fill = level.map(|l| if l <= 15 { color::WARN } else { color::OK }).unwrap_or(color::FAINT);
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().text_size(px(11.)).text_color(c(color::DIM)).child(label))
        .when_some(level, |d, l| {
            d.child(
                div()
                    .w(px(22.))
                    .h(px(11.))
                    .p(px(1.5))
                    .rounded(px(3.))
                    .border_1()
                    .border_color(c(color::FAINT))
                    .child(div().h_full().w(px(17. * l as f32 / 100.)).rounded(px(1.5)).bg(c(fill))),
            )
        })
        .into_any_element()
}

/// The sticks, triggers, face buttons and d-pad as they are now.
fn live_input(s: &PadState, accent: Rgba) -> AnyElement {
    let lit = |on: bool| -> Hsla { if on { accent.into() } else { c(color::SURFACE_HI).into() } };
    let stick = |x: i16, y: i16, pressed: bool| {
        let r = 15.0;
        let (dx, dy) = (x as f32 / 32768. * r, y as f32 / 32768. * r);
        div()
            .relative()
            .size(px(38.))
            .rounded(px(19.))
            .bg(c(color::BG))
            .border_1()
            .border_color(if pressed { accent } else { c(color::BORDER) })
            .child(div().absolute().left(px(15. + dx)).top(px(15. + dy)).size(px(8.)).rounded(px(4.)).bg(accent))
    };
    let trigger = |v: i16| {
        let h = 30. * (v.max(0) as f32 / 32767.);
        div()
            .w(px(6.))
            .h(px(30.))
            .rounded(px(3.))
            .bg(c(color::BG))
            .flex()
            .flex_col()
            .justify_end()
            .child(div().w_full().h(px(h)).rounded(px(3.)).bg(accent))
    };
    let dot = |on: bool| div().size(px(8.)).rounded(px(4.)).bg(lit(on));
    let diamond = |up: bool, right: bool, down: bool, left: bool| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(1.))
            .child(dot(up))
            .child(div().flex().gap(px(9.)).child(dot(left)).child(dot(right)))
            .child(dot(down))
    };
    let b = |i| s.pressed(i);
    let shoulder = |on: bool| div().w(px(16.)).h(px(4.)).rounded(px(2.)).bg(lit(on));
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .pt(px(2.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .items_center()
                .child(shoulder(b(button::LEFT_SHOULDER)))
                .child(trigger(s.axes[axis::LEFT_TRIGGER])),
        )
        .child(stick(s.axes[axis::LEFT_X], s.axes[axis::LEFT_Y], b(button::LEFT_STICK)))
        .child(diamond(b(button::DPAD_UP), b(button::DPAD_RIGHT), b(button::DPAD_DOWN), b(button::DPAD_LEFT)))
        .child(div().flex().flex_col().gap(px(4.)).child(dot(b(button::BACK))).child(dot(b(button::GUIDE))).child(dot(b(button::START))))
        .child(diamond(b(button::NORTH), b(button::EAST), b(button::SOUTH), b(button::WEST)))
        .child(stick(s.axes[axis::RIGHT_X], s.axes[axis::RIGHT_Y], b(button::RIGHT_STICK)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .items_center()
                .child(shoulder(b(button::RIGHT_SHOULDER)))
                .child(trigger(s.axes[axis::RIGHT_TRIGGER])),
        )
        .into_any_element()
}
