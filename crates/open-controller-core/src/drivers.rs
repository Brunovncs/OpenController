//! The drivers OpenController relies on or can use, whether each is installed, and installing
//! one when the user asks: the installer is downloaded from its maker's GitHub release, checked
//! against the hash it had when this version was made, and run elevated. VIIPER's server, a
//! program rather than a driver, is unpacked next to OpenController instead. Nothing is installed
//! on its own.

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
use windows_sys::Win32::System::SystemInformation::IMAGE_FILE_MACHINE_ARM64;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetExitCodeProcess, INFINITE, IsWow64Process2, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// What Windows Installer and most setups return when the change needs a restart.
const RESTART_NEEDED: u32 = 3010;
/// The user said no to an installer's own cancel prompt.
const USER_EXIT: u32 = 1602;
/// Inno Setup's codes for "cancelled before it started" and "cancelled while installing".
const INNO_CANCELLED: u32 = 2;
const INNO_ABORTED: u32 = 5;

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
    /// The USB/IP driver VIIPER attaches its controllers with. Only with VIIPER chosen.
    UsbipWin2,
    /// VIIPER's server, a program OpenController starts when VIIPER is chosen.
    Viiper,
}

/// How a package is installed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Setup {
    /// An Advanced Installer setup, run as administrator.
    Exe,
    Msi,
    /// An Inno Setup installer, run as administrator. It never says that Windows has to restart,
    /// so a driver installed this way is taken to need one.
    Inno,
    /// A zip with a program, unpacked into [`crate::viiper::server_dir`] without elevation.
    Zip,
}

/// How to tell that a package is installed.
#[derive(Clone, Copy)]
enum Found {
    /// Its exact name in Programs and Features.
    Named(&'static str),
    /// The start of its name there, which goes on with the version.
    Prefixed(&'static str),
    /// The program it unpacked, with the version written next to it.
    Unpacked,
}

/// A download: where from and the SHA-256 it had when this version was made.
pub struct Source {
    pub url: &'static str,
    sha256: &'static str,
}

pub struct Package {
    pub component: Component,
    pub name: &'static str,
    pub version: &'static str,
    pub source: Source,
    /// The download for ARM64 Windows, where it is a different one: a driver has to be native.
    arm64: Option<Source>,
    found: Found,
    setup: Setup,
    /// Its project page, for what it does and its own instructions.
    pub home: &'static str,
}

pub const PACKAGES: [Package; 6] = [
    Package {
        component: Component::ViGEmBus,
        name: "ViGEmBus",
        version: "1.22.0",
        source: Source {
            url: "https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe",
            sha256: "89220a7865076b342892f98865f3499fb7c4cfd673159e89d352c360fd014c6a",
        },
        arm64: None,
        found: Found::Named("ViGEm Bus Driver"),
        setup: Setup::Exe,
        home: "https://github.com/nefarius/ViGEmBus",
    },
    Package {
        component: Component::HidHide,
        name: "HidHide",
        version: "1.5.230",
        source: Source {
            url: "https://github.com/nefarius/HidHide/releases/download/v1.5.230.0/HidHide_1.5.230_x64.exe",
            sha256: "f4bbbcb82e6258641b887c74bc81c4c5f66e4aa811808dfc304347687b7605f6",
        },
        arm64: None,
        found: Found::Named("HidHide"),
        setup: Setup::Exe,
        home: "https://github.com/nefarius/HidHide",
    },
    Package {
        component: Component::DsHidMini,
        name: "DsHidMini",
        version: "3.20.1",
        source: Source {
            url: "https://github.com/nefarius/DsHidMini/releases/download/setup-v3.20.1/Nefarius_DsHidMini_Drivers_x64_arm64_v3.20.1.msi",
            sha256: "f2622723e63b1a537a27b2e49f92fb90d5e96cde18b5cd4505dbd8ffb947e529",
        },
        arm64: None,
        found: Found::Named("Nefarius DsHidMini Driver"),
        setup: Setup::Msi,
        home: "https://docs.nefarius.at/projects/DsHidMini/",
    },
    Package {
        component: Component::BthPs3,
        name: "BthPS3",
        version: "3.2.0",
        source: Source {
            url: "https://github.com/nefarius/BthPS3/releases/download/setup-v3.2.0/Nefarius_BthPS3_Drivers_x64_arm64_v3.2.0.msi",
            sha256: "750cfe3caeb38d3443db5293599a8d4ce952702aa108610bc5c7248322990b77",
        },
        arm64: None,
        found: Found::Named("Nefarius BthPS3 Bluetooth Drivers"),
        setup: Setup::Msi,
        home: "https://docs.nefarius.at/projects/BthPS3/",
    },
    // The IOCTL VIIPER attaches devices with changed with almost every usbip-win2 release, so
    // this stays at a version VIIPER 0.8.2 knows.
    Package {
        component: Component::UsbipWin2,
        name: "usbip-win2",
        version: "0.9.8.1",
        source: Source {
            url: "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.8.1/USBip-0.9.8.1-x64.exe",
            sha256: "38cad6d4432b52d5bb9409d9ad03b72fdffc4ada4cd3a48fbeca1a2752a8518a",
        },
        arm64: Some(Source {
            url: "https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.8.1/USBip-0.9.8.1-arm64.exe",
            sha256: "cab7ff97f79275eeb5c5b8bb2eb3111ff6b6c4eead07d9bec9be5bd1e3a35800",
        }),
        found: Found::Prefixed("USBip version "),
        setup: Setup::Inno,
        home: "https://github.com/vadimgrn/usbip-win2",
    },
    Package {
        component: Component::Viiper,
        name: "VIIPER",
        version: "0.8.2",
        source: Source {
            url: "https://github.com/Alia5/VIIPER/releases/download/v0.8.2/viiper-windows-amd64.zip",
            sha256: "386bce764f128d7e504f884c5bb3f15ae7e21cc53a84e48f4b449990a7a1b6ec",
        },
        arm64: Some(Source {
            url: "https://github.com/Alia5/VIIPER/releases/download/v0.8.2/viiper-windows-arm64.zip",
            sha256: "36229bfb93de4be9695d90000f68281139420052b9f1ae4fc37370be977e227f",
        }),
        found: Found::Unpacked,
        setup: Setup::Zip,
        home: "https://github.com/Alia5/VIIPER",
    },
];

/// The SHA-256 of the `viiper.exe` inside each of the zips above, x64 then ARM64: the server
/// started is the one this version was made with.
const VIIPER_EXE_SHA256: [&str; 2] = [
    "ab08285a025d0bb08005a8bca160e86bae8c536c8a77ca7945e4ccc6b158ee93",
    "fab244072c1d035d2843946363500cd682b168d019466ade068a042cf4550fa5",
];
/// Written next to the unpacked server: the version it is.
const VERSION_FILE: &str = "version.txt";

pub fn package(c: Component) -> &'static Package {
    PACKAGES.iter().find(|p| p.component == c).expect("every component has a package")
}

/// The installed version, from Programs and Features or next to the unpacked program; `None`
/// when it is not installed.
pub fn installed(c: Component) -> Option<String> {
    match package(c).found {
        Found::Named(want) => installed_programs().into_iter().find(|(name, _)| name == want).map(|(_, v)| v),
        Found::Prefixed(start) => installed_programs().into_iter().find(|(name, _)| name.starts_with(start)).map(|(_, v)| v),
        Found::Unpacked => crate::viiper::server_exe()
            .is_file()
            .then(|| std::fs::read_to_string(crate::viiper::server_dir().join(VERSION_FILE)).unwrap_or_default().trim().to_string()),
    }
}

/// Whether `exe` is the VIIPER server this version was made with.
pub fn is_pinned_server(exe: &Path) -> bool {
    std::fs::read(exe).is_ok_and(|bytes| VIIPER_EXE_SHA256.contains(&hex(&Sha256::digest(bytes)).as_str()))
}

/// What to download on this machine: ARM64 Windows runs this x64 build emulated, but a driver
/// it installs has to be native.
fn source(p: &Package) -> &Source {
    p.arm64.as_ref().filter(|_| native_arm64()).unwrap_or(&p.source)
}

fn native_arm64() -> bool {
    let (mut process, mut native) = (0, 0);
    unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) != 0 && native == IMAGE_FILE_MACHINE_ARM64 }
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
    if p.setup == Setup::Zip {
        let done = unpack(&file, &crate::viiper::server_dir(), p.version);
        let _ = std::fs::remove_file(&file);
        return done.map(|()| Outcome::Installed);
    }
    let (exe, args) = match p.setup {
        Setup::Msi => ("msiexec.exe".to_string(), format!("/i \"{}\" {} /norestart", file.display(), if silent { "/passive" } else { "" })),
        Setup::Inno => (file.display().to_string(), if silent { "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART" } else { "/NORESTART" }.into()),
        _ => (file.display().to_string(), if silent { "/exenoui /qn /norestart".to_string() } else { String::new() }),
    };
    let code = run_elevated(&exe, &args)?;
    let _ = std::fs::remove_file(&file);
    match (code, p.setup) {
        (0, Setup::Inno) => Ok(Outcome::RestartNeeded),
        (0, _) => Ok(Outcome::Installed),
        (RESTART_NEEDED, _) => Ok(Outcome::RestartNeeded),
        (USER_EXIT, _) | (INNO_CANCELLED | INNO_ABORTED, Setup::Inno) => Ok(Outcome::Cancelled),
        (n, _) => Err(format!("the installer ended with code {n}")),
    }
}

/// Unpacks VIIPER's zip (the server and its licenses) into `dir` with the `tar` that comes with
/// Windows, and checks the server it holds.
fn unpack(zip: &Path, dir: &Path, version: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tar = system32("tar.exe");
    let out = Command::new(tar)
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(dir)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("could not unpack it: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let exe = dir.join("viiper.exe");
    if !is_pinned_server(&exe) {
        let _ = std::fs::remove_file(&exe);
        return Err("the unpacked VIIPER server is not the one this version of OpenController knows".into());
    }
    std::fs::write(dir.join(VERSION_FILE), version).map_err(|e| e.to_string())
}

fn system32(program: &str) -> PathBuf {
    std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("System32").join(program)
}

fn download(p: &Package, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let Source { url, sha256 } = source(p);
    let name = url.rsplit('/').next().unwrap_or("setup.exe");
    let file = dir.join(name);
    if !matches(&file, sha256) {
        // curl and tar ship with Windows 10 1803 and later.
        let out = Command::new(system32("curl.exe"))
            .args(["--silent", "--show-error", "--fail", "--location", "--retry", "2", "--output"])
            .arg(&file)
            .arg(url)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("could not start the download: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        if !matches(&file, sha256) {
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

    fn is_sha256(s: &str) -> bool {
        s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    #[test]
    fn packages_are_pinned() {
        // Each maker's own GitHub releases, at a fixed version.
        const MAKERS: [&str; 3] = [
            "https://github.com/nefarius/",
            "https://github.com/vadimgrn/usbip-win2/releases/",
            "https://github.com/Alia5/VIIPER/releases/",
        ];
        for p in &PACKAGES {
            for s in std::iter::once(&p.source).chain(&p.arm64) {
                assert!(MAKERS.iter().any(|m| s.url.starts_with(m)), "{}: {}", p.name, s.url);
                assert!(is_sha256(s.sha256), "{}", p.name);
                assert!(s.url.contains(p.version), "{}", p.name);
            }
            if let Some(arm) = &p.arm64 {
                assert!(arm.url.contains("arm64") && arm.sha256 != p.source.sha256, "{}", p.name);
            }
        }
        assert!(VIIPER_EXE_SHA256.iter().all(|s| is_sha256(s)));
    }

    #[test]
    fn only_the_pinned_server_is_taken() {
        let dir = std::env::temp_dir().join(format!("oc-viiper-pin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("viiper.exe");
        std::fs::write(&fake, b"MZ not the server").unwrap();
        assert!(!is_pinned_server(&fake));
        assert!(!is_pinned_server(&dir.join("missing.exe")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Downloads the DsHidMini installer and checks it, without running it. Needs the network.
    #[test]
    #[ignore]
    fn downloads_and_checks_an_installer() {
        let dir = std::env::temp_dir().join(format!("oc-download-{}", std::process::id()));
        let file = download(package(Component::DsHidMini), &dir).expect("download");
        assert!(matches(&file, package(Component::DsHidMini).source.sha256));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Downloads VIIPER's zip and unpacks it into a temporary folder, without running anything.
    /// Needs the network: `cargo test -p open-controller-core -- --ignored unpacks_the_viiper`.
    #[test]
    #[ignore]
    fn unpacks_the_viiper_server() {
        let dir = std::env::temp_dir().join(format!("oc-viiper-{}", std::process::id()));
        let p = package(Component::Viiper);
        let zip = download(p, &dir.join("download")).expect("download");
        unpack(&zip, &dir.join("viiper"), p.version).expect("unpack");
        assert!(is_pinned_server(&dir.join("viiper").join("viiper.exe")));
        assert!(dir.join("viiper").join("licenses.txt").is_file(), "the licenses go with it");
        assert_eq!(std::fs::read_to_string(dir.join("viiper").join(VERSION_FILE)).unwrap(), "0.8.2");
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
