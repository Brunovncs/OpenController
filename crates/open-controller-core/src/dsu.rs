//! Loopback-only Cemuhook/DSU 1001 server. Protocol: https://v1993.github.io/cemuhook-protocol/
//! UDP work, subscriptions and checksums stay off the controller input thread.
use crate::ds4::{Motion, STANDARD_GRAVITY};
use crate::mapping::{PadState, axis, button};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const VERSION: u16 = 1001;
const INFO: u32 = 0x100001;
const DATA: u32 = 0x100002;
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub state: PadState,
    pub motion: Motion,
    pub connection: u8,
    pub battery: u8,
    pub mac: [u8; 6],
    pub timestamp_us: u64,
    pub touch_ids: [u8; 2],
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    pub running: bool,
    pub port: u16,
    pub clients: usize,
    pub error: Option<String>,
}
struct Update {
    enabled: bool,
    frames: [Option<Frame>; 4],
}
pub struct Server {
    tx: crossbeam_channel::Sender<Update>,
    status: Arc<Mutex<Status>>,
}
impl Server {
    pub fn start() -> Self {
        Self::with_port(26760)
    }
    fn with_port(port: u16) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        let _ = std::thread::Builder::new().name("open-controller-dsu".into()).spawn(move || serve(rx, port, shared));
        Self { tx, status }
    }
    pub fn update(&self, enabled: bool, frames: [Option<Frame>; 4]) {
        let _ = self.tx.try_send(Update { enabled, frames });
    }
    pub fn status(&self) -> Option<Status> {
        self.status.try_lock().ok().map(|s| s.clone())
    }
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
fn packet(magic: &[u8; 4], id: u32, kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(20 + payload.len());
    bytes.extend(magic);
    bytes.extend(VERSION.to_le_bytes());
    bytes.extend(((payload.len() + 4) as u16).to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend(id.to_le_bytes());
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    let crc = crc32(&bytes);
    bytes[8..12].copy_from_slice(&crc.to_le_bytes());
    bytes
}
fn request(bytes: &[u8]) -> Option<(u32, &[u8])> {
    if bytes.len() < 20 || &bytes[..4] != b"DSUC" || u16::from_le_bytes(bytes[4..6].try_into().ok()?) != VERSION {
        return None;
    }
    let len = usize::from(u16::from_le_bytes(bytes[6..8].try_into().ok()?)) + 16;
    if !(20..=512).contains(&len) || bytes.len() < len {
        return None;
    }
    let mut checked = bytes[..len].to_vec();
    checked[8..12].fill(0);
    if crc32(&checked) != u32::from_le_bytes(bytes[8..12].try_into().ok()?) {
        return None;
    }
    Some((u32::from_le_bytes(bytes[16..20].try_into().ok()?), &bytes[20..len]))
}
fn info(slot: u8, frame: Option<Frame>) -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[0] = slot;
    if let Some(f) = frame {
        bytes[1] = 2;
        bytes[2] = 2;
        bytes[3] = f.connection;
        bytes[4..10].copy_from_slice(&f.mac);
        bytes[10] = f.battery;
    }
    bytes
}
fn data(slot: u8, f: Frame, sequence: u32) -> [u8; 80] {
    let mut bytes = [0; 80];
    bytes[..12].copy_from_slice(&info(slot, Some(f)));
    bytes[11] = 1;
    bytes[12..16].copy_from_slice(&sequence.to_le_bytes());
    let button_lists = [
        [
            button::DPAD_LEFT,
            button::DPAD_DOWN,
            button::DPAD_RIGHT,
            button::DPAD_UP,
            button::START,
            button::RIGHT_STICK,
            button::LEFT_STICK,
            button::BACK,
        ],
        [button::NORTH, button::EAST, button::SOUTH, button::WEST, button::RIGHT_SHOULDER, button::LEFT_SHOULDER, 62, 63],
    ];
    for (i, list) in button_lists.iter().enumerate() {
        for (bit, &b) in list.iter().take(if i == 0 { 8 } else { 6 }).enumerate() {
            if f.state.pressed(b) {
                bytes[16 + i] |= 1 << (7 - bit);
            }
        }
    }
    bytes[18] = u8::from(f.state.pressed(button::GUIDE));
    bytes[19] = u8::from(f.state.pressed(button::TOUCHPAD));
    for (i, &a) in [axis::LEFT_X, axis::LEFT_Y, axis::RIGHT_X, axis::RIGHT_Y].iter().enumerate() {
        let value = i32::from(f.state.axes[a]);
        let value = if i % 2 == 1 { -value } else { value };
        bytes[20 + i] = ((value + 32768).clamp(0, 65535) >> 8) as u8;
    }
    for (i, &b) in [
        button::DPAD_LEFT,
        button::DPAD_DOWN,
        button::DPAD_RIGHT,
        button::DPAD_UP,
        button::NORTH,
        button::EAST,
        button::SOUTH,
        button::WEST,
        button::RIGHT_SHOULDER,
        button::LEFT_SHOULDER,
    ]
    .iter()
    .enumerate()
    {
        bytes[24 + i] = if f.state.pressed(b) { 255 } else { 0 };
    }
    for (at, a, mask) in [(34, axis::RIGHT_TRIGGER, 2), (35, axis::LEFT_TRIGGER, 1)] {
        bytes[at] = (i32::from(f.state.axes[a].max(0)) * 255 / 32767) as u8;
        if bytes[at] > 0 {
            bytes[17] |= mask;
        }
    }
    for (i, touch) in f.state.touches.iter().enumerate() {
        let at = 36 + i * 6;
        bytes[at] = u8::from(touch.down);
        bytes[at + 1] = f.touch_ids[i];
        bytes[at + 2..at + 4].copy_from_slice(&((u32::from(touch.x) * 1919 / 65535) as u16).to_le_bytes());
        bytes[at + 4..at + 6].copy_from_slice(&((u32::from(touch.y) * 941 / 65535) as u16).to_le_bytes());
    }
    bytes[48..56].copy_from_slice(&f.timestamp_us.to_le_bytes());
    // SDL: X right, Y up, Z toward player. DSU: X right, Y forward, Z up.
    let accel = [f.motion.accel[0], -f.motion.accel[2], f.motion.accel[1]].map(|v| v / STANDARD_GRAVITY);
    let gyro = [f.motion.gyro[0], f.motion.gyro[1], f.motion.gyro[2]].map(f32::to_degrees);
    for (i, value) in accel.into_iter().chain(gyro).enumerate() {
        bytes[56 + i * 4..60 + i * 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
struct Client {
    slots: u8,
    touched: Instant,
    sequence: u32,
}
fn serve(rx: crossbeam_channel::Receiver<Update>, port: u16, status: Arc<Mutex<Status>>) {
    let id = std::process::id() ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos();
    let mut socket: Option<UdpSocket> = None;
    let mut clients: HashMap<SocketAddr, Client> = HashMap::new();
    let mut enabled = false;
    let mut frames = [None; 4];
    let mut retry = Instant::now();
    let mut last_send = Instant::now();
    let mut error = None;
    loop {
        match rx.recv_timeout(if enabled { Duration::from_millis(8) } else { Duration::from_millis(200) }) {
            Ok(update) => {
                enabled = update.enabled;
                frames = update.frames;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
        let now = Instant::now();
        if !enabled {
            socket = None;
            clients.clear();
            error = None;
        } else if socket.is_none() && now >= retry {
            retry = now + Duration::from_secs(1);
            match UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, port)).and_then(|s| {
                s.set_nonblocking(true)?;
                Ok(s)
            }) {
                Ok(s) => {
                    socket = Some(s);
                    error = None;
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        if let Some(socket) = &socket {
            let mut buffer = [0; 512];
            for _ in 0..32 {
                let Ok((n, peer)) = socket.recv_from(&mut buffer) else { break };
                if !peer.ip().is_loopback() {
                    continue;
                }
                let Some((kind, payload)) = request(&buffer[..n]) else { continue };
                match kind {
                    0x100000 => {
                        let _ = socket.send_to(&packet(b"DSUS", id, kind, &VERSION.to_le_bytes()), peer);
                    }
                    INFO if payload.len() >= 4 => {
                        let count = u32::from_le_bytes(payload[..4].try_into().unwrap()) as usize;
                        if count > 4 || payload.len() < 4 + count {
                            continue;
                        }
                        for &slot in &payload[4..4 + count] {
                            if slot < 4 {
                                let _ = socket.send_to(&packet(b"DSUS", id, kind, &info(slot, frames[usize::from(slot)])), peer);
                            }
                        }
                    }
                    DATA if payload.len() >= 8 => {
                        let mask = if payload[0] == 0 {
                            15
                        } else {
                            let mut slots = 0u8;
                            if payload[0] & 1 != 0 && payload[1] < 4 {
                                slots |= 1 << payload[1];
                            }
                            if payload[0] & 2 != 0 {
                                for (slot, frame) in frames.iter().enumerate() {
                                    if frame.is_some_and(|f| f.mac.as_slice() == &payload[2..8]) {
                                        slots |= 1 << slot;
                                    }
                                }
                            }
                            slots
                        };
                        if clients.len() < 16 || clients.contains_key(&peer) {
                            let client = clients.entry(peer).or_insert(Client { slots: 0, touched: now, sequence: 0 });
                            client.slots |= mask;
                            client.touched = now;
                        }
                    }
                    _ => {}
                }
            }
            clients.retain(|_, c| now.duration_since(c.touched) < Duration::from_secs(5));
            if now.duration_since(last_send) >= Duration::from_millis(8) {
                last_send = now;
                for (peer, client) in &mut clients {
                    for (slot, frame) in frames.iter().enumerate() {
                        if client.slots & (1 << slot) != 0
                            && let Some(frame) = frame
                        {
                            client.sequence = client.sequence.wrapping_add(1);
                            let _ = socket.send_to(&packet(b"DSUS", id, DATA, &data(slot as u8, *frame, client.sequence)), peer);
                        }
                    }
                }
            }
        }
        if let Ok(mut shared) = status.lock() {
            *shared = Status {
                running: socket.is_some(),
                port: socket.as_ref().and_then(|s| s.local_addr().ok()).map_or(port, |a| a.port()),
                clients: clients.len(),
                error: error.clone(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_and_request_validation() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        let mut bytes = packet(b"DSUC", 5, INFO, &[1, 0, 0, 0, 0]);
        assert!(request(&bytes).is_some());
        bytes[20] ^= 1;
        assert!(request(&bytes).is_none());
        assert!(request(&bytes[..15]).is_none());
    }
    #[test]
    fn motion_packet_has_standard_size_units_and_touch() {
        let f = Frame {
            state: PadState::default(),
            motion: Motion::REST,
            connection: 2,
            battery: 5,
            mac: [2; 6],
            timestamp_us: 123,
            touch_ids: [0, 1],
        };
        let p = packet(b"DSUS", 1, DATA, &data(0, f, 3));
        assert_eq!(p.len(), 100);
        assert_eq!(&p[..4], b"DSUS");
        assert_eq!(&p[40..44], &[128; 4], "all DSU sticks are centred at 128");
        assert_eq!(f32::from_le_bytes(p[84..88].try_into().unwrap()), 1.);
        assert_eq!(u64::from_le_bytes(p[68..76].try_into().unwrap()), 123);
    }
    #[test]
    fn real_udp_version_info_and_subscription() {
        let server = Server::with_port(0);
        let frame = Frame {
            state: PadState::default(),
            motion: Motion::REST,
            connection: 1,
            battery: 5,
            mac: [2; 6],
            timestamp_us: 42,
            touch_ids: [0, 1],
        };
        server.update(true, [Some(frame), None, None, None]);
        let deadline = Instant::now() + Duration::from_secs(2);
        let port = loop {
            if let Some(s) = server.status()
                && s.running
            {
                break s.port;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut bytes = [0; 512];
        client.send_to(&packet(b"DSUC", 1, 0x100000, &[]), ("127.0.0.1", port)).unwrap();
        let n = client.recv(&mut bytes).unwrap();
        assert_eq!(n, 22);
        client.send_to(&packet(b"DSUC", 1, INFO, &[1, 0, 0, 0, 0]), ("127.0.0.1", port)).unwrap();
        let n = client.recv(&mut bytes).unwrap();
        assert_eq!(n, 32);
        assert_eq!(bytes[21], 2);
        client.send_to(&packet(b"DSUC", 1, DATA, &[1, 0, 0, 0, 0, 0, 0, 0]), ("127.0.0.1", port)).unwrap();
        let n = client.recv(&mut bytes).unwrap();
        assert_eq!(n, 100);
        assert_eq!(bytes[31], 1);
    }
}
