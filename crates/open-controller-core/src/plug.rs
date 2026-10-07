//! Plugging virtual controllers in, on a thread of its own. The bus answers once the new
//! controller is ready, which takes up to a second (longer the first time on a machine, while
//! Windows installs the Xbox 360 driver), and the input thread must keep forwarding the other
//! players meanwhile.

use crate::platform::{Bus, Io};
use crate::roster::SlotId;
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct Plugger {
    bus: Arc<Bus>,
    tx: Option<Sender<SlotId>>,
    rx: Receiver<(SlotId, Result<u32, String>)>,
    thread: Option<JoinHandle<()>>,
    /// Slots whose virtual controller is being plugged in.
    pending: Vec<SlotId>,
}

impl Plugger {
    pub fn start(bus: Arc<Bus>) -> Plugger {
        let (tx, requests) = unbounded::<SlotId>();
        let (done, rx) = unbounded();
        let worker = bus.clone();
        let thread = std::thread::Builder::new()
            .name("open-controller-plug".into())
            .spawn(move || {
                let mut io = Io::new();
                for slot in requests {
                    let r = worker.plug_x360(&mut io).map_err(|e| e.to_string());
                    if let Err(crossbeam_channel::SendError((_, Ok(serial)))) = done.send((slot, r)) {
                        // Nobody to hand it to: the engine has stopped.
                        let _ = worker.unplug(&mut io, serial);
                    }
                }
            })
            .expect("could not start the plug thread");
        Plugger { bus, tx: Some(tx), rx, thread: Some(thread), pending: Vec::new() }
    }

    /// Asks for a virtual controller for `slot`, unless one is already on its way.
    pub fn request(&mut self, slot: SlotId) {
        if !self.pending.contains(&slot)
            && let Some(tx) = &self.tx
            && tx.send(slot).is_ok()
        {
            self.pending.push(slot);
        }
    }

    pub fn is_pending(&self, slot: SlotId) -> bool {
        self.pending.contains(&slot)
    }

    /// The virtual controllers plugged in (their serial on the bus) or refused since the last
    /// call.
    pub fn finished(&mut self) -> Vec<(SlotId, Result<u32, String>)> {
        let done: Vec<_> = self.rx.try_iter().collect();
        self.pending.retain(|s| !done.iter().any(|(d, _)| d == s));
        done
    }

    /// Waits for a plug still under way and unplugs whatever nobody took.
    pub fn stop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let mut io = Io::new();
        for (_, r) in self.rx.try_iter() {
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
        plugger.request(SlotId(1));
        plugger.request(SlotId(1));
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
        let serial = *done[0].1.as_ref().expect("plugged");
        assert!(!plugger.is_pending(SlotId(1)));
        bus.unplug(&mut Io::new(), serial).expect("unplug");
        // One more, left for stop() to undo.
        plugger.request(SlotId(2));
        plugger.stop();
    }
}
