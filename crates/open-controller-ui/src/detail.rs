//! One controller: a header with its profiles, and a rail of what can be set for it. Buttons,
//! with the drawing and its extra buttons and what each does; Motion and Sticks (in `tuning.rs`);
//! Light, for controllers with a light bar SDL can colour; Information, what it is and what games
//! see.

use crate::art;
use crate::home::{links, power, status};
use crate::keys;
use crate::theme::{Theme, icon as glyph, radius};
use crate::ui::{MainView, Section, Tab};
use crate::widgets::{Kind, body, button, caption, card, chip, display, group, icon, icon_button, keycap, row, stage, strong, title};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use open_controller_core::binding::{self, Action, Chord, Step, XboxButton};
use open_controller_core::device::Power;
use open_controller_core::extras::{self, Family, HANDHELD, Hint};
use open_controller_core::handheld;
use open_controller_core::i18n::{Text, fill};
use open_controller_core::ipc::ToTray;
use open_controller_core::keyboard;
use open_controller_core::profile::{self, Edit, Light, MAX_PROFILES};
use open_controller_core::{PadKey, PadView, Role};

/// Colours offered for the light bar.
const SWATCHES: [[u8; 3]; 12] = [
    [255, 0, 0],
    [255, 80, 0],
    [255, 170, 0],
    [255, 240, 0],
    [120, 255, 0],
    [0, 255, 60],
    [0, 255, 200],
    [0, 170, 255],
    [0, 60, 255],
    [120, 0, 255],
    [255, 0, 200],
    [255, 255, 255],
];

/// Where the button is or what it is: "Back left", "Touchpad click".
pub fn place_name(text: &Text, family: Family, b: u8) -> String {
    match extras::kind(family, b) {
        extras::Kind::BackLeft => text.back_left.into(),
        extras::Kind::BackRight => text.back_right.into(),
        extras::Kind::BackLeft2 => text.back_left2.into(),
        extras::Kind::BackRight2 => text.back_right2.into(),
        extras::Kind::Touchpad => text.touchpad_click.into(),
        extras::Kind::TrackpadLeft => text.trackpad_left.into(),
        extras::Kind::TrackpadRight => text.trackpad_right.into(),
        extras::Kind::StickTouchLeft => text.stick_touch_left.into(),
        extras::Kind::StickTouchRight => text.stick_touch_right.into(),
        extras::Kind::TriggerClickLeft => text.trigger_click_left.into(),
        extras::Kind::TriggerClickRight => text.trigger_click_right.into(),
        extras::Kind::Capture => text.capture.into(),
        extras::Kind::Mic => text.mic.into(),
        extras::Kind::Share => text.share.into(),
        extras::Kind::Assistant => text.assistant.into(),
        extras::Kind::TouchLeft => text.touch_left.into(),
        extras::Kind::TouchRight => text.touch_right.into(),
        extras::Kind::TouchTwo => text.touch_two.into(),
        extras::Kind::Extra(_) if family == Family::Handheld => text.handheld_button.into(),
        extras::Kind::Extra(n) => fill(text.extra_n, n),
    }
}

/// The name printed on the button: SDL's families from the table, a handheld's from its machine.
pub fn printed_name(family: Family, b: u8) -> Option<&'static str> {
    if family == Family::Handheld
        && let Some(i) = HANDHELD.iter().position(|&h| h == b)
    {
        return handheld::this_machine().and_then(|m| m.buttons.get(i)).map(|x| x.label);
    }
    extras::printed_name(family, b)
}

/// Whether games read this controller directly, so its buttons can only become keys and macros.
pub fn native(pad: &PadView) -> bool {
    matches!(pad.role, Role::Native { .. })
}

fn hint_text(text: &Text, h: Hint) -> &'static str {
    match h {
        Hint::EightBitDoDInput => text.hint_8bitdo,
        Hint::EightBitDoSwitchD => text.hint_8bitdo_switch,
        Hint::EightBitDo2CBluetooth => text.hint_8bitdo_2c,
        Hint::EightBitDoForXbox => text.hint_8bitdo_xbox,
        Hint::XboxPaddlesHidden => text.hint_elite,
        Hint::SteamInput => text.hint_steam,
        Hint::FlydigiThirdParty => text.hint_flydigi,
        Hint::Switch2Unsupported => text.hint_switch2,
    }
}

pub fn profile_label(text: &Text, pad: &PadView, i: usize) -> String {
    profile_name(text, pad.profiles.list.get(i).map_or("", |p| p.name.as_str()), i)
}

fn profile_name(text: &Text, name: &str, i: usize) -> String {
    match (name.is_empty(), i) {
        (false, _) => name.to_string(),
        (true, 0) => text.default_profile.to_string(),
        (true, i) => fill(text.profile_n, i + 1),
    }
}

fn keycaps(c: Chord, t: &Theme) -> Div {
    div().flex().items_center().gap(px(4.)).children(keyboard::chord_names(c).into_iter().map(|n| keycap(n, t)))
}

/// What a button does, right-aligned in its row.
fn summary(action: Option<&Action>, text: &Text, t: &Theme) -> Div {
    match action {
        None => caption(text.not_assigned, t.text3),
        Some(Action::Xbox(x)) => div().flex().items_center().gap(px(8.)).child(caption("Xbox", t.text2)).child(keycap(x.label(), t)),
        Some(Action::Keys(c)) => keycaps(*c, t),
        Some(Action::Macro(steps)) => caption(fill(text.macro_steps, steps.iter().filter(|s| matches!(s, Step::Keys(_))).count()), t.text2),
    }
}

fn rgb_of([r, g, b]: [u8; 3]) -> gpui::Hsla {
    rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into()
}

impl MainView {
    fn bind_now(&mut self, key: PadKey, button: u8, action: Option<Action>, cx: &mut Context<Self>) {
        if let Some(pad) = self.pad(key, cx) {
            self.bind(&pad, button, action, cx);
        }
        cx.notify();
    }

    fn save_steps(&mut self, key: PadKey, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let (button, steps) = (e.button, e.steps.clone());
        self.bind_now(key, button, Some(Action::Macro(steps)), cx);
    }

    pub(crate) fn edit_now(&mut self, key: PadKey, edit: Edit, cx: &mut Context<Self>) {
        if let Some(pad) = self.pad(key, cx) {
            self.edit_profiles(&pad, edit, cx);
        }
        cx.notify();
    }

    pub fn render_device(&mut self, pad: &PadView, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let text = self.model.read(cx).text;
        let key = pad.key;
        let assignable = pad.store.is_some();

        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .pb(px(20.))
            .child(icon_button("back", glyph::BACK, &t).on_click(cx.listener(|this, _, _, cx| this.home(cx))))
            .child(
                div().flex().flex_col().flex_1().min_w(px(240.)).gap(px(6.)).child(display(pad.name.clone(), t.text).truncate()).child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(10.))
                        .child(caption(status(pad, text), t.text2))
                        .child(links(pad, text, &t))
                        .children(power(pad, text, &t)),
                ),
            )
            .when(assignable, |d| d.child(self.profile_button(pad, text, cx)))
            .when(pad.features.rumble || matches!(pad.role, Role::Native { .. }), |d| {
                d.child(
                    button("identify", text.identify, Kind::Standard, &t)
                        .on_click(cx.listener(move |this, _, _, cx| this.send(ToTray::Identify(key), cx))),
                )
            })
            .when(pad.can_power_off, |d| {
                d.child(
                    button("off", text.turn_off, Kind::Standard, &t)
                        .on_click(cx.listener(move |this, _, _, cx| this.send(ToTray::PowerOff(key), cx))),
                )
            });

        let mut sections = vec![(Section::Buttons, glyph::BUTTONS, text.nav_buttons)];
        if pad.features.motion && assignable && !native(pad) {
            sections.push((Section::Motion, glyph::MOTION, text.nav_motion));
        }
        if assignable && !native(pad) {
            sections.push((Section::Sticks, glyph::STICKS, text.nav_sticks));
        }
        if pad.features.light_bar && assignable {
            sections.push((Section::Light, glyph::LIGHT, text.nav_light));
        }
        sections.push((Section::Info, glyph::INFO, text.nav_info));
        let current = if sections.iter().any(|s| s.0 == self.section) { self.section } else { Section::Buttons };
        let mut rail = div().flex().flex_col().flex_none().w(px(196.)).gap(px(4.));
        for (i, (s, g, label)) in sections.into_iter().enumerate() {
            let on = s == current;
            rail = rail.child(
                div()
                    .id(("nav", i))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke))
                    .when(!on, |d| d.hover(move |st| st.bg(t.layer)))
                    .child(div().w(px(3.)).h(px(16.)).rounded(px(2.)).when(on, |d| d.bg(t.accent)))
                    .child(icon(g, 16., if on { t.text } else { t.text2 }))
                    .child(body(label, if on { t.text } else { t.text2 }).when(on, |d| d.font_weight(gpui::FontWeight::SEMIBOLD)))
                    .on_click(cx.listener(move |this, _, _, cx| this.show(s, cx))),
            );
        }

        let content = match current {
            Section::Buttons => self.buttons_section(pad, text, cx),
            Section::Light => self.light_section(pad, text, cx),
            Section::Motion => self.motion_section(pad, text, cx),
            Section::Sticks => self.sticks_section(pad, text, cx),
            Section::Info => self.info_section(pad, text),
        };

        div()
            .flex()
            .flex_col()
            .child(header)
            .when(self.profile_menu && assignable, |d| d.child(self.profile_menu(pad, text, cx)))
            .when(pad.in_use != pad.profiles.active, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .mb(px(16.))
                        .px(px(14.))
                        .py(px(10.))
                        .rounded(px(radius::CARD))
                        .bg(t.accent_soft)
                        .child(icon(glyph::INFO, 14., t.accent))
                        .child(body(fill(text.in_use_auto, profile_label(text, pad, pad.in_use)), t.text)),
                )
            })
            .child(div().flex().items_start().gap(px(24.)).child(rail).child(div().flex_1().min_w(px(0.)).child(content)))
            .into_any_element()
    }

    fn profile_button(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let active = pad.profiles.active;
        let name = profile_name(text, &pad.profiles.active().name, active);
        div()
            .id("profile")
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(34.))
            .px(px(12.))
            .rounded(px(radius::CONTROL))
            .bg(t.control)
            .border_1()
            .border_color(if self.profile_menu { t.accent } else { t.stroke })
            .cursor_pointer()
            .hover(move |s| s.bg(t.control_hover))
            .on_click(cx.listener(|this, _, _, cx| {
                this.profile_menu = !this.profile_menu;
                this.renaming = None;
                cx.notify();
            }))
            .child(caption(text.profile, t.text3))
            .child(strong(name, t.text))
            .child(icon(glyph::CHEVRON_DOWN, 10., t.text2))
            .into_any_element()
    }

    fn profile_menu(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let count = pad.profiles.list.len();
        let mut list = div().flex().flex_col().gap(px(2.));
        for (i, p) in pad.profiles.list.iter().enumerate() {
            let on = i == pad.profiles.active;
            let renaming = self.renaming.as_ref().filter(|(r, _)| *r == i).map(|(_, n)| n.clone());
            let name: AnyElement = match renaming {
                Some(n) => div()
                    .flex()
                    .items_center()
                    .h(px(30.))
                    .px(px(8.))
                    .flex_1()
                    .rounded(px(radius::CHIP))
                    .bg(t.control)
                    .border_1()
                    .border_color(t.accent)
                    .child(body(format!("{n}|"), t.text))
                    .into_any_element(),
                None => body(profile_name(text, &p.name, i), t.text).flex_1().into_any_element(),
            };
            list = list.child(
                div()
                    .id(("profile-item", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(40.))
                    .px(px(10.))
                    .rounded(px(radius::CONTROL))
                    .cursor_pointer()
                    .when(on, |d| d.bg(t.accent_soft))
                    .when(!on, |d| d.hover(move |s| s.bg(t.control)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.renaming.is_none() {
                            this.editor = None;
                            this.edit_now(key, Edit::Select(i), cx);
                        }
                    }))
                    .child(div().w(px(16.)).child(if on { icon(glyph::CHECK, 12., t.accent) } else { div() }))
                    .child(name)
                    .child(icon_button(("rename", i), glyph::EDIT, &t).size(px(28.)).on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        let current = this.pad(key, cx).and_then(|p| p.profiles.list.get(i).map(|p| p.name.clone())).unwrap_or_default();
                        this.renaming = Some((i, current));
                        this.focus.focus(window, cx);
                        cx.notify();
                    })))
                    .when(count > 1, |d| {
                        d.child(icon_button(("delete", i), glyph::DELETE, &t).size(px(28.)).on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.renaming = None;
                            this.editor = None;
                            this.edit_now(key, Edit::Delete(i), cx);
                        })))
                    }),
            );
        }
        card(&t)
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(8.))
            .mb(px(20.))
            .max_w(px(560.))
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .px(px(6.))
                    .pb(px(4.))
                    .child(caption(text.profiles_hint, t.text3).flex_1().min_w(px(0.)))
                    .when(count < MAX_PROFILES, |d| {
                        d.child(button("new-profile", text.new_profile, Kind::Standard, &t).on_click(cx.listener(move |this, _, _, cx| {
                            this.editor = None;
                            this.edit_now(key, Edit::Add(String::new()), cx);
                        })))
                    }),
            )
            .child(self.programs(pad, text, cx))
            .into_any_element()
    }

    /// The programs that switch the chosen profile on, and the way to add one that is open.
    fn programs(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let i = pad.profiles.active;
        let programs = pad.profiles.active().programs.clone();
        let mut chips = div().flex().flex_wrap().gap(px(6.));
        for (n, p) in programs.iter().enumerate() {
            let name = p.clone();
            chips = chips.child(
                div()
                    .id(("program", n))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(28.))
                    .pl(px(10.))
                    .pr(px(4.))
                    .rounded(px(radius::CHIP))
                    .bg(t.control)
                    .child(caption(p.clone(), t.text))
                    .child(icon_button(("unprogram", n), glyph::DELETE, &t).size(px(22.)).on_click(cx.listener(move |this, _, _, cx| {
                        this.edit_now(key, Edit::RemoveProgram(i, name.clone()), cx);
                    }))),
            );
        }
        let picker = self.picker.clone().map(|open| {
            let mut list = div().flex().flex_wrap().gap(px(6.));
            if open.is_empty() {
                list = list.child(caption(text.no_open_programs, t.text3));
            }
            for (n, p) in open.into_iter().enumerate() {
                let name = p.clone();
                list = list.child(
                    div()
                        .id(("open-program", n))
                        .flex()
                        .items_center()
                        .h(px(28.))
                        .px(px(10.))
                        .rounded(px(radius::CHIP))
                        .border_1()
                        .border_color(t.stroke_strong)
                        .cursor_pointer()
                        .hover(move |s| s.bg(t.control))
                        .child(caption(p, t.text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.picker = None;
                            this.edit_now(key, Edit::AddProgram(i, name.clone()), cx);
                        })),
                );
            }
            list
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(4.))
            .px(px(6.))
            .pt(px(12.))
            .pb(px(6.))
            .border_t_1()
            .border_color(t.stroke)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(strong(format!("{} · {}", text.programs_title, profile_label(text, pad, i)), t.text))
                    .child(button("add-program", text.add_program, Kind::Subtle, &t).text_color(t.accent).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.picker = if this.picker.is_some() { None } else { Some(crate::programs::open()) };
                            cx.notify();
                        },
                    ))),
            )
            .child(caption(text.programs_hint, t.text3))
            .when(!programs.is_empty(), |d| d.child(chips))
            .children(picker)
            .into_any_element()
    }

    fn buttons_section(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let drawing = card(&t).flex().flex_col().flex_none().overflow_hidden().child(
            stage(&t)
                .flex()
                .justify_center()
                .px(px(20.))
                .pt(px(28.))
                .pb(px(20.))
                .child(art::controller(&pad.input, pad.art, pad.brand, 1.1, &t)),
        );
        let hint = pad.hint.map(|h| {
            div()
                .flex()
                .items_start()
                .gap(px(12.))
                .p(px(14.))
                .mb(px(16.))
                .rounded(px(radius::CARD))
                .bg(t.accent_soft)
                .child(icon(glyph::INFO, 16., t.accent).mt(px(2.)))
                .child(body(hint_text(text, h), t.text).flex_1().min_w(px(0.)))
        });
        div()
            .flex()
            .flex_col()
            .children(hint)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(20.))
                    .child(drawing)
                    .child(div().flex_1().min_w(px(320.)).child(self.extras_column(pad, text, &t, cx))),
            )
            .into_any_element()
    }

    fn extras_column(&self, pad: &PadView, text: &'static Text, t: &Theme, cx: &mut Context<Self>) -> Div {
        let key = pad.key;
        let mut col = div().flex().flex_col().child(title(text.extra_buttons, t.text).pb(px(4.)));
        if pad.extras.is_empty() {
            let message = if matches!(pad.role, Role::Native { .. }) { text.no_extras_native } else { text.no_extras };
            return col.child(caption(message, t.text2));
        }
        col = col.child(caption(text.extras_hint, t.text2).pb(px(12.)));
        let mut g = group();
        for &b in &pad.extras {
            let selected = self.editor.as_ref().is_some_and(|e| e.button == b);
            let pressed = pad.input.pressed(b as u32);
            let pad_for_click = pad.clone();
            g = g.child(
                row(t)
                    .id(("extra", b as usize))
                    .cursor_pointer()
                    .when(pressed, |d| d.bg(t.accent_soft).border_color(t.accent))
                    .when(selected && !pressed, |d| d.border_color(t.accent))
                    .hover(move |s| s.bg(t.layer_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.edit(&pad_for_click, b, cx)))
                    .children(printed_name(pad.family, b).map(|n| {
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .min_w(px(44.))
                            .h(px(28.))
                            .px(px(8.))
                            .rounded(px(radius::CHIP))
                            .bg(if pressed { t.accent } else { t.control })
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(if pressed { t.on_accent } else { t.text })
                            .child(n)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .when(printed_name(pad.family, b).is_some(), |d| d.child(caption(place_name(text, pad.family, b), t.text2)))
                            .when(printed_name(pad.family, b).is_none(), |d| d.child(strong(place_name(text, pad.family, b), t.text))),
                    )
                    .child(summary(pad.profiles.active().bindings.get(&b), text, t))
                    .child(icon(if selected { glyph::CHEVRON_DOWN } else { glyph::CHEVRON_RIGHT }, 12., t.text2)),
            );
            if selected {
                g = g.child(self.editor_panel(pad, b, text, t, cx));
            }
        }
        col = col.child(g);
        if !pad.profiles.active().bindings.is_empty() {
            let confirm = self.confirm_reset;
            let label = if confirm { text.reset_confirm } else { text.reset_extras };
            col = col.child(div().flex().pt(px(12.)).child(
                button("reset", label, if confirm { Kind::Standard } else { Kind::Subtle }, t).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.confirm_reset {
                            this.confirm_reset = false;
                            this.editor = None;
                            this.edit_now(key, Edit::ClearBindings, cx);
                        } else {
                            this.confirm_reset = true;
                        }
                        cx.notify();
                    },
                )),
            ));
        }
        col
    }

    fn light_section(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let p = pad.profiles.active();
        let battery = match pad.power {
            Power::Battery(l) | Power::Charging(l) => l,
            _ => None,
        };
        let shown = profile::light_color(p.light, p.brightness, crate::home::player(&pad.role), battery);
        // The light at full strength, so a dim setting still reads as its colour on screen.
        let max = shown.iter().copied().max().unwrap_or(0).max(1);
        let preview = shown.map(|v| (u16::from(v) * 255 / u16::from(max)) as u8);
        let lit = shown != [0, 0, 0];

        let option = |id: &'static str, label: &'static str, desc: Option<&'static str>, on: bool, light: Light, cx: &mut Context<Self>| {
            row(&t)
                .id(id)
                .cursor_pointer()
                .when(on, |d| d.border_color(t.accent))
                .hover(move |s| s.bg(t.layer_hover))
                .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Light(light), cx)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .rounded_full()
                        .border_1()
                        .border_color(if on { t.accent } else { t.text3 })
                        .when(on, |d| d.child(div().size(px(8.)).rounded_full().bg(t.accent))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(2.))
                        .child(body(label, t.text))
                        .when_some(desc, |d, s| d.child(caption(s, t.text2))),
                )
        };
        let color_on = matches!(p.light, Light::Color(_));
        let current_color = match p.light {
            Light::Color(c) => c,
            _ => SWATCHES[7],
        };
        let mut swatches = div().flex().flex_wrap().gap(px(8.));
        for (i, c) in SWATCHES.into_iter().enumerate() {
            let on = color_on && current_color == c;
            swatches = swatches.child(
                div()
                    .id(("swatch", i))
                    .size(px(32.))
                    .rounded_full()
                    .p(px(3.))
                    .border_2()
                    .border_color(if on { t.text } else { gpui::transparent_black() })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Light(Light::Color(c)), cx)))
                    .child(div().size_full().rounded_full().bg(rgb_of(c)).border_1().border_color(t.stroke)),
            );
        }
        let mut levels = div().flex().gap(px(4.)).p(px(3.)).rounded(px(radius::CONTROL)).bg(t.control);
        for (i, level) in [25u8, 50, 75, 100].into_iter().enumerate() {
            let on = p.brightness == level;
            levels = levels.child(
                div()
                    .id(("level", i))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(56.))
                    .h(px(28.))
                    .rounded(px(radius::CHIP))
                    .cursor_pointer()
                    .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke_strong))
                    .child(caption(format!("{level} %"), if on { t.text } else { t.text2 }))
                    .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Brightness(level), cx))),
            );
        }
        let dims = matches!(p.light, Light::Color(_) | Light::Battery);

        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(20.))
            .child(
                card(&t)
                    .flex()
                    .flex_col()
                    .flex_none()
                    .items_center()
                    .gap(px(16.))
                    .w(px(220.))
                    .p(px(24.))
                    .child(
                        div()
                            .size(px(112.))
                            .rounded_full()
                            .p(px(16.))
                            .bg(if lit { rgb_of(preview).opacity(0.18) } else { t.control })
                            .child(
                                div()
                                    .size_full()
                                    .rounded_full()
                                    .bg(if lit { rgb_of(preview) } else { t.art_well })
                                    .border_1()
                                    .border_color(t.stroke_strong),
                            ),
                    )
                    .child(caption(text.light_desc, t.text2).text_center()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(320.))
                    .gap(px(4.))
                    .child(option(
                        "light-player",
                        text.light_player,
                        Some(text.light_player_desc),
                        p.light == Light::Player,
                        Light::Player,
                        cx,
                    ))
                    .child(option("light-color", text.light_color, None, color_on, Light::Color(current_color), cx))
                    .when(color_on, |d| d.child(div().px(px(16.)).py(px(12.)).child(swatches)))
                    .child(option(
                        "light-battery",
                        text.light_battery,
                        Some(text.light_battery_desc),
                        p.light == Light::Battery,
                        Light::Battery,
                        cx,
                    ))
                    .child(option("light-off", text.light_off, Some(text.light_off_desc), p.light == Light::Off, Light::Off, cx))
                    .when(dims, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(12.))
                                .pt(px(16.))
                                .child(strong(text.brightness, t.text))
                                .child(levels),
                        )
                    })
                    .child({
                        let on = p.low_battery_flash;
                        row(&t)
                            .id("low-battery")
                            .mt(px(16.))
                            .cursor_pointer()
                            .hover(move |s| s.bg(t.layer_hover))
                            .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::LowBatteryFlash(!on), cx)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .gap(px(2.))
                                    .child(body(text.low_battery_flash, t.text))
                                    .child(caption(text.low_battery_flash_desc, t.text2)),
                            )
                            .child(crate::widgets::switch(on, text.on, text.off, &t))
                    }),
            )
            .into_any_element()
    }

    fn info_section(&self, pad: &PadView, text: &'static Text) -> AnyElement {
        let t = self.theme;
        let f = pad.features;
        let features: Vec<&str> = [
            (f.touchpad, text.touchpad),
            (f.motion, text.motion),
            (f.rumble, text.rumble),
            (f.trigger_rumble, text.trigger_rumble),
            (f.light_bar, text.light_bar),
            (f.player_lights, text.player_lights),
        ]
        .into_iter()
        .filter_map(|(has, name)| has.then_some(name))
        .collect();
        let games = match &pad.role {
            Role::Virtual { player: Some(n) } => fill(text.as_xbox, n + 1),
            Role::Virtual { player: None } => text.as_xbox_fifth.into(),
            Role::Waiting { remaining, .. } => fill(text.waiting_slot, remaining.as_secs() + 1),
            Role::Native { player: Some(n) } => fill(text.as_xbox, n + 1),
            Role::Native { player: None } => text.native.into(),
            Role::Unmapped => text.unmapped.into(),
            Role::Unavailable => text.unavailable.into(),
        };
        let note = match (&pad.role, pad.hidden) {
            (Role::Virtual { .. } | Role::Waiting { .. }, true) => Some(text.original_hidden),
            (Role::Virtual { .. } | Role::Waiting { .. }, false) => Some(text.original_visible),
            (Role::Native { .. }, _) => Some(text.as_itself),
            _ => None,
        };
        let line = |label: &'static str, value: AnyElement| {
            div()
                .flex()
                .items_start()
                .gap(px(16.))
                .px(px(16.))
                .py(px(12.))
                .border_b_1()
                .border_color(t.stroke)
                .child(caption(label, t.text2).w(px(140.)).flex_none().pt(px(2.)))
                .child(div().flex_1().min_w(px(0.)).child(value))
        };
        card(&t)
            .flex()
            .flex_col()
            .max_w(px(720.))
            .overflow_hidden()
            .child(line(text.info_model, body(pad.name.clone(), t.text).into_any_element()))
            .when(pad.vendor != 0, |d| {
                d.child(line(text.info_usb_id, body(format!("{:04X}:{:04X}", pad.vendor, pad.product), t.text).into_any_element()))
            })
            .child(line(text.info_connection, links(pad, text, &t).into_any_element()))
            .when_some(power(pad, text, &t), |d, p| d.child(line(text.info_battery, p.into_any_element())))
            .child(line(
                text.in_games,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(body(games, t.text))
                    .when_some(note, |d, n| d.child(caption(n, t.text2)))
                    .into_any_element(),
            ))
            .when(!features.is_empty(), |d| {
                d.child(line(
                    text.features,
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(features.into_iter().map(|f| chip(f, t.text2, t.control)))
                        .into_any_element(),
                ))
            })
            .into_any_element()
    }

    fn editor_panel(&self, pad: &PadView, b: u8, text: &'static Text, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some(e) = &self.editor else { return div().into_any_element() };
        let key = pad.key;
        let current = pad.profiles.active().bindings.get(&b).cloned();
        let keys_only = native(pad);

        let tab = |id: &'static str, label: &'static str, which: Tab, cx: &mut Context<Self>| {
            let on = e.tab == which;
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .h(px(30.))
                .px(px(12.))
                .rounded(px(radius::CHIP))
                .cursor_pointer()
                .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke_strong))
                .child(body(label, if on { t.text } else { t.text2 }).when(on, |d| d.font_weight(gpui::FontWeight::SEMIBOLD)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let Some(e) = this.editor.as_mut() else { return };
                    e.tab = which;
                    e.recording = which == Tab::Key;
                    e.other_keys = false;
                    if which == Tab::Key {
                        this.focus.focus(window, cx);
                    }
                    if which == Tab::Nothing {
                        this.bind_now(key, b, None, cx);
                    }
                    cx.notify();
                }))
        };
        let tabs = div()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .p(px(3.))
            .rounded(px(radius::CONTROL))
            .bg(t.control)
            .when(!keys_only, |d| d.child(tab("tab-xbox", text.tab_xbox, Tab::Xbox, cx)))
            .child(tab("tab-key", text.tab_key, Tab::Key, cx))
            .child(tab("tab-macro", text.tab_macro, Tab::Macro, cx))
            .child(tab("tab-nothing", text.tab_nothing, Tab::Nothing, cx));

        let content: AnyElement = match e.tab {
            Tab::Xbox => {
                let mut grid = div().flex().flex_wrap().gap(px(6.));
                for (i, x) in XboxButton::ALL.into_iter().enumerate() {
                    let on = current == Some(Action::Xbox(x));
                    grid = grid.child(
                        div()
                            .id(("xbox", i))
                            .flex()
                            .items_center()
                            .justify_center()
                            .min_w(px(46.))
                            .h(px(34.))
                            .px(px(8.))
                            .rounded(px(radius::CONTROL))
                            .border_1()
                            .text_size(px(13.))
                            .cursor_pointer()
                            .when(on, |d| d.bg(t.accent).border_color(t.accent).text_color(t.on_accent))
                            .when(!on, |d| d.bg(t.control).border_color(t.stroke).text_color(t.text).hover(move |s| s.bg(t.control_hover)))
                            .child(x.label())
                            .on_click(cx.listener(move |this, _, _, cx| this.bind_now(key, b, Some(Action::Xbox(x)), cx))),
                    );
                }
                div().flex().flex_col().gap(px(10.)).child(caption(text.xbox_desc, t.text2)).child(grid).into_any_element()
            }
            Tab::Key => {
                let shown = match (&current, e.recording) {
                    (_, true) if e.holding != 0 => keycaps(Chord { mods: e.holding, key: 0 }, t),
                    (_, true) => caption(text.listening, t.accent),
                    (Some(Action::Keys(c)), false) => keycaps(*c, t),
                    _ => caption(text.press_key, t.text3),
                };
                let field = div()
                    .id("recorder")
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .h(px(42.))
                    .px(px(12.))
                    .rounded(px(radius::CONTROL))
                    .bg(t.control)
                    .border_1()
                    .border_color(if e.recording { t.accent } else { t.stroke })
                    .cursor_pointer()
                    .child(shown)
                    .child(if e.recording { keycap("Esc", t) } else { caption(text.change, t.text2) })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(e) = this.editor.as_mut() {
                            e.recording = !e.recording;
                            e.holding = 0;
                        }
                        this.focus.focus(window, cx);
                        cx.notify();
                    }));
                let other = e.other_keys;
                let mut list = div().flex().flex_wrap().gap(px(6.));
                if other {
                    for (i, vk) in keys::OTHER.into_iter().enumerate() {
                        let c = Chord { mods: 0, key: vk };
                        list = list.child(div().id(("other", i)).cursor_pointer().child(keycap(keyboard::key_name(vk), t)).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if let Some(e) = this.editor.as_mut() {
                                    e.recording = false;
                                }
                                this.bind_now(key, b, Some(Action::Keys(c)), cx);
                            }),
                        ));
                    }
                }
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(caption(text.key_desc, t.text2))
                    .child(field)
                    .child(div().flex().child(
                        button("other-keys", text.other_keys, Kind::Subtle, t).px(px(0.)).text_color(t.accent).on_click(cx.listener(
                            |this, _, _, cx| {
                                if let Some(e) = this.editor.as_mut() {
                                    e.other_keys = !e.other_keys;
                                }
                                cx.notify();
                            },
                        )),
                    ))
                    .when(other, |d| d.child(list))
                    .into_any_element()
            }
            Tab::Macro => self.macro_editor(key, b, text, t, cx),
            Tab::Nothing => caption(text.nothing_desc, t.text2).into_any_element(),
        };

        card(t).flex().flex_col().gap(px(14.)).p(px(16.)).child(div().flex().child(tabs)).child(content).into_any_element()
    }

    fn macro_editor(&self, key: PadKey, b: u8, text: &'static Text, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some(e) = &self.editor else { return div().into_any_element() };
        let mut steps = div().flex().flex_wrap().items_center().gap(px(6.));
        for (i, step) in e.steps.iter().enumerate() {
            let chip = div()
                .flex()
                .items_center()
                .gap(px(4.))
                .h(px(32.))
                .pl(px(6.))
                .rounded(px(radius::CONTROL))
                .bg(t.control)
                .border_1()
                .border_color(t.stroke);
            let chip = match *step {
                Step::Keys(c) => chip.child(keycaps(c, t)),
                Step::Wait(ms) => {
                    let nudge = |id: &'static str, label: &'static str, delta: i32, cx: &mut Context<Self>| {
                        div()
                            .id((id, i))
                            .px(px(6.))
                            .text_color(t.text2)
                            .cursor_pointer()
                            .hover(move |s| s.text_color(t.text))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(Step::Wait(ms)) = this.editor.as_mut().and_then(|e| e.steps.get_mut(i)) {
                                    *ms = (*ms as i32 + delta).clamp(10, binding::MAX_WAIT_MS as i32) as u16;
                                }
                                this.save_steps(key, cx);
                            }))
                    };
                    chip.child(icon(glyph::CLOCK, 12., t.text2))
                        .child(caption(fill(text.wait_ms, ms), t.text))
                        .child(nudge("less", "−", -50, cx))
                        .child(nudge("more", "+", 50, cx))
                }
            };
            steps = steps.child(chip.child(
                div().id(("remove", i)).px(px(6.)).cursor_pointer().child(icon(glyph::DELETE, 12., t.text3)).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(e) = this.editor.as_mut()
                            && i < e.steps.len()
                        {
                            e.steps.remove(i);
                        }
                        this.save_steps(key, cx);
                    },
                )),
            ));
        }
        let recording = e.recording;
        let empty = e.steps.is_empty();
        let full = e.steps.len() >= binding::MAX_STEPS;
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(caption(text.macro_desc, t.text2))
            .child(if empty {
                caption(if recording { text.listening } else { text.macro_empty }, if recording { t.accent } else { t.text3 })
            } else {
                steps
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(
                            "record",
                            if recording { text.stop } else { text.record },
                            if recording { Kind::Primary } else { Kind::Standard },
                            t,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Some(e) = this.editor.as_mut() {
                                e.recording = !e.recording;
                                e.last_key = None;
                            }
                            this.focus.focus(window, cx);
                            cx.notify();
                        })),
                    )
                    .when(!full, |d| {
                        d.child(button("add-wait", text.add_wait, Kind::Standard, t).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(e) = this.editor.as_mut() {
                                e.steps.push(Step::Wait(100));
                            }
                            this.save_steps(key, cx);
                        })))
                    })
                    .when(!empty, |d| {
                        d.child(button("clear", text.clear, Kind::Subtle, t).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(e) = this.editor.as_mut() {
                                e.steps.clear();
                                e.recording = false;
                            }
                            this.bind_now(key, b, None, cx);
                        })))
                    }),
            )
            .into_any_element()
    }
}
