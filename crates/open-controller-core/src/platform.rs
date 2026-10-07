//! What differs between systems, under one set of names for the engine: the virtual controller
//! bus, hiding, what the device tree says and turning a Bluetooth controller off.

use serde::{Deserialize, Deserializer, Serialize};

#[cfg(windows)]
pub use crate::{
    bluetooth,
    bus::{Bus, BusError, Feedback},
    devnode,
    hidhide::{Cloak, CloakError, JOURNAL_FILE},
    win::Overlapped as Io,
};

#[cfg(target_os = "linux")]
pub use crate::linux::{
    bluetooth, devnode,
    grab::{Cloak, CloakError, JOURNAL_FILE},
    uinput::{Bus, BusError, Feedback, Io},
};

#[cfg(target_os = "macos")]
pub use crate::macos::{Bus, BusError, Cloak, CloakError, Feedback, Io, JOURNAL_FILE, bluetooth, devnode};

/// Whether this system lets a program create game controllers. Without it every controller is
/// left to games as it is, and OpenController only types keys and sets lights for it.
pub const VIRTUAL_PADS: bool = cfg!(not(target_os = "macos"));

/// Whether games find controllers in fixed player slots that OpenController can read back
/// (XInput's four). Elsewhere players are numbered by OpenController in the order controllers
/// were made.
pub const PLAYER_SLOTS: bool = cfg!(windows);

/// Whether the user picks the driver that makes the virtual controllers (Windows: ViGEmBus or
/// VIIPER). Elsewhere the system has one way and [`VirtualDriver`] is ignored.
pub const DRIVER_CHOICE: bool = cfg!(windows);

/// The driver that makes the virtual controllers on Windows. ViGEmBus is the default and the one
/// recommended; VIIPER is experimental.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub enum VirtualDriver {
    #[default]
    ViGEmBus,
    Viiper,
}

/// A value this version does not know, written by a newer one, reads as the default instead of
/// failing the whole settings file.
impl<'de> Deserialize<'de> for VirtualDriver {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match serde_json::Value::deserialize(d)?.as_str() {
            Some("Viiper") => VirtualDriver::Viiper,
            _ => VirtualDriver::ViGEmBus,
        })
    }
}

/// Connects to the virtual controller bus: the chosen one on Windows, the system's own elsewhere.
#[cfg(windows)]
pub fn connect_bus(driver: VirtualDriver) -> Result<Bus, BusError> {
    Bus::connect(driver)
}

#[cfg(not(windows))]
pub fn connect_bus(_: VirtualDriver) -> Result<Bus, BusError> {
    Bus::connect()
}

/// The bus driver's version, as its maker numbers it.
#[cfg(windows)]
pub fn bus_version(bus: &Bus) -> Option<String> {
    bus.version()
}

#[cfg(not(windows))]
pub fn bus_version(bus: &Bus) -> Option<String> {
    devnode::driver_version(&bus.path)
}

/// Why a bus that worked stopped working (VIIPER's server can end on its own); `None` while it
/// works.
#[cfg(windows)]
pub fn bus_lost(bus: &Bus) -> Option<String> {
    bus.lost()
}

#[cfg(not(windows))]
pub fn bus_lost(_: &Bus) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_driver_reads_as_the_default() {
        assert_eq!(serde_json::from_str::<VirtualDriver>(r#""Viiper""#).unwrap(), VirtualDriver::Viiper);
        assert_eq!(serde_json::from_str::<VirtualDriver>(r#""ViGEmBus""#).unwrap(), VirtualDriver::ViGEmBus);
        assert_eq!(serde_json::from_str::<VirtualDriver>(r#""SomethingNewer""#).unwrap(), VirtualDriver::ViGEmBus);
        assert_eq!(serde_json::from_str::<VirtualDriver>("7").unwrap(), VirtualDriver::ViGEmBus);
        assert_eq!(serde_json::to_string(&VirtualDriver::Viiper).unwrap(), r#""Viiper""#);
    }
}
