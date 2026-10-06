//! The X server says when the active window changes (XWayland on Wayland desktops, where games
//! under Wine and Proton run); nothing is polled. Without an X server, profiles do not switch.

pub fn watch() {
    open_controller_core::linux::x11::watch(super::report);
}
