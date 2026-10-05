# Design notes

Why Open Controller is built the way it is, and what was learned from the two programs closest to
it: DS4Windows (the maintained fork, schmaldeo/DS4Windows) and PadForge. Both were read as
references; no code was taken from either, and PadForge's license (CC BY-NC-SA) would not allow it.

## Reading controllers: SDL 3

DS4Windows parses each controller's HID reports itself: DualShock 4, DualSense, Switch Pro and
Joy-Con, with their Bluetooth CRCs, feature reports and receiver quirks. That is why it supports
those and nothing else. SDL 3 has drivers for those and for Xbox, Stadia, Steam, Luna, 8BitDo and
many more, plus a mapping format for generic pads, maintained by people who see every new
controller first. Open Controller uses it and adds the community mapping database on top.

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
kept. Its XInput slot is known microseconds later normally (DS4Windows sleeps 250 ms first); if
it is not known after 5 s, it is asked once a second from then on.

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

## Identity, slots and the handoff

A slot is keyed by the controller's identity, not its connection. SDL reports the Bluetooth
address as the serial for PlayStation controllers on every transport (DS4Windows reads it from
feature report 0x12 on USB, 0x09 on a DualSense). All-zero and all-F serials, which clones and
receivers report, are ignored; so is a serial shared by two devices on the same kind of link, which
cannot be one controller. Without a serial, the device path stands in.

DS4Windows assigns slots by arrival and can keep a virtual controller plugged with no physical one
("permanent" slots). PadForge debounces disconnects for 2 s and destroys a virtual controller after
60 s. Open Controller keeps a slot for 15 s after its controller leaves, sending a neutral report,
and gives it back to the same controller on any transport. A controller turned off from the window
skips the wait. When one controller is connected two ways, the slot follows the connection where
something happened: a button, or a stick moved further than noise, so an idle link's jitter does
not pull it back and forth.

## Hiding the originals: HidHide

DS4Windows relies on its users to configure HidHide by hand and falls back to "exclusive mode",
which disables and re-enables the device; its own user guide says that can leave a controller
disabled in Device Manager. Open Controller configures HidHide itself, carefully, because the
configuration is global and shared with other programs:

- Only its own entries are added or removed, by reading the list, changing it and writing it back.
  An entry that was already there is left to whoever put it there.
- HidHide's control device admits one handle at a time. Each change keeps one handle open from the
  read to the write, which makes it a lock: no other program can change the list in between. While
  another program holds the device, opening is retried briefly, and for up to 5 s when quitting.
- What it is about to hide is written to a journal before the driver is touched, through a
  temporary file and a rename so a crash never leaves half a journal; if the journal cannot be
  written, nothing is hidden. A run that is killed leaves the journal behind, and the next start
  (or the uninstaller's `--restore`) undoes it.
- A controller is hidden only once its virtual controller exists, and shown again if creating one
  fails: hiding it otherwise would take it away from games altogether.
- Every change is read back and checked.
- None of this requires administrator rights.
- Inverse mode, where the program list means the opposite, is reported and left alone.

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
