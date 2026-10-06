# Open Controller

<img src="assets/icon-256.png" width="96" align="right" alt="">

A Windows app that makes any game controller work in any game. Every controller connected to the
computer becomes an Xbox controller that games understand: a DualSense on Bluetooth, a Switch Pro
Controller on a cable and a DualShock 4 on Sony's receiver, all at once, each as its own player.
There is nothing to map. The buttons an Xbox controller lacks (back paddles, L4 and R4, Capture,
the touchpad click) can press an Xbox button, hold a key or type a macro instead.

It does what DS4Windows does for PlayStation controllers, for the several hundred controllers SDL
knows, and keeps out of the way: a resident process of 3.2 MB that adds about 0.6 ms between the
controller and the game. Xbox controllers are left alone, since games already read them. The
window is a separate program that only exists while it is open.

Open Controller is open source (MIT) and in development, at version 0.1.0. The engine is tested
end to end with a simulated controller on Windows 11, and on hardware with an 8BitDo Ultimate 2
Wireless; every other controller below is SDL's support, not yet a test of Open Controller with
it. Treat it as a beta, and please report what you plug in.

![Open Controller with eight controllers as tiles, each drawn with its live input: an 8BitDo Ultimate 2 on its receiver, a DualSense Edge, a Switch Pro Controller, Joy-Cons, a DualShock 3, an Xbox controller, a DualShock 4 reconnecting and an 8BitDo in XInput mode](docs/window.png)

## Installing

Download `open-controller-0.1.0-windows-x64.zip` from the
[latest release](https://github.com/Brunovncs/OpenController/releases/latest), extract it anywhere
and run `open-controller.exe`. The programs are not code-signed yet, so Windows SmartScreen warns
on the first start ("More info", then "Run anyway"). Neither the app nor its installation needs
administrator rights.

Open Controller needs two free drivers by Nefarius, the same ones DS4Windows uses:
[ViGEmBus](https://github.com/nefarius/ViGEmBus), which creates the virtual Xbox controllers games
see, and [HidHide](https://github.com/nefarius/HidHide), which hides the original controllers from
games so a game that also understands a DualSense does not see it twice. ViGEmBus is required and
HidHide recommended. Settings, Requirements in the window shows whether each is installed and in
which version, and installs or updates it when you click; the same page offers DsHidMini and
BthPS3, which a DualShock 3 needs. Nothing is installed on its own.

To build and install from source instead, with Windows 10 or 11 and [Rust](https://rustup.rs):

```powershell
.\install.ps1     # builds, installs to %LOCALAPPDATA%\Programs\open-controller, adds a Start menu entry and starts it
.\uninstall.ps1   # quits it, shows any hidden controller again and removes the program and its settings
```

Open Controller lives in the notification area: its icon opens the window and has "Start with
Windows" and "Quit". Closing the window does not stop anything.

## What it does

Every controller gets a player slot. Games see an Xbox 360 controller in that XInput slot, and the
physical controller shows the same number: the player lights on a DualSense or a Switch controller,
the light bar colour on a DualShock 4 (blue, red, green, pink, as on a PlayStation). Buttons follow
their position, so the bottom face button is A whether it says Cross, B or A. Sticks and triggers
pass through untouched, without a deadzone, as a real Xbox controller's do; the game applies its
own. Rumble goes back the other way, from the game to the controller's motors.

A slot belongs to the controller, not to the connection. A DualShock 4 or DualSense gives the same
Bluetooth address as its serial over Bluetooth and over USB, so plugging a cable into a DualSense
that was on Bluetooth moves it to the cable without the game noticing: the virtual controller
stays, same slot, same player. A controller that disappears keeps its slot for 15 seconds, which
covers a cable swap, a Bluetooth reconnect or a receiver hiccup; meanwhile the game sees it at
rest. A controller connected two ways at once (charging over USB while on Bluetooth) is one player,
driven by whichever connection sent the last input.

With HidHide installed, the originals are hidden from games while Open Controller runs and shown
again when it quits. Open Controller only touches its own entries in HidHide's configuration,
writes down what it hid before hiding it, and undoes it on the next start if it was killed in
between. Signing out or shutting down Windows quits it cleanly.

### The window

Each controller is a tile, drawn in its own shape (an Xbox-style pad, a PlayStation pad with its
touchpad, a symmetric pad like the DualShock 3 or 8BitDo Pro 2, a pad without sticks, a pair of
Joy-Cons or a handheld) with its sticks, triggers and buttons lit as they are pressed, and its
player, connection and battery. A controller's page has its profiles at the top and three
sections: Buttons, with the drawing and the extra buttons; Light, for controllers whose light bar
can be coloured; and Information, with the model, its USB id, what games see and what it can do.
"Identify" makes it rumble, and "Turn off" disconnects a Bluetooth PlayStation or Switch
controller.

Xbox controllers are listed but not duplicated: they are XInput controllers already, so they keep
their own slot and add no latency. Controllers SDL reads through XInput, which gives no name, get
the name Windows has for the device ("8BitDo Ultimate 2 Wireless Controller for PC" rather than
"XInput Controller"), and generic names are replaced from a table of 334 known models. The
interface follows Windows' light or dark mode and language, English or Brazilian Portuguese.

![A controller's page: the 8BitDo drawn with its live input, its profile, and its four extra buttons, one assigned to Xbox A, one to Ctrl+Shift+M and one to a macro](docs/controller.png)

### Extra buttons

Buttons an Xbox controller does not have never reach games through a virtual Xbox controller, so
they are free to do something else. Each one, named as on the controller (L4, PL, Fn, Capture,
Mic), can be:

- an **Xbox button**, pressed while you hold it, like the paddles of an Elite controller;
- a **key or shortcut**, held while you hold it: press it to record it, or pick one you cannot
  press, such as F13 to F24 or the media keys;
- a **macro**: keys typed once per press, recorded with the gaps you left between them, which can
  be edited afterwards;
- nothing, which is where every button starts.

Pressing an extra button on the controller picks it in the window. Changes apply as they are made
and follow the controller across cables and pairings when it reports a serial (Bluetooth
PlayStation, Switch and 8BitDo pads do); otherwise they belong to the model.

Each controller has up to eight profiles, each with its own assignments and light. A new profile
starts as a copy of the one in use, and switching applies at once, keys held by the old profile
included.

### Light bar

On a controller whose light bar SDL can colour (DualShock 4, DualSense, DualSense Edge and the
licensed pads that report one), the Light section sets what it shows: the player's colour, as a
PlayStation does, a colour of your choice, the battery (green when full to red when it needs
charging) or nothing, with four steps of brightness. The section only appears on controllers that
have such a light. The RGB lights of 8BitDo pads are set through 8BitDo's own protocol, which is
not public, and are left alone.

## Supported controllers

Controllers are read through [SDL 3](https://libsdl.org), which speaks each one's own protocol,
plus the 869 Windows entries of the community
[SDL_GameControllerDB](https://github.com/mdqinc/SDL_GameControllerDB) for generic ones.

| | USB | Bluetooth | Receiver | Extra buttons |
|---|---|---|---|---|
| DualShock 4 (both versions) | yes | yes | Sony USB wireless adaptor | touchpad click |
| DualSense, Access controller | yes | yes | | mic, touchpad click |
| DualSense Edge | yes | yes | | back buttons, Fn buttons, mic, touchpad click |
| DualShock 3, Sixaxis | with DsHidMini | with DsHidMini and BthPS3 | | none |
| PlayStation and PlayStation 2 pads | through USB adapters | | | none |
| Switch Pro Controller and licensed pads | yes | yes | | Capture |
| Joy-Con (pair or single) | | yes | | Capture, SL and SR |
| Nintendo Switch Online SNES, N64, Mega Drive | | yes | | Capture on the N64 |
| 8BitDo Ultimate 2 Wireless, Ultimate 3, Pro 3 | yes | yes | yes | L4, R4, PL, PR in D-input mode |
| 8BitDo Pro 2 | yes | yes | yes | PL and PR in D-input mode |
| 8BitDo Ultimate (first), Ultimate 2C | yes | yes | yes | back buttons and the star button; L4 and R4 on the 2C over Bluetooth |
| 8BitDo SN30 Pro, Lite, Zero, M30 and the retro range | yes | yes | yes | none |
| Steam Deck, Steam Controller | yes | yes | yes | L4, R4, L5, R5, quick access, trackpad clicks; grips on the 2015 controller |
| Flydigi Vader, Apex | yes | | yes | M1 to M4, C and Z or LM and RM |
| Google Stadia, Amazon Luna, NVIDIA Shield | yes | yes | | Capture and Assistant, mic, share and volume |
| Xbox 360, One, Series, Elite | yes | yes | Xbox Wireless Adapter | none: read by games directly, and Windows passes no paddles on |
| PowerA, Hori, Razer, Nacon and other licensed pads | yes | yes | yes | those SDL reports |
| Generic USB and Bluetooth gamepads | yes | yes | yes | if SDL or the database maps them |
| **Tested on hardware** | | | | **8BitDo Ultimate 2 Wireless on its receiver** |

Light bars and player lights show the player number. Windows has no driver for the DualShock 3:
with Nefarius' DsHidMini set to its SXS mode, Open Controller reads it over USB, and BthPS3 adds
Bluetooth once it has been paired by cable. Valve's controllers are taken by Steam while it runs,
and a Flydigi pad's extra buttons need "Allow third-party apps to take over mappings" in Flydigi
Space Station; the window says so on their pages. A device SDL has no button layout for is listed
as such and not passed to games.

Any pad in XInput mode hides its extra buttons from every program, so 8BitDo pads list them only
in D-input mode. On its receiver, the 8BitDo Ultimate 2 Wireless starts in XInput mode, where
Windows sees an Xbox controller and L4, R4, PL and PR send nothing any program can read. Turned on
while holding B, it is in D-input mode, on the receiver or over Bluetooth, where all four extra
buttons and the gyro work; put it on its dock right after, or it goes back to XInput mode the next
time it is turned on. A Pro 2 has a mode switch on its back instead, whose D position is D-input.
The window says which applies.

On hardware so far: an 8BitDo Ultimate 2 Wireless on its receiver in XInput mode is recognised,
named and left to games as the Xbox controller it is, while a virtual controller added next to it
gets the next player slot.

## How it works

```
crates/
  open-controller-core/   the engine (SDL input, ViGEmBus client, HidHide, slots, extra buttons),
                          the model table, profiles, driver setup, the pipe protocol and the texts
  open-controller/        open-controller.exe: the resident process, a Win32 message loop and the
                          notification-area icon
  open-controller-ui/     open-controller-ui.exe: the window, in GPUI, started on demand
```

The engine runs on one thread at multimedia "Games" priority, opted out of Windows 11's power
throttling, which would otherwise slow it down once no window of it is visible, as during a game.
Every millisecond it lets SDL read the controllers, reads the state of each one that sent a report,
and submits a report to the virtual controller if the state changed. Windows' high-resolution timer
wakes on 0.5 ms boundaries, so a plain 1 ms sleep loop runs every 1.5 ms; the engine sleeps to an
absolute deadline instead and runs at 1000 Hz. With nothing connected it polls 50 times a second.
Extra buttons are applied on the same pass; keys and macros are typed by a thread of their own.

ViGEmBus is driven through its IOCTL interface directly, without ViGEmClient.dll. Each virtual
controller belongs to the engine's handle on the bus, so if the process dies Windows unplugs them:
a crash never leaves phantom controllers. Its own virtual controllers come back through XInput as
new devices; they are recognised by their XInput slot, or by descending from ViGEmBus in the device
tree, and ignored. That slot is found where games look: ViGEmBus answers the question wrongly when
another XInput controller holds slot 0, so a new virtual controller shows a marker (four stick
positions inside every game's deadzone) and XInput is read until one slot shows it.

The window talks to the resident process over a named pipe private to the user and the session,
with JSON messages: snapshots one way (60 per second while it is open, so the input view is live),
requests the other. While no window is connected, snapshots carry no live input and are only taken
when something changes.

### Network

Open Controller sends nothing anywhere and checks for no updates. The only connection it makes is
the one you start in Settings, Requirements: clicking Install downloads that driver's installer
from its GitHub release with Windows' own `curl`, and the installer runs only if its SHA-256
matches the one recorded in this version.

[docs/design.md](docs/design.md) explains the decisions, including what was learned from DS4Windows
and PadForge.

## Measurements

| | Value | How it was measured |
|---|---|---|
| Controller state to what a game reads (XInput) | median 0.55 to 0.62 ms, p99 1.2 to 1.6 ms, max 1.5 to 2.1 ms | `loopback` example, 300 stick moves through the whole engine, several runs |
| First input after a controller connects | 34 to 62 ms, once; 1 ms if it comes half a second later | same; the delay is Windows finishing the new virtual controller |
| Report to ViGEmBus | median 16.8 µs, p99 33.5 µs | `latency` example, 1000 reports |
| Poll loop | median period 1.006 ms, 1000 per second (a `sleep(1 ms)` loop: 1.519 ms) | `latency` example, 5000 periods |
| Resident process, memory | 3.2 MB private, 13 threads | `Get-Process`, window closed, nothing connected |
| Resident process, CPU | 0.00 % of one core with nothing connected, 1.3 to 2.8 % with a controller connected | process CPU time over 10 s |
| Window process | 94 MB private and about 1 % of one core while open, nothing once closed | `Get-Process`, nothing connected |
| `open-controller.exe` / `open-controller-ui.exe` | 3.7 MB / 8.8 MB | `local` profile, GNU toolchain, no DLLs beyond Windows' own |
| Plugging in a virtual controller | 17 ms; over 1 s the first time on a machine, while Windows installs the Xbox 360 driver | `diagnose` example |

Measured on Windows 11 with ViGEmBus 1.22.0. A first version kept the GPUI window and the engine in
one process, which held 83 MB and 43 threads in the background; splitting them is why the resident
process is small.

## Building and testing

```powershell
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Release builds of GPUI precompile shaders with `fxc.exe` from the Windows SDK. Without the SDK,
`cargo build --profile local` builds the same optimized programs and compiles the shaders at
start-up. SDL is built from source and linked statically, which needs CMake and a C compiler.
Without the Visual Studio build tools, the GNU toolchain works for this folder
(`rustup override set stable-x86_64-pc-windows-gnu`) with a MinGW gcc on `PATH`.

The 60 tests cover the Xbox report mapping and the stick-noise filter, the slot roster (handoff,
grace period, two connections of one controller), device identity and connection type, the
ViGEmBus and HidHide request layouts and IOCTL codes, HidHide's list handling (other programs'
entries survive) and its journal file, the bundled mappings, the table of known models and the
extra buttons' names, families and drawings, the assignments (what an Xbox button adds to a
report, macro limits, keys pressed and let go in order, extended keys, recorded keys), profiles
(edits, limits, deleting the one in use) and light colours, the XInput slot markers, the pinned
driver installers and version checks, the settings file and a real pipe round trip. These talk to
the real drivers:

```powershell
cargo run -p open-controller-core --example diagnose            # what SDL sees; plugs one virtual controller and back
cargo run -p open-controller-core --release --example latency   # poll loop and ViGEmBus timings
cargo run -p open-controller-core --release --example loopback  # the whole engine, with a simulated controller
cargo test -p open-controller-core -- --ignored hidhide         # hides a made-up device, shows it, and checks crash recovery
cargo test -p open-controller-core -- --ignored download        # downloads a driver installer and checks it, without running it
```

`loopback` attaches an SDL virtual controller in the same process and checks, through XInput, that
it gets a slot and its player number, that stick, Y axis and A arrive, that a back paddle assigned
to Y presses Y and one assigned to a key (F24, which nothing uses) holds it while held, that
switching to another profile applies at once and lets go of the old one's keys, that rumble set by
a "game" reaches it, that disconnecting and reconnecting keeps the slot, and that stopping unplugs
it. CI runs the tests and starts both programs on a runner without the drivers.
`open-controller-ui --demo` fills the window with example controllers, for working on it without
hardware; changes made there stay in it.

The icon is drawn in `assets/*.svg`; `python scripts/render_icon.py` rasterises it with headless
Edge into the `.ico` files.

## Limitations

- Windows only. The engine's input side is SDL and would port; ViGEmBus and HidHide are Windows
  drivers, and Linux would need uinput instead.
- Tested on hardware with one controller so far; every claim above about another controller is
  SDL's support, not a test of Open Controller.
- ViGEmBus is retired upstream. It is stable and still what DS4Windows uses, but it gets no new
  versions.
- XInput games see at most four controllers. A fifth one gets a virtual controller that only
  DirectInput, Windows.Gaming.Input and GameInput games can see; the window says so.
- Everything is an Xbox 360 controller. Games that show PlayStation button prompts for a DualSense
  will show Xbox ones, and the DualSense's adaptive triggers, touchpad and motion sensors are not
  passed on.
- Hiding needs HidHide, and a game that opened a controller before Open Controller hid it keeps it
  until the game restarts. If Steam Input is on for PlayStation or Switch controllers, Steam reads
  them as well; turn it off for those controllers in Steam's settings.
- When a new controller connects, the engine pauses about 17 ms to plug in its virtual controller.
- Keys and macros from extra buttons do not reach a game running as administrator: Windows keeps
  a normal program's input out of elevated ones. Games with anti-cheat may also ignore them.
- Extra buttons are only what SDL reports. Pads in XInput mode and Xbox Elite paddles send nothing
  a program can read.
- 8BitDo pads' RGB lights, on-board profiles and firmware settings are left to 8BitDo's app.
- Switch 2 controllers and the official Wii U GameCube adapter need libusb, which this build of SDL
  leaves out.
- The programs are not code-signed, so SmartScreen warns the first time they run.

## Disclaimer

Open Controller is not affiliated with Sony, Microsoft, Nintendo, Valve, 8BitDo or any other
controller maker; their names identify compatible hardware only. ViGEmBus, HidHide, DsHidMini and
BthPS3 are by Nefarius Software Solutions and are downloaded from their releases, not bundled. No
code from DS4Windows or PadForge is included. Use it at your own risk.

## License

MIT, see [LICENSE](LICENSE). The release includes SDL and other third-party code under their own
licenses, listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
