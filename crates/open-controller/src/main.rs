#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The resident process: the engine, and on Windows the notification-area icon, a few megabytes.
//! The window is a separate program (`open-controller-ui`), started on demand and connected over
//! a pipe, so the GPU-backed interface takes no memory while you play. On Linux and macOS this
//! process runs in the background with no icon of its own; the window is opened from the
//! applications menu or the Dock.

mod autostart;
mod foreground;
mod server;
mod settings;
#[cfg(windows)]
mod tray;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;
#[cfg(windows)]
mod windows;

use open_controller_core::instance::{self, Instance};
use open_controller_core::ipc::{Prefs, ToTray};
use open_controller_core::profile::{Edit, Profiles};
use open_controller_core::{Command, Config, Engine};
use server::Notify;
use settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
#[cfg(unix)]
use unix as sys;
#[cfg(windows)]
use windows as sys;

pub const APP_NAME: &str = "io.github.brunovncs.open-controller";

/// Messages to the main thread, which owns the tray icon on Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    waker: sys::Waker,
}

impl Control {
    pub fn post(&self, m: Msg) {
        self.waker.post(m);
    }

    pub fn prefs(&self) -> Prefs {
        let hide = self.settings.lock().map(|s| s.hide_originals).unwrap_or(true);
        Prefs { hide_originals: hide, autostart: autostart::enabled() }
    }

    /// Changes a controller's profiles, saves them and returns them.
    fn edit_profiles(&self, store: &str, edit: Edit) -> Profiles {
        let Ok(mut s) = self.settings.lock() else { return Profiles::default() };
        let p = s.controllers.entry(store.to_string()).or_default();
        p.apply(edit);
        let out = p.clone();
        if out.is_default() {
            s.controllers.remove(store);
        }
        s.save(&self.data_dir);
        out
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
            ToTray::SwapPlayers(a, b) => return self.engine.send(Command::SwapPlayers(a, b)),
            ToTray::Edit { store, edit } => {
                let profiles = self.edit_profiles(&store, edit);
                return self.engine.send(Command::SetProfiles(store, profiles));
            }
            ToTray::Quit => return self.post(Msg::Quit),
        }
        self.notify.prefs();
        self.post(Msg::Prefs);
    }
}

pub struct Args {
    pub minimized: bool,
    smoke: bool,
    restore: bool,
    quit: bool,
    udev_rule: bool,
    data_dir: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args { minimized: false, smoke: false, restore: false, quit: false, udev_rule: false, data_dir: None };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--minimized" => args.minimized = true,
            "--smoke" => args.smoke = true,
            "--restore" => args.restore = true,
            "--quit" => args.quit = true,
            "--udev-rule" => args.udev_rule = true,
            "--data-dir" => args.data_dir = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    args
}

/// Starts the window, which sits next to this executable.
pub fn open_window() {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    let exe = dir.join(format!("open-controller-ui{}", std::env::consts::EXE_SUFFIX));
    if let Err(e) = std::process::Command::new(exe).spawn() {
        eprintln!("open-controller: could not start the window: {e}");
    }
}

/// Reachable from the window procedure and the foreground watcher without borrowing anything.
pub static CONTROL: OnceLock<Arc<Control>> = OnceLock::new();

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
        std::process::exit(sys::restore(&data_dir));
    }
    if args.udev_rule {
        // For packagers and the install script: the udev rule the window would install.
        #[cfg(target_os = "linux")]
        print!("{}", open_controller_core::linux::setup::rule());
        return;
    }
    let Instance::First { _mutex, show } = instance::claim(APP_NAME) else { return };
    let _ = std::fs::create_dir_all(&data_dir);
    let settings = Settings::load(&data_dir);
    autostart::refresh();

    let (waker, main_loop) = sys::prepare();
    let notify = Arc::new(Notify::default());
    let on_change = {
        let notify = notify.clone();
        let waker = waker.clone();
        move || {
            notify.snapshot();
            waker.post(Msg::Snapshot);
        }
    };
    let config = Config {
        data_dir: data_dir.clone(),
        hide: settings.hide_originals,
        profiles: settings.controllers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    };
    let engine = Engine::start(config, on_change);
    if args.smoke {
        let code = smoke(&engine, &data_dir);
        engine.stop();
        std::process::exit(code);
    }
    let control = Arc::new(Control { engine, settings: Mutex::new(settings), data_dir, notify: notify.clone(), waker });
    let _ = CONTROL.set(control.clone());
    server::start(control.clone(), notify);
    let c = control.clone();
    instance::on_signal(show, move || c.post(Msg::Open));
    if let Some(quit) = instance::event(APP_NAME, "quit") {
        let c = control.clone();
        instance::on_signal(quit, move || c.post(Msg::Quit));
    }

    sys::run(&control, main_loop, &args);
    // Unplugs the virtual controllers and shows the hidden ones again.
    control.engine.stop();
}
