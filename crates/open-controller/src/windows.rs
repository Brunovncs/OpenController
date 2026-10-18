//! Windows: a hidden window that receives the session-end broadcast and the other threads'
//! messages, the notification-area icon, and the message loop that drives both.

use crate::tray::Tray;
use crate::{Args, CONTROL, Control, Msg, foreground, open_window};
use open_controller_core::{hidhide, i18n};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
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

/// A window that fails to start is only written to the log here.
pub fn window_failed(_: i18n::Lang) {}

/// Whether this session is the one at the console. `None` while the console is between
/// sessions, as during a switch of user.
fn at_console() -> Option<bool> {
    let console = unsafe { WTSGetActiveConsoleSessionId() };
    let mut mine = 0u32;
    if console == u32::MAX || unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut mine) } == 0 {
        return None;
    }
    Some(console == mine)
}

/// Runs the engine while this session is at the console and stops it, which unplugs its
/// controllers and shows its hidden ones, while another user's session is (fast user switching)
/// or this one is a remote desktop. Asked every two seconds, on a thread of its own so stopping
/// never holds up the tray: the console's session is read from memory the kernel shares, so
/// asking costs nothing, and unlike session notifications it needs no service to be running.
pub fn follow_console(control: Arc<Control>) {
    if at_console() != Some(false) {
        control.resume();
    }
    let _ = std::thread::Builder::new().name("console".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(2));
            match at_console() {
                Some(true) => control.resume(),
                Some(false) => control.engine.pause(),
                None => {}
            }
        }
    });
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
                // A snapshot changes only the count; the preferences (a registry read and the
                // whole menu) only when they change.
                1 => {
                    let n = s.control.engine.pad_count();
                    let lang = s.control.lang();
                    if let Some(t) = s.tray.as_mut() {
                        t.set_count(lang, n);
                    }
                }
                2 => {
                    let (n, prefs) = (s.control.engine.pad_count(), s.control.prefs());
                    if let Some(t) = s.tray.as_mut() {
                        t.sync(prefs);
                        t.set_count(prefs.lang, n);
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
    let prefs = control.prefs();
    let tray = Tray::install(i18n::text(prefs.lang), prefs, control.clone());
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
