//! A controller's Motion and Sticks sections: aiming with the gyro, and a deadzone for sticks
//! that drift. Both edit the chosen profile and apply as they are changed.

use crate::detail::{place_name, printed_name};
use crate::theme::{Theme, icon as glyph};
use crate::ui::MainView;
use crate::widgets::{self, body, caption, card, icon, row, strong, switch};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use open_controller_core::i18n::Text;
use open_controller_core::mapping::button;
use open_controller_core::profile::{Acceleration, Edit, Gyro, GyroMode, Sticks};
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

impl MainView {
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
            let name = printed_name(pad.family, b).map(String::from).unwrap_or_else(|| place_name(text, pad.family, b));
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
            .child(setting(text.gyro_mode, &t, modes))
            .when_some(hold_picker, |d, p| d.child(setting(text.gyro_button, &t, p).child(caption(text.gyro_button_note, t.text2))))
            .when_some(press, |d, p| d.child(setting(text.gyro_press, &t, p)))
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

    pub fn sticks_section(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let key = pad.key;
        let s = pad.profiles.active().sticks;
        let dz = segmented(
            "deadzone",
            [0u8, 5, 10, 15, 20, 25].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            s.deadzone,
            &t,
            move |v| Edit::Sticks(Sticks { deadzone: v, ..s }),
            key,
            cx,
        );
        let ad = segmented(
            "anti-deadzone",
            [0u8, 10, 15, 20, 30].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            s.anti_deadzone,
            &t,
            move |v| Edit::Sticks(Sticks { anti_deadzone: v, ..s }),
            key,
            cx,
        );
        card(&t)
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(20.))
            .max_w(px(760.))
            .child(caption(text.sticks_desc, t.text2))
            .child(div().flex().flex_col().gap(px(8.)).child(strong(text.deadzone, t.text)).child(dz))
            .child(div().flex().flex_col().gap(px(8.)).child(strong(text.anti_deadzone, t.text)).child(ad))
            .into_any_element()
    }
}
