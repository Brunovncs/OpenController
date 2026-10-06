//! One instance at a time, and signals between instances. The first instance owns a named
//! mutex; later starts find it and signal a named event instead ("show" to bring the window
//! forward, "quit" for the uninstaller), then exit.

use crate::win::{Handle, wide};
use std::ptr::null;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    WaitForSingleObject,
};

pub enum Instance {
    First { _mutex: Handle, show: Handle },
    Second,
}

fn object(name: &str, suffix: &str) -> Vec<u16> {
    if suffix.is_empty() { wide(format!(r"Local\{name}")) } else { wide(format!(r"Local\{name}.{suffix}")) }
}

/// `name` is a session-local object name, such as `io.github.brunovncs.open-controller`.
/// A second start signals the first one's "show" event.
pub fn claim(name: &str) -> Instance {
    unsafe {
        let mutex = Handle::valid(CreateMutexW(null(), 0, object(name, "").as_ptr()));
        if mutex.is_none() || GetLastError() == ERROR_ALREADY_EXISTS {
            signal(name, "show");
            return Instance::Second;
        }
        match (mutex, event(name, "show")) {
            (Some(m), Some(show)) => Instance::First { _mutex: m, show },
            _ => Instance::Second,
        }
    }
}

/// Creates a named auto-reset event other starts can signal.
pub fn event(name: &str, what: &str) -> Option<Handle> {
    Handle::valid(unsafe { CreateEventW(null(), 0, 0, object(name, what).as_ptr()) })
}

/// Signals the running instance's event. False when no instance is running.
pub fn signal(name: &str, what: &str) -> bool {
    match Handle::valid(unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, object(name, what).as_ptr()) }) {
        Some(ev) => unsafe { SetEvent(ev.0) != 0 },
        None => false,
    }
}

/// Waits until the running instance has exited, up to `timeout`. True if it did.
pub fn wait_gone(name: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if Handle::valid(unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, object(name, "").as_ptr()) }).is_none() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Calls `f` each time the event is signalled.
pub fn on_signal(event: Handle, f: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("instance-signals".into())
        .spawn(move || {
            // Moved whole: the wrapper is Send, its raw field is not.
            let event = event;
            loop {
                unsafe { WaitForSingleObject(event.0, INFINITE) };
                f();
            }
        })
        .expect("could not start the signal thread");
}
