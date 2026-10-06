//! An XDG autostart entry, which every desktop runs at sign-in.

use std::path::PathBuf;

fn entry() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("autostart").join("io.github.brunovncs.open-controller.desktop"))
}

fn contents() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!(
        "[Desktop Entry]\nType=Application\nName=OpenController\nComment=Controllers as Xbox controllers\n\
         Exec=\"{}\" --minimized\nIcon=io.github.brunovncs.open-controller\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
        exe.display()
    ))
}

pub fn enabled() -> bool {
    entry().is_some_and(|e| e.exists())
}

pub fn set(on: bool) -> bool {
    let Some(path) = entry() else { return false };
    if on {
        let Some(text) = contents() else { return false };
        path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, text).is_ok()
    } else {
        std::fs::remove_file(&path).is_ok() || !path.exists()
    }
}

/// Keeps the entry pointing at this executable if it moved.
pub fn refresh() {
    if let (Some(path), Some(want)) = (entry(), contents())
        && let Ok(now) = std::fs::read_to_string(&path)
        && now != want
    {
        set(true);
    }
}
