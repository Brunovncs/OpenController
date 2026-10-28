//! A controller's Motion section, aiming with the gyro, and its pressure-sensitive buttons. Both
//! edit the chosen profile and apply as they are changed; the Sticks section is in `sticks.rs`.

use crate::detail::{place_name, printed_name};
use crate::theme::{Theme, icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{self, body, caption, card, icon, row, strong, switch};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled,
    div, px,
};
use open_controller_core::i18n::Text;
use open_controller_core::mapping::{self, button};
use open_controller_core::platform;
use open_controller_core::profile::{Acceleration, DpadStick, Edit, Gyro, GyroMode, Output, Pressure};
use open_controller_core::{PadKey, PadView};

/// A row of choices that edits the controller's profile.
fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    choices: Vec<(T, SharedString)>,
    picked: T,
    t: &Theme,
    on_pick: impl Fn(T) -> Edit + 'static,
    key: PadKey,
    cx: &mut Context<MainView>,
) -> Div {
    widgets::segmented(id, choices, picked, t, cx, move |this, v, cx| this.edit_now(key, on_pick(v), cx))
}

fn setting(label: &'static str, t: &Theme, control: Div) -> Div {
    div().flex().flex_col().gap(px(8.)).child(strong(label, t.text)).child(control)
}

/// A pressure-sensitive button's name, as printed on a PlayStation controller.
fn pressure_name(text: &Text, b: u32) -> SharedString {
    match b {
        button::SOUTH => text.cross,
        button::EAST => text.circle,
        button::WEST => text.square,
        button::NORTH => text.triangle,
        button::LEFT_SHOULDER => "L1",
        button::RIGHT_SHOULDER => "R1",
        button::DPAD_UP => "↑",
        button::DPAD_DOWN => "↓",
        button::DPAD_LEFT => "←",
        _ => "→",
    }
    .into()
}

/// What a controller is shown to games as, where there is a choice: an Xbox 360 controller, or
/// a DualShock 4 that carries the motion sensors too.
pub fn can_choose_output(pad: &PadView) -> bool {
    platform::DS4_OUTPUT && matches!(pad.key, PadKey::Slot(_)) && pad.store.is_some()
}

impl MainView {
    /// A setting that is on or off, with its name and what it does.
    pub(crate) fn toggle(
        &self,
        id: &'static str,
        (label, desc): (&'static str, &'static str),
        on: bool,
        edit: Edit,
        key: PadKey,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = self.theme;
        let text = self.model.read(cx).text;
        row(&t)
            .id(id)
            .cursor_pointer()
            .hover(move |s| s.bg(t.layer_hover))
            .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, edit.clone(), cx)))
            .child(div().flex().flex_col().flex_1().min_w(px(0.)).gap(px(2.)).child(body(label, t.text)).child(caption(desc, t.text2)))
            .child(switch(on, text.on, text.off, &t))
    }

    pub fn motion_section(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let g = pad.profiles.active().gyro;
        let holding = match g.mode {
            GyroMode::Holding(b) => Some(b),
            _ => None,
        };
        let mode_of = move |m: GyroMode| Edit::Gyro(Gyro { mode: m, ..g });
        // The modes; "holding a button" starts on the first extra button, or the left bumper.
        let first_hold = pad.extras.first().copied().unwrap_or(button::LEFT_SHOULDER as u8);
        let modes = segmented(
            "gyro-mode",
            vec![(0u8, text.gyro_off.into()), (1, text.gyro_always.into()), (2, text.gyro_aiming.into()), (3, text.gyro_holding.into())],
            match g.mode {
                GyroMode::Off => 0,
                GyroMode::Always => 1,
                GyroMode::Aiming => 2,
                GyroMode::Holding(_) => 3,
            },
            &t,
            move |m| {
                mode_of(match m {
                    0 => GyroMode::Off,
                    1 => GyroMode::Always,
                    2 => GyroMode::Aiming,
                    _ => GyroMode::Holding(holding.unwrap_or(first_hold)),
                })
            },
            key,
            cx,
        );
        // Buttons that can hold the gyro on: the bumpers, the stick clicks and the extras.
        let mut holds: Vec<(u8, SharedString)> = vec![
            (button::LEFT_SHOULDER as u8, "LB".into()),
            (button::RIGHT_SHOULDER as u8, "RB".into()),
            (button::LEFT_STICK as u8, "LS".into()),
            (button::RIGHT_STICK as u8, "RS".into()),
        ];
        for &b in &pad.extras {
            let name = printed_name(pad, b).map(String::from).unwrap_or_else(|| place_name(text, pad, b));
            holds.push((b, name.into()));
        }
        // The button that pauses the gyro is any of the same, but not the one that turns it on.
        let mut pauses: Vec<(Option<u8>, SharedString)> = vec![(None, text.gyro_none.into())];
        pauses.extend(holds.iter().filter(|(b, _)| Some(*b) != holding).map(|(b, name)| (Some(*b), name.clone())));
        let hold_picker =
            holding.map(|h| segmented("gyro-hold", holds, h, &t, move |b| Edit::Gyro(Gyro { mode: GyroMode::Holding(b), ..g }), key, cx));
        let press = holding.map(|_| {
            segmented(
                "gyro-press",
                vec![(false, text.gyro_press_hold.into()), (true, text.gyro_press_toggle.into())],
                g.toggle,
                &t,
                move |v| Edit::Gyro(Gyro { toggle: v, ..g }),
                key,
                cx,
            )
        });
        let sens = segmented(
            "gyro-sens",
            [50u16, 75, 100, 150, 200, 300].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            g.sensitivity,
            &t,
            move |v| Edit::Gyro(Gyro { sensitivity: v, ..g }),
            key,
            cx,
        );
        let mut aims: Vec<(Option<u16>, SharedString)> = vec![(None, text.gyro_same.into())];
        aims.extend([50u16, 75, 100, 150, 200].into_iter().map(|v| (Some(v), format!("{v} %").into())));
        let aim_sens =
            segmented("gyro-aim-sens", aims, g.aim_sensitivity, &t, move |v| Edit::Gyro(Gyro { aim_sensitivity: v, ..g }), key, cx);
        let vertical = segmented(
            "gyro-vertical",
            [50u16, 75, 100, 125, 150].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            g.y_scale,
            &t,
            move |v| Edit::Gyro(Gyro { y_scale: v, ..g }),
            key,
            cx,
        );
        let pause = segmented("gyro-pause", pauses, g.off_button, &t, move |b| Edit::Gyro(Gyro { off_button: b, ..g }), key, cx);
        let invert = g.invert_y;
        let on = g.mode != GyroMode::Off;
        // While aiming is the only time the gyro works in that mode, so it has one sensitivity;
        // and a held button already stops it when let go.
        let aim_shown = on && g.mode != GyroMode::Aiming;
        let pause_shown = on && !(holding.is_some() && !g.toggle);
        let sixaxis = pad.features.sixaxis;
        let note = sixaxis.then(|| {
            div()
                .flex()
                .items_start()
                .gap(px(10.))
                .p(px(12.))
                .rounded(px(radius::CARD))
                .bg(t.accent_soft)
                .child(icon(glyph::INFO, 14., t.accent).mt(px(2.)))
                .child(caption(text.ds3_motion_note, t.text).flex_1().min_w(px(0.)))
        });
        let tilt = (sixaxis && on).then(|| {
            self.toggle("gyro-tilt", (text.gyro_tilt, text.gyro_tilt_desc), g.tilt, Edit::Gyro(Gyro { tilt: !g.tilt, ..g }), key, cx)
        });
        let ds4 = pad.profiles.output == Output::DualShock4;
        let to_games = can_choose_output(pad).then(|| {
            let next = if ds4 { Output::Xbox360 } else { Output::DualShock4 };
            self.toggle("motion-to-games", (text.motion_to_games, text.motion_to_games_desc), ds4, Edit::Output(next), key, cx)
        });
        let open = self.gyro_advanced;
        let advanced = div()
            .id("gyro-advanced")
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .child(icon(if open { glyph::CHEVRON_DOWN } else { glyph::CHEVRON_RIGHT }, 14., t.accent))
            .child(body(text.gyro_advanced, t.accent))
            .on_click(cx.listener(|this, _, _, cx| {
                this.gyro_advanced = !this.gyro_advanced;
                cx.notify();
            }));
        card(&t)
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(20.))
            .max_w(px(760.))
            .child(caption(text.gyro_desc, t.text2))
            .children(note)
            .child(setting(text.gyro_mode, &t, modes))
            .when_some(hold_picker, |d, p| d.child(setting(text.gyro_button, &t, p).child(caption(text.gyro_button_note, t.text2))))
            .when_some(press, |d, p| d.child(setting(text.gyro_press, &t, p)))
            .children(tilt)
            .children(to_games)
            .when(on, |d| {
                d.child(setting(text.gyro_sensitivity, &t, sens))
                    .when(aim_shown, |d| d.child(setting(text.gyro_aim_sensitivity, &t, aim_sens)))
                    .child(
                        setting(text.gyro_vertical, &t, vertical).child(
                            row(&t)
                                .id("gyro-invert")
                                .cursor_pointer()
                                .hover(move |s| s.bg(t.layer_hover))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Gyro(Gyro { invert_y: !invert, ..g }), cx)),
                                )
                                .child(body(text.gyro_invert, t.text).flex_1())
                                .child(switch(invert, text.on, text.off, &t)),
                        ),
                    )
                    .when(pause_shown, |d| d.child(setting(text.gyro_pause, &t, pause).child(caption(text.gyro_pause_desc, t.text2))))
                    .child(advanced)
                    .when(open, |d| d.child(self.gyro_advanced_settings(g, key, text, cx)))
            })
            .into_any_element()
    }

    /// Acceleration, tightening and the gyro's anti-deadzone: settings most people leave alone.
    fn gyro_advanced_settings(&self, g: Gyro, key: PadKey, text: &'static Text, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let custom = match g.acceleration {
            Some(a) => self.gyro_custom || (a != Acceleration::LIGHT && a != Acceleration::STRONG),
            None => false,
        };
        let kind = match g.acceleration {
            None => 0u8,
            Some(_) if custom => 3,
            Some(a) if a == Acceleration::LIGHT => 1,
            Some(_) => 2,
        };
        let accel = widgets::segmented(
            "gyro-accel",
            vec![(0u8, text.off.into()), (1, text.gyro_light.into()), (2, text.gyro_strong.into()), (3, text.gyro_custom.into())],
            kind,
            &t,
            cx,
            move |this, v, cx| {
                this.gyro_custom = v == 3;
                let acceleration = match v {
                    0 => None,
                    1 => Some(Acceleration::LIGHT),
                    2 => Some(Acceleration::STRONG),
                    _ => Some(g.acceleration.unwrap_or(Acceleration::LIGHT)),
                };
                this.edit_now(key, Edit::Gyro(Gyro { acceleration, ..g }), cx);
            },
        );
        let custom_rows = g.acceleration.filter(|_| custom).map(|a| {
            let mut preset =
                |id: &'static str, values: &[u16], picked: u16, label: fn(u16) -> String, set: fn(Acceleration, u16) -> Acceleration| {
                    segmented(
                        id,
                        values.iter().map(|&v| (v, label(v).into())).collect(),
                        picked,
                        &t,
                        move |v| Edit::Gyro(Gyro { acceleration: Some(set(a, v)), ..g }),
                        key,
                        cx,
                    )
                };
            let speed = |v: u16| format!("{v} °/s");
            div()
                .flex()
                .flex_col()
                .gap(px(20.))
                .pl(px(16.))
                .border_l_2()
                .border_color(t.stroke)
                .child(setting(
                    text.gyro_fast,
                    &t,
                    preset(
                        "gyro-fast",
                        &[125, 150, 200, 300],
                        a.factor,
                        |v| format!("+{} %", v - 100),
                        |a, v| Acceleration { factor: v, ..a },
                    ),
                ))
                .child(setting(
                    text.gyro_from,
                    &t,
                    preset("gyro-from", &[0, 20, 40, 60, 90], a.from, speed, |a, v| Acceleration { from: v, ..a }),
                ))
                .child(setting(
                    text.gyro_to,
                    &t,
                    preset("gyro-to", &[120, 160, 180, 240, 300], a.to, speed, |a, v| Acceleration { to: v, ..a }),
                ))
        });
        let tightening = segmented(
            "gyro-tightening",
            [0u16, 5, 10, 15].into_iter().map(|v| (v, if v == 0 { text.off.into() } else { format!("{v} °/s").into() })).collect(),
            g.tightening,
            &t,
            move |v| Edit::Gyro(Gyro { tightening: v, ..g }),
            key,
            cx,
        );
        let ad = segmented(
            "gyro-anti-deadzone",
            [0u8, 6, 12, 18, 24].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            g.anti_deadzone,
            &t,
            move |v| Edit::Gyro(Gyro { anti_deadzone: v, ..g }),
            key,
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(setting(text.gyro_acceleration, &t, accel).child(caption(text.gyro_acceleration_desc, t.text2)))
            .when_some(custom_rows, |d, r| d.child(r))
            .child(setting(text.gyro_tightening, &t, tightening).child(caption(text.gyro_tightening_desc, t.text2)))
            .child(setting(text.gyro_anti_deadzone, &t, ad).child(caption(text.gyro_anti_deadzone_desc, t.text2)))
    }

    /// Pressure-sensitive buttons on the triggers, with how hard each is pressed right now.
    pub fn pressure_card(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let p = pad.profiles.active().pressure;
        // The face buttons and the bumpers: the d-pad senses pressure too, but nobody steers with it
        // on a trigger.
        let choices = || {
            let mut c: Vec<(Option<u8>, SharedString)> = vec![(None, text.gyro_none.into())];
            c.extend(mapping::PRESSURE[..6].iter().map(|&b| (Some(b as u8), pressure_name(text, b))));
            c
        };
        let lt =
            segmented("pressure-lt", choices(), p.left_trigger, &t, move |b| Edit::Pressure(Pressure { left_trigger: b, ..p }), key, cx);
        let rt =
            segmented("pressure-rt", choices(), p.right_trigger, &t, move |b| Edit::Pressure(Pressure { right_trigger: b, ..p }), key, cx);
        let dpad = segmented(
            "pressure-dpad",
            vec![(DpadStick::Off, text.off.into()), (DpadStick::Left, text.stick_left.into()), (DpadStick::Right, text.stick_right.into())],
            p.dpad,
            &t,
            move |v| Edit::Pressure(Pressure { dpad: v, ..p }),
            key,
            cx,
        );
        let used = p.left_trigger.is_some() || p.right_trigger.is_some() || p.dpad != DpadStick::Off;
        let keep = used.then(|| {
            let edit = Edit::Pressure(Pressure { keep_buttons: !p.keep_buttons, ..p });
            self.toggle("pressure-keep", (text.pressure_keep, text.pressure_keep_desc), p.keep_buttons, edit, key, cx)
        });
        let mut meters = div().flex().flex_wrap().gap(px(8.));
        for (i, &b) in mapping::PRESSURE.iter().enumerate() {
            let level = f32::from(pad.input.pressure[i]) / 255.;
            meters = meters.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .w(px(56.))
                    .child(caption(pressure_name(text, b), if level > 0. { t.text } else { t.text2 }))
                    .child(
                        div()
                            .h(px(6.))
                            .w_full()
                            .rounded(px(3.))
                            .bg(t.control)
                            .child(div().h_full().w(gpui::relative(level)).rounded(px(3.)).bg(t.accent)),
                    ),
            );
        }
        card(&t)
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(20.))
            .mt(px(20.))
            .child(
                div().flex().flex_col().gap(px(6.)).child(strong(text.pressure_title, t.text)).child(caption(text.pressure_desc, t.text2)),
            )
            .child(meters)
            .child(setting(text.pressure_lt, &t, lt))
            .child(setting(text.pressure_rt, &t, rt))
            .child(setting(text.pressure_dpad, &t, dpad).child(caption(text.pressure_dpad_desc, t.text2)))
            .children(keep)
            .into_any_element()
    }
}
