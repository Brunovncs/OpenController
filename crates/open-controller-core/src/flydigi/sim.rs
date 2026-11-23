//! A Flydigi Apex 2 and SDL 3.4.18's reading of it, simulated, to try the fix without the pad.
//!
//! [`Pad`] builds the pad's report from what is held, by the layout in the module above. [`SdlV1`]
//! is SDL ported line by line: `HIDAPI_DriverFlydigi_HandleStatePacketV1` (SDL_hidapi_flydigi.c),
//! the first-value rules of `SDL_SendJoystickAxis` (SDL_joystick.c), and `SDL_GetGamepadAxis`
//! with the mapping SDL made for the owner's pad. The first test holds the two to the report the
//! owner sent, byte for byte and value for value, before the rest trust them.

use super::*;
use crate::extras::{LEFT_PADDLE1, LEFT_PADDLE2, RIGHT_PADDLE1, RIGHT_PADDLE2};
use crate::mapping::{HidBytes, button, to_xusb};

/// What is held on the pad. `m` is M1 to M6.
#[derive(Clone, Copy, Debug)]
struct Pad {
    a: bool,
    b: bool,
    x: bool,
    y: bool,
    select: bool,
    start: bool,
    lb: bool,
    rb: bool,
    lt: bool,
    rt: bool,
    ls: bool,
    rs: bool,
    home: bool,
    pair: bool,
    ret: bool,
    c: bool,
    z: bool,
    m: [bool; 6],
    /// Up 1, right 2, down 4, left 8.
    dpad: u8,
    left: (u8, u8),
    right: (u8, u8),
    /// How far the ABXY wheel is dragged; it rests near the middle.
    wheel: (u8, u8),
    /// The air mouse's yaw and pitch, 12 bits each.
    air: (i16, i16),
}

/// The pad at rest, its sticks and wheel where the owner's were.
const REST: Pad = Pad {
    a: false,
    b: false,
    x: false,
    y: false,
    select: false,
    start: false,
    lb: false,
    rb: false,
    lt: false,
    rt: false,
    ls: false,
    rs: false,
    home: false,
    pair: false,
    ret: false,
    c: false,
    z: false,
    m: [false; 6],
    dpad: 0,
    left: (0x84, 0x7f),
    right: (0x7e, 0x7f),
    wheel: (0x81, 0x85),
    air: (0, 0),
};

impl Pad {
    fn report(&self) -> [u8; 32] {
        let on = |held: bool, bit: u8| if held { bit } else { 0 };
        let mut r = [0u8; 32];
        (r[0], r[1], r[2], r[28], r[29], r[30]) = (0x04, 0xfe, 0x66, 0x01, 0x04, 0x36);
        let (yaw, pitch) = (self.air.0 as u16 & 0xfff, self.air.1 as u16 & 0xfff);
        r[4] = yaw as u8;
        r[5] = (yaw >> 8) as u8 | ((pitch & 0x0f) as u8) << 4;
        r[6] = (pitch >> 4) as u8;
        r[7] = self.m.iter().enumerate().fold(on(self.c, 0x01) | on(self.z, 0x02), |b, (i, &held)| b | on(held, 0x04 << i));
        r[8] = on(self.pair, 0x01) | on(self.home, 0x08) | on(self.ret, 0x10);
        r[9] = self.dpad & 0x0f | on(self.a, 0x10) | on(self.b, 0x20) | on(self.select, 0x40) | on(self.x, 0x80);
        r[10] = on(self.y, 0x01)
            | on(self.start, 0x02)
            | on(self.lb, 0x04)
            | on(self.rb, 0x08)
            | on(self.lt, 0x10)
            | on(self.rt, 0x20)
            | on(self.ls, 0x40)
            | on(self.rs, 0x80);
        (r[17], r[19], r[21], r[22], r[23], r[24]) = (self.left.0, self.left.1, self.right.0, self.right.1, self.wheel.0, self.wheel.1);
        r
    }
}

#[derive(Clone, Copy, Debug)]
struct Axis {
    value: i32,
    initial: i32,
    has_second: bool,
    sent_initial: bool,
}

/// SDL's joystick for the pad: 6 axes, 15 buttons (no C and Z for an Apex 2) and a hat.
struct SdlV1 {
    axes: [Axis; 6],
    buttons: [bool; 15],
    hat: u8,
    last: [u8; 32],
    /// Whether SDL would send `SDL_EVENT_GAMEPAD_UPDATE_COMPLETE` for the last report.
    updated: bool,
}

const AXIS_MAX: i32 = 32767;

impl SdlV1 {
    /// As `SDL_OpenJoystick` leaves a HIDAPI joystick: the axes known, the triggers released.
    fn new() -> SdlV1 {
        let axis = |i: usize| {
            let v = if i >= 4 { -32768 } else { 0 };
            Axis { value: v, initial: v, has_second: false, sent_initial: false }
        };
        SdlV1 { axes: std::array::from_fn(axis), buttons: [false; 15], hat: 0, last: [0; 32], updated: false }
    }

    /// `SDL_SendJoystickAxis`, without the focus rule (background events are allowed).
    fn axis(&mut self, i: usize, value: i32) {
        let a = &mut self.axes[i];
        if !a.has_second && (a.initial <= -32767 || a.initial == 32767) && value.abs() < AXIS_MAX / 4 {
            (a.initial, a.value) = (value, value);
        } else if value == a.value {
            return;
        } else {
            a.has_second = true;
        }
        if !a.sent_initial {
            if (value - a.value).abs() <= AXIS_MAX / 80 {
                return;
            }
            a.sent_initial = true;
        }
        a.value = value;
        self.updated = true;
    }

    fn button(&mut self, i: usize, on: bool) {
        if self.buttons[i] != on {
            self.buttons[i] = on;
            self.updated = true;
        }
    }

    /// `HIDAPI_DriverFlydigi_HandlePacketV1` for a pad without C and Z or sensors.
    fn feed(&mut self, d: &[u8]) {
        self.updated = false;
        if d.len() < 32 || d[0] != 0x04 || d[1] != 0xfe {
            return;
        }
        if self.last[9] != d[9] {
            let hat = match d[9] & 0x0f {
                h @ (0x01 | 0x03 | 0x02 | 0x06 | 0x04 | 0x0c | 0x08 | 0x09) => h,
                _ => 0,
            };
            if hat != self.hat {
                (self.hat, self.updated) = (hat, true);
            }
            for (i, bit) in [(0, 0x10), (1, 0x20), (4, 0x40), (2, 0x80)] {
                self.button(i, d[9] & bit != 0);
            }
        }
        if self.last[10] != d[10] {
            for (i, bit) in [(3, 0x01), (6, 0x02), (9, 0x04), (10, 0x08), (7, 0x40), (8, 0x80)] {
                self.button(i, d[10] & bit != 0);
            }
        }
        if self.last[7] != d[7] {
            for (i, bit) in [(11, 0x04), (12, 0x08), (13, 0x10), (14, 0x20)] {
                self.button(i, d[7] & bit != 0);
            }
        }
        if self.last[8] != d[8] {
            self.button(5, d[8] & 0x08 != 0);
        }
        let stick = |b: u8| -> i32 {
            if b == 0x7f {
                0
            } else {
                (-32768.0f32 + (32767.0f32 - -32768.0) * ((b as f32 - 127.0) - -127.0) / (128.0 - -127.0)) as i16 as i32
            }
        };
        for (i, at) in [(0, 17), (1, 19), (2, 21), (3, 22)] {
            self.axis(i, stick(d[at]));
        }
        self.axis(4, d[23] as i32 * 257 - 32768);
        self.axis(5, d[24] as i32 * 257 - 32768);
        self.last.copy_from_slice(&d[..32]);
    }

    /// `SDL_GetGamepadButton` and `SDL_GetGamepadAxis` through the owner's mapping: a:b0, b:b1,
    /// x:b2, y:b3, back:b4, guide:b5, start:b6, the stick clicks b7 and b8, the bumpers b9 and
    /// b10, paddle1 to paddle4 b11 to b14, the d-pad on hat 0, and the axes a0 to a5 in order.
    fn gamepad(&self) -> PadState {
        let mut s = PadState::default();
        for (i, &held) in self.buttons.iter().enumerate().take(11) {
            s.buttons |= u64::from(held) << i;
        }
        for (i, b) in [RIGHT_PADDLE1, LEFT_PADDLE1, RIGHT_PADDLE2, LEFT_PADDLE2].into_iter().enumerate() {
            s.buttons |= u64::from(self.buttons[11 + i]) << b;
        }
        for (bit, b) in [(1, button::DPAD_UP), (4, button::DPAD_DOWN), (8, button::DPAD_LEFT), (2, button::DPAD_RIGHT)] {
            s.buttons |= u64::from(self.hat & bit != 0) << b;
        }
        for i in 0..4 {
            s.axes[i] = self.axes[i].value as i16;
        }
        for i in 4..6 {
            let normalized = (self.axes[i].value + 32768) as f32 / 65535.0;
            s.axes[i] = (normalized * AXIS_MAX as f32) as i32 as i16;
        }
        s
    }
}

/// The pad through SDL and through the fix, report by report, as the engine sees it.
struct Run {
    sdl: SdlV1,
    ours: Apex2,
    hid: HidBytes,
    /// Buttons ever held and each axis's range, as SDL alone gives them (the report's "Input").
    seen: u64,
    ranges: [(i16, i16); 6],
}

impl Run {
    fn new() -> Run {
        Run { sdl: SdlV1::new(), ours: Apex2::default(), hid: HidBytes::default(), seen: 0, ranges: [(i16::MAX, i16::MIN); 6] }
    }

    /// Takes one report. Whether SDL, and the fix, would have the engine look again.
    fn feed(&mut self, r: &[u8]) -> (bool, bool) {
        self.hid.feed(r);
        self.sdl.feed(r);
        let before = self.ours;
        self.ours = self.ours.after(r);
        let s = self.sdl.gamepad();
        self.seen |= s.buttons;
        for (range, &v) in self.ranges.iter_mut().zip(&s.axes) {
            *range = (range.0.min(v), range.1.max(v));
        }
        (self.sdl.updated, self.ours != before)
    }

    fn hold(&mut self, p: Pad) -> (bool, bool) {
        self.feed(&p.report())
    }

    /// What reaches games: SDL's state with the fix applied.
    fn fixed(&self) -> PadState {
        let mut s = self.sdl.gamepad();
        self.ours.apply(&mut s);
        s
    }
}

/// What the owner did while the report was open, as far as the report shows: every face button,
/// Back and Start, the bumpers, M1 to M4, C and Z one at a time, both triggers together, the
/// sticks all the way round, and the pad moved about.
fn owners_session() -> Vec<Pad> {
    let mut s = vec![REST];
    let mut press = |p: Pad| {
        s.push(p);
        s.push(REST);
    };
    for p in [
        Pad { a: true, ..REST },
        Pad { b: true, ..REST },
        Pad { x: true, ..REST },
        Pad { y: true, ..REST },
        Pad { select: true, ..REST },
        Pad { start: true, ..REST },
        Pad { lb: true, ..REST },
        Pad { rb: true, ..REST },
        Pad { lt: true, rt: true, ..REST },
        Pad { c: true, ..REST },
        Pad { z: true, ..REST },
    ] {
        press(p);
    }
    for i in 0..4 {
        let mut m = [false; 6];
        m[i] = true;
        press(Pad { m, ..REST });
    }
    // Each stick axis swept from rest to one end, across to the other and back, a step at a time.
    let sweep = |rest: u8, low: u8, high: u8| (low..rest).rev().chain(low..=high).chain((rest..high).rev()).collect::<Vec<u8>>();
    for v in sweep(REST.left.0, 4, 254) {
        s.push(Pad { left: (v, REST.left.1), ..REST });
    }
    for v in sweep(REST.left.1, 1, 254) {
        s.push(Pad { left: (REST.left.0, v), ..REST });
    }
    for v in sweep(REST.right.0, 1, 254) {
        s.push(Pad { right: (v, REST.right.1), ..REST });
    }
    for v in sweep(REST.right.1, 1, 254) {
        s.push(Pad { right: (REST.right.0, v), ..REST });
    }
    s.push(REST);
    let mut press = |p: Pad| {
        s.push(p);
        s.push(REST);
    };
    for air in [(-1, 0), (0, -1), (-1, -1), (5, -3)] {
        press(Pad { air, ..REST });
    }
    s
}

/// The owner's report, "The controller's own reports".
const OWNERS_REST: &str = "04 fe 66 00 00 00 00 00 00 00 00 00 00 00 00 00 00 84 00 7f 00 7e 7f 81 85 00 00 00 01 04 36 00";
const OWNERS_MOVED: &str = "byte 4 0 to 255 (bits ff), byte 5 0 to 255 (bits ff), byte 6 0 to 255 (bits ff), byte 7 0 to 32 (bits 3f), \
byte 9 0 to 128 (bits f0), byte 10 0 to 48 (bits 3f), byte 17 4 to 254 (bits ff), byte 19 1 to 254 (bits ff), \
byte 21 1 to 254 (bits ff), byte 22 1 to 254 (bits ff)";

#[test]
fn the_simulation_gives_back_the_owners_report() {
    let mut run = Run::new();
    for p in owners_session() {
        run.hold(p);
    }
    let r = &run.hid.reports[0];
    let rest: Vec<String> = r.first.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(rest.join(" "), OWNERS_REST);
    let moved: Vec<String> = (0..r.first.len())
        .filter(|&i| r.flipped[i] != 0)
        .map(|i| format!("byte {i} {} to {} (bits {:02x})", r.low[i], r.high[i], r.flipped[i]))
        .collect();
    assert_eq!(moved.join(", "), OWNERS_MOVED);

    // "Input while this report was open": the same buttons, and no trigger.
    let expected = [
        button::SOUTH,
        button::EAST,
        button::WEST,
        button::NORTH,
        button::BACK,
        button::START,
        button::LEFT_SHOULDER,
        button::RIGHT_SHOULDER,
        u32::from(RIGHT_PADDLE1),
        u32::from(LEFT_PADDLE1),
        u32::from(RIGHT_PADDLE2),
        u32::from(LEFT_PADDLE2),
    ]
    .iter()
    .fold(0u64, |b, &i| b | 1 << i);
    assert_eq!(run.seen, expected);
    // "left x: -31740 to 32510", and -32511 to 32510 for the other three.
    assert_eq!(&run.ranges[..4], &[(-31740, 32510), (-32511, 32510), (-32511, 32510), (-32511, 32510)]);
    // The triggers never moved: held about half way the whole time, the blue on the drawing.
    assert_eq!((run.ranges[4], run.ranges[5]), ((16576, 16576), (17090, 17090)));
}

#[test]
fn games_got_half_pressed_triggers_and_now_get_them_released() {
    let mut run = Run::new();
    run.hold(REST);
    let before = to_xusb(&run.sdl.gamepad());
    assert_eq!((before.left_trigger, before.right_trigger), (129, 133), "what a game saw at rest until now");
    let after = to_xusb(&run.fixed());
    assert_eq!((after.left_trigger, after.right_trigger), (0, 0));
}

#[test]
fn the_triggers_reach_games_and_alone_wake_the_engine() {
    let mut run = Run::new();
    run.hold(REST);
    for (pad, lt, rt) in
        [(Pad { lt: true, ..REST }, 255, 0), (Pad { rt: true, ..REST }, 0, 255), (Pad { lt: true, rt: true, ..REST }, 255, 255)]
    {
        let (sdl_noticed, we_noticed) = run.hold(pad);
        assert!(!sdl_noticed, "SDL reads nothing that changed, so it sends no update");
        assert!(we_noticed, "so the engine has to be woken by the pad's own report");
        let x = to_xusb(&run.fixed());
        assert_eq!((x.left_trigger, x.right_trigger), (lt, rt), "{pad:?}");
        assert_eq!(to_xusb(&run.sdl.gamepad()).left_trigger, 129, "SDL alone still has it half way");
        run.hold(REST);
        let x = to_xusb(&run.fixed());
        assert_eq!((x.left_trigger, x.right_trigger), (0, 0));
    }
}

#[test]
fn every_extra_button_arrives_on_its_own_bit() {
    let mut m5 = [false; 6];
    m5[4] = true;
    let mut m6 = [false; 6];
    m6[5] = true;
    let cases = [
        (Pad { c: true, ..REST }, MISC2, "C"),
        (Pad { z: true, ..REST }, MISC3, "Z"),
        (Pad { m: m6, ..REST }, MISC4, "M6"),
        (Pad { m: m5, ..REST }, MISC5, "M5"),
        (Pad { ret: true, ..REST }, MISC6, "Return"),
        (Pad { pair: true, ..REST }, MISC1, ")))"),
    ];
    let mut run = Run::new();
    run.hold(REST);
    for (pad, b, name) in cases {
        let (sdl_noticed, we_noticed) = run.hold(pad);
        assert!(!sdl_noticed && we_noticed, "{name}: SDL reads none of them");
        assert_eq!(run.fixed().buttons, 1 << b, "{name}");
        assert_eq!(crate::extras::printed_name(crate::extras::Family::FlydigiApex2, crate::extras::Art::Offset, b), Some(name));
        run.hold(REST);
        assert_eq!(run.fixed().buttons, 0, "{name} let go");
    }
}

#[test]
fn sdls_own_buttons_are_left_alone() {
    let mut m = [false; 6];
    m[0] = true;
    let mut run = Run::new();
    run.hold(REST);
    run.hold(Pad { a: true, home: true, m, dpad: 0x03, lt: true, c: true, ..REST });
    let s = run.fixed();
    let want = [button::SOUTH, button::GUIDE, u32::from(RIGHT_PADDLE1), button::DPAD_UP, button::DPAD_RIGHT, u32::from(MISC2)];
    assert_eq!(s.buttons, want.iter().fold(0u64, |b, &i| b | 1 << i));
    assert_eq!(s.axes[axis::LEFT_TRIGGER], i16::MAX);
    let x = to_xusb(&s);
    assert_eq!(
        x.buttons,
        crate::mapping::xusb::A | crate::mapping::xusb::GUIDE | crate::mapping::xusb::DPAD_UP | crate::mapping::xusb::DPAD_RIGHT
    );
}

#[test]
fn dragging_the_wheel_no_longer_pulls_the_triggers() {
    let mut run = Run::new();
    run.hold(REST);
    for wheel in [(0xff, 0x85), (0x81, 0xff), (0x00, 0x00), (0xff, 0xff)] {
        run.hold(Pad { wheel, b: true, ..REST });
        let sdl = to_xusb(&run.sdl.gamepad());
        assert_ne!((sdl.left_trigger, sdl.right_trigger), (129, 133), "SDL took the drag for the triggers");
        let x = to_xusb(&run.fixed());
        assert_eq!((x.left_trigger, x.right_trigger), (0, 0), "{wheel:?}");
    }
}

#[test]
fn replies_and_motion_change_nothing() {
    let mut run = Run::new();
    run.hold(Pad { lt: true, c: true, ..REST });
    let held = run.fixed();
    // A reply to a question about the model, as Flydigi's app or SDL may ask while it runs.
    let mut reply = [0u8; 32];
    (reply[0], reply[1], reply[2], reply[3], reply[15]) = (0x04, 0xff, 0xf0, 0x13, 0xec);
    assert_eq!(run.feed(&reply), (false, false));
    assert_eq!(run.fixed(), held);
    for air in [(2047, -2048), (-1, 1), (0, 0)] {
        let (_, we_noticed) = run.hold(Pad { lt: true, c: true, air, ..REST });
        assert!(!we_noticed, "{air:?}");
    }
    assert_eq!(run.fixed(), held);
}
