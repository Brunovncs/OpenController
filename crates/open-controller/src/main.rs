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

use open_controller_core::i18n::Lang;
use open_controller_core::instance::{self, Instance};
use open_controller_core::ipc::{Prefs, ToTray};
use open_controller_core::platform::VirtualDriver;
use open_controller_core::profile::{Edit, Profiles};
use open_controller_core::report::Diagnosis;
use open_controller_core::{Command, Config, Engine, PadKey, Snapshot};
use server::Notify;
use settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};
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

/// The engine, while it runs. On Windows it runs only while this session is the one at the
/// console: ViGEmBus and HidHide serve the whole machine, so with two users signed in an engine
/// in each session would plug every controller in twice, and each would undo the other's hiding.
pub struct Input {
    engine: RwLock<Option<Engine>>,
    /// Held while the engine starts or stops, so the last stop waits for one under way; true
    /// once the engine has stopped for good.
    turning: Mutex<bool>,
    on_change: Arc<dyn Fn() + Send + Sync>,
    /// What the window and the foreground watcher last said, for an engine started again.
    watching: AtomicBool,
    focused: AtomicBool,
    foreground: Mutex<Option<String>>,
}

impl Input {
    fn new(on_change: impl Fn() + Send + Sync + 'static) -> Input {
        Input {
            engine: RwLock::new(None),
            turning: Mutex::new(false),
            on_change: Arc::new(on_change),
            watching: AtomicBool::new(false),
            focused: AtomicBool::new(true),
            foreground: Mutex::new(None),
        }
    }

    pub fn send(&self, c: Command) {
        // Noted before the engine is reached, so an engine starting meanwhile gets it either way.
        match &c {
            Command::Watch(on) => {
                self.watching.store(*on, Ordering::Release);
                self.focused.store(true, Ordering::Release);
            }
            Command::Focused(on) => self.focused.store(*on, Ordering::Release),
            Command::Foreground(p) => *self.foreground.lock().unwrap_or_else(PoisonError::into_inner) = Some(p.clone()),
            _ => {}
        }
        if let Some(e) = self.engine.read().unwrap_or_else(PoisonError::into_inner).as_ref() {
            e.send(c);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.engine.read().unwrap_or_else(PoisonError::into_inner).as_ref().map(Engine::snapshot).unwrap_or_default()
    }

    pub fn pad_count(&self) -> usize {
        self.engine.read().unwrap_or_else(PoisonError::into_inner).as_ref().map_or(0, Engine::pad_count)
    }

    pub fn diagnose(&self, key: PadKey) -> Option<Diagnosis> {
        self.engine.read().unwrap_or_else(PoisonError::into_inner).as_ref().and_then(|e| e.diagnose(key))
    }

    /// Starts the engine, unless it runs already or has stopped for good.
    fn start(&self, config: impl FnOnce() -> Config) {
        let ended = self.turning.lock().unwrap_or_else(PoisonError::into_inner);
        let mut slot = self.engine.write().unwrap_or_else(PoisonError::into_inner);
        if *ended || slot.is_some() {
            return;
        }
        let on_change = self.on_change.clone();
        let engine = Engine::start(config(), move || on_change());
        if self.watching.load(Ordering::Acquire) {
            engine.send(Command::Watch(true));
            engine.send(Command::Focused(self.focused.load(Ordering::Acquire)));
        }
        if let Some(p) = self.foreground.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            engine.send(Command::Foreground(p));
        }
        *slot = Some(engine);
    }

    /// Unplugs the virtual controllers, shows the hidden ones again and waits for the engine's
    /// thread. `for_good` keeps it from starting again.
    fn halt(&self, for_good: bool) {
        let mut ended = self.turning.lock().unwrap_or_else(PoisonError::into_inner);
        *ended |= for_good;
        let engine = self.engine.write().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(e) = engine {
            e.stop();
            (self.on_change)();
        }
    }

    /// Until `start`: this session has left the console.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn pause(&self) {
        self.halt(false);
    }

    pub fn stop(&self) {
        self.halt(true);
    }
}

/// What every thread shares: the engine and the preferences.
pub struct Control {
    pub engine: Input,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    notify: Arc<Notify>,
    waker: sys::Waker,
}

impl Control {
    pub fn post(&self, m: Msg) {
        self.waker.post(m);
    }

    /// Starts the engine with the current settings, if it is not running.
    pub fn resume(&self) {
        self.engine.start(|| {
            let s = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
            config(&self.data_dir, &s)
        });
    }

    pub fn prefs(&self) -> Prefs {
        let (hide, updates, lang, driver, split) = self
            .settings
            .lock()
            .map(|s| (s.hide_originals, s.check_updates, s.language, s.virtual_driver, s.split_joycons))
            .unwrap_or((true, true, Lang::En, VirtualDriver::ViGEmBus, false));
        Prefs {
            hide_originals: hide,
            autostart: autostart::enabled(),
            check_updates: updates,
            lang,
            virtual_driver: driver,
            split_joycons: split,
        }
    }

    pub fn lang(&self) -> Lang {
        self.settings.lock().map(|s| s.language).unwrap_or(Lang::En)
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
            ToTray::SetCheckUpdates(on) => {
                if let Ok(mut s) = self.settings.lock() {
                    s.check_updates = on;
                    s.save(&self.data_dir);
                }
            }
            ToTray::SetLanguage(lang) => {
                if let Ok(mut s) = self.settings.lock() {
                    s.language = lang;
                    s.save(&self.data_dir);
                }
            }
            ToTray::SetVirtualDriver(driver) => {
                if let Ok(mut s) = self.settings.lock() {
                    s.virtual_driver = driver;
                    s.save(&self.data_dir);
                }
                self.engine.send(Command::SetVirtualDriver(driver));
            }
            ToTray::SplitJoyCons(on) => {
                if let Ok(mut s) = self.settings.lock() {
                    s.split_joycons = on;
                    s.save(&self.data_dir);
                }
                self.engine.send(Command::SplitJoyCons(on));
            }
            ToTray::Focused(on) => return self.engine.send(Command::Focused(on)),
            ToTray::Identify(key) => return self.engine.send(Command::Identify(key)),
            ToTray::PowerOff(key) => return self.engine.send(Command::PowerOff(key)),
            ToTray::SwapPlayers(a, b) => return self.engine.send(Command::SwapPlayers(a, b)),
            ToTray::Edit { store, edit } => {
                let profiles = self.edit_profiles(&store, edit);
                return self.engine.send(Command::SetProfiles(store, profiles));
            }
            ToTray::Quit => return self.post(Msg::Quit),
            // Answered by the window server, which has the pipe to answer on.
            ToTray::Diagnose(_) => return,
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

/// Starts the window, which sits next to this executable, and waits for it on a thread of its
/// own, so a window that fails right away (a library missing on Linux) is reported, not left
/// as a zombie.
pub fn open_window() {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    let exe = dir.join(format!("open-controller-ui{}", std::env::consts::EXE_SUFFIX));
    let mut child = match std::process::Command::new(exe).spawn() {
        Ok(c) => c,
        Err(e) => return eprintln!("open-controller: could not start the window: {e}"),
    };
    let started = Instant::now();
    let _ = std::thread::Builder::new().name("window".into()).spawn(move || {
        let Ok(status) = child.wait() else { return };
        if !status.success() && started.elapsed() < Duration::from_secs(10) {
            eprintln!("open-controller: the window closed right after starting ({status})");
            sys::window_failed(CONTROL.get().map(|c| c.lang()).unwrap_or_default());
        }
    });
}

/// Removes what this instance keeps in the user's runtime directory: the window's socket and
/// the signals' sockets and lock.
fn leave() {
    open_controller_core::ipc::remove_socket();
    instance::remove_files(APP_NAME, &["show", "quit"]);
}

/// Reachable from the window procedure and the foreground watcher without borrowing anything.
pub static CONTROL: OnceLock<Arc<Control>> = OnceLock::new();

fn config(data_dir: &Path, settings: &Settings) -> Config {
    Config {
        data_dir: data_dir.to_path_buf(),
        hide: settings.hide_originals,
        profiles: settings.controllers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        driver: settings.virtual_driver,
        split_joycons: settings.split_joycons,
    }
}

/// Logs a panic to `crash.log`. Release builds abort right after, so on Windows a `--restore` of
/// its own then shows again the controllers this process hid, instead of leaving them hidden
/// until the next start.
fn on_panic(data_dir: &Path) {
    let dir = data_dir.to_path_buf();
    open_controller_core::crash::install(data_dir, move || {
        if cfg!(all(windows, panic = "abort"))
            && let Ok(exe) = std::env::current_exe()
        {
            let _ = std::process::Command::new(exe).arg("--restore").arg("--data-dir").arg(&dir).spawn();
        }
    });
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
        // For the uninstaller: ask the running instance to quit the clean way and wait for it. A
        // copy still starting up holds the lock before it listens for "quit", so keep asking.
        let start = Instant::now();
        let done = loop {
            if !instance::running(APP_NAME) {
                break true;
            }
            if instance::signal(APP_NAME, "quit") {
                break instance::wait_gone(APP_NAME, Duration::from_secs(10));
            }
            if start.elapsed() > Duration::from_secs(10) {
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
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
    // Only the copy that runs the engine: a `--restore` that panicked would start another.
    on_panic(&data_dir);
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
    if args.smoke {
        let engine = Engine::start(config(&data_dir, &settings), on_change);
        let code = smoke(&engine, &data_dir);
        engine.stop();
        leave();
        std::process::exit(code);
    }
    let engine = Input::new(on_change);
    let control = Arc::new(Control { engine, settings: Mutex::new(settings), data_dir, notify: notify.clone(), waker });
    let _ = CONTROL.set(control.clone());
    #[cfg(windows)]
    windows::follow_console(control.clone());
    #[cfg(not(windows))]
    control.resume();
    server::start(control.clone(), notify);
    let c = control.clone();
    instance::on_signal(show, move || c.post(Msg::Open));
    if let Some(quit) = instance::event(APP_NAME, "quit") {
        let c = control.clone();
        instance::on_signal(quit, move || c.post(Msg::Quit));
    }

    sys::run(&control, main_loop, &args);
    // The window closes with this process rather than start it again.
    control.notify.bye();
    // Unplugs the virtual controllers and shows the hidden ones again.
    control.engine.stop();
    leave();
}
