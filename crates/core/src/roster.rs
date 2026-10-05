//! Which virtual controller each physical controller drives.
//!
//! A slot is one virtual Xbox 360 controller, and so one player in a game. It belongs to a
//! physical controller's [`Identity`], not to a connection: when a DualSense moves from Bluetooth
//! to the cable, the new USB device joins the same slot and the game never sees a disconnect.
//! A slot whose controller is gone is kept for a grace period before it is unplugged, which
//! covers a cable swap, a Bluetooth reconnect after the pad slept, or a flaky receiver.
//!
//! The same controller can be connected twice at once (a Bluetooth pad charging over USB). Both
//! devices join the slot and whichever sent input last drives it.

use crate::device::Identity;
use std::time::{Duration, Instant};

/// SDL's joystick instance id: one per connection, never reused while the program runs.
pub type DeviceId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub u32);

#[derive(Debug)]
pub struct Slot {
    pub id: SlotId,
    pub identity: Identity,
    /// Connected devices of this controller, in the order they connected.
    pub devices: Vec<DeviceId>,
    /// The device whose input the slot forwards.
    pub source: Option<DeviceId>,
    /// When the last device left; `None` while one is connected.
    pub lost_since: Option<Instant>,
}

impl Slot {
    /// Time left before the slot is unplugged, while its controller is away.
    pub fn remaining(&self, grace: Duration, now: Instant) -> Option<Duration> {
        self.lost_since.map(|t| grace.saturating_sub(now.saturating_duration_since(t)))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Attached {
    /// A controller seen for the first time: a virtual controller has to be plugged in.
    New(SlotId),
    /// A controller that already has a slot, back or connected a second way.
    Rejoined(SlotId),
}

pub struct Roster {
    grace: Duration,
    slots: Vec<Slot>,
    next: u32,
}

impl Roster {
    pub fn new(grace: Duration) -> Roster {
        Roster { grace, slots: Vec::new(), next: 1 }
    }

    pub fn grace(&self) -> Duration {
        self.grace
    }

    pub fn attach(&mut self, device: DeviceId, identity: Identity) -> Attached {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.identity == identity) {
            if !slot.devices.contains(&device) {
                slot.devices.push(device);
            }
            slot.source.get_or_insert(device);
            slot.lost_since = None;
            return Attached::Rejoined(slot.id);
        }
        let id = SlotId(self.next);
        self.next += 1;
        self.slots.push(Slot { id, identity, devices: vec![device], source: Some(device), lost_since: None });
        Attached::New(id)
    }

    /// The device is gone. Returns its slot, which stays until [`Roster::expire`] removes it.
    pub fn detach(&mut self, device: DeviceId, now: Instant) -> Option<SlotId> {
        let slot = self.slots.iter_mut().find(|s| s.devices.contains(&device))?;
        slot.devices.retain(|&d| d != device);
        if slot.source == Some(device) {
            slot.source = slot.devices.last().copied();
        }
        if slot.devices.is_empty() {
            slot.lost_since = Some(now);
        }
        Some(slot.id)
    }

    /// The device sent input. Returns true when that makes it the slot's source.
    pub fn touched(&mut self, device: DeviceId) -> bool {
        match self.slots.iter_mut().find(|s| s.devices.contains(&device)) {
            Some(slot) if slot.source != Some(device) => {
                slot.source = Some(device);
                true
            }
            _ => false,
        }
    }

    /// Removes and returns the slots whose controller has been away longer than the grace period.
    pub fn expire(&mut self, now: Instant) -> Vec<SlotId> {
        let grace = self.grace;
        let mut gone = Vec::new();
        self.slots.retain(|s| {
            let keep = s.lost_since.is_none_or(|t| now.saturating_duration_since(t) < grace);
            if !keep {
                gone.push(s.id);
            }
            keep
        });
        gone
    }

    /// Removes a slot whatever its state, as when its virtual controller could not be created.
    pub fn remove(&mut self, id: SlotId) -> Option<Slot> {
        let i = self.slots.iter().position(|s| s.id == id)?;
        Some(self.slots.remove(i))
    }

    pub fn slot_of(&self, device: DeviceId) -> Option<SlotId> {
        self.slots.iter().find(|s| s.devices.contains(&device)).map(|s| s.id)
    }

    pub fn get(&self, id: SlotId) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serial(s: &str) -> Identity {
        Identity::Serial(s.into())
    }

    const GRACE: Duration = Duration::from_secs(15);

    #[test]
    fn three_controllers_three_slots() {
        let mut r = Roster::new(GRACE);
        assert_eq!(r.attach(1, serial("ds4")), Attached::New(SlotId(1)));
        assert_eq!(r.attach(2, serial("dualsense")), Attached::New(SlotId(2)));
        assert_eq!(r.attach(3, Identity::Path("045e:0b12:xinput#0".into())), Attached::New(SlotId(3)));
        assert_eq!(r.slots().len(), 3);
    }

    #[test]
    fn cable_swap_keeps_the_slot() {
        let t0 = Instant::now();
        let mut r = Roster::new(GRACE);
        r.attach(1, serial("pad"));
        assert_eq!(r.detach(1, t0), Some(SlotId(1)));
        assert_eq!(r.get(SlotId(1)).unwrap().remaining(GRACE, t0 + Duration::from_secs(5)), Some(Duration::from_secs(10)));
        assert!(r.expire(t0 + Duration::from_secs(2)).is_empty());
        assert_eq!(r.attach(7, serial("pad")), Attached::Rejoined(SlotId(1)));
        let s = r.get(SlotId(1)).unwrap();
        assert_eq!((s.devices.as_slice(), s.source, s.lost_since), (&[7][..], Some(7), None));
        assert!(r.expire(t0 + GRACE * 2).is_empty(), "back before the grace period ended");
    }

    #[test]
    fn a_controller_that_stays_away_is_unplugged() {
        let t0 = Instant::now();
        let mut r = Roster::new(GRACE);
        r.attach(1, serial("a"));
        r.attach(2, serial("b"));
        r.detach(1, t0);
        assert!(r.expire(t0 + GRACE - Duration::from_millis(1)).is_empty());
        assert_eq!(r.expire(t0 + GRACE), vec![SlotId(1)]);
        assert_eq!(r.slots().len(), 1);
        assert_eq!(r.attach(3, serial("a")), Attached::New(SlotId(3)), "a new slot once the old one is gone");
    }

    #[test]
    fn two_connections_of_one_controller_share_a_slot() {
        let t0 = Instant::now();
        let mut r = Roster::new(GRACE);
        r.attach(1, serial("pad")); // Bluetooth
        assert_eq!(r.attach(2, serial("pad")), Attached::Rejoined(SlotId(1))); // and the cable
        assert_eq!(r.get(SlotId(1)).unwrap().source, Some(1), "the source moves only on input");
        assert!(r.touched(2));
        assert!(!r.touched(2));
        assert_eq!(r.get(SlotId(1)).unwrap().source, Some(2));
        r.detach(2, t0); // cable out: Bluetooth takes over without a gap
        let s = r.get(SlotId(1)).unwrap();
        assert_eq!((s.source, s.lost_since), (Some(1), None));
    }

    #[test]
    fn detaching_an_unknown_device_is_harmless() {
        let mut r = Roster::new(GRACE);
        assert_eq!(r.detach(9, Instant::now()), None);
        assert!(!r.touched(9));
    }

    #[test]
    fn slots_are_never_reused() {
        let t0 = Instant::now();
        let mut r = Roster::new(GRACE);
        r.attach(1, serial("a"));
        r.detach(1, t0);
        r.expire(t0 + GRACE);
        assert_eq!(r.attach(2, serial("b")), Attached::New(SlotId(2)));
        assert_eq!(r.remove(SlotId(2)).map(|s| s.id), Some(SlotId(2)));
        assert!(r.slots().is_empty());
    }
}
