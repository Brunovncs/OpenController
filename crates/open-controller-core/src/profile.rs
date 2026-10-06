//! A controller's profiles: named sets of what its extra buttons do and what colour its light
//! shows, one of them in use. Every controller starts with one; more are added for games that
//! want different assignments.

use crate::binding::{Action, Bindings};
use serde::{Deserialize, Serialize};

pub const MAX_PROFILES: usize = 8;
pub const MAX_NAME: usize = 32;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Empty for the first profile until it is renamed; the window shows it as "Default".
    pub name: String,
    pub bindings: Bindings,
    pub light: Light,
    /// How bright a chosen or battery colour is, in percent.
    pub brightness: u8,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { name: String::new(), bindings: Bindings::new(), light: Light::Player, brightness: 100 }
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

    /// Whether this is what a controller has before anything is changed, and need not be kept.
    pub fn is_default(&self) -> bool {
        self.list.len() == 1 && {
            let p = &self.list[0];
            p.name.is_empty() && p.bindings.is_empty() && p.light == Light::Player && p.brightness == 100
        }
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
            Edit::Add(name) if self.list.len() < MAX_PROFILES => {
                let copy = Profile { name: clean_name(&name), ..self.active().clone() };
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
            p.bindings = std::mem::take(&mut p.bindings).into_iter().filter_map(|(b, a)| a.sanitised().map(|a| (b, a))).collect();
        }
        self.active = self.active.min(self.list.len() - 1);
        self
    }
}

fn clean_name(name: &str) -> String {
    name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect()
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
    fn light_colors() {
        assert_eq!(light_color(Light::Player, 100, Some(1), None), [0x40, 0, 0]);
        assert_eq!(light_color(Light::Color([200, 100, 0]), 50, None, None), [100, 50, 0]);
        assert_eq!(light_color(Light::Battery, 100, None, Some(10)), [255, 0, 0]);
        assert_eq!(light_color(Light::Battery, 100, None, Some(100)), [0, 255, 0]);
        assert_eq!(light_color(Light::Off, 100, Some(0), Some(50)), [0, 0, 0]);
    }
}
