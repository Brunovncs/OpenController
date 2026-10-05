//! The notification-area icon and its menu. Open Controller lives here; the window is opened
//! from it and closed without stopping anything.

use crate::{Control, Msg};
use open_controller_core::i18n::Text;
use open_controller_core::ipc::{Prefs, ToTray};
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub struct Tray {
    icon: TrayIcon,
    hide: CheckMenuItem,
    autostart: CheckMenuItem,
    count: Option<usize>,
}

impl Tray {
    pub fn install(text: &'static Text, prefs: Prefs, control: Arc<Control>) -> Option<Tray> {
        let open = MenuItem::new(text.open, true, None);
        let hide = CheckMenuItem::new(text.hide_originals, true, prefs.hide_originals, None);
        let autostart = CheckMenuItem::new(text.start_with_windows, true, prefs.autostart, None);
        let quit = MenuItem::new(text.quit, true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &PredefinedMenuItem::separator(), &hide, &autostart, &PredefinedMenuItem::separator(), &quit]).ok()?;

        let ids = (open.id().clone(), hide.id().clone(), autostart.id().clone(), quit.id().clone());
        let c = control.clone();
        // Both handlers run on the main thread, inside the icon's window procedure.
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == ids.0 {
                c.post(Msg::Open);
            } else if e.id == ids.1 {
                let on = !c.prefs().hide_originals;
                c.apply(ToTray::SetHiding(on));
            } else if e.id == ids.2 {
                let on = !c.prefs().autostart;
                c.apply(ToTray::SetAutostart(on));
            } else if e.id == ids.3 {
                c.apply(ToTray::Quit);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if matches!(e, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                control.post(Msg::Open);
            }
        }));

        let icon = TrayIconBuilder::new()
            .with_icon(Icon::from_resource(1, None).ok()?)
            .with_tooltip("Open Controller")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .ok()?;
        Some(Tray { icon, hide, autostart, count: None })
    }

    pub fn sync(&self, prefs: Prefs) {
        self.hide.set_checked(prefs.hide_originals);
        self.autostart.set_checked(prefs.autostart);
    }

    pub fn set_count(&mut self, text: &Text, n: usize) {
        if self.count == Some(n) {
            return;
        }
        self.count = Some(n);
        let what = if n == 1 { text.controllers_one.to_string() } else { format!("{n} {}", text.controllers_many) };
        let _ = self.icon.set_tooltip(Some(format!("Open Controller · {what}")));
    }
}
