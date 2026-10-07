//! The one thing Linux needs set up as root: a udev rule that lets the signed-in user create
//! virtual controllers (`/dev/uinput`) and read the HID reports of known controllers, which
//! carry their extra buttons, gyro and light. The access goes to whoever is at the seat
//! (`uaccess`), as Steam's own rules do, and only for controllers in the model table, so no
//! keyboard's raw reports become readable.

use crate::models::MODELS;
use std::collections::BTreeSet;
use std::io::Write;

pub const RULE_PATH: &str = "/etc/udev/rules.d/70-open-controller.rules";
/// Loads uinput at boot on systems that build it as a module and load it on demand only.
pub const MODULES_PATH: &str = "/etc/modules-load.d/open-controller.conf";

/// Makers whose every HID device is a controller.
const VENDORS: [u16; 4] = [0x054C, 0x057E, 0x28DE, 0x2DC8];

pub fn rule() -> String {
    let mut out = String::from(
        "# OpenController: virtual controllers for the user at the seat, and the HID reports of\n\
         # known controllers. Written by OpenController; remove this file to undo.\n\
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
    /// The password dialog was dismissed.
    Cancelled,
    /// The password could not be asked: no polkit agent is running, or polkit refused.
    NoAuthorization,
    /// The rule is in place, but the kernel has no uinput, so no controller can be made.
    NoUinput,
}

/// Exit codes of the setup script, clear of those of `sh` (126, 127) and `pkexec`.
const NO_UINPUT: i32 = 3;
const NOT_WRITTEN: i32 = 4;
const NOT_APPLIED: i32 = 5;

/// What runs as root: the rule comes in on standard input, so no file another user could swap
/// stands between this program and root.
fn script() -> String {
    format!(
        "umask 022; cat > {RULE_PATH}.new && mv -f {RULE_PATH}.new {RULE_PATH} && printf 'uinput\\n' > {MODULES_PATH} || exit {NOT_WRITTEN}; \
         modprobe uinput 2>/dev/null; \
         udevadm control --reload-rules && udevadm trigger || exit {NOT_APPLIED}; \
         [ -e /dev/uinput ] || exit {NO_UINPUT}"
    )
}

fn outcome(code: Option<i32>) -> Result<Outcome, String> {
    match code {
        Some(0) => Ok(Outcome::Installed),
        Some(NO_UINPUT) => Ok(Outcome::NoUinput),
        Some(126) => Ok(Outcome::Cancelled),
        Some(127) => Ok(Outcome::NoAuthorization),
        Some(NOT_WRITTEN) => Err("the rule could not be written".into()),
        Some(NOT_APPLIED) => Err("udev did not reload the rules".into()),
        other => Err(format!("the setup ended with code {}", other.unwrap_or(-1))),
    }
}

/// Writes the rule as root through polkit, which asks for the password, and applies it to the
/// devices already connected.
pub fn install_rule() -> Result<Outcome, String> {
    let child = std::process::Command::new("pkexec").args(["/bin/sh", "-c", &script()]).stdin(std::process::Stdio::piped()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Outcome::NoAuthorization),
        Err(e) => return Err(format!("pkexec: {e}")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        // Fails when pkexec ends without running the script; its exit code says why.
        let _ = stdin.write_all(rule().as_bytes());
    }
    let status = child.wait().map_err(|e| format!("pkexec: {e}"))?;
    outcome(status.code())
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

    #[test]
    fn setup_results() {
        assert_eq!(outcome(Some(0)), Ok(Outcome::Installed));
        assert_eq!(outcome(Some(126)), Ok(Outcome::Cancelled));
        assert_eq!(outcome(Some(127)), Ok(Outcome::NoAuthorization));
        assert_eq!(outcome(Some(NO_UINPUT)), Ok(Outcome::NoUinput));
        assert!(outcome(Some(NOT_WRITTEN)).is_err());
        assert!(outcome(None).is_err());
        assert!(!script().contains("/tmp"), "the rule comes in on standard input");
    }
}
