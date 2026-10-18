//! A launch agent inside the app bundle, registered with the system (SMAppService), which starts
//! the resident process at sign-in. Login Items shows it under the app's name, and it follows the
//! app wherever it is moved. Versions up to 0.8.8 wrote an agent with the executable's path into
//! the user's library instead; that one is moved over, and stays the way for a copy outside an
//! app bundle, as in development.

use objc2_foundation::NSString;
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

const LABEL: &str = "io.github.brunovncs.open-controller";
/// The agent's label, in `packaging/macos`, which copies it into the bundle.
const AGENT: &str = "io.github.brunovncs.open-controller.agent";

/// Turned off while the agent itself is running: unregistering would end this process, so it
/// waits for the process to end.
static PENDING_OFF: AtomicBool = AtomicBool::new(false);

/// Calls `f` with the bundle's agent, when this executable runs from a bundle that has one.
fn with_agent<T>(f: impl FnOnce(&SMAppService) -> T) -> Option<T> {
    let exe = std::env::current_exe().ok()?;
    let plist = format!("{AGENT}.plist");
    if !exe.parent()?.parent()?.join("Library/LaunchAgents").join(&plist).is_file() {
        return None;
    }
    let agent = unsafe { SMAppService::agentServiceWithPlistName(&NSString::from_str(&plist)) };
    Some(f(&agent))
}

fn status() -> Option<SMAppServiceStatus> {
    with_agent(|a| unsafe { a.status() })
}

/// An app opened straight from Downloads runs from a random read-only copy, which is gone at the
/// next sign-in.
fn translocated() -> bool {
    std::env::current_exe().is_ok_and(|e| e.to_string_lossy().contains("/AppTranslocation/"))
}

/// launchd names the job that started a process in `XPC_SERVICE_NAME`.
fn started_by_agent() -> bool {
    std::env::var("XPC_SERVICE_NAME").is_ok_and(|job| job == AGENT)
}

/// The version that registered the agent. The system wants it registered again when the
/// executable changes, or it may not start it.
fn version_file() -> PathBuf {
    crate::settings::default_data_dir().join("login-item")
}

fn register() -> Result<(), String> {
    if status() != Some(SMAppServiceStatus::Enabled) {
        with_agent(|a| unsafe { a.registerAndReturnError() }).ok_or("no agent in the bundle")?.map_err(|e| e.to_string())?;
    }
    let _ = std::fs::write(version_file(), env!("CARGO_PKG_VERSION"));
    Ok(())
}

fn legacy() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn legacy_contents() -> Option<String> {
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

fn write_legacy() -> bool {
    let (Some(path), Some(text)) = (legacy(), legacy_contents()) else { return false };
    path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, text).is_ok()
}

fn remove_legacy() -> bool {
    legacy().is_none_or(|p| std::fs::remove_file(&p).is_ok() || !p.exists())
}

extern "C" fn unregister_at_exit() {
    if PENDING_OFF.load(Ordering::Relaxed) {
        let _ = with_agent(|a| unsafe { a.unregisterAndReturnError() });
    }
}

pub fn enabled() -> bool {
    let agent = status() == Some(SMAppServiceStatus::Enabled) && !PENDING_OFF.load(Ordering::Relaxed);
    agent || legacy().is_some_and(|p| p.exists())
}

pub fn set(on: bool) -> bool {
    if !on {
        let mut ok = remove_legacy();
        if status().is_some_and(|s| s != SMAppServiceStatus::NotRegistered) {
            if started_by_agent() {
                static AT_EXIT: Once = Once::new();
                AT_EXIT.call_once(|| unsafe {
                    libc::atexit(unregister_at_exit);
                });
                PENDING_OFF.store(true, Ordering::Relaxed);
            } else {
                ok &= with_agent(|a| unsafe { a.unregisterAndReturnError() }).is_some_and(|r| r.is_ok());
            }
        }
        return ok;
    }
    PENDING_OFF.store(false, Ordering::Relaxed);
    if translocated() {
        eprintln!("open-controller: move OpenController to Applications before turning on starting at sign-in");
        return false;
    }
    if status().is_none() {
        return write_legacy();
    }
    match register() {
        Ok(()) => {
            remove_legacy();
            true
        }
        // Turned off in System Settings, Login Items, where only the user can turn it on again.
        Err(_) if status() == Some(SMAppServiceStatus::RequiresApproval) => {
            unsafe { SMAppService::openSystemSettingsLoginItems() };
            false
        }
        Err(e) => {
            eprintln!("open-controller: the login item was not registered ({e}); using a launch agent instead");
            write_legacy()
        }
    }
}

/// Moves an agent of an earlier version over to the bundle's, registers the bundle's again
/// after an update, and keeps a fallback agent pointing at this executable if the app moved.
pub fn refresh() {
    if translocated() || started_by_agent() {
        return;
    }
    let had_legacy = legacy().is_some_and(|p| p.exists());
    match status() {
        None => {
            if let (Some(path), Some(want)) = (legacy(), legacy_contents())
                && let Ok(now) = std::fs::read_to_string(&path)
                && now != want
            {
                write_legacy();
            }
        }
        Some(SMAppServiceStatus::Enabled) => {
            let registered = std::fs::read_to_string(version_file()).unwrap_or_default();
            if registered.is_empty() {
                let _ = std::fs::write(version_file(), env!("CARGO_PKG_VERSION"));
            } else if registered != env!("CARGO_PKG_VERSION") {
                let _ = with_agent(|a| unsafe { a.unregisterAndReturnError() });
                if let Err(e) = register() {
                    eprintln!("open-controller: the login item was not registered again ({e})");
                }
            }
        }
        Some(_) if had_legacy => {
            if register().is_ok() {
                remove_legacy();
            }
        }
        Some(_) => {}
    }
}
