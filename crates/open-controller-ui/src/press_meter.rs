//! Explicit controller hold measurement. Uses resident input observations rather than UI timing.
use crate::{
    theme::Theme,
    ui::MainView,
    widgets::{Kind, button, caption, card, strong},
};
use gpui::{Context, Div, ParentElement, StatefulInteractiveElement, Styled, px};
use open_controller_core::{PadKey, PadView, Snapshot, i18n::fill, ipc::ToTray, press_meter::Reading};

pub struct MeterUi {
    pub key: PadKey,
    pub active: bool,
    reading: Option<Reading>,
}

impl MainView {
    pub fn stop_press_meter(&mut self, cx: &mut Context<Self>) {
        if let Some(meter) = self.press_meter.as_mut()
            && meter.active
        {
            meter.active = false;
            self.send(ToTray::RecordShortcuts(None), cx);
        }
    }

    fn start_press_meter(&mut self, key: PadKey, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.stop_press_meter(cx);
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        if let Some(e) = self.editor.as_mut() {
            e.recording = false;
        }
        self.press_meter = Some(MeterUi { key, active: true, reading: None });
        self.send(ToTray::RecordShortcuts(Some(key)), cx);
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn feed_press_meter(&mut self, snapshot: &Snapshot) {
        if let Some(meter) = self.press_meter.as_mut()
            && meter.active
            && let Some(observed) = &snapshot.press_measurement
            && let (key, reading) = observed.as_ref()
            && *key == meter.key
        {
            meter.reading = Some(reading.clone());
        }
    }

    pub fn press_meter_panel(&self, pad: &PadView, t: &Theme, cx: &mut Context<Self>) -> Div {
        let text = &self.model.read(cx).text.combinations;
        let key = pad.key;
        let meter = self.press_meter.as_ref().filter(|m| m.key == key);
        let active = meter.is_some_and(|m| m.active);
        let mut panel = card(t)
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(strong(text.meter_title, t.text))
            .child(caption(text.meter_hint, t.text2))
            .child(button("press-measure", if active { text.meter_stop } else { text.meter_start }, Kind::Standard, t).on_click(
                cx.listener(move |this, _, window, cx| {
                    if this.press_meter.as_ref().is_some_and(|m| m.active) {
                        this.stop_press_meter(cx);
                        cx.notify();
                    } else {
                        this.start_press_meter(key, window, cx);
                    }
                }),
            ));
        if active {
            panel = panel.child(caption(text.meter_recording, t.accent));
        }
        if let Some(reading) = meter.and_then(|m| m.reading.as_ref()) {
            panel = panel
                .child(strong(crate::shortcuts::names(pad, reading.input, self.model.read(cx).text), t.text))
                .child(caption(fill(text.meter_result, format!("{:.2} ms", reading.held_us as f64 / 1000.)), t.text))
                .child(caption(
                    fill(text.meter_minimum, format!("{:.2} ms · n={}", reading.minimum_us as f64 / 1000., reading.samples)),
                    t.text2,
                ));
            if pad.profiles.active().shortcuts.contains_key(&reading.input) {
                panel = panel.child(caption(
                    fill(
                        text.meter_output,
                        if pad.profiles.active().shortcut_held.contains(&reading.input) {
                            text.held_label.into()
                        } else {
                            format!("{} ms", pad.profiles.active().shortcut_hold_ms(reading.input))
                        },
                    ),
                    t.text2,
                ));
            }
        }
        panel.child(caption(text.meter_limit, t.text3))
    }
}
