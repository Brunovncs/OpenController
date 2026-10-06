//! The core of OpenController: the engine that reads every connected controller through SDL 3
//! and presents each one to games as a virtual Xbox 360 controller (ViGEmBus on Windows, uinput
//! on Linux), hiding the original (HidHide, an evdev grab), and the protocol between the
//! resident process and the window. On macOS, which lets no program create controllers, it
//! types keys for extra buttons and sets lights. What differs between systems sits behind
//! `cfg` in [`platform`] and its modules; the mapping, device and roster logic is plain Rust and
//! tested on its own.

pub mod binding;
pub mod device;
pub mod extras;
pub mod i18n;
pub mod mapping;
pub mod models;
pub mod motion;
pub mod profile;
pub mod roster;

mod engine;
pub mod handheld;
pub mod instance;
pub mod ipc;
pub mod keyboard;
pub mod platform;
pub mod rt;
pub mod sdl;
pub mod update;

#[cfg(windows)]
pub mod bluetooth;
#[cfg(windows)]
pub mod devnode;
#[cfg(windows)]
pub mod drivers;
#[cfg(windows)]
pub mod hidhide;
#[cfg(windows)]
pub mod vigem;
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub mod xinput;
#[cfg(windows)]
pub use win::Handle;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

pub use engine::{Command, Config, Driver, Engine, GRACE, PadKey, PadView, Role, Snapshot};
