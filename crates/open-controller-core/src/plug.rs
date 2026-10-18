//! Plugging virtual controllers in, on a thread of its own. The bus answers once the new
//! controller is ready, which takes up to a second (longer the first time on a machine, while
//! Windows installs the Xbox 360 driver), and the input thread must keep forwarding the other
//! players meanwhile.

use crate::platform::{self, Bus, Io};
use crate::profile::Output;
use crate::roster::SlotId;
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long stopping waits for a plug under way once it is cancelled. VIIPER's server cannot be
/// interrupted and may take ten seconds to answer; past this the plug finishes on its own thread
/// and undoes itself, rather than every player's input freezing meanwhile.
const STOP_WAIT: Duration = Duration::from_millis(250);

/// A slot's virtual controller, plugged in (its serial on the bus) or refused.
pub type Plugged = (SlotId, Output, Result<u32, String>);

pub struct Plugger {
    bus: Arc<Bus>,
    tx: Option<Sender<(SlotId, Output)>>,
    rx: Receiver<Plugged>,
    thread: Option<JoinHandle<()>>,
    /// Set on stop. Requests still queued are dropped, and a plug that finishes after it is
    /// undone on the thread; it is held while a result is handed over, so none slips past.
    stopping: Arc<Mutex<bool>>,
    /// Slots whose virtual controller is being plugged in.
    pending: Vec<SlotId>,
}

impl Plugger {
    pub fn start(bus: Arc<Bus>) -> Plugger {
        let (tx, requests) = unbounded::<(SlotId, Output)>();
        let (done, rx) = unbounded();
        let worker = bus.clone();
        let stopping = Arc::new(Mutex::new(false));
        let stop = stopping.clone();
        let thread = std::thread::Builder::new()
            .name("open-controller-plug".into())
            .spawn(move || {
                let mut io = Io::new();
                for (slot, output) in requests {
                    if stop.lock().map_or(true, |s| *s) {
                        return;
                    }
                    let r = platform::plug(&worker, &mut io, output).map_err(|e| e.to_string());
                    let unwanted = match stop.lock() {
                        Ok(s) if !*s => match done.send((slot, output, r)) {
                            Err(crossbeam_channel::SendError((_, _, Ok(serial)))) => Some(serial),
                            _ => None,
                        },
                        _ => r.ok(),
                    };
                    // Nobody to take it: the engine has stopped, or let go of this bus.
                    if let Some(serial) = unwanted {
                        let _ = worker.unplug(&mut io, serial);
                    }
                }
            })
            .expect("could not start the plug thread");
        Plugger { bus, tx: Some(tx), rx, thread: Some(thread), stopping, pending: Vec::new() }
    }

    /// Asks for a virtual controller of the kind `output` for `slot`, unless one is already on
    /// its way.
    pub fn request(&mut self, slot: SlotId, output: Output) {
        if !self.pending.contains(&slot)
            && let Some(tx) = &self.tx
            && tx.send((slot, output)).is_ok()
        {
            self.pending.push(slot);
        }
    }

    pub fn is_pending(&self, slot: SlotId) -> bool {
        self.pending.contains(&slot)
    }

    /// The virtual controllers plugged in or refused since the last call.
    pub fn finished(&mut self) -> Vec<Plugged> {
        let done: Vec<Plugged> = self.rx.try_iter().collect();
        self.pending.retain(|s| !done.iter().any(|(d, _, _)| d == s));
        done
    }

    /// Drops the requests still queued, cancels a plug under way and unplugs whatever nobody
    /// took. Only called as the bus is let go of: cancelling ends every request on it.
    pub fn stop(&mut self) {
        if let Ok(mut s) = self.stopping.lock() {
            *s = true;
        }
        self.tx = None;
        if let Some(t) = self.thread.take() {
            if !t.is_finished() {
                self.bus.cancel_all();
            }
            let until = Instant::now() + STOP_WAIT;
            while !t.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(2));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
        let mut io = Io::new();
        for (_, _, r) in self.rx.try_iter() {
            if let Ok(serial) = r {
                let _ = self.bus.unplug(&mut io, serial);
            }
        }
        self.pending.clear();
    }
}

impl Drop for Plugger {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Plugs a virtual controller in through the thread and checks that asking returns at once
    /// while the bus takes its time. Needs ViGEmBus:
    /// `cargo test -p open-controller-core -- --ignored plug`.
    #[test]
    #[ignore]
    fn plug_on_the_bus_without_blocking() {
        let bus = Arc::new(Bus::connect(crate::platform::VirtualDriver::ViGEmBus).expect("ViGEmBus"));
        let mut plugger = Plugger::start(bus.clone());
        let asked = Instant::now();
        plugger.request(SlotId(1), Output::Xbox360);
        plugger.request(SlotId(1), Output::Xbox360);
        assert!(asked.elapsed() < Duration::from_millis(20), "asking took {:?}", asked.elapsed());
        assert!(plugger.is_pending(SlotId(1)));
        let until = Instant::now() + Duration::from_secs(10);
        let done = loop {
            let done = plugger.finished();
            if !done.is_empty() || Instant::now() > until {
                break done;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        println!("ready after {:?}", asked.elapsed());
        assert_eq!(done.len(), 1, "one controller for a slot asked twice");
        let serial = *done[0].2.as_ref().expect("plugged");
        assert!(!plugger.is_pending(SlotId(1)));
        bus.unplug(&mut Io::new(), serial).expect("unplug");
        // One more, left for stop() to undo.
        plugger.request(SlotId(2), Output::DualShock4);
        plugger.stop();
    }
}
