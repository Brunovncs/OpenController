//! Linux and macOS: no icon of its own; the main thread waits for the other threads' messages
//! and for the system asking it to stop (SIGTERM at shutdown, SIGINT, SIGHUP at sign-out). On
//! macOS it also turns its run loop while it waits, which the workspace needs to see the
//! application in front change.

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
            #[cfg(target_os = "macos")]
            open_controller_core::macos::run_loop::stop();
        }
    }
}

pub struct MainLoop(Receiver<Msg>);

impl MainLoop {
    #[cfg(not(target_os = "macos"))]
    fn next(&self) -> Option<Msg> {
        self.0.recv().ok()
    }

    /// The run loop turns in short spells, and messages are looked for between them, since a
    /// stop that comes between two spells is lost.
    #[cfg(target_os = "macos")]
    fn next(&self) -> Option<Msg> {
        use crossbeam_channel::TryRecvError;
        loop {
            match self.0.try_recv() {
                Ok(m) => return Some(m),
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => open_controller_core::macos::run_loop::run_for(std::time::Duration::from_millis(500)),
            }
        }
    }
}

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
    let waker = Waker(tx);
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::sigaddset(&mut set, s);
        }
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        let quit = waker.clone();
        std::thread::Builder::new()
            .name("open-controller-signals".into())
            .spawn(move || {
                let mut sig = 0;
                libc::sigwait(&set, &mut sig);
                quit.post(Msg::Quit);
            })
            .expect("could not start the signal thread");
    }
    (waker, MainLoop(rx))
}

pub fn run(_: &Arc<Control>, main_loop: MainLoop, args: &Args) {
    foreground::watch();
    if !args.minimized {
        open_window();
    }
    while let Some(m) = main_loop.next() {
        match m {
            Msg::Open => open_window(),
            Msg::Quit => return,
            Msg::Snapshot | Msg::Prefs => {}
        }
    }
}
