//! The link between the resident process (engine and tray icon) and the window, which runs as
//! a separate process only while it is open. A named pipe local to the user's session, which
//! only that user can open, carries newline-separated JSON messages both ways.

use crate::engine::{PadKey, Snapshot};
use crate::profile::Edit;
use crate::win::{Handle, wide};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io;
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE,
    GetLastError, HANDLE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    WaitNamedPipeW,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{CreateEventW, GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};

/// What the resident process tells the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToWindow {
    Snapshot(Snapshot),
    Prefs(Prefs),
}

/// What the window asks of the resident process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToTray {
    SetHiding(bool),
    SetAutostart(bool),
    Identify(PadKey),
    PowerOff(PadKey),
    /// Changes the profiles of the controllers whose settings are kept under `store`.
    Edit {
        store: String,
        edit: Edit,
    },
    Quit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prefs {
    pub hide_originals: bool,
    pub autostart: bool,
}

/// One pipe per Windows session, so two users signed in at once each have their own.
fn pipe_name() -> String {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    format!(r"\\.\pipe\io.github.brunovncs.open-controller.{session}")
}

/// The current user's SID as a string (`S-1-5-21-...`).
fn user_sid() -> Option<String> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let token = Handle(token);
        let mut buf = vec![0u8; 256];
        let mut n = 0u32;
        if GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n) == 0 {
            return None;
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
            return None;
        }
        let len = (0..).take_while(|&i| *s.add(i) != 0).count();
        let out = String::from_utf16_lossy(std::slice::from_raw_parts(s, len));
        LocalFree(s.cast());
        Some(out)
    }
}

/// Owner-only access: the user and SYSTEM, nobody else on the machine.
struct Security(PSECURITY_DESCRIPTOR);

impl Security {
    fn for_user() -> Option<Security> {
        let sddl = format!("D:P(A;;GA;;;{})(A;;GA;;;SY)", user_sid()?);
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        let ok =
            unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(wide(&sddl).as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) };
        (ok != 0).then_some(Security(sd))
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}

/// An overlapped request with its own event, so reading and writing can wait on the same
/// handle from two threads.
struct Io {
    ov: OVERLAPPED,
    event: Handle,
}

unsafe impl Send for Io {}

impl Io {
    fn new() -> Io {
        let e = unsafe { CreateEventW(null(), 1, 0, null()) };
        Io { ov: unsafe { std::mem::zeroed() }, event: Handle::valid(e).expect("CreateEventW failed") }
    }

    fn reset(&mut self) -> *mut OVERLAPPED {
        self.ov = unsafe { std::mem::zeroed() };
        self.ov.hEvent = self.event.0;
        &mut self.ov
    }

    /// Waits for the request started with `started` (the call's return value).
    fn finish(&mut self, h: HANDLE, started: i32) -> io::Result<u32> {
        if started == 0 {
            let e = unsafe { GetLastError() };
            if e != ERROR_IO_PENDING {
                return Err(io::Error::from_raw_os_error(e as i32));
            }
        }
        let mut n = 0u32;
        if unsafe { GetOverlappedResult(h, &self.ov, &mut n, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(n)
    }
}

/// One end of the pipe. Messages are sent from any thread; one thread reads.
pub struct Pipe {
    handle: Handle,
    write: Mutex<Io>,
    server: bool,
}

pub fn is_disconnect(e: &io::Error) -> bool {
    e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) || e.kind() == io::ErrorKind::BrokenPipe || e.kind() == io::ErrorKind::UnexpectedEof
}

impl Pipe {
    /// Creates the server end. Fails if another process already owns the name.
    pub fn create() -> io::Result<Pipe> {
        Self::create_at(&pipe_name())
    }

    fn create_at(name: &str) -> io::Result<Pipe> {
        let security = Security::for_user();
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.as_ref().map(|s| s.0).unwrap_or(null_mut()),
            bInheritHandle: 0,
        };
        let h = unsafe {
            CreateNamedPipeW(
                wide(name).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                64 * 1024,
                64 * 1024,
                0,
                &attrs,
            )
        };
        let handle = Handle::valid(h).ok_or_else(io::Error::last_os_error)?;
        Ok(Pipe { handle, write: Mutex::new(Io::new()), server: true })
    }

    /// Waits for the window to connect.
    pub fn accept(&self) -> io::Result<()> {
        let mut io = Io::new();
        let ov = io.reset();
        let started = unsafe { ConnectNamedPipe(self.handle.0, ov) };
        if started == 0 && unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
            return Ok(());
        }
        io.finish(self.handle.0, started).map(drop)
    }

    /// Drops the current client so the next one can connect.
    pub fn disconnect(&self) {
        unsafe { DisconnectNamedPipe(self.handle.0) };
    }

    /// Opens the client end. `Ok(None)` when no resident process is running.
    pub fn connect() -> io::Result<Option<Pipe>> {
        Self::connect_at(&pipe_name())
    }

    fn connect_at(name: &str) -> io::Result<Option<Pipe>> {
        let name = wide(name);
        for _ in 0..10 {
            let h = unsafe {
                CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, null(), OPEN_EXISTING, FILE_FLAG_OVERLAPPED, null_mut())
            };
            if let Some(handle) = Handle::valid(h) {
                return Ok(Some(Pipe { handle, write: Mutex::new(Io::new()), server: false }));
            }
            match unsafe { GetLastError() } {
                ERROR_FILE_NOT_FOUND => return Ok(None),
                ERROR_PIPE_BUSY => unsafe {
                    WaitNamedPipeW(name.as_ptr(), 1000);
                },
                e => return Err(io::Error::from_raw_os_error(e as i32)),
            }
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, "the pipe stayed busy"))
    }

    pub fn send<T: Serialize>(&self, msg: &T) -> io::Result<()> {
        let mut line = serde_json::to_vec(msg).map_err(io::Error::other)?;
        line.push(b'\n');
        let mut io = self.write.lock().map_err(|_| io::Error::other("poisoned"))?;
        let mut sent = 0;
        while sent < line.len() {
            let ov = io.reset();
            let rest = &line[sent..];
            let started = unsafe { WriteFile(self.handle.0, rest.as_ptr(), rest.len() as u32, null_mut(), ov) };
            sent += io.finish(self.handle.0, started)? as usize;
        }
        Ok(())
    }

    pub fn reader(&self) -> Reader<'_> {
        Reader { pipe: self, io: Io::new(), buf: Vec::with_capacity(16 * 1024), start: 0 }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        if self.server {
            self.disconnect();
        }
    }
}

/// Reads whole messages from a pipe.
pub struct Reader<'a> {
    pipe: &'a Pipe,
    io: Io,
    buf: Vec<u8>,
    start: usize,
}

impl Reader<'_> {
    /// The next message. Unreadable lines are skipped; an error means the other side is gone.
    pub fn recv<T: DeserializeOwned>(&mut self) -> io::Result<T> {
        loop {
            if let Some(end) = self.buf[self.start..].iter().position(|&b| b == b'\n') {
                let line = &self.buf[self.start..self.start + end];
                self.start += end + 1;
                if let Ok(msg) = serde_json::from_slice(line) {
                    return Ok(msg);
                }
                continue;
            }
            self.buf.drain(..self.start);
            self.start = 0;
            let len = self.buf.len();
            self.buf.resize(len + 16 * 1024, 0);
            let ov = self.io.reset();
            let started =
                unsafe { ReadFile(self.pipe.handle.0, self.buf[len..].as_mut_ptr(), (self.buf.len() - len) as u32, null_mut(), ov) };
            let n = self.io.finish(self.pipe.handle.0, started);
            match n {
                Ok(0) => {
                    self.buf.truncate(len);
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok(n) => self.buf.truncate(len + n as usize),
                Err(e) => {
                    self.buf.truncate(len);
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Snapshot;

    #[test]
    fn messages_round_trip_over_a_real_pipe() {
        let name = format!("{}.test{}", pipe_name(), std::process::id());
        let server_name = name.clone();
        let server = std::thread::spawn(move || {
            let pipe = Pipe::create_at(&server_name).expect("create");
            pipe.accept().expect("accept");
            pipe.send(&ToWindow::Prefs(Prefs { hide_originals: true, autostart: false })).unwrap();
            // A large message, split across several reads.
            let big = Snapshot { sdl_error: Some("x".repeat(100_000)), ..Snapshot::default() };
            pipe.send(&ToWindow::Snapshot(big)).unwrap();
            let mut r = pipe.reader();
            let got: ToTray = r.recv().unwrap();
            assert_eq!(got, ToTray::Identify(PadKey::Slot(3)));
            let gone = r.recv::<ToTray>().unwrap_err();
            assert!(is_disconnect(&gone), "{gone:?}");
        });
        let client = loop {
            if let Some(p) = Pipe::connect_at(&name).unwrap() {
                break p;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let mut r = client.reader();
        assert_eq!(r.recv::<ToWindow>().unwrap(), ToWindow::Prefs(Prefs { hide_originals: true, autostart: false }));
        match r.recv::<ToWindow>().unwrap() {
            ToWindow::Snapshot(s) => assert_eq!(s.sdl_error.map(|e| e.len()), Some(100_000)),
            other => panic!("{other:?}"),
        }
        client.send(&ToTray::Identify(PadKey::Slot(3))).unwrap();
        drop(r);
        drop(client);
        server.join().unwrap();
    }
}
