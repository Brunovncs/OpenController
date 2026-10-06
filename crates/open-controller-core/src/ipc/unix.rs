//! A Unix socket in the user's runtime directory, which only that user can enter.

use crate::instance::runtime_dir;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Mutex;

pub(super) fn pipe_name() -> String {
    runtime_dir().join("open-controller.sock").to_string_lossy().into_owned()
}

pub struct Pipe {
    listener: Option<(UnixListener, PathBuf)>,
    /// The connection: the client's own, or the server's current client.
    stream: Mutex<Option<UnixStream>>,
}

pub fn is_disconnect(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::NotConnected
    )
}

impl Pipe {
    /// Creates the server end. Fails if another process is already listening there.
    pub fn create() -> io::Result<Pipe> {
        Self::create_at(&pipe_name())
    }

    pub(super) fn create_at(name: &str) -> io::Result<Pipe> {
        let path = PathBuf::from(name);
        if UnixStream::connect(&path).is_ok() {
            return Err(io::ErrorKind::AddrInUse.into());
        }
        // Left by a process that did not exit cleanly.
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        Ok(Pipe { listener: Some((listener, path)), stream: Mutex::new(None) })
    }

    /// Waits for the window to connect.
    pub fn accept(&self) -> io::Result<()> {
        let (listener, _) = self.listener.as_ref().ok_or(io::ErrorKind::Unsupported)?;
        let (stream, _) = listener.accept()?;
        *self.stream.lock().map_err(|_| io::Error::other("poisoned"))? = Some(stream);
        Ok(())
    }

    /// Drops the current client so the next one can connect.
    pub fn disconnect(&self) {
        if let Ok(mut s) = self.stream.lock()
            && let Some(s) = s.take()
        {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    /// Opens the client end. `Ok(None)` when no resident process is running.
    pub fn connect() -> io::Result<Option<Pipe>> {
        Self::connect_at(&pipe_name())
    }

    pub(super) fn connect_at(name: &str) -> io::Result<Option<Pipe>> {
        match UnixStream::connect(name) {
            Ok(s) => Ok(Some(Pipe { listener: None, stream: Mutex::new(Some(s)) })),
            Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn send<T: Serialize>(&self, msg: &T) -> io::Result<()> {
        let mut line = serde_json::to_vec(msg).map_err(io::Error::other)?;
        line.push(b'\n');
        let guard = self.stream.lock().map_err(|_| io::Error::other("poisoned"))?;
        let mut stream = guard.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        stream.write_all(&line)
    }

    pub fn reader(&self) -> Reader<'_> {
        let stream = self.stream.lock().ok().and_then(|s| s.as_ref().and_then(|s| s.try_clone().ok()));
        Reader { stream, buf: Vec::with_capacity(16 * 1024), start: 0, _pipe: PhantomData }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        self.disconnect();
        if let Some((_, path)) = &self.listener {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Reads whole messages from a pipe.
pub struct Reader<'a> {
    stream: Option<UnixStream>,
    buf: Vec<u8>,
    start: usize,
    _pipe: PhantomData<&'a Pipe>,
}

impl Reader<'_> {
    /// The next message. Unreadable lines are skipped; an error means the other side is gone.
    pub fn recv<T: DeserializeOwned>(&mut self) -> io::Result<T> {
        let stream = self.stream.as_mut().ok_or(io::ErrorKind::NotConnected)?;
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
            match stream.read(&mut self.buf[len..]) {
                Ok(0) => {
                    self.buf.truncate(len);
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok(n) => self.buf.truncate(len + n),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => self.buf.truncate(len),
                Err(e) => {
                    self.buf.truncate(len);
                    return Err(e);
                }
            }
        }
    }
}
