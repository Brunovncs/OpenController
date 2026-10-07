//! A client for VIIPER, the experimental way to make the virtual Xbox 360 controllers on Windows.
//!
//! VIIPER's server (`viiper.exe`, GPL-3, run as a program of its own) emulates USB devices and
//! serves them over USB/IP; the usbip-win2 driver attaches them, so Windows finds a wired Xbox 360
//! controller on a virtual USB host controller and gives it its usual driver. The server is
//! started here, listening on localhost only, inside a Job Object that ends it with this process:
//! if OpenController crashes, the server goes with it and USB/IP drops its controllers.
//!
//! Its API is plain TCP, without a password on localhost (`internal/server/api` in VIIPER
//! v0.8.2). A request is `path[ payload]\0` on a new connection, answered with one line of JSON,
//! or of RFC 7807 problem JSON when it fails, and the connection closes. A device's stream starts
//! with `bus/{bus}/{device}\0` and then carries 20-byte input reports one way and 2-byte rumble
//! the other. The server removes a device whose stream is not open within its handler timeout,
//! counted from before the attach, and a bus left empty after that long.

use crate::mapping::XusbReport;
use crate::vigem::Feedback;
use crate::win::{Handle, interface_paths};
use crossbeam_channel::Sender;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_GEN_FAILURE, GetLastError};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::core::GUID;

/// The device interface of usbip-win2's virtual host controller, which the server attaches
/// devices to (`autoattach_windows.go`).
const GUID_DEVINTERFACE_USBIP_VHCI: GUID = GUID::from_u128(0xB4030C06_DC5F_4FCC_87EB_E5515A0935C0);
/// How long a new device may go without its stream before the server removes it. The timer
/// starts before the attach, so this has to cover it; it also bounds how long a controller stays
/// if this process loses its connection without the server going too.
const HANDLER_TIMEOUT: &str = "3s";
/// How long the server may take to answer its first ping.
const START_WAIT: Duration = Duration::from_secs(5);
const CONNECT_WAIT: Duration = Duration::from_secs(1);
const REQUEST_WAIT: Duration = Duration::from_secs(3);
/// Adding a device waits for usbip-win2 to attach it.
const ADD_WAIT: Duration = Duration::from_secs(10);
/// A stream the server refuses is answered at once with a problem; one it takes is silent.
const STREAM_CHECK: Duration = Duration::from_millis(100);
/// A report the server does not take this fast means it has stopped reading.
const WRITE_WAIT: Duration = Duration::from_millis(100);
/// How often a feedback thread waiting for rumble checks whether its stream was closed here.
const LISTEN_POLL: Duration = Duration::from_millis(100);
/// Serials are handed out from 1, lowest free first, as ViGEmBus does; the XInput marker
/// (`xinput::marker`) tells up to this many apart.
const MAX_PADS: u32 = 64;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    /// `viiper.exe` is not where the settings install it.
    ServerMissing,
    /// usbip-win2's virtual host controller is not there: not installed, or installed and
    /// waiting for Windows to restart.
    UsbipMissing,
    /// The `viiper.exe` there is not the build this version was made with.
    ServerMismatch,
    /// The server did not start, or did not answer.
    Start(String),
    /// The server refused a request: its HTTP-like status and why.
    Problem(u16, String),
    Io(String),
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BusError::ServerMissing => f.write_str("the VIIPER server is not installed"),
            BusError::UsbipMissing => f.write_str("usbip-win2 is not installed, or Windows has not restarted since"),
            BusError::ServerMismatch => f.write_str("the VIIPER server is not the version this OpenController knows"),
            BusError::Start(e) => write!(f, "the VIIPER server did not start: {e}"),
            BusError::Problem(status, detail) => write!(f, "VIIPER: {detail} ({status})"),
            BusError::Io(e) => write!(f, "VIIPER: {e}"),
        }
    }
}

fn io(e: std::io::Error) -> BusError {
    BusError::Io(e.to_string())
}

/// The Win32 error behind a failure, for the bus's `Result<_, u32>` requests.
fn code(e: &std::io::Error) -> u32 {
    e.raw_os_error().map_or(ERROR_GEN_FAILURE, |c| c as u32)
}

/// Where the settings install the server: a folder next to OpenController's own programs.
pub fn server_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default().join("viiper")
}

pub fn server_exe() -> PathBuf {
    server_dir().join("viiper.exe")
}

/// usbip-win2's virtual host controller, if the driver is installed and running.
pub fn vhci_path() -> Option<String> {
    interface_paths(&GUID_DEVINTERFACE_USBIP_VHCI).into_iter().next()
}

/// The server's API, one connection per request.
#[derive(Clone)]
struct Api {
    addr: SocketAddr,
    /// How long a request may take, `ADD_WAIT` for an add.
    wait: Duration,
}

impl Api {
    fn request(&self, path: &str, payload: Option<&str>, wait: Duration) -> Result<Value, BusError> {
        let mut s = TcpStream::connect_timeout(&self.addr, CONNECT_WAIT).map_err(io)?;
        let _ = s.set_nodelay(true);
        s.set_read_timeout(Some(wait)).map_err(io)?;
        s.set_write_timeout(Some(wait)).map_err(io)?;
        let mut req = path.to_string();
        if let Some(p) = payload {
            req.push(' ');
            req.push_str(p);
        }
        req.push('\0');
        s.write_all(req.as_bytes()).map_err(io)?;
        let mut out = Vec::new();
        s.read_to_end(&mut out).map_err(io)?;
        answer(&out)
    }

    /// The server's name and version.
    fn ping(&self) -> Result<(String, String), BusError> {
        let v = self.request("ping", None, self.wait)?;
        let field = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        field("server").zip(field("version")).ok_or_else(|| unexpected(&v))
    }

    fn create_bus(&self) -> Result<u32, BusError> {
        let v = self.request("bus/create", None, self.wait)?;
        v.get("busId").and_then(Value::as_u64).map(|b| b as u32).ok_or_else(|| unexpected(&v))
    }

    /// Adds a wired Xbox 360 controller and waits for it to be attached; returns its device id.
    fn add(&self, bus: u32) -> Result<String, BusError> {
        let v = self.request(&format!("bus/{bus}/add"), Some(r#"{"type":"xbox360"}"#), self.wait.max(ADD_WAIT))?;
        v.get("devId").and_then(Value::as_str).map(str::to_string).ok_or_else(|| unexpected(&v))
    }

    fn remove(&self, bus: u32, device: &str) -> Result<(), BusError> {
        self.request(&format!("bus/{bus}/remove"), Some(device), self.wait).map(drop)
    }

    /// Opens a device's stream. The server answers a stream it refuses with a problem and
    /// closes it; one it takes stays silent until there is rumble.
    fn stream(&self, bus: u32, device: &str) -> Result<TcpStream, BusError> {
        let mut s = TcpStream::connect_timeout(&self.addr, CONNECT_WAIT).map_err(io)?;
        let _ = s.set_nodelay(true);
        s.set_write_timeout(Some(WRITE_WAIT)).map_err(io)?;
        s.write_all(format!("bus/{bus}/{device}\0").as_bytes()).map_err(io)?;
        s.set_read_timeout(Some(STREAM_CHECK)).map_err(io)?;
        let mut first = [0u8; 1];
        match s.peek(&mut first) {
            Ok(0) => return Err(BusError::Io("the server closed the stream".into())),
            Ok(_) if first[0] == b'{' => {
                let mut out = Vec::new();
                let _ = s.read_to_end(&mut out);
                answer(&out)?;
            }
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(io(e)),
        }
        s.set_read_timeout(None).map_err(io)?;
        Ok(s)
    }
}

/// A request's answer: its JSON, or the problem the server reported.
fn answer(bytes: &[u8]) -> Result<Value, BusError> {
    let text = String::from_utf8_lossy(bytes);
    let v: Value = serde_json::from_str(text.trim_end()).map_err(|_| BusError::Io(format!("unexpected answer {:?}", text.trim_end())))?;
    if let (Some(status), Some(detail)) = (v.get("status").and_then(Value::as_u64), v.get("detail").and_then(Value::as_str))
        && v.get("title").is_some()
    {
        return Err(BusError::Problem(status as u16, detail.to_string()));
    }
    Ok(v)
}

fn unexpected(v: &Value) -> BusError {
    BusError::Io(format!("unexpected answer {v}"))
}

/// The 20 bytes the server takes for a report: the buttons widened to 32 bits, then the same
/// fields as `XUSB_REPORT`, then six reserved bytes (`device/xbox360/inputstate.go`).
fn wire(r: &XusbReport) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..4].copy_from_slice(&u32::from(r.buttons).to_le_bytes());
    b[4] = r.left_trigger;
    b[5] = r.right_trigger;
    b[6..8].copy_from_slice(&r.thumb_lx.to_le_bytes());
    b[8..10].copy_from_slice(&r.thumb_ly.to_le_bytes());
    b[10..12].copy_from_slice(&r.thumb_rx.to_le_bytes());
    b[12..14].copy_from_slice(&r.thumb_ry.to_le_bytes());
    b
}

/// The server process, ended with its Job Object: when this process exits or crashes, Windows
/// closes the job and the server goes too.
struct Server {
    child: Mutex<Child>,
    _job: Handle,
}

impl Server {
    fn spawn(exe: &Path) -> Result<(Server, SocketAddr), BusError> {
        let (api, usb) = free_ports().map_err(io)?;
        let dir = exe.parent().unwrap_or(Path::new("."));
        let mut cmd = Command::new(exe);
        // Settings from the environment (VIIPER_API_ADDR...) would override nothing set below,
        // but could turn on what is left at its default.
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().to_ascii_uppercase().starts_with("VIIPER_") {
                cmd.env_remove(k);
            }
        }
        cmd.current_dir(dir)
            .arg("--update-notify=none")
            .arg("--log.level=info")
            .arg(format!("--log.file={}", dir.join("viiper.log").display()))
            .arg("server")
            .arg(format!("--api.addr=127.0.0.1:{api}"))
            .arg(format!("--usb.addr=127.0.0.1:{usb}"))
            .arg(format!("--api.device-handler-connect-timeout={HANDLER_TIMEOUT}"))
            .arg("--api.auto-attach-local-client=true")
            .arg("--api.require-local-host-auth=false")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| BusError::Start(e.to_string()))?;
        match kill_on_close(&child) {
            Ok(job) => Ok((Server { child: Mutex::new(child), _job: job }, SocketAddr::from(([127, 0, 0, 1], api)))),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(BusError::Start(format!("no job object for it (error {e})")))
            }
        }
    }

    /// The exit code, once the server has ended.
    fn ended(&self) -> Option<i32> {
        let mut c = self.child.lock().ok()?;
        c.try_wait().ok().flatten().map(|s| s.code().unwrap_or(-1))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(c) = self.child.get_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Two ports nothing listens on, for the API and USB/IP. Another VIIPER, such as the one a
/// DS4Windows build runs, keeps its default ports.
fn free_ports() -> std::io::Result<(u16, u16)> {
    let a = TcpListener::bind("127.0.0.1:0")?;
    let b = TcpListener::bind("127.0.0.1:0")?;
    Ok((a.local_addr()?.port(), b.local_addr()?.port()))
}

fn kill_on_close(child: &Child) -> Result<Handle, u32> {
    unsafe {
        let job = Handle::valid(CreateJobObjectW(std::ptr::null(), std::ptr::null())).ok_or_else(|| GetLastError())?;
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set == 0 || AssignProcessToJobObject(job.0, child.as_raw_handle()) == 0 {
            return Err(GetLastError());
        }
        Ok(job)
    }
}

/// One virtual controller: where it is on the server and its stream.
struct Pad {
    bus: u32,
    device: String,
    stream: TcpStream,
    /// Set before this side closes the stream, so its end is not taken for a failure.
    closing: AtomicBool,
}

impl Pad {
    fn close(&self) {
        self.closing.store(true, Ordering::Release);
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

pub struct Bus {
    api: Api,
    version: String,
    /// `None` when talking to a server started elsewhere (the tests' stand-in).
    server: Option<Server>,
    /// The bus this process adds its controllers to. The server removes a bus left empty for a
    /// while, so it is made again when it is gone.
    bus: Mutex<u32>,
    pads: Mutex<HashMap<u32, Arc<Pad>>>,
    /// One plug at a time, so two never take the same serial.
    plugging: Mutex<()>,
    /// A stream the server closed or stopped reading: the controller behind it is gone.
    broken: Arc<Mutex<Option<String>>>,
}

impl Bus {
    /// Starts the server at `exe` and connects to it. Needs usbip-win2 and the server this
    /// version was made with.
    pub fn start(exe: &Path) -> Result<Bus, BusError> {
        if !exe.is_file() {
            return Err(BusError::ServerMissing);
        }
        if vhci_path().is_none() {
            return Err(BusError::UsbipMissing);
        }
        if !crate::drivers::is_pinned_server(exe) {
            return Err(BusError::ServerMismatch);
        }
        let (server, addr) = Server::spawn(exe)?;
        let api = Api { addr, wait: REQUEST_WAIT };
        let until = Instant::now() + START_WAIT;
        loop {
            if let Some(code) = server.ended() {
                return Err(BusError::Start(format!("it ended with code {code}")));
            }
            match api.ping() {
                Ok(_) => return Bus::open(api, Some(server)),
                Err(e) if Instant::now() >= until => return Err(BusError::Start(e.to_string())),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    /// Connects to a server another process started, which this one does not end. For the
    /// experiments in `examples/windows/viiper.rs`.
    pub fn attach(addr: SocketAddr) -> Result<Bus, BusError> {
        Bus::open(Api { addr, wait: REQUEST_WAIT }, None)
    }

    /// Where the server's API listens.
    pub fn addr(&self) -> SocketAddr {
        self.api.addr
    }

    fn open(api: Api, server: Option<Server>) -> Result<Bus, BusError> {
        let (name, version) = api.ping()?;
        if name != "VIIPER" {
            return Err(BusError::Start(format!("{name} answered instead")));
        }
        let bus = api.create_bus()?;
        Ok(Bus {
            api,
            version,
            server,
            bus: Mutex::new(bus),
            pads: Mutex::new(HashMap::new()),
            plugging: Mutex::new(()),
            broken: Arc::new(Mutex::new(None)),
        })
    }

    /// The server's version, as its ping gives it.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Why the controllers stopped working: the server ended, or dropped a controller's stream.
    pub fn lost(&self) -> Option<String> {
        if let Some(code) = self.server.as_ref().and_then(Server::ended) {
            return Some(format!("the VIIPER server stopped (code {code})"));
        }
        self.broken.lock().ok().and_then(|b| b.clone())
    }

    /// Adds a virtual Xbox 360 controller, attached and with its stream open. Returns its serial.
    pub fn plug_x360(&self) -> Result<u32, BusError> {
        let _one = self.plugging.lock().map_err(|_| BusError::Io("poisoned".into()))?;
        let serial = {
            let pads = self.pads.lock().map_err(|_| BusError::Io("poisoned".into()))?;
            (1..=MAX_PADS).find(|s| !pads.contains_key(s)).ok_or_else(|| BusError::Io("too many controllers".into()))?
        };
        let (bus, device) = self.add()?;
        let stream = match self.api.stream(bus, &device) {
            Ok(s) => s,
            Err(e) => {
                let _ = self.api.remove(bus, &device);
                return Err(e);
            }
        };
        let pad = Arc::new(Pad { bus, device, stream, closing: AtomicBool::new(false) });
        if let Ok(mut pads) = self.pads.lock() {
            pads.insert(serial, pad);
        }
        Ok(serial)
    }

    /// Adds a device to this process's bus, making the bus again if the server removed it.
    fn add(&self) -> Result<(u32, String), BusError> {
        let mut bus = self.bus.lock().map_err(|_| BusError::Io("poisoned".into()))?;
        match self.api.add(*bus) {
            Err(BusError::Problem(404, _)) => {
                *bus = self.api.create_bus()?;
                self.api.add(*bus).map(|d| (*bus, d))
            }
            r => r.map(|d| (*bus, d)),
        }
    }

    pub fn unplug(&self, serial: u32) -> Result<(), u32> {
        let pad = self.pads.lock().ok().and_then(|mut p| p.remove(&serial)).ok_or(ERROR_FILE_NOT_FOUND)?;
        pad.close();
        match self.api.remove(pad.bus, &pad.device) {
            // Already gone with its stream.
            Ok(()) | Err(BusError::Problem(404, _)) => Ok(()),
            Err(_) => Err(ERROR_GEN_FAILURE),
        }
    }

    pub fn submit(&self, serial: u32, report: &XusbReport) -> Result<(), u32> {
        let pad = self.pads.lock().ok().and_then(|p| p.get(&serial).cloned()).ok_or(ERROR_FILE_NOT_FOUND)?;
        (&pad.stream).write_all(&wire(report)).map_err(|e| {
            // A report half written leaves the stream out of step: it cannot be used again.
            if !pad.closing.load(Ordering::Acquire) {
                self.fail(format!("a controller's stream failed: {e}"));
            }
            pad.close();
            code(&e)
        })
    }

    fn fail(&self, why: String) {
        if let Ok(mut b) = self.broken.lock() {
            b.get_or_insert(why);
        }
    }

    /// Starts a thread that passes on the game's rumble for one controller: two bytes, the large
    /// (left, low-frequency) motor then the small one. It ends when the stream closes.
    pub fn listen(&self, serial: u32, tx: Sender<Feedback>) -> JoinHandle<()> {
        let pad = self.pads.lock().ok().and_then(|p| p.get(&serial).cloned());
        let reader = pad.as_ref().and_then(|p| p.stream.try_clone().ok());
        let broken = self.broken.clone();
        std::thread::Builder::new()
            .name(format!("viiper-feedback-{serial}"))
            .spawn(move || {
                let (Some(pad), Some(mut reader)) = (pad, reader) else { return };
                // On Windows, shutting the stream down here does not wake a read already waiting
                // on it, so the read gives up now and then to see whether to stop.
                if reader.set_read_timeout(Some(LISTEN_POLL)).is_err() {
                    return;
                }
                let (mut b, mut have) = ([0u8; 2], 0);
                let end = loop {
                    match reader.read(&mut b[have..]) {
                        Ok(0) => break "it ended".to_string(),
                        Ok(n) => have += n,
                        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
                        Err(e) => break e.to_string(),
                    }
                    if pad.closing.load(Ordering::Acquire) {
                        return;
                    }
                    if have == 2 {
                        have = 0;
                        if tx.send(Feedback { serial, large_motor: b[0], small_motor: b[1] }).is_err() {
                            return;
                        }
                    }
                };
                if !pad.closing.load(Ordering::Acquire)
                    && let Ok(mut w) = broken.lock()
                {
                    w.get_or_insert(format!("the server closed a controller's stream: {end}"));
                }
            })
            .expect("could not start the feedback thread")
    }

    /// Closes every stream, which ends the feedback threads. The server removes the controllers
    /// once their handler timeout passes, or at once when it is ended.
    pub fn cancel_all(&self) {
        if let Ok(pads) = self.pads.lock() {
            for p in pads.values() {
                p.close();
            }
        }
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.cancel_all();
        // The server, if this process started it, is ended with its job.
        self.server.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::io::BufRead;
    use std::sync::mpsc;

    #[test]
    fn reports_go_out_in_the_servers_layout() {
        let r = XusbReport {
            buttons: 0x1000 | 0x0400 | 0x0001,
            left_trigger: 7,
            right_trigger: 255,
            thumb_lx: -32768,
            thumb_ly: 32767,
            thumb_rx: 1,
            thumb_ry: -2,
        };
        let b = wire(&r);
        assert_eq!(&b[0..4], &[0x01, 0x14, 0, 0]);
        assert_eq!((b[4], b[5]), (7, 255));
        assert_eq!(i16::from_le_bytes([b[6], b[7]]), -32768);
        assert_eq!(i16::from_le_bytes([b[8], b[9]]), 32767);
        assert_eq!(i16::from_le_bytes([b[10], b[11]]), 1);
        assert_eq!(i16::from_le_bytes([b[12], b[13]]), -2);
        assert_eq!(&b[14..], &[0; 6]);
    }

    #[test]
    fn answers_and_problems_are_told_apart() {
        let ok = answer(b"{\"server\":\"VIIPER\",\"version\":\"0.8.2\"}\n").unwrap();
        assert_eq!(ok["version"], "0.8.2");
        let problem = answer(b"{\"status\":409,\"title\":\"Conflict\",\"detail\":\"Failed to auto-attach device: x\"}\n");
        assert_eq!(problem, Err(BusError::Problem(409, "Failed to auto-attach device: x".into())));
        assert!(matches!(answer(b"garbage"), Err(BusError::Io(_))));
        assert!(matches!(answer(b""), Err(BusError::Io(_))));
    }

    #[test]
    fn missing_pieces_read_as_not_installed() {
        // The engine shows "not installed" errors as a driver to install, not as a failure.
        assert!(BusError::ServerMissing.to_string().contains("not installed"));
        assert!(BusError::UsbipMissing.to_string().contains("not installed"));
        assert!(!BusError::Start("x".into()).to_string().contains("not installed"));
        let dir = std::env::temp_dir().join(format!("oc-viiper-missing-{}", std::process::id()));
        assert_eq!(Bus::start(&dir.join("viiper.exe")).err(), Some(BusError::ServerMissing));
    }

    /// A stand-in for VIIPER's server that speaks its API as v0.8.2 does: requests up to a NUL,
    /// one line of JSON or problem JSON and a close; device streams of 20-byte reports with
    /// 2-byte rumble back; devices removed when their stream is not open within the handler
    /// timeout, and empty buses removed.
    struct Fake {
        addr: SocketAddr,
        state: Arc<Mutex<State>>,
    }

    #[derive(Default)]
    struct State {
        next_bus: u32,
        buses: HashMap<u32, Vec<Dev>>,
        log: Vec<String>,
        /// The next add fails with this problem.
        refuse_add: Option<(u16, &'static str)>,
        /// The next add's device is gone before its stream opens.
        drop_after_add: bool,
        /// Keeps its end of a stream open after this side closes it, as a hung server would.
        hang: bool,
        timeout: Duration,
    }

    struct Dev {
        id: u32,
        /// Until when it may go without a stream; `None` while its stream is open.
        deadline: Option<Instant>,
        reports: Arc<Mutex<Vec<[u8; 20]>>>,
        /// Rumble to send down its stream.
        rumble: Option<mpsc::Sender<[u8; 2]>>,
        /// Closes its stream from the server's side.
        stream: Option<TcpStream>,
    }

    fn problem(status: u16, detail: &str) -> String {
        let title = match status {
            400 => "Bad Request",
            404 => "Not Found",
            409 => "Conflict",
            _ => "Internal Server Error",
        };
        format!("{}\n", serde_json::json!({"status": status, "title": title, "detail": detail}))
    }

    impl Fake {
        fn start(timeout: Duration) -> Fake {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let state = Arc::new(Mutex::new(State { next_bus: 1, timeout, ..State::default() }));
            let s = state.clone();
            std::thread::spawn(move || {
                for conn in listener.incoming() {
                    let Ok(conn) = conn else { return };
                    let s = s.clone();
                    std::thread::spawn(move || serve(conn, s));
                }
            });
            let s = state.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                    let mut st = s.lock().unwrap();
                    let now = Instant::now();
                    let mut emptied = Vec::new();
                    for (&bus, devs) in st.buses.iter_mut() {
                        let before = devs.len();
                        devs.retain(|d| d.deadline.is_none_or(|t| now < t));
                        if before != devs.len() && devs.is_empty() {
                            emptied.push(bus);
                        }
                    }
                    for bus in emptied {
                        st.buses.remove(&bus);
                        st.log.push(format!("timeout: removed empty bus {bus}"));
                    }
                }
            });
            Fake { addr, state }
        }

        fn api(&self) -> Api {
            Api { addr: self.addr, wait: Duration::from_secs(2) }
        }

        fn bus(&self) -> Bus {
            Bus::open(self.api(), None).unwrap()
        }

        fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
            f(&mut self.state.lock().unwrap())
        }

        fn devices(&self) -> Vec<(u32, u32)> {
            self.with(|s| {
                let mut v: Vec<(u32, u32)> = s.buses.iter().flat_map(|(&b, d)| d.iter().map(move |d| (b, d.id))).collect();
                v.sort();
                v
            })
        }

        fn reports(&self, bus: u32, dev: u32) -> Vec<[u8; 20]> {
            self.with(|s| s.buses[&bus].iter().find(|d| d.id == dev).map(|d| d.reports.lock().unwrap().clone()).unwrap_or_default())
        }
    }

    fn serve(mut conn: TcpStream, s: Arc<Mutex<State>>) {
        let mut reader = std::io::BufReader::new(conn.try_clone().unwrap());
        let mut req = Vec::new();
        if reader.read_until(0, &mut req).is_err() || req.last() != Some(&0) {
            return;
        }
        req.pop();
        let req = String::from_utf8(req).unwrap();
        let (path, payload) = req.split_once(' ').unwrap_or((&req, ""));
        let parts: Vec<&str> = path.split('/').collect();
        let mut st = s.lock().unwrap();
        st.log.push(req.clone());
        let reply = match parts.as_slice() {
            ["ping"] => r#"{"server":"VIIPER","version":"0.8.2"}"#.to_string() + "\n",
            ["bus", "create"] => {
                let id = st.next_bus;
                st.next_bus += 1;
                st.buses.insert(id, Vec::new());
                format!("{{\"busId\":{id}}}\n")
            }
            ["bus", b, "add"] => {
                let b: u32 = b.parse().unwrap();
                let timeout = st.timeout;
                if let Some((status, detail)) = st.refuse_add.take() {
                    problem(status, detail)
                } else if !payload.contains(r#""type":"xbox360""#) {
                    problem(400, "missing device type")
                } else if let Some(devs) = st.buses.get_mut(&b) {
                    let id = devs.iter().map(|d| d.id).max().unwrap_or(0) + 1;
                    let dev = Dev { id, deadline: Some(Instant::now() + timeout), reports: Default::default(), rumble: None, stream: None };
                    devs.push(dev);
                    if std::mem::take(&mut st.drop_after_add) {
                        st.buses.get_mut(&b).unwrap().retain(|d| d.id != id);
                    }
                    format!(r#"{{"busId":{b},"devId":"{id}","vid":"0x045e","pid":"0x028e","type":"xbox360","deviceSpecific":{{}}}}"#) + "\n"
                } else {
                    problem(404, &format!("bus {b} not found"))
                }
            }
            ["bus", b, "remove"] => {
                let b: u32 = b.parse().unwrap();
                let id: u32 = payload.parse().unwrap_or(0);
                match st.buses.get_mut(&b) {
                    Some(devs) if devs.iter().any(|d| d.id == id) => {
                        if let Some(d) = devs.iter().find(|d| d.id == id)
                            && let Some(c) = &d.stream
                        {
                            let _ = c.shutdown(Shutdown::Both);
                        }
                        devs.retain(|d| d.id != id);
                        format!("{{\"busId\":{b},\"devId\":\"{id}\"}}\n")
                    }
                    _ => problem(404, &format!("device {payload} not found on bus {b}")),
                }
            }
            ["bus", b, d] => {
                let (b, d): (u32, u32) = (b.parse().unwrap(), d.parse().unwrap_or(0));
                let Some(dev) = st.buses.get_mut(&b).and_then(|devs| devs.iter_mut().find(|x| x.id == d)) else {
                    let _ = conn.write_all(problem(404, &format!("device {d} not found on bus {b}")).as_bytes());
                    return;
                };
                dev.deadline = None;
                let (tx, rx) = mpsc::channel::<[u8; 2]>();
                dev.rumble = Some(tx);
                dev.stream = conn.try_clone().ok();
                let reports = dev.reports.clone();
                let timeout = st.timeout;
                drop(st);
                let mut out = conn.try_clone().unwrap();
                std::thread::spawn(move || {
                    for r in rx {
                        if out.write_all(&r).is_err() {
                            return;
                        }
                    }
                });
                let mut buf = [0u8; 20];
                while reader.read_exact(&mut buf).is_ok() {
                    reports.lock().unwrap().push(buf);
                }
                if !s.lock().unwrap().hang {
                    let _ = conn.shutdown(Shutdown::Both);
                }
                // The stream ended: the device has the handler timeout to get a new one.
                if let Some(dev) = s.lock().unwrap().buses.get_mut(&b).and_then(|devs| devs.iter_mut().find(|x| x.id == d)) {
                    dev.deadline = Some(Instant::now() + timeout);
                    dev.rumble = None;
                    dev.stream = None;
                }
                return;
            }
            _ => problem(404, &format!("unknown path: {path}")),
        };
        let _ = conn.write_all(reply.as_bytes());
    }

    fn wait_until(what: &str, f: impl Fn() -> bool) {
        let until = Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(Instant::now() < until, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_controller_is_added_fed_and_removed() {
        let fake = Fake::start(Duration::from_secs(5));
        let bus = fake.bus();
        assert_eq!(bus.version(), "0.8.2");
        let serial = bus.plug_x360().unwrap();
        assert_eq!(serial, 1);
        assert_eq!(fake.devices(), [(1, 1)]);
        let r = XusbReport { buttons: 0x1000, thumb_lx: 1234, ..XusbReport::default() };
        bus.submit(serial, &r).unwrap();
        bus.submit(serial, &XusbReport::default()).unwrap();
        wait_until("both reports", || fake.reports(1, 1).len() == 2);
        assert_eq!(fake.reports(1, 1), [wire(&r), [0; 20]]);
        bus.unplug(serial).unwrap();
        assert_eq!(fake.devices(), []);
        assert_eq!(bus.submit(serial, &r), Err(ERROR_FILE_NOT_FOUND));
        assert_eq!(bus.unplug(serial), Err(ERROR_FILE_NOT_FOUND));
        assert!(bus.lost().is_none(), "{:?}", bus.lost());
        let log = fake.with(|s| s.log.clone());
        assert_eq!(log, ["ping", "bus/create", "bus/1/add {\"type\":\"xbox360\"}", "bus/1/1", "bus/1/remove 1"]);
    }

    #[test]
    fn rumble_comes_back_until_the_controller_goes() {
        let fake = Fake::start(Duration::from_secs(5));
        let bus = fake.bus();
        let serial = bus.plug_x360().unwrap();
        let (tx, rx) = unbounded();
        let listener = bus.listen(serial, tx);
        wait_until("the stream", || fake.with(|s| s.buses[&1][0].rumble.is_some()));
        let rumble = fake.with(|s| s.buses[&1][0].rumble.clone().unwrap());
        rumble.send([200, 10]).unwrap();
        rumble.send([0, 0]).unwrap();
        let got: Vec<Feedback> = (0..2).map(|_| rx.recv_timeout(Duration::from_secs(2)).unwrap()).collect();
        assert_eq!(got[0], Feedback { serial, large_motor: 200, small_motor: 10 });
        assert_eq!(got[1], Feedback { serial, large_motor: 0, small_motor: 0 });
        bus.unplug(serial).unwrap();
        listener.join().unwrap();
        assert!(bus.lost().is_none(), "closing it here is not a failure: {:?}", bus.lost());
    }

    #[test]
    fn closing_here_ends_the_feedback_threads_whatever_the_server_does() {
        let fake = Fake::start(Duration::from_secs(5));
        fake.with(|s| s.hang = true);
        let bus = fake.bus();
        let serials = [bus.plug_x360().unwrap(), bus.plug_x360().unwrap()];
        let listeners: Vec<JoinHandle<()>> = serials.iter().map(|&s| bus.listen(s, unbounded().0)).collect();
        let start = Instant::now();
        bus.cancel_all();
        for l in listeners {
            l.join().unwrap();
        }
        assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
        assert!(bus.lost().is_none(), "{:?}", bus.lost());
    }

    #[test]
    fn serials_are_the_lowest_free_and_streams_beat_the_handler_timeout() {
        let fake = Fake::start(Duration::from_millis(300));
        let bus = fake.bus();
        let serials: Vec<u32> = (0..4).map(|_| bus.plug_x360().unwrap()).collect();
        assert_eq!(serials, [1, 2, 3, 4]);
        bus.unplug(2).unwrap();
        assert_eq!(bus.plug_x360().unwrap(), 2);
        // Their streams are open, so the server keeps them past its timeout.
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(fake.devices().len(), 4);
        for s in serials {
            bus.submit(s, &XusbReport::default()).unwrap();
        }
    }

    #[test]
    fn a_bus_the_server_removed_is_made_again() {
        let fake = Fake::start(Duration::from_millis(200));
        let bus = fake.bus();
        let serial = bus.plug_x360().unwrap();
        bus.unplug(serial).unwrap();
        // Nothing on it any more: the server drops the empty bus. Here it goes at once.
        fake.with(|s| s.buses.clear());
        let serial = bus.plug_x360().unwrap();
        assert_eq!(fake.devices(), [(2, 1)]);
        bus.submit(serial, &XusbReport::default()).unwrap();
    }

    #[test]
    fn refusals_reach_the_caller_and_leave_nothing_behind() {
        let fake = Fake::start(Duration::from_secs(5));
        let bus = fake.bus();
        fake.with(|s| s.refuse_add = Some((409, "Failed to auto-attach device: no VHCI")));
        let e = bus.plug_x360().unwrap_err();
        assert_eq!(e, BusError::Problem(409, "Failed to auto-attach device: no VHCI".into()));
        assert!(e.to_string().contains("auto-attach"));
        // The device is gone before its stream opens, as when the attach outlasts the timeout.
        fake.with(|s| s.drop_after_add = true);
        assert!(matches!(bus.plug_x360(), Err(BusError::Problem(404, _))));
        assert_eq!(fake.devices(), []);
        // The serial was not kept.
        assert_eq!(bus.plug_x360().unwrap(), 1);
    }

    #[test]
    fn a_stream_the_server_drops_is_reported() {
        let fake = Fake::start(Duration::from_secs(5));
        let bus = fake.bus();
        let serial = bus.plug_x360().unwrap();
        let (tx, _rx) = unbounded();
        let listener = bus.listen(serial, tx);
        wait_until("the stream", || fake.with(|s| s.buses[&1][0].stream.is_some()));
        fake.with(|s| s.buses[&1][0].stream.as_ref().unwrap().shutdown(Shutdown::Both).unwrap());
        listener.join().unwrap();
        assert!(bus.lost().is_some_and(|w| w.contains("closed")), "{:?}", bus.lost());
    }

    #[test]
    fn a_server_that_does_not_answer_times_out() {
        // Accepts, reads and never answers.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let held: Vec<TcpStream> = listener.incoming().filter_map(Result::ok).collect();
            drop(held);
        });
        let api = Api { addr, wait: Duration::from_millis(200) };
        let start = Instant::now();
        assert!(matches!(api.ping(), Err(BusError::Io(_))));
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        // Nothing listening at all.
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        assert!(Api { addr: closed, wait: Duration::from_millis(200) }.ping().is_err());
    }

    #[test]
    fn something_else_on_the_port_is_not_taken_for_viiper() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut c in listener.incoming().filter_map(Result::ok) {
                let mut b = [0u8; 5];
                let _ = c.read_exact(&mut b);
                let _ = c.write_all(b"{\"server\":\"other\",\"version\":\"1\"}\n");
            }
        });
        let e = Bus::open(Api { addr, wait: Duration::from_secs(1) }, None).err().unwrap();
        assert!(matches!(e, BusError::Start(_)), "{e:?}");
    }
}
