#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window. It runs as its own process while it is open and talks to the resident process
//! (`open-controller.exe`) over a pipe; closing it frees the GPU-backed interface entirely
//! while the controllers keep working.

mod art;
mod demo;
mod detail;
mod frame;
mod home;
mod icons;
mod keys;
mod learn;
mod programs;
mod report;
mod requirements;
mod settings;
mod sticks;
mod text_field;
mod theme;
mod tuning;
mod ui;
mod updates;
mod widgets;

use gpui::{
    App, AppContext, Bounds, Context, Entity, QuitMode, TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use open_controller_core::Snapshot;
use open_controller_core::i18n::{self, Lang, Text};
use open_controller_core::instance::{self, Instance};
use open_controller_core::ipc::{PROTO, Pipe, Prefs, ToTray, ToWindow};
use open_controller_core::profile::Profiles;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use ui::MainView;

const APP_NAME: &str = "io.github.brunovncs.open-controller.ui";
/// The resident process's instance name, to wait for it to quit before an update.
pub const RESIDENT: &str = "io.github.brunovncs.open-controller";

/// Set while the window has asked a resident process of another version to quit so the one next
/// to the window can take its place: its goodbye then does not close the window.
static RESTARTING: AtomicBool = AtomicBool::new(false);

/// How the window stands with the resident process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    /// Connecting to it as the window opens, when it is running: too quick to be worth saying.
    Connecting,
    /// Starting it, as the window opens.
    Starting,
    Up,
    /// It went away without saying goodbye: connecting again, starting it if need be.
    Lost,
    /// Not running, and it could not be started. Still looked for, should it be started.
    Down,
}

/// What the window shows, kept up to date from the pipe.
pub struct Model {
    /// Shared, so drawing a frame never copies it.
    pub snapshot: Arc<Snapshot>,
    pub prefs: Prefs,
    pub text: &'static Text,
    pub connection: Connection,
    /// The resident process's version, when it speaks another version of the pipe's messages.
    pub other_version: Option<String>,
    /// The resident process's answer about a controller, for the report on it.
    pub diagnosis: Option<(open_controller_core::PadKey, Option<Box<open_controller_core::report::Diagnosis>>)>,
    pipe: Option<Arc<Pipe>>,
    demo: bool,
    /// `--demo`: changes made in the window stay in it, so the example controllers never reach
    /// the settings file.
    demo_profiles: HashMap<String, Profiles>,
    demo_swaps: Vec<(open_controller_core::PadKey, open_controller_core::PadKey)>,
    /// The GUIDs of the example joysticks mapped in the window.
    demo_mapped: Vec<String>,
    last_real: Snapshot,
}

impl Model {
    pub fn connected(&self) -> bool {
        self.connection == Connection::Up
    }

    /// Quits a resident process of another version and starts the one next to this window.
    pub fn restart_resident(&mut self, cx: &mut Context<Self>) {
        RESTARTING.store(true, Ordering::SeqCst);
        self.other_version = None;
        if let Some(p) = &self.pipe {
            let _ = p.send(&ToTray::Quit);
        }
        cx.notify();
    }

    /// The connection dropped: what came over it is out of date.
    fn lost(&mut self, cx: &mut Context<Self>) {
        self.pipe = None;
        self.connection = Connection::Lost;
        if self.demo {
            self.last_real = Snapshot::default();
            self.refresh_demo(cx);
        } else {
            self.snapshot = Default::default();
        }
        cx.notify();
    }

    pub fn send(&mut self, req: ToTray, cx: &mut Context<Self>) {
        if self.demo
            && let ToTray::SwapPlayers(a, b) = req
        {
            self.demo_swaps.push((a, b));
            return self.refresh_demo(cx);
        }
        if self.demo
            && let ToTray::Edit { store, edit } = req
        {
            let current = self.snapshot.pads.iter().find(|p| p.store.as_ref() == Some(&store)).map(|p| p.profiles.clone());
            self.demo_profiles.entry(store).or_insert_with(|| current.unwrap_or_default()).apply(edit);
            return self.refresh_demo(cx);
        }
        if self.demo
            && let ToTray::Map { guid, .. } | ToTray::Unmap(guid) = &req
        {
            match req {
                ToTray::Map { .. } => self.demo_mapped.push(guid.clone()),
                _ => self.demo_mapped.retain(|g| g != guid),
            }
            return self.refresh_demo(cx);
        }
        if self.demo && matches!(req, ToTray::SetVirtualDriver(_) | ToTray::SplitJoyCons(_)) {
            // Shown in the window only: the resident process's controllers stay as they are.
            return;
        }
        if self.demo
            && let ToTray::Diagnose(key) = req
        {
            // The demo's controllers exist only here: what SDL would say is made up from them.
            self.diagnosis = Some((key, self.snapshot.pads.iter().find(|p| p.key == key).map(|p| Box::new(demo::diagnosis(p)))));
            return cx.notify();
        }
        if let Some(p) = &self.pipe {
            let _ = p.send(&req);
        }
    }

    fn refresh_demo(&mut self, cx: &mut Context<Self>) {
        self.snapshot = Arc::new(demo::snapshot(&self.last_real, &self.demo_profiles, &self.demo_swaps, &self.demo_mapped));
        cx.notify();
    }
}

enum Event {
    Linked(Arc<Pipe>),
    FromTray(ToWindow),
    Lost,
    Down,
    /// The resident process quit on purpose: the window goes with it.
    Bye,
    Show,
}

/// Connects to the resident process, starting it first if it is not running.
fn connect(may_start: bool) -> Option<Pipe> {
    if let Ok(Some(p)) = Pipe::connect() {
        return Some(p);
    }
    if !may_start {
        return None;
    }
    let exe = std::env::current_exe().ok()?.parent()?.join(format!("open-controller{}", std::env::consts::EXE_SUFFIX));
    std::process::Command::new(exe).arg("--minimized").spawn().ok()?;
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(Some(p)) = Pipe::connect() {
            return Some(p);
        }
    }
    None
}

/// Keeps the window connected to the resident process for as long as it is open: connects,
/// starting it when it is not running, passes on what it sends, and when the connection drops
/// connects again, unless it said goodbye.
fn keep_linked(tx: async_channel::Sender<Event>, first_prefs: std::sync::mpsc::Sender<Prefs>, may_start: bool) {
    let mut first_prefs = Some(first_prefs);
    let mut wait = Duration::from_millis(250);
    let mut failed = 0;
    loop {
        // An update has it quit for the installer: starting it now would bring the old version
        // back.
        while updates::installing() {
            std::thread::sleep(Duration::from_millis(200));
        }
        if RESTARTING.load(Ordering::SeqCst) {
            instance::wait_gone(RESIDENT, Duration::from_secs(10));
        }
        // One that fails to start a few times running is left alone, though still picked up
        // should the person start it.
        let Some(pipe) = connect(may_start && failed < 3).map(Arc::new) else {
            failed += 1;
            first_prefs = None;
            if tx.send_blocking(Event::Down).is_err() {
                return;
            }
            std::thread::sleep(wait);
            wait = (wait * 2).min(Duration::from_secs(5));
            continue;
        };
        failed = 0;
        RESTARTING.store(false, Ordering::SeqCst);
        if tx.send_blocking(Event::Linked(pipe.clone())).is_err() {
            return;
        }
        let since = std::time::Instant::now();
        let mut reader = pipe.reader();
        let mut hello = false;
        let bye = loop {
            match reader.recv::<ToWindow>() {
                Ok(ToWindow::Bye) => break true,
                Ok(msg) => {
                    hello |= matches!(msg, ToWindow::Hello { .. });
                    if let ToWindow::Prefs(p) = &msg
                        && let Some(first) = first_prefs.take()
                    {
                        let _ = first.send(*p);
                    }
                    if tx.send_blocking(Event::FromTray(msg)).is_err() {
                        return;
                    }
                }
                Err(_) => break false,
            }
        };
        // One from before the goodbye quits without saying it: its going is taken as on purpose,
        // as it always was.
        if (bye || !hello) && !updates::installing() && !RESTARTING.load(Ordering::SeqCst) {
            let _ = tx.send_blocking(Event::Bye);
            return;
        }
        if tx.send_blocking(Event::Lost).is_err() {
            return;
        }
        // A connection that keeps dropping right away is tried again less and less often.
        wait = if since.elapsed() > Duration::from_secs(5) { Duration::from_millis(250) } else { (wait * 2).min(Duration::from_secs(5)) };
        std::thread::sleep(wait);
    }
}

fn main() {
    open_controller_core::crash::install(&open_controller_core::crash::data_dir(), || {});
    let mut demo = false;
    let mut smoke = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--demo" => demo = true,
            "--smoke" => smoke = true,
            _ => {}
        }
    }
    #[cfg(target_os = "linux")]
    if !has_display() {
        let running = connect(!smoke).is_some();
        eprintln!("open-controller-ui: no graphical session to open the window in (DISPLAY and WAYLAND_DISPLAY are not set).");
        if running {
            eprintln!("OpenController keeps running in the background, so your controllers keep working.");
        }
        std::process::exit(1);
    }
    let Instance::First { _mutex, show } = instance::claim(APP_NAME) else { return };
    let (tx, rx) = async_channel::unbounded::<Event>();
    let (first_prefs, first_prefs_rx) = std::sync::mpsc::channel::<Prefs>();

    // Off the interface thread: starting the resident process can take seconds, and the window
    // shows that meanwhile.
    let link_tx = tx.clone();
    std::thread::Builder::new()
        .name("pipe-reader".into())
        .spawn(move || keep_linked(link_tx, first_prefs, !smoke))
        .expect("could not start the pipe reader");
    let show_tx = tx.clone();
    instance::on_signal(show, move || {
        let _ = show_tx.send_blocking(Event::Show);
    });

    // So the window opens in the language picked, not in English first. One not running yet is
    // not waited for: the language follows once it is up.
    let running = instance::running(RESIDENT);
    let mut prefs = if running { first_prefs_rx.recv_timeout(Duration::from_millis(500)).unwrap_or_default() } else { Prefs::default() };
    let forced_lang = forced_lang();
    prefs.lang = forced_lang.unwrap_or(prefs.lang);

    // The resident process does the work, so closing the window ends this one on macOS too, where
    // GPUI would otherwise keep it running with no window.
    gpui_platform::application().with_quit_mode(QuitMode::LastWindowClosed).run(move |cx: &mut App| {
        text_field::bind_keys(cx);
        let text = i18n::text(prefs.lang);
        let last_real = Snapshot::default();
        let snapshot = Arc::new(if demo { demo::snapshot(&last_real, &HashMap::new(), &[], &[]) } else { Snapshot::default() });
        let model = cx.new(|_| Model {
            snapshot,
            prefs,
            text,
            connection: if running { Connection::Connecting } else { Connection::Starting },
            other_version: None,
            diagnosis: None,
            pipe: None,
            demo,
            demo_profiles: HashMap::new(),
            demo_swaps: Vec::new(),
            demo_mapped: Vec::new(),
            last_real,
        });
        // The example joystick presses its parts on its own.
        if demo && std::env::var_os("OPEN_CONTROLLER_DEMO_STICKS").is_some() {
            let model = model.clone();
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                    model.update(cx, |m, cx| m.refresh_demo(cx));
                }
            })
            .detach();
        }
        let Some(window) = open_window(model.clone(), cx) else {
            eprintln!("open-controller-ui: could not open a window");
            leave();
            std::process::exit(1);
        };
        cx.on_app_quit(|_| {
            leave();
            async {}
        })
        .detach();

        cx.spawn(async move |cx| {
            while let Ok(first) = rx.recv().await {
                let mut batch = vec![first];
                while let Ok(ev) = rx.try_recv() {
                    batch.push(ev);
                }
                // Snapshots can come faster than frames are drawn: only the newest of those
                // waiting is worth taking in.
                let newest = batch.iter().rposition(|e| matches!(e, Event::FromTray(ToWindow::Snapshot(_))));
                for (i, ev) in batch.into_iter().enumerate() {
                    match ev {
                        Event::FromTray(ToWindow::Snapshot(_)) if Some(i) != newest => {}
                        Event::FromTray(ToWindow::Snapshot(s)) => model.update(cx, |m, cx| {
                            if m.demo {
                                m.last_real = s;
                                m.refresh_demo(cx);
                            } else {
                                m.snapshot = Arc::new(s);
                                cx.notify();
                            }
                        }),
                        Event::FromTray(ToWindow::Diagnosis(key, d)) => model.update(cx, |m, cx| {
                            m.diagnosis = Some((key, d));
                            cx.notify();
                        }),
                        Event::FromTray(ToWindow::Prefs(mut p)) => model.update(cx, |m, cx| {
                            p.lang = forced_lang.unwrap_or(p.lang);
                            m.prefs = p;
                            m.text = i18n::text(p.lang);
                            cx.notify();
                        }),
                        Event::FromTray(ToWindow::Hello { proto, version }) => model.update(cx, |m, cx| {
                            if proto != PROTO {
                                eprintln!(
                                    "open-controller-ui: OpenController {version} speaks version {proto} of the pipe, this window {PROTO}"
                                );
                            }
                            m.other_version = (proto != PROTO).then_some(version);
                            cx.notify();
                        }),
                        // Taken by the reader, which turns it into `Event::Bye`.
                        Event::FromTray(ToWindow::Bye) => {}
                        Event::Linked(pipe) => {
                            model.update(cx, |m, cx| {
                                m.pipe = Some(pipe);
                                m.connection = Connection::Up;
                                m.other_version = None;
                                cx.notify();
                            });
                            let _ = window.update(cx, |view, w, cx| view.linked(w, cx));
                        }
                        // The update had it quit for the installer, and quits the window itself.
                        Event::Lost if updates::installing() => model.update(cx, |m, _| m.pipe = None),
                        Event::Lost => model.update(cx, |m, cx| m.lost(cx)),
                        Event::Down => model.update(cx, |m, cx| {
                            m.pipe = None;
                            m.connection = Connection::Down;
                            cx.notify();
                        }),
                        // OpenController quit from the tray or the settings: the window goes with
                        // it, unless it is the update that asked, which still has the installer
                        // to start.
                        Event::Bye if updates::installing() => {}
                        Event::Bye => cx.update(|cx| cx.quit()),
                        Event::Show => {
                            let _ = window.update(cx, |_, w, _| w.activate_window());
                        }
                    }
                }
            }
        })
        .detach();

        if smoke {
            // CI: the window opened and drew; that is all a smoke run checks.
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                println!("smoke: window drawn");
                leave();
                std::process::exit(0);
            })
            .detach();
        }
        cx.activate(true);
    });
}

/// Removes this window's lock and signal socket from the user's runtime directory, on its way out.
fn leave() {
    instance::remove_files(APP_NAME, &["show"]);
}

/// Whether there is a desktop to open the window on: the same test GPUI makes on Linux.
#[cfg(target_os = "linux")]
fn has_display() -> bool {
    ["WAYLAND_DISPLAY", "DISPLAY"].iter().any(|v| std::env::var_os(v).is_some_and(|d| !d.is_empty()))
}

/// `OPEN_CONTROLLER_LANG=en` or `pt` shows the window in that language whatever was picked, for
/// screenshots.
fn forced_lang() -> Option<Lang> {
    match std::env::var("OPEN_CONTROLLER_LANG").ok()?.as_str() {
        "en" => Some(Lang::En),
        "pt" => Some(Lang::Pt),
        _ => None,
    }
}

fn open_window(model: Entity<Model>, cx: &mut App) -> Option<WindowHandle<MainView>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1160.), px(800.)), cx))),
        titlebar: Some(TitlebarOptions { title: Some("OpenController".into()), ..Default::default() }),
        // The Wayland app id and the X11 class: the .desktop file's name, so the dock shows its icon.
        app_id: Some("io.github.brunovncs.open-controller".into()),
        window_min_size: Some(size(px(720.), px(520.))),
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    };
    cx.open_window(options, |window, cx| cx.new(|cx| MainView::new(model, window, cx))).ok()
}
