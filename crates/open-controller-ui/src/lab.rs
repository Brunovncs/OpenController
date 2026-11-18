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
use open_controller_core::lab::{Config, EffectMode};
use open_controller_core::profile::Edit;
use open_controller_core::{PadKey, PadView};

pub struct Number {
    key: (PadKey, usize),
    range: (u16, u16),
    last: u16,
    field: Entity<TextField>,
    _events: Subscription,
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
        let tabs = widgets::segmented(
            "lab-tabs",
            l.tabs.iter().take(1).enumerate().map(|(i, &label)| (i as u8, label.into())).collect(),
            self.lab_tab,
            &t,
            cx,
            |this, tab, cx| {
                this.lab_tab = tab;
                cx.notify();
            },
        );
        let content = self.lab_effects(pad, text, cx);
        div().flex().flex_col().gap(px(16.)).child(caption(l.intro, t.text2)).child(tabs).child(content).into_any_element()
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
}
