//! Hiding the original controllers, on a thread of its own. HidHide's control device admits one
//! handle at a time and every change is journaled to disk first, so a change takes from a few
//! milliseconds to seconds while another program holds the device. The input thread only queues
//! requests and reads back what is hidden; the players keep playing meanwhile.

use crate::engine::Driver;
use crate::platform::{Cloak, CloakError, VIRTUAL_PADS};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often to try again when HidHide could not be opened (another program held it, as at
/// sign-in).
const RETRY: Duration = Duration::from_secs(5);

enum Request {
    Hide(Vec<String>),
    Reveal(Vec<String>),
    RevealAll,
    /// Shows everything again and ends the thread.
    Stop,
}

enum Report {
    State(Driver),
    /// The device instance ids hidden right now.
    Hidden(HashSet<String>),
}

pub struct Hider {
    tx: Sender<Request>,
    rx: Receiver<Report>,
    thread: Option<JoinHandle<()>>,
    state: Driver,
    hidden: HashSet<String>,
}

fn state_of(r: &Result<Cloak, CloakError>) -> Driver {
    match r {
        Ok(_) => Driver::Ready { version: None },
        Err(_) if !VIRTUAL_PADS => Driver::Unsupported,
        Err(CloakError::NotInstalled) => Driver::Missing,
        #[allow(unreachable_patterns)]
        Err(e) => Driver::Failed(e.to_string()),
    }
}

impl Hider {
    /// Starts the thread and waits for it to open HidHide, as the engine did before it read any
    /// controller: undoing what a killed run left hidden comes first.
    pub fn start(journal: PathBuf) -> Hider {
        let (tx, requests) = unbounded();
        let (reports, rx) = unbounded();
        let thread = std::thread::Builder::new()
            .name("open-controller-hiding".into())
            .spawn(move || run(&journal, &requests, &reports))
            .expect("could not start the hiding thread");
        let state = match rx.recv() {
            Ok(Report::State(s)) => s,
            _ => Driver::Failed("the hiding thread stopped".into()),
        };
        Hider { tx, rx, thread: Some(thread), state, hidden: HashSet::new() }
    }

    pub fn hide(&self, ids: Vec<String>) {
        if !ids.is_empty() {
            let _ = self.tx.send(Request::Hide(ids));
        }
    }

    pub fn reveal(&self, ids: Vec<String>) {
        if !ids.is_empty() {
            let _ = self.tx.send(Request::Reveal(ids));
        }
    }

    pub fn reveal_all(&self) {
        let _ = self.tx.send(Request::RevealAll);
    }

    /// Takes in what the thread reported since the last call. Returns whether anything changed,
    /// and whether HidHide just became usable, when what should be hidden has to be asked again.
    pub fn poll(&mut self) -> (bool, bool) {
        let (mut changed, mut opened) = (false, false);
        while let Ok(r) = self.rx.try_recv() {
            changed = true;
            match r {
                Report::State(s) => {
                    opened |= !matches!(self.state, Driver::Ready { .. }) && matches!(s, Driver::Ready { .. });
                    self.state = s;
                }
                Report::Hidden(h) => self.hidden = h,
            }
        }
        (changed, opened)
    }

    pub fn state(&self) -> &Driver {
        &self.state
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.contains(id)
    }

    /// Shows every hidden controller again and waits for that to be done.
    pub fn stop(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Hider {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(journal: &Path, requests: &Receiver<Request>, reports: &Sender<Report>) {
    let first = Cloak::start(journal);
    let mut state = state_of(&first);
    let _ = reports.send(Report::State(state.clone()));
    let mut cloak = first.ok();
    // What the engine asked to hide, and what of it is hidden.
    let mut asked: HashSet<String> = HashSet::new();
    let mut hidden = HashSet::new();
    let mut retry_at = Instant::now() + RETRY;
    // When to look again at controllers still being hidden (their nodes arrive one by one).
    let mut look_at: Option<Instant> = None;
    loop {
        // Only a failure is worth retrying; a missing driver stays missing until a restart.
        let retrying = cloak.is_none() && matches!(state, Driver::Failed(_));
        if retrying && Instant::now() >= retry_at {
            retry_at = Instant::now() + RETRY;
            let r = Cloak::start(journal);
            if r.is_ok() {
                state = state_of(&r);
                cloak = r.ok();
                let _ = reports.send(Report::State(state.clone()));
            }
        }
        let wake = [retrying.then_some(retry_at), look_at].into_iter().flatten().min();
        let request = match wake {
            Some(at) => requests.recv_deadline(at),
            None => requests.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let request = match request {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => {
                if let Some(c) = cloak.as_mut()
                    && look_at.is_some_and(|at| Instant::now() >= at)
                {
                    look_at = c.follow_up().map(|d| Instant::now() + d);
                    let now: HashSet<String> = asked.iter().filter(|id| c.is_hidden(id)).cloned().collect();
                    if now != hidden {
                        hidden = now;
                        let _ = reports.send(Report::Hidden(hidden.clone()));
                    }
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => Request::Stop,
        };
        // Without HidHide there is nothing to change; the engine asks again once it opens.
        let Some(c) = cloak.as_mut() else {
            if matches!(request, Request::Stop) {
                return;
            }
            continue;
        };
        let result = match &request {
            Request::Hide(ids) => {
                let r = c.hide(ids);
                asked.extend(ids.iter().cloned());
                hidden.extend(ids.iter().filter(|id| c.is_hidden(id)).cloned());
                look_at = c.follow_up().map(|d| Instant::now() + d);
                r
            }
            Request::Reveal(ids) => {
                let r = c.reveal(ids);
                asked.retain(|id| !ids.contains(id));
                hidden.retain(|id| !ids.contains(id));
                r
            }
            Request::RevealAll | Request::Stop => {
                asked.clear();
                hidden.clear();
                c.reveal_all()
            }
        };
        if matches!(request, Request::Stop) {
            if let Err(e) = result {
                // The journal stays; the next start shows them again.
                eprintln!("open-controller: could not show the hidden controllers again: {e}");
            }
            return;
        }
        // A failure shows in the settings, and goes away once a later change works.
        let now = match result {
            Ok(()) => Driver::Ready { version: None },
            Err(e) => Driver::Failed(e.to_string()),
        };
        if now != state {
            state = now;
            let _ = reports.send(Report::State(state.clone()));
        }
        let _ = reports.send(Report::Hidden(hidden.clone()));
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::hidhide::HidHide;

    /// Hides a made-up device through the thread and checks that stopping shows it again.
    /// Needs HidHide: `cargo test -p open-controller-core -- --ignored hiding`.
    #[test]
    #[ignore]
    fn hiding_thread_round_trip_on_the_driver() {
        let dir = std::env::temp_dir().join(format!("oc-hiding-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let probe = r"HID\VID_0000&PID_0000\OPEN-CONTROLLER-HIDING-TEST".to_string();
        let white0 = HidHide::open().unwrap().whitelist().unwrap();
        let listed = || HidHide::open().unwrap().blacklist().unwrap().iter().any(|x| x.eq_ignore_ascii_case(&probe));

        let mut hider = Hider::start(dir.join("journal.json"));
        assert_eq!(hider.state(), &Driver::Ready { version: None });
        let asked = Instant::now();
        hider.hide(vec![probe.clone()]);
        assert!(asked.elapsed() < Duration::from_millis(20), "asking took {:?}", asked.elapsed());
        let until = Instant::now() + Duration::from_secs(5);
        while !hider.is_hidden(&probe) && Instant::now() < until {
            hider.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(hider.is_hidden(&probe) && listed());
        hider.stop();
        assert!(!listed(), "shown again on stop");

        // This test binary is not a program that needs to see hidden devices.
        HidHide::open().unwrap().set_whitelist(&white0).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
