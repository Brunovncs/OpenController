use open_controller_core::mapping::XusbReport;
use open_controller_core::rt::boost_current_thread;
use open_controller_core::{vigem, viiper, xinput};
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, GetLastError, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};
use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

/// How long a controller may take to show up in XInput before it counts as not showing up.
const SHOW_UP: Duration = Duration::from_secs(10);

fn summary(name: &str, mut v: Vec<Duration>) {
    if v.is_empty() {
        return println!("{name:<44} no samples");
    }
    v.sort();
    let at = |q: f64| v[((v.len() - 1) as f64 * q) as usize];
    println!("{name:<44} median {:>9.3?}  p99 {:>9.3?}  max {:>9.3?}  (n={})", at(0.5), at(0.99), v[v.len() - 1], v.len());
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn state(slot: u8) -> Option<XINPUT_STATE> {
    let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
    (unsafe { XInputGetState(u32::from(slot), &mut s) } == 0).then_some(s)
}

/// The XInput slot showing `serial`'s marker, once it shows.
fn find(serial: u32, within: Duration) -> Option<(u8, Duration)> {
    let marker = xinput::marker(serial);
    let start = Instant::now();
    while start.elapsed() < within {
        if let Some(i) = xinput::find(&marker) {
            return Some((i, start.elapsed()));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

/// How long until `slot` is free, polling every millisecond.
fn freed(slot: u8, within: Duration) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < within {
        if state(slot).is_none() {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

fn server_path() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--server"
            && let Some(p) = args.next()
        {
            return p.into();
        }
    }
    let installed = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|d| d.join(r"Programs\open-controller\viiper\viiper.exe"));
    installed.filter(|p| p.is_file()).unwrap_or_else(viiper::server_exe)
}

/// V5: the virtual host controller, opened the way VIIPER's server opens it.
fn v5() {
    println!("\n== V5: usbip-win2's virtual host controller without administrator rights");
    let admin = unsafe { IsUserAnAdmin() } != 0;
    println!("running as administrator: {admin}{}", if admin { " (run it from a normal terminal for this check)" } else { "" });
    let Some(path) = viiper::vhci_path() else {
        return println!("not found: usbip-win2 is not installed, or Windows has not restarted since");
    };
    println!("found {path}");
    let h = unsafe {
        CreateFileW(
            wide(&path).as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if h.is_null() || h == INVALID_HANDLE_VALUE {
        println!("V5: it does NOT open: error {}", unsafe { GetLastError() });
    } else {
        println!("V5: it opens");
        unsafe { CloseHandle(h) };
    }
}

/// V1: from asking for a controller to XInput showing it, five times.
fn v1(bus: &viiper::Bus) {
    println!("\n== V1: from adding a controller to XInput showing it");
    let (mut added, mut shown) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        let t = Instant::now();
        let serial = match bus.plug_x360() {
            Ok(s) => s,
            Err(e) => return println!("V1: could not add a controller: {e}"),
        };
        added.push(t.elapsed());
        let _ = bus.submit(serial, &xinput::marker(serial));
        match find(serial, SHOW_UP) {
            Some((slot, _)) => {
                shown.push(t.elapsed());
                let _ = bus.unplug(serial);
                if freed(slot, Duration::from_secs(5)).is_none() {
                    println!("slot {slot} still taken 5 s after removing the controller");
                }
            }
            None => {
                println!("V1: the controller did not show up in XInput within {SHOW_UP:?}");
                let _ = bus.unplug(serial);
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    summary("add answered (attach included)", added);
    summary("add until XInput shows it", shown);
    println!("V1 passes if the median is under 1 to 2 s");
}

/// V2: several controllers at once, all with the serial number 296013F.
fn v2(bus: &viiper::Bus) {
    for n in [2, 4] {
        println!("\n== V2: {n} controllers at once");
        let mut serials = Vec::new();
        for _ in 0..n {
            match bus.plug_x360() {
                Ok(s) => {
                    let _ = bus.submit(s, &xinput::marker(s));
                    serials.push(s);
                }
                Err(e) => println!("could not add controller {}: {e}", serials.len() + 1),
            }
        }
        let mut slots: Vec<u8> = serials.iter().filter_map(|&s| find(s, SHOW_UP).map(|(slot, _)| slot)).collect();
        slots.sort();
        slots.dedup();
        println!("V2 ({n}): {} of {n} in XInput, in slots {slots:?}", slots.len());
        for s in serials {
            let _ = bus.unplug(s);
        }
        let stuck: Vec<u8> = slots.iter().copied().filter(|&s| freed(s, Duration::from_secs(5)).is_none()).collect();
        println!("V2 ({n}): {}", if stuck.is_empty() { "all gone after removing them".to_string() } else { format!("slots {stuck:?} still taken") });
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The child process for V3: makes one controller (on its own server, or on `attach`), shows
/// its marker and waits to be killed.
fn child(server: PathBuf, attach: Option<SocketAddr>) {
    let bus = match attach {
        Some(addr) => viiper::Bus::attach(addr),
        None => viiper::Bus::start(&server),
    };
    let bus = bus.unwrap_or_else(|e| {
        println!("error {e}");
        std::process::exit(1)
    });
    let serial = bus.plug_x360().unwrap_or_else(|e| {
        println!("error {e}");
        std::process::exit(1)
    });
    println!("serial {serial}");
    loop {
        let _ = bus.submit(serial, &xinput::marker(serial));
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// V3: kills a program that made a controller and times how long the controller stays.
fn v3(server: &std::path::Path, attach: Option<SocketAddr>) {
    let how = if attach.is_some() { "server left running by another program" } else { "server in the program's Job Object" };
    println!("\n== V3: the program is killed, {how}");
    let me = std::env::current_exe().expect("this program");
    let mut cmd = Command::new(me);
    cmd.arg("--child").arg("--server").arg(server).stdout(Stdio::piped());
    if let Some(a) = attach {
        cmd.arg("--attach").arg(a.to_string());
    }
    let mut c = cmd.spawn().expect("start the child");
    let mut line = String::new();
    let _ = BufReader::new(c.stdout.take().expect("its output")).read_line(&mut line);
    let Some(serial) = line.trim().strip_prefix("serial ").and_then(|s| s.parse::<u32>().ok()) else {
        let _ = c.kill();
        let _ = c.wait();
        return println!("V3: the child failed: {}", line.trim());
    };
    let Some((slot, _)) = find(serial, SHOW_UP) else {
        let _ = c.kill();
        let _ = c.wait();
        return println!("V3: the child's controller did not show up in XInput");
    };
    let _ = c.kill();
    let _ = c.wait();
    match freed(slot, Duration::from_secs(30)) {
        Some(d) => println!("V3 ({how}): the controller stayed {d:?} after the kill"),
        None => println!("V3 ({how}): the controller was still there 30 s after the kill"),
    }
}

/// V4: report to XInput, through VIIPER and through ViGEmBus.
fn v4(bus: &viiper::Bus) {
    println!("\n== V4: from sending a report to XInput returning it");
    let _boost = boost_current_thread();
    let Ok(serial) = bus.plug_x360() else { return println!("V4: could not add a controller") };
    let _ = bus.submit(serial, &xinput::marker(serial));
    let Some((slot, _)) = find(serial, SHOW_UP) else {
        let _ = bus.unplug(serial);
        return println!("V4: the controller did not show up in XInput");
    };
    std::thread::sleep(Duration::from_millis(500));
    summary("VIIPER: send until XInput returns it", measure(slot, |r| bus.submit(serial, r).is_ok()));
    let _ = bus.unplug(serial);

    let Ok(vigem) = vigem::Bus::connect() else { return println!("ViGEmBus is not installed: nothing to compare with") };
    let mut io = vigem::Overlapped::new();
    let Ok(serial) = vigem.plug_x360(&mut io) else { return println!("ViGEmBus: could not add a controller") };
    let _ = vigem.submit(&mut io, serial, &xinput::marker(serial));
    if let Some((slot, _)) = find(serial, SHOW_UP) {
        std::thread::sleep(Duration::from_millis(500));
        summary("ViGEmBus: send until XInput returns it", measure(slot, |r| vigem.submit(&mut io, serial, r).is_ok()));
    }
    let _ = vigem.unplug(&mut io, serial);
}

fn measure(slot: u8, mut send: impl FnMut(&XusbReport) -> bool) -> Vec<Duration> {
    let mut seen = Vec::new();
    for i in 1..=500i16 {
        let value = i * 60;
        let t = Instant::now();
        if !send(&XusbReport { thumb_lx: value, ..XusbReport::default() }) {
            println!("report {i} was not taken");
            break;
        }
        loop {
            if state(slot).is_some_and(|s| s.Gamepad.sThumbLX == value) {
                seen.push(t.elapsed());
                break;
            }
            if t.elapsed() > Duration::from_millis(200) {
                println!("report {i} not seen within 200 ms");
                break;
            }
        }
    }
    seen
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server = server_path();
    if args.iter().any(|a| a == "--child") {
        let attach = args.iter().position(|a| a == "--attach").and_then(|i| args.get(i + 1)).and_then(|a| a.parse().ok());
        return child(server, attach);
    }
    v5();
    println!("\nserver: {}", server.display());
    let bus = match viiper::Bus::start(&server) {
        Ok(b) => b,
        Err(e) => return println!("VIIPER: {e}"),
    };
    println!("VIIPER {} listening on {}", bus.version(), bus.addr());
    v1(&bus);
    v2(&bus);
    v3(&server, None);
    v3(&server, Some(bus.addr()));
    v4(&bus);
    if let Some(why) = bus.lost() {
        println!("\nthe server stopped working along the way: {why}");
    }
}
