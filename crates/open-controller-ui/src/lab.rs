//! Controller Lab: explicit opt-ins and per-profile settings for desktop/motion/trigger tools.
use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::Theme;
use crate::ui::{MainView, Screen, Section};
use crate::widgets::{self, Kind, button, caption, card, strong};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Div, Entity, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Subscription, div, px,
};
use open_controller_core::extras::Family;
use open_controller_core::i18n::Text;
use open_controller_core::ipc::ToTray;
use open_controller_core::lab::{Config, EffectMode, LightMode, TouchMode, TriggerMode};
use open_controller_core::mapping::{axis, button as b};
use open_controller_core::profile::Edit;
use open_controller_core::{PadKey, PadView, Role};

pub struct Number {
    key: (PadKey, usize),
    range: (u16, u16),
    last: u16,
    field: Entity<TextField>,
    _events: Subscription,
}
fn field(label: &'static str, control: Div, t: &Theme) -> Div {
    div().flex().flex_col().gap(px(6.)).child(strong(label, t.text)).child(control)
}
fn panel(t: &Theme) -> Div {
    card(t).p(px(16.)).flex().flex_col().gap(px(14.))
}
#[allow(clippy::too_many_arguments)]
fn choices<T: Copy + PartialEq + 'static>(
    id: &'static str,
    labels: &[&'static str],
    values: &[T],
    picked: T,
    key: PadKey,
    t: &Theme,
    set: impl Fn(&mut Config, T) + Copy + 'static,
    cx: &mut Context<MainView>,
) -> Div {
    widgets::segmented(
        id,
        values.iter().zip(labels).map(|(&v, &label)| (v, SharedString::from(label))).collect(),
        picked,
        t,
        cx,
        move |this, value, cx| this.lab_edit(key, |cfg| set(cfg, value), cx),
    )
}

impl MainView {
    fn lab_edit(&mut self, key: PadKey, edit: impl FnOnce(&mut Config), cx: &mut Context<Self>) {
        if let Some(pad) = self.pad(key, cx) {
            let mut cfg = pad.profiles.active().lab;
            edit(&mut cfg);
            self.edit_now(key, Edit::Lab(cfg.clean()), cx);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn lab_number(
        &mut self,
        id: &'static str,
        label: &'static str,
        value: u16,
        range: (u16, u16),
        step: u16,
        pad: &PadView,
        set: impl Fn(&mut Config, u16) + Copy + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme;
        let key = pad.key;
        let profile = pad.profiles.active;
        let tab = self.lab_tab;
        let range = (range.0.min(range.1), range.1);
        if self.lab_numbers.get(id).is_none_or(|n| n.key != (key, profile) || n.range != range) {
            let field = cx.new(|cx| TextField::new(cx, false, 5).placeholder(format!("{}–{}", range.0, range.1)));
            let events = cx.subscribe(&field, move |this, entity, event, cx| {
                if this.section != Section::Lab
                    || this.lab_tab != tab
                    || !matches!(this.screen, Screen::Device(k, _) if k == key)
                    || !this.pad(key, cx).is_some_and(|p| p.profiles.active == profile)
                {
                    return;
                }
                if matches!(event, TextFieldEvent::Submit) {
                    let fallback = this.lab_numbers.get(id).map_or(value, |n| n.last);
                    let accepted = entity.read(cx).text().trim().parse::<u16>().unwrap_or(fallback).clamp(range.0, range.1);
                    this.lab_edit(key, |cfg| set(cfg, accepted), cx);
                    entity.update(cx, |field, cx| field.set_text(&accepted.to_string(), cx));
                    return;
                }
                if let Ok(value) = entity.read(cx).text().trim().parse::<u16>()
                    && (range.0..=range.1).contains(&value)
                {
                    this.lab_edit(key, |cfg| set(cfg, value), cx);
                }
            });
            field.update(cx, |field, cx| field.set_text(&value.to_string(), cx));
            self.lab_numbers.insert(id, Number { key: (key, profile), range, last: value, field, _events: events });
        }
        let number = self.lab_numbers.get_mut(id).unwrap();
        if number.last != value {
            number.last = value;
            number.field.update(cx, |field, cx| field.set_text(&value.to_string(), cx));
        }
        let field = number.field.clone();
        div().flex().flex_col().gap(px(6.)).child(caption(label, t.text2)).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(button((id, 0usize), "−", Kind::Standard, &t).on_click(
                    cx.listener(move |this, _, _, cx| this.lab_edit(key, |cfg| set(cfg, value.saturating_sub(step).max(range.0)), cx)),
                ))
                .child(div().w(px(96.)).child(field))
                .child(button((id, 1usize), "+", Kind::Standard, &t).on_click(
                    cx.listener(move |this, _, _, cx| this.lab_edit(key, |cfg| set(cfg, value.saturating_add(step).min(range.1)), cx)),
                )),
        )
    }

    pub fn lab_section(&mut self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let l = text.lab;
        let key = pad.key;
        let cfg = pad.profiles.active().lab;
        let tabs = widgets::segmented(
            "lab-tabs",
            l.tabs.iter().enumerate().map(|(i, &label)| (i as u8, label.into())).collect(),
            self.lab_tab,
            &t,
            cx,
            |this, tab, cx| {
                this.lab_tab = tab;
                cx.notify();
            },
        );
        let native = matches!(pad.role, Role::KeptNative | Role::Native { .. });
        let content = match self.lab_tab {
            0 => self.lab_effects(pad, text, cx),
            1 => {
                if !cfg!(windows) {
                    return panel(&t).child(caption(l.windows, t.text2)).into_any_element();
                }
                let mode = choices(
                    "lab-touch-mode",
                    &l.touch_modes,
                    &[TouchMode::Off, TouchMode::Mouse, TouchMode::Trackball],
                    cfg.touch.mode,
                    key,
                    &t,
                    |c, mode| c.touch.mode = mode,
                    cx,
                );
                let sens = self.lab_number(
                    "lab-touch-sens",
                    l.sensitivity,
                    cfg.touch.sensitivity,
                    (10, 400),
                    10,
                    pad,
                    |c, v| c.touch.sensitivity = v,
                    cx,
                );
                let friction = self.lab_number(
                    "lab-touch-friction",
                    l.friction,
                    cfg.touch.friction_ms,
                    (40, 1000),
                    20,
                    pad,
                    |c, v| c.touch.friction_ms = v,
                    cx,
                );
                panel(&t)
                    .child(caption(l.touch_note, t.text2))
                    .child(mode)
                    .child(sens)
                    .when(cfg.touch.mode == TouchMode::Trackball, |d| d.child(friction))
                    .child(self.toggle(
                        "lab-touch-tap",
                        (l.tap, ""),
                        cfg.touch.tap,
                        Edit::Lab(Config { touch: open_controller_core::lab::TouchMouse { tap: !cfg.touch.tap, ..cfg.touch }, ..cfg }),
                        key,
                        cx,
                    ))
                    .child(self.toggle(
                        "lab-touch-scroll",
                        (l.scroll, ""),
                        cfg.touch.scroll,
                        Edit::Lab(Config {
                            touch: open_controller_core::lab::TouchMouse { scroll: !cfg.touch.scroll, ..cfg.touch },
                            ..cfg
                        }),
                        key,
                        cx,
                    ))
                    .into_any_element()
            }
            2 => {
                if !cfg!(windows) {
                    return panel(&t).child(caption(l.windows, t.text2)).into_any_element();
                }
                let counts = self.lab_number(
                    "lab-flick-counts",
                    l.counts,
                    cfg.flick.counts_per_turn,
                    (100, 60000),
                    100,
                    pad,
                    |c, v| c.flick.counts_per_turn = v,
                    cx,
                );
                let duration = self.lab_number(
                    "lab-flick-duration",
                    l.duration,
                    cfg.flick.duration_ms,
                    (20, 300),
                    10,
                    pad,
                    |c, v| c.flick.duration_ms = v,
                    cx,
                );
                let threshold = self.lab_number(
                    "lab-flick-threshold",
                    l.threshold,
                    u16::from(cfg.flick.threshold),
                    (50, 99),
                    5,
                    pad,
                    |c, v| c.flick.threshold = v as u8,
                    cx,
                );
                let sens = self.lab_number(
                    "lab-flick-gyro-sens",
                    l.gyro_sens,
                    cfg.flick.gyro_sensitivity,
                    (1, 100),
                    1,
                    pad,
                    |c, v| c.flick.gyro_sensitivity = v,
                    cx,
                );
                panel(&t)
                    .child(caption(l.flick_note, t.text2))
                    .child(self.toggle(
                        "lab-flick-on",
                        (l.flick_on, ""),
                        cfg.flick.enabled,
                        Edit::Lab(Config { flick: open_controller_core::lab::Flick { enabled: !cfg.flick.enabled, ..cfg.flick }, ..cfg }),
                        key,
                        cx,
                    ))
                    .child(counts)
                    .child(duration)
                    .child(threshold)
                    .when(pad.features.motion, |d| {
                        d.child(self.toggle(
                            "lab-flick-gyro",
                            (l.gyro, ""),
                            cfg.flick.gyro_mouse,
                            Edit::Lab(Config {
                                flick: open_controller_core::lab::Flick { gyro_mouse: !cfg.flick.gyro_mouse, ..cfg.flick },
                                ..cfg
                            }),
                            key,
                            cx,
                        ))
                        .child(sens)
                    })
                    .into_any_element()
            }
            3 => {
                let mut rows = div().flex().flex_wrap().gap(px(12.));
                for side in 0..2 {
                    rows = rows.child(self.lab_trigger_input(pad, side, text, cx));
                }
                div().flex().flex_col().gap(px(12.)).child(caption(l.input_note, t.text2)).child(rows).into_any_element()
            }
            4 => {
                let number = self.lab_number("lab-debounce", l.debounce, cfg.debounce_ms, (0, 100), 5, pad, |c, v| c.debounce_ms = v, cx);
                panel(&t).child(caption(l.debounce_note, t.text2)).child(number).into_any_element()
            }
            5 => {
                let status = &pad.lab_status.dsu;
                panel(&t)
                    .child(self.toggle("lab-dsu", (l.dsu_on, ""), cfg.dsu, Edit::Lab(Config { dsu: !cfg.dsu, ..cfg }), key, cx))
                    .child(caption(l.dsu_note, t.text2))
                    .when(cfg.dsu, |d| {
                        d.child(strong(
                            format!("127.0.0.1:{} · {}: {}", status.port.max(26760), l.clients, status.clients),
                            if status.running { t.accent } else { t.text2 },
                        ))
                        .when(pad.gyro.is_none(), |d| d.child(caption(l.waiting, t.text2)))
                        .when_some(status.error.clone(), |d, error| d.child(caption(error, t.caution)))
                    })
                    .into_any_element()
            }
            _ => {
                let mode = choices(
                    "lab-light-mode",
                    &l.light_modes,
                    &[LightMode::Off, LightMode::Rainbow, LightMode::Breathe, LightMode::Rumble],
                    cfg.dynamic.mode,
                    key,
                    &t,
                    |c, v| c.dynamic.mode = v,
                    cx,
                );
                let period = self.lab_number(
                    "lab-light-period",
                    l.period,
                    cfg.dynamic.period_ms,
                    (500, 30000),
                    500,
                    pad,
                    |c, v| c.dynamic.period_ms = v,
                    cx,
                );
                let idle = self.lab_number(
                    "lab-light-idle",
                    l.idle,
                    cfg.dynamic.idle_seconds,
                    (0, 600),
                    10,
                    pad,
                    |c, v| c.dynamic.idle_seconds = v,
                    cx,
                );
                panel(&t)
                    .child(caption(l.light_note, t.text2))
                    .child(mode)
                    .when(matches!(cfg.dynamic.mode, LightMode::Rainbow | LightMode::Breathe), |d| d.child(period))
                    .child(idle)
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(caption(l.intro, t.text2))
            .when(native, |d| d.child(caption(l.native, t.caution)))
            .child(tabs)
            .child(content)
            .into_any_element()
    }

    fn lab_effects(&mut self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let l = text.lab;
        let cfg = pad.profiles.active().lab;
        let key = pad.key;
        if !matches!(pad.family, Family::DualSense | Family::DualSenseEdge) {
            return panel(&t).child(caption(l.unsupported, t.text2)).into_any_element();
        }
        let mut cards = div().flex().flex_wrap().gap(px(12.));
        const IDS: [[&str; 5]; 2] = [
            ["lab-l-effect", "lab-l-start", "lab-l-end", "lab-l-force", "lab-l-freq"],
            ["lab-r-effect", "lab-r-start", "lab-r-end", "lab-r-force", "lab-r-freq"],
        ];
        for (side, ids) in IDS.iter().enumerate() {
            let e = cfg.adaptive[side];
            let modes = choices(
                ids[0],
                &l.effects,
                &[EffectMode::Off, EffectMode::Resistance, EffectMode::Vibration, EffectMode::Click],
                e.mode,
                key,
                &t,
                move |c, mode| c.adaptive[side].mode = mode,
                cx,
            );
            let start = self.lab_number(
                ids[1],
                l.start,
                u16::from(e.start),
                (if e.mode == EffectMode::Click { 2 } else { 0 }, if e.mode == EffectMode::Click { 7 } else { 9 }),
                1,
                pad,
                move |c, v| c.adaptive[side].start = v as u8,
                cx,
            );
            let end = self.lab_number(
                ids[2],
                l.end,
                u16::from(e.end),
                (u16::from(e.start) + 1, 8),
                1,
                pad,
                move |c, v| c.adaptive[side].end = v as u8,
                cx,
            );
            let force = self.lab_number(
                ids[3],
                l.strength,
                u16::from(e.strength),
                (0, 8),
                1,
                pad,
                move |c, v| c.adaptive[side].strength = v as u8,
                cx,
            );
            let frequency = self.lab_number(
                ids[4],
                l.frequency,
                u16::from(e.frequency),
                (1, 100),
                1,
                pad,
                move |c, v| c.adaptive[side].frequency = v as u8,
                cx,
            );
            cards = cards.child(
                panel(&t).flex_1().min_w(px(270.)).child(strong(if side == 0 { l.left } else { l.right }, t.text)).child(modes).when(
                    e.mode != EffectMode::Off,
                    |d| {
                        d.child(start)
                            .when(e.mode == EffectMode::Click, |d| d.child(end))
                            .child(force)
                            .when(e.mode == EffectMode::Vibration, |d| d.child(frequency))
                    },
                ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(caption(l.effect_note, t.text2))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        button("lab-try", l.test, Kind::Primary, &t)
                            .on_click(cx.listener(move |this, _, _, cx| this.send(ToTray::TriggerPreview(key, Some(cfg.adaptive)), cx))),
                    )
                    .child(button("lab-stop", l.stop, Kind::Standard, &t).on_click(cx.listener(move |this, _, _, cx| {
                        this.lab_edit(key, |c| c.adaptive_enabled = false, cx);
                        this.send(ToTray::TriggerPreview(key, None), cx);
                    }))),
            )
            .when(pad.lab_status.testing, |d| d.child(caption(l.testing, t.accent)))
            .when_some(pad.lab_status.trigger_error.clone(), |d, error| d.child(caption(error, t.caution)))
            .child(self.toggle(
                "lab-effect-persist",
                (l.persist, ""),
                cfg.adaptive_enabled,
                Edit::Lab(Config { adaptive_enabled: !cfg.adaptive_enabled, ..cfg }),
                key,
                cx,
            ))
            .child(cards)
            .into_any_element()
    }

    fn lab_trigger_input(&mut self, pad: &PadView, side: usize, text: &'static Text, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let l = text.lab;
        let cfg = pad.profiles.active().lab;
        let trig = cfg.triggers[side];
        let key = pad.key;
        const IDS: [[&str; 8]; 2] = [
            [
                "lab-l-input",
                "lab-l-soft",
                "lab-l-full",
                "lab-l-hyst",
                "lab-l-delay",
                "lab-l-soft-action",
                "lab-l-full-action",
                "lab-l-exclusive",
            ],
            [
                "lab-r-input",
                "lab-r-soft",
                "lab-r-full",
                "lab-r-hyst",
                "lab-r-delay",
                "lab-r-soft-action",
                "lab-r-full-action",
                "lab-r-exclusive",
            ],
        ];
        let ids = IDS[side];
        let mode = choices(
            ids[0],
            &l.input_modes,
            &[TriggerMode::Normal, TriggerMode::Hair, TriggerMode::TwoStage, TriggerMode::HipFire],
            trig.mode,
            key,
            &t,
            move |c, mode| {
                if c.triggers[side].mode == TriggerMode::Normal {
                    c.triggers[side].soft_button = if side == 0 { b::LEFT_SHOULDER } else { b::RIGHT_SHOULDER } as u8;
                    c.triggers[side].full_button = if side == 0 { b::SOUTH } else { b::EAST } as u8;
                }
                c.triggers[side].mode = mode;
                if mode == TriggerMode::Hair {
                    c.triggers[side].soft = 5;
                }
            },
            cx,
        );
        let soft = self.lab_number(ids[1], l.soft, u16::from(trig.soft), (1, 90), 5, pad, move |c, v| c.triggers[side].soft = v as u8, cx);
        let full = self.lab_number(
            ids[2],
            l.full,
            u16::from(trig.full),
            (u16::from(trig.soft) + 1, 100),
            5,
            pad,
            move |c, v| c.triggers[side].full = v as u8,
            cx,
        );
        let hyst = self.lab_number(
            ids[3],
            l.hysteresis,
            u16::from(trig.hysteresis),
            (1, 20),
            1,
            pad,
            move |c, v| c.triggers[side].hysteresis = v as u8,
            cx,
        );
        let delay = self.lab_number(ids[4], l.delay, trig.delay_ms, (10, 1000), 10, pad, move |c, v| c.triggers[side].delay_ms = v, cx);
        let labels = ["A / ×", "B / ○", "X / □", "Y / △", "LB / L1", "RB / R1", "LS / L3", "RS / R3"];
        let values = [
            b::SOUTH as u8,
            b::EAST as u8,
            b::WEST as u8,
            b::NORTH as u8,
            b::LEFT_SHOULDER as u8,
            b::RIGHT_SHOULDER as u8,
            b::LEFT_STICK as u8,
            b::RIGHT_STICK as u8,
        ];
        let soft_action = choices(ids[5], &labels, &values, trig.soft_button, key, &t, move |c, v| c.triggers[side].soft_button = v, cx);
        let full_action = choices(ids[6], &labels, &values, trig.full_button, key, &t, move |c, v| c.triggers[side].full_button = v, cx);
        let mut replacement = cfg;
        replacement.triggers[side].exclusive = !trig.exclusive;
        let a = if side == 0 { axis::LEFT_TRIGGER } else { axis::RIGHT_TRIGGER };
        let raw = u32::from(pad.input.axes[a].max(0) as u16) * 100 / 32767;
        let out = u32::from(pad.lab_status.output.axes[a].max(0) as u16) * 100 / 32767;
        panel(&t)
            .flex_1()
            .min_w(px(270.))
            .child(strong(if side == 0 { l.left } else { l.right }, t.text))
            .child(caption(format!("{}: {raw}%  →  {}: {out}%", l.raw, l.output), t.accent))
            .child(mode)
            .when(trig.mode != TriggerMode::Normal, |d| {
                d.child(soft)
                    .when(trig.mode == TriggerMode::Hair, |d| d.child(hyst))
                    .when(matches!(trig.mode, TriggerMode::TwoStage | TriggerMode::HipFire), |d| {
                        d.child(full)
                            .child(field(l.soft_action, soft_action, &t))
                            .child(field(l.full_action, full_action, &t))
                            .child(self.toggle(ids[7], (l.exclusive, ""), trig.exclusive, Edit::Lab(replacement), key, cx))
                    })
                    .when(trig.mode == TriggerMode::HipFire, |d| d.child(delay))
            })
    }
}
