//! The window: a bar on top, and under it one of three screens: every controller as a tile, one
//! controller with its buttons, light and details, or the settings with what Windows needs.

use crate::Model;
use crate::keys;
use crate::requirements::{self, Component, Outcome};
use crate::theme::{FONT, Theme, icon as glyph, radius};
use crate::widgets::{icon_button, strong};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Image, ImageFormat, InteractiveElement, IntoElement, KeyDownEvent, ModifiersChangedEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, WindowAppearance, div, img, px,
};
use open_controller_core::binding::{self, Action, Step};
use open_controller_core::ipc::ToTray;
use open_controller_core::profile::Edit;
use open_controller_core::{PadKey, PadView};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// The theme for the window's appearance. `OPEN_CONTROLLER_THEME=light` or `dark` overrides
/// Windows, for screenshots.
fn theme(window: &Window) -> Theme {
    match std::env::var("OPEN_CONTROLLER_THEME").as_deref() {
        Ok("light") => Theme::new(WindowAppearance::Light),
        Ok("dark") => Theme::new(WindowAppearance::Dark),
        _ => Theme::new(window.appearance()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Device(PadKey),
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Buttons,
    Motion,
    Sticks,
    Light,
    Info,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Xbox,
    Key,
    Macro,
    Nothing,
}

/// The assignment being edited: one extra button of the open controller.
pub struct Editor {
    pub button: u8,
    pub tab: Tab,
    /// Listening for keys: one shortcut on the Key tab, a sequence on the Macro tab.
    pub recording: bool,
    pub steps: Vec<Step>,
    pub last_key: Option<Instant>,
    /// Modifiers held right now, shown while listening.
    pub holding: u8,
    pub other_keys: bool,
}

/// Where installing a driver from the settings stands.
#[derive(Clone, PartialEq, Eq)]
pub enum Install {
    Running,
    Done(Outcome),
    Failed(String),
}

pub struct MainView {
    pub model: Entity<Model>,
    pub theme: Theme,
    pub focus: FocusHandle,
    pub screen: Screen,
    pub section: Section,
    pub editor: Option<Editor>,
    pub confirm_reset: bool,
    pub profile_menu: bool,
    /// A profile being renamed, and its name so far.
    pub renaming: Option<(usize, String)>,
    pub installs: HashMap<Component, Install>,
    /// What Programs and Features says is installed, read when the settings open.
    pub installed: HashMap<Component, Option<String>>,
    /// Whether a newer version is out, and what is being done about it.
    pub update: crate::updates::Update,
    /// Open programs offered for a profile to switch on with, while the list is shown.
    pub picker: Option<Vec<String>>,
    /// The app's icon, in its version drawn for small sizes, for the bar on top.
    brand: Arc<Image>,
    /// Extra buttons held on the open controller in the last snapshot, to notice a new press.
    held: u64,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> MainView {
        let observe = cx.observe(&model, |this, _, cx| {
            this.maybe_check_updates(cx);
            this.follow_snapshot(cx);
            cx.notify();
        });
        let theme_change = cx.observe_window_appearance(window, |this, window, cx| {
            this.theme = theme(window);
            cx.notify();
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut view = MainView {
            model,
            theme: theme(window),
            focus,
            screen: Screen::Home,
            section: Section::Buttons,
            editor: None,
            confirm_reset: false,
            profile_menu: false,
            renaming: None,
            installs: HashMap::new(),
            installed: HashMap::new(),
            update: Default::default(),
            picker: None,
            brand: Arc::new(Image::from_bytes(ImageFormat::Svg, include_bytes!("../../../assets/icon-small.svg").to_vec())),
            held: 0,
            _subscriptions: vec![observe, theme_change],
        };
        view.read_installed();
        view
    }

    pub fn pad(&self, key: PadKey, cx: &Context<Self>) -> Option<PadView> {
        self.model.read(cx).snapshot.pads.iter().find(|p| p.key == key).cloned()
    }

    pub fn send(&self, req: ToTray, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.send(req, cx));
    }

    fn reset_page(&mut self) {
        self.editor = None;
        self.confirm_reset = false;
        self.profile_menu = false;
        self.renaming = None;
        self.picker = None;
    }

    pub fn open(&mut self, key: PadKey, cx: &mut Context<Self>) {
        self.screen = Screen::Device(key);
        self.section = Section::Buttons;
        self.reset_page();
        self.held = self.pad(key, cx).map(|p| extras_held(&p)).unwrap_or(0);
        cx.notify();
    }

    pub fn home(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Home;
        self.reset_page();
        cx.notify();
    }

    pub fn settings(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Settings;
        self.reset_page();
        self.read_installed();
        cx.notify();
    }

    pub fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.reset_page();
        cx.notify();
    }

    pub fn read_installed(&mut self) {
        self.installed.extend(requirements::installed());
    }

    /// Whether something keeps controllers from working fully, for the dot on the settings
    /// button and the banner on the home screen.
    pub fn needs_attention(&self, cx: &Context<Self>) -> bool {
        let m = self.model.read(cx);
        let s = &m.snapshot;
        !m.connected || requirements::attention(s, m.text).is_some()
    }

    /// Downloads and runs a driver's installer, away from the interface thread.
    pub fn install(&mut self, c: Component, silent: bool, cx: &mut Context<Self>) {
        if self.installs.get(&c) == Some(&Install::Running) {
            return;
        }
        self.installs.insert(c, Install::Running);
        cx.notify();
        let dir = std::env::temp_dir().join("open-controller-setup");
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { requirements::install(c, &dir, silent) }).await;
            let _ = this.update(cx, |this, cx| {
                this.installs.insert(
                    c,
                    match result {
                        Ok(o) => Install::Done(o),
                        Err(e) => Install::Failed(e),
                    },
                );
                this.read_installed();
                cx.notify();
            });
        })
        .detach();
    }

    /// Changes the open controller's profiles.
    pub fn edit_profiles(&self, pad: &PadView, edit: Edit, cx: &mut Context<Self>) {
        if let Some(store) = pad.store.clone() {
            self.send(ToTray::Edit { store, edit }, cx);
        }
    }

    /// Opens the editor for `button`, on the tab of what it does now.
    pub fn edit(&mut self, pad: &PadView, button: u8, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(|e| e.button == button) {
            self.editor = None;
        } else {
            let current = pad.profiles.active().bindings.get(&button);
            let keys_only = crate::detail::native(pad);
            let tab = match current {
                None if keys_only => Tab::Key,
                Some(Action::Xbox(_)) | None => Tab::Xbox,
                Some(Action::Keys(_)) => Tab::Key,
                Some(Action::Macro(_)) => Tab::Macro,
            };
            let steps = match current {
                Some(Action::Macro(s)) => s.clone(),
                _ => Vec::new(),
            };
            self.editor = Some(Editor { button, tab, recording: false, steps, last_key: None, holding: 0, other_keys: false });
        }
        self.confirm_reset = false;
        cx.notify();
    }

    pub fn bind(&self, pad: &PadView, button: u8, action: Option<Action>, cx: &mut Context<Self>) {
        self.edit_profiles(pad, Edit::Bind { button, action }, cx);
    }

    /// Keeps the screen in step with the controllers: a controller that left closes its page,
    /// and pressing one of its extra buttons picks that button.
    fn follow_snapshot(&mut self, cx: &mut Context<Self>) {
        let Screen::Device(key) = self.screen else { return };
        let Some(pad) = self.pad(key, cx) else {
            self.home(cx);
            return;
        };
        let held = extras_held(&pad);
        let pressed = held & !self.held;
        self.held = held;
        let listening = self.editor.as_ref().is_some_and(|e| e.recording);
        if pressed != 0 && !listening && self.renaming.is_none() {
            let button = pressed.trailing_zeros() as u8;
            self.section = Section::Buttons;
            if self.editor.as_ref().is_none_or(|e| e.button != button) {
                self.edit(&pad, button, cx);
            }
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let escape = key == "escape";
        if let Some((i, name)) = self.renaming.as_mut() {
            cx.stop_propagation();
            match key {
                "escape" => self.renaming = None,
                "enter" => {
                    let (i, name) = (*i, name.clone());
                    self.renaming = None;
                    if let Screen::Device(k) = self.screen
                        && let Some(pad) = self.pad(k, cx)
                    {
                        self.edit_profiles(&pad, Edit::Rename(i, name), cx);
                    }
                }
                "backspace" => {
                    name.pop();
                }
                _ if !ev.keystroke.modifiers.control && !ev.keystroke.modifiers.alt => {
                    if let Some(c) = ev.keystroke.key_char.as_deref() {
                        name.push_str(c);
                    }
                }
                _ => {}
            }
            cx.notify();
            return;
        }
        let Screen::Device(pad_key) = self.screen else {
            if escape && self.screen == Screen::Settings {
                self.home(cx);
            }
            return;
        };
        let Some(e) = self.editor.as_mut().filter(|e| e.recording) else {
            if escape {
                if self.profile_menu {
                    self.profile_menu = false;
                } else if self.editor.is_some() {
                    self.editor = None;
                } else {
                    self.screen = Screen::Home;
                }
                cx.notify();
            }
            return;
        };
        cx.stop_propagation();
        if escape || ev.is_held {
            e.recording = !escape && e.recording;
            cx.notify();
            return;
        }
        let Some(chord) = keys::chord(&ev.keystroke) else { return };
        let button = e.button;
        let action = match e.tab {
            Tab::Key => {
                e.recording = false;
                Action::Keys(chord)
            }
            _ => {
                let now = Instant::now();
                if let Some(gap) = e.last_key.map(|t| now.duration_since(t).as_millis() as u64) {
                    // Gaps are kept to the nearest 10 ms; shorter ones are just typing.
                    let ms = ((gap + 5) / 10 * 10).min(binding::MAX_WAIT_MS as u64) as u16;
                    if ms >= 30 {
                        e.steps.push(Step::Wait(ms));
                    }
                }
                e.steps.push(Step::Keys(chord));
                e.last_key = Some(now);
                if e.steps.len() >= binding::MAX_STEPS - 1 {
                    e.recording = false;
                }
                Action::Macro(e.steps.clone())
            }
        };
        if let Some(pad) = self.pad(pad_key, cx) {
            self.bind(&pad, button, Some(action), cx);
        }
        cx.notify();
    }

    fn modifiers_changed(&mut self, ev: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = self.editor.as_mut().filter(|e| e.recording) {
            e.holding = keys::mods(&ev.modifiers);
            cx.notify();
        }
    }

    /// The bar across the top: where you are, and the way to the settings.
    fn top_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let attention = self.needs_attention(cx);
        let on_settings = self.screen == Screen::Settings;
        let mark = img(self.brand.clone()).size(px(28.)).flex_none();
        let home = div()
            .id("brand")
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(6.))
            .py(px(4.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover(move |s| s.bg(t.control))
            .on_click(cx.listener(|this, _, _, cx| this.home(cx)))
            .child(mark)
            .child(strong("Open Controller", t.text));
        let gear =
            div()
                .relative()
                .child(icon_button("settings", glyph::SETTINGS, &t).when(on_settings, |d| d.bg(t.control)).on_click(
                    cx.listener(|this, _, _, cx| if this.screen == Screen::Settings { this.home(cx) } else { this.settings(cx) }),
                ))
                .when(attention, |d| d.child(div().absolute().top(px(6.)).right(px(6.)).size(px(8.)).rounded_full().bg(t.caution)));
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(56.))
            .px(px(18.))
            .border_b_1()
            .border_color(t.stroke)
            .child(home)
            .child(gear)
            .into_any_element()
    }
}

/// The extra buttons held on a controller, as bits.
fn extras_held(pad: &PadView) -> u64 {
    let mask = pad.extras.iter().fold(0u64, |m, &b| m | 1 << b);
    pad.input.buttons & mask
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let content: AnyElement = match self.screen {
            Screen::Home => self.render_home(window, cx),
            Screen::Settings => self.render_settings(window, cx),
            Screen::Device(key) => match self.pad(key, cx) {
                Some(pad) => self.render_device(&pad, window, cx),
                None => self.render_home(window, cx),
            },
        };
        div()
            .id("root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            .size_full()
            .flex()
            .flex_col()
            .bg(t.base)
            .text_color(t.text)
            .font_family(FONT)
            .child(self.top_bar(cx))
            .children(self.update_bar(cx))
            .child(
                div()
                    .id("scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(div().w_full().max_w(px(1120.)).px(px(28.)).pt(px(24.)).pb(px(32.)).child(content)),
            )
    }
}
