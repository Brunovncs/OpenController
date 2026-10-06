//! Serves the window over the pipe: the latest snapshot whenever the engine has a new one, the
//! preferences when they change, and the window's requests in the other direction. One window
//! at a time; while it is connected the engine keeps live input in its snapshots.

use crate::Control;
use crossbeam_channel::{Receiver, Sender, bounded};
use open_controller_core::Command;
use open_controller_core::ipc::{Pipe, ToTray, ToWindow};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

const SNAPSHOT: u8 = 1;
const PREFS: u8 = 2;

/// What changed since the window was last told, and a wake-up for the sender.
pub struct Notify {
    pending: AtomicU8,
    tx: Sender<()>,
    rx: Receiver<()>,
}

impl Default for Notify {
    fn default() -> Self {
        let (tx, rx) = bounded(1);
        Notify { pending: AtomicU8::new(0), tx, rx }
    }
}

impl Notify {
    fn mark(&self, what: u8) {
        self.pending.fetch_or(what, Ordering::AcqRel);
        let _ = self.tx.try_send(());
    }

    pub fn snapshot(&self) {
        self.mark(SNAPSHOT);
    }

    pub fn prefs(&self) {
        self.mark(PREFS);
    }
}

pub fn start(control: Arc<Control>, notify: Arc<Notify>) {
    std::thread::Builder::new()
        .name("window-server".into())
        .spawn(move || {
            // A previous instance's pipe can take a moment to close.
            let pipe = loop {
                match Pipe::create() {
                    Ok(p) => break p,
                    Err(e) => {
                        eprintln!("open-controller: no pipe for the window yet: {e}");
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            };
            loop {
                if pipe.accept().is_err() {
                    // A client that came and went before the accept leaves the pipe closing;
                    // it takes new ones again only once disconnected.
                    pipe.disconnect();
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
                control.engine.send(Command::Watch(true));
                serve(&pipe, &control, &notify);
                pipe.disconnect();
                control.engine.send(Command::Watch(false));
            }
        })
        .expect("could not start the window server");
}

fn serve(pipe: &Pipe, control: &Control, notify: &Notify) {
    let gone = AtomicBool::new(false);
    notify.mark(SNAPSHOT | PREFS);
    std::thread::scope(|s| {
        s.spawn(|| {
            let mut reader = pipe.reader();
            while let Ok(req) = reader.recv::<ToTray>() {
                control.apply(req);
            }
            gone.store(true, Ordering::Release);
            notify.mark(0);
        });
        while !gone.load(Ordering::Acquire) {
            let _ = notify.rx.recv_timeout(Duration::from_secs(1));
            let what = notify.pending.swap(0, Ordering::AcqRel);
            let mut sent = Ok(());
            if what & PREFS != 0 {
                sent = pipe.send(&ToWindow::Prefs(control.prefs()));
            }
            if sent.is_ok() && what & SNAPSHOT != 0 {
                sent = pipe.send(&ToWindow::Snapshot(control.engine.snapshot()));
            }
            if sent.is_err() {
                // Ends the reader's pending read too.
                pipe.disconnect();
                break;
            }
        }
    });
}
