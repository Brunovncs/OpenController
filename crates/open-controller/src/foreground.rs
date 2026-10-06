//! Which program is in front, for profiles that switch with it.

use crate::CONTROL;
use open_controller_core::Command;

#[cfg_attr(windows, path = "foreground/windows.rs")]
#[cfg_attr(target_os = "linux", path = "foreground/linux.rs")]
#[cfg_attr(target_os = "macos", path = "foreground/macos.rs")]
mod sys;

pub use sys::watch;

/// Programs that come to the front without being what the user is playing: OpenController's own
/// window, and the Start menu, search and the like. Switching profiles for them would only flicker.
const PASSING: [&str; 7] = [
    "open-controller-ui.exe",
    "open-controller-ui",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "textinputhost.exe",
    "lockapp.exe",
];

fn report(program: String) {
    if PASSING.contains(&program.as_str()) {
        return;
    }
    if let Some(c) = CONTROL.get() {
        c.engine.send(Command::Foreground(program));
    }
}
