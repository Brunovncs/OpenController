//! What differs between systems, under one set of names for the engine: the virtual controller
//! bus, hiding, what the device tree says and turning a Bluetooth controller off.

use crate::ds4::Ds4Report;
use crate::mapping::XusbReport;
use crate::profile::Output;
use crossbeam_channel::Sender;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;
use std::thread::JoinHandle;

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

/// Whether a controller can be shown to games as a DualShock 4 (both Windows drivers make one).
/// Linux's uinput cannot pair a motion node with a controller in a way SDL finds, so there every
/// controller is an Xbox one.
pub const DS4_OUTPUT: bool = cfg!(windows);

/// What a virtual controller is sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Xusb(XusbReport),
    Ds4(Ds4Report),
}

impl Report {
    /// A controller of this kind at rest.
    pub fn rest(output: Output) -> Report {
        match output {
            Output::Xbox360 => Report::Xusb(XusbReport::default()),
            Output::DualShock4 => Report::Ds4(Ds4Report::default()),
        }
    }
}

/// Plugs in a virtual controller of the kind asked for; its serial on the bus.
#[cfg(windows)]
pub fn plug(bus: &Bus, io: &mut Io, output: Output) -> Result<u32, BusError> {
    bus.plug(io, output)
}

#[cfg(not(windows))]
pub fn plug(bus: &Bus, io: &mut Io, _: Output) -> Result<u32, BusError> {
    bus.plug_x360(io)
}

#[cfg(windows)]
pub fn submit(bus: &Bus, io: &mut Io, serial: u32, report: &Report) -> Result<(), u32> {
    match report {
        Report::Xusb(r) => bus.submit(io, serial, r),
        Report::Ds4(r) => bus.submit_ds4(io, serial, r),
    }
}

#[cfg(not(windows))]
pub fn submit(bus: &Bus, io: &mut Io, serial: u32, report: &Report) -> Result<(), u32> {
    match report {
        Report::Xusb(r) => bus.submit(io, serial, r),
        Report::Ds4(_) => Err(0),
    }
}

/// Starts the thread that passes on the rumble a game asks of a virtual controller.
#[cfg(windows)]
pub fn listen(bus: &Arc<Bus>, serial: u32, output: Output, tx: Sender<Feedback>) -> JoinHandle<()> {
    bus.listen(serial, output, tx)
}

#[cfg(not(windows))]
pub fn listen(bus: &Arc<Bus>, serial: u32, _: Output, tx: Sender<Feedback>) -> JoinHandle<()> {
    bus.listen(serial, tx)
}

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
