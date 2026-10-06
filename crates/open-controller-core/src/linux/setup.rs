//! The one thing Linux needs set up as root: a udev rule that lets the signed-in user create
//! virtual controllers (`/dev/uinput`) and read the HID reports of known controllers, which
//! carry their extra buttons, gyro and light. The access goes to whoever is at the seat
//! (`uaccess`), as Steam's own rules do, and only for controllers in the model table, so no
//! keyboard's raw reports become readable.

use crate::models::MODELS;
use std::collections::BTreeSet;
use std::path::Path;

pub const RULE_PATH: &str = "/etc/udev/rules.d/70-open-controller.rules";
/// Loads uinput at boot on systems that build it as a module and load it on demand only.
pub const MODULES_PATH: &str = "/etc/modules-load.d/open-controller.conf";

/// Makers whose every HID device is a controller.
const VENDORS: [u16; 4] = [0x054C, 0x057E, 0x28DE, 0x2DC8];

pub fn rule() -> String {
    let mut out = String::from(
        "# Open Controller: virtual controllers for the user at the seat, and the HID reports of\n\
         # known controllers. Written by Open Controller; remove this file to undo.\n\
         KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", OPTIONS+=\"static_node=uinput\"\n",
    );
    for v in VENDORS {
        out += &format!("KERNEL==\"hidraw*\", KERNELS==\"*:{v:04X}:*\", TAG+=\"uaccess\"\n");
    }
    let rest: BTreeSet<(u16, u16)> = MODELS.iter().filter(|r| !VENDORS.contains(&r.0)).map(|r| (r.0, r.1)).collect();
    for (v, p) in rest {
        out += &format!("KERNEL==\"hidraw*\", KERNELS==\"*:{v:04X}:{p:04X}.*\", TAG+=\"uaccess\"\n");
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleState {
    Missing,
    /// Written by an older version, which knew fewer controllers.
    Outdated,
    Current,
}

pub fn rule_state() -> RuleState {
    match std::fs::read_to_string(RULE_PATH) {
        Ok(s) if s == rule() => RuleState::Current,
        Ok(_) => RuleState::Outdated,
        Err(_) => RuleState::Missing,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    Cancelled,
}

/// Writes the rule as root through polkit, which asks for the password, and applies it to the
/// devices already connected.
pub fn install_rule(dir: &Path) -> Result<Outcome, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let rule_file = dir.join("70-open-controller.rules");
    let modules_file = dir.join("open-controller.conf");
    std::fs::write(&rule_file, rule()).map_err(|e| e.to_string())?;
    std::fs::write(&modules_file, "uinput\n").map_err(|e| e.to_string())?;
    let script = format!(
        "install -m 0644 '{}' {RULE_PATH} && install -m 0644 '{}' {MODULES_PATH} && \
         (modprobe uinput || true) && udevadm control --reload-rules && udevadm trigger",
        rule_file.display(),
        modules_file.display()
    );
    let status = std::process::Command::new("pkexec").args(["sh", "-c", &script]).status().map_err(|e| format!("pkexec: {e}"))?;
    match status.code() {
        Some(0) => Ok(Outcome::Installed),
        // Dismissed, or not authorised.
        Some(126 | 127) => Ok(Outcome::Cancelled),
        other => Err(format!("the setup ended with code {}", other.unwrap_or(-1))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_covers_uinput_and_known_controllers_only() {
        let r = rule();
        assert!(r.contains("KERNEL==\"uinput\""));
        assert!(r.contains("KERNELS==\"*:054C:*\""));
        // An 8BitDo pad is covered by its maker, a HORI pad by its own ids.
        assert!(!r.contains("2DC8:3106"));
        assert!(r.contains("KERNELS==\"*:0F0D:00C1.*\""));
        assert!(r.lines().all(|l| l.starts_with('#') || l.contains("TAG+=\"uaccess\"")));
    }
}
