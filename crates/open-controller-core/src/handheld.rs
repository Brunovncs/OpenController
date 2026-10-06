//! Handheld PCs, whose built-in controller is an XInput pad with buttons beyond the Xbox set
//! that never reach Windows as gamepad buttons. Depending on the maker they arrive as function
//! keys (AYANEO, OneXPlayer, ZOTAC) or on a HID interface of the maker's own (Lenovo Legion Go).
//! The machine is recognised by the name its firmware gives, the buttons are read without
//! writing anything to the controller, and they are handed to the engine as extra buttons of the
//! built-in pad (`extras::HANDHELD`), which the user can assign keys and macros like any other.
//!
//! None of this has been tried on the hardware yet: the keys and report layouts come from what
//! other projects document about these machines.

use crate::extras::HANDHELD;
use crossbeam_channel::Sender;
#[cfg(windows)]
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
#[cfg(windows)]
use std::sync::atomic::Ordering;

/// Where a button's presses come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A key the firmware types, swallowed so it does nothing else.
    Key(u16),
    /// A bit in the maker's HID input report: byte offset (report id included) and mask.
    Hid { byte: usize, mask: u8 },
}

#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub label: &'static str,
    pub source: Source,
}

/// The maker's HID interface a machine's buttons are read from.
#[derive(Clone, Copy, Debug)]
pub struct Interface {
    pub vendor: u16,
    pub products: &'static [u16],
    /// `None` takes the first interface with a vendor-defined usage page.
    pub number: Option<i32>,
    pub report_id: Option<u8>,
}

#[derive(Debug)]
pub struct Machine {
    pub name: &'static str,
    /// Words of the firmware's manufacturer name, one of which must appear (lowercase).
    maker: &'static [&'static str],
    /// Words of its product name or version, one of which must appear; empty for any.
    product: &'static [&'static str],
    pub buttons: &'static [Button],
    pub interface: Option<Interface>,
}

const fn key(label: &'static str, vk: u16) -> Button {
    Button { label, source: Source::Key(vk) }
}

const fn hid(label: &'static str, byte: usize, mask: u8) -> Button {
    Button { label, source: Source::Hid { byte, mask } }
}

// Virtual-key codes of F15 to F24.
const F15: u16 = 0x7E;
const F16: u16 = 0x7F;
const F17: u16 = 0x80;
const F18: u16 = 0x81;
const F19: u16 = 0x82;
const F20: u16 = 0x83;
const F21: u16 = 0x84;
const F22: u16 = 0x85;
const F23: u16 = 0x86;
const F24: u16 = 0x87;

pub static MACHINES: &[Machine] = &[
    // Recent AYANEO models (AYANEO 3, NEXT II, KUN, Flip): each button is its own function key.
    Machine {
        name: "AYANEO",
        maker: &["ayaneo", "ayadevice"],
        product: &[],
        buttons: &[
            key("AYA", F23),
            key("◫", F24),
            key("LC", F21),
            key("RC", F22),
            key("L4", F19),
            key("R4", F20),
            key("L5", F17),
            key("R5", F18),
        ],
        interface: None,
    },
    Machine {
        name: "ZOTAC Gaming Zone",
        maker: &["zotac"],
        product: &["zone"],
        buttons: &[key("ZOTAC", F17), key("···", F18)],
        interface: None,
    },
    // OneXPlayer models whose back buttons type F15 and F16 (X2 Mini Pro and others).
    Machine {
        name: "OneXPlayer",
        maker: &["one-netbook", "onexplayer"],
        product: &[],
        buttons: &[key("M1", F15), key("M2", F16)],
        interface: None,
    },
    Machine {
        name: "Lenovo Legion Go S",
        maker: &["lenovo"],
        product: &["legion go s", "83l3", "83n6", "83q2", "83q3"],
        buttons: &[hid("Y1", 2, 0x01), hid("Y2", 2, 0x02), hid("Legion L", 0, 0x01), hid("Legion R", 0, 0x02)],
        interface: Some(Interface { vendor: 0x1A86, products: &[0xE310, 0xE311], number: None, report_id: None }),
    },
    Machine {
        name: "Lenovo Legion Go 2",
        maker: &["lenovo"],
        product: &["legion go 2", "83n0", "83n1"],
        buttons: LEGION_GO,
        interface: Some(Interface { vendor: 0x17EF, products: &[0x61EB, 0x61EC, 0x61ED, 0x61EE], number: Some(2), report_id: Some(0x04) }),
    },
    Machine {
        name: "Lenovo Legion Go",
        maker: &["lenovo"],
        product: &["legion go", "83e1"],
        buttons: LEGION_GO,
        interface: Some(Interface { vendor: 0x17EF, products: &[0x6182, 0x6183, 0x6184, 0x6185], number: Some(2), report_id: Some(0x04) }),
    },
];

const LEGION_GO: &[Button] = &[
    hid("Y1", 20, 0x80),
    hid("Y2", 20, 0x40),
    hid("Y3", 20, 0x20),
    hid("M1", 20, 0x10),
    hid("M2", 20, 0x08),
    hid("M3", 20, 0x04),
    hid("Legion L", 18, 0x80),
    hid("Legion R", 18, 0x40),
    hid("Wheel", 21, 0x80),
];

/// The machine for a firmware's manufacturer name and its product name and version.
pub fn machine_for(maker: &str, product: &str) -> Option<&'static Machine> {
    let (maker, product) = (maker.to_lowercase(), product.to_lowercase());
    MACHINES
        .iter()
        .find(|m| m.maker.iter().any(|w| maker.contains(w)) && (m.product.is_empty() || m.product.iter().any(|w| product.contains(w))))
}

/// The machine this is, from the names its firmware gives; read once. Only Windows reads
/// handheld buttons: on Linux, Handheld Daemon and InputPlumber do.
#[cfg(not(windows))]
pub fn this_machine() -> Option<&'static Machine> {
    None
}

/// The machine this is, from the names its firmware gives; read once.
#[cfg(windows)]
pub fn this_machine() -> Option<&'static Machine> {
    static M: OnceLock<Option<&'static Machine>> = OnceLock::new();
    *M.get_or_init(|| {
        let maker = bios_string("SystemManufacturer").unwrap_or_default();
        // Lenovo puts the machine type in the product name and "Legion Go" in the version.
        let product =
            format!("{} {}", bios_string("SystemProductName").unwrap_or_default(), bios_string("SystemVersion").unwrap_or_default());
        machine_for(&maker, &product)
    })
}

/// The SDL button index each of a machine's buttons is reported as.
pub fn button_index(i: usize) -> Option<u8> {
    HANDHELD.get(i).copied()
}

/// The buttons held in a HID report, as bits of `PadState::buttons`.
pub fn read_report(m: &Machine, report: &[u8]) -> u64 {
    let iface = m.interface.as_ref();
    if let Some(id) = iface.and_then(|i| i.report_id)
        && report.first() != Some(&id)
    {
        return 0;
    }
    let mut bits = 0u64;
    for (i, b) in m.buttons.iter().enumerate() {
        if let (Source::Hid { byte, mask }, Some(index)) = (b.source, button_index(i))
            && report.get(byte).is_some_and(|v| v & mask != 0)
        {
            bits |= 1 << index;
        }
    }
    bits
}

#[cfg(windows)]
fn bios_string(value: &str) -> Option<String> {
    use crate::win::{from_wide, wide};
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let mut buf = [0u16; 256];
    let mut len = size_of_val(&buf) as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(r"HARDWARE\DESCRIPTION\System\BIOS").as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    (ok == 0).then(|| from_wide(&buf))
}

/// A change in the buttons of the machine's built-in controller: all that are held now.
pub type Held = u64;

#[cfg(not(windows))]
pub fn start(_: Sender<Held>, _: &'static AtomicBool) {}

/// Starts reading the machine's buttons, if it is a known handheld, sending every change of
/// what is held. The readers stop when `stop` is set.
#[cfg(windows)]
pub fn start(tx: Sender<Held>, stop: &'static AtomicBool) {
    let Some(m) = this_machine() else { return };
    if m.buttons.iter().any(|b| matches!(b.source, Source::Key(_))) {
        hook::start(m, tx.clone());
    }
    if let Some(iface) = m.interface {
        std::thread::Builder::new().name("open-controller-handheld".into()).spawn(move || hid_reader(m, iface, tx, stop)).ok();
    }
}

#[cfg(windows)]
fn hid_reader(m: &'static Machine, iface: Interface, tx: Sender<Held>, stop: &AtomicBool) {
    use sdl3_sys::everything::*;
    use std::ffi::CStr;
    unsafe {
        if SDL_hid_init() != 0 {
            return;
        }
        let mut dev: *mut SDL_hid_device = std::ptr::null_mut();
        for &product in iface.products {
            let list = SDL_hid_enumerate(iface.vendor, product);
            let mut info = list;
            while !info.is_null() && dev.is_null() {
                let i = &*info;
                let fits = match iface.number {
                    Some(n) => i.interface_number == n,
                    None => i.usage_page >= 0xFF00,
                };
                if fits && !i.path.is_null() {
                    dev = SDL_hid_open_path(CStr::from_ptr(i.path).as_ptr());
                }
                info = i.next;
            }
            SDL_hid_free_enumeration(list);
            if !dev.is_null() {
                break;
            }
        }
        if dev.is_null() {
            SDL_hid_exit();
            return;
        }
        let mut buf = [0u8; 64];
        let mut last = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let n = SDL_hid_read_timeout(dev, buf.as_mut_ptr(), buf.len(), 200);
            if n < 0 {
                break;
            }
            if n == 0 {
                continue;
            }
            let held = read_report(m, &buf[..n as usize]);
            if held != last {
                last = held;
                if tx.send(held).is_err() {
                    break;
                }
            }
        }
        // Whatever was held is let go: the engine keeps the last state it was sent.
        if last != 0 {
            let _ = tx.send(0);
        }
        SDL_hid_close(dev);
        SDL_hid_exit();
    }
}

/// The function keys some handhelds type for their buttons, caught system-wide with a
/// low-level keyboard hook and swallowed. Only the exact keys of the recognised machine are
/// touched; keys OpenController types itself pass through.
#[cfg(windows)]
mod hook {
    use super::{Held, Machine, Source, button_index};
    use crossbeam_channel::Sender;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_LWIN, VK_RWIN,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, SetWindowsHookExW, WH_KEYBOARD_LL, WM_KEYDOWN,
        WM_SYSKEYDOWN,
    };

    struct State {
        keys: Vec<(u16, u8)>,
        held: Held,
        tx: Sender<Held>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    pub fn start(m: &'static Machine, tx: Sender<Held>) {
        let keys = m
            .buttons
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match b.source {
                Source::Key(vk) => button_index(i).map(|index| (vk, index)),
                Source::Hid { .. } => None,
            })
            .collect();
        if let Ok(mut s) = STATE.lock() {
            *s = Some(State { keys, held: 0, tx });
        }
        std::thread::Builder::new()
            .name("open-controller-handheld-keys".into())
            .spawn(|| unsafe {
                // The hook runs on this thread, which must pump messages for it to be called.
                if SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc_), std::ptr::null_mut(), 0).is_null() {
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            })
            .ok();
    }

    unsafe extern "system" fn proc_(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let k = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            if k.flags & LLKHF_INJECTED == 0
                && let Ok(mut guard) = STATE.lock()
                && let Some(s) = guard.as_mut()
                && let Some(&(_, index)) = s.keys.iter().find(|(vk, _)| u32::from(*vk) == k.vkCode)
            {
                let down = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;
                let held = if down { s.held | 1 << index } else { s.held & !(1 << index) };
                if held != s.held {
                    s.held = held;
                    let _ = s.tx.send(held);
                }
                if down && unsafe { GetAsyncKeyState(i32::from(VK_LWIN)) < 0 || GetAsyncKeyState(i32::from(VK_RWIN)) < 0 } {
                    // The firmware held Windows with the key: with the key swallowed, letting
                    // go of Windows alone would open the Start menu. An unassigned key in
                    // between keeps it closed.
                    mask_start_menu();
                }
                return 1;
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    fn mask_start_menu() {
        const UNASSIGNED: u16 = 0xE8;
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: UNASSIGNED, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let inputs = [key(0), key(KEYEVENTF_KEYUP)];
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machines_by_firmware_name() {
        assert_eq!(machine_for("AYANEO", "AYANEO 3").map(|m| m.name), Some("AYANEO"));
        assert_eq!(machine_for("LENOVO", "83E1 Legion Go 8APU1").map(|m| m.name), Some("Lenovo Legion Go"));
        assert_eq!(machine_for("LENOVO", "83L3 Legion Go S 8APU1").map(|m| m.name), Some("Lenovo Legion Go S"));
        assert_eq!(machine_for("ZOTAC", "ZBOX-EN173070C").map(|m| m.name), None, "a ZOTAC mini PC is not the Zone");
        assert_eq!(machine_for("LENOVO", "ThinkPad X1 Carbon").map(|m| m.name), None, "not every Lenovo is a handheld");
        assert_eq!(machine_for("Dell Inc.", "XPS 15").map(|m| m.name), None);
    }

    #[test]
    fn every_button_fits_the_bits_reserved_for_them() {
        for m in MACHINES {
            assert!(m.buttons.len() <= HANDHELD.len(), "{}", m.name);
        }
    }

    #[test]
    fn legion_go_report() {
        let m = machine_for("LENOVO", "83E1").unwrap();
        let mut report = [0u8; 64];
        report[0] = 0x04;
        report[20] = 0x80 | 0x10;
        report[18] = 0x40;
        let bits = read_report(m, &report);
        assert_eq!(bits, 1 << HANDHELD[0] | 1 << HANDHELD[3] | 1 << HANDHELD[7], "Y1, M1, Legion R");
        report[0] = 0x05;
        assert_eq!(read_report(m, &report), 0, "another report id is not the buttons");
    }
}
