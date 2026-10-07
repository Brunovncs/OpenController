# Design notes

Why OpenController is built the way it is, and what was learned from the two programs closest to
it: DS4Windows (the maintained fork, schmaldeo/DS4Windows) and PadForge. Both were read as
references; no code was taken from either, and PadForge's license (CC BY-NC-SA) would not allow it.

## Reading controllers: SDL 3

DS4Windows parses each controller's HID reports itself: DualShock 4, DualSense, Switch Pro and
Joy-Con, with their Bluetooth CRCs, feature reports and receiver quirks. That is why it supports
those and nothing else. SDL 3 has drivers for those and for Xbox, Stadia, Steam, Luna, 8BitDo and
many more, plus a mapping format for generic pads, maintained by people who see every new
controller first. OpenController uses it and adds the community mapping database on top.

SDL is built from source and linked statically, with only the joystick, HIDAPI, haptic, sensor
and power subsystems: no video, audio or GPU code. Three things about it matter:

- Hot-plug reaches only the thread that initialised SDL. PadForge splits SDL between its UI thread
  and its poll thread and logs stalls of about 100 ms when they contend for SDL's joystick lock.
  Here one thread owns SDL entirely: initialisation, events and state reads.
- There is no window, so the background-events hint is on; without it SDL would ignore input
  whenever another program has focus, which is always.
- Each report produces dozens of per-axis and per-button events. The engine reads the whole state
  once per report (on `GAMEPAD_UPDATE_COMPLETE`) and turns the other events off; SDL's internal
  state does not depend on them.

## Presenting them to games: ViGEmBus, as Xbox 360 controllers

Every Windows game that supports controllers supports XInput, so every controller becomes an Xbox
360 controller on ViGEmBus. ViGEmBus is retired upstream but stable, the driver DS4Windows uses,
and needs no signing work from this project. PadForge moved to its own
user-mode HID driver; writing and signing a driver is out of scope here.

The client speaks ViGEmBus's IOCTLs directly. Plugging a target waits for "device ready", which
the driver gives up on after 1 s; the first virtual controller on a machine can take longer while
Windows installs the Xbox 360 driver, so that timeout is treated as success and the controller is
kept. That wait happens on a thread of its own (`plug.rs`): the input thread asks for a controller
and takes it once it is ready, so the other players' input never stops while someone connects. A
controller that leaves before its virtual one is ready gets it unplugged again. Right after plugging, the bus refuses a report or two (error 259) until the Xbox 360 driver
has opened the controller, so a refused report is sent again on the next pass instead of being
dropped: otherwise a button let go at that moment would stay pressed in the game.

ViGEmBus can be asked which XInput slot a controller got, and DS4Windows does, but the answer is
wrong as soon as another XInput controller exists: with an 8BitDo pad in slot 0, a new virtual
controller in slot 1 is reported as 0. The slot is found where games look instead. The new
controller shows a marker, four exact stick positions well inside every game's deadzone and
different for each bus serial, and XInput's four slots are read until one shows it, a few
milliseconds after plugging. A controller that finds no slot within 5 s is a fifth one; it looks
again, briefly, only when a slot is free.

Rumble requests come back as a pending IOCTL per controller, on its own thread, and are forwarded
to the physical controller from the input thread. SDL stops a rumble after 65.5 s at most, while
an XInput rumble lasts until changed, so a steady rumble is renewed every 10 s. The Xbox 360 "LED
number" in the same notification is ignored: PadForge found that writing the ring LED moves a
controller to another XInput slot in other processes. The player number shown on the physical
controller comes from the XInput slot instead.

## Not reading itself back

The virtual Xbox controllers appear to SDL like any other XInput controller. Each one is
recognised by the XInput slot its virtual controller holds (SDL's XInput paths are `XInput#<slot>`)
or, for paths that name a device, by walking the device tree up to ViGEmBus, as DS4Windows does.
An XInput device that appears while a virtual controller's slot is not known yet waits until it
is, rather than being guessed.

## Names

SDL names the controllers it reads through XInput "XInput Controller", since XInput reports no
name. Windows still knows the USB product string of the device behind it, which XInput's vendor
and product ids lead to: an 8BitDo receiver becomes "8BitDo Ultimate 2 Wireless Controller for PC".
The lookup takes about a millisecond and is cached per model.

DirectInput lists every XInput pad a second time, as "Controller (<name>)". SDL drops the leading
word and normally skips these, but when a receiver connects it can ask before Windows has marked
the device as an XInput one, and the copies come through as joysticks without a gamepad layout.
A joystick without a layout whose name or USB ids match a controller that is being read is left
out of the list (`device::is_twin`). XInput itself has no charging state: SDL turns its "wired"
battery type into charging at full, and third-party receivers report that type for themselves, so
on XInput it is read as "powered, battery unknown" (`device::power`).

## Extra buttons and what they do

SDL reports buttons beyond the Xbox set as paddles, misc buttons and the touchpad click, and whether
a controller has each one. What they are called depends on the controller (`extras.rs`): L4, R4, PL
and PR on an 8BitDo Ultimate 2, the back buttons and Fn buttons on a DualSense Edge, Capture on a
Switch controller, the mic button on a DualSense. Games never see them through a virtual Xbox
controller, so they are what the user assigns: an Xbox button (held with them), a key or shortcut
(held with them), or a macro of keys and waits (typed once per press, 32 steps at most).

Only extra buttons can be assigned. Remapping the standard buttons is what games' own settings and
Steam Input are for, and a mapper is what this project set out not to be.

Assignments belong to a controller's serial when it has one, so they follow it across cables and
pairings, and to its model otherwise. Keys go out through `SendInput` from a thread of their own,
so a macro's waits never touch the input thread, with scan codes for games that read Raw Input.
Held keys follow the buttons' state, not their release events: a controller connected twice can
press on one connection and release on the other, and an edge-driven key would stay down. They are
let go when the controller leaves, when its assignments change and when the engine stops.

Some controllers hide their extra buttons from every program in some modes. An 8BitDo Ultimate 2
on its receiver in XInput mode (2DC8:310B) sends nothing for L4, R4, PL and PR; turned on while
holding B, it switches to D-input mode (2DC8:6012), where SDL reads all four and the gyro. The
window says so when it sees the XInput one. An Xbox Elite controller's paddles never reach programs
on Windows except as copies of other buttons.

## Known models

`models.rs` names 602 controllers by USB id and puts each in a family, taken from SDL 3.4's own
lists (`controller_list.h`, `usb_ids.h`, its HIDAPI drivers and built-in Windows mappings) plus the
8BitDo range, and the ids Linux's xpad, hid-sony and hid-nintendo drivers know (facts only, no
code). The family decides what the extra buttons are called, how the controller is drawn
and what the window tells the user about it (switch an 8BitDo to D-input, close Steam, allow
third-party apps in Flydigi's app). Which extra buttons a controller really has is always asked of
SDL when it connects; the table only names them. Where SDL's driver reports buttons its mapping
leaves out (the 8BitDo Ultimate 3's L4, R4, PL, PR and Share), the missing mapping fields are
added to the open gamepad. SDL is told to use its PS3 driver through a "Sixaxis" driver, which is
what DsHidMini's SXS mode presents, since Windows itself cannot start a DualShock 3.

## Profiles and light

A controller's settings are a list of profiles, one in use, each with its assignments, what its
light bar shows and how bright. The window sends edits (assign a button, add, rename, select,
delete a profile, change the light), the resident process applies them to the settings file and
hands the engine the whole list, and the engine uses the profile in use. A light bar is given its
colour only when the colour changes: the player's (SDL's own palette), a chosen one, the battery's
or none. Once OpenController sets a colour, SDL no longer changes it with the player number, so the
player's colour is set the same way.

## Playing together

Swapping two players does not replug anything: the two slots trade virtual controllers, so each
game keeps seeing the same four XInput controllers, and the rumble a game asked of one goes to
its new hands. A profile can name programs; the resident process asks Windows to say when the
foreground window changes (`SetWinEventHook`, no polling), and the engine uses the first profile
that names it, or the chosen one. Only assignments, light, gyro and sticks follow; players never
change behind anyone's back. OpenController's own window and the Start menu do not count as a
program coming to the front, so opening the window mid-game keeps the game's profile.

## Gyro, sticks and touchpad

The gyro is read only while a profile uses it (SDL turns the sensor's reports on and off), at the
controller's own rate, on the input thread. Yaw and pitch, in radians per second, go through a
one-euro filter (a low-pass whose cutoff rises with speed: a still hand is still, a flick is not
delayed), a 0.03 rad/s deadzone and a 12 % anti-deadzone, so games with their own stick deadzone
still react to a slow turn, and are added to the right stick. The stick deadzone is radial, so a
diagonal is not cut short, and off by default. The touchpad's halves and two-finger touch are
worked out from SDL's finger positions on every report and carried as buttons past SDL's own
(26 to 28), which made `PadState::buttons` 64 bits wide; a handheld's own buttons take 29 to 40.

## Handhelds

A handheld PC is recognised by its firmware's manufacturer, product name and version (Lenovo puts
"Legion Go" in the version), not by its pad's USB id, since many present a generic Xbox 360 pad.
Buttons that arrive as function keys are caught with a low-level keyboard hook on a thread of its
own, only for the keys of the recognised machine; keys OpenController types itself pass through,
and when the firmware held Windows with the key, an unassigned key is sent between so that
letting go of Windows does not open the Start menu. Buttons that arrive as HID reports are read
with SDL's own hidapi, read-only. Nothing is written to any controller: the ROG Ally's M1 and M2
and the MSI Claw's mode would need configuration written to them, which vendor apps fight over.

## Drivers

The settings list ViGEmBus, HidHide, DsHidMini and BthPS3 with whether each works: the engine's own
view for the first two, Programs and Features for the rest. Installing one downloads the
installer of a pinned version from its GitHub release with Windows' own curl, refuses it unless its
SHA-256 is the one recorded in `drivers.rs`, and runs it through `ShellExecuteEx` with the `runas`
verb, so the user sees Windows' administrator prompt with Nefarius' signature. ViGEmBus and HidHide
install silently once allowed; an update runs the installer's own wizard, since HidHide's may ask to
restart halfway. Nothing is installed without a click: these are kernel drivers, two of them need a
restart, and HidHide changes which devices other programs see. ViGEmBus is retired upstream at
1.22.0, so its pin will not move.

## The icon

Two pads, an outline behind and a solid one in front: any controller in, one virtual controller
out. `assets/icon.svg` is drawn on a 1024 grid for 40 px and up; `icon-small.svg` redraws it on a
16 grid for 16 to 32 px, with the front pad larger, since the large drawing turns to mush there.
The notification area gets its own pair without the tile, white for a dark taskbar and near-black
for a light one (`tray-dark.svg`, `tray-light.svg`), and the resident process swaps them when the
taskbar changes theme, which Windows sets apart from the apps' own. `scripts/icon_small.py` writes
the small drawings and `scripts/render_icon.py` rasterises everything with headless Edge into the
`.ico` files.

## The window

A bar on top, the controllers as tiles, and a page per controller with a rail of sections, the
way peripheral apps lay it out, in Windows' own type (Segoe UI Variable) and icons (Segoe Fluent
Icons). Surfaces are opaque and step up from the page to cards to controls, with hairline edges
rather than shadows, 16 px corners on tiles, 12 on cards and 8 on controls, one accent taken from
the Windows accent palette (the default blue when the user's accent is a grey that would not read),
and colour for state only where something needs attention. The drawings are vector, one of 14
by family, each traced from its maker's own front view (the line art in its manual, or a straight
product shot) so its outline and the place and size of every control are the real ones, filled with
a soft gradient and lit with the accent as the controller is used. The shapes live in
`assets/pads.json`, which the website draws from too; `scripts/pads` traces them. Photos themselves
are not shown: they are the makers'.
Pressing an extra button on the controller picks it in the window; recording a key is pressing it.
Changes apply as they are made, with no Save button.

## Identity, slots and the handoff

A slot is keyed by the controller's identity, not its connection. SDL reports the Bluetooth
address as the serial for PlayStation controllers on every transport (DS4Windows reads it from
feature report 0x12 on USB, 0x09 on a DualSense). All-zero and all-F serials, which clones and
receivers report, are ignored; so is a serial shared by two devices on the same kind of link, which
cannot be one controller. Without a serial, the device path stands in.

DS4Windows assigns slots by arrival and can keep a virtual controller plugged with no physical one
("permanent" slots). PadForge debounces disconnects for 2 s and destroys a virtual controller after
60 s. OpenController keeps a slot for 15 s after its controller leaves, sending a neutral report,
and gives it back to the same controller on any transport. A controller turned off from the window
skips the wait. When one controller is connected two ways, the slot follows the connection where
something happened: a button, or a stick moved further than noise, so an idle link's jitter does
not pull it back and forth.

## Hiding the originals: HidHide

DS4Windows relies on its users to configure HidHide by hand and falls back to "exclusive mode",
which disables and re-enables the device; its own user guide says that can leave a controller
disabled in Device Manager. OpenController configures HidHide itself, carefully, because the
configuration is global and shared with other programs:

- Only its own entries are added or removed, by reading the list, changing it and writing it back.
  An entry that was already there is left to whoever put it there.
- HidHide's control device admits one handle at a time. Each change keeps one handle open from the
  read to the write, which makes it a lock: no other program can change the list in between. While
  another program holds the device, opening is retried briefly, and for up to 5 s when quitting.
- All of it runs on a thread of its own (`hiding.rs`), which the input thread only sends requests
  to: a change can take seconds while another program holds the device, and the players must not
  notice. If HidHide cannot be opened at start (another program held it, as at sign-in), that
  thread tries again every 5 s, and the controllers are hidden once it can.
- What it is about to hide is written to a journal before the driver is touched, through a
  temporary file and a rename so a crash never leaves half a journal; if the journal cannot be
  written, nothing is hidden. A run that is killed leaves the journal behind, and the next start
  (or the uninstaller's `--restore`) undoes it.
- A controller is hidden only once its virtual controller exists, and shown again if creating one
  fails: hiding it otherwise would take it away from games altogether.
- Every change is read back and checked.
- None of this requires administrator rights.
- Inverse mode, where the program list means the opposite, is reported and left alone.

## Linux and macOS

The engine is the same program everywhere; what differs sits behind one set of names in
`platform.rs` (the bus that makes virtual controllers, hiding, what the device tree says), with
the keyboard, the pipe, single-instance and the resident process's main loop split the same way.
The settings file, profiles and key assignments are identical: keys are stored as Windows
virtual-key codes on every system and turned into each system's key positions when typed.

On Linux, uinput is what Steam Input, InputPlumber and xboxdrv use, and a uinput device shaped
exactly like `xpad`'s (ids 045e:028e, version 0x110, its button codes and axis ranges, Y flipped
with `~y` as `xpad` does) is matched by SDL's and Wine's built-in Xbox 360 mappings, so no game
needs to be told about it. Rumble needs answering the kernel's force-feedback upload and erase
requests on the device's own handle; each virtual controller has a thread for that, which keeps
the effects a game uploaded and passes on the one playing until it stops or its length runs out.
Hiding on Linux has no HidHide: an exclusive grab (`EVIOCGRAB`) on the controller's event nodes
takes them from every other reader, and OpenController keeps reading through `hidraw`, which SDL
prefers when it may open it. That is why the udev rule grants `hidraw` access for known
controllers, generated from the model table rather than by maker, so no keyboard's raw reports
become readable. What a grab cannot do is stop another program from opening the same `hidraw`
node; a root helper that moves device nodes away, as InputPlumber does, would, and is left out on
purpose. The uinput request numbers are computed from the structures' sizes and checked against
the kernel headers.

On macOS, creating a game controller needs `com.apple.developer.hid.virtual.device`, an
entitlement Apple grants case by case, and the Game Controller framework already reads the common
controllers natively. So every controller is left as it is (the engine treats them all as it
treats an Xbox controller on Windows), and what remains is what games do not do: keys and macros
for extra buttons through Quartz events, lights and battery. The resident process is a plain
background process there and on Linux, without an icon of its own: a status icon needs a GTK or
AppKit main loop, which would bring the window toolkit's weight back into the process kept small
for that reason.

## The poll loop

SDL reads controllers when it is polled, so the loop's period is latency. Windows' high-resolution
timer, which `std::thread::sleep` uses, wakes on 0.5 ms boundaries: asking for 1 ms sleeps 1.5 ms
(median 1.519 ms measured), so a naive "sleep 1 ms" loop runs at 660 Hz. Sleeping to an absolute
deadline minus 0.4 ms lands on the boundary before it, and the loop runs at 1000 Hz (median period
1.006 ms) without spinning. The thread joins the multimedia "Games" class and is opted out of
EcoQoS, Windows 11's throttling of background processes.

## One process or two

The first version ran the GPUI window and the engine in one process. GPUI keeps its GPU device,
font system and executor threads after the last window closes: 83 MB private and 43 threads while
sitting in the notification area. Like COMPX Tray, the resident process is now a plain Win32
message loop with the engine (3.4 MB, 11 threads) and the window a separate program started on
demand. They talk over a named pipe whose security descriptor admits only the user and SYSTEM, one
per Windows session. The resident process listens for the session-end broadcast on a hidden
top-level window (a message-only window would not receive it) so signing out still unplugs the
virtual controllers and shows the hidden ones.
