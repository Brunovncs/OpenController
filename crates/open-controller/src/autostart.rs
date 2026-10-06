//! Starting with the system, in the notification area or in the background, without the window.
//! Per user everywhere, so no administrator rights are needed.

#[cfg_attr(windows, path = "autostart/windows.rs")]
#[cfg_attr(target_os = "linux", path = "autostart/linux.rs")]
#[cfg_attr(target_os = "macos", path = "autostart/macos.rs")]
mod sys;

pub use sys::{enabled, refresh, set};
