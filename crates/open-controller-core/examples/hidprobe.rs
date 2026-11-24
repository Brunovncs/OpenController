//! Watches what controllers send, report by report, to see how they behave when idle, switched
//! off, unplugged or plugged back: how often they report, whether they repeat themselves, the
//! gaps, and what a read returns when the device goes away.
//!
//!     cargo run -p open-controller-core --example hidprobe -- list
//!     cargo run -p open-controller-core --example hidprobe -- watch 90 054c 2dc8

use sdl3_sys::everything::*;
use std::ffi::{CStr, CString};
use std::time::{Duration, Instant};

struct Iface {
    path: String,
    vendor: u16,
    product: u16,
    number: i32,
    page: u16,
    name: String,
}

fn enumerate() -> Vec<Iface> {
    let mut out = Vec::new();
    unsafe {
        SDL_SetHint(SDL_HINT_HIDAPI_ENUMERATE_ONLY_CONTROLLERS, c"0".as_ptr());
        let list = SDL_hid_enumerate(0, 0);
        let mut d = list;
        while !d.is_null() {
            let i = &*d;
            let s = |p: *const std::ffi::c_char| if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() };
            // wchar_t is UTF-16 on Windows and UTF-32 elsewhere.
            let w = |p: *const sdl3_sys::ffi::c_wchar_t| {
                if p.is_null() {
                    return String::new();
                }
                let mut units = Vec::new();
                let mut n = 0;
                while *p.add(n) != 0 {
                    units.push(*p.add(n) as u32);
                    n += 1;
                }
                if size_of::<sdl3_sys::ffi::c_wchar_t>() == 2 {
                    String::from_utf16_lossy(&units.iter().map(|&u| u as u16).collect::<Vec<_>>())
                } else {
                    units.into_iter().map(|u| char::from_u32(u).unwrap_or('?')).collect()
                }
            };
            out.push(Iface {
                path: s(i.path),
                vendor: i.vendor_id,
                product: i.product_id,
                number: i.interface_number,
                page: i.usage_page,
                name: w(i.product_string),
            });
            d = i.next;
        }
        SDL_hid_free_enumeration(list);
    }
    out
}

struct Watched {
    label: String,
    vendor: u16,
    product: u16,
    number: i32,
    page: u16,
    dev: *mut SDL_hid_device,
    last: Vec<u8>,
    last_at: Option<Instant>,
    // This second.
    n: u32,
    same: u32,
    longest_gap: Duration,
    lost_at: Option<Instant>,
}

fn open(path: &str) -> *mut SDL_hid_device {
    let Ok(c) = CString::new(path) else { return std::ptr::null_mut() };
    unsafe { SDL_hid_open_path(c.as_ptr()) }
}

fn hex(b: &[u8]) -> String {
    b.iter().take(20).map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    unsafe { SDL_hid_init() };
    match args.first().map(String::as_str) {
        Some("list") => {
            for i in enumerate() {
                println!("{:04x}:{:04x} if{} page {:04x} {:?} {}", i.vendor, i.product, i.number, i.page, i.name, i.path);
            }
        }
        Some("watch") => {
            let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
            let vendors: Vec<u16> = args[2..].iter().filter_map(|v| u16::from_str_radix(v, 16).ok()).collect();
            let start = Instant::now();
            let t = |at: Instant| format!("{:7.3}", at.duration_since(start).as_secs_f32());
            let mut w: Vec<Watched> = enumerate()
                .into_iter()
                .filter(|i| vendors.contains(&i.vendor))
                .map(|i| Watched {
                    label: format!("{:04x}:{:04x} if{} page {:04x} {}", i.vendor, i.product, i.number, i.page, i.name),
                    vendor: i.vendor,
                    product: i.product,
                    number: i.number,
                    page: i.page,
                    dev: open(&i.path),
                    last: Vec::new(),
                    last_at: None,
                    n: 0,
                    same: 0,
                    longest_gap: Duration::ZERO,
                    lost_at: None,
                })
                .collect();
            for x in &w {
                println!("{} open {} {}", t(start), !x.dev.is_null(), x.label);
            }
            let mut second = Instant::now();
            let mut buf = [0u8; 128];
            while start.elapsed() < Duration::from_secs(secs) {
                let now = Instant::now();
                for x in w.iter_mut() {
                    if x.dev.is_null() {
                        // Gone: look for it again every second, as a reader would.
                        if x.lost_at.is_some_and(|at| now.duration_since(at) >= Duration::from_secs(1)) {
                            x.lost_at = Some(now);
                            if let Some(i) = enumerate()
                                .into_iter()
                                .find(|i| (i.vendor, i.product, i.number, i.page) == (x.vendor, x.product, x.number, x.page))
                            {
                                x.dev = open(&i.path);
                                println!("{} reopened {} {}", t(now), !x.dev.is_null(), x.label);
                                x.last_at = None;
                            }
                        }
                        continue;
                    }
                    loop {
                        let n = unsafe { SDL_hid_read_timeout(x.dev, buf.as_mut_ptr(), buf.len(), 0) };
                        let now = Instant::now();
                        if n < 0 {
                            let err = unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() };
                            println!("{} READ ERROR {n} ({err}) {}", t(now), x.label);
                            unsafe { SDL_hid_close(x.dev) };
                            x.dev = std::ptr::null_mut();
                            x.lost_at = Some(now);
                            break;
                        }
                        if n == 0 {
                            break;
                        }
                        let r = &buf[..n as usize];
                        if let Some(at) = x.last_at {
                            let gap = now.duration_since(at);
                            if gap > Duration::from_millis(100) {
                                println!("{} gap {:5} ms, then: {} | {}", t(now), gap.as_millis(), hex(r), x.label);
                            }
                            x.longest_gap = x.longest_gap.max(gap);
                        } else {
                            println!("{} first report: {} | {}", t(now), hex(r), x.label);
                        }
                        if r == x.last.as_slice() {
                            x.same += 1;
                        }
                        x.n += 1;
                        x.last = r.to_vec();
                        x.last_at = Some(now);
                    }
                }
                if second.elapsed() >= Duration::from_secs(1) {
                    second = Instant::now();
                    for x in w.iter_mut().filter(|x| !x.dev.is_null()) {
                        let silent = x.last_at.map_or(0, |at| second.duration_since(at).as_millis());
                        println!(
                            "{} {:4}/s, {:4} repeats, longest gap {:4} ms, silent {:5} ms, last {} | {}",
                            t(second),
                            x.n,
                            x.same,
                            x.longest_gap.as_millis(),
                            silent,
                            hex(&x.last),
                            x.label
                        );
                        (x.n, x.same, x.longest_gap) = (0, 0, Duration::ZERO);
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        _ => eprintln!("hidprobe list | hidprobe watch <seconds> <vendor hex>..."),
    }
}
