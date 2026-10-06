use open_controller_core::mapping::XusbReport;
use open_controller_core::rt::{Pacer, boost_current_thread};
use open_controller_core::vigem::{Bus, Overlapped};
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

fn summary(name: &str, mut v: Vec<Duration>) {
    v.sort();
    let at = |q: f64| v[((v.len() - 1) as f64 * q) as usize];
    println!("{name:<44} median {:>9.3?}  p99 {:>9.3?}  max {:>9.3?}  (n={})", at(0.5), at(0.99), v[v.len() - 1], v.len());
}

fn main() {
    let _boost = boost_current_thread();
    let n = 5000;
    let mut pacer = Pacer::new();
    let mut paced = Vec::with_capacity(n);
    let start = Instant::now();
    let mut last = start;
    for _ in 0..n {
        pacer.wait(Duration::from_millis(1));
        let now = Instant::now();
        paced.push(now - last);
        last = now;
    }
    let rate = n as f64 / start.elapsed().as_secs_f64();
    summary("poll period, deadline-paced 1 kHz loop", paced);
    println!("{:<44} {rate:.0} iterations per second", "");
    let mut slept = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        std::thread::sleep(Duration::from_millis(1));
        slept.push(t.elapsed());
    }
    summary("poll period, sleep(1 ms) loop", slept);

    let bus = match Bus::connect() {
        Ok(b) => b,
        Err(e) => return println!("ViGEmBus: {e}; skipping the virtual controller"),
    };
    let mut io = Overlapped::new();
    let serial = bus.plug_x360(&mut io).expect("plug");
    let slot = loop {
        if let Ok(i) = bus.user_index(&mut io, serial) {
            break i as u32;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // Let the XInput stack settle on the new controller.
    std::thread::sleep(Duration::from_millis(500));

    let mut submit = Vec::new();
    let mut seen = Vec::new();
    let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
    for i in 1..=1000i16 {
        let value = i * 30;
        let report = XusbReport { thumb_lx: value, ..Default::default() };
        let t = Instant::now();
        bus.submit(&mut io, serial, &report).expect("submit");
        submit.push(t.elapsed());
        loop {
            if unsafe { XInputGetState(slot, &mut state) } == 0 && state.Gamepad.sThumbLX == value {
                break;
            }
            if t.elapsed() > Duration::from_millis(100) {
                println!("report {i} not seen within 100 ms");
                break;
            }
        }
        seen.push(t.elapsed());
    }
    summary("submit a report to ViGEmBus", submit);
    summary("submit until XInputGetState returns it", seen);
    let _ = bus.submit(&mut io, serial, &XusbReport::default());
    let _ = bus.unplug(&mut io, serial);
}
