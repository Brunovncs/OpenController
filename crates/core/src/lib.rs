//! The core of Open Controller: the engine that reads every connected controller through SDL 3
//! and presents each one to games as a virtual Xbox 360 controller through ViGEmBus, hiding the
//! original with HidHide, and the protocol between the resident process and the window.
//! Everything that touches Windows lives behind `cfg(windows)`; the mapping, device and roster
//! logic is plain Rust and tested on its own.

pub mod device;
pub mod i18n;
pub mod mapping;
pub mod roster;

#[cfg(windows)]
mod bluetooth;
#[cfg(windows)]
pub mod devnode;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
pub mod hidhide;
#[cfg(windows)]
pub mod instance;
#[cfg(windows)]
pub mod ipc;
#[cfg(windows)]
pub mod rt;
#[cfg(windows)]
pub mod sdl;
#[cfg(windows)]
pub mod vigem;
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::Handle;

#[cfg(windows)]
pub use engine::{Command, Config, Driver, Engine, GRACE, PadKey, PadView, Role, Snapshot};
