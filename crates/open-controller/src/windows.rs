//! Windows: a hidden window that receives the session-end broadcast and the other threads'
//! messages, the notification-area icon, and the message loop that drives both.

use crate::tray::Tray;
use crate::{Args, CONTROL, Control, Msg, foreground, open_window};
use open_controller_core::{hidhide, i18n};
use std::cell::RefCell;
use std::sync::Arc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostMessageW, PostQuitMessage, RegisterClassW, TranslateMessage,
    WM_APP, WM_ENDSESSION, WM_SETTINGCHANGE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

/// Posts to the main thread's hidden window. Window messages, unlike thread messages, are not
/// lost while the tray menu runs its modal loop.
#[derive(Clone)]
pub struct Waker(usize);

impl Waker {
    pub fn post(&self, m: Msg) {
        unsafe { PostMessageW(self.0 as HWND, WM_APP + m as u32, 0, 0) };
    }
}

pub struct MainLoop;

/// `--restore`, for the uninstaller: shows again anything a killed run left hidden.
pub fn restore(data_dir: &std::path::Path) -> i32 {
    match hidhide::restore(&data_dir.join(hidhide::JOURNAL_FILE)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("open-controller: {e}");
            1
        }
    }
}

struct MainState {
    control: Arc<Control>,
    tray: Option<Tray>,
}

thread_local! {
    static STATE: RefCell<Option<MainState>> = const { RefCell::new(None) };
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_ENDSESSION && wparam != 0 {
        // Signing out or shutting down: the process may be ended as soon as this returns.
        if let Some(c) = CONTROL.get() {
            c.engine.stop();
        }
        return 0;
    }
    if msg == WM_SETTINGCHANGE {
        // The taskbar may have switched between light and dark.
        STATE.with(|cell| {
            if let Ok(mut s) = cell.try_borrow_mut()
                && let Some(t) = s.as_mut().and_then(|s| s.tray.as_mut())
            {
                t.follow_theme();
            }
        });
    }
    if msg > WM_APP && msg <= WM_APP + Msg::Quit as u32 {
        let handled = STATE.with(|cell| {
            let Ok(mut s) = cell.try_borrow_mut() else { return false };
            let Some(s) = s.as_mut() else { return true };
            match msg - WM_APP {
                1 => {
                    let n = s.control.engine.pad_count();
                    if let Some(t) = s.tray.as_mut() {
                        t.set_count(i18n::text(), n);
                    }
                }
                2 => {
                    if let Some(t) = &s.tray {
                        t.sync(s.control.prefs());
                    }
                }
                3 => open_window(),
                _ => unsafe { PostQuitMessage(0) },
            }
            true
        });
        if !handled {
            // Arrived inside another handler (a nested message loop): handle it afterwards.
            unsafe { PostMessageW(hwnd, msg, wparam, lparam) };
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// A hidden top-level window: it receives the session-end broadcast, which a message-only
/// window would not.
pub fn prepare() -> (Waker, MainLoop) {
    let hwnd = unsafe {
        let class = crate::win::wide("OpenControllerTray");
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: instance, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
        RegisterClassW(&wc);
        let null = std::ptr::null_mut();
        CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), class.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, null, null, instance, std::ptr::null())
    };
    (Waker(hwnd as usize), MainLoop)
}

pub fn run(control: &Arc<Control>, _: MainLoop, args: &Args) {
    let tray = Tray::install(i18n::text(), control.prefs(), control.clone());
    foreground::watch();
    STATE.with_borrow_mut(|s| *s = Some(MainState { control: control.clone(), tray }));
    if !args.minimized {
        open_window();
    }
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    STATE.with_borrow_mut(|s| *s = None);
}
