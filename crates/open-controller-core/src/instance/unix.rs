//! One instance at a time, and signals between instances. The first instance holds a lock on a
//! file in the user's runtime directory and listens on a datagram socket per signal ("show" to
//! bring the window forward, "quit"); later starts find the lock taken and send the signal.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Where the user's sockets and locks live, the first of `candidates` that is the user's alone.
/// When none is, a path that cannot exist, so a start fails rather than trust a folder someone
/// else can reach.
pub fn runtime_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let uid = unsafe { libc::getuid() };
        let candidates = candidates(uid);
        if let Some(d) = candidates.iter().find(|d| private_dir(d, uid)) {
            return d.clone();
        }
        eprintln!("open-controller: no private folder for its sockets; tried {candidates:?}");
        PathBuf::from("/dev/null/open-controller")
    })
    .clone()
}

/// `$XDG_RUNTIME_DIR` (private to the user, emptied at sign-out), else a folder of the user's in
/// /tmp, the temporary directory or the cache.
#[cfg(not(target_os = "macos"))]
fn candidates(uid: u32) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).into_iter().collect();
    out.push(PathBuf::from(format!("/tmp/open-controller-{uid}")));
    if let Some(tmp) = std::env::var_os("TMPDIR").filter(|t| Path::new(t) != Path::new("/tmp")) {
        out.push(Path::new(&tmp).join(format!("open-controller-{uid}")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        out.push(Path::new(&home).join(".cache/open-controller"));
    }
    out
}

/// A folder in the user's Library, which nobody else can create first and no periodic cleanup
/// empties, as it does /tmp after three days. Its name is short and so are the files in it
/// (`file_name`), since a socket path may have 104 bytes there. The per-user temporary
/// directory is the fallback.
#[cfg(target_os = "macos")]
fn candidates(_: u32) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        out.push(Path::new(&home).join("Library/Application Support/OpenController"));
    }
    out.push(std::env::temp_dir().join("open-controller"));
    out
}

/// Creates `dir` if needed, and whether it is a directory of `uid`'s that nobody else can reach.
/// A directory of the user's that others can reach is closed first.
fn private_dir(dir: &Path, uid: u32) -> bool {
    let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir);
    let Ok(m) = std::fs::symlink_metadata(dir) else { return false };
    if !m.is_dir() || m.uid() != uid {
        return false;
    }
    m.mode() & 0o077 == 0 || std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).is_ok()
}

/// macOS keeps the names short, without the reverse-DNS prefix every name shares
/// (`ui.show`, `lock`); elsewhere they stay as earlier versions made them, so a new start still
/// finds an older instance.
fn file_name(name: &str, suffix: &str) -> String {
    let what = if suffix.is_empty() { "lock" } else { suffix };
    if cfg!(target_os = "macos") {
        let rest = name.strip_prefix("io.github.brunovncs.open-controller").map(|r| r.trim_start_matches('.')).unwrap_or(name);
        return if rest.is_empty() { what.to_string() } else { format!("{rest}.{what}") };
    }
    format!("{name}.{what}")
}

/// Holds the instance lock while alive.
pub struct Lock(#[allow(dead_code)] File);

/// A signal another start can send.
pub struct Event {
    socket: UnixDatagram,
    path: PathBuf,
}

impl Drop for Event {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub enum Instance {
    First { _mutex: Lock, show: Event },
    Second,
}

fn path(name: &str, suffix: &str) -> PathBuf {
    runtime_dir().join(file_name(name, suffix))
}

fn try_lock(name: &str) -> Option<File> {
    let path = path(name, "");
    // An instance on its way out removes the file while it still holds the lock, so the lock
    // taken may be on a file that is gone; then it counts for nothing, and the new file decides.
    for _ in 0..3 {
        let f = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(&path).ok()?;
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return None;
        }
        let (Ok(held), Ok(now)) = (f.metadata(), std::fs::metadata(&path)) else { continue };
        if held.dev() == now.dev() && held.ino() == now.ino() {
            return Some(f);
        }
    }
    None
}

/// `name` names the program, such as `io.github.brunovncs.open-controller`. A second start
/// signals the first one's "show" event, unless the login item started it: that happens when
/// the user turns starting at sign-in on with the app already running, and shows nothing.
pub fn claim(name: &str) -> Instance {
    let Some(lock) = try_lock(name) else {
        if !started_at_sign_in(name) {
            signal(name, "show");
        }
        return Instance::Second;
    };
    #[cfg(target_os = "macos")]
    quit_legacy(name);
    match event(name, "show") {
        Some(show) => Instance::First { _mutex: Lock(lock), show },
        None => Instance::Second,
    }
}

/// launchd names the job that started a program in `XPC_SERVICE_NAME`; the login item's label
/// starts with the program's name. On Linux the login entries never start a second copy.
fn started_at_sign_in(name: &str) -> bool {
    cfg!(target_os = "macos") && std::env::var("XPC_SERVICE_NAME").is_ok_and(|job| job.starts_with(name))
}

/// Versions up to 0.8.8 kept their files in /tmp on macOS. One still running from before an
/// update is asked to quit, so two copies do not type every key twice.
#[cfg(target_os = "macos")]
fn quit_legacy(name: &str) {
    let uid = unsafe { libc::getuid() };
    let old = PathBuf::from(format!("/tmp/open-controller-{uid}"));
    let ours = std::fs::symlink_metadata(&old).is_ok_and(|m| m.is_dir() && m.uid() == uid);
    if ours && old != runtime_dir() {
        let _ = UnixDatagram::unbound().and_then(|s| s.send_to(b"!", old.join(format!("{name}.quit"))));
    }
}

/// Creates a signal other starts can send.
pub fn event(name: &str, what: &str) -> Option<Event> {
    let path = path(name, what);
    let _ = std::fs::remove_file(&path);
    let socket = UnixDatagram::bind(&path).ok()?;
    Some(Event { socket, path })
}

/// Sends the running instance a signal. False when no instance is listening.
pub fn signal(name: &str, what: &str) -> bool {
    UnixDatagram::unbound().and_then(|s| s.send_to(b"!", path(name, what))).is_ok()
}

/// Whether an instance holds the lock, including one still starting up. Asking leaves no lock
/// file behind.
pub fn running(name: &str) -> bool {
    match OpenOptions::new().write(true).open(path(name, "")) {
        Ok(f) => (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }) != 0,
        Err(_) => false,
    }
}

/// Removes the first instance's signal sockets and lock file, on its way out and after everything
/// else has stopped: the threads that listen for the signals never end, so nothing else would.
pub fn remove_files(name: &str, signals: &[&str]) {
    for what in signals {
        let _ = std::fs::remove_file(path(name, what));
    }
    // Last, since a new start can take over as soon as the lock file is gone.
    let _ = std::fs::remove_file(path(name, ""));
}

/// Waits until the running instance has exited, up to `timeout`. True if it did.
pub fn wait_gone(name: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !running(name) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Calls `f` each time the signal arrives.
pub fn on_signal(event: Event, f: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("instance-signals".into())
        .spawn(move || {
            let mut buf = [0u8; 8];
            while event.socket.recv(&mut buf).is_ok() {
                f();
            }
        })
        .expect("could not start the signal thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_removes_the_lock_and_signals() {
        let name = format!("io.github.brunovncs.open-controller.test{}", std::process::id());
        let Instance::First { _mutex, show } = claim(&name) else { panic!("not first") };
        let quit = event(&name, "quit").expect("quit event");
        assert!(running(&name));
        remove_files(&name, &["show", "quit"]);
        for what in ["show", "quit", ""] {
            assert!(!path(&name, what).exists(), "{what:?} is left");
        }
        drop((_mutex, show, quit));
        assert!(!running(&name));
        assert!(!path(&name, "").exists(), "asking left a lock file");
    }

    #[test]
    fn a_lock_on_a_removed_file_counts_for_nothing() {
        let name = format!("io.github.brunovncs.open-controller.race{}", std::process::id());
        let old = try_lock(&name).expect("first lock");
        std::fs::remove_file(path(&name, "")).unwrap();
        let new = try_lock(&name).expect("the file that is there now is free");
        assert!(try_lock(&name).is_none());
        drop((old, new));
        let _ = std::fs::remove_file(path(&name, ""));
    }

    /// macOS takes socket paths of up to 104 bytes, Linux 108.
    #[test]
    fn the_real_sockets_fit() {
        for name in ["io.github.brunovncs.open-controller", "io.github.brunovncs.open-controller.ui"] {
            for what in ["show", "quit"] {
                let p = path(name, what);
                assert!(p.as_os_str().len() < 104, "{} is {} bytes", p.display(), p.as_os_str().len());
            }
        }
    }
}
