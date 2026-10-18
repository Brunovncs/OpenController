//! A systemd user service where the desktop runs one for its session, so the resident process
//! comes back if it crashes; elsewhere an XDG autostart entry, which every desktop runs at
//! sign-in. The service hangs on `graphical-session.target`, not `default.target`: that one is
//! reached before the desktop has said where its display is, and without it the program in
//! front cannot be read nor the window opened.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ID: &str = "io.github.brunovncs.open-controller";

fn entry() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("autostart").join(format!("{ID}.desktop")))
}

fn unit() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("systemd/user").join(format!("{ID}.service")))
}

fn entry_contents() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!(
        "[Desktop Entry]\nType=Application\nName=OpenController\nComment=Controllers as Xbox controllers\n\
         Exec=\"{}\" --minimized\nIcon=io.github.brunovncs.open-controller\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
        exe.display()
    ))
}

fn unit_contents() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.to_str()?;
    // systemd splits ExecStart on spaces and expands % and $; a path with any of them stays with XDG.
    if exe.contains([' ', '%', '$', '"', '\\']) {
        return None;
    }
    Some(format!(
        "[Unit]\nDescription=OpenController\nPartOf=graphical-session.target\nAfter=graphical-session.target\n\n\
         [Service]\nExecStart={exe} --minimized\nRestart=on-failure\nRestartSec=2\n\n\
         [Install]\nWantedBy=graphical-session.target\n"
    ))
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl").arg("--user").args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Whether this session's desktop starts the user services of `graphical-session.target`, as
/// GNOME and Plasma do; asked from inside the session, so it holds for the next sign-in too.
fn session_has_systemd() -> bool {
    systemctl(&["is-active", "--quiet", "graphical-session.target"])
}

fn write(path: &Path, text: &str) -> bool {
    path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(path, text).is_ok()
}

fn remove(path: Option<PathBuf>) -> bool {
    path.is_none_or(|p| std::fs::remove_file(&p).is_ok() || !p.exists())
}

fn install_unit() -> bool {
    let (Some(path), Some(text)) = (unit(), unit_contents()) else { return false };
    write(&path, &text) && systemctl(&["daemon-reload"]) && systemctl(&["enable", &format!("{ID}.service")])
}

fn remove_unit() -> bool {
    let Some(path) = unit() else { return true };
    if !path.exists() {
        return true;
    }
    // Disabling leaves the running process alone; it only stops the next start.
    systemctl(&["disable", &format!("{ID}.service")]);
    let gone = remove(Some(path));
    systemctl(&["daemon-reload"]);
    gone
}

pub fn enabled() -> bool {
    entry().is_some_and(|e| e.exists()) || unit().is_some_and(|u| u.exists())
}

pub fn set(on: bool) -> bool {
    if !on {
        return remove_unit() & remove(entry());
    }
    if session_has_systemd() && install_unit() {
        return remove(entry());
    }
    remove_unit();
    let (Some(path), Some(text)) = (entry(), entry_contents()) else { return false };
    write(&path, &text)
}

/// Keeps the service or the entry pointing at this executable if it moved, and moves an entry of
/// an earlier version over to a service where the session has them.
pub fn refresh() {
    if let (Some(path), Some(want)) = (unit(), unit_contents())
        && let Ok(now) = std::fs::read_to_string(&path)
        && now != want
    {
        set(true);
    } else if let (Some(path), Some(want)) = (entry(), entry_contents())
        && let Ok(now) = std::fs::read_to_string(&path)
        && (now != want || session_has_systemd())
    {
        set(true);
    }
}
