//! The input thread. It owns SDL, the virtual controllers and hiding, and runs one loop:
//! read what the controllers sent, forward it to the virtual controllers, pass rumble back,
//! sleep about a millisecond. The UI only sends commands and reads snapshots.

use crate::binding::{self, Action, Bindings, Chord};
use crate::device::{self, Brand, Identity, Link, Power};
use crate::extras::{self, Art, Family, Features, Hint};
use crate::handheld;
use crate::hiding::Hider;
use crate::keyboard::Keyboard;
use crate::mapping::{self, PadState, XusbReport};
use crate::models;
use crate::motion::{self, GyroAim};
use crate::platform::{
    self, Bus, DRIVER_CHOICE, Feedback, Io, JOURNAL_FILE, PLAYER_SLOTS, VIRTUAL_PADS, VirtualDriver, bluetooth, devnode,
};
use crate::plug::Plugger;
use crate::profile::{self, GyroMode, Profile, Profiles};
use crate::report::Diagnosis;
use crate::roster::{Attached, DeviceId, Roster, SlotId};
use crate::rt;
use crate::sdl::{Event, Gamepad, Sdl};
#[cfg(windows)]
use crate::xinput;
use crossbeam_channel::{Receiver, Sender, unbounded};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Without XInput's slots there is nothing to look for: players are numbered in order.
#[cfg(not(windows))]
mod xinput {
    use crate::mapping::XusbReport;

    pub fn marker(_: u32) -> XusbReport {
        XusbReport::default()
    }

    pub fn find(_: &XusbReport) -> Option<u8> {
        None
    }

    pub fn has_free_slot() -> bool {
        false
    }
}

/// How many lines of what happened a report carries.
const LOG_LINES: usize = 80;

/// How long a virtual controller waits for its physical controller to come back.
pub const GRACE: Duration = Duration::from_secs(15);
/// Poll period while a controller is connected. SDL reads the controllers' reports when polled.
const ACTIVE_PERIOD: Duration = Duration::from_millis(1);
/// Poll period with nothing connected: only hot-plug is watched.
const IDLE_PERIOD: Duration = Duration::from_millis(20);
/// How long to look for a new virtual controller's XInput slot. It takes a few milliseconds
/// normally, and seconds the first time, while Windows installs the Xbox 360 driver.
const SLOT_WAIT: Duration = Duration::from_secs(5);
/// A controller that found no slot (a fifth one) looks again when a slot is free, this briefly,
/// at growing intervals up to `SEARCH_BACKOFF_MAX`.
const RESEARCH: Duration = Duration::from_millis(100);
const SEARCH_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// How often a controller without a slot checks whether one has freed up.
const FREE_SLOT_POLL: Duration = Duration::from_millis(500);
/// How long a request to turn a controller off is waited on; past it, the controller leaving
/// is taken as any other disconnect, with the grace period.
const TURN_OFF_WAIT: Duration = Duration::from_secs(10);
/// SDL stops a rumble after at most 65.5 s; a steady one is renewed well before.
const RUMBLE_MS: u32 = 20_000;
const RUMBLE_RENEW: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_secs(5);
const VIEW_PERIOD: Duration = Duration::from_millis(16);
/// The bits of the buttons with no Xbox equivalent in `PadState::buttons`.
const EXTRA_BUTTONS: u64 = {
    let mut m = 0u64;
    let mut i = 0;
    while i < extras::ALL.len() {
        m |= 1 << extras::ALL[i];
        i += 1;
    }
    let mut i = 0;
    while i < extras::HANDHELD.len() {
        m |= 1 << extras::HANDHELD[i];
        i += 1;
    }
    m
};
/// Below this charge, a light bar set to blink does.
const LOW_BATTERY: u8 = 15;
/// Stops the handheld button readers when the engine stops.
static HANDHELD_STOP: AtomicBool = AtomicBool::new(false);

pub struct Config {
    /// Where the HidHide journal is kept (Windows).
    pub data_dir: PathBuf,
    /// Hide the physical controllers that are turned into virtual ones from games.
    pub hide: bool,
    /// Each controller's profiles, by where its settings are kept (see [`binding::store_key`]).
    pub profiles: HashMap<String, Profiles>,
    /// The driver that makes the virtual controllers, where there is a choice.
    pub driver: VirtualDriver,
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
    /// Replaces the profiles of the controllers whose settings are kept under this key.
    SetProfiles(String, Profiles),
    /// The program in front changed (its executable's name, lowercase): profiles that name it
    /// take over.
    Foreground(String),
    /// Two controllers trade players: each takes the other's virtual controller, so no game
    /// sees a controller leave.
    SwapPlayers(PadKey, PadKey),
    /// What SDL says about a controller and what happened lately, for a report on it, with its
    /// device path for the caller to look up in the device tree away from the input thread.
    Diagnose(PadKey, Sender<Option<(Diagnosis, String)>>),
    /// Makes the virtual controllers again with this driver. Choosing VIIPER again after it
    /// failed tries it again.
    SetVirtualDriver(VirtualDriver),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Role {
    /// Games see it as an Xbox 360 controller in XInput slot `player` (0 to 3); `None` for a
    /// fifth controller and up, which XInput games cannot see. Outside Windows, `player` is its
    /// place in the order the virtual controllers were made, with no limit.
    Virtual { player: Option<u8> },
    /// The controller is away; its virtual controller stays for `remaining`.
    Waiting { player: Option<u8>, remaining: Duration },
    /// An XInput controller, which games already read directly.
    Native { player: Option<u8> },
    /// SDL has no button layout for this device.
    Unmapped,
    /// Recognised, but no virtual controller could be created.
    Unavailable,
    /// Left as it is by choice: games read the controller itself, no virtual controller is
    /// made for it and nothing is written to it.
    KeptNative,
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
    pub vendor: u16,
    pub product: u16,
    pub family: Family,
    pub art: Art,
    /// The buttons it has beyond the Xbox set, as SDL button indices.
    pub extras: Vec<u8>,
    pub features: Features,
    pub hint: Option<Hint>,
    /// Where its settings are kept; `None` for a controller whose buttons cannot be assigned
    /// (an XInput controller, which games read directly).
    pub store: Option<String>,
    pub profiles: Profiles,
    /// The profile in use right now: the chosen one, or one that names the program in front.
    pub in_use: usize,
}

impl PadView {
    pub fn profile(&self) -> &Profile {
        &self.profiles.list[self.in_use.min(self.profiles.list.len() - 1)]
    }

    /// What its extra buttons do in the profile in use.
    pub fn bindings(&self) -> &Bindings {
        &self.profile().bindings
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Driver {
    Ready {
        version: Option<String>,
    },
    Missing,
    Failed(String),
    /// This system has no such thing: macOS makes no virtual controllers and hides none.
    Unsupported,
    /// The Linux kernel has no uinput (WSL, some custom kernels), so no controller can be made.
    NoKernelSupport,
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
    /// The program in front, as profiles name programs.
    pub foreground: String,
    /// The driver making the virtual controllers, which `vigem` is the state of. ViGEmBus unless
    /// VIIPER was chosen and started.
    #[serde(default)]
    pub bus: VirtualDriver,
    /// VIIPER when it is chosen: `Ready` while it makes the controllers, `Missing` or `Failed`
    /// when it could not and ViGEmBus took over. `Unsupported` when it is not chosen.
    #[serde(default = "unsupported")]
    pub viiper: Driver,
}

fn unsupported() -> Driver {
    Driver::Unsupported
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
            foreground: String::new(),
            bus: VirtualDriver::ViGEmBus,
            viiper: Driver::Unsupported,
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

    /// Asks the input thread about a controller and looks it up in the device tree here. `None`
    /// when it is not connected or the thread does not answer within two seconds.
    pub fn diagnose(&self, key: PadKey) -> Option<Diagnosis> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.send(Command::Diagnose(key, tx));
        let (mut d, path) = rx.recv_timeout(Duration::from_secs(2)).ok()??;
        d.facts.devices = devnode::ancestry(&path);
        Some(d)
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

/// What a controller is, kept for its slot while it is away.
#[derive(Clone)]
struct Model {
    name: String,
    brand: Brand,
    vendor: u16,
    product: u16,
    family: Family,
    art: Art,
    extras: Vec<u8>,
    features: Features,
    hint: Option<Hint>,
    store: String,
}

/// A connected device, as SDL sees it.
struct Phys {
    pad: Gamepad,
    model: Model,
    link: Link,
    identity: Identity,
    /// SDL's device path, read once: it never changes while the device is connected.
    path: String,
    /// Device instance id, for HidHide.
    instance: Option<String>,
    kind: Kind,
    state: PadState,
    power: Power,
    /// The colour last given its light bar.
    led: Option<[u8; 3]>,
    /// Whether its gyro reports are on, whether a toggle turned the gyro on, the gyro's push on
    /// the right stick and when it was last read.
    gyro_on: bool,
    gyro_toggled: bool,
    aim: GyroAim,
    aim_out: (i16, i16),
    aim_at: Option<Instant>,
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

/// A joystick SDL has no button layout for.
struct Unmapped {
    name: String,
    vendor: u16,
    product: u16,
}

/// Who holds a key down: a slot (its virtual controller's controller), or an XInput controller
/// of its own (a handheld's built-in one).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Owner {
    Slot(SlotId),
    Device(DeviceId),
}

/// A virtual controller.
struct Target {
    serial: u32,
    player: Option<u8>,
    /// While its XInput slot is looked for: the marker report it shows meanwhile, and until
    /// when. Input waits until the slot is found; XInput devices seen meanwhile wait to be
    /// classified.
    search: Option<(XusbReport, Instant)>,
    /// Without a slot, when to look again, and the interval after that.
    next_search: Instant,
    backoff: Duration,
    /// What the game should see.
    want: XusbReport,
    /// What the bus last took. Right after plugging in, it refuses a report or two until the
    /// Xbox 360 driver has opened the controller; a refused report is sent again next pass.
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
    bus_error: Option<Driver>,
    bus_retry: Instant,
    /// The driver chosen, the one making the controllers, and why VIIPER is not when it was
    /// chosen and failed. It is tried once per choice.
    driver: VirtualDriver,
    bus_driver: VirtualDriver,
    viiper_error: Option<String>,
    io: Io,
    /// Plugs virtual controllers in away from this thread; there once the bus is.
    plugger: Option<Plugger>,
    /// HidHide, on a thread of its own.
    hider: Hider,
    hiding: bool,
    roster: Roster,
    phys: HashMap<DeviceId, Phys>,
    unmapped: HashMap<DeviceId, Unmapped>,
    targets: HashMap<SlotId, Target>,
    failed: HashMap<SlotId, Instant>,
    feedback_tx: Sender<Feedback>,
    feedback_rx: Receiver<Feedback>,
    /// Controllers rumbling to be identified, and until when.
    identify: Vec<(DeviceId, Instant)>,
    /// What each slot's controller is, still shown while it is away.
    memo: HashMap<SlotId, Model>,
    /// USB product names by vendor and product id, for XInput controllers SDL cannot name.
    names: HashMap<(u16, u16), Option<String>>,
    /// Devices turned off from here: when one leaves and its slot is empty, the virtual
    /// controller is unplugged at once, without the grace period.
    turning_off: Vec<(DeviceId, Instant)>,
    profiles: HashMap<String, Profiles>,
    keyboard: Keyboard,
    /// Keys held down for extra buttons that are held down, to be let go with them.
    held: Vec<(Owner, u8, Chord)>,
    started: Instant,
    /// The program in front, which can switch profiles.
    foreground: String,
    /// The handheld PC this is, if any, and what its built-in controller's own buttons hold.
    handheld: Option<&'static handheld::Machine>,
    handheld_rx: Receiver<handheld::Held>,
    handheld_held: u64,
    updated: Vec<DeviceId>,
    /// Controllers coming and going, for reports: the last `LOG_LINES`.
    log: VecDeque<String>,
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
    let hider = Hider::start(config.data_dir.join(JOURNAL_FILE));
    let (feedback_tx, feedback_rx) = unbounded();
    let (handheld_tx, handheld_rx) = unbounded();
    HANDHELD_STOP.store(false, Ordering::Relaxed);
    handheld::start(handheld_tx, &HANDHELD_STOP);
    let mut core = Core {
        bus: None,
        bus_version: None,
        bus_error: None,
        bus_retry: Instant::now(),
        driver: config.driver,
        bus_driver: VirtualDriver::ViGEmBus,
        viiper_error: None,
        io: Io::new(),
        plugger: None,
        hider,
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
        names: HashMap::new(),
        turning_off: Vec::new(),
        profiles: config.profiles,
        keyboard: Keyboard::start(),
        held: Vec::new(),
        started: Instant::now(),
        foreground: String::new(),
        handheld: handheld::this_machine(),
        handheld_rx,
        handheld_held: 0,
        updated: Vec::with_capacity(16),
        log: VecDeque::new(),
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
        core.handheld_buttons();
        core.forward(now);
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
    HANDHELD_STOP.store(true, Ordering::Relaxed);
    core.shutdown();
    publish(Snapshot::default());
}

impl Core {
    fn connect_bus(&mut self) {
        let want = if DRIVER_CHOICE && self.viiper_error.is_none() { self.driver } else { VirtualDriver::ViGEmBus };
        let mut result = platform::connect_bus(want);
        self.bus_driver = want;
        if want == VirtualDriver::Viiper
            && let Err(e) = &result
        {
            let why = e.to_string();
            self.note(format!("VIIPER: {why}; ViGEmBus makes the controllers instead"));
            self.viiper_error = Some(why);
            self.bus_driver = VirtualDriver::ViGEmBus;
            result = platform::connect_bus(VirtualDriver::ViGEmBus);
        }
        match result {
            Ok(b) => {
                self.bus_version = platform::bus_version(&b);
                let bus = Arc::new(b);
                self.plugger = Some(Plugger::start(bus.clone()));
                self.bus = Some(bus);
                self.bus_error = None;
            }
            Err(e) => {
                let state = e.driver();
                if self.bus_error.as_ref() != Some(&state) {
                    self.note(format!("virtual controller bus: {e}"));
                }
                self.bus_error = Some(state);
            }
        }
        self.bus_retry = Instant::now() + RETRY;
        self.changed = true;
    }

    /// VIIPER stopped working: its controllers are made again with ViGEmBus.
    fn leave_viiper(&mut self, why: String) {
        self.note(format!("VIIPER: {why}; ViGEmBus makes the controllers instead"));
        self.viiper_error = Some(why);
        self.close_bus();
        self.connect_bus();
    }

    /// Unplugs every virtual controller and lets go of the bus. The slots keep their controllers,
    /// shown to games again until the next bus gives them new virtual controllers.
    fn close_bus(&mut self) {
        let targets: Vec<(SlotId, Target)> = self.targets.drain().collect();
        let mut shown = Vec::new();
        for (s, t) in targets {
            if let Some(p) = t.rumble_to.and_then(|d| self.phys.get(&d)) {
                p.pad.rumble(0, 0, 0);
            }
            if let Some(bus) = self.bus.as_ref() {
                let _ = bus.submit(&mut self.io, t.serial, &XusbReport::default());
                let _ = bus.unplug(&mut self.io, t.serial);
            }
            let devices = self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default();
            shown.extend(devices.iter().filter_map(|d| self.phys.get(d).and_then(|p| p.instance.clone())));
        }
        self.hider.reveal(shown);
        // A plug under way finishes and is undone.
        if let Some(mut p) = self.plugger.take() {
            p.stop();
        }
        if let Some(bus) = self.bus.take() {
            bus.cancel_all();
        }
        self.failed.clear();
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
                } else {
                    self.hider.reveal_all();
                }
            }
            Command::Identify(key) => {
                if let Some(dev) = self.device_for(key)
                    && !self.roster.slot_of(dev).is_some_and(|s| self.is_native(s))
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
                    self.turning_off.push((dev, now));
                    // A blocking request to each radio: kept off the input thread.
                    std::thread::spawn(move || bluetooth::disconnect(a));
                }
            }
            Command::Watch(_) => {}
            Command::SetProfiles(store, profiles) => {
                let devices: Vec<DeviceId> = self.phys.iter().filter(|(_, p)| p.model.store == store).map(|(&id, _)| id).collect();
                for &d in &devices {
                    self.release_keys(self.owner_of(d));
                }
                let profiles = profiles.sanitised();
                let (was_native, now_native) = (native(&self.profiles, &store), profiles.native);
                if profiles.is_default() {
                    self.profiles.remove(&store);
                } else {
                    self.profiles.insert(store.clone(), profiles);
                }
                if was_native != now_native {
                    self.set_native(&store, now_native);
                }
                self.profiles_changed(&devices);
            }
            Command::Foreground(program) => {
                if program != self.foreground {
                    let before: Vec<(DeviceId, usize)> = self.in_use_by_device();
                    self.foreground = program;
                    // Only controllers whose profile in use changed are touched.
                    let devices: Vec<DeviceId> =
                        self.in_use_by_device().into_iter().filter(|x| !before.contains(x)).map(|(d, _)| d).collect();
                    for &d in &devices {
                        self.release_keys(self.owner_of(d));
                    }
                    self.profiles_changed(&devices);
                }
            }
            Command::SwapPlayers(PadKey::Slot(a), PadKey::Slot(b)) if a != b => self.swap(SlotId(a), SlotId(b)),
            Command::SwapPlayers(..) => {}
            Command::Diagnose(key, reply) => {
                let found = self.device_for(key).and_then(|d| match self.phys.get(&d) {
                    Some(p) => Some((p.pad.facts(), p.path.clone())),
                    None => self.unmapped.contains_key(&d).then(|| (self.sdl.joystick_facts(d), String::new())),
                });
                let _ = reply.send(found.map(|(facts, path)| (Diagnosis { facts, log: self.log.iter().cloned().collect() }, path)));
                return;
            }
            Command::SetVirtualDriver(_) if !DRIVER_CHOICE => {}
            Command::SetVirtualDriver(d) => {
                let retry = d == VirtualDriver::Viiper && self.viiper_error.is_some();
                self.driver = d;
                self.viiper_error = None;
                if retry || self.bus.is_none() || self.bus_driver != d {
                    self.note(format!("virtual controllers now from {d:?}"));
                    self.close_bus();
                    self.connect_bus();
                }
            }
        }
        self.changed = true;
    }

    fn note(&mut self, line: String) {
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(format!("{:>8.1}s  {line}", self.started.elapsed().as_secs_f32()));
    }

    /// Each controller with settings, and which of its profiles is in use.
    fn in_use_by_device(&self) -> Vec<(DeviceId, usize)> {
        self.phys.iter().filter_map(|(&d, p)| self.profiles.get(&p.model.store).map(|pr| (d, pr.in_use(&self.foreground)))).collect()
    }

    /// After a profile change: the gyro is turned on or off to match, and the virtual
    /// controllers get the new assignments at once.
    fn profiles_changed(&mut self, devices: &[DeviceId]) {
        for d in devices {
            if let Some(p) = self.phys.get_mut(d) {
                p.gyro_toggled = false;
            }
        }
        self.sync_gyro();
        let slots: Vec<SlotId> = devices.iter().filter_map(|&d| self.roster.slot_of(d)).collect();
        for s in slots {
            self.resend(s);
        }
    }

    /// Whether the slot's controller is kept native: no virtual controller, not hidden, and
    /// nothing written to it.
    fn is_native(&self, s: SlotId) -> bool {
        self.memo.get(&s).is_some_and(|m| native(&self.profiles, &m.store))
    }

    /// The controllers kept under `store` were made native or Xbox controllers again; the
    /// profiles already say which, so `plug_missing` never plugs a native one back in. Going
    /// native, the virtual controller lets go of everything and leaves before games are shown
    /// the original. Coming back, `plug_missing` makes a virtual controller, which hides it
    /// again, and the light, player and gyro are set anew.
    fn set_native(&mut self, store: &str, on: bool) {
        let slots: Vec<SlotId> = self.memo.iter().filter(|(_, m)| m.store == store).map(|(&s, _)| s).collect();
        for s in slots {
            let devices = self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default();
            self.note(format!("slot {} {}", s.0, if on { "kept native" } else { "back to Xbox" }));
            if !on {
                for d in &devices {
                    if let Some(p) = self.phys.get_mut(d) {
                        p.led = None;
                    }
                }
                continue;
            }
            for d in &devices {
                if let Some(p) = self.phys.get(d) {
                    p.pad.rumble(0, 0, 0);
                    p.pad.set_player(-1);
                }
            }
            self.sync_gyro();
            // Unplugging forgets what the controller is, and it is still here.
            let model = self.memo.get(&s).cloned();
            self.unplug(s);
            if let Some(m) = model {
                self.memo.insert(s, m);
            }
            let ids: Vec<String> = devices.iter().filter_map(|d| self.phys.get(d).and_then(|p| p.instance.clone())).collect();
            self.hider.reveal(ids);
        }
    }

    /// Two slots trade virtual controllers, and so players: games see the same controllers
    /// with different hands on them.
    fn swap(&mut self, a: SlotId, b: SlotId) {
        if self.roster.get(a).is_none() || self.roster.get(b).is_none() || self.is_native(a) || self.is_native(b) {
            return;
        }
        self.release_keys(Owner::Slot(a));
        self.release_keys(Owner::Slot(b));
        let ta = self.targets.remove(&a);
        let tb = self.targets.remove(&b);
        for (slot, target) in [(a, tb), (b, ta)] {
            if let Some(mut t) = target {
                // The rumble the game asked of this virtual controller moves to its new hands.
                if let Some(p) = t.rumble_to.and_then(|d| self.phys.get(&d)) {
                    p.pad.rumble(0, 0, 0);
                }
                t.rumble_to = None;
                self.targets.insert(slot, t);
            }
        }
        for s in [a, b] {
            let player = self.targets.get(&s).and_then(|t| t.player);
            for d in self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default() {
                if let Some(p) = self.phys.get(&d) {
                    p.pad.set_player(player.map_or(-1, i32::from));
                }
            }
            self.resend(s);
            self.apply_rumble(s);
        }
    }

    fn owner_of(&self, d: DeviceId) -> Owner {
        match self.phys.get(&d).map(|p| p.kind) {
            Some(Kind::Slot(s)) => Owner::Slot(s),
            _ => Owner::Device(d),
        }
    }

    /// The profile in use for the controllers kept under `store`.
    fn profile_of(&self, store: &str) -> Option<&Profile> {
        self.profiles.get(store).map(|p| p.for_program(&self.foreground))
    }

    /// Turns each controller's gyro reports on while its profile uses them.
    fn sync_gyro(&mut self) {
        let ids: Vec<DeviceId> = self.phys.keys().copied().collect();
        for id in ids {
            let want = {
                let p = &self.phys[&id];
                matches!(p.kind, Kind::Slot(s) if !self.is_native(s))
                    && p.model.features.motion
                    && self.profile_of(&p.model.store).is_some_and(|pr| pr.gyro.mode != GyroMode::Off)
            };
            let p = self.phys.get_mut(&id).unwrap();
            if p.gyro_on != want {
                p.pad.set_gyro(want);
                p.gyro_on = want;
                p.aim.reset();
                p.aim_out = (0, 0);
                p.aim_at = None;
            }
        }
    }

    /// What the handheld's own buttons hold, merged into its built-in controller's state.
    fn handheld_buttons(&mut self) {
        let mut changed = false;
        while let Ok(held) = self.handheld_rx.try_recv() {
            self.handheld_held = held;
            changed = true;
        }
        if changed
            && let Some((&id, _)) = self.phys.iter().find(|(_, p)| p.model.family == Family::Handheld && p.kind == Kind::Native)
            && !self.updated.contains(&id)
        {
            self.updated.push(id);
        }
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
                        p.power = device::power(p.pad.power(), device::is_xinput_path(&p.path));
                        self.changed = true;
                    }
                }
                Event::JoystickAdded(id) => {
                    if !self.sdl.is_gamepad(id) {
                        let (name, vendor, product) = self.sdl.joystick_info(id);
                        self.note(format!("joystick without a gamepad mapping {vendor:04x}:{product:04x} \"{name}\""));
                        self.unmapped.insert(id, Unmapped { name, vendor, product });
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
        let mut link = device::link(vendor, product, &path, pad.connection());
        if devnode::is_bluetooth(&path) {
            link = Link::Bluetooth;
        }
        let mut identity = device::identity(vendor, product, pad.serial().as_deref(), &path);
        // One controller cannot be connected twice the same way. Two devices with one serial and
        // one kind of link are two controllers that share a serial, as some clones do.
        if self.shares_link(&identity, link) {
            identity = device::identity(vendor, product, None, &path);
        }
        let mut name = pad.name();
        if device::is_xinput_path(&path) && name.starts_with("XInput ") {
            let usb = self.names.entry((vendor, product)).or_insert_with(|| devnode::usb_product_name(vendor, product));
            if let Some(n) = usb {
                name = n.clone();
            }
        }
        if models::is_generic_name(&name)
            && let Some(row) = models::lookup(vendor, product)
        {
            name = row.3.to_string();
        }
        if let Some(fields) = extras::missing_mapping(vendor, product) {
            pad.extend_mapping(fields);
        }
        let mut family = extras::family(vendor, product, pad.pad_type());
        // On a handheld PC, its XInput pad is the built-in controller, whatever id it gives.
        let built_in = self.handheld.filter(|_| device::is_xinput_path(&path) && !devnode::is_virtual(&path));
        if let Some(m) = built_in {
            family = Family::Handheld;
            if models::is_generic_name(&name) || name.starts_with("Xbox 360") {
                name = m.name.to_string();
            }
        }
        let features = pad.features();
        let mut model = Model {
            name,
            brand: device::brand(vendor, pad.pad_type()),
            vendor,
            product,
            family,
            art: extras::art(family, vendor, product),
            extras: extras::available(&features, |b| pad.has_button(b)),
            features,
            hint: extras::hint(vendor, product, family),
            store: binding::store_key(&identity, vendor, product),
        };
        if let Some(m) = built_in {
            model.extras = extras::HANDHELD[..m.buttons.len()].to_vec();
            model.store = format!("handheld:{}", m.name);
        }
        let mut p = Phys {
            model,
            link,
            identity,
            instance: devnode::instance_id(&path),
            power: device::power(pad.power(), device::is_xinput_path(&path)),
            path,
            kind: Kind::Pending,
            state: PadState::default(),
            led: None,
            gyro_on: false,
            gyro_toggled: false,
            aim: GyroAim::default(),
            aim_out: (0, 0),
            aim_at: None,
            pad,
        };
        p.pad.read(&mut p.state);
        let xbox = matches!(p.pad.pad_type(), device::PadType::Xbox360 | device::PadType::XboxOne);
        p.kind = if devnode::is_virtual(&p.path) {
            Kind::Own
        } else if device::is_xinput_path(&p.path) {
            self.classify_xinput(&p.path).unwrap_or(Kind::Pending)
        } else if !VIRTUAL_PADS || devnode::is_native(&p.path, xbox) {
            Kind::Native
        } else {
            match self.roster.attach(id, p.identity.clone()) {
                Attached::New(s) | Attached::Rejoined(s) => Kind::Slot(s),
            }
        };
        let kind = p.kind;
        self.note(format!(
            "added {vendor:04x}:{product:04x} \"{}\" as \"{}\" ({:?}, {:?}, {:?})",
            p.pad.name(),
            p.model.name,
            p.model.family,
            p.link,
            kind
        ));
        if let Kind::Slot(s) = kind {
            self.memo.insert(s, p.model.clone());
        }
        self.phys.insert(id, p);
        if let Kind::Slot(s) = kind {
            if self.is_native(s) {
                // SDL numbered it when it opened; without a number, numbering another controller
                // never moves this one and rewrites its lights.
                self.phys[&id].pad.set_player(-1);
            } else {
                self.hide(id);
                if let Some(player) = self.targets.get(&s).and_then(|t| t.player) {
                    self.phys[&id].pad.set_player(player as i32);
                }
                self.resend(s);
                self.move_rumble(s);
            }
        }
        self.sync_gyro();
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
        if self.targets.values().any(|t| t.player.is_none() && t.search.is_some()) {
            return None;
        }
        Some(Kind::Native)
    }

    fn removed(&mut self, id: DeviceId, now: Instant) {
        self.release_keys(self.owner_of(id));
        let Some(p) = self.phys.remove(&id) else { return };
        self.note(format!("removed {:04x}:{:04x} \"{}\" ({:?})", p.model.vendor, p.model.product, p.model.name, p.link));
        self.updated.retain(|&d| d != id);
        if let Kind::Slot(s) = p.kind {
            self.roster.detach(id, now);
            let empty = self.roster.get(s).is_some_and(|slot| slot.devices.is_empty());
            let turned_off = self.turning_off.iter().any(|&(x, at)| x == id && now.duration_since(at) < TURN_OFF_WAIT);
            self.turning_off.retain(|&(x, at)| x != id && now.duration_since(at) < TURN_OFF_WAIT);
            if empty && (turned_off || self.is_native(s)) {
                // Turned off on purpose, or kept native with no virtual controller to keep: no
                // point waiting for it to come back.
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
        let Some(p) = self.phys.get(&id) else { return };
        // Only a controller games can see as an Xbox controller is hidden; hiding one whose
        // virtual controller does not exist would take it away from games altogether.
        let Kind::Slot(s) = p.kind else { return };
        if !self.targets.contains_key(&s) || self.is_native(s) {
            return;
        }
        if let Some(instance) = p.instance.clone() {
            self.hider.hide(vec![instance]);
        }
    }

    /// What the hiding thread did: its state for the settings, and, once HidHide opens after
    /// failing to (another program held it, as at sign-in), the controllers hidden after all.
    fn follow_hider(&mut self) {
        let (changed, opened) = self.hider.poll();
        self.changed |= changed;
        if opened {
            let ids: Vec<DeviceId> = self.phys.keys().copied().collect();
            for id in ids {
                self.hide(id);
            }
        }
    }

    /// Sends a slot's current state: after its source changed or sent input, or to a new
    /// virtual controller.
    fn resend(&mut self, s: SlotId) {
        let report = self.roster.get(s).and_then(|slot| slot.source).map(|d| self.report_of(d)).unwrap_or_default();
        let Some(t) = self.targets.get_mut(&s) else { return };
        t.want = report;
        self.send(s);
    }

    /// The Xbox report for a device's state, with the Xbox buttons its extra buttons stand for.
    fn report_of(&self, dev: DeviceId) -> XusbReport {
        let Some(p) = self.phys.get(&dev) else { return XusbReport::default() };
        let profile = self.profile_of(&p.model.store);
        let mut state = p.state;
        if let Some(pr) = profile {
            let (lx, ly) = motion::shape_stick(state.axes[mapping::axis::LEFT_X], state.axes[mapping::axis::LEFT_Y], pr.sticks);
            let (rx, ry) = motion::shape_stick(state.axes[mapping::axis::RIGHT_X], state.axes[mapping::axis::RIGHT_Y], pr.sticks);
            state.axes[mapping::axis::LEFT_X] = lx;
            state.axes[mapping::axis::LEFT_Y] = ly;
            state.axes[mapping::axis::RIGHT_X] = rx;
            state.axes[mapping::axis::RIGHT_Y] = ry;
        }
        let mut r = mapping::to_xusb(&state);
        // The gyro adds to the right stick, so the stick still turns and the gyro fine-tunes.
        if p.aim_out != (0, 0) {
            r.thumb_rx = (i32::from(r.thumb_rx) + i32::from(p.aim_out.0)).clamp(-32768, 32767) as i16;
            r.thumb_ry = (i32::from(r.thumb_ry) + i32::from(p.aim_out.1)).clamp(-32768, 32767) as i16;
        }
        if let Some(b) = profile.map(|pr| &pr.bindings) {
            let (bits, lt, rt) = binding::xbox_overlay(b, p.state.buttons);
            r.buttons |= bits;
            if lt {
                r.left_trigger = u8::MAX;
            }
            if rt {
                r.right_trigger = u8::MAX;
            }
        }
        r
    }

    /// Brings the keys an owner holds in line with its controller's extra buttons, and starts
    /// the macros of the buttons just pressed. Held keys follow the buttons' state rather than
    /// their release, which can come from another connection of the controller than the press.
    fn type_keys(&mut self, s: Owner, dev: DeviceId, before: u64, after: u64) {
        let program = self.foreground.as_str();
        let bindings = self.phys.get(&dev).and_then(|p| self.profiles.get(&p.model.store)).map(|p| &p.for_program(program).bindings);
        let mut i = 0;
        while i < self.held.len() {
            let (owner, button, chord) = self.held[i];
            let still = owner != s
                || (after & 1 << button != 0 && matches!(bindings.and_then(|b| b.get(&button)), Some(Action::Keys(c)) if *c == chord));
            if still {
                i += 1;
            } else {
                self.keyboard.release(chord);
                self.held.remove(i);
            }
        }
        let Some(b) = bindings else { return };
        for (&button, action) in b {
            if after & 1 << button == 0 {
                continue;
            }
            match action {
                Action::Keys(c) if !self.held.iter().any(|h| h.0 == s && h.1 == button) => {
                    self.keyboard.press(*c);
                    self.held.push((s, button, *c));
                }
                Action::Macro(steps) if before & 1 << button == 0 => self.keyboard.play(steps.clone()),
                _ => {}
            }
        }
    }

    /// Gives each light bar the colour its profile asks for: the player's, a chosen one, the
    /// battery's or none, blinking once a second when the battery is nearly empty. Only a change
    /// is sent.
    fn update_lights(&mut self, now: Instant) {
        // Half a second on, half off.
        let blink_off = now.duration_since(self.started).as_millis() % 1000 >= 500;
        for p in self.phys.values_mut() {
            // Where controllers stay as they are, their lights are still OpenController's to set.
            let player = match p.kind {
                Kind::Slot(_) if native(&self.profiles, &p.model.store) => continue,
                Kind::Slot(s) => self.targets.get(&s).and_then(|t| t.player),
                Kind::Native if !VIRTUAL_PADS => None,
                _ => continue,
            };
            if !p.model.features.light_bar {
                continue;
            }
            let battery = match p.power {
                Power::Battery(l) | Power::Charging(l) => l,
                _ => None,
            };
            let default = Profile::default();
            let pr = self.profiles.get(&p.model.store).map(|x| x.for_program(&self.foreground)).unwrap_or(&default);
            let low = matches!(p.power, Power::Battery(Some(l)) if l <= LOW_BATTERY);
            let color = if pr.low_battery_flash && low && blink_off {
                [0, 0, 0]
            } else {
                profile::light_color(pr.light, pr.brightness, player, battery)
            };
            if p.led != Some(color) {
                p.pad.set_led(color);
                p.led = Some(color);
            }
        }
    }

    /// Lets go of every key an owner holds down: its controller left, or its assignments changed.
    fn release_keys(&mut self, s: Owner) {
        let (mine, rest): (Vec<_>, Vec<_>) = self.held.drain(..).partition(|h| h.0 == s);
        self.held = rest;
        for (_, _, c) in mine {
            self.keyboard.release(c);
        }
    }

    /// Sends a virtual controller what it should show: its marker while its slot is looked
    /// for, its input otherwise.
    fn send(&mut self, s: SlotId) {
        let (Some(bus), Some(t)) = (self.bus.as_ref(), self.targets.get_mut(&s)) else { return };
        let report = t.search.map(|(marker, _)| marker).unwrap_or(t.want);
        if t.last != Some(report) && bus.submit(&mut self.io, t.serial, &report).is_ok() {
            t.last = Some(report);
        }
    }

    /// Forwards every controller that sent a report since the last pass.
    fn forward(&mut self, now: Instant) {
        let updated = std::mem::take(&mut self.updated);
        for &id in &updated {
            let Some(p) = self.phys.get_mut(&id) else { continue };
            // One of our own virtual controllers, reporting back what was just sent to it.
            if p.kind == Kind::Own {
                continue;
            }
            let before = p.state;
            let aim_before = p.aim_out;
            p.pad.read(&mut p.state);
            if p.model.family == Family::Handheld && p.kind == Kind::Native {
                p.state.buttons |= self.handheld_held;
            }
            if p.gyro_on {
                let gyro = self.profiles.get(&p.model.store).map(|x| x.for_program(&self.foreground).gyro).unwrap_or_default();
                let active = motion::gyro_active(&gyro, &p.state, &before, &mut p.gyro_toggled);
                match (active, p.pad.gyro()) {
                    (true, Some(rate)) => {
                        let dt = p.aim_at.map_or(0.004, |t| now.duration_since(t).as_secs_f32());
                        p.aim_out = p.aim.update(rate, dt, gyro, motion::aiming(&p.state));
                        p.aim_at = Some(now);
                    }
                    _ => {
                        p.aim.reset();
                        p.aim_out = (0, 0);
                        p.aim_at = None;
                    }
                }
            }
            if p.state == before && p.aim_out == aim_before {
                continue;
            }
            self.input_changed = true;
            let (kind, state) = (p.kind, p.state);
            if kind == Kind::Native && (before.buttons ^ state.buttons) & EXTRA_BUTTONS != 0 {
                self.type_keys(Owner::Device(id), id, before.buttons, state.buttons);
            }
            if let Kind::Slot(s) = kind {
                // Stick noise on the idle connection of a controller connected twice must not
                // pull the slot over to it.
                if mapping::significant(&before, &state) && self.roster.touched(id) {
                    self.changed = true;
                    self.move_rumble(s);
                }
                if self.roster.get(s).and_then(|x| x.source) == Some(id) && !self.is_native(s) {
                    self.resend(s);
                    if (before.buttons ^ state.buttons) & EXTRA_BUTTONS != 0 || !self.held.is_empty() {
                        self.type_keys(Owner::Slot(s), id, before.buttons, state.buttons);
                    }
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
        if let Some(why) = self.bus.as_deref().and_then(platform::bus_lost) {
            self.leave_viiper(why);
        }
        self.follow_hider();
        self.plug_missing(now);
        self.find_player_slots(now);
        self.update_lights(now);
        let unsent: Vec<SlotId> = self
            .targets
            .iter()
            .filter(|(_, t)| t.last != Some(t.search.map(|(marker, _)| marker).unwrap_or(t.want)))
            .map(|(&s, _)| s)
            .collect();
        for s in unsent {
            self.send(s);
        }
        let pending: Vec<DeviceId> = self.phys.iter().filter(|(_, p)| p.kind == Kind::Pending).map(|(&id, _)| id).collect();
        for id in pending {
            if let Some(k) = self.classify_xinput(&self.phys[&id].path) {
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

    /// Asks for a virtual controller for every slot that has a controller and none yet, and
    /// sets up the ones that are ready.
    fn plug_missing(&mut self, now: Instant) {
        let Some(plugger) = self.plugger.as_ref() else { return };
        let wanting: Vec<SlotId> = self
            .roster
            .slots()
            .iter()
            .filter(|s| s.lost_since.is_none() && !self.targets.contains_key(&s.id) && !plugger.is_pending(s.id))
            .filter(|s| self.failed.get(&s.id).is_none_or(|&t| now.duration_since(t) >= RETRY))
            .filter(|s| !self.is_native(s.id))
            .map(|s| s.id)
            .collect();
        let Some(plugger) = self.plugger.as_mut() else { return };
        for s in wanting {
            plugger.request(s);
        }
        let mut viiper_failed = None;
        for (s, result) in plugger.finished() {
            match result {
                Ok(serial) => self.plugged(s, serial, now),
                Err(e) => {
                    if self.bus_driver == VirtualDriver::Viiper {
                        viiper_failed = Some(e.clone());
                    }
                    self.failed.insert(s, now);
                    self.bus_error = Some(Driver::Failed(e));
                    // Hidden by an earlier run of this slot: games must at least see the original.
                    let ids: Vec<String> = self
                        .roster
                        .get(s)
                        .map(|x| x.devices.iter().filter_map(|d| self.phys.get(d).and_then(|p| p.instance.clone())).collect())
                        .unwrap_or_default();
                    self.hider.reveal(ids);
                }
            }
            self.changed = true;
        }
        // A VIIPER that starts but cannot attach a controller (a usbip-win2 it does not know)
        // gives none at all: ViGEmBus takes over.
        if let Some(e) = viiper_failed {
            self.leave_viiper(e);
        }
    }

    /// A virtual controller for `s` is ready: it gets the slot's input, rumble and player.
    fn plugged(&mut self, s: SlotId, serial: u32, now: Instant) {
        let Some(bus) = self.bus.clone() else { return };
        // The controller was turned off, stayed away or was kept native while it was being
        // plugged in.
        if self.roster.get(s).is_none() || self.targets.contains_key(&s) || self.is_native(s) {
            let _ = bus.unplug(&mut self.io, serial);
            return;
        }
        self.failed.remove(&s);
        let listener = Some(bus.listen(serial, self.feedback_tx.clone()));
        self.targets.insert(
            s,
            Target {
                serial,
                player: None,
                search: PLAYER_SLOTS.then(|| (xinput::marker(serial), now + SLOT_WAIT)),
                next_search: now,
                backoff: Duration::from_secs(1),
                want: XusbReport::default(),
                last: None,
                rumble: (0, 0),
                rumble_sent: now,
                rumble_to: None,
                listener,
            },
        );
        let devices = self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default();
        for &d in &devices {
            self.hide(d);
        }
        if !PLAYER_SLOTS {
            self.number_players();
        }
        self.resend(s);
    }

    /// Finds which XInput slot each new virtual controller got, by looking for its marker
    /// where games look, and lights that player number on the physical controller.
    fn find_player_slots(&mut self, now: Instant) {
        if !PLAYER_SLOTS {
            return;
        }
        let mut lit = Vec::new();
        let mut free = None;
        for (&s, t) in self.targets.iter_mut() {
            if t.player.is_some() {
                continue;
            }
            match t.search {
                Some((marker, until)) => {
                    if let Some(i) = (t.last == Some(marker)).then(|| xinput::find(&marker)).flatten() {
                        t.player = Some(i);
                        t.search = None;
                        lit.push((s, i));
                    } else if now >= until {
                        // No slot: a fifth controller, which XInput games cannot see.
                        t.search = None;
                        t.next_search = now + t.backoff;
                        t.backoff = (t.backoff * 2).min(SEARCH_BACKOFF_MAX);
                        self.changed = true;
                    }
                }
                None if now >= t.next_search => {
                    if *free.get_or_insert_with(xinput::has_free_slot) {
                        t.search = Some((xinput::marker(t.serial), now + RESEARCH));
                    } else {
                        // All four taken: look again later, not on every pass.
                        t.next_search = now + FREE_SLOT_POLL;
                    }
                }
                None => {}
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
                if p.kind == Kind::Native && device::xinput_slot(&p.path) == Some(player) {
                    p.kind = Kind::Own;
                }
            }
            self.changed = true;
        }
    }

    /// Without XInput, players are numbered in the order their virtual controllers were made,
    /// with no limit, as games list them; when one goes, the later ones move up.
    fn number_players(&mut self) {
        let serials: Vec<(SlotId, u32)> = self.targets.iter().map(|(&s, t)| (s, t.serial)).collect();
        for &(s, serial) in &serials {
            let place = serials.iter().filter(|&&(_, other)| other < serial).count();
            let player = Some(u8::try_from(place).unwrap_or(u8::MAX));
            let Some(t) = self.targets.get_mut(&s) else { continue };
            if t.player == player {
                continue;
            }
            t.player = player;
            for d in self.roster.get(s).map(|x| x.devices.clone()).unwrap_or_default() {
                if let Some(p) = self.phys.get(&d) {
                    p.pad.set_player(place as i32);
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
        if !PLAYER_SLOTS {
            self.number_players();
        }
    }

    fn snapshot(&self, now: Instant) -> Snapshot {
        let mut pads = Vec::new();
        for slot in self.roster.slots() {
            let target = self.targets.get(&slot.id);
            let source = slot.source.and_then(|d| self.phys.get(&d));
            let any = source.or_else(|| slot.devices.iter().find_map(|d| self.phys.get(d)));
            let role = slot_role(self.is_native(slot.id), slot.remaining(self.roster.grace(), now), target.map(|t| t.player));
            let mut links: Vec<Link> = source.map(|p| p.link).into_iter().collect();
            for d in &slot.devices {
                if let Some(p) = self.phys.get(d)
                    && !links.contains(&p.link)
                {
                    links.push(p.link);
                }
            }
            let model = any.map(|p| &p.model).or_else(|| self.memo.get(&slot.id));
            let mut view = self.view(PadKey::Slot(slot.id.0), model, true);
            view.links = links;
            view.power = any.map(|p| p.power).unwrap_or(Power::Unknown);
            view.role = role;
            view.hidden = slot.devices.iter().any(|d| self.is_hidden(*d));
            view.input = source.map(|p| p.state).unwrap_or_default();
            view.can_power_off = source.is_some_and(|p| p.link == Link::Bluetooth && device::bluetooth_address(&p.identity).is_some());
            pads.push(view);
        }
        let mut native: Vec<(&DeviceId, &Phys)> = self.phys.iter().filter(|(_, p)| p.kind == Kind::Native).collect();
        native.sort_by_key(|(_, p)| p.pad.player().unwrap_or(u8::MAX));
        for (&id, p) in native {
            // A handheld's built-in controller keeps its own buttons' assignments, and so does
            // every controller where games read them directly anyway (outside Windows).
            let assignable = p.model.family == Family::Handheld || !cfg!(windows);
            let mut view = self.view(PadKey::Device(id), Some(&p.model), assignable);
            view.links = vec![p.link];
            view.power = p.power;
            let player = device::xinput_slot(&p.path).or_else(|| if VIRTUAL_PADS { None } else { p.pad.player() });
            view.role = Role::Native { player };
            view.input = p.state;
            pads.push(view);
        }
        // In player order, whoever holds the slot: games number them that way. The sort is
        // stable, so controllers without a number keep the order they came in.
        pads.sort_by_key(|p| match &p.role {
            Role::Virtual { player: Some(i) } | Role::Waiting { player: Some(i), .. } | Role::Native { player: Some(i) } => u32::from(*i),
            _ => 100,
        });
        // DirectInput's second view of a controller already listed is left out.
        let known: Vec<(&str, u16, u16)> = self.phys.values().map(|p| (p.model.name.as_str(), p.model.vendor, p.model.product)).collect();
        let mut unmapped: Vec<(&DeviceId, &Unmapped)> =
            self.unmapped.iter().filter(|(_, u)| !device::is_twin(&u.name, u.vendor, u.product, &known)).collect();
        unmapped.sort_by_key(|&(&id, _)| id);
        for (&id, u) in unmapped {
            let mut view = self.view(PadKey::Device(id), None, false);
            view.name = u.name.clone();
            view.vendor = u.vendor;
            view.product = u.product;
            pads.push(view);
        }
        let vigem = match (&self.bus, &self.bus_error) {
            _ if !VIRTUAL_PADS => Driver::Unsupported,
            (Some(_), _) => Driver::Ready { version: self.bus_version.clone() },
            (None, Some(e)) => e.clone(),
            (None, None) => Driver::Missing,
        };
        let viiper = match &self.viiper_error {
            _ if !DRIVER_CHOICE || self.driver != VirtualDriver::Viiper => Driver::Unsupported,
            Some(e) if e.contains("not installed") => Driver::Missing,
            Some(e) => Driver::Failed(e.clone()),
            None if self.bus.is_some() && self.bus_driver == VirtualDriver::Viiper => Driver::Ready { version: self.bus_version.clone() },
            None => Driver::Missing,
        };
        Snapshot {
            pads,
            vigem,
            hidhide: self.hider.state().clone(),
            hiding: self.hiding,
            sdl_version: crate::sdl::version(),
            sdl_error: None,
            running: true,
            foreground: self.foreground.clone(),
            bus: self.bus_driver,
            viiper,
        }
    }

    /// A view of a controller with what its model says filled in; the caller sets the rest.
    fn view(&self, key: PadKey, model: Option<&Model>, assignable: bool) -> PadView {
        let store = model.filter(|_| assignable).map(|m| m.store.clone());
        PadView {
            key,
            name: model.map(|m| m.name.clone()).unwrap_or_default(),
            brand: model.map(|m| m.brand).unwrap_or(Brand::Other),
            links: vec![Link::Unknown],
            power: Power::Unknown,
            role: Role::Unmapped,
            hidden: false,
            input: PadState::default(),
            can_power_off: false,
            vendor: model.map(|m| m.vendor).unwrap_or(0),
            product: model.map(|m| m.product).unwrap_or(0),
            family: model.map(|m| m.family).unwrap_or(Family::Other),
            art: model.map(|m| m.art).unwrap_or(Art::Offset),
            extras: model.filter(|_| assignable).map(|m| m.extras.clone()).unwrap_or_default(),
            features: model.map(|m| m.features).unwrap_or_default(),
            hint: model.and_then(|m| m.hint),
            profiles: store.as_ref().and_then(|p| self.profiles.get(p)).cloned().unwrap_or_default(),
            in_use: store.as_ref().and_then(|p| self.profiles.get(p)).map_or(0, |p| p.in_use(&self.foreground)),
            store,
        }
    }

    fn is_hidden(&self, id: DeviceId) -> bool {
        let Some(p) = self.phys.get(&id) else { return false };
        p.instance.as_deref().is_some_and(|i| self.hider.is_hidden(i))
    }

    fn shutdown(&mut self) {
        for p in self.phys.values().filter(|p| !(matches!(p.kind, Kind::Slot(_)) && native(&self.profiles, &p.model.store))) {
            p.pad.rumble(0, 0, 0);
        }
        for (_, _, c) in std::mem::take(&mut self.held) {
            self.keyboard.release(c);
        }
        self.close_bus();
        self.hider.stop();
        self.phys.clear();
    }
}

/// Whether the controllers kept under `store` are kept native.
fn native(profiles: &HashMap<String, Profiles>, store: &str) -> bool {
    profiles.get(store).is_some_and(|p| p.native)
}

/// What games see of a slot: the controller itself while it is kept native, otherwise its
/// virtual controller (`target`, with its player), kept for `remaining` while the controller is
/// away.
fn slot_role(native: bool, remaining: Option<Duration>, target: Option<Option<u8>>) -> Role {
    match (remaining, target) {
        _ if native => Role::KeptNative,
        (Some(remaining), player) => Role::Waiting { player: player.flatten(), remaining },
        (None, Some(player)) => Role::Virtual { player },
        (None, None) => Role::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Edit;

    #[test]
    fn only_a_store_kept_native_is_native() {
        let mut profiles = HashMap::new();
        assert!(!native(&profiles, "model:054c:0ce6"), "nothing saved is an Xbox controller");
        let mut p = Profiles::default();
        p.apply(Edit::Native(true));
        profiles.insert("model:054c:0ce6".to_string(), p);
        assert!(native(&profiles, "model:054c:0ce6"));
        assert!(!native(&profiles, "serial:aa:bb"));
    }

    #[test]
    fn a_native_slot_shows_as_native_whatever_else() {
        let wait = Some(Duration::from_secs(3));
        assert_eq!(slot_role(true, None, None), Role::KeptNative);
        assert_eq!(slot_role(true, wait, Some(Some(1))), Role::KeptNative);
        assert_eq!(slot_role(false, None, Some(Some(1))), Role::Virtual { player: Some(1) });
        assert_eq!(slot_role(false, wait, Some(Some(1))), Role::Waiting { player: Some(1), remaining: Duration::from_secs(3) });
        assert_eq!(slot_role(false, wait, None), Role::Waiting { player: None, remaining: Duration::from_secs(3) });
        assert_eq!(slot_role(false, None, None), Role::Unavailable);
    }
}
