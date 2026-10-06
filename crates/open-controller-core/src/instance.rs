//! One instance at a time, and signals between instances: "show" to bring the window forward,
//! "quit" for the uninstaller and the window's Quit.

#[cfg_attr(windows, path = "instance/windows.rs")]
#[cfg_attr(unix, path = "instance/unix.rs")]
mod sys;

pub use sys::*;
