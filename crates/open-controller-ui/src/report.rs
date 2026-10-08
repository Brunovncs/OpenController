//! "Report a problem": a dialog over the window that gathers what the resident process knows
//! about a controller, lets the user add what they know (the model, what goes wrong, an email),
//! records the buttons they press while it is open, shows the whole report on request and sends
//! it to the website, which emails it to the maintainer. Opened from a controller's Information
//! page, or from the notice a controller gets when it is not on the list.

use crate::art;
use crate::text_field::TextField;
use crate::theme::{Theme, icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{Kind, body, button, caption, icon, icon_label_button, strong, title};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, ClipboardItem, Context, Entity, Focusable, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::ipc::ToTray;
use open_controller_core::mapping::axis;
use open_controller_core::platform::VirtualDriver;
use open_controller_core::rating::{self, Rating};
use open_controller_core::report::{self, Diagnosis, Submission};
use open_controller_core::{Driver, PadKey, PadView, Role, Snapshot};
use std::fmt::Write as _;

const MONO: &str = "Consolas";

/// SDL's gamepad buttons, then the ones OpenController adds, by bit.
const BUTTON_NAMES: [&str; 41] = [
    "south (A/✕)",
    "east (B/○)",
    "west (X/□)",
    "north (Y/△)",
    "back",
    "guide",
    "start",
    "left stick",
    "right stick",
    "left shoulder",
    "right shoulder",
    "dpad up",
    "dpad down",
    "dpad left",
    "dpad right",
    "misc1",
    "right paddle 1",
    "left paddle 1",
    "right paddle 2",
    "left paddle 2",
    "touchpad",
    "misc2",
    "misc3",
    "misc4",
    "misc5",
    "misc6",
    "touch left",
    "touch right",
    "touch two fingers",
    "handheld 1",
    "handheld 2",
    "handheld 3",
    "handheld 4",
    "handheld 5",
    "handheld 6",
    "handheld 7",
    "handheld 8",
    "handheld 9",
    "handheld 10",
    "handheld 11",
    "handheld 12",
];
const AXIS_NAMES: [&str; axis::COUNT] = ["left x", "left y", "right x", "right y", "left trigger", "right trigger"];

#[derive(Clone, PartialEq, Eq)]
pub enum Stage {
    Editing,
    Sending,
    Sent,
    Failed(String),
}

pub struct Report {
    pub key: PadKey,
    /// Opened from the notice of a controller that is not on the list.
    pub unknown: bool,
    pub model: Entity<TextField>,
    pub message: Entity<TextField>,
    pub email: Entity<TextField>,
    pub show_data: bool,
    pub state: Stage,
    pub copied: bool,
    pub bad_email: bool,
    /// The system's name, read away from the interface thread when the dialog opens.
    pub system: Option<String>,
    /// The controller as it was when the dialog opened, kept should it disconnect.
    pub pad: PadView,
    /// Buttons seen pressed while the dialog is open, and how far each axis went.
    pub seen: u64,
    pub axes: [(i16, i16); axis::COUNT],
    /// Gyro readings seen while the dialog is open, the fastest turn on each axis in radians
    /// per second, and the most the gyro pushed the right stick.
    pub gyro_readings: u32,
    pub gyro_peak: [f32; 3],
    pub aim_peak: u16,
}

impl Report {
    /// Takes in what the controller sends while the dialog is open.
    pub fn observe(&mut self, pad: &PadView) {
        self.seen |= pad.input.buttons;
        for (r, &v) in self.axes.iter_mut().zip(pad.input.axes.iter()) {
            *r = (r.0.min(v), r.1.max(v));
        }
        if let Some(rate) = pad.gyro {
            self.gyro_readings += 1;
            for (peak, v) in self.gyro_peak.iter_mut().zip(rate) {
                *peak = peak.max(v.abs());
            }
        }
        self.aim_peak = self.aim_peak.max(pad.aim.0.unsigned_abs()).max(pad.aim.1.unsigned_abs());
    }

    fn arrived(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = (0..BUTTON_NAMES.len()).filter(|&b| self.seen & 1 << b != 0).map(|b| BUTTON_NAMES[b]).collect();
        out.extend(self.axes.iter().zip(AXIS_NAMES).filter(|((lo, hi), _)| moved(*lo, *hi)).map(|(_, n)| n));
        if turned(self.gyro_peak) {
            out.push("gyro");
        }
        out
    }
}

/// An axis counts as moved once it went past a fifth of its range: stick drift stays below that.
fn moved(lo: i16, hi: i16) -> bool {
    i32::from(hi) - i32::from(lo) > 13_000
}

/// The gyro counts as turned past 20 °/s on any axis: a hand holding still stays well below.
fn turned(peak: [f32; 3]) -> bool {
    peak.iter().any(|v| v.to_degrees() > 20.)
}

fn rating_label(text: &Text, r: Rating) -> &'static str {
    match r {
        Rating::Verified => text.rating_verified,
        Rating::Compatible => text.rating_compatible,
        Rating::Caveats => text.rating_caveats,
        Rating::Unsupported => text.rating_unsupported,
        Rating::Unknown => text.rating_unknown,
    }
}

pub fn rating_hint(text: &Text, r: Rating) -> &'static str {
    match r {
        Rating::Verified => text.rating_verified_hint,
        Rating::Compatible => text.rating_compatible_hint,
        Rating::Caveats => text.rating_caveats_hint,
        Rating::Unsupported => text.rating_unsupported_hint,
        Rating::Unknown => text.rating_unknown_hint,
    }
}

pub fn rating_of(pad: &PadView) -> Rating {
    if pad.role == Role::Unmapped && rating::rating(pad.vendor, pad.product, pad.family, pad.hint) != Rating::Unsupported {
        return Rating::Unknown;
    }
    rating::rating(pad.vendor, pad.product, pad.family, pad.hint)
}

/// The grade as a chip: green verified, the accent for compatible, amber for a note, red for not
/// supported, grey for unknown.
pub fn rating_chip(text: &Text, pad: &PadView, t: &Theme) -> gpui::Div {
    let r = rating_of(pad);
    let (fg, glyph) = match r {
        Rating::Verified => (t.success, glyph::CHECK),
        Rating::Compatible => (t.accent, glyph::CHECK),
        Rating::Caveats => (t.caution, glyph::INFO),
        Rating::Unsupported => (t.critical, glyph::WARNING),
        Rating::Unknown => (t.text2, glyph::INFO),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .h(px(22.))
        .px(px(8.))
        .rounded(px(radius::CHIP))
        .bg(fg.opacity(0.14))
        .child(icon(glyph, 12., fg))
        .child(caption(rating_label(text, r), fg).font_weight(FontWeight::SEMIBOLD))
}

fn driver(d: &Driver) -> String {
    match d {
        Driver::Ready { version } => format!("ready {}", version.as_deref().unwrap_or("")).trim().to_string(),
        Driver::Missing => "missing".into(),
        Driver::Failed(e) => format!("failed: {e}"),
        Driver::Unsupported => "not used here".into(),
        Driver::NoKernelSupport => "no uinput in this kernel".into(),
    }
}

/// The report's text, in English: it is read by the maintainer, not translated.
pub fn compose(r: &Report, diagnosis: Option<&Diagnosis>, snapshot: &Snapshot, lang: &str) -> String {
    let p = &r.pad;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "OpenController {} · {} · SDL {} · window in {lang}",
        env!("CARGO_PKG_VERSION"),
        r.system.as_deref().unwrap_or("system unknown"),
        snapshot.sdl_version
    );
    let _ = writeln!(s, "Why: {}", if r.unknown { "controller not on the list" } else { "problem reported by the user" });
    let _ = writeln!(s, "\n== As OpenController sees it");
    let _ = writeln!(s, "Name: {}", p.name);
    if let Some(m) = &p.profiles.model {
        let _ = writeln!(s, "Model chosen in the window: {m}");
    }
    let _ = writeln!(s, "USB id: {:04x}:{:04x}", p.vendor, p.product);
    let _ = writeln!(s, "Family: {:?} · drawn as: {} · rating: {:?}", p.family, art::look(p).key(), rating_of(p));
    let _ = writeln!(s, "Role: {:?} · links: {:?} · power: {:?} · hidden: {}", p.role, p.links, p.power, p.hidden);
    let _ = writeln!(s, "Extra buttons: {:?} · hint: {:?}", p.extras, p.hint);
    let _ = writeln!(s, "Features: {:?}", p.features);
    let _ = writeln!(
        s,
        "Drivers: {} {} · HidHide {} · hiding {}",
        if snapshot.bus == VirtualDriver::Viiper { "VIIPER" } else { "ViGEmBus" },
        driver(&snapshot.vigem),
        driver(&snapshot.hidhide),
        if snapshot.hiding { "on" } else { "off" }
    );
    match diagnosis {
        Some(d) => {
            let f = &d.facts;
            let _ = writeln!(s, "\n== As SDL sees it");
            let _ = writeln!(s, "Joystick name: {}", f.joystick_name);
            if let Some(n) = &f.gamepad_name {
                let _ = writeln!(s, "Gamepad name: {n}");
            }
            let _ = writeln!(s, "Ids: {:04x}:{:04x} · version {:04x} · firmware {:04x}", f.vendor, f.product, f.version, f.firmware);
            let _ = writeln!(s, "GUID: {}", f.guid);
            let _ = writeln!(s, "Path: {}", f.path);
            let _ = writeln!(
                s,
                "Type: {} (real {}) · joystick type: {} · connection: {}",
                f.gamepad_type, f.real_type, f.joystick_type, f.connection
            );
            let _ =
                writeln!(s, "Axes {} · buttons {} · hats {} · balls {} · touchpads {}", f.axes, f.buttons, f.hats, f.balls, f.touchpads);
            let _ = writeln!(s, "Sensors: {} · capabilities: {}", f.sensors.join(", "), f.capabilities.join(", "));
            let _ = writeln!(s, "Mapping: {}", f.mapping.as_deref().unwrap_or("none"));
            if !f.devices.is_empty() {
                let _ = writeln!(s, "\n== Device tree, from the controller up");
                for line in &f.devices {
                    let _ = writeln!(s, "{line}");
                }
            }
        }
        None => {
            let _ = writeln!(s, "\n(The resident process gave no details: the controller had left, or the window ran without it.)");
        }
    }
    let _ = writeln!(s, "\n== Input while this report was open");
    let arrived = r.arrived();
    let _ = writeln!(s, "{}", if arrived.is_empty() { "nothing".to_string() } else { arrived.join(", ") });
    for ((lo, hi), name) in r.axes.iter().zip(AXIS_NAMES) {
        if lo != hi {
            let _ = writeln!(s, "{name}: {lo} to {hi}");
        }
    }
    let gyro = diagnosis.and_then(|d| d.gyro.as_ref());
    if gyro.is_some() || r.gyro_readings > 0 {
        let _ = writeln!(s, "\n== Gyro");
        if let Some(g) = gyro {
            let yes = |b: bool| if b { "yes" } else { "no" };
            let _ = writeln!(s, "Mode: {} · reports asked for: {} · on in SDL: {}", g.mode, yes(g.wanted), yes(g.enabled));
            if let Some(e) = &g.error {
                let _ = writeln!(s, "Turning its reports on failed: {e}");
            }
        }
        let _ = writeln!(s, "{}", gyro_seen(r.gyro_readings, r.gyro_peak, r.aim_peak));
    }
    if let Some(d) = diagnosis.filter(|d| !d.log.is_empty()) {
        let _ = writeln!(s, "\n== Controllers coming and going");
        for line in &d.log {
            let _ = writeln!(s, "{line}");
        }
    }
    s.truncate(report::MAX_LEN);
    s
}

/// What the gyro sent while the dialog was open: readings that are all zero mean a controller
/// that answers for a gyro without reporting its motion.
fn gyro_seen(readings: u32, peak: [f32; 3], aim: u16) -> String {
    if readings == 0 {
        return "While this report was open: no readings".into();
    }
    let [pitch, yaw, roll] = peak.map(|v| v.to_degrees().round());
    let zero = if peak == [0.; 3] { ", all zero" } else { "" };
    format!(
        "While this report was open: {readings} readings{zero} · fastest turn: pitch {pitch} °/s, yaw {yaw} °/s, roll {roll} °/s · right stick pushed up to {aim}"
    )
}

fn plausible_email(e: &str) -> bool {
    let Some((user, host)) = e.split_once('@') else { return false };
    !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && !e.contains(char::is_whitespace)
}

impl MainView {
    pub fn open_report(&mut self, key: PadKey, unknown: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pad) = self.pad(key, cx) else { return };
        let text = self.model.read(cx).text;
        let model = cx.new(|cx| TextField::new(cx, false, 120).placeholder(text.report_model_placeholder));
        let message = cx.new(|cx| TextField::new(cx, true, 3000).placeholder(text.report_message_placeholder));
        let email = cx.new(|cx| TextField::new(cx, false, 200).placeholder("email@example.com"));
        model.focus_handle(cx).focus(window, cx);
        self.report = Some(Report {
            key,
            unknown,
            model,
            message,
            email,
            show_data: false,
            state: Stage::Editing,
            copied: false,
            bad_email: false,
            system: None,
            pad,
            seen: 0,
            axes: [(0, 0); axis::COUNT],
            gyro_readings: 0,
            gyro_peak: [0.; 3],
            aim_peak: 0,
        });
        if let Some(r) = self.report.as_mut() {
            // Resting triggers sit at 0 and sticks near it: the range starts where they are.
            for (a, &v) in r.axes.iter_mut().zip(r.pad.input.axes.iter()) {
                *a = (v, v);
            }
        }
        self.model.update(cx, |m, _| m.diagnosis = None);
        self.send(ToTray::Diagnose(key), cx);
        cx.spawn(async move |this, cx| {
            let system = cx.background_executor().spawn(async { report::system() }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(r) = this.report.as_mut() {
                    r.system = Some(system);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn close_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.report.as_ref().is_some_and(|r| r.state == Stage::Sending) {
            return;
        }
        self.report = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Tab and Shift+Tab move between the dialog's fields.
    pub fn report_tab(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = &self.report else { return };
        let fields = [r.model.focus_handle(cx), r.message.focus_handle(cx), r.email.focus_handle(cx)];
        let at = fields.iter().position(|f| f.is_focused(window));
        let next = match (at, back) {
            (Some(i), false) => (i + 1) % fields.len(),
            (Some(i), true) => (i + fields.len() - 1) % fields.len(),
            (None, _) => 0,
        };
        fields[next].focus(window, cx);
    }

    fn report_text(&self, cx: &Context<Self>) -> String {
        let Some(r) = &self.report else { return String::new() };
        let m = self.model.read(cx);
        let d = m.diagnosis.as_ref().filter(|(k, _)| *k == r.key).and_then(|(_, d)| d.as_deref());
        compose(r, d, &m.snapshot, m.prefs.lang.name())
    }

    fn send_report(&mut self, cx: &mut Context<Self>) {
        let report_text = self.report_text(cx);
        let Some(r) = self.report.as_mut() else { return };
        let email = r.email.read(cx).text().trim().to_string();
        r.bad_email = !email.is_empty() && !plausible_email(&email);
        if r.bad_email || r.state == Stage::Sending {
            cx.notify();
            return;
        }
        let submission = Submission {
            source: "app",
            version: env!("CARGO_PKG_VERSION").to_string(),
            system: r.system.clone().unwrap_or_default(),
            lang: format!("{:?}", self.model.read(cx).prefs.lang).to_lowercase(),
            controller: format!("{} ({:04x}:{:04x})", r.pad.name, r.pad.vendor, r.pad.product),
            model: r.model.read(cx).text().trim().to_string(),
            message: r.message.read(cx).text().trim().to_string(),
            email,
            report: report_text,
        };
        r.state = Stage::Sending;
        let demo = self.model.read(cx).demo && std::env::var_os("OPEN_CONTROLLER_REPORT_URL").is_none();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = if demo {
                // The demo's controllers are made up: nothing is sent unless a test address is set.
                cx.background_executor().timer(std::time::Duration::from_millis(700)).await;
                Ok(())
            } else {
                cx.background_executor().spawn(async move { report::send(&submission) }).await
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(r) = this.report.as_mut() {
                    r.state = match result {
                        Ok(()) => Stage::Sent,
                        Err(e) => Stage::Failed(e),
                    };
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn render_report(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let report_text = self.report.as_ref().filter(|r| r.show_data || r.copied).map(|_| self.report_text(cx));
        let r = self.report.as_ref()?;
        let t = self.theme;
        let m = self.model.read(cx);
        let text = m.text;
        let waiting = !m.diagnosis.as_ref().is_some_and(|(k, _)| *k == r.key);
        let gone = m.diagnosis.as_ref().is_some_and(|(k, d)| *k == r.key && d.is_none()) && m.connected;

        let label = |s: &'static str, optional: bool| {
            div()
                .flex()
                .items_baseline()
                .gap(px(6.))
                .child(strong(s, t.text))
                .when(optional, |d| d.child(caption(text.report_optional, t.text3)))
        };
        let field = |l: AnyElement, f: AnyElement| div().flex().flex_col().gap(px(6.)).child(l).child(f);

        let content: AnyElement = match &r.state {
            Stage::Sent => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(icon(glyph::CHECK, 20., t.success))
                        .child(title(text.report_sent_title, t.text)),
                )
                .child(body(text.report_sent_body, t.text2))
                .child(
                    div().flex().justify_end().pt(px(8.)).child(
                        button("report-close", text.report_close, Kind::Primary, &t)
                            .on_click(cx.listener(|this, _, w, cx| this.close_report(w, cx))),
                    ),
                )
                .into_any_element(),
            state => {
                let sending = *state == Stage::Sending;
                let arrived = r.arrived();
                let arrived_text = if arrived.is_empty() { text.report_nothing_pressed.to_string() } else { arrived.join(", ") };
                let failure = match state {
                    Stage::Failed(e) if e == "rate" => Some(text.report_rate.to_string()),
                    Stage::Failed(e) => Some(fill(text.report_failed, e)),
                    _ => None,
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(title(if r.unknown { text.report_title_unknown } else { text.report_title }, t.text))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(body(r.pad.name.clone(), t.text))
                            .child(caption(format!("{:04X}:{:04X}", r.pad.vendor, r.pad.product), t.text3)),
                    )
                    .child(body(text.report_intro, t.text2))
                    .when(gone, |d| d.child(body(text.report_gone, t.caution)))
                    .child(field(label(text.report_model, true).into_any_element(), r.model.clone().into_any_element()))
                    .child(field(label(text.report_message, true).into_any_element(), r.message.clone().into_any_element()))
                    .child(field(
                        label(text.report_email, true).into_any_element(),
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(r.email.clone())
                            .when(r.bad_email, |d| d.child(caption(text.report_bad_email, t.critical)))
                            .into_any_element(),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(radius::CONTROL))
                            .bg(t.accent_soft)
                            .child(
                                div().flex().items_start().gap(px(8.)).child(icon(glyph::BUTTONS, 14., t.accent).mt(px(3.))).child(
                                    body(if r.pad.features.motion { text.report_press_gyro } else { text.report_press }, t.text)
                                        .flex_1()
                                        .min_w(px(0.)),
                                ),
                            )
                            .child(caption(fill(text.report_pressed, arrived_text), t.text2)),
                    )
                    .child(
                        div()
                            .id("report-toggle")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .child(icon(if r.show_data { glyph::CHEVRON_DOWN } else { glyph::CHEVRON_RIGHT }, 14., t.accent))
                            .child(body(if r.show_data { text.report_hide } else { text.report_show }, t.accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(r) = this.report.as_mut() {
                                    r.show_data = !r.show_data;
                                }
                                cx.notify();
                            })),
                    )
                    .when(r.show_data, |d| {
                        d.child(
                            div()
                                .id("report-data")
                                .max_h(px(220.))
                                .overflow_y_scroll()
                                .px(px(12.))
                                .py(px(10.))
                                .rounded(px(radius::CONTROL))
                                .bg(t.base)
                                .border_1()
                                .border_color(t.stroke)
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .line_height(px(16.))
                                .text_color(t.text2)
                                .child(if waiting { text.report_reading.to_string() } else { report_text.clone().unwrap_or_default() }),
                        )
                    })
                    .when_some(failure, |d, f| d.child(body(f, t.critical)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .pt(px(4.))
                            .child(
                                (if r.copied {
                                    icon_label_button("report-copy", glyph::CHECK, text.report_copied, Kind::Subtle, &t)
                                } else {
                                    button("report-copy", text.report_copy, Kind::Subtle, &t)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let data = this.report_text(cx);
                                    cx.write_to_clipboard(ClipboardItem::new_string(data));
                                    if let Some(r) = this.report.as_mut() {
                                        r.copied = true;
                                    }
                                    cx.notify();
                                })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .child(
                                        button("report-cancel", text.report_cancel, Kind::Standard, &t)
                                            .on_click(cx.listener(|this, _, w, cx| this.close_report(w, cx))),
                                    )
                                    .child(
                                        button(
                                            "report-send",
                                            if sending { text.report_sending } else { text.report_send },
                                            Kind::Primary,
                                            &t,
                                        )
                                        .when(sending || waiting, |b| b.opacity(0.6).cursor_default())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !waiting {
                                                this.send_report(cx);
                                            }
                                        })),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
        };

        Some(
            div()
                .id("report-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .bg(gpui::black().opacity(0.45))
                .occlude()
                .child(
                    div()
                        .id("report-dialog")
                        .w_full()
                        .max_w(px(560.))
                        .max_h_full()
                        .overflow_y_scroll()
                        .p(px(24.))
                        .rounded(px(radius::TILE))
                        .bg(t.layer)
                        .border_1()
                        .border_color(t.stroke_strong)
                        .shadow_lg()
                        .child(content),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert!(plausible_email("a@b.co"));
        assert!(!plausible_email("a@b"));
        assert!(!plausible_email("@b.co"));
        assert!(!plausible_email("a b@c.co"));
    }

    #[test]
    fn names_cover_every_bit() {
        assert!(BUTTON_NAMES.len() > usize::from(*open_controller_core::extras::HANDHELD.last().unwrap()));
        assert!(moved(-20_000, 20_000) && !moved(-3000, 4000));
    }

    #[test]
    fn gyro_lines() {
        assert!(turned([0., 1., 0.]) && !turned([0.1, 0., 0.]));
        assert!(gyro_seen(0, [0.; 3], 0).ends_with("no readings"));
        assert!(gyro_seen(240, [0.; 3], 0).contains("240 readings, all zero"));
        let moving = gyro_seen(240, [std::f32::consts::PI, 0., 0.], 32767);
        assert!(moving.contains("pitch 180 °/s") && moving.ends_with("32767") && !moving.contains("all zero"));
    }
}
