//! The experiments VIIPER has to pass before more than "experimental" can be said of it (V1 to V5
//! in the plan), on a machine with usbip-win2 installed and Windows restarted since:
//!
//! - V1: how long from adding a controller to games seeing it in XInput;
//! - V2: two and then four controllers at once, all with VIIPER's one serial number;
//! - V3: how long a controller outlives the program that made it, when that program is killed:
//!   with the server in its Job Object (how OpenController runs it) and with the server left
//!   running by another program;
//! - V4: the delay from sending a report to XInput returning it, next to ViGEmBus's;
//! - V5: whether usbip-win2's virtual host controller opens without administrator rights.
//!
//! It plugs in virtual controllers for a few seconds each and changes nothing permanent. The
//! server is the one OpenController's settings installed, or `--server <path to viiper.exe>`.
//!
//!     cargo run -p open-controller-core --release --example viiper
//!
//! Windows only: it reads the virtual controllers back through XInput.

#[cfg(windows)]
include!("windows/viiper.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("VIIPER is a Windows option; this example reads controllers back through XInput");
}
