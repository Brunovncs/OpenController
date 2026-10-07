//! A report on one controller, for when it misbehaves or OpenController does not know it: what
//! SDL and the system say about it, gathered only when the user asks, shown to them in full and
//! sent only when they press Send. It goes to the website, which emails it to the maintainer, so
//! the controller can be added or fixed without having it in hand. Nothing in it tells one person
//! or one unit from another: no serial numbers, no Bluetooth addresses, no device instance ids.

use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Where reports go. `OPEN_CONTROLLER_REPORT_URL` replaces it, to try a local copy of the site.
pub const ENDPOINT: &str = "https://opencontroller.com.br/api/report";

/// The most a report may hold, so a runaway log cannot make a huge request.
pub const MAX_LEN: usize = 24_000;

/// What SDL and the system say about one connected controller.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facts {
    pub vendor: u16,
    pub product: u16,
    /// The device's release number (USB `bcdDevice`), which often tells hardware revisions apart.
    pub version: u16,
    pub firmware: u16,
    /// SDL's name for the joystick, before OpenController renames it.
    pub joystick_name: String,
    pub gamepad_name: Option<String>,
    /// SDL's id for the device: bus, ids, version and which of its drivers reads it.
    pub guid: String,
    /// The device path up to its ids (`\\?\HID#VID_054C&PID_09CC&MI_03`), without the parts that
    /// tell one unit from another.
    pub path: String,
    pub gamepad_type: String,
    pub real_type: String,
    pub joystick_type: String,
    pub connection: String,
    /// SDL's mapping from the joystick's buttons and axes to the gamepad's, `None` without one.
    pub mapping: Option<String>,
    pub axes: i32,
    pub buttons: i32,
    pub hats: i32,
    pub balls: i32,
    pub touchpads: i32,
    pub sensors: Vec<String>,
    pub capabilities: Vec<String>,
    /// The device and its parents up to the USB or Bluetooth device, as Device Manager names them.
    pub devices: Vec<String>,
}

/// A controller's facts and what the resident process saw happen lately, as it hands them to
/// the window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub facts: Facts,
    /// Controllers coming and going since the resident process started, oldest first.
    pub log: Vec<String>,
}

/// Cuts a device path down to its first two parts (`\\?\HID#VID_054C&PID_09CC&MI_03`): what
/// follows on Windows is an instance id, which can hold a Bluetooth address.
pub fn model_path(path: &str) -> String {
    if path.contains('#') { path.split('#').take(2).collect::<Vec<_>>().join("#") } else { path.to_string() }
}

/// What the user adds and what the window knows, sent with the text of the report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Submission {
    /// `"app"`: the website tells reports from the app apart from requests made on the site.
    pub source: &'static str,
    pub version: String,
    pub system: String,
    pub lang: String,
    /// `Name (vvvv:pppp)`, for the email's subject.
    pub controller: String,
    /// The model the user says it is.
    pub model: String,
    pub message: String,
    pub email: String,
    pub report: String,
}

/// The system's name and version, such as `Windows 11 (10.0.26200)`.
pub fn system() -> String {
    #[cfg(windows)]
    {
        let out =
            cmd().args(["/d", "/c", "ver"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        windows_name(&out)
    }
    #[cfg(not(windows))]
    {
        let uname = |a: &str| {
            Command::new("uname").arg(a).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
        };
        format!("{} {}", uname("-s"), uname("-r")).trim().to_string()
    }
}

/// From what `ver` prints, "Microsoft Windows [Version 10.0.26200.1234]" in the system's language,
/// to `Windows 11 (10.0.26200.1234)`: Windows 11 is the one from build 22000 on.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_name(ver: &str) -> String {
    let version = ver.split(|c: char| c.is_whitespace() || c == '[' || c == ']').find(|w| w.split('.').count() >= 3).unwrap_or("");
    let build: u32 = version.split('.').nth(2).and_then(|b| b.parse().ok()).unwrap_or(0);
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    if version.is_empty() { "Windows".to_string() } else { format!("{name} ({version})") }
}

#[cfg(windows)]
fn cmd() -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::var_os("ComSpec").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows\\System32\\cmd.exe".into());
    let mut c = Command::new(exe);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

fn curl() -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("System32\\curl.exe");
        let mut c = Command::new(exe);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    }
    #[cfg(not(windows))]
    Command::new("curl")
}

/// Sends a report to the website. Blocks for up to 20 seconds; run it away from the interface.
/// The body goes through curl's standard input, so no command line limit or quoting applies.
pub fn send(s: &Submission) -> Result<(), String> {
    use std::io::Write;
    let url = std::env::var("OPEN_CONTROLLER_REPORT_URL").unwrap_or_else(|_| ENDPOINT.to_string());
    let body = serde_json::to_vec(s).map_err(|e| e.to_string())?;
    let mut child = curl()
        .args(["--silent", "--show-error", "--fail-with-body", "--max-time", "20", "--request", "POST"])
        .args(["--header", "Content-Type: application/json", "--user-agent", &format!("open-controller/{}", s.version)])
        .args(["--data-binary", "@-"])
        .arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&body).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let answer = String::from_utf8_lossy(&out.stdout);
    let error = String::from_utf8_lossy(&out.stderr);
    Err(if answer.contains("\"rate\"") { "rate".to_string() } else { format!("{} {}", error.trim(), answer.trim()).trim().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_lose_their_instance() {
        assert_eq!(
            model_path(
                r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002054c_PID&05c4#9&2c3c6d1&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"
            ),
            r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002054c_PID&05c4"
        );
        assert_eq!(model_path("/dev/input/event12"), "/dev/input/event12");
        assert_eq!(model_path("XInput#0"), "XInput#0");
    }

    #[test]
    fn windows_in_any_language() {
        assert_eq!(windows_name("\r\nMicrosoft Windows [versão 10.0.26200.6584]\r\n"), "Windows 11 (10.0.26200.6584)");
        assert_eq!(windows_name("Microsoft Windows [Version 10.0.19045.5737]"), "Windows 10 (10.0.19045.5737)");
        assert_eq!(windows_name(""), "Windows");
    }
}
