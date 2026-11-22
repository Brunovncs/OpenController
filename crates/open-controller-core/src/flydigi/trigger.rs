//! The triggers of the Flydigi Apex 3, 4 and 5, which push back against the finger.
//!
//! One command sets one trigger: a mode and five parameters, the same on both generations
//! (FlydigiPcSpace's `SetTriggerParams`, Space Station's `ForceTriggerConfig*`, and PadForge and
//! ApexSenseBridge, whose authors ran them on the pads). Its "save" byte is always 0 here: the
//! effect then lasts until the pad is turned off or told otherwise, and the profile stored in the
//! pad stays as it was. With it set, the pad also rewrites its profile, and the Apex 4 stalls for
//! about a second on its receiver.
//!
//! The Lab's effects are DualSense-like (zones 0 to 9, strength 0 to 8); [`Command::from_effect`]
//! turns them into Flydigi's modes. Game mods made for Flydigi's app send Flydigi's own modes
//! over UDP ([`from_mod`]), and are passed on as they are.

use crate::lab::{EffectMode, TriggerEffect};

/// Which trigger, as the pads number them.
pub const LEFT: u8 = 1;
pub const RIGHT: u8 = 2;

/// Flydigi's modes (`AdapterTriggerType`).
pub mod mode {
    /// No effect: clears the one before.
    pub const NORMAL: u8 = 0;
    /// Even resistance past a point: start, force, match.
    pub const RACE: u8 = 1;
    /// Vibration past a point: start, pressure, strength, frequency, match.
    pub const SNIPER: u8 = 2;
    /// Resistance over a stretch that then gives way: start, length, force, 0, match.
    pub const RECOIL: u8 = 3;
    /// A hard stop: start, strength, match.
    pub const LOCK: u8 = 4;
    /// As `SNIPER`.
    pub const VIBRATION: u8 = 5;
}

/// The whole travel of a trigger in Flydigi's units: the "start" sliders of Space Station end there.
pub const TRAVEL: u8 = 192;

/// One trigger's effect as the pad takes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub mode: u8,
    pub params: [u8; 5],
}

impl Command {
    pub const OFF: Command = Command { mode: mode::NORMAL, params: [0; 5] };

    pub fn is_off(self) -> bool {
        self.mode == mode::NORMAL
    }

    /// A Lab effect in Flydigi's terms. Resistance is a Race from its start zone, a click a Recoil
    /// from its start to its end zone, a vibration a Vibration at its frequency in hertz (Flydigi
    /// gives no unit; the Lab's 1 to 100 fit its 1 to 255).
    pub fn from_effect(e: TriggerEffect) -> Command {
        let e = e.clean();
        if e.mode == EffectMode::Off || e.strength == 0 {
            return Command::OFF;
        }
        let travel = |zone: u8| (u16::from(zone.min(9)) * u16::from(TRAVEL) / 9) as u8;
        let force = ((u16::from(e.strength) * 255).div_ceil(8)).max(1) as u8;
        let start = travel(e.start);
        match e.mode {
            EffectMode::Resistance => Command { mode: mode::RACE, params: [start, force, 0, 0, 0] },
            EffectMode::Click => {
                let length = travel(e.end).saturating_sub(start).max(1);
                Command { mode: mode::RECOIL, params: [start, length, force, 0, 0] }
            }
            EffectMode::Vibration => Command { mode: mode::VIBRATION, params: [start, 1, force, e.frequency.max(1), 0] },
            EffectMode::Off => Command::OFF,
        }
    }

    /// Mode and parameters as they go out. Flydigi's own live path turns a Race from the very
    /// start with match on into one with match off.
    fn wire(self) -> [u8; 6] {
        let mut p = self.params;
        if self.mode == mode::RACE && p[0] == 0 && p[2] == 1 {
            p[2] = 0;
        }
        [self.mode, p[0], p[1], p[2], p[3], p[4]]
    }
}

/// The older pads' command (Apex 3, Apex 4): output report 0x05, command 0xA0 1. The byte after the
/// parameters is the sum of all before it, which FlydigiPcSpace writes; the Apex 4 takes it
/// without (PadForge sends 0 there), so it is always sent.
pub fn v1_report(side: u8, c: Command) -> [u8; 15] {
    let mut r = [0u8; 15];
    r[..5].copy_from_slice(&[0x05, 0xA0, 0x01, 0, side]);
    r[5..11].copy_from_slice(&c.wire());
    r[11] = r[..11].iter().fold(0u8, |s, &b| s.wrapping_add(b));
    r
}

/// The Apex 5's command: output report 0x03, `5A A5`, command 0x51, length 10. It carries no
/// checksum, unlike the newer pads' other commands.
pub fn v2_report(side: u8, c: Command) -> [u8; 32] {
    let mut r = [0u8; 32];
    r[..7].copy_from_slice(&[0x03, 0x5A, 0xA5, 0x51, 10, 0, side]);
    r[7..13].copy_from_slice(&c.wire());
    r
}

/// The port game mods made for Flydigi's app send to (Space Station's `adapter_trigger_config`).
pub const MOD_PORT: u16 = 7878;

/// The effects in a packet from such a mod, left and right; `None` leaves that trigger as it was.
/// The packets are DSX-style JSON, `{"instructions":[{"type":1,"parameters":[...]}]}`, but the
/// parameters already speak Flydigi: the controller, the side (1 left, 2 right), one ignored,
/// then the mode and its five parameters, as Space Station's `OnTriggerCommandReceived` reads
/// them. Mode 255 keeps what was there. Anything else in the packet is left alone.
pub fn from_mod(packet: &[u8]) -> [Option<Command>; 2] {
    let mut out = [None; 2];
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(packet) else { return out };
    let Some(list) = v.get("instructions").and_then(|i| i.as_array()) else { return out };
    for i in list.iter().take(16) {
        if i.get("type").and_then(number) != Some(1) {
            continue;
        }
        let Some(p) = i.get("parameters").and_then(|p| p.as_array()) else { continue };
        let byte = |k: usize| p.get(k).and_then(number).filter(|&v| v <= 255).map(|v| v as u8);
        let (Some(side), Some(m)) = (byte(1), byte(3)) else { continue };
        let slot = match side {
            LEFT => 0,
            RIGHT => 1,
            _ => continue,
        };
        if m == 255 {
            continue;
        }
        if m > mode::VIBRATION {
            continue;
        }
        let params = [4, 5, 6, 7, 8].map(|k| byte(k).unwrap_or(0));
        out[slot] = Some(Command { mode: m, params });
    }
    out
}

/// A whole JSON number, as an integer or a float that is one.
fn number(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().filter(|f| f.fract() == 0. && *f >= 0.).map(|f| f as u64))
}

/// How long to wait before trying the port again when another program has it (Space Station
/// takes it while its adaptive triggers are on).
const RETRY: std::time::Duration = std::time::Duration::from_secs(30);
/// The most packets taken in one look.
const MOST_PER_LOOK: usize = 32;

/// Listens for game mods on [`MOD_PORT`], on this computer only, while a pad with these triggers
/// is connected. A mod's effects hold while the program it was sent from stays in front: once
/// the game closes or the player switches away, the Lab's come back, and the mod's return with the
/// game.
pub struct Mods {
    port: u16,
    socket: Option<std::net::UdpSocket>,
    tried: Option<std::time::Instant>,
    effects: [Option<Command>; 2],
    /// The program in front when the effects came.
    program: Option<String>,
    refused: bool,
}

/// What changed in a look, for the log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    pub listening: Option<bool>,
    /// The port was taken.
    pub refused: bool,
    pub packets: usize,
}

impl Mods {
    pub fn new(port: u16) -> Mods {
        Mods { port, socket: None, tried: None, effects: [None; 2], program: None, refused: false }
    }

    /// Opens or closes the port as `wanted` says, and takes in what mods sent. `free` is asked just
    /// before each try to open it, for whether no other program should have it.
    pub fn poll(&mut self, wanted: bool, foreground: &str, now: std::time::Instant, free: impl Fn() -> bool) -> Heard {
        let mut heard = Heard::default();
        if !wanted {
            if self.socket.take().is_some() {
                heard.listening = Some(false);
            }
            (self.effects, self.program, self.tried) = ([None; 2], None, None);
            return heard;
        }
        if self.socket.is_none() && self.tried.is_none_or(|t| now.duration_since(t) >= RETRY) {
            self.tried = Some(now);
            let bound = if free() {
                std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, self.port)).and_then(|s| s.set_nonblocking(true).map(|_| s))
            } else {
                Err(std::io::ErrorKind::AddrInUse.into())
            };
            match bound {
                Ok(s) => {
                    self.socket = Some(s);
                    self.refused = false;
                    heard.listening = Some(true);
                }
                // Told once, not at every try.
                Err(_) => {
                    heard.refused = !self.refused;
                    self.refused = true;
                }
            }
        }
        let Some(socket) = &self.socket else { return heard };
        let mut buf = [0u8; 2048];
        for _ in 0..MOST_PER_LOOK {
            let Ok((n, from)) = socket.recv_from(&mut buf) else { break };
            if !from.ip().is_loopback() {
                continue;
            }
            heard.packets += 1;
            let got = from_mod(&buf[..n]);
            if got.iter().any(Option::is_some) {
                if self.program.as_deref() != Some(foreground) {
                    self.effects = [None; 2];
                    self.program = Some(foreground.to_string());
                }
                for (e, g) in self.effects.iter_mut().zip(got) {
                    if g.is_some() {
                        *e = g;
                    }
                }
            }
        }
        heard
    }

    /// The mods' effects for the program now in front, left and right.
    pub fn effects(&self, foreground: &str) -> [Option<Command>; 2] {
        if self.program.as_deref() == Some(foreground) { self.effects } else { [None; 2] }
    }

    /// The port it listens on, once open.
    pub fn local_port(&self) -> Option<u16> {
        self.socket.as_ref().and_then(|s| s.local_addr().ok()).map(|a| a.port())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(mode: EffectMode, start: u8, end: u8, strength: u8, frequency: u8) -> TriggerEffect {
        TriggerEffect { mode, start, end, strength, frequency }
    }

    #[test]
    fn the_older_pads_report_as_flydigi_and_padforge_build_it() {
        let c = Command { mode: mode::RACE, params: [60, 200, 0, 0, 0] };
        let r = v1_report(RIGHT, c);
        assert_eq!(&r[..11], &[0x05, 0xA0, 0x01, 0x00, 0x02, 0x01, 60, 200, 0, 0, 0]);
        let sum = (0x05u32 + 0xA0 + 1 + 2 + 1 + 60 + 200) as u8;
        assert_eq!(r[11], sum, "FlydigiPcSpace's sum of the bytes before");
        assert_eq!(&r[12..], &[0, 0, 0]);
        assert_eq!(r[3], 0, "never saved into the pad's profile");
    }

    #[test]
    fn the_apex_5s_report_has_no_checksum() {
        let r = v2_report(LEFT, Command { mode: mode::RECOIL, params: [40, 80, 255, 0, 0] });
        assert_eq!(&r[..13], &[0x03, 0x5A, 0xA5, 0x51, 10, 0, 1, 3, 40, 80, 255, 0, 0]);
        assert!(r[13..].iter().all(|&b| b == 0));
    }

    #[test]
    fn a_race_from_the_start_goes_out_without_match_as_flydigi_sends_it() {
        let c = Command { mode: mode::RACE, params: [0, 100, 1, 0, 0] };
        assert_eq!(v1_report(LEFT, c)[8], 0);
        assert_eq!(v2_report(LEFT, c)[10], 0);
        // Only from the very start.
        assert_eq!(v1_report(LEFT, Command { params: [5, 100, 1, 0, 0], ..c })[8], 1);
    }

    #[test]
    fn lab_effects_become_flydigi_modes() {
        assert_eq!(Command::from_effect(TriggerEffect::default()), Command::OFF);
        assert_eq!(Command::from_effect(effect(EffectMode::Resistance, 3, 7, 0, 20)), Command::OFF, "no strength, no effect");
        let race = Command::from_effect(effect(EffectMode::Resistance, 0, 7, 8, 20));
        assert_eq!(race, Command { mode: mode::RACE, params: [0, 255, 0, 0, 0] });
        assert_eq!(Command::from_effect(effect(EffectMode::Resistance, 9, 9, 1, 20)).params[..2], [TRAVEL, 32]);
        let click = Command::from_effect(effect(EffectMode::Click, 2, 7, 4, 20));
        assert_eq!(click.mode, mode::RECOIL);
        assert_eq!(click.params, [42, 107, 128, 0, 0], "from zone 2 to zone 7, half strength");
        let buzz = Command::from_effect(effect(EffectMode::Vibration, 3, 7, 8, 35));
        assert_eq!(buzz, Command { mode: mode::VIBRATION, params: [64, 1, 255, 35, 0] });
    }

    #[test]
    fn every_lab_effect_stays_in_flydigis_ranges() {
        for m in [EffectMode::Off, EffectMode::Resistance, EffectMode::Vibration, EffectMode::Click] {
            for start in 0..=12 {
                for end in 0..=12 {
                    for strength in 0..=10 {
                        for frequency in [0, 1, 50, 100, 255] {
                            let c = Command::from_effect(effect(m, start, end, strength, frequency));
                            assert!(c.mode <= mode::VIBRATION);
                            if c.is_off() {
                                assert_eq!(c.params, [0; 5]);
                                continue;
                            }
                            assert!(c.params[0] <= TRAVEL, "{c:?}");
                            match c.mode {
                                mode::RACE => assert!(c.params[1] >= 1),
                                mode::RECOIL => assert!(
                                    c.params[1] >= 1
                                        && c.params[2] >= 1
                                        && u16::from(c.params[0]) + u16::from(c.params[1]) <= u16::from(TRAVEL)
                                ),
                                mode::VIBRATION => assert!(c.params[1..4].iter().all(|&p| p >= 1)),
                                _ => panic!("{c:?}"),
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mods_speak_flydigi_already() {
        let packet = br#"{"instructions":[
            {"type":1,"parameters":[0,1,0,1,40,200,0,0,0]},
            {"type":1,"parameters":[0,2,0,5,10,1,255,30,0]}]}"#;
        assert_eq!(
            from_mod(packet),
            [
                Some(Command { mode: mode::RACE, params: [40, 200, 0, 0, 0] }),
                Some(Command { mode: mode::VIBRATION, params: [10, 1, 255, 30, 0] })
            ]
        );
        // Floats that are whole numbers, as some JSON writers make them.
        let floats = br#"{"instructions":[{"type":1,"parameters":[0.0,2.0,0.0,0.0,0,0,0,0,0]}]}"#;
        assert_eq!(from_mod(floats), [None, Some(Command::OFF)]);
    }

    #[test]
    fn mod_packets_that_say_nothing_change_nothing() {
        let keep = br#"{"instructions":[{"type":1,"parameters":[0,1,0,255,1,2,3,4,5]}]}"#;
        let lights = br#"{"instructions":[{"type":2,"parameters":[0,255,0,0]}]}"#;
        let bad_side = br#"{"instructions":[{"type":1,"parameters":[0,3,0,1,1,1,1,1,1]}]}"#;
        let bad_mode = br#"{"instructions":[{"type":1,"parameters":[0,1,0,9,1,1,1,1,1]}]}"#;
        let too_big = br#"{"instructions":[{"type":1,"parameters":[0,1,0,1,999,1,1,1,1]}]}"#;
        let negative = br#"{"instructions":[{"type":1,"parameters":[0,-1,0,1,1,1,1,1,1]}]}"#;
        let short = br#"{"instructions":[{"type":1,"parameters":[0,1]}]}"#;
        for p in [&keep[..], lights, bad_side, bad_mode, negative, short, b"", b"not json", b"{}", b"[1,2]", br#"{"instructions":7}"#] {
            assert_eq!(from_mod(p), [None, None], "{}", String::from_utf8_lossy(p));
        }
        // A parameter out of range is taken as 0, as a missing one is: the mode still applies.
        assert_eq!(from_mod(too_big)[0], Some(Command { mode: mode::RACE, params: [0, 1, 1, 1, 1] }));
    }

    /// Sends `packet` to the listener and looks until it came, as loopback takes a moment.
    fn deliver(m: &mut Mods, packet: &[u8], foreground: &str) -> Heard {
        let port = m.local_port().unwrap();
        let out = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        out.send_to(packet, (std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        let start = std::time::Instant::now();
        loop {
            let h = m.poll(true, foreground, std::time::Instant::now(), || true);
            if h.packets > 0 || start.elapsed() > std::time::Duration::from_secs(2) {
                return h;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    const RACE_LEFT: &[u8] = br#"{"instructions":[{"type":1,"parameters":[0,1,0,1,40,200,0,0,0]}]}"#;
    const LOCK_RIGHT: &[u8] = br#"{"instructions":[{"type":1,"parameters":[0,2,0,4,90,255,0,0,0]}]}"#;

    #[test]
    fn a_mods_effects_hold_while_its_game_is_in_front() {
        let mut m = Mods::new(0);
        let now = std::time::Instant::now();
        assert_eq!(m.poll(true, "game.exe", now, || true).listening, Some(true));
        assert_eq!(deliver(&mut m, RACE_LEFT, "game.exe").packets, 1);
        deliver(&mut m, LOCK_RIGHT, "game.exe");
        let both = m.effects("game.exe");
        assert_eq!(both[0].map(|c| c.mode), Some(mode::RACE), "one packet a side, both kept");
        assert_eq!(both[1].map(|c| c.mode), Some(mode::LOCK));
        assert_eq!(m.effects("explorer.exe"), [None, None], "switched away: the Lab's come back");
        assert_eq!(m.effects("game.exe"), both, "and the mod's with the game");
        // Effects from another program replace the old game's.
        deliver(&mut m, LOCK_RIGHT, "other.exe");
        assert_eq!(m.effects("other.exe")[0], None);
        assert_eq!(m.effects("game.exe"), [None, None]);
        // No pad that takes them: the port closes and the effects go.
        assert_eq!(m.poll(false, "other.exe", now, || true).listening, Some(false));
        assert_eq!(m.local_port(), None);
        assert_eq!(m.effects("other.exe"), [None, None]);
    }

    #[test]
    fn a_taken_port_is_tried_again_later() {
        let holder = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = holder.local_addr().unwrap().port();
        let mut m = Mods::new(port);
        let now = std::time::Instant::now();
        assert!(m.poll(true, "", now, || true).refused);
        assert!(!m.poll(true, "", now + std::time::Duration::from_secs(1), || true).refused, "not every look");
        assert!(!m.poll(true, "", now + RETRY, || true).refused, "tried again, but told once");
        let now = now + RETRY;
        drop(holder);
        let later = m.poll(true, "", now + RETRY, || true);
        assert_eq!(later.listening, Some(true));
        assert_eq!(m.local_port(), Some(port));
    }

    #[test]
    fn the_port_is_left_to_flydigis_app_while_it_runs() {
        // Windows would let 127.0.0.1 be taken from it, so it is not even tried.
        let mut m = Mods::new(0);
        let now = std::time::Instant::now();
        let h = m.poll(true, "", now, || false);
        assert!(h.refused && h.listening.is_none());
        assert_eq!(m.local_port(), None);
        assert_eq!(m.poll(true, "", now + RETRY, || true).listening, Some(true), "once it is gone");
    }

    #[test]
    fn packets_that_are_not_effects_change_nothing() {
        let mut m = Mods::new(0);
        m.poll(true, "game.exe", std::time::Instant::now(), || true);
        assert_eq!(deliver(&mut m, b"hello", "game.exe").packets, 1);
        assert_eq!(m.effects("game.exe"), [None, None]);
    }

    #[test]
    fn junk_on_the_port_never_panics() {
        let mut x = 0x9e3779b97f4a7c15u64;
        for n in 0..3000 {
            let len = n % 300;
            let junk: Vec<u8> = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    b"{}[]\":,0123456789.-typeinstructionsparameters "[(x % 46) as usize]
                })
                .collect();
            from_mod(&junk);
        }
    }
}
