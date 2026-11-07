//! One portable file: controller profiles plus a reusable library of named outputs.
use crate::{
    theme::Theme,
    ui::MainView,
    widgets::{Kind, button, caption, card, strong},
};
use gpui::{Context, Div, ParentElement, PathPromptOptions, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};
use open_controller_core::{
    PadKey, PadView,
    i18n::fill,
    ipc::ToTray,
    portable::{self, ControllerFile},
};
use std::path::PathBuf;

#[derive(Default)]
pub struct ConfigUi {
    pub busy: bool,
    pub status: Option<(bool, String)>,
    pub pending: Option<(PadKey, usize, u8, ControllerFile)>,
    pub directory: Option<PathBuf>,
}

impl MainView {
    fn config_error(&mut self, error: impl ToString, cx: &mut Context<Self>) {
        self.config.busy = false;
        self.config.status = Some((false, fill(self.model.read(cx).text.configuration.failed, error.to_string())));
        cx.notify();
    }

    fn pause_assignment(&mut self, cx: &mut Context<Self>) {
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        self.mouse_picker = None;
        if let Some(e) = self.editor.as_mut() {
            e.recording = false;
        }
    }

    fn save_configuration(&mut self, key: PadKey, cx: &mut Context<Self>) {
        if self.config.busy {
            return;
        }
        let Some(pad) = self.pad(key, cx).filter(|p| p.store.is_some()) else {
            return;
        };
        let file = match ControllerFile::new(&pad, Default::default()) {
            Ok(file) => file,
            Err(e) => {
                self.config_error(e, cx);
                return;
            }
        };
        self.pause_assignment(cx);
        self.config.busy = true;
        self.config.pending = None;
        self.config.status = None;
        let directory = self.config.directory.clone().or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from)).unwrap_or_default();
        let name: String = pad.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        let dialog = cx.prompt_for_new_path(&directory, Some(&format!("OpenController-{name}.json")));
        cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.config.busy = false;
                        cx.notify();
                    });
                    return;
                }
                other => {
                    let _ = this.update(cx, |this, cx| this.config_error(format!("{other:?}"), cx));
                    return;
                }
            };
            let target = path.clone();
            let actions = file.actions.clone();
            let result = cx.background_executor().spawn(async move { portable::write(&target, &file) }).await;
            let _ = this.update(cx, |this, cx| {
                this.config.busy = false;
                match result {
                    Ok(()) => {
                        this.config.directory = path.parent().map(|p| p.to_owned());
                        this.config.status = Some((true, fill(this.model.read(cx).text.configuration.saved, path.display())));
                        this.send(ToTray::ApplyConfiguration { store: None, profiles: None, actions }, cx);
                    }
                    Err(e) => this.config_error(e, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_configuration(&mut self, key: PadKey, cx: &mut Context<Self>) {
        if self.config.busy {
            return;
        }
        self.pause_assignment(cx);
        self.config.busy = true;
        self.config.pending = None;
        self.config.status = None;
        let Some(pad) = self.pad(key, cx) else {
            self.config.busy = false;
            return;
        };
        let profile = pad.profiles.active;
        let anchor = self.editor.as_ref().map_or(15, |e| e.button);
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.model.read(cx).text.configuration.load_file.into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(paths))) if !paths.is_empty() => paths[0].clone(),
                Ok(Ok(_)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.config.busy = false;
                        cx.notify();
                    });
                    return;
                }
                other => {
                    let _ = this.update(cx, |this, cx| this.config_error(format!("{other:?}"), cx));
                    return;
                }
            };
            let target = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { portable::read::<ControllerFile>(&target).and_then(ControllerFile::validate) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.config.busy = false;
                match result {
                    Ok(file) => {
                        this.config.directory = path.parent().map(|p| p.to_owned());
                        if this.pad(key, cx).is_some() {
                            this.config.pending = Some((key, profile, anchor, file));
                        }
                    }
                    Err(e) => this.config_error(e, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn apply_configuration(&mut self, key: PadKey, full: bool, cx: &mut Context<Self>) {
        let Some((target, profile, anchor, file)) = self.config.pending.as_ref() else {
            return;
        };
        if *target != key {
            return;
        }
        let Some(pad) = self.pad(key, cx).filter(|p| p.store.is_some()) else {
            return;
        };
        if *profile != pad.profiles.active {
            self.config.pending = None;
            cx.notify();
            return;
        }
        let profiles =
            match if full { file.profiles_for(&(&pad).into(), true) } else { file.global_profiles(&pad.profiles, *anchor).map(Some) } {
                Ok(profiles) => profiles.map(Box::new),
                Err(e) => {
                    self.config_error(e, cx);
                    return;
                }
            };
        let actions = match self.model.read(cx).action_library.clone().merge(file.actions.clone()) {
            Ok(actions) => actions,
            Err(e) => {
                self.config_error(e, cx);
                return;
            }
        };
        let store = pad.store.clone();
        self.close_shortcut(cx);
        self.send(ToTray::ApplyConfiguration { store, profiles, actions }, cx);
        self.config.pending = None;
        let t = &self.model.read(cx).text.configuration;
        self.config.status = Some((true, if full { t.loaded } else { t.global_loaded }.into()));
        cx.notify();
    }

    pub fn configuration_panel(&self, pad: &PadView, t: &Theme, cx: &mut Context<Self>) -> Div {
        let text = &self.model.read(cx).text.configuration;
        let key = pad.key;
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .mt(px(18.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        button("config-save", text.save_file, Kind::Standard, t)
                            .on_click(cx.listener(move |this, _, _, cx| this.save_configuration(key, cx))),
                    )
                    .child(
                        button("config-load", text.load_file, Kind::Standard, t)
                            .on_click(cx.listener(move |this, _, _, cx| this.load_configuration(key, cx))),
                    ),
            )
            .child(caption(if self.config.busy { text.busy } else { text.file_hint }, t.text2));
        if let Some((ok, message)) = &self.config.status {
            panel = panel.child(caption(message.clone(), if *ok { t.text2 } else { t.critical }));
        }
        if let Some((target, profile, _, file)) = &self.config.pending
            && *target == key
            && *profile == pad.profiles.active
        {
            let matches = file.controller.compatible(&pad.into());
            panel = panel.child(
                card(t)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(strong(if matches { text.match_title } else { text.mismatch_title }, if matches { t.text } else { t.critical }))
                    .child(caption(
                        fill(
                            text.source,
                            format!("{} ({:04X}:{:04X})", file.controller.name, file.controller.vendor, file.controller.product),
                        ),
                        t.text2,
                    ))
                    .child(caption(fill(text.current, format!("{} ({:04X}:{:04X})", pad.name, pad.vendor, pad.product)), t.text2))
                    .child(caption(if matches { text.match_hint } else { text.mismatch_hint }, t.text2))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.))
                            .when(matches, |d| {
                                d.child(
                                    button("config-full", text.full, Kind::Primary, t)
                                        .on_click(cx.listener(move |this, _, _, cx| this.apply_configuration(key, true, cx))),
                                )
                            })
                            .child(
                                button("config-global", text.global, if matches { Kind::Standard } else { Kind::Primary }, t)
                                    .on_click(cx.listener(move |this, _, _, cx| this.apply_configuration(key, false, cx))),
                            )
                            .child(button("config-cancel", text.cancel, Kind::Subtle, t).on_click(cx.listener(|this, _, _, cx| {
                                this.config.pending = None;
                                cx.notify();
                            }))),
                    ),
            );
        }
        panel
    }
}
