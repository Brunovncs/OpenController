//! What differs between systems, under one set of names for the engine: the virtual controller
//! bus, hiding, what the device tree says and turning a Bluetooth controller off.

#[cfg(windows)]
pub use crate::{
    bluetooth, devnode,
    hidhide::{Cloak, CloakError, JOURNAL_FILE},
    vigem::{Bus, BusError, Feedback},
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
