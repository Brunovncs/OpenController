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
    /// Whether the icon drawn is the one for a light taskbar.
    light: bool,
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

        let light = light_taskbar();
        let icon = TrayIconBuilder::new()
            .with_icon(tray_icon(light)?)
            .with_tooltip("Open Controller")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .ok()?;
        Some(Tray { icon, light, hide, autostart, count: None })
    }

    /// Follows the taskbar between light and dark: a white icon disappears on a light one.
    pub fn follow_theme(&mut self) {
        let light = light_taskbar();
        if light != self.light
            && let Some(icon) = tray_icon(light)
        {
            let _ = self.icon.set_icon(Some(icon));
            self.light = light;
        }
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

/// The notification-area icon: resource 2 for a dark taskbar, 3 for a light one, at the size
/// Windows uses there.
fn tray_icon(light: bool) -> Option<Icon> {
    Icon::from_resource(if light { 3 } else { 2 }, None).ok()
}

/// Whether the taskbar is light, which Windows keeps apart from the apps' own light or dark mode.
fn light_taskbar() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let key = crate::win::wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = crate::win::wide("SystemUsesLightTheme");
    let mut data = 0u32;
    let mut len = size_of::<u32>() as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut len,
        )
    };
    ok == 0 && data != 0
}
