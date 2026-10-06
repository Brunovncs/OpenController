//! Measures the two delays Open Controller adds on top of the controller itself:
//!
//! 1. the poll loop: the period of the deadline-paced 1 kHz loop it runs, and of a plain
//!    `std::thread::sleep(1 ms)` loop for comparison;
//! 2. the virtual controller: from submitting a report to ViGEmBus until XInput, which is what
//!    games read, returns it. This plugs in one virtual controller for a few seconds.
//!
//!     cargo run -p open-controller-core --release --example latency
//!
//! Windows only: it reads the virtual controllers back through XInput.

#[cfg(windows)]
include!("windows/latency.rs");

#[cfg(not(windows))]
fn main() {
    eprintln!("this example reads controllers back through XInput, which only Windows has");
}
