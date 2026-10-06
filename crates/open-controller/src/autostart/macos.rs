//! A launch agent in the user's library, which launchd starts at sign-in.

use std::path::PathBuf;

const LABEL: &str = "io.github.brunovncs.open-controller";

fn agent() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn contents() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.to_string_lossy().replace('&', "&amp;").replace('<', "&lt;");
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--minimized</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#
    ))
}

pub fn enabled() -> bool {
    agent().is_some_and(|a| a.exists())
}

pub fn set(on: bool) -> bool {
    let Some(path) = agent() else { return false };
    if on {
        let Some(text) = contents() else { return false };
        path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, text).is_ok()
    } else {
        std::fs::remove_file(&path).is_ok() || !path.exists()
    }
}

/// Keeps the agent pointing at this executable if the app moved.
pub fn refresh() {
    if let (Some(path), Some(want)) = (agent(), contents())
        && let Ok(now) = std::fs::read_to_string(&path)
        && now != want
    {
        set(true);
    }
}
