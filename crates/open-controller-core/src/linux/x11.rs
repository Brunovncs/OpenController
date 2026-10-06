//! The program in front and the programs with a window, from the X server. Games under Wine and
//! Proton are X clients, XWayland included, so this covers them on Wayland desktops too. They
//! are named by their Windows executable, as on Windows, so one profile fits both.

use crate::profile::program_name;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window};
use x11rb::rust_connection::RustConnection;

struct Atoms {
    active: u32,
    clients: u32,
    pid: u32,
}

fn atoms(conn: &RustConnection) -> Option<Atoms> {
    let get = |name: &[u8]| conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
    Some(Atoms { active: get(b"_NET_ACTIVE_WINDOW")?, clients: get(b"_NET_CLIENT_LIST")?, pid: get(b"_NET_WM_PID")? })
}

fn windows(conn: &RustConnection, root: Window, atom: u32) -> Vec<Window> {
    conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 4096)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

fn pid_of(conn: &RustConnection, w: Window, atom: u32) -> Option<u32> {
    let r = conn.get_property(false, w, atom, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    r.value32()?.next()
}

/// A process's program as profiles name it. Under Wine the process is Wine's loader and the
/// game's Windows path is its first argument.
pub fn program_of_pid(pid: u32) -> Option<String> {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let name = program_name(&exe.to_string_lossy());
    if name.contains("wine") || name.contains("preloader") {
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        let first = cmdline.split(|&b| b == 0).find(|a| a.to_ascii_lowercase().ends_with(b".exe"))?;
        return Some(program_name(&String::from_utf8_lossy(first)));
    }
    Some(name)
}

fn active(conn: &RustConnection, root: Window, a: &Atoms) -> Option<String> {
    let w = *windows(conn, root, a.active).first()?;
    program_of_pid(pid_of(conn, w, a.pid)?)
}

/// The programs with a window, sorted.
pub fn open_programs() -> Vec<String> {
    let Ok((conn, screen)) = x11rb::connect(None) else { return Vec::new() };
    let root = conn.setup().roots[screen].root;
    let Some(a) = atoms(&conn) else { return Vec::new() };
    let mut names: Vec<String> =
        windows(&conn, root, a.clients).into_iter().filter_map(|w| pid_of(&conn, w, a.pid)).filter_map(program_of_pid).collect();
    names.sort();
    names.dedup();
    names
}

/// Calls `f` with the program in front now and whenever it changes, on a thread of its own.
/// False when there is no X server to ask.
pub fn watch(f: impl Fn(String) + Send + 'static) -> bool {
    let Ok((conn, screen)) = x11rb::connect(None) else { return false };
    let root = conn.setup().roots[screen].root;
    let Some(a) = atoms(&conn) else { return false };
    let mask = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    if conn.change_window_attributes(root, &mask).is_err() || conn.flush().is_err() {
        return false;
    }
    std::thread::Builder::new()
        .name("open-controller-foreground".into())
        .spawn(move || {
            let mut last = String::new();
            let mut report = |conn: &RustConnection| {
                if let Some(p) = active(conn, root, &a)
                    && p != last
                {
                    last = p.clone();
                    f(p);
                }
            };
            report(&conn);
            while let Ok(ev) = conn.wait_for_event() {
                if let Event::PropertyNotify(e) = ev
                    && e.atom == a.active
                {
                    report(&conn);
                }
            }
        })
        .is_ok()
}
