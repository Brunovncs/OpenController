//! A controller's profiles: named sets of what its extra buttons do, what its light shows, how
//! its sticks and gyro behave, and which programs switch to them. One is in use; a profile that
//! names the program in front takes over while that program is in front.

use crate::binding::{Action, Bindings};
use serde::{Deserialize, Serialize};

pub const MAX_PROFILES: usize = 8;
pub const MAX_NAME: usize = 32;
/// Programs a profile can name, and how long each name may be.
pub const MAX_PROGRAMS: usize = 16;
pub const MAX_PROGRAM: usize = 64;

/// What a light bar shows (DualShock 4, DualSense and others SDL can colour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Light {
    /// The player's colour, as the console does: blue, red, green, pink.
    #[default]
    Player,
    Color([u8; 3]),
    /// Green when full to red when nearly empty.
    Battery,
    Off,
}

/// When the gyro moves the right stick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GyroMode {
    #[default]
    Off,
    Always,
    /// While the left trigger is past halfway, as when aiming down sights.
    Aiming,
    /// While this button (an SDL gamepad button index) is held.
    Holding(u8),
}

/// Aiming by tilting the controller: its rotation added to the right stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gyro {
    pub mode: GyroMode,
    /// Turning speed in percent: at 100, turning the controller 180 degrees a second pushes the
    /// stick all the way.
    pub sensitivity: u16,
    pub invert_y: bool,
}

impl Default for Gyro {
    fn default() -> Self {
        Gyro { mode: GyroMode::Off, sensitivity: 100, invert_y: false }
    }
}

/// A radial deadzone for worn sticks that drift, and an anti-deadzone for games whose own
/// deadzone swallows small movements. Both in percent of the stick's travel, both off by default:
/// games apply their own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sticks {
    pub deadzone: u8,
    pub anti_deadzone: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Empty for the first profile until it is renamed; the window shows it as "Default".
    pub name: String,
    pub bindings: Bindings,
    pub light: Light,
    /// How bright a chosen or battery colour is, in percent.
    pub brightness: u8,
    /// Blink the light bar when the battery is nearly empty.
    pub low_battery_flash: bool,
    pub gyro: Gyro,
    pub sticks: Sticks,
    /// Executable names (`game.exe`, lowercase) that make this the profile in use while one of
    /// them is the program in front.
    pub programs: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: String::new(),
            bindings: Bindings::new(),
            light: Light::Player,
            brightness: 100,
            low_battery_flash: true,
            gyro: Gyro::default(),
            sticks: Sticks::default(),
            programs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub list: Vec<Profile>,
    pub active: usize,
}

impl Default for Profiles {
    fn default() -> Self {
        Profiles::new(String::new())
    }
}

/// A change to a controller's profiles, as the window asks for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    /// Assigns an extra button in the profile in use, or clears it.
    Bind {
        button: u8,
        action: Option<Action>,
    },
    /// Clears every assignment of the profile in use.
    ClearBindings,
    Light(Light),
    Brightness(u8),
    LowBatteryFlash(bool),
    Gyro(Gyro),
    Sticks(Sticks),
    /// Makes a profile the one in use while this program is in front.
    AddProgram(usize, String),
    RemoveProgram(usize, String),
    /// A new profile, a copy of the one in use, which it replaces as the one in use.
    Add(String),
    Rename(usize, String),
    Delete(usize),
    Select(usize),
}

impl Profiles {
    pub fn new(first: String) -> Profiles {
        Profiles { list: vec![Profile { name: first, ..Profile::default() }], active: 0 }
    }

    pub fn active(&self) -> &Profile {
        &self.list[self.active.min(self.list.len() - 1)]
    }

    fn active_mut(&mut self) -> &mut Profile {
        let i = self.active.min(self.list.len() - 1);
        &mut self.list[i]
    }

    /// The profile in use while `program` is in front: the first that names it, or the one the
    /// user chose.
    pub fn in_use(&self, program: &str) -> usize {
        if !program.is_empty()
            && let Some(i) = self.list.iter().position(|p| p.programs.iter().any(|x| x == program))
        {
            return i;
        }
        self.active.min(self.list.len() - 1)
    }

    pub fn for_program(&self, program: &str) -> &Profile {
        &self.list[self.in_use(program)]
    }

    /// Whether this is what a controller has before anything is changed, and need not be kept.
    pub fn is_default(&self) -> bool {
        self.list.len() == 1 && self.list[0] == Profile::default()
    }

    pub fn apply(&mut self, edit: Edit) {
        match edit {
            Edit::Bind { button, action } => {
                let b = &mut self.active_mut().bindings;
                match action.and_then(|a| a.sanitised()) {
                    Some(a) => b.insert(button, a),
                    None => b.remove(&button),
                };
            }
            Edit::ClearBindings => self.active_mut().bindings.clear(),
            Edit::Light(l) => self.active_mut().light = l,
            Edit::Brightness(b) => self.active_mut().brightness = b.clamp(10, 100),
            Edit::LowBatteryFlash(on) => self.active_mut().low_battery_flash = on,
            Edit::Gyro(g) => self.active_mut().gyro = clean_gyro(g),
            Edit::Sticks(s) => self.active_mut().sticks = clean_sticks(s),
            Edit::AddProgram(i, name) => {
                let name = program_name(&name);
                // A program switches to one profile only: the last one it was given to.
                for p in &mut self.list {
                    p.programs.retain(|x| *x != name);
                }
                if let Some(p) = self.list.get_mut(i)
                    && !name.is_empty()
                    && p.programs.len() < MAX_PROGRAMS
                {
                    p.programs.push(name);
                }
            }
            Edit::RemoveProgram(i, name) => {
                if let Some(p) = self.list.get_mut(i) {
                    p.programs.retain(|x| *x != program_name(&name));
                }
            }
            Edit::Add(name) if self.list.len() < MAX_PROFILES => {
                // Programs stay with the profile that named them.
                let copy = Profile { name: clean_name(&name), programs: Vec::new(), ..self.active().clone() };
                self.list.push(copy);
                self.active = self.list.len() - 1;
            }
            Edit::Add(_) => {}
            Edit::Rename(i, name) => {
                if let Some(p) = self.list.get_mut(i) {
                    p.name = clean_name(&name);
                }
            }
            Edit::Delete(i) if i < self.list.len() && self.list.len() > 1 => {
                self.list.remove(i);
                if self.active > i || self.active >= self.list.len() {
                    self.active = self.active.saturating_sub(1);
                }
            }
            Edit::Delete(_) => {}
            Edit::Select(i) if i < self.list.len() => self.active = i,
            Edit::Select(_) => {}
        }
    }

    /// Made valid whatever a settings file says: at least one profile, the one in use among
    /// them, assignments within their limits.
    pub fn sanitised(mut self) -> Profiles {
        self.list.truncate(MAX_PROFILES);
        if self.list.is_empty() {
            self.list.push(Profile::default());
        }
        for p in &mut self.list {
            p.name = clean_name(&p.name);
            p.brightness = p.brightness.clamp(10, 100);
            p.gyro = clean_gyro(p.gyro);
            p.sticks = clean_sticks(p.sticks);
            p.programs =
                std::mem::take(&mut p.programs).iter().map(|x| program_name(x)).filter(|x| !x.is_empty()).take(MAX_PROGRAMS).collect();
            p.bindings = std::mem::take(&mut p.bindings).into_iter().filter_map(|(b, a)| a.sanitised().map(|a| (b, a))).collect();
        }
        self.active = self.active.min(self.list.len() - 1);
        self
    }
}

fn clean_name(name: &str) -> String {
    name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect()
}

/// A program as profiles name it: the executable's file name, lowercase.
pub fn program_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    file.trim().chars().filter(|c| !c.is_control()).take(MAX_PROGRAM).collect::<String>().to_lowercase()
}

fn clean_gyro(g: Gyro) -> Gyro {
    Gyro { sensitivity: g.sensitivity.clamp(25, 400), ..g }
}

fn clean_sticks(s: Sticks) -> Sticks {
    Sticks { deadzone: s.deadzone.min(40), anti_deadzone: s.anti_deadzone.min(40) }
}

/// The colour a light bar shows for `light`, with the player's colours SDL and the consoles use.
pub fn light_color(light: Light, brightness: u8, player: Option<u8>, battery: Option<u8>) -> [u8; 3] {
    const PLAYERS: [[u8; 3]; 4] = [[0x00, 0x00, 0x40], [0x40, 0x00, 0x00], [0x00, 0x40, 0x00], [0x20, 0x00, 0x20]];
    let scale = |c: [u8; 3]| c.map(|v| (u16::from(v) * u16::from(brightness.clamp(10, 100)) / 100) as u8);
    match light {
        Light::Player => PLAYERS[usize::from(player.unwrap_or(0)) % PLAYERS.len()],
        Light::Color(c) => scale(c),
        Light::Battery => {
            // Red below 15 %, through yellow at half, to green when full.
            let level = u16::from(battery.unwrap_or(100).min(100));
            let (r, g) = if level <= 15 {
                (255, 0)
            } else if level < 50 {
                (255, ((level - 15) * 255 / 35) as u8)
            } else {
                ((255 - (level - 50) * 255 / 50) as u8, 255)
            };
            scale([r, g, 0])
        }
        Light::Off => [0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::XboxButton;

    #[test]
    fn edits_apply_to_the_profile_in_use() {
        let mut p = Profiles::new("Default".into());
        p.apply(Edit::Bind { button: 17, action: Some(Action::Xbox(XboxButton::A)) });
        p.apply(Edit::Add("Racing".into()));
        assert_eq!(p.active, 1);
        assert_eq!(p.active().bindings.len(), 1, "a new profile starts as a copy");
        p.apply(Edit::Bind { button: 17, action: None });
        assert!(p.active().bindings.is_empty());
        assert_eq!(p.list[0].bindings.len(), 1);
        p.apply(Edit::Select(0));
        assert_eq!(p.active().name, "Default");
    }

    #[test]
    fn deleting_keeps_one_and_a_valid_active() {
        let mut p = Profiles::new("A".into());
        p.apply(Edit::Delete(0));
        assert_eq!(p.list.len(), 1, "the last profile stays");
        p.apply(Edit::Add("B".into()));
        p.apply(Edit::Add("C".into()));
        p.apply(Edit::Delete(2));
        assert_eq!((p.list.len(), p.active().name.as_str()), (2, "B"));
        p.apply(Edit::Select(1));
        p.apply(Edit::Delete(0));
        assert_eq!(p.active().name, "B");
    }

    #[test]
    fn limits() {
        let mut p = Profiles::new("A".into());
        for i in 0..20 {
            p.apply(Edit::Add(format!("{i}")));
        }
        assert_eq!(p.list.len(), MAX_PROFILES);
        p.apply(Edit::Rename(0, format!(" {}\n ", "x".repeat(80))));
        assert_eq!(p.list[0].name.len(), MAX_NAME);
        let bad = Profiles { list: vec![], active: 9 }.sanitised();
        assert_eq!((bad.list.len(), bad.active), (1, 0));
    }

    #[test]
    fn programs_pick_the_profile_in_use() {
        let mut p = Profiles::new("A".into());
        p.apply(Edit::Add("Racing".into()));
        p.apply(Edit::Select(0));
        p.apply(Edit::AddProgram(1, r"C:\Games\Forza\ForzaHorizon5.exe".into()));
        assert_eq!(p.list[1].programs, ["forzahorizon5.exe"]);
        assert_eq!(p.in_use("forzahorizon5.exe"), 1);
        assert_eq!(p.in_use("explorer.exe"), 0, "otherwise the one chosen");
        assert_eq!(p.in_use(""), 0);
        p.apply(Edit::AddProgram(0, "ForzaHorizon5.exe".into()));
        assert_eq!((p.list[0].programs.len(), p.list[1].programs.len()), (1, 0), "one program, one profile");
        p.apply(Edit::Add("Copy".into()));
        assert!(p.active().programs.is_empty(), "a copy does not take the programs");
        p.apply(Edit::RemoveProgram(0, "forzahorizon5.exe".into()));
        assert!(p.list[0].programs.is_empty());
    }

    #[test]
    fn new_settings_are_clamped_and_count_as_changes() {
        let mut p = Profiles::default();
        assert!(p.is_default());
        p.apply(Edit::Gyro(Gyro { mode: GyroMode::Aiming, sensitivity: 5000, invert_y: true }));
        assert_eq!(p.active().gyro.sensitivity, 400);
        assert!(!p.is_default());
        p.apply(Edit::Sticks(Sticks { deadzone: 90, anti_deadzone: 15 }));
        assert_eq!(p.active().sticks, Sticks { deadzone: 40, anti_deadzone: 15 });
    }

    #[test]
    fn light_colors() {
        assert_eq!(light_color(Light::Player, 100, Some(1), None), [0x40, 0, 0]);
        assert_eq!(light_color(Light::Color([200, 100, 0]), 50, None, None), [100, 50, 0]);
        assert_eq!(light_color(Light::Battery, 100, None, Some(10)), [255, 0, 0]);
        assert_eq!(light_color(Light::Battery, 100, None, Some(100)), [0, 255, 0]);
        assert_eq!(light_color(Light::Off, 100, Some(0), Some(50)), [0, 0, 0]);
    }
}
