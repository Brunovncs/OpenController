//! The input thread. It owns SDL, the virtual controllers and HidHide, and runs one loop:
//! read what the controllers sent, forward it to the virtual controllers, pass rumble back,
//! sleep about a millisecond. The UI only sends commands and reads snapshots.

use crate::bluetooth;
use crate::device::{self, Brand, Identity, Link, Power};
use crate::devnode;
use crate::hidhide::Cloak;
use crate::mapping::{self, PadState, XusbReport};
use crate::roster::{Attached, DeviceId, Roster, SlotId};
use crate::rt;
use crate::sdl::{Event, Gamepad, Sdl};
use crate::vigem::{Bus, Feedback};
use crate::win::Overlapped;
use crossbeam_channel::{Receiver, Sender, unbounded};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long a virtual controller waits for its physical controller to come back.
pub const GRACE: Duration = Duration::from_secs(15);
/// Poll period while a controller is connected. SDL reads the controllers' reports when polled.
const ACTIVE_PERIOD: Duration = Duration::from_millis(1);
/// Poll period with nothing connected: only hot-plug is watched.
const IDLE_PERIOD: Duration = Duration::from_millis(20);
/// How long to wait for Windows to give a new virtual controller its XInput slot. It takes
/// microseconds normally, and seconds the first time, while the Xbox 360 driver is installed.
const SLOT_WAIT: Duration = Duration::from_secs(5);
/// SDL stops a rumble after at most 65.5 s; a steady one is renewed well before.
const RUMBLE_MS: u32 = 20_000;
const RUMBLE_RENEW: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_secs(5);
const VIEW_PERIOD: Duration = Duration::from_millis(16);

pub struct Config {
    /// Where the HidHide journal is kept.
    pub data_dir: PathBuf,
    /// Hide the physical controllers that are turned into virtual ones from games.
    pub hide: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadKey {
    Slot(u32),
    Device(u32),
}

pub enum Command {
    SetHiding(bool),
    /// Rumble a controller briefly, to tell which one is which.
    Identify(PadKey),
    /// Turn a Bluetooth controller off.
    PowerOff(PadKey),
    /// Whether someone is looking: live input in snapshots is only kept up to date then.
    Watch(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Role {
    /// Games see it as an Xbox 360 controller in XInput slot `player` (0 to 3); `None` for a
    /// fifth controller and up, which XInput games cannot see.
    Virtual { player: Option<u8> },
    /// The controller is away; its virtual controller stays for `remaining`.
    Waiting { player: Option<u8>, remaining: Duration },
    /// An XInput controller, which games already read directly.
    Native { player: Option<u8> },
    /// SDL has no button layout for this device.
    Unmapped,
    /// Recognised, but no virtual controller could be created.
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PadView {
    pub key: PadKey,
    pub name: String,
    pub brand: Brand,
    /// Every connection the controller has right now, the active one first.
    pub links: Vec<Link>,
    pub power: Power,
    pub role: Role,
    /// Hidden from games by HidHide.
    pub hidden: bool,
    pub input: PadState,
    pub can_power_off: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Driver {
    Ready { version: Option<String> },
    Missing,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub pads: Vec<PadView>,
    pub vigem: Driver,
    pub hidhide: Driver,
    pub hiding: bool,
    pub sdl_version: String,
    pub sdl_error: Option<String>,
    pub running: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            pads: Vec::new(),
            vigem: Driver::Missing,
            hidhide: Driver::Missing,
            hiding: false,
            sdl_version: String::new(),
            sdl_error: None,
            running: false,
        }
    }
}

pub struct Engine {
    tx: Sender<Msg>,
    shared: Arc<Mutex<Snapshot>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

enum Msg {
    Command(Command),
    Quit,
}

impl Engine {
    /// Starts the input thread. `on_change` is called from it whenever the snapshot changes.
    pub fn start(config: Config, on_change: impl Fn() + Send + 'static) -> Engine {
        let (tx, rx) = unbounded();
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("open-controller-input".into())
            .spawn(move || run(config, rx, s, Box::new(on_change)))
            .expect("could not start the input thread");
        Engine { tx, shared, thread: Mutex::new(Some(thread)) }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// How many controllers the last snapshot lists, without copying it.
    pub fn pad_count(&self) -> usize {
        self.shared.lock().map(|s| s.pads.len()).unwrap_or(0)
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(Msg::Command(c));
    }

    /// Unplugs the virtual controllers, shows the hidden ones again and waits for the thread.
    pub fn stop(&self) {
        let _ = self.tx.send(Msg::Quit);
        let thread = self.thread.lock().ok().and_then(|mut t| t.take());
        if let Some(t) = thread {
            let _ = t.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A connected device, as SDL sees it.
struct Phys {
    pad: Gamepad,
    name: String,
    brand: Brand,
    link: Link,
    identity: Identity,
    /// Device instance id, for HidHide.
    instance: Option<String>,
    kind: Kind,
    state: PadState,
    power: Power,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Drives a slot, and so a virtual controller.
    Slot(SlotId),
    /// An XInput controller of its own.
    Native,
    /// One of the virtual controllers, read back through XInput: ignored.
    Own,
    /// An XInput device seen while a virtual controller's slot is still unknown; it cannot be
    /// told apart from that controller yet.
    Pending,
}

/// A virtual controller.
struct Target {
    serial: u32,
    player: Option<u8>,
    /// Until when its XInput slot is asked for on every pass; XInput devices seen meanwhile
    /// wait to be classified.
    asking_until: Option<Instant>,
    /// After that it is asked once a second: a fifth controller has none, and the first one on
    /// a machine can take longer while Windows installs its driver.
    next_ask: Instant,
    last: Option<XusbReport>,
    rumble: (u8, u8),
    rumble_sent: Instant,
    rumble_to: Option<DeviceId>,
    listener: Option<JoinHandle<()>>,
}

struct Core {
    bus: Option<Arc<Bus>>,
    /// Read once on connecting: the device tree is slow to query, and this is the input thread.
    bus_version: Option<String>,
    bus_error: Option<String>,
    bus_retry: Instant,
    io: Overlapped,
    cloak: Option<Cloak>,
    hidhide: Driver,
    hiding: bool,
    roster: Roster,
    phys: HashMap<DeviceId, Phys>,
    unmapped: HashMap<DeviceId, String>,
    targets: HashMap<SlotId, Target>,
    failed: HashMap<SlotId, Instant>,
    feedback_tx: Sender<Feedback>,
    feedback_rx: Receiver<Feedback>,
    /// Controllers rumbling to be identified, and until when.
    identify: Vec<(DeviceId, Instant)>,
    /// Name and maker of each slot's controller, still shown while it is away.
    memo: HashMap<SlotId, (String, Brand)>,
    /// Devices turned off from here: when one leaves and its slot is empty, the virtual
    /// controller is unplugged at once, without the grace period.
    turning_off: Vec<DeviceId>,
    updated: Vec<DeviceId>,
    changed: bool,
    input_changed: bool,
    /// Last, so it is dropped last: SDL_Quit must come after every gamepad is closed.
    sdl: Sdl,
}

fn run(config: Config, rx: Receiver<Msg>, shared: Arc<Mutex<Snapshot>>, on_change: Box<dyn Fn() + Send>) {
    let _boost = rt::boost_current_thread();
    let mut pacer = rt::Pacer::new();
    let publish = |s: Snapshot| {
        if let Ok(mut g) = shared.lock() {
            *g = s;
        }
        on_change();
    };

    let sdl = match Sdl::init() {
        Ok(s) => s,
        Err(e) => {
            publish(Snapshot { sdl_error: Some(e), ..Snapshot::default() });
            let _ = rx.recv();
            return;
        }
    };
    let _ = std::fs::create_dir_all(&config.data_dir);
    let (cloak, hidhide) = match Cloak::start(&config.data_dir.join(crate::hidhide::JOURNAL_FILE)) {
        Ok(c) => (Some(c), Driver::Ready { version: None }),
        Err(crate::hidhide::CloakError::NotInstalled) => (None, Driver::Missing),

        Err(e) => (None, Driver::Failed(e.to_string())),
    };
    let (feedback_tx, feedback_rx) = unbounded();
    let mut core = Core {
        bus: None,
        bus_version: None,
        bus_error: None,
        bus_retry: Instant::now(),
        io: Overlapped::new(),
        cloak,
        hidhide,
        hiding: config.hide,
        roster: Roster::new(GRACE),
        phys: HashMap::new(),
        unmapped: HashMap::new(),
        targets: HashMap::new(),
        failed: HashMap::new(),
        feedback_tx,
        feedback_rx,
        identify: Vec::new(),
        memo: HashMap::new(),
        turning_off: Vec::new(),
        updated: Vec::with_capacity(16),
        changed: true,
        input_changed: false,
        sdl,
    };
    core.connect_bus();

    let mut watching = false;
    let mut last_view = Instant::now() - VIEW_PERIOD;
    let mut last_countdown = Instant::now();
    'run: loop {
        let now = Instant::now();
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Quit => break 'run,
                Msg::Command(Command::Watch(on)) => {
                    watching = on;
                    core.changed = true;
                }
                Msg::Command(c) => core.command(c, now),
            }
        }
        core.pump(now);
        core.forward();
        core.feedback(now);
        core.housekeeping(now);

        let waiting = core.roster.slots().iter().any(|s| s.lost_since.is_some());
        let countdown = waiting && now.duration_since(last_countdown) >= Duration::from_millis(500);
        let live = watching && core.input_changed && now.duration_since(last_view) >= VIEW_PERIOD;
        if core.changed || countdown || live {
            publish(core.snapshot(now));
            core.changed = false;
            core.input_changed = false;
            last_view = now;
            if countdown {
                last_countdown = now;
            }
        }

        let period = if core.phys.is_empty() && core.targets.is_empty() { IDLE_PERIOD } else { ACTIVE_PERIOD };
        pacer.wait(period);
    }
    core.shutdown();
    publish(Snapshot::default());
}

impl Core {
    fn connect_bus(&mut self) {
        match Bus::connect() {
            Ok(b) => {
                self.bus_version = devnode::driver_version(&b.path);
                self.bus = Some(Arc::new(b));
                self.bus_error = None;
            }
            Err(e) => self.bus_error = Some(e.to_string()),
        }
        self.bus_retry = Instant::now() + RETRY;
        self.changed = true;
    }

    fn command(&mut self, c: Command, now: Instant) {
        match c {
            Command::SetHiding(on) => {
                self.hiding = on;
                if on {
                    let ids: Vec<DeviceId> = self.phys.keys().copied().collect();
                    for id in ids {
                        self.hide(id);
                    }
                } else if let Some(c) = self.cloak.as_mut()
                    && let Err(e) = c.reveal_all()
                {
                    self.hidhide = Driver::Failed(e.to_string());
                }
            }
            Command::Identify(key) => {
                if let Some(dev) = self.device_for(key)
                    && let Some(p) = self.phys.get(&dev)
                {
                    p.pad.rumble(0x7000, 0x7000, 400);
                    self.identify.push((dev, now + Duration::from_millis(400)));
                }
            }
            Command::PowerOff(key) => {
                let dev = self.device_for(key);
                let addr = dev.and_then(|d| self.phys.get(&d)).and_then(|p| device::bluetooth_address(&p.identity));
                if let (Some(dev), Some(a)) = (dev, addr) {
                    self.turning_off.push(dev);
                    // A blocking request to each radio: kept off the input thread.
                    std::thread::spawn(move || bluetooth::disconnect(a));
                }
            }
            Command::Watch(_) => {}
        }
        self.changed = true;
    }

    fn device_for(&self, key: PadKey) -> Option<DeviceId> {
        match key {
            PadKey::Slot(s) => self.roster.get(SlotId(s)).and_then(|s| s.source),
            PadKey::Device(d) => Some(d),
        }
    }

    fn pump(&mut self, now: Instant) {
        while let Some(ev) = self.sdl.poll() {
            match ev {
                Event::GamepadAdded(id) => self.added(id),
                Event::GamepadRemoved(id) => self.removed(id, now),
                Event::GamepadUpdated(id) => {
                    if !self.updated.contains(&id) {
                        self.updated.push(id);
                    }
                }
                Event::Battery(id) => {
                    if let Some(p) = self.phys.get_mut(&id) {
                        p.power = p.pad.power();
                        self.changed = true;
                    }
                }
                Event::JoystickAdded(id) => {
                    if !self.sdl.is_gamepad(id) {
                        let (name, _, _) = self.sdl.joystick_info(id);
                        self.unmapped.insert(id, name);
                        self.changed = true;
                    }
                }
                Event::JoystickRemoved(id) => {
                    self.changed |= self.unmapped.remove(&id).is_some();
                }
                Event::Other => {}
            }
        }
    }

    fn added(&mut self, id: DeviceId) {
        self.unmapped.remove(&id);
        let Some(pad) = self.sdl.open(id) else { return };
        let (vendor, product, path) = (pad.vendor(), pad.product(), pad.path());
        let link = device::link(vendor, product, &path, pad.connection());
        let mut identity = device::identity(vendor, product, pad.serial().as_deref(), &path);
        // One controller cannot be connected twice the same way. Two devices with one serial and
        // one kind of link are two controllers that share a serial, as some clones do.
        if self.shares_link(&identity, link) {
            identity = device::identity(vendor, product, None, &path);
        }
        let mut p = Phys {
            name: pad.name(),
            brand: device::brand(vendor, pad.pad_type()),
            link,
            identity,
            instance: devnode::instance_id(&path),
            kind: Kind::Pending,
            state: PadState::default(),
            power: pad.power(),
            pad,
        };
        p.pad.read(&mut p.state);
        p.kind = if devnode::is_virtual(&path) {
            Kind::Own
        } else if device::is_xinput_path(&path) {
            self.classify_xinput(&path).unwrap_or(Kind::Pending)
        } else {
            match self.roster.attach(id, p.identity.clone()) {
                Attached::New(s) | Attached::Rejoined(s) => Kind::Slot(s),
            }
        };
        let kind = p.kind;
        if let Kind::Slot(s) = kind {
            self.memo.insert(s, (p.name.clone(), p.brand));
        }
        self.phys.insert(id, p);
        if let Kind::Slot(s) = kind {
            self.hide(id);
            if let Some(player) = self.targets.get(&s).and_then(|t| t.player) {
                self.phys[&id].pad.set_player(player as i32);
            }
            self.resend(s);
            self.move_rumble(s);
        }
        self.changed = true;
    }

    fn shares_link(&self, identity: &Identity, link: Link) -> bool {
        self.roster
            .slots()
            .iter()
            .filter(|s| &s.identity == identity)
            .flat_map(|s| s.devices.iter())
            .any(|d| self.phys.get(d).is_some_and(|p| p.link == link))
    }

    /// An XInput device is one of ours if its slot is one our controllers hold. While one of
    /// ours has no slot yet, the answer has to wait.
    fn classify_xinput(&self, path: &str) -> Option<Kind> {
        let Some(slot) = device::xinput_slot(path) else { return Some(Kind::Native) };
        if self.targets.values().any(|t| t.player == Some(slot)) {
            return Some(Kind::Own);
        }
        if self.targets.values().any(|t| t.player.is_none() && t.asking_until.is_some()) {
            return None;
        }
        Some(Kind::Native)
    }

    fn removed(&mut self, id: DeviceId, now: Instant) {
        let Some(p) = self.phys.remove(&id) else { return };
        self.updated.retain(|&d| d != id);
        if let Kind::Slot(s) = p.kind {
            self.roster.detach(id, now);
            let empty = self.roster.get(s).is_some_and(|slot| slot.devices.is_empty());
            let turned_off = self.turning_off.contains(&id);
            self.turning_off.retain(|&x| x != id);
            if empty && turned_off {
                // Turned off on purpose: no point waiting for it to come back.
                self.roster.remove(s);
                self.unplug(s);
            } else {
                // Another connection of the same controller takes over, rumble included, or the
                // game sees it at rest.
                self.resend(s);
                self.move_rumble(s);
            }
        }
        self.changed = true;
    }

    fn hide(&mut self, id: DeviceId) {
        if !self.hiding {
            return;
        }
        let (Some(cloak), Some(p)) = (self.cloak.as_mut(), self.phys.get(&id)) else { return };
        // Only a controller games can see as an Xbox controller is hidden; hiding one whose
        // virtual controller does not exist would take it away from games altogether.
        let Kind::Slot(s) = p.kind else { return };
        if !self.targets.contains_key(&s) {
            return;
        }
        if let Some(instance) = p.instance.clone()
            && let Err(e) = cloak.hide(&[instance])
        {
            self.hidhide = Driver::Failed(e.to_string());
        }
    }

    /// Sends a slot's current state again: after its source changed, or to a new controller.
    fn resend(&mut self, s: SlotId) {
        let state = self.roster.get(s).and_then(|slot| slot.source).and_then(|d| self.phys.get(&d)).map(|p| p.state).unwrap_or_default();
        self.submit(s, &state);
    }

    fn submit(&mut self, s: SlotId, state: &PadState) {
        let (Some(bus), Some(t)) = (self.bus.as_ref(), self.targets.get_mut(&s)) else { return };
        let report = mapping::to_xusb(state);
        if t.last != Some(report) && bus.submit(&mut self.io, t.serial, &report).is_ok() {
            t.last = Some(report);
        }
    }

    /// Forwards every controller that sent a report since the last pass.
    fn forward(&mut self) {
        let updated = std::mem::take(&mut self.updated);
        for &id in &updated {
            let Some(p) = self.phys.get_mut(&id) else { continue };
            let before = p.state;
            p.pad.read(&mut p.state);
            if p.state == before {
                continue;
            }
            self.input_changed = true;
            let (kind, state) = (p.kind, p.state);
            if let Kind::Slot(s) = kind {
                // Stick noise on the idle connection of a controller connected twice must not
                // pull the slot over to it.
                if mapping::significant(&before, &state) && self.roster.touched(id) {
                    self.changed = true;
                    self.move_rumble(s);
                }
                if self.roster.get(s).and_then(|x| x.source) == Some(id) {
                    self.submit(s, &state);
                }
            }
        }
        self.updated = updated;
        self.updated.clear();
    }

    fn feedback(&mut self, now: Instant) {
        while let Ok(fb) = self.feedback_rx.try_recv() {
            let Some((&s, t)) = self.targets.iter_mut().find(|(_, t)| t.serial == fb.serial) else { continue };
            let rumble = (fb.large_motor, fb.small_motor);
            if rumble == t.rumble {
                continue;
            }
            t.rumble = rumble;
            t.rumble_sent = now;
            self.apply_rumble(s);
        }
        let stale: Vec<SlotId> = self
            .targets
            .iter()
            .filter(|(_, t)| t.rumble != (0, 0) && now.duration_since(t.rumble_sent) >= RUMBLE_RENEW)
            .map(|(&s, _)| s)
            .collect();
        for s in stale {
            if let Some(t) = self.targets.get_mut(&s) {
                t.rumble_sent = now;
            }
            self.apply_rumble(s);
        }
    }

    fn apply_rumble(&mut self, s: SlotId) {
        let source = self.roster.get(s).and_then(|x| x.source);
        let Some(t) = self.targets.get_mut(&s) else { return };
        if let Some(p) = source.and_then(|d| self.phys.get(&d)) {
            p.pad.rumble(mapping::motor(t.rumble.0), mapping::motor(t.rumble.1), RUMBLE_MS);
            t.rumble_to = source;
        }
    }

    /// The source of a slot changed: the rumble follows it.
    fn move_rumble(&mut self, s: SlotId) {
        let source = self.roster.get(s).and_then(|x| x.source);
        let Some(t) = self.targets.get(&s) else { return };
        if t.rumble_to != source
            && let Some(p) = t.rumble_to.and_then(|d| self.phys.get(&d))
        {
            p.pad.rumble(0, 0, 0);
        }
        if t.rumble != (0, 0) {
            self.apply_rumble(s);
        }
    }

    fn housekeeping(&mut self, now: Instant) {
        if self.bus.is_none() && now >= self.bus_retry {
            self.connect_bus();
        }
        self.plug_missing(now);
        self.ask_player_slots(now);
        let pending: Vec<DeviceId> = self.phys.iter().filter(|(_, p)| p.kind == Kind::Pending).map(|(&id, _)| id).collect();
        for id in pending {
            let path = self.phys[&id].pad.path();
            if let Some(k) = self.classify_xinput(&path) {
                self.phys.get_mut(&id).unwrap().kind = k;
                self.changed = true;
            }
        }
        for s in self.roster.expire(now) {
            self.unplug(s);
            self.changed = true;
        }
        let (done, waiting): (Vec<_>, Vec<_>) = self.identify.drain(..).partition(|(_, until)| now >= *until);
        self.identify = waiting;
        // The identification rumble replaced the game's; put it back.
        for (dev, _) in done {
            if let Some(s) = self.roster.slot_of(dev) {
                self.apply_rumble(s);
            }
        }
    }

    /// Plugs a virtual controller for every slot that has a controller and none yet.
    fn plug_missing(&mut self, now: Instant) {
        let Some(bus) = self.bus.clone() else { return };
        let wanting: Vec<SlotId> = self
            .roster
            .slots()
            .iter()
            .filter(|s| s.lost_since.is_none() && !self.targets.contains_key(&s.id))
            .filter(|s| self.failed.get(&s.id).is_none_or(|&t| now.duration_since(t) >= RETRY))
            .map(|s| s.id)
            .collect();
        for s in wanting {
            match bus.plug_x360(&mut self.io) {
                Ok(serial) => {
                    self.failed.remove(&s);
                    let listener = Some(bus.listen(serial, self.feedback_tx.clone()));
                    self.targets.insert(
                        s,
                        Target {
                            serial,
                            player: None,
                            asking_until: Some(now + SLOT_WAIT),
                            next_ask: now,
                            last: None,
                            rumble: (0, 0),
                            rumble_sent: now,
                            rumble_to: None,
                            listener,
                        },
                    );
                    let devices = self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default();
                    for d in devices {
                        self.hide(d);
                    }
                    self.resend(s);
                }
                Err(e) => {
                    self.failed.insert(s, now);
                    self.bus_error = Some(e.to_string());
                    // Hidden by an earlier run of this slot: games must at least see the original.
                    let ids: Vec<String> = self
                        .roster
                        .get(s)
                        .map(|x| x.devices.iter().filter_map(|d| self.phys.get(d).and_then(|p| p.instance.clone())).collect())
                        .unwrap_or_default();
                    if let Some(c) = self.cloak.as_mut()
                        && let Err(e) = c.reveal(&ids)
                    {
                        self.hidhide = Driver::Failed(e.to_string());
                    }
                }
            }
            self.changed = true;
        }
    }

    /// Asks Windows which XInput slot each new virtual controller got, and lights that player
    /// number on the physical controller.
    fn ask_player_slots(&mut self, now: Instant) {
        let Some(bus) = self.bus.clone() else { return };
        let mut lit = Vec::new();
        for (&s, t) in self.targets.iter_mut() {
            if t.player.is_some() || (t.asking_until.is_none() && now < t.next_ask) {
                continue;
            }
            match bus.user_index(&mut self.io, t.serial) {
                Ok(i) => {
                    t.player = Some(i);
                    t.asking_until = None;
                    lit.push((s, i));
                }
                Err(_) => {
                    if t.asking_until.is_some_and(|until| now >= until) {
                        t.asking_until = None;
                        self.changed = true;
                    }
                    t.next_ask = now + Duration::from_secs(1);
                }
            }
        }
        for (s, player) in lit {
            for d in self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default() {
                if let Some(p) = self.phys.get(&d) {
                    p.pad.set_player(player as i32);
                }
            }
            // An XInput device in that slot, taken for an Xbox controller while the slot was
            // unknown, was this virtual controller all along.
            for p in self.phys.values_mut() {
                if p.kind == Kind::Native && device::xinput_slot(&p.pad.path()) == Some(player) {
                    p.kind = Kind::Own;
                }
            }
            self.changed = true;
        }
    }

    fn unplug(&mut self, s: SlotId) {
        self.failed.remove(&s);
        self.memo.remove(&s);
        let Some(t) = self.targets.remove(&s) else { return };
        if let Some(bus) = self.bus.as_ref() {
            let _ = bus.submit(&mut self.io, t.serial, &XusbReport::default());
            let _ = bus.unplug(&mut self.io, t.serial);
        }
        // The feedback thread ends once its controller is gone.
        drop(t.listener);
    }

    fn snapshot(&self, now: Instant) -> Snapshot {
        let mut pads = Vec::new();
        for slot in self.roster.slots() {
            let target = self.targets.get(&slot.id);
            let player = target.and_then(|t| t.player);
            let source = slot.source.and_then(|d| self.phys.get(&d));
            let any = source.or_else(|| slot.devices.iter().find_map(|d| self.phys.get(d)));
            let role = match (slot.remaining(self.roster.grace(), now), target) {
                (Some(remaining), _) => Role::Waiting { player, remaining },
                (None, Some(_)) => Role::Virtual { player },
                (None, None) => Role::Unavailable,
            };
            let mut links: Vec<Link> = source.map(|p| p.link).into_iter().collect();
            for d in &slot.devices {
                if let Some(p) = self.phys.get(d)
                    && !links.contains(&p.link)
                {
                    links.push(p.link);
                }
            }
            pads.push(PadView {
                key: PadKey::Slot(slot.id.0),
                name: self.memo.get(&slot.id).map(|m| m.0.clone()).unwrap_or_default(),
                brand: self.memo.get(&slot.id).map(|m| m.1).unwrap_or(Brand::Other),
                links,
                power: any.map(|p| p.power).unwrap_or(Power::Unknown),
                role,
                hidden: slot.devices.iter().any(|d| self.is_hidden(*d)),
                input: source.map(|p| p.state).unwrap_or_default(),
                can_power_off: source.is_some_and(|p| p.link == Link::Bluetooth && device::bluetooth_address(&p.identity).is_some()),
            });
        }
        pads.sort_by_key(|p| match &p.role {
            Role::Virtual { player: Some(i) } | Role::Waiting { player: Some(i), .. } => *i as u32,
            _ => 100,
        });
        let mut native: Vec<(&DeviceId, &Phys)> = self.phys.iter().filter(|(_, p)| p.kind == Kind::Native).collect();
        native.sort_by_key(|(_, p)| p.pad.player().unwrap_or(u8::MAX));
        for (&id, p) in native {
            pads.push(PadView {
                key: PadKey::Device(id),
                name: p.name.clone(),
                brand: p.brand,
                links: vec![p.link],
                power: p.power,
                role: Role::Native { player: device::xinput_slot(&p.pad.path()) },
                hidden: false,
                input: p.state,
                can_power_off: false,
            });
        }
        let mut unmapped: Vec<(&DeviceId, &String)> = self.unmapped.iter().collect();
        unmapped.sort();
        for (&id, name) in unmapped {
            pads.push(PadView {
                key: PadKey::Device(id),
                name: name.clone(),
                brand: Brand::Other,
                links: vec![Link::Unknown],
                power: Power::Unknown,
                role: Role::Unmapped,
                hidden: false,
                input: PadState::default(),
                can_power_off: false,
            });
        }
        let vigem = match (&self.bus, &self.bus_error) {
            (Some(_), _) => Driver::Ready { version: self.bus_version.clone() },
            (None, Some(e)) if e.contains("not installed") => Driver::Missing,
            (None, Some(e)) => Driver::Failed(e.clone()),
            (None, None) => Driver::Missing,
        };
        Snapshot {
            pads,
            vigem,
            hidhide: self.hidhide.clone(),
            hiding: self.hiding,
            sdl_version: crate::sdl::version(),
            sdl_error: None,
            running: true,
        }
    }

    fn is_hidden(&self, id: DeviceId) -> bool {
        let (Some(cloak), Some(p)) = (self.cloak.as_ref(), self.phys.get(&id)) else { return false };
        p.instance.as_deref().is_some_and(|i| cloak.is_hidden(i))
    }

    fn shutdown(&mut self) {
        for p in self.phys.values() {
            p.pad.rumble(0, 0, 0);
        }
        let slots: Vec<SlotId> = self.targets.keys().copied().collect();
        for s in slots {
            self.unplug(s);
        }
        if let Some(bus) = self.bus.take() {
            bus.cancel_all();
        }
        if let Some(mut c) = self.cloak.take()
            && let Err(e) = c.reveal_all()
        {
            // The journal stays; the next start shows them again.
            eprintln!("open-controller: could not show the hidden controllers again: {e}");
        }
        self.phys.clear();
    }
}
