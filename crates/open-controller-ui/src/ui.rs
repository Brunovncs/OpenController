//! The window: a bar on top, and under it one of three screens: every controller as a tile, one
//! controller with its buttons, light and details, or the settings with what Windows needs.

use crate::keys;
use crate::requirements::{self, Component, Outcome};
use crate::theme::{FONT, Theme, icon as glyph, layout, radius};
use crate::widgets::{caption, icon_button, strong};
use crate::{Connection, Model};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Image, ImageFormat, InteractiveElement, IntoElement, KeyDownEvent, ModifiersChangedEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, WindowAppearance, div, img, px,
};
use open_controller_core::binding::{self, Action, Step};
use open_controller_core::ipc::ToTray;
use open_controller_core::platform::VirtualDriver;
use open_controller_core::profile::Edit;
use open_controller_core::{PadKey, PadView, Snapshot};
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

#[derive(Clone, PartialEq, Eq)]
pub enum Screen {
    Home,
    /// A controller's page: its key, and where its settings are kept, so another controller that
    /// takes the key (Joy-Cons split or joined) does not inherit the page.
    Device(PadKey, Option<String>),
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
    pub renaming: Option<crate::detail::Rename>,
    pub installs: HashMap<Component, Install>,
    /// What Programs and Features says is installed, read when the settings open.
    pub installed: HashMap<Component, Option<String>>,
    /// Whether a newer version is out, and what is being done about it.
    pub update: crate::updates::Update,
    /// Open programs offered for a profile to switch on with, while the list is shown.
    pub picker: Option<Vec<String>>,
    /// The report on a controller, while its dialog is open.
    pub report: Option<crate::report::Report>,
    /// Whether the gyro's advanced settings are shown, and whether its custom acceleration was
    /// picked (it stays picked when its values match a preset).
    pub gyro_advanced: bool,
    pub gyro_custom: bool,
    /// Whether the list of every button is open; `None` until opened or closed, when it is open if a
    /// button already does something else.
    pub all_buttons: Option<bool>,
    /// With a shape for each stick, whether the right one's is being changed.
    pub stick_right: bool,
    /// The box a stick setting's value is being typed into.
    pub custom: Option<crate::sticks::Custom>,
    /// The open controller's sticks being measured, or the outcome of the last time.
    pub measuring: Option<crate::sticks::Measuring>,
    /// The app's icon, in its version drawn for small sizes, for the bar on top, made for the
    /// window's scale.
    brand: (f32, Arc<Image>),
    /// Extra buttons held on the open controller in the last snapshot, to notice a new press.
    held: u64,
    /// The bar on top was pressed where the window draws its own frame: moving the mouse now
    /// moves the window.
    pub moving: bool,
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
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            // Behind a game, the drawings need not follow every frame of input.
            this.send(ToTray::Focused(window.is_window_active()), cx);
            // GPUI tells of light and dark changing but not of the accent colour, which is
            // picked in Windows' settings: coming back from them makes the window active.
            let now = theme(window);
            if now.accent != this.theme.accent {
                crate::theme::accent_changed();
                this.theme = now;
                cx.notify();
            }
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
            report: None,
            gyro_advanced: false,
            gyro_custom: false,
            all_buttons: None,
            stick_right: false,
            custom: None,
            measuring: None,
            brand: brand(window.scale_factor()),
            held: 0,
            moving: false,
            _subscriptions: vec![observe, theme_change, activation],
        };
        view.read_installed(cx);
        // `--demo` with `OPEN_CONTROLLER_PAGE` set: the first example controller's page, an extra
        // button picked, for the screenshot of a controller's page.
        if view.model.read(cx).demo && std::env::var_os("OPEN_CONTROLLER_PAGE").is_some() {
            let key = PadKey::Slot(1);
            view.open(key, cx);
            if let Some(pad) = view.pad(key, cx) {
                view.edit(&pad, open_controller_core::extras::LEFT_PADDLE1, cx);
            }
        }
        view
    }

    pub fn pad(&self, key: PadKey, cx: &Context<Self>) -> Option<PadView> {
        self.model.read(cx).snapshot.pads.iter().find(|p| p.key == key).cloned()
    }

    /// The snapshot, shared: what is drawn borrows from it instead of copying a controller with
    /// all its profiles on every frame.
    fn snapshot(&self, cx: &Context<Self>) -> Arc<Snapshot> {
        self.model.read(cx).snapshot.clone()
    }

    /// The controller whose page is open, unless it left or another one took its key.
    fn page_pad<'a>(&self, snap: &'a Snapshot) -> Option<&'a PadView> {
        let Screen::Device(key, store) = &self.screen else { return None };
        snap.pads.iter().find(|p| p.key == *key && p.store == *store)
    }

    pub fn send(&self, req: ToTray, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.send(req, cx));
    }

    /// A new connection to the resident process knows nothing of the window: what it was told
    /// before is told again. The snapshot and the preferences come on their own.
    pub fn linked(&self, window: &Window, cx: &mut Context<Self>) {
        self.send(ToTray::Focused(window.is_window_active()), cx);
        let m = self.model.read(cx);
        if let Some(r) = &self.report
            && !m.diagnosis.as_ref().is_some_and(|(k, _)| *k == r.key)
        {
            self.send(ToTray::Diagnose(r.key), cx);
        }
    }

    fn reset_page(&mut self) {
        self.editor = None;
        self.confirm_reset = false;
        self.profile_menu = false;
        self.renaming = None;
        self.picker = None;
        self.gyro_custom = false;
        self.all_buttons = None;
        self.custom = None;
        self.measuring = None;
    }

    pub fn open(&mut self, key: PadKey, cx: &mut Context<Self>) {
        let snap = self.snapshot(cx);
        let pad = snap.pads.iter().find(|p| p.key == key);
        self.screen = Screen::Device(key, pad.and_then(|p| p.store.clone()));
        self.section = Section::Buttons;
        self.reset_page();
        self.stick_right = false;
        self.held = pad.map(extras_held).unwrap_or(0);
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
        self.read_installed(cx);
        cx.notify();
    }

    pub fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.reset_page();
        cx.notify();
    }

    /// Asks what is installed away from the interface thread: on Windows it walks the registry.
    pub fn read_installed(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let found = cx.background_executor().spawn(async { requirements::installed() }).await;
            let _ = this.update(cx, |this, cx| {
                this.installed.extend(found);
                cx.notify();
            });
        })
        .detach();
    }

    /// Whether something keeps controllers from working fully, for the dot on the settings
    /// button and the banner on the home screen.
    pub fn needs_attention(&self, cx: &Context<Self>) -> bool {
        let m = self.model.read(cx);
        let s = &m.snapshot;
        let off = matches!(m.connection, Connection::Lost | Connection::Down);
        off || m.other_version.is_some() || requirements::attention(s, m.text).is_some()
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
                // VIIPER, chosen but missing something, is tried again with what was installed.
                let driver = this.model.read(cx).prefs.virtual_driver;
                if result.is_ok() && driver == VirtualDriver::Viiper {
                    this.send(ToTray::SetVirtualDriver(driver), cx);
                }
                this.installs.insert(
                    c,
                    match result {
                        Ok(o) => Install::Done(o),
                        Err(e) => Install::Failed(e),
                    },
                );
                this.read_installed(cx);
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
                None if open_controller_core::binding::is_normal(button) => Tab::Nothing,
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
        let snap = self.snapshot(cx);
        if let Some(r) = self.report.as_mut() {
            // The controller the report is on, not another one that took its key: that one
            // counts as gone.
            if let Some(p) = snap.pads.iter().find(|p| p.key == r.key && p.store == r.pad.store) {
                r.observe(p);
            }
            // The dialog stays over the page, which is kept as it is meanwhile.
            return;
        }
        if !matches!(self.screen, Screen::Device(..)) {
            return;
        }
        let Some(pad) = self.page_pad(&snap) else {
            self.home(cx);
            return;
        };
        if let Some(m) = self.measuring.as_mut().filter(|m| m.key == pad.key) {
            m.sample(&pad.input);
        }
        let held = extras_held(pad);
        let pressed = held & !self.held;
        self.held = held;
        let listening = self.editor.as_ref().is_some_and(|e| e.recording);
        if pressed != 0 && !listening && self.renaming.is_none() && self.measuring.is_none() {
            let button = pressed.trailing_zeros() as u8;
            self.section = Section::Buttons;
            if self.editor.as_ref().is_none_or(|e| e.button != button) {
                self.edit(pad, button, cx);
            }
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let escape = key == "escape";
        if self.report.is_some() {
            if escape {
                self.close_report(window, cx);
            } else if key == "tab" {
                // Handled here, so a multi-line field does not take it as a tab character.
                cx.stop_propagation();
                self.report_tab(ev.keystroke.modifiers.shift, window, cx);
            }
            return;
        }
        if escape && self.custom.is_some() {
            cx.stop_propagation();
            self.close_custom(window, cx);
            return;
        }
        if self.renaming.is_some() {
            // The box takes the typing, and Enter; Escape leaves the name as it was.
            if escape {
                cx.stop_propagation();
                self.stop_renaming(window, cx);
            }
            return;
        }
        let Screen::Device(pad_key, _) = self.screen else {
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
                    // Through `home`, so a measurement under way stops with the page.
                    self.home(cx);
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
                let room = binding::MAX_STEPS.saturating_sub(e.steps.len());
                if room == 0 {
                    e.recording = false;
                    cx.notify();
                    return;
                }
                if let Some(gap) = e.last_key.map(|t| now.duration_since(t).as_millis() as u64) {
                    // Gaps are kept to the nearest 10 ms; shorter ones are just typing.
                    let ms = ((gap + 5) / 10 * 10).min(binding::MAX_WAIT_MS as u64) as u16;
                    if ms >= 30 && room >= 2 {
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
    fn top_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let framed = crate::frame::tiling(window).is_some();
        let attention = self.needs_attention(cx);
        let on_settings = self.screen == Screen::Settings;
        let mark = img(self.brand.1.clone()).size(px(BRAND)).flex_none();
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
            .when(framed, |d| d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
            .child(mark)
            .child(strong("OpenController", t.text))
            .child(caption(concat!("v", env!("CARGO_PKG_VERSION")), t.text2));
        let gear =
            div()
                .relative()
                .child(icon_button("settings", glyph::SETTINGS, &t).when(on_settings, |d| d.bg(t.control)).on_click(
                    cx.listener(|this, _, _, cx| if this.screen == Screen::Settings { this.home(cx) } else { this.settings(cx) }),
                ))
                .when(attention, |d| d.child(div().absolute().top(px(6.)).right(px(6.)).size(px(8.)).rounded_full().bg(t.caution)))
                .when(framed, |d| d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()));
        let bar = div().flex().flex_none().justify_center().h(px(56.)).border_b_1().border_color(t.stroke);
        let row = div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .max_w(px(layout::MAX_W))
            // The brand's own padding and the gear's hit area sit in the gutter.
            .px(px(layout::GUTTER - 6.))
            .child(home)
            .child(gear);
        self.title_bar(bar, row, window, cx).into_any_element()
    }
}

/// The extra buttons held on a controller, as bits. A button past the 64 bits, which a resident
/// process of another version could send, is left out.
fn extras_held(pad: &PadView) -> u64 {
    let mask = pad.extras.iter().fold(0u64, |m, &b| m | 1u64.checked_shl(u32::from(b)).unwrap_or(0));
    pad.input.buttons & mask
}

/// The small icon for the window's `scale`. GPUI rasterizes an SVG image at twice its own size
/// whatever the scale, and this one is 16 by 16, blurry above 100%: given a size, it comes out
/// with one pixel for each of the screen's.
fn brand(scale: f32) -> (f32, Arc<Image>) {
    let side = BRAND * scale / 2.;
    let svg = include_str!("../../../assets/icon-small.svg").replacen("<svg ", &format!("<svg width=\"{side}\" height=\"{side}\" "), 1);
    (scale, Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
}

/// How big the icon is in the bar on top.
const BRAND: f32 = 28.;

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        if self.brand.0 != window.scale_factor() {
            self.brand = brand(window.scale_factor());
        }
        let max_w = if self.screen == Screen::Settings { layout::SETTINGS_W } else { layout::MAX_W };
        let snap = self.snapshot(cx);
        let content: AnyElement = match self.screen {
            Screen::Home => self.render_home(window, cx),
            Screen::Settings => self.render_settings(window, cx),
            Screen::Device(..) => match self.page_pad(&snap) {
                Some(pad) => self.render_device(pad, window, cx),
                None => self.render_home(window, cx),
            },
        };
        let report = self.render_report(cx);
        let page = div()
            .id("root")
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            .size_full()
            .flex()
            .flex_col()
            .bg(t.base)
            .text_color(t.text)
            .font_family(FONT)
            .child(self.top_bar(window, cx))
            .children(self.update_bar(cx))
            .child(
                div().id("scroll").flex_1().min_h(px(0.)).overflow_y_scroll().child(
                    div()
                        .flex()
                        .justify_center()
                        .w_full()
                        .child(div().w_full().max_w(px(max_w)).px(px(layout::GUTTER)).pt(px(28.)).pb(px(40.)).child(content)),
                ),
            )
            .children(report);
        crate::frame::frame(page.into_any_element(), &t, window)
    }
}
