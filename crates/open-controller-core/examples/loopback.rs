//! An end-to-end check of the engine without a physical controller. SDL can host virtual
//! joysticks in the same process; this one plays the part of a controller, the engine turns it
//! into a virtual Xbox 360 controller, and XInput, which is what games read, is checked:
//!
//! - the controller gets an XInput slot, and the slot's player number comes back to it;
//! - a stick move and a button press arrive, and how long that takes end to end;
//! - its back paddles do what they are assigned: an Xbox button, a key held with them, and a new
//!   assignment sent while it runs (the key is F24, which nothing uses);
//! - a click on the left half of its touchpad is a button of its own;
//! - a profile that names the program in front takes over while it is in front;
//! - its gyro turns the right stick, always or only while aiming;
//! - a stick deadzone centres small movements;
//! - two controllers trade players, and each one's input reaches the other's slot;
//! - rumble a game sets through XInput reaches the controller;
//! - the controller disconnecting and reconnecting keeps the same XInput slot (the handoff);
//! - stopping the engine unplugs the virtual controller.
//!
//! Needs ViGEmBus. Changes nothing permanent; HidHide is left alone.
//!
//!     cargo run -p open-controller-core --release --example loopback
//!
//! Windows only: it reads the virtual controllers back through XInput.

#[cfg(windows)]
include!("windows/loopback.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("this example reads controllers back through XInput, which only Windows has");
}
