//! One instance at a time, and signals between instances. The first instance holds a lock on a
//! file in the user's runtime directory and listens on a datagram socket per signal ("show" to
//! bring the window forward, "quit"); later starts find the lock taken and send the signal.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Where the user's sockets and locks live: `$XDG_RUNTIME_DIR` (private to the user, emptied at
/// sign-out), else a private directory under the temporary one (per user on macOS already).
pub fn runtime_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|d| d.is_dir()) {
        return d;
    }
    let dir = std::env::temp_dir().join(format!("open-controller-{}", unsafe { libc::getuid() }));
    let _ = std::fs::DirBuilder::new().mode(0o700).create(&dir);
    dir
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
    runtime_dir().join(if suffix.is_empty() { format!("{name}.lock") } else { format!("{name}.{suffix}") })
}

fn try_lock(name: &str) -> Option<File> {
    let f = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(path(name, "")).ok()?;
    (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(f)
}

/// `name` names the program, such as `io.github.brunovncs.open-controller`. A second start
/// signals the first one's "show" event.
pub fn claim(name: &str) -> Instance {
    let Some(lock) = try_lock(name) else {
        signal(name, "show");
        return Instance::Second;
    };
    match event(name, "show") {
        Some(show) => Instance::First { _mutex: Lock(lock), show },
        None => Instance::Second,
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

/// Waits until the running instance has exited, up to `timeout`. True if it did.
pub fn wait_gone(name: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if try_lock(name).is_some() {
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
