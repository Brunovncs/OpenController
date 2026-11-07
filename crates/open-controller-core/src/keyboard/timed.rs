//! Independent timed shortcut deadlines: a long hold never queues another shortcut behind it.
use super::Out;
use crate::{PadKey, binding::Chord};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub(super) enum Command {
    Press(Chord, PadKey, Duration),
    Cancel(PadKey),
}

struct Lease {
    chord: Chord,
    owner: PadKey,
    until: Instant,
}
#[derive(Default)]
struct Leases(Vec<Lease>);
impl Leases {
    fn press(&mut self, chord: Chord, owner: PadKey, until: Instant) -> Option<Out> {
        if self.0.len() >= 128 || self.0.iter().any(|p| p.owner == owner && p.chord == chord) {
            return None;
        }
        self.0.push(Lease { chord, owner, until });
        Some(Out::Down(chord))
    }
    fn release(&mut self, predicate: impl Fn(&Lease) -> bool) -> Vec<Out> {
        let mut out = Vec::new();
        self.0.retain(|p| {
            if predicate(p) {
                out.push(Out::Up(p.chord));
                false
            } else {
                true
            }
        });
        out
    }
    fn wait(&self, now: Instant) -> Duration {
        self.0.iter().map(|p| p.until.saturating_duration_since(now)).min().unwrap_or(Duration::from_secs(60))
    }
}

pub(super) fn start(keys: Sender<Out>, stop: Receiver<()>) -> (Sender<Command>, std::thread::JoinHandle<()>) {
    let (send, jobs) = unbounded();
    let worker = std::thread::Builder::new()
        .name("open-controller-shortcuts".into())
        .spawn(move || {
            let mut leases = Leases::default();
            loop {
                crossbeam_channel::select! {
                    recv(stop) -> _ => break,
                    recv(jobs) -> job => match job {
                        Ok(Command::Press(chord, owner, duration)) => {
                            if let Some(out) = leases.press(chord, owner, Instant::now() + duration) { let _ = keys.send(out); }
                        }
                        Ok(Command::Cancel(owner)) => for out in leases.release(|p| p.owner == owner) { let _ = keys.send(out); },
                        Err(_) => break,
                    },
                    default(leases.wait(Instant::now())) => {}
                }
                for out in leases.release(|p| p.until <= Instant::now()) {
                    let _ = keys.send(out);
                }
            }
            for out in leases.release(|_| true) {
                let _ = keys.send(out);
            }
        })
        .expect("could not start the shortcut timer");
    (send, worker)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_shortcut_does_not_delay_another_and_cancel_is_owner_scoped() {
        let a = Chord::new(1, 9);
        let b = Chord::new(3, 5);
        let now = Instant::now();
        let mut leases = Leases::default();
        assert_eq!(leases.press(a, PadKey::Slot(0), now + Duration::from_secs(60)), Some(Out::Down(a)));
        assert_eq!(leases.press(b, PadKey::Slot(1), now + Duration::from_millis(100)), Some(Out::Down(b)));
        assert_eq!(leases.release(|p| p.until <= now + Duration::from_millis(100)), [Out::Up(b)]);
        assert!(leases.release(|p| p.owner == PadKey::Slot(1)).is_empty());
        assert_eq!(leases.release(|p| p.owner == PadKey::Slot(0)), [Out::Up(a)]);
    }
    #[test]
    fn stopping_the_worker_releases_long_outputs_immediately() {
        let chord = Chord::new(4, 9);
        let (keys, output) = unbounded();
        let (stop, stopped) = crossbeam_channel::bounded(0);
        let (jobs, worker) = start(keys, stopped);
        jobs.send(Command::Press(chord, PadKey::Slot(0), Duration::from_secs(60))).unwrap();
        assert_eq!(output.recv().unwrap(), Out::Down(chord));
        drop(stop);
        assert_eq!(output.recv_timeout(Duration::from_secs(1)).unwrap(), Out::Up(chord));
        worker.join().unwrap();
    }
}
