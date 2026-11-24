//! Opens every gamepad SDL finds and, for each connection, prints the state SDL gives right at
//! open (before any report), whether its gyro and rumble work, and the gyro and sticks over time.
//! For checking what a controller does across a reconnect.
//!
//!     cargo run -p open-controller-core --example padcheck -- 120

use sdl3_sys::everything::*;
use std::ffi::CStr;
use std::time::{Duration, Instant};

struct Open {
    pad: *mut SDL_Gamepad,
    name: String,
    opened: Instant,
    gyro_on: bool,
    gyro_seen: f32,
    reports: u32,
}

fn axes(pad: *mut SDL_Gamepad) -> [i16; 4] {
    unsafe {
        [
            SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTX),
            SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTY),
            SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_RIGHTX),
            SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_RIGHTY),
        ]
    }
}

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    unsafe {
        let v = SDL_GetVersion();
        println!("SDL {}.{}.{}", v / 1_000_000, v / 1000 % 1000, v % 1000);
        if !SDL_Init(SDL_INIT_GAMEPAD | SDL_INIT_SENSOR) {
            println!("init failed: {}", CStr::from_ptr(SDL_GetError()).to_string_lossy());
            return;
        }
    }
    let start = Instant::now();
    let t = || format!("{:7.3}", start.elapsed().as_secs_f32());
    let mut open: Vec<(SDL_JoystickID, Open)> = Vec::new();
    let mut tick = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        unsafe {
            let mut e: SDL_Event = std::mem::zeroed();
            while SDL_PollEvent(&mut e) {
                let ty = SDL_EventType(e.r#type);
                if ty == SDL_EVENT_GAMEPAD_ADDED {
                    let id = e.gdevice.which;
                    let pad = SDL_OpenGamepad(id);
                    if pad.is_null() {
                        continue;
                    }
                    let name = CStr::from_ptr(SDL_GetGamepadName(pad)).to_string_lossy().into_owned();
                    let at_open = axes(pad);
                    let has_gyro = SDL_GamepadHasSensor(pad, SDL_SENSOR_GYRO);
                    let gyro_on = has_gyro && SDL_SetGamepadSensorEnabled(pad, SDL_SENSOR_GYRO, true);
                    let rumble = SDL_RumbleGamepad(pad, 0x8000, 0x8000, 300);
                    println!(
                        "{} ADDED {name:?} vid {:04x} pid {:04x}: sticks at open {at_open:?}, gyro {has_gyro}/{gyro_on}, rumble {rumble}",
                        t(),
                        SDL_GetGamepadVendor(pad),
                        SDL_GetGamepadProduct(pad)
                    );
                    open.push((id, Open { pad, name, opened: Instant::now(), gyro_on, gyro_seen: 0., reports: 0 }));
                } else if ty == SDL_EVENT_GAMEPAD_REMOVED {
                    let id = e.gdevice.which;
                    if let Some(i) = open.iter().position(|(x, _)| *x == id) {
                        let (_, o) = open.remove(i);
                        println!("{} REMOVED {:?} after {:.1}s", t(), o.name, o.opened.elapsed().as_secs_f32());
                        SDL_CloseGamepad(o.pad);
                    }
                } else if ty == SDL_EVENT_GAMEPAD_SENSOR_UPDATE {
                    let id = e.gsensor.which;
                    if let Some((_, o)) = open.iter_mut().find(|(x, _)| *x == id) {
                        let d = e.gsensor.data;
                        o.gyro_seen = o.gyro_seen.max(d[0].abs()).max(d[1].abs()).max(d[2].abs());
                        o.reports += 1;
                    }
                } else if ty == SDL_EVENT_GAMEPAD_AXIS_MOTION || ty == SDL_EVENT_GAMEPAD_BUTTON_DOWN {
                    let id = e.gdevice.which;
                    if let Some((_, o)) = open.iter_mut().find(|(x, _)| *x == id)
                        && o.opened.elapsed() < Duration::from_millis(500)
                    {
                        println!("{}   early {:?}: sticks {:?}", t(), o.name, axes(o.pad));
                    }
                }
            }
        }
        if tick.elapsed() >= Duration::from_secs(2) {
            tick = Instant::now();
            for (_, o) in open.iter_mut() {
                println!(
                    "{}   {:?}: gyro on {} readings/2s {} biggest {:.2} rad/s, sticks {:?}",
                    t(),
                    o.name,
                    o.gyro_on,
                    o.reports,
                    o.gyro_seen,
                    axes(o.pad)
                );
                (o.reports, o.gyro_seen) = (0, 0.);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
