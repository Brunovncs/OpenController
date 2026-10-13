//! An end-to-end check of a controller shown to games as a DualShock 4, without a physical
//! controller. An SDL virtual joystick with a gyro and an accelerometer plays the controller, the
//! engine makes it a virtual DualShock 4, and the DualShock 4's HID reports, what games and Steam
//! Input read, are checked:
//!
//! - its buttons, sticks and triggers arrive where a DualShock 4 has them;
//! - the gyro and the accelerometer arrive as SDL reads them back, through the calibration the
//!   virtual DualShock 4 gives;
//! - a stick set to stand in for the d-pad presses the d-pad;
//! - rumble a game sends to the DualShock 4 reaches the controller;
//! - stopping the engine unplugs it.
//!
//! Needs ViGEmBus. Changes nothing permanent; HidHide is left alone.
//!
//!     cargo run -p open-controller-core --example ds4_loopback
//!
//! Windows only, where a controller can be shown as a DualShock 4.

#[cfg(windows)]
include!("windows/ds4_loopback.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("only Windows shows controllers to games as a DualShock 4");
}
