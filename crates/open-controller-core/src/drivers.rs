//! The drivers OpenController relies on or can use, whether each is installed, and installing
//! one when the user asks: the installer is downloaded from its maker's GitHub release, checked
//! against the hash it had when this version was made, and run elevated. Nothing is installed on
//! its own.

use crate::win::{from_wide, wide};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW,
};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// What Windows Installer and most setups return when the change needs a restart.
const RESTART_NEEDED: u32 = 3010;
/// The user said no to an installer's own cancel prompt.
const USER_EXIT: u32 = 1602;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Component {
    /// Virtual Xbox controllers. Required.
    ViGEmBus,
    /// Hides the original controller from games, so they see one controller, not two.
    HidHide,
    /// DualShock 3 and Sixaxis over USB: Windows has no driver for them.
    DsHidMini,
    /// DualShock 3 over Bluetooth, on top of DsHidMini.
    BthPs3,
}

pub struct Package {
    pub component: Component,
    pub name: &'static str,
    pub version: &'static str,
    pub url: &'static str,
    sha256: &'static str,
    /// The name it has in Programs and Features, to tell whether it is installed.
    display_name: &'static str,
    msi: bool,
    /// Its project page, for what it does and its own instructions.
    pub home: &'static str,
}

pub const PACKAGES: [Package; 4] = [
    Package {
        component: Component::ViGEmBus,
        name: "ViGEmBus",
        version: "1.22.0",
        url: "https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe",
        sha256: "89220a7865076b342892f98865f3499fb7c4cfd673159e89d352c360fd014c6a",
        display_name: "ViGEm Bus Driver",
        msi: false,
        home: "https://github.com/nefarius/ViGEmBus",
    },
    Package {
        component: Component::HidHide,
        name: "HidHide",
        version: "1.5.230",
        url: "https://github.com/nefarius/HidHide/releases/download/v1.5.230.0/HidHide_1.5.230_x64.exe",
        sha256: "f4bbbcb82e6258641b887c74bc81c4c5f66e4aa811808dfc304347687b7605f6",
        display_name: "HidHide",
        msi: false,
        home: "https://github.com/nefarius/HidHide",
    },
    Package {
        component: Component::DsHidMini,
        name: "DsHidMini",
        version: "3.20.1",
        url: "https://github.com/nefarius/DsHidMini/releases/download/setup-v3.20.1/Nefarius_DsHidMini_Drivers_x64_arm64_v3.20.1.msi",
        sha256: "f2622723e63b1a537a27b2e49f92fb90d5e96cde18b5cd4505dbd8ffb947e529",
        display_name: "Nefarius DsHidMini Driver",
        msi: true,
        home: "https://docs.nefarius.at/projects/DsHidMini/",
    },
    Package {
        component: Component::BthPs3,
        name: "BthPS3",
        version: "3.2.0",
        url: "https://github.com/nefarius/BthPS3/releases/download/setup-v3.2.0/Nefarius_BthPS3_Drivers_x64_arm64_v3.2.0.msi",
        sha256: "750cfe3caeb38d3443db5293599a8d4ce952702aa108610bc5c7248322990b77",
        display_name: "Nefarius BthPS3 Bluetooth Drivers",
        msi: true,
        home: "https://docs.nefarius.at/projects/BthPS3/",
    },
];

pub fn package(c: Component) -> &'static Package {
    PACKAGES.iter().find(|p| p.component == c).expect("every component has a package")
}

/// The installed version, from Programs and Features; `None` when it is not installed.
pub fn installed(c: Component) -> Option<String> {
    let want = package(c).display_name;
    installed_programs().into_iter().find(|(name, _)| name == want).map(|(_, v)| v)
}

/// Whether `installed` is older than the version this release knows.
pub fn outdated(c: Component, installed: &str) -> bool {
    version_key(installed) < version_key(package(c).version)
}

fn version_key(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    /// Installed; Windows has to restart before it works.
    RestartNeeded,
    /// The user turned down the administrator prompt or cancelled the installer.
    Cancelled,
}

/// Downloads `c`'s installer into `dir`, checks it and runs it as administrator, waiting for it
/// to finish. `silent` runs it without its own windows; otherwise its setup wizard shows, which
/// an update of HidHide needs, since it may ask to restart halfway.
pub fn install(c: Component, dir: &Path, silent: bool) -> Result<Outcome, String> {
    let p = package(c);
    let file = download(p, dir)?;
    let (exe, args) = if p.msi {
        let mode = if silent { "/passive" } else { "" };
        ("msiexec.exe".to_string(), format!("/i \"{}\" {mode} /norestart", file.display()))
    } else {
        (file.display().to_string(), if silent { "/exenoui /qn /norestart".to_string() } else { String::new() })
    };
    let code = run_elevated(&exe, &args)?;
    let _ = std::fs::remove_file(&file);
    match code {
        0 => Ok(Outcome::Installed),
        RESTART_NEEDED => Ok(Outcome::RestartNeeded),
        USER_EXIT => Ok(Outcome::Cancelled),
        n => Err(format!("the installer ended with code {n}")),
    }
}

fn download(p: &Package, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let name = p.url.rsplit('/').next().unwrap_or("setup.exe");
    let file = dir.join(name);
    if !matches(&file, p.sha256) {
        // curl ships with Windows 10 1803 and later.
        let curl = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("System32\\curl.exe");
        let out = Command::new(curl)
            .args(["--silent", "--show-error", "--fail", "--location", "--retry", "2", "--output"])
            .arg(&file)
            .arg(p.url)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("could not start the download: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        if !matches(&file, p.sha256) {
            let _ = std::fs::remove_file(&file);
            return Err(format!("the downloaded {} is not the one this version of OpenController knows", p.name));
        }
    }
    Ok(file)
}

fn matches(file: &Path, sha256: &str) -> bool {
    std::fs::read(file).is_ok_and(|bytes| hex(&Sha256::digest(bytes)) == sha256)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn run_elevated(exe: &str, args: &str) -> Result<u32, String> {
    let (verb, exe, args) = (wide("runas"), wide(exe), wide(args));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe.as_ptr();
    info.lpParameters = args.as_ptr();
    info.nShow = 1;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let e = unsafe { GetLastError() };
        return if e == ERROR_CANCELLED { Ok(USER_EXIT) } else { Err(std::io::Error::from_raw_os_error(e as i32).to_string()) };
    }
    if info.hProcess.is_null() {
        return Err("the installer did not start".into());
    }
    let mut code = 0u32;
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    Ok(code)
}

/// (display name, version) of every program in Programs and Features, both registry views.
fn installed_programs() -> Vec<(String, String)> {
    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut out = Vec::new();
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        let mut key: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, wide(UNINSTALL).as_ptr(), 0, KEY_READ | view, &mut key) } != 0 {
            continue;
        }
        for i in 0.. {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let r = unsafe {
                RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if r != 0 {
                break;
            }
            let sub = from_wide(&name);
            if let Some(display) = string_value(key, &sub, "DisplayName") {
                out.push((display, string_value(key, &sub, "DisplayVersion").unwrap_or_default()));
            }
        }
        unsafe { RegCloseKey(key) };
    }
    out
}

fn string_value(key: HKEY, sub: &str, value: &str) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut len = size_of_val(&buf) as u32;
    let r = unsafe {
        RegGetValueW(key, wide(sub).as_ptr(), wide(value).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len)
    };
    (r == 0).then(|| from_wide(&buf)).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(outdated(Component::HidHide, "1.2.98"));
        assert!(!outdated(Component::HidHide, "1.5.230"));
        assert!(!outdated(Component::ViGEmBus, "1.22.0"));
        assert!(outdated(Component::DsHidMini, "3.9.0"));
    }

    #[test]
    fn packages_are_pinned() {
        for p in &PACKAGES {
            assert!(p.url.starts_with("https://github.com/nefarius/"), "{}", p.name);
            assert_eq!(p.sha256.len(), 64, "{}", p.name);
            assert!(p.url.contains(p.version), "{}", p.name);
        }
    }

    /// Downloads the DsHidMini installer and checks it, without running it. Needs the network.
    #[test]
    #[ignore]
    fn downloads_and_checks_an_installer() {
        let dir = std::env::temp_dir().join(format!("oc-download-{}", std::process::id()));
        let file = download(package(Component::DsHidMini), &dir).expect("download");
        assert!(matches(&file, package(Component::DsHidMini).sha256));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_installed_programs() {
        // Every Windows install has some; the list must not come back empty or garbled.
        let all = installed_programs();
        assert!(!all.is_empty());
        assert!(all.iter().all(|(n, _)| !n.contains('\0')));
    }
}
