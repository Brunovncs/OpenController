use open_controller_core::device;
use open_controller_core::devnode;
use open_controller_core::mapping::XusbReport;
use open_controller_core::sdl::{Event, Gamepad, Sdl};
use open_controller_core::vigem::{Bus, Overlapped};
use std::time::{Duration, Instant};

/// Gamepads that connected during `wait`, kept open so SDL keeps reporting them.
fn list(sdl: &Sdl, wait: Duration) -> Vec<Gamepad> {
    let until = Instant::now() + wait;
    let mut seen = Vec::new();
    while Instant::now() < until {
        while let Some(ev) = sdl.poll() {
            if let Event::GamepadAdded(id) = ev {
                seen.push(id);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if seen.is_empty() {
        println!("  (no gamepads)");
    }
    let mut open = Vec::new();
    for id in seen {
        let Some(p) = sdl.open(id) else { continue };
        let path = p.path();
        let link = device::link(p.vendor(), p.product(), &path, p.connection());
        println!(
            "  #{id} {} [{:04x}:{:04x}] {:?} {:?} serial={:?} player={:?}",
            p.name(),
            p.vendor(),
            p.product(),
            p.pad_type(),
            link,
            p.serial(),
            p.player()
        );
        println!("     path={path}");
        println!("     instance={:?} vigem={} power={:?}", devnode::instance_id(&path), devnode::is_virtual(&path), p.power());
        if device::is_xinput_path(&path) {
            let t = Instant::now();
            let name = devnode::usb_product_name(p.vendor(), p.product());
            println!("     usb product name={name:?} (looked up in {:?})", t.elapsed());
        }
        open.push(p);
    }
    open
}

fn main() {
    let sdl = Sdl::init().expect("SDL");
    println!("Gamepads before:");
    let _before = list(&sdl, Duration::from_millis(700));

    let bus = match Bus::connect() {
        Ok(b) => b,
        Err(e) => {
            println!("ViGEmBus: {e}");
            return;
        }
    };
    println!("ViGEmBus {} at {}", devnode::driver_version(&bus.path).unwrap_or_default(), bus.path);
    let mut io = Overlapped::new();
    let t = Instant::now();
    let serial = bus.plug_x360(&mut io).expect("plug");
    println!("Plugged virtual controller, serial {serial}, ready in {:?}", t.elapsed());
    let t = Instant::now();
    let index = loop {
        match bus.user_index(&mut io, serial) {
            Ok(i) => break Some(i),
            Err(_) if t.elapsed() > Duration::from_secs(2) => break None,
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    println!("XInput slot {index:?} after {:?}", t.elapsed());

    let mut times = Vec::new();
    for i in 0..1000 {
        let r = XusbReport { thumb_lx: (i * 32) as i16, ..Default::default() };
        let t = Instant::now();
        if let Err(e) = bus.submit(&mut io, serial, &r) {
            println!("submit {i} failed: {e}");
        }
        times.push(t.elapsed());
    }
    times.sort();
    println!("Submit latency over 1000 reports: median {:?}, p99 {:?}, max {:?}", times[500], times[990], times[999]);

    println!("Gamepads with the virtual controller plugged in:");
    let _after = list(&sdl, Duration::from_millis(1500));

    bus.submit(&mut io, serial, &XusbReport::default()).ok();
    bus.unplug(&mut io, serial).expect("unplug");
    println!("Unplugged.");
}
