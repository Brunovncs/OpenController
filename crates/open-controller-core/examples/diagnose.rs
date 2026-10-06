//! Lists what SDL sees, then plugs in one virtual Xbox 360 controller, moves its left stick and
//! checks that it is recognised as OpenController's own and not read back as a new controller.
//! Changes nothing permanent: the virtual controller is gone when the program ends.
//!
//!     cargo run -p open-controller-core --example diagnose
//!
//! Windows only: it reads the virtual controllers back through XInput.

#[cfg(windows)]
include!("windows/diagnose.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("this example reads controllers back through XInput, which only Windows has");
}
