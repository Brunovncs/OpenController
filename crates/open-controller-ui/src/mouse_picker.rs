//! Pick mouse outputs explicitly; recording a physical side click could trigger other apps.
use crate::{
    theme::Theme,
    ui::MainView,
    widgets::{Kind, button, caption, card},
};
use gpui::{Context, Div, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::{
    PadKey,
    binding::{Action, Chord, modifier},
    keyboard,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Button(PadKey, u8),
    Shortcut,
}
pub struct MousePicker {
    pub target: Target,
    mods: u8,
}

impl MainView {
    pub fn open_mouse_picker(&mut self, target: Target, cx: &mut Context<Self>) {
        self.stop_press_meter(cx);
        let close = self.mouse_picker.as_ref().is_some_and(|p| p.target == target);
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        if let Some(e) = self.editor.as_mut() {
            e.recording = false;
        }
        let mods = match target {
            Target::Shortcut => self.shortcut_editor.as_ref().map_or(0, |e| e.keys.mods),
            Target::Button(key, b) => self
                .pad(key, cx)
                .and_then(|p| match p.profiles.active().bindings.get(&b) {
                    Some(Action::Keys(c)) => Some(c.mods),
                    _ => None,
                })
                .unwrap_or(0),
        };
        self.mouse_picker = (!close).then_some(MousePicker { target, mods });
        cx.notify();
    }

    pub fn render_mouse_picker(&self, target: Target, t: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let picker = self.mouse_picker.as_ref().filter(|p| p.target == target)?;
        let mut modifiers = div().flex().flex_wrap().gap(px(6.));
        for (i, (name, bit)) in
            [("Ctrl", modifier::CTRL), ("Shift", modifier::SHIFT), ("Alt", modifier::ALT), ("Win", modifier::WIN)].into_iter().enumerate()
        {
            modifiers = modifiers.child(
                button(("mouse-mod", i), name, if picker.mods & bit != 0 { Kind::Primary } else { Kind::Standard }, t).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if let Some(p) = &mut this.mouse_picker {
                            p.mods ^= bit;
                        }
                        cx.notify();
                    }),
                ),
            );
        }
        let mut buttons = div().flex().flex_wrap().gap(px(6.));
        for (i, vk) in [1, 2, 4, 5, 6].into_iter().enumerate() {
            buttons = buttons.child(button(("mouse-button", i), keyboard::key_name(vk), Kind::Standard, t).on_click(cx.listener(
                move |this, _, _, cx| {
                    let Some(picker) = this.mouse_picker.take().filter(|p| p.target == target) else {
                        return;
                    };
                    let chord = Chord::new(picker.mods, vk);
                    match target {
                        Target::Shortcut => {
                            if let Some(e) = this.shortcut_editor.as_mut() {
                                e.keys = chord;
                            }
                        }
                        Target::Button(key, b) => {
                            if let Some(pad) = this.pad(key, cx) {
                                this.bind(&pad, b, Some(Action::Keys(chord)), cx);
                            }
                        }
                    }
                    cx.notify();
                },
            )));
        }
        Some(
            card(t)
                .p(px(12.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(caption(self.model.read(cx).text.configuration.mouse_hint, t.text2))
                .child(modifiers)
                .child(buttons)
                .child(button("mouse-cancel", self.model.read(cx).text.configuration.cancel, Kind::Subtle, t).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.mouse_picker = None;
                        cx.notify();
                    },
                ))),
        )
    }
}
