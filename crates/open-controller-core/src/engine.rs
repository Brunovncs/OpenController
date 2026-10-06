//! The input thread. It owns SDL, the virtual controllers and HidHide, and runs one loop:
//! read what the controllers sent, forward it to the virtual controllers, pass rumble back,
//! sleep about a millisecond. The UI only sends commands and reads snapshots.

use crate::binding::{self, Action, Bindings, Chord};
use crate::bluetooth;
use crate::device::{self, Brand, Identity, Link, Power};
use crate::devnode;
use crate::extras::{self, Art, Family, Features, Hint};
use crate::hidhide::Cloak;
use crate::keyboard::Keyboard;
use crate::mapping::{self, PadState, XusbReport};
use crate::models;
use crate::profile::{self, Profiles};
use crate::roster::{Attached, DeviceId, Roster, SlotId};
use crate::rt;
use crate::sdl::{Event, Gamepad, Sdl};
use crate::vigem::{Bus, Feedback};
use crate::win::Overlapped;
use crate::xinput;
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
/// How long to look for a new virtual controller's XInput slot. It takes a few milliseconds
/// normally, and seconds the first time, while Windows installs the Xbox 360 driver.
const SLOT_WAIT: Duration = Duration::from_secs(5);
/// A controller that found no slot (a fifth one) looks again when a slot is free, this briefly,
/// at growing intervals up to `SEARCH_BACKOFF_MAX`.
const RESEARCH: Duration = Duration::from_millis(100);
const SEARCH_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// SDL stops a rumble after at most 65.5 s; a steady one is renewed well before.
const RUMBLE_MS: u32 = 20_000;
const RUMBLE_RENEW: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_secs(5);
const VIEW_PERIOD: Duration = Duration::from_millis(16);
/// The bits of the buttons with no Xbox equivalent in `PadState::buttons`.
const EXTRA_BUTTONS: u32 = {
    let mut m = 0;
    let mut i = 0;
    while i < extras::ALL.len() {
        m |= 1 << extras::ALL[i];
        i += 1;
    }
    m
};

pub struct Config {
    /// Where the HidHide journal is kept.
    pub data_dir: PathBuf,
    /// Hide the physical controllers that are turned into virtual ones from games.
    pub hide: bool,
    /// Each controller's profiles, by where its settings are kept (see [`binding::store_key`]).
    pub profiles: HashMap<String, Profiles>,
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
}

impl PadView {
    /// What its extra buttons do in the profile in use.
    pub fn bindings(&self) -> &Bindings {
        &self.profiles.active().bindings
    }
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
    /// Device instance id, for HidHide.
    instance: Option<String>,
    kind: Kind,
    state: PadState,
    power: Power,
    /// The colour last given its light bar.
    led: Option<[u8; 3]>,
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
    /// What each slot's controller is, still shown while it is away.
    memo: HashMap<SlotId, Model>,
    /// USB product names by vendor and product id, for XInput controllers SDL cannot name.
    names: HashMap<(u16, u16), Option<String>>,
    /// Devices turned off from here: when one leaves and its slot is empty, the virtual
    /// controller is unplugged at once, without the grace period.
    turning_off: Vec<DeviceId>,
    profiles: HashMap<String, Profiles>,
    keyboard: Keyboard,
    /// Keys held down for extra buttons that are held down, by slot, to be let go with them.
    held: Vec<(SlotId, u8, Chord)>,
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
        names: HashMap::new(),
        turning_off: Vec::new(),
        profiles: config.profiles,
        keyboard: Keyboard::start(),
        held: Vec::new(),
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
            Command::SetProfiles(store, profiles) => {
                let devices: Vec<DeviceId> = self.phys.iter().filter(|(_, p)| p.model.store == store).map(|(&id, _)| id).collect();
                let slots: Vec<SlotId> = devices.iter().filter_map(|&d| self.roster.slot_of(d)).collect();
                for &s in &slots {
                    self.release_keys(s);
                }
                let profiles = profiles.sanitised();
                if profiles.is_default() {
                    self.profiles.remove(&store);
                } else {
                    self.profiles.insert(store, profiles);
                }
                for s in slots {
                    self.resend(s);
                }
            }
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
        let family = extras::family(vendor, product, pad.pad_type());
        let model = Model {
            name,
            brand: device::brand(vendor, pad.pad_type()),
            vendor,
            product,
            family,
            art: extras::art(family, vendor, product),
            extras: extras::ALL.iter().copied().filter(|&b| pad.has_button(b as u32)).collect(),
            features: pad.features(),
            hint: extras::hint(vendor, product, family),
            store: binding::store_key(&identity, vendor, product),
        };
        let mut p = Phys {
            model,
            link,
            identity,
            instance: devnode::instance_id(&path),
            kind: Kind::Pending,
            state: PadState::default(),
            power: pad.power(),
            led: None,
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
            self.memo.insert(s, p.model.clone());
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
        if self.targets.values().any(|t| t.player.is_none() && t.search.is_some()) {
            return None;
        }
        Some(Kind::Native)
    }

    fn removed(&mut self, id: DeviceId, now: Instant) {
        if let Some(s) = self.roster.slot_of(id) {
            self.release_keys(s);
        }
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
        let mut r = mapping::to_xusb(&p.state);
        if let Some(b) = self.bindings_of(&p.model.store) {
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

    /// Brings the keys a slot holds in line with its source's extra buttons, and starts the
    /// macros of the buttons just pressed. Held keys follow the buttons' state rather than
    /// their release, which can come from another connection of the controller than the press.
    fn type_keys(&mut self, s: SlotId, dev: DeviceId, before: u32, after: u32) {
        let bindings = self.phys.get(&dev).and_then(|p| self.profiles.get(&p.model.store)).map(|p| &p.active().bindings);
        let mut i = 0;
        while i < self.held.len() {
            let (slot, button, chord) = self.held[i];
            let still = slot != s
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

    fn bindings_of(&self, store: &str) -> Option<&Bindings> {
        self.profiles.get(store).map(|p| &p.active().bindings)
    }

    /// Gives each light bar the colour its profile asks for: the player's, a chosen one, the
    /// battery's or none. Only a change is sent.
    fn update_lights(&mut self) {
        for p in self.phys.values_mut() {
            let Kind::Slot(s) = p.kind else { continue };
            if !p.model.features.light_bar {
                continue;
            }
            let player = self.targets.get(&s).and_then(|t| t.player);
            let battery = match p.power {
                Power::Battery(l) | Power::Charging(l) => l,
                _ => None,
            };
            let color = match self.profiles.get(&p.model.store) {
                Some(pr) => profile::light_color(pr.active().light, pr.active().brightness, player, battery),
                None => profile::light_color(profile::Light::Player, 100, player, battery),
            };
            if p.led != Some(color) {
                p.pad.set_led(color);
                p.led = Some(color);
            }
        }
    }

    /// Lets go of every key a slot holds down: its controller left, or its assignments changed.
    fn release_keys(&mut self, s: SlotId) {
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
                    self.resend(s);
                    if (before.buttons ^ state.buttons) & EXTRA_BUTTONS != 0 || !self.held.is_empty() {
                        self.type_keys(s, id, before.buttons, state.buttons);
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
        self.plug_missing(now);
        self.find_player_slots(now);
        self.update_lights();
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
                            search: Some((xinput::marker(serial), now + SLOT_WAIT)),
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

    /// Finds which XInput slot each new virtual controller got, by looking for its marker
    /// where games look, and lights that player number on the physical controller.
    fn find_player_slots(&mut self, now: Instant) {
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
                None if now >= t.next_search && *free.get_or_insert_with(xinput::has_free_slot) => {
                    t.search = Some((xinput::marker(t.serial), now + RESEARCH));
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
        pads.sort_by_key(|p| match &p.role {
            Role::Virtual { player: Some(i) } | Role::Waiting { player: Some(i), .. } => *i as u32,
            _ => 100,
        });
        let mut native: Vec<(&DeviceId, &Phys)> = self.phys.iter().filter(|(_, p)| p.kind == Kind::Native).collect();
        native.sort_by_key(|(_, p)| p.pad.player().unwrap_or(u8::MAX));
        for (&id, p) in native {
            let mut view = self.view(PadKey::Device(id), Some(&p.model), false);
            view.links = vec![p.link];
            view.power = p.power;
            view.role = Role::Native { player: device::xinput_slot(&p.pad.path()) };
            view.input = p.state;
            pads.push(view);
        }
        let mut unmapped: Vec<(&DeviceId, &String)> = self.unmapped.iter().collect();
        unmapped.sort();
        for (&id, name) in unmapped {
            let mut view = self.view(PadKey::Device(id), None, false);
            view.name = name.clone();
            pads.push(view);
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
            store,
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
        for (_, _, c) in std::mem::take(&mut self.held) {
            self.keyboard.release(c);
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
