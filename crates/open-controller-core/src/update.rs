//! Whether a newer release is out, asked of GitHub's API when the window opens (unless the user
//! turned it off), and on Windows fetching its installer. `curl` does the requests: it ships with
//! Windows 10 and later, macOS and every desktop Linux, so no TLS stack is built in.

#[cfg(windows)]
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RELEASES_API: &str = "https://api.github.com/repos/Brunovncs/OpenController/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/Brunovncs/OpenController/releases/latest";

/// The latest release, as far as an update needs to know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// Its page on GitHub, with the notes and every download.
    pub page: String,
    /// The Windows installer and the file holding its SHA-256, when the release has them.
    pub setup: Option<(String, String)>,
}

/// This version, or `OPEN_CONTROLLER_PRETEND_VERSION` to try updating from an older one.
pub fn current() -> String {
    std::env::var("OPEN_CONTROLLER_PRETEND_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

fn numbers(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
}

/// `candidate` is a later version than `current` (1.10.0 after 1.9.2; a pre-release suffix is
/// ignored).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (numbers(candidate), numbers(current));
    !a.is_empty() && a > b
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

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let out = curl()
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "20"])
        .args(["--header", "Accept: application/vnd.github+json", "--user-agent", "open-controller"])
        .arg(url)
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if out.status.success() { Ok(out.stdout) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

pub fn parse(json: &[u8]) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("the release has no tag")?;
    let assets = v["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |suffix: &str| {
        assets
            .iter()
            .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(suffix)))
            .and_then(|a| a["browser_download_url"].as_str())
            .map(String::from)
    };
    let setup = url_of("-windows-x64-setup.exe").zip(url_of("-windows-x64-setup.exe.sha256"));
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        page: v["html_url"].as_str().unwrap_or(RELEASES_PAGE).to_string(),
        setup,
    })
}

/// The latest release, if it is newer than this one.
pub fn check() -> Result<Option<Release>, String> {
    let r = parse(&fetch(RELEASES_API)?)?;
    Ok(is_newer(&r.version, &current()).then_some(r))
}

/// Downloads the release's installer into `dir` and checks it against the SHA-256 published with
/// it, so a broken or swapped download never runs.
#[cfg(windows)]
pub fn download_setup(r: &Release, dir: &Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let (url, sha_url) = r.setup.as_ref().ok_or("this release has no installer")?;
    let expected = String::from_utf8_lossy(&fetch(sha_url)?).split_whitespace().next().unwrap_or_default().to_lowercase();
    if expected.len() != 64 {
        return Err("the installer's checksum could not be read".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = dir.join(url.rsplit('/').next().unwrap_or("open-controller-setup.exe"));
    let bytes = fetch(url)?;
    let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got != expected {
        return Err("the downloaded installer does not match its checksum".into());
    }
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    Ok(file)
}

/// Starts the installer without its wizard. It closes what is still running, replaces the
/// programs in place, keeps the settings and starts OpenController again.
#[cfg(windows)]
pub fn run_setup(file: &Path) -> Result<(), String> {
    Command::new(file)
        .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"])
        .spawn()
        .map(drop)
        .map_err(|e| format!("could not start the installer: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }

    #[test]
    fn reads_a_release() {
        let json = br#"{"tag_name":"v0.3.0","html_url":"https://example/v0.3.0","assets":[
            {"name":"open-controller-0.3.0-windows-x64.zip","browser_download_url":"https://example/zip"},
            {"name":"open-controller-0.3.0-windows-x64-setup.exe","browser_download_url":"https://example/setup"},
            {"name":"open-controller-0.3.0-windows-x64-setup.exe.sha256","browser_download_url":"https://example/sha"}]}"#;
        let r = parse(json).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.page, "https://example/v0.3.0");
        assert_eq!(r.setup, Some(("https://example/setup".into(), "https://example/sha".into())));
        let without = parse(br#"{"tag_name":"v0.2.0","assets":[]}"#).unwrap();
        assert_eq!(without.setup, None);
        assert_eq!(without.page, RELEASES_PAGE);
    }
}
