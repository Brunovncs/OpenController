//! Logs what games read through XInput, slot by slot: every change of buttons, sticks and
//! triggers, and slots appearing or going away, with the time.
//!
//!     cargo run -p open-controller-core --example xinputwatch -- 120

#[cfg(windows)]
fn main() {
    use std::time::{Duration, Instant};
    use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let start = Instant::now();
    type Seen = (u16, u8, u8, i16, i16, i16, i16);
    let mut last: [Option<Seen>; 4] = [None; 4];
    let mut connected = [false; 4];
    while start.elapsed() < Duration::from_secs(secs) {
        for i in 0..4u32 {
            let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
            let ok = unsafe { XInputGetState(i, &mut s) } == 0;
            let t = start.elapsed().as_secs_f32();
            if ok != connected[i as usize] {
                println!("{t:8.3} slot {i} {}", if ok { "connected" } else { "GONE" });
                connected[i as usize] = ok;
                last[i as usize] = None;
            }
            if !ok {
                continue;
            }
            let g = s.Gamepad;
            // Small stick noise left out: a change of more than 2000 or a crossing of the centre.
            let now =
                (g.wButtons, g.bLeftTrigger, g.bRightTrigger, g.sThumbLX / 2000, g.sThumbLY / 2000, g.sThumbRX / 2000, g.sThumbRY / 2000);
            if last[i as usize] != Some(now) {
                println!(
                    "{t:8.3} slot {i} buttons {:04x} LT {:3} RT {:3} L ({:6},{:6}) R ({:6},{:6})",
                    g.wButtons, g.bLeftTrigger, g.bRightTrigger, g.sThumbLX, g.sThumbLY, g.sThumbRX, g.sThumbRY
                );
                last[i as usize] = Some(now);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("XInput is Windows only");
}
