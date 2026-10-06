#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window. It runs as its own process while it is open and talks to the resident process
//! (`open-controller.exe`) over a pipe; closing it frees the GPU-backed interface entirely
//! while the controllers keep working.

mod art;
mod demo;
mod detail;
mod home;
mod icons;
mod keys;
mod programs;
mod requirements;
mod settings;
mod theme;
mod tuning;
mod ui;
mod updates;
mod widgets;

use gpui::{
    App, AppContext, Bounds, Context, Entity, TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowOptions, px,
    size,
};
use open_controller_core::Snapshot;
use open_controller_core::i18n::{self, Text};
use open_controller_core::instance::{self, Instance};
use open_controller_core::ipc::{Pipe, Prefs, ToTray, ToWindow};
use open_controller_core::profile::Profiles;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use ui::MainView;

const APP_NAME: &str = "io.github.brunovncs.open-controller.ui";
/// The resident process's instance name, to wait for it to quit before an update.
pub const RESIDENT: &str = "io.github.brunovncs.open-controller";

/// What the window shows, kept up to date from the pipe.
pub struct Model {
    pub snapshot: Snapshot,
    pub prefs: Prefs,
    pub text: &'static Text,
    /// False when the resident process could not be reached.
    pub connected: bool,
    pipe: Option<Arc<Pipe>>,
    demo: bool,
    /// `--demo`: changes made in the window stay in it, so the example controllers never reach
    /// the settings file.
    demo_profiles: HashMap<String, Profiles>,
    demo_swaps: Vec<(open_controller_core::PadKey, open_controller_core::PadKey)>,
    last_real: Snapshot,
}

impl Model {
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
        if let Some(p) = &self.pipe {
            let _ = p.send(&req);
        }
    }

    fn refresh_demo(&mut self, cx: &mut Context<Self>) {
        self.snapshot = demo::snapshot(&self.last_real, &self.demo_profiles, &self.demo_swaps);
        cx.notify();
    }
}

enum Event {
    FromTray(ToWindow),
    TrayGone,
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

fn main() {
    let mut demo = false;
    let mut smoke = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--demo" => demo = true,
            "--smoke" => smoke = true,
            _ => {}
        }
    }
    let Instance::First { _mutex, show } = instance::claim(APP_NAME) else { return };
    let (tx, rx) = async_channel::unbounded::<Event>();
    let (first_prefs, first_prefs_rx) = std::sync::mpsc::channel::<Prefs>();

    let pipe = connect(!smoke).map(Arc::new);
    if let Some(p) = pipe.clone() {
        let tx = tx.clone();
        std::thread::Builder::new()
            .name("pipe-reader".into())
            .spawn(move || {
                let mut reader = p.reader();
                let mut first_prefs = Some(first_prefs);
                while let Ok(msg) = reader.recv::<ToWindow>() {
                    if let ToWindow::Prefs(p) = &msg
                        && let Some(first) = first_prefs.take()
                    {
                        let _ = first.send(*p);
                    }
                    if tx.send_blocking(Event::FromTray(msg)).is_err() {
                        return;
                    }
                }
                let _ = tx.send_blocking(Event::TrayGone);
            })
            .expect("could not start the pipe reader");
    }
    let show_tx = tx.clone();
    instance::on_signal(show, move || {
        let _ = show_tx.send_blocking(Event::Show);
    });

    // So the window opens in the language picked, not in English first.
    let prefs = first_prefs_rx.recv_timeout(Duration::from_millis(500)).unwrap_or_default();

    gpui_platform::application().run(move |cx: &mut App| {
        let text = i18n::text(prefs.lang);
        let connected = pipe.is_some();
        let mut snapshot = Snapshot::default();
        let last_real = snapshot.clone();
        if demo {
            snapshot = demo::snapshot(&snapshot, &HashMap::new(), &[]);
        }
        let model = cx.new(|_| Model {
            snapshot,
            prefs,
            text,
            connected,
            pipe,
            demo,
            demo_profiles: HashMap::new(),
            demo_swaps: Vec::new(),
            last_real,
        });
        let Some(window) = open_window(model.clone(), cx) else {
            eprintln!("open-controller-ui: could not open a window");
            std::process::exit(1);
        };

        cx.spawn(async move |cx| {
            while let Ok(ev) = rx.recv().await {
                match ev {
                    Event::FromTray(ToWindow::Snapshot(s)) => model.update(cx, |m, cx| {
                        if m.demo {
                            m.last_real = s;
                            m.refresh_demo(cx);
                        } else {
                            m.snapshot = s;
                            cx.notify();
                        }
                    }),
                    Event::FromTray(ToWindow::Prefs(p)) => model.update(cx, |m, cx| {
                        m.prefs = p;
                        m.text = i18n::text(p.lang);
                        cx.notify();
                    }),
                    // OpenController quit from the tray: the window goes with it, unless it is the
                    // update that asked, which still has the installer to start.
                    Event::TrayGone if updates::installing() => {}
                    Event::TrayGone => cx.update(|cx| cx.quit()),
                    Event::Show => {
                        let _ = window.update(cx, |_, w, _| w.activate_window());
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
                std::process::exit(0);
            })
            .detach();
        }
        cx.activate(true);
    });
}

fn open_window(model: Entity<Model>, cx: &mut App) -> Option<WindowHandle<MainView>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1160.), px(800.)), cx))),
        titlebar: Some(TitlebarOptions { title: Some("OpenController".into()), ..Default::default() }),
        app_id: Some("open-controller".into()),
        window_min_size: Some(size(px(720.), px(520.))),
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    };
    cx.open_window(options, |window, cx| cx.new(|cx| MainView::new(model, window, cx))).ok()
}
