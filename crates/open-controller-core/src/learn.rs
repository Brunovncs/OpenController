//! Mapping a joystick SDL has no gamepad layout for, by its owner: each part of an Xbox
//! controller is asked for in turn, and what the joystick sends for it is named the way SDL's
//! mapping lines name it (`b3`, `h0.4`, `+a2`, `a1~`). The line made from them is kept in the
//! settings and given to SDL, which from then on reads the joystick as a gamepad.

use crate::mapping::{RawInput, axis, button};

/// How far an axis must move from where it rested to count as pressed or pushed.
const MOVED: i32 = 16000;
/// How close to where it rested an axis must come back to count as let go.
const BACK: i32 = 8000;
/// An axis resting this far out is a trigger read over its whole range.
const AT_END: i32 = 24000;

/// What a step takes: anything pressed, or a stick pushed, which only an axis can be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    Press,
    Push,
}

/// How the drawing shows the part asked for: a button held, or an axis pushed all the way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Button(u32),
    Axis(usize),
}

pub struct Step {
    /// The part's name in a mapping line.
    pub field: &'static str,
    pub want: Want,
    pub show: Show,
}

const fn press(field: &'static str, b: u32) -> Step {
    Step { field, want: Want::Press, show: Show::Button(b) }
}

/// Every part of an Xbox controller, in the order they are asked for. Sticks are pushed right
/// and down, the ways SDL's axes grow.
pub const STEPS: [Step; 21] = [
    press("a", button::SOUTH),
    press("b", button::EAST),
    press("x", button::WEST),
    press("y", button::NORTH),
    press("dpup", button::DPAD_UP),
    press("dpright", button::DPAD_RIGHT),
    press("dpdown", button::DPAD_DOWN),
    press("dpleft", button::DPAD_LEFT),
    press("leftshoulder", button::LEFT_SHOULDER),
    press("rightshoulder", button::RIGHT_SHOULDER),
    Step { field: "lefttrigger", want: Want::Press, show: Show::Axis(axis::LEFT_TRIGGER) },
    Step { field: "righttrigger", want: Want::Press, show: Show::Axis(axis::RIGHT_TRIGGER) },
    Step { field: "leftx", want: Want::Push, show: Show::Axis(axis::LEFT_X) },
    Step { field: "lefty", want: Want::Push, show: Show::Axis(axis::LEFT_Y) },
    Step { field: "rightx", want: Want::Push, show: Show::Axis(axis::RIGHT_X) },
    Step { field: "righty", want: Want::Push, show: Show::Axis(axis::RIGHT_Y) },
    press("leftstick", button::LEFT_STICK),
    press("rightstick", button::RIGHT_STICK),
    press("back", button::BACK),
    press("start", button::START),
    press("guide", button::GUIDE),
];

/// Which part of an axis a part reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Whole,
    Plus,
    Minus,
}

/// One thing a joystick sends, by its own numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Button(u8),
    /// A hat and one direction: up 1, right 2, down 4, left 8.
    Hat(u8, u8),
    Axis {
        index: u8,
        half: Half,
        inverted: bool,
    },
}

impl Input {
    /// Whether two parts would read the same thing. The two halves of one axis are two things: a
    /// d-pad sent as an axis is left and right on it.
    fn overlaps(self, other: Input) -> bool {
        match (self, other) {
            (Input::Axis { index: a, half: h, .. }, Input::Axis { index: b, half: k, .. }) => {
                a == b && (h == Half::Whole || k == Half::Whole || h == k)
            }
            _ => self == other,
        }
    }
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Input::Button(b) => write!(f, "b{b}"),
            Input::Hat(h, d) => write!(f, "h{h}.{d}"),
            Input::Axis { index, half: Half::Whole, inverted } => write!(f, "a{index}{}", if inverted { "~" } else { "" }),
            Input::Axis { index, half: Half::Plus, .. } => write!(f, "+a{index}"),
            Input::Axis { index, half: Half::Minus, .. } => write!(f, "-a{index}"),
        }
    }
}

fn axis_at(raw: &RawInput, i: usize) -> i32 {
    raw.axes.get(i).copied().map_or(0, i32::from)
}

fn hat_at(raw: &RawInput, i: usize) -> u8 {
    raw.hats.get(i).copied().unwrap_or(0)
}

/// What the joystick sends now that it did not at rest, for a step that wants `want`, leaving
/// out what another part already reads. A diagonal on a hat waits for one direction.
pub fn detect(rest: &RawInput, now: &RawInput, want: Want, used: &[Input]) -> Option<Input> {
    let free = |i: &Input| !used.iter().any(|u| u.overlaps(*i));
    let mut found: Vec<Input> = Vec::new();
    if want == Want::Press {
        let new = now.buttons & !rest.buttons;
        found.extend((0..64u8).filter(|&b| new & 1 << b != 0).map(Input::Button));
        for h in 0..now.hats.len() {
            let new = hat_at(now, h) & !hat_at(rest, h);
            if matches!(new, 1 | 2 | 4 | 8) {
                found.push(Input::Hat(h as u8, new));
            }
        }
    }
    if let Some(i) = found.into_iter().find(free) {
        return Some(i);
    }
    // The axis that moved the most.
    let mut axes: Vec<(usize, i32, i32)> = (0..now.axes.len())
        .map(|i| (i, axis_at(rest, i), axis_at(now, i) - axis_at(rest, i)))
        .filter(|&(_, _, moved)| moved.abs() > MOVED)
        .collect();
    axes.sort_by_key(|&(_, _, moved)| std::cmp::Reverse(moved.abs()));
    axes.into_iter()
        .map(|(i, at, moved)| {
            let index = i as u8;
            match want {
                Want::Push => Input::Axis { index, half: Half::Whole, inverted: moved < 0 },
                // A trigger read over the whole range rests at one end.
                Want::Press if at.abs() > AT_END => Input::Axis { index, half: Half::Whole, inverted: at > 0 },
                Want::Press => Input::Axis { index, half: if moved > 0 { Half::Plus } else { Half::Minus }, inverted: false },
            }
        })
        .find(free)
}

/// Whether everything is back where it rested.
pub fn let_go(rest: &RawInput, now: &RawInput) -> bool {
    now.buttons & !rest.buttons == 0
        && (0..now.hats.len()).all(|h| hat_at(now, h) & !hat_at(rest, h) == 0)
        && (0..now.axes.len()).all(|i| (axis_at(now, i) - axis_at(rest, i)).abs() < BACK)
}

/// A mapping in progress: where the joystick rested when it began, and what each step got.
#[derive(Clone, Debug)]
pub struct Learning {
    rest: RawInput,
    step: usize,
    bound: [Option<Input>; STEPS.len()],
    /// What the last step got is still held: the next one waits for it to be let go.
    held: bool,
}

impl Learning {
    /// Starts with the joystick as it is now, left alone.
    pub fn new(rest: RawInput) -> Learning {
        Learning { rest, step: 0, bound: [None; STEPS.len()], held: false }
    }

    /// The step being asked for, by its place in [`STEPS`]; `STEPS.len()` once every one was.
    pub fn step(&self) -> usize {
        self.step
    }

    pub fn done(&self) -> bool {
        self.step == STEPS.len()
    }

    /// Whether what the last step got is still held.
    pub fn held(&self) -> bool {
        self.held
    }

    /// What a step got, if it got anything.
    pub fn bound(&self, step: usize) -> Option<Input> {
        self.bound.get(step).copied().flatten()
    }

    /// Takes in what the joystick sends now. Returns whether the step moved on or let go.
    pub fn feed(&mut self, now: &RawInput) -> bool {
        if self.done() {
            return false;
        }
        if self.held {
            self.held = !let_go(&self.rest, now);
            return !self.held;
        }
        let used: Vec<Input> = self.bound.iter().flatten().copied().collect();
        let Some(input) = detect(&self.rest, now, STEPS[self.step].want, &used) else { return false };
        self.bound[self.step] = Some(input);
        self.step += 1;
        self.held = true;
        true
    }

    /// The controller has no such part.
    pub fn skip(&mut self) {
        if !self.done() {
            self.bound[self.step] = None;
            self.step += 1;
        }
    }

    /// Asks for the step before again.
    pub fn back(&mut self) {
        if self.step > 0 {
            self.step -= 1;
            self.bound[self.step] = None;
            self.held = false;
        }
    }

    /// Whether any part got something: a mapping without one is no mapping.
    pub fn any(&self) -> bool {
        self.bound.iter().any(Option::is_some)
    }

    /// The parts as a mapping line has them, `a:b2,b:b1,` and so on.
    pub fn fields(&self) -> String {
        STEPS.iter().zip(&self.bound).filter_map(|(s, b)| b.map(|b| format!("{}:{b},", s.field))).collect()
    }
}

/// The system's name as mapping lines give it.
pub const PLATFORM: &str = if cfg!(windows) {
    "Windows"
} else if cfg!(target_os = "macos") {
    "Mac OS X"
} else {
    "Linux"
};

/// A mapping line for SDL: the joystick's GUID, a name, the parts and the system. `None` when
/// the GUID or the parts are not what a line takes, so nothing sent over the pipe can make one
/// SDL would misread.
pub fn line(guid: &str, name: &str, fields: &str) -> Option<String> {
    let guid_ok = guid.len() == 32 && guid.bytes().all(|b| b.is_ascii_hexdigit());
    let fields_ok = !fields.is_empty()
        && fields.ends_with(',')
        && fields.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+-~.:,".contains(&b))
        && fields.split_terminator(',').all(|f| STEPS.iter().any(|s| f.strip_prefix(s.field).is_some_and(|r| r.starts_with(':'))));
    if !guid_ok || !fields_ok {
        return None;
    }
    let name: String = name.chars().map(|c| if c == ',' || c.is_control() { ' ' } else { c }).collect();
    let name = name.trim();
    let name = if name.is_empty() { "Controller" } else { name };
    Some(format!("{guid},{name},{fields}platform:{PLATFORM},"))
}

/// The GUID a mapping line is for.
pub fn guid_of(line: &str) -> &str {
    line.split(',').next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(buttons: u64, axes: &[i16], hats: &[u8]) -> RawInput {
        RawInput { buttons, axes: axes.to_vec(), hats: hats.to_vec() }
    }

    #[test]
    fn a_press_is_named_as_mapping_lines_name_it() {
        let rest = raw(0, &[0, 0, -32768, 0], &[0]);
        assert_eq!(detect(&rest, &raw(1 << 2, &[0, 0, -32768, 0], &[0]), Want::Press, &[]).map(|i| i.to_string()), Some("b2".into()));
        assert_eq!(detect(&rest, &raw(0, &[0, 0, -32768, 0], &[4]), Want::Press, &[]).map(|i| i.to_string()), Some("h0.4".into()));
        // A diagonal waits for one direction.
        assert_eq!(detect(&rest, &raw(0, &[0, 0, -32768, 0], &[3]), Want::Press, &[]), None);
        // A trigger resting at one end is read whole; an axis resting in the middle by half.
        assert_eq!(detect(&rest, &raw(0, &[0, 0, 32767, 0], &[0]), Want::Press, &[]).map(|i| i.to_string()), Some("a2".into()));
        assert_eq!(detect(&rest, &raw(0, &[-32768, 0, -32768, 0], &[0]), Want::Press, &[]).map(|i| i.to_string()), Some("-a0".into()));
        assert_eq!(detect(&rest, &raw(0, &[0, 0, -32768, 30000], &[0]), Want::Press, &[]).map(|i| i.to_string()), Some("+a3".into()));
        // Small moves are not presses.
        assert_eq!(detect(&rest, &raw(0, &[9000, -9000, -32768, 0], &[0]), Want::Press, &[]), None);
    }

    #[test]
    fn a_stick_is_an_axis_pushed_either_way() {
        let rest = raw(0, &[128, -128, 0, 0], &[0]);
        let now = raw(1, &[32767, -128, 0, 0], &[2]);
        assert_eq!(detect(&rest, &now, Want::Push, &[]).map(|i| i.to_string()), Some("a0".into()));
        let now = raw(0, &[128, -128, 0, -32768], &[0]);
        assert_eq!(detect(&rest, &now, Want::Push, &[]).map(|i| i.to_string()), Some("a3~".into()));
        // Buttons and hats are no stick.
        assert_eq!(detect(&rest, &raw(1, &[128, -128, 0, 0], &[2]), Want::Push, &[]), None);
    }

    #[test]
    fn what_a_part_reads_is_not_given_to_another() {
        let rest = raw(0, &[0, 0], &[0]);
        let used = [Input::Button(0), Input::Axis { index: 0, half: Half::Minus, inverted: false }];
        assert_eq!(detect(&rest, &raw(1, &[0, 0], &[0]), Want::Press, &used), None);
        assert_eq!(detect(&rest, &raw(0b11, &[0, 0], &[0]), Want::Press, &used), Some(Input::Button(1)));
        // The other half of a d-pad sent as an axis is free; the whole axis is not.
        assert_eq!(detect(&rest, &raw(0, &[32767, 0], &[0]), Want::Press, &used).map(|i| i.to_string()), Some("+a0".into()));
        assert_eq!(detect(&rest, &raw(0, &[32767, 0], &[0]), Want::Push, &used), None);
    }

    #[test]
    fn each_step_waits_for_the_last_one_to_be_let_go() {
        let rest = raw(0, &[0, 0, 0, 0], &[0]);
        let mut l = Learning::new(rest.clone());
        assert!(l.feed(&raw(1 << 2, &[0, 0, 0, 0], &[0])));
        assert_eq!(l.step(), 1);
        // Still held: B does not get it too, nor anything pressed with it.
        assert!(!l.feed(&raw(1 << 2 | 1 << 1, &[0, 0, 0, 0], &[0])));
        assert!(l.held());
        assert!(l.feed(&rest));
        assert!(!l.held());
        assert!(l.feed(&raw(1 << 1, &[0, 0, 0, 0], &[0])));
        l.feed(&rest);
        l.skip();
        assert_eq!(l.step(), 3);
        l.back();
        assert_eq!((l.step(), l.bound(2)), (2, None));
        assert_eq!(l.fields(), "a:b2,b:b1,");
    }

    #[test]
    fn a_whole_mapping_makes_a_line_sdl_takes() {
        let rest = raw(0, &[0, 0, 0, 0], &[0]);
        let mut l = Learning::new(rest.clone());
        let mut press = |now: RawInput| {
            assert!(l.feed(&now));
            l.feed(&rest);
        };
        for b in [2, 1, 3, 0] {
            press(raw(1 << b, &[0, 0, 0, 0], &[0]));
        }
        for d in [1, 2, 4, 8] {
            press(raw(0, &[0, 0, 0, 0], &[d]));
        }
        for b in [4, 5, 6, 7] {
            press(raw(1 << b, &[0, 0, 0, 0], &[0]));
        }
        press(raw(0, &[32767, 0, 0, 0], &[0]));
        press(raw(0, &[0, 32767, 0, 0], &[0]));
        press(raw(0, &[0, 0, 32767, 0], &[0]));
        press(raw(0, &[0, 0, 0, 32767], &[0]));
        for b in [10, 11, 8, 9] {
            press(raw(1 << b, &[0, 0, 0, 0], &[0]));
        }
        l.skip();
        assert!(l.done());
        let line = line("0300be79790000000600000000000000", "Microntek, USB Joystick", &l.fields()).unwrap();
        assert_eq!(
            line,
            format!(
                "0300be79790000000600000000000000,Microntek  USB Joystick,a:b2,b:b1,x:b3,y:b0,dpup:h0.1,dpright:h0.2,dpdown:h0.4,dpleft:h0.8,\
                 leftshoulder:b4,rightshoulder:b5,lefttrigger:b6,righttrigger:b7,leftx:a0,lefty:a1,rightx:a2,righty:a3,leftstick:b10,\
                 rightstick:b11,back:b8,start:b9,platform:{PLATFORM},"
            )
        );
        assert_eq!(guid_of(&line), "0300be79790000000600000000000000");
    }

    #[test]
    fn a_line_takes_only_what_a_mapping_has() {
        let guid = "0300be79790000000600000000000000";
        assert!(line(guid, "x", "a:b0,").is_some());
        assert!(line("0300be79", "x", "a:b0,").is_none());
        assert!(line(guid, "x", "").is_none());
        assert!(line(guid, "x", "a:b0").is_none());
        assert!(line(guid, "x", "a:b0,platform:Linux,").is_none());
        assert!(line(guid, "x", "a:b0,\nxinput,").is_none());
        assert!(line(guid, "x", "turbo:b0,").is_none());
    }
}
