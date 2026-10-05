#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The resident process: the engine and the notification-area icon, a few megabytes. The
//! window is a separate program (`open-controller-ui.exe`), started on demand and connected
//! over a pipe, so the GPU-backed interface takes no memory while you play.

mod autostart;
mod server;
mod settings;
mod tray;
mod win;

use open_controller_core::i18n;
use open_controller_core::instance::{self, Instance};
use open_controller_core::ipc::{Prefs, ToTray};
use open_controller_core::{Command, Config, Engine, hidhide};
use server::Notify;
use settings::Settings;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tray::Tray;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostMessageW, PostQuitMessage, RegisterClassW, TranslateMessage,
    WM_APP, WM_ENDSESSION, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

pub const APP_NAME: &str = "io.github.brunovncs.open-controller";

/// Messages to the main thread, which owns the tray icon.
#[derive(Clone, Copy)]
pub enum Msg {
    Snapshot = 1,
    Prefs = 2,
    Open = 3,
    Quit = 4,
}

/// What every thread shares: the engine and the preferences.
pub struct Control {
    pub engine: Engine,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    notify: Arc<Notify>,
    /// The main thread's hidden window. Window messages, unlike thread messages, are not lost
    /// while the tray menu runs its modal loop.
    hwnd: HWND,
}

// The window handle is only used to post messages, which any thread may do.
unsafe impl Send for Control {}
unsafe impl Sync for Control {}

impl Control {
    pub fn post(&self, m: Msg) {
        unsafe { PostMessageW(self.hwnd, WM_APP + m as u32, 0, 0) };
    }

    pub fn prefs(&self) -> Prefs {
        let hide = self.settings.lock().map(|s| s.hide_originals).unwrap_or(true);
        Prefs { hide_originals: hide, autostart: autostart::enabled() }
    }

    pub fn apply(&self, req: ToTray) {
        match req {
            ToTray::SetHiding(on) => {
                if let Ok(mut s) = self.settings.lock() {
                    s.hide_originals = on;
                    s.save(&self.data_dir);
                }
                self.engine.send(Command::SetHiding(on));
            }
            ToTray::SetAutostart(on) => {
                autostart::set(on);
            }
            ToTray::Identify(key) => return self.engine.send(Command::Identify(key)),
            ToTray::PowerOff(key) => return self.engine.send(Command::PowerOff(key)),
            ToTray::Quit => return self.post(Msg::Quit),
        }
        self.notify.prefs();
        self.post(Msg::Prefs);
    }
}

struct Args {
    minimized: bool,
    smoke: bool,
    restore: bool,
    quit: bool,
    data_dir: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args { minimized: false, smoke: false, restore: false, quit: false, data_dir: None };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--minimized" => args.minimized = true,
            "--smoke" => args.smoke = true,
            "--restore" => args.restore = true,
            "--quit" => args.quit = true,
            "--data-dir" => args.data_dir = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    args
}

/// Starts the window, which sits next to this executable.
fn open_window() {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    if let Err(e) = std::process::Command::new(dir.join("open-controller-ui.exe")).spawn() {
        eprintln!("open-controller: could not start the window: {e}");
    }
}

struct MainState {
    control: Arc<Control>,
    tray: Option<Tray>,
}

thread_local! {
    static STATE: RefCell<Option<MainState>> = const { RefCell::new(None) };
}

/// Reachable without borrowing `STATE`: the session-end message is sent, so it can arrive while
/// a handler below is waiting on the shell and holds that borrow.
static CONTROL: OnceLock<Arc<Control>> = OnceLock::new();

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_ENDSESSION && wparam != 0 {
        // Signing out or shutting down: the process may be ended as soon as this returns.
        if let Some(c) = CONTROL.get() {
            c.engine.stop();
        }
        return 0;
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
fn hidden_window() -> HWND {
    unsafe {
        let class = win::wide("OpenControllerTray");
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: instance, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
        RegisterClassW(&wc);
        let null = std::ptr::null_mut();
        CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), class.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, null, null, instance, std::ptr::null())
    }
}

/// `--smoke`: the engine must come up (SDL initialised; drivers may be missing, as on CI).
fn smoke(engine: &Engine, data_dir: &std::path::Path) -> i32 {
    let start = Instant::now();
    loop {
        let snap = engine.snapshot();
        if (snap.running && snap.sdl_error.is_none()) || start.elapsed() > Duration::from_secs(20) {
            let ok = snap.running && snap.sdl_error.is_none();
            let line = format!(
                "smoke: running={} sdl_error={:?} vigem={:?} hidhide={:?} pads={}",
                snap.running,
                snap.sdl_error,
                snap.vigem,
                snap.hidhide,
                snap.pads.len()
            );
            println!("{line}");
            let _ = std::fs::write(data_dir.join("smoke.log"), format!("{line}\n"));
            return if ok { 0 } else { 1 };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn main() {
    let args = parse_args();
    let data_dir = args.data_dir.clone().unwrap_or_else(settings::default_data_dir);
    if args.quit {
        // For the uninstaller: ask the running instance to quit the clean way and wait for it.
        let done = !instance::signal(APP_NAME, "quit") || instance::wait_gone(APP_NAME, Duration::from_secs(10));
        std::process::exit(if done { 0 } else { 1 });
    }
    if args.restore {
        // For the uninstaller: show again anything a killed run left hidden, and unregister.
        let code = match hidhide::restore(&data_dir.join(hidhide::JOURNAL_FILE)) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("open-controller: {e}");
                1
            }
        };
        std::process::exit(code);
    }
    let Instance::First { _mutex, show } = instance::claim(APP_NAME) else { return };
    let _ = std::fs::create_dir_all(&data_dir);
    let settings = Settings::load(&data_dir);
    autostart::refresh();

    let hwnd = hidden_window();
    let notify = Arc::new(Notify::default());
    let on_change = {
        let notify = notify.clone();
        let hwnd = hwnd as usize;
        move || {
            notify.snapshot();
            unsafe { PostMessageW(hwnd as HWND, WM_APP + Msg::Snapshot as u32, 0, 0) };
        }
    };
    let engine = Engine::start(Config { data_dir: data_dir.clone(), hide: settings.hide_originals }, on_change);
    if args.smoke {
        let code = smoke(&engine, &data_dir);
        engine.stop();
        std::process::exit(code);
    }
    let control = Arc::new(Control { engine, settings: Mutex::new(settings), data_dir, notify: notify.clone(), hwnd });
    let _ = CONTROL.set(control.clone());
    server::start(control.clone(), notify);
    let c = control.clone();
    instance::on_signal(show, move || c.post(Msg::Open));
    if let Some(quit) = instance::event(APP_NAME, "quit") {
        let c = control.clone();
        instance::on_signal(quit, move || c.post(Msg::Quit));
    }

    let tray = Tray::install(i18n::text(), control.prefs(), control.clone());
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
    // Unplugs the virtual controllers and shows the hidden ones again.
    control.engine.stop();
}
