//! Record an incoming controller chord, then choose the keyboard chord it taps.
use crate::detail::{normal_label, place_name, printed_name};
use crate::keys;
use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::Theme;
use crate::ui::MainView;
use crate::widgets::{Kind, button, caption, card, keycap, strong, switch};
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Div, Entity, Focusable, InteractiveElement, KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled,
    Subscription, div, px,
};
use open_controller_core::binding::Chord;
use open_controller_core::ipc::ToTray;
use open_controller_core::keyboard;
use open_controller_core::profile::Edit;
use open_controller_core::shortcuts::{self, Recorder};
use open_controller_core::{PadKey, PadView};

pub struct ShortcutEditor {
    original: Option<u64>,
    unassigned: Option<u64>,
    profile: usize,
    conflict: Option<String>,
    pub name: Entity<TextField>,
    pub hold: Entity<TextField>,
    pub fixed_duration: bool,
    duration_info: bool,
    _hold_events: Subscription,
    _hold_focus: Subscription,
    _name_events: Subscription,
    _name_focus: Subscription,
    anchor: u8,
    input: u64,
    pub(crate) keys: Chord,
    pub recorder: Option<Recorder>,
    pub key_recording: bool,
    pub holding: u8,
    error: Option<String>,
    #[cfg(windows)]
    key_capture: Option<crate::key_capture::Capture>,
}

pub(crate) fn names(pad: &PadView, mask: u64, text: &open_controller_core::i18n::Text) -> String {
    (0..64)
        .filter(|b| mask & (1 << b) != 0)
        .map(|b| match 1u64 << b {
            shortcuts::LT => "LT / ZL".into(),
            shortcuts::RT => "RT / ZR".into(),
            _ => {
                let normal = normal_label(pad, b);
                if !normal.is_empty() {
                    normal.into()
                } else {
                    printed_name(pad, b).map(str::to_owned).unwrap_or_else(|| place_name(text, pad, b))
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" + ")
}

impl MainView {
    pub fn stop_shortcut_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.shortcut_editor.as_mut()
            && e.recorder.take().is_some()
        {
            self.send(ToTray::RecordShortcuts(None), cx);
        }
    }

    pub fn stop_shortcut_key_capture(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.shortcut_editor.as_mut() {
            e.key_recording = false;
            e.holding = 0;
            #[cfg(windows)]
            {
                e.key_capture.take();
            }
        }
        cx.notify();
    }

    fn choose_shortcut_key(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.mouse_picker = None;
        self.stop_press_meter(cx);
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        self.focus.focus(window, cx);
        if self.shortcut_editor.is_none() {
            return;
        }
        #[cfg(windows)]
        match crate::key_capture::Capture::start() {
            Ok(capture) => {
                let id = capture.id;
                let events = capture.events.clone();
                if let Some(e) = self.shortcut_editor.as_mut() {
                    e.key_recording = true;
                    e.error = None;
                    e.key_capture = Some(capture);
                }
                cx.spawn(async move |this, cx| {
                    while let Ok(event) = events.recv().await {
                        let done = !matches!(event, crate::key_capture::Event::Holding(_));
                        if this
                            .update(cx, |this, cx| {
                                let Some(e) = this.shortcut_editor.as_mut().filter(|e| e.key_capture.as_ref().is_some_and(|c| c.id == id))
                                else {
                                    return;
                                };
                                match event {
                                    crate::key_capture::Event::Holding(mods) => e.holding = mods,
                                    crate::key_capture::Event::Chosen(chord) => {
                                        e.keys = chord;
                                        e.key_recording = false;
                                        e.holding = 0;
                                        e.key_capture.take();
                                    }
                                    crate::key_capture::Event::Cancelled => {
                                        e.key_recording = false;
                                        e.holding = 0;
                                        e.key_capture.take();
                                    }
                                }
                                cx.notify();
                            })
                            .is_err()
                            || done
                        {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                let message = format!("{}: {error}", self.model.read(cx).text.combinations.key_capture_error);
                if let Some(e) = self.shortcut_editor.as_mut() {
                    e.error = Some(message);
                }
            }
        }
        #[cfg(not(windows))]
        if let Some(e) = self.shortcut_editor.as_mut() {
            e.key_recording = true;
        }
        cx.notify();
    }

    pub fn close_shortcut(&mut self, cx: &mut Context<Self>) {
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        self.shortcut_editor = None;
        self.mouse_picker = None;
        cx.notify();
    }

    fn open_shortcut(&mut self, key: PadKey, anchor: u8, input: Option<u64>, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.stop_press_meter(cx);
        self.close_shortcut(cx);
        let Some(pad) = self.pad(key, cx) else { return };
        let keys = input.and_then(|i| pad.profiles.active().shortcuts.get(&i)).copied().unwrap_or_default();
        let current_name = input.and_then(|i| pad.profiles.active().shortcut_names.get(&i)).cloned().unwrap_or_default();
        let placeholder = self.model.read(cx).text.combinations.name_placeholder;
        let name = cx.new(|cx| TextField::new(cx, false, shortcuts::MAX_NAME).placeholder(placeholder));
        name.update(cx, |field, cx| field.set_text(&current_name, cx));
        let name_events = cx.subscribe(&name, move |this, _, event, cx| {
            if let TextFieldEvent::Submit = event {
                this.save_shortcut(key, cx);
            }
        });
        let name_focus = cx.on_focus(&name.focus_handle(cx), window, |this, _, cx| {
            this.stop_press_meter(cx);
            this.stop_shortcut_recording(cx);
            this.stop_shortcut_key_capture(cx);
        });
        let hold = cx.new(|cx| TextField::new(cx, false, 5));
        hold.update(cx, |field, cx| field.set_text(&pad.profiles.active().shortcut_hold_ms(input.unwrap_or(0)).to_string(), cx));
        let hold_events = cx.subscribe(&hold, move |this, _, event, cx| {
            if let TextFieldEvent::Submit = event {
                this.save_shortcut(key, cx);
            }
        });
        let hold_focus = cx.on_focus(&hold.focus_handle(cx), window, |this, _, cx| {
            this.stop_press_meter(cx);
            this.stop_shortcut_recording(cx);
            this.stop_shortcut_key_capture(cx);
        });
        name.focus_handle(cx).focus(window, cx);
        self.shortcut_editor = Some(ShortcutEditor {
            original: input,
            unassigned: None,
            profile: pad.profiles.active,
            conflict: None,
            name,
            hold,
            fixed_duration: !pad.profiles.active().shortcut_held.contains(&input.unwrap_or(0)),
            duration_info: false,
            _hold_events: hold_events,
            _hold_focus: hold_focus,
            _name_events: name_events,
            _name_focus: name_focus,
            anchor,
            input: input.unwrap_or(0),
            keys,
            recorder: None,
            key_recording: false,
            holding: 0,
            error: None,
            #[cfg(windows)]
            key_capture: None,
        });
        cx.notify();
    }

    fn record_shortcut(&mut self, key: PadKey, cx: &mut Context<Self>) {
        self.stop_press_meter(cx);
        self.stop_shortcut_key_capture(cx);
        let Some(pad) = self.pad(key, cx) else { return };
        let Some(e) = self.shortcut_editor.as_mut() else { return };
        e.recorder = Some(Recorder::new(shortcuts::input(&pad.input)));
        e.key_recording = false;
        e.error = None;
        self.send(ToTray::RecordShortcuts(Some(key)), cx);
        cx.notify();
    }

    pub fn feed_shortcut(&mut self, pad: &PadView, cx: &mut Context<Self>) {
        let Some(e) = self.shortcut_editor.as_mut() else { return };
        let Some(r) = e.recorder.as_mut() else { return };
        if let Some(mask) = r.feed(shortcuts::input(&pad.input)) {
            e.recorder = None;
            if mask != 0 && mask & (1 << e.anchor) != 0 {
                e.input = mask;
                e.conflict = None;
                e.error = None;
            } else {
                e.error = Some(self.model.read(cx).text.combinations.need_buttons.into());
            }
            self.send(ToTray::RecordShortcuts(None), cx);
            cx.notify();
        }
    }

    pub fn shortcut_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(e) = self.shortcut_editor.as_mut() else { return };
        #[cfg(windows)]
        if e.key_capture.is_some() {
            return;
        }
        if ev.keystroke.key == "escape" && keys::mods(&ev.keystroke.modifiers) == 0 {
            if e.key_recording {
                e.key_recording = false;
            } else if e.recorder.is_some() {
                self.stop_shortcut_recording(cx);
            } else {
                self.close_shortcut(cx);
            }
        } else if e.key_recording
            && !ev.is_held
            && let Some(chord) = keys::chord(&ev.keystroke)
        {
            e.keys = chord;
            e.key_recording = false;
            e.holding = 0;
            e.error = None;
        }
        cx.notify();
    }

    fn open_unassigned(&mut self, key: PadKey, anchor: u8, id: u64, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some(pad) = self.pad(key, cx) else { return };
        let Some(action) = pad.profiles.active().unassigned_shortcuts.get(&id).cloned() else { return };
        self.open_shortcut(key, anchor, None, window, cx);
        if let Some(e) = self.shortcut_editor.as_mut() {
            e.unassigned = Some(id);
            e.keys = action.keys;
            e.fixed_duration = !action.hold_until_release;
            e.hold.update(cx, |field, cx| field.set_text(&action.hold_ms.to_string(), cx));
            e.name.update(cx, |field, cx| field.set_text(&action.name, cx));
        }
    }

    fn save_shortcut(&mut self, key: PadKey, cx: &mut Context<Self>) {
        self.commit_shortcut(key, false, cx);
    }

    fn commit_shortcut(&mut self, key: PadKey, replace: bool, cx: &mut Context<Self>) {
        let Some(pad) = self.pad(key, cx) else { return };
        let Some(e) = self.shortcut_editor.as_mut() else { return };
        if e.profile != pad.profiles.active {
            self.close_shortcut(cx);
            return;
        }
        let entered =
            e.hold.read(cx).text().trim().parse::<u16>().ok().filter(|ms| (shortcuts::MIN_HOLD_MS..=shortcuts::MAX_HOLD_MS).contains(ms));
        if e.fixed_duration && entered.is_none() {
            e.error = Some(self.model.read(cx).text.combinations.duration_error.into());
            cx.notify();
            return;
        }
        let hold_ms = entered.unwrap_or(shortcuts::DEFAULT_HOLD_MS);
        if e.keys.is_empty() || (e.input != 0 && !shortcuts::valid(e.input, e.keys)) {
            e.error = Some(self.model.read(cx).text.combinations.need_pair.into());
        } else if !replace && e.input != 0 && e.original != Some(e.input) && pad.profiles.active().shortcuts.contains_key(&e.input) {
            e.conflict = Some(
                pad.profiles
                    .active()
                    .shortcut_names
                    .get(&e.input)
                    .cloned()
                    .unwrap_or_else(|| names(&pad, e.input, self.model.read(cx).text)),
            );
            e.error = None;
        } else if e.input != 0
            && e.original.is_none()
            && !pad.profiles.active().shortcuts.contains_key(&e.input)
            && pad.profiles.active().shortcuts.len() >= shortcuts::MAX_SHORTCUTS
        {
            e.error = Some(self.model.read(cx).text.combinations.limit.into());
        } else if pad.profiles.active().unassigned_shortcuts.len() >= shortcuts::MAX_UNASSIGNED
            && e.unassigned.is_none()
            && (e.input == 0 || (replace && e.original != Some(e.input)))
        {
            e.error = Some(self.model.read(cx).text.combinations.capacity.into());
        } else {
            let change = open_controller_core::shortcut_edit::Change {
                hold_until_release: !e.fixed_duration,
                hold_ms,
                profile: e.profile,
                original: e.original,
                unassigned: e.unassigned,
                anchor: e.anchor,
                input: (e.input != 0).then_some(e.input),
                keys: e.keys,
                name: e.name.read(cx).text(),
                replace,
            };
            self.stop_shortcut_recording(cx);
            self.edit_profiles(&pad, Edit::ChangeShortcut(change), cx);
            self.close_shortcut(cx);
        }
        cx.notify();
    }

    /// Render one draft where it belongs, directly after its own combination row.
    fn shortcut_editor_panel(&self, pad: &PadView, anchor: u8, t: &Theme, cx: &mut Context<Self>) -> Div {
        let text = self.model.read(cx).text;
        let key = pad.key;
        let Some(e) = self.shortcut_editor.as_ref().filter(|e| e.anchor == anchor) else { return div() };
        let recording = e.recorder.is_some();
        let shown = e.recorder.as_ref().map_or(e.input, |r| r.shown);
        let input_label = if shown == 0 { text.combinations.unassigned.into() } else { names(pad, shown, text) };
        let key_label = if e.key_recording {
            if e.holding == 0 { text.press_key.into() } else { keyboard::chord_names(Chord::new(e.holding, 0)).join(" + ") }
        } else if e.keys.is_empty() {
            text.not_assigned.into()
        } else {
            keyboard::chord_names(e.keys).join(" + ")
        };
        card(t)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(caption(text.combinations.name, t.text2))
            .child(e.name.clone())
            .child(caption(text.combinations.input, t.text2))
            .child(div().flex().flex_wrap().gap(px(8.)).items_center().child(keycap(input_label, t)).child(
                button("shortcut-record", if recording { text.stop } else { text.combinations.record }, Kind::Standard, t).on_click(
                    cx.listener(move |this, _, window, cx| {
                        if this.shortcut_editor.as_ref().is_some_and(|e| e.recorder.is_some()) {
                            this.stop_shortcut_recording(cx);
                        } else {
                            this.record_shortcut(key, cx);
                        }
                        this.focus.focus(window, cx);
                    }),
                ),
            ))
            .when(e.input != 0, |d| {
                d.child(button("shortcut-unassign", text.combinations.unassign, Kind::Subtle, t).on_click(cx.listener(|this, _, _, cx| {
                    this.stop_shortcut_recording(cx);
                    if let Some(e) = this.shortcut_editor.as_mut() {
                        e.input = 0;
                        e.conflict = None;
                    }
                    cx.notify();
                })))
            })
            .when(recording, |d| {
                d.child(caption(open_controller_core::i18n::fill(text.combinations.record_hint, names(pad, 1 << anchor, text)), t.accent))
            })
            .child(caption(text.combinations.output, t.text2))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .items_center()
                    .child(keycap(key_label, t))
                    .child(button("shortcut-key", text.combinations.choose_key, Kind::Standard, t).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.choose_shortcut_key(window, cx);
                        },
                    )))
                    .when(cfg!(windows), |d| {
                        d.child(button("shortcut-mouse", text.configuration.mouse, Kind::Standard, t).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open_mouse_picker(crate::mouse_picker::Target::Shortcut, cx);
                            },
                        )))
                    }),
            )
            .children(self.render_mouse_picker(crate::mouse_picker::Target::Shortcut, t, cx))
            .child(caption(text.combinations.duration_title, t.text2))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        switch(e.fixed_duration, text.combinations.fixed_on, text.combinations.fixed_off, t)
                            .id("shortcut-duration-mode")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.stop_press_meter(cx);
                                this.stop_shortcut_recording(cx);
                                this.stop_shortcut_key_capture(cx);
                                if let Some(e) = this.shortcut_editor.as_mut() {
                                    e.fixed_duration = !e.fixed_duration;
                                    e.error = None;
                                }
                                this.focus.focus(window, cx);
                                cx.notify();
                            })),
                    )
                    .when(e.fixed_duration, |d| d.child(div().w(px(100.)).child(e.hold.clone())).child(caption("ms", t.text2)))
                    .when(!e.fixed_duration, |d| d.child(caption(text.combinations.held_label, t.text2)))
                    .child(button("duration-info", "?", Kind::Subtle, t).size(px(22.)).p(px(0.)).on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(e) = this.shortcut_editor.as_mut() {
                                e.duration_info = !e.duration_info;
                            }
                            cx.notify();
                        },
                    ))),
            )
            .when(e.duration_info, |d| d.child(caption(text.combinations.duration_modes_help, t.text2)))
            .when(e.key_recording && cfg!(windows), |d| d.child(caption(text.combinations.key_capture_hint, t.accent)))
            .children(e.error.as_ref().map(|error| caption(error.clone(), t.critical)))
            .children(e.conflict.as_ref().map(|name| {
                card(t)
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(caption(open_controller_core::i18n::fill(text.combinations.conflict, name), t.text2))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                button("shortcut-replace", text.combinations.replace, Kind::Primary, t)
                                    .on_click(cx.listener(move |this, _, _, cx| this.commit_shortcut(key, true, cx))),
                            )
                            .child(button("shortcut-keep", text.combinations.cancel, Kind::Standard, t).on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some(e) = this.shortcut_editor.as_mut() {
                                        e.conflict = None;
                                    }
                                    cx.notify();
                                },
                            ))),
                    )
            }))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button("shortcut-save", text.combinations.save, Kind::Primary, t)
                            .on_click(cx.listener(move |this, _, _, cx| this.save_shortcut(key, cx))),
                    )
                    .child(
                        button("shortcut-cancel", text.combinations.cancel, Kind::Standard, t)
                            .on_click(cx.listener(|this, _, _, cx| this.close_shortcut(cx))),
                    ),
            )
    }

    pub fn shortcut_panel(&self, pad: &PadView, anchor: u8, t: &Theme, cx: &mut Context<Self>) -> Div {
        let text = self.model.read(cx).text;
        let key = pad.key;
        let pad_profile = pad.profiles.active;
        let mut col = div().flex().flex_col().gap(px(10.)).child(
            div().flex().items_start().gap(px(12.)).child(caption(text.combinations.description, t.text2).flex_1().min_w(px(0.))).child(
                button("combinations-help", "?", Kind::Standard, t)
                    .on_click(cx.listener(|this, _, window, cx| this.open_combination_help(window, cx))),
            ),
        );
        for (i, (&mask, &keys)) in
            pad.profiles.active().shortcuts.iter().filter(|(mask, _)| pad.profiles.active().shortcut_anchor(**mask) == anchor).enumerate()
        {
            col = col.child(
                card(t)
                    .p(px(12.))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(140.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .children(pad.profiles.active().shortcut_names.get(&mask).map(|name| strong(name.clone(), t.text)))
                            .child(caption(
                                names(pad, mask, text),
                                if pad.profiles.active().shortcut_names.contains_key(&mask) { t.text2 } else { t.text },
                            )),
                    )
                    .child(caption("→", t.text2))
                    .child(caption(
                        format!(
                            "{} · {}",
                            keyboard::chord_names(keys).join(" + "),
                            if pad.profiles.active().shortcut_held.contains(&mask) {
                                text.combinations.held_label.into()
                            } else {
                                format!("{} ms", pad.profiles.active().shortcut_hold_ms(mask))
                            }
                        ),
                        t.accent,
                    ))
                    .child(
                        button(("shortcut-edit", i), text.combinations.edit, Kind::Subtle, t)
                            .on_click(cx.listener(move |this, _, window, cx| this.open_shortcut(key, anchor, Some(mask), window, cx))),
                    )
                    .child(button(("shortcut-delete", i), text.delete, Kind::Subtle, t).on_click(cx.listener(move |this, _, _, cx| {
                        this.close_shortcut(cx);
                        this.edit_now(key, Edit::RemoveShortcut { profile: pad_profile, input: mask }, cx);
                    }))),
            );
            if self.shortcut_editor.as_ref().is_some_and(|e| e.anchor == anchor && e.original == Some(mask)) {
                col = col.child(self.shortcut_editor_panel(pad, anchor, t, cx));
            }
        }
        for (i, (&id, action)) in pad.profiles.active().unassigned_shortcuts.iter().filter(|(_, a)| a.anchor == anchor).enumerate() {
            col = col.child(
                card(t)
                    .p(px(12.))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(140.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(strong(
                                if action.name.is_empty() { keyboard::chord_names(action.keys).join(" + ") } else { action.name.clone() },
                                t.text,
                            ))
                            .child(caption(text.combinations.unassigned, t.text3)),
                    )
                    .child(caption(
                        format!(
                            "{} · {}",
                            keyboard::chord_names(action.keys).join(" + "),
                            if action.hold_until_release { text.combinations.held_label.into() } else { format!("{} ms", action.hold_ms) }
                        ),
                        t.accent,
                    ))
                    .child(
                        button(("unassigned-edit", i), text.combinations.edit, Kind::Subtle, t)
                            .on_click(cx.listener(move |this, _, window, cx| this.open_unassigned(key, anchor, id, window, cx))),
                    )
                    .child(button(("unassigned-delete", i), text.delete, Kind::Subtle, t).on_click(cx.listener(move |this, _, _, cx| {
                        this.close_shortcut(cx);
                        if let Some(pad) = this.pad(key, cx) {
                            this.edit_profiles(&pad, Edit::RemoveUnassigned { profile: pad.profiles.active, id }, cx);
                        }
                    }))),
            );
            if self.shortcut_editor.as_ref().is_some_and(|e| e.anchor == anchor && e.unassigned == Some(id)) {
                col = col.child(self.shortcut_editor_panel(pad, anchor, t, cx));
            }
        }
        if let Some(keys) = pad.profiles.active().button_presets.get(&anchor).and_then(|p| p.key) {
            col = col.child(caption(
                open_controller_core::i18n::fill(text.combinations.saved_key, keyboard::chord_names(keys).join(" + ")),
                t.text2,
            ));
        }
        if let Some(e) = self.shortcut_editor.as_ref().filter(|e| e.anchor == anchor) {
            if e.original.is_none() && e.unassigned.is_none() {
                col = col.child(self.shortcut_editor_panel(pad, anchor, t, cx));
            }
        } else {
            col = col.child(
                div().flex().child(
                    button("shortcut-add", text.combinations.add, Kind::Standard, t)
                        .on_click(cx.listener(move |this, _, window, cx| this.open_shortcut(key, anchor, None, window, cx))),
                ),
            );
        }
        col.child(self.press_meter_panel(pad, t, cx))
            .child(caption(text.combinations.native_hint, t.text2))
            .child(self.configuration_panel(pad, t, cx))
    }
}
