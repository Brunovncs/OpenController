//! Linux and macOS: no icon of its own; the main thread waits for the other threads' messages
//! and for the system asking it to stop (SIGTERM at shutdown, SIGINT, SIGHUP at sign-out).

use crate::{Args, Control, Msg, foreground, open_window};
use crossbeam_channel::{Receiver, Sender, unbounded};
use open_controller_core::i18n::{self, Lang};
use std::process::Stdio;
use std::sync::Arc;

#[derive(Clone)]
pub struct Waker(Sender<Msg>);

impl Waker {
    /// Only opening the window and quitting need the main thread here; with no icon to update,
    /// the snapshot and preference messages, dozens a second while the window is open, are dropped.
    pub fn post(&self, m: Msg) {
        if matches!(m, Msg::Open | Msg::Quit) {
            let _ = self.0.send(m);
        }
    }
}

pub struct MainLoop(Receiver<Msg>);

/// Nothing is left hidden by a run that was killed: grabs end with the process.
pub fn restore(_: &std::path::Path) -> i32 {
    0
}

/// Tells the user the window did not open, with a desktop notification where `notify-send`
/// exists, since a program started from the applications menu has nowhere to print.
pub fn window_failed(lang: Lang) {
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=OpenController", "--icon=io.github.brunovncs.open-controller", "OpenController", i18n::text(lang).window_failed])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Blocks the stop signals in every thread to come, and turns them into a quit message on a
/// thread that waits for them. Called before any other thread starts.
pub fn prepare() -> (Waker, MainLoop) {
    let (tx, rx) = unbounded();
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::sigaddset(&mut set, s);
        }
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        let quit = tx.clone();
        std::thread::Builder::new()
            .name("open-controller-signals".into())
            .spawn(move || {
                let mut sig = 0;
                libc::sigwait(&set, &mut sig);
                let _ = quit.send(Msg::Quit);
            })
            .expect("could not start the signal thread");
    }
    (Waker(tx), MainLoop(rx))
}

pub fn run(_: &Arc<Control>, main_loop: MainLoop, args: &Args) {
    foreground::watch();
    if !args.minimized {
        open_window();
    }
    while let Ok(m) = main_loop.0.recv() {
        match m {
            Msg::Open => open_window(),
            Msg::Quit => return,
            Msg::Snapshot | Msg::Prefs => {}
        }
    }
}
