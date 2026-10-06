//! A controller's Motion and Sticks sections: aiming with the gyro, and a deadzone for sticks
//! that drift. Both edit the chosen profile and apply as they are changed.

use crate::detail::{place_name, printed_name};
use crate::theme::{Theme, radius};
use crate::ui::MainView;
use crate::widgets::{body, caption, card, row, strong, switch};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use open_controller_core::PadView;
use open_controller_core::i18n::Text;
use open_controller_core::mapping::button;
use open_controller_core::profile::{Edit, Gyro, GyroMode, Sticks};

/// A row of choices, one of them picked.
fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    choices: Vec<(T, SharedString)>,
    picked: T,
    t: &Theme,
    on_pick: impl Fn(T) -> Edit + 'static,
    key: open_controller_core::PadKey,
    cx: &mut Context<MainView>,
) -> Div {
    let on_pick = std::rc::Rc::new(on_pick);
    let mut d = div().flex().flex_wrap().gap(px(4.)).p(px(3.)).rounded(px(radius::CONTROL)).bg(t.control);
    for (i, (value, label)) in choices.into_iter().enumerate() {
        let on = value == picked;
        let on_pick = on_pick.clone();
        d = d.child(
            div()
                .id((id, i))
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(56.))
                .h(px(30.))
                .px(px(10.))
                .rounded(px(radius::CHIP))
                .cursor_pointer()
                .when(on, |d| d.bg(t.layer).border_1().border_color(t.stroke_strong))
                .child(caption(label, if on { t.text } else { t.text2 }))
                .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, on_pick(value), cx))),
        );
    }
    d
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
        let hold_picker =
            holding.map(|h| segmented("gyro-hold", holds, h, &t, move |b| Edit::Gyro(Gyro { mode: GyroMode::Holding(b), ..g }), key, cx));
        let sens = segmented(
            "gyro-sens",
            [50u16, 75, 100, 150, 200, 300].into_iter().map(|v| (v, format!("{v} %").into())).collect(),
            g.sensitivity,
            &t,
            move |v| Edit::Gyro(Gyro { sensitivity: v, ..g }),
            key,
            cx,
        );
        let invert = g.invert_y;
        let on = g.mode != GyroMode::Off;
        card(&t)
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(20.))
            .max_w(px(760.))
            .child(caption(text.gyro_desc, t.text2))
            .child(setting(text.gyro_mode, &t, modes))
            .when_some(hold_picker, |d, p| d.child(setting(text.gyro_button, &t, p)))
            .when(on, |d| {
                d.child(setting(text.gyro_sensitivity, &t, sens)).child(
                    row(&t)
                        .id("gyro-invert")
                        .cursor_pointer()
                        .hover(move |s| s.bg(t.layer_hover))
                        .on_click(cx.listener(move |this, _, _, cx| this.edit_now(key, Edit::Gyro(Gyro { invert_y: !invert, ..g }), cx)))
                        .child(body(text.gyro_invert, t.text).flex_1())
                        .child(switch(invert, text.on, text.off, &t)),
                )
            })
            .into_any_element()
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
