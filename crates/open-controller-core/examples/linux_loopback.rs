//! Checks the whole Linux path with SDL's virtual controller, no hardware needed: input reaches a
//! uinput Xbox 360 controller as the kernel's event node shows it, a paddle presses Y and holds
//! a key on Open Controller's own keyboard, and a game's force-feedback rumble comes back.
//!
//!     cargo build -p open-controller-core --example linux_loopback
//!     sudo modprobe uinput && sudo ./target/debug/examples/linux_loopback
//!
//! Root, or the device rule plus read access to the event nodes, is needed to read them back.

#[cfg(target_os = "linux")]
include!("linux/loopback.rs");

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("this example checks Linux's uinput controllers");
}
