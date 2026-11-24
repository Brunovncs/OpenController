# Finding out what a controller sends

The steps that found the Flydigi Apex 2's bugs (0.9.7) without the controller. They work for any
controller whose maker ships a Windows app, and most of them for any controller at all.

## 1. What others already know

- **SDL**: `src/joystick/hidapi/SDL_hidapi_*.c` for its own drivers, `SDL_gamecontrollerdb.h` and
  the community `gamecontrollerdb.txt` for mappings. Read the exact version OpenController ships.
  Check which product ids the driver claims (`SDL_IsJoystick...`) and on which interface: a
  receiver with another id falls to the generic path.
- **Other projects**, by searching GitHub for the `vvvv:pppp` pair and the model name:
  ExtendInput (Nielk1), InputPlumber, padctl, RetroArch autoconfig, Linux drivers, small bridges
  people write for one controller. They often name bits SDL ignores.
- **The manual**: button names, LED meanings, how modes switch.

## 2. The maker's app

Many are .NET, which keeps names in the binary. `vendorapp.py` here does every step below.

```
python -m venv .venv && .venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python -I vendorapp.py unpack Setup.exe out
.venv\Scripts\python -I vendorapp.py resources out\pe\000000000_App.exe_NET.bin out\res
```

- **Getting the installer**: winget manifests (`microsoft/winget-pkgs`) list the maker's own
  download URLs, old versions included. Use the version the user has. Never run the installer:
  it puts drivers and virtual devices on the PC. If you did, `pnputil /enum-devices` and
  `/enum-drivers` show what it left after uninstalling.
- **Unpack**: `unpack` takes an Inno Setup installer apart and cuts out every program and every
  symbol file (`.pdb`). Makers ship PDBs by mistake more often than you would think, and those
  name every local variable: `apex2MinBY` next to `battery` told the battery scale.
- **Translations**: `resources` then `baml` on `language*.baml` gives every text of the app. The
  keys name the model (`connect_switch_mode_apex2`) and the texts explain modes, LEDs and pairing.
- **Where the controller is opened**: `strings` and search for `vid_`, `pid_`, `mi_`; `uses`
  with that string's offset names the method; `dis` shows it. That is how we learned the Apex 2's
  receiver carries the same report on interface 2.
- **What a byte means**: find the method that reads the report (`dis` on candidates, `callers`
  and `fields` to walk around). Look for the report id and the bytes it compares, masks and
  shifts, and for what the value is handed to (`MoveToPos` told us bytes 4 to 6 are mouse motion
  and their direction).

`callers`, `fields` and `uses` scan the code's bytes for the token and keep only hits that start
an instruction, so they are fast and exact.

## 3. A user's report

OpenController's "Report a problem" records the controller's raw reports: the first one (at rest)
and, for each byte, its range and which bits changed. Ask the user to press each button once while
it is open. Then every changed bit must be explained: by a known button, a stick, or motion. A bit
nobody explains is a button nobody reads.

## 4. Prove it, then fix it

- Write the controller into the test bench (`crates/open-controller-core/src/testbench`).
- When SDL misreads it, port SDL's handling into a test and replay the user's session
  (`crates/open-controller-core/src/flydigi/sim.rs`). Matching the user's report exactly proves
  the cause; the same session then tests the fix.
- Whatever is still a guess (a scale, a direction) goes into the CHANGELOG as untested and into
  the reply to the user as a question.
- Break the fix on purpose, one piece at a time, and run the tests: each piece must make at least
  one test fail. A guard no test misses is a guard nobody checked.
- Before blaming our code, compare SDL's release tags (`gh api
  repos/libsdl-org/SDL/compare/release-A...release-B`): the fix may already be upstream.

## 5. With the controller in hand

Three tools in `crates/open-controller-core/examples`, built with `cargo build -p
open-controller-core --example <name>`:

- `hidprobe list` names every HID interface with its usage page. `hidprobe watch <seconds>
  <vendor>...` logs, per interface and per second, how many reports came, how many repeated the
  last one byte for byte, the longest gap, read errors with Windows' reason, and reopens what
  went away. It opens only what exists when it starts: start it again after a device turns up.
- `xinputwatch <seconds>` logs what games read through XInput, slot by slot: every change, and
  slots appearing or going.
- `padcheck <seconds>` opens every gamepad through SDL and prints the state SDL gives right at
  open, whether gyro and rumble work, and gyro readings over time, for each connection.

Setting up:

- OpenController hides controllers from every other program while it runs (HidHide). Quit it
  (`open-controller.exe --quit`) and the tools see them; it reveals them on the way out. Reopen
  it with `--minimized` when done.
- To run a local build, quit the installed one first and start `target\debug\open-controller.exe`.
  To try a setting without touching the user's own, copy `settings.json` into a scratch folder,
  change it there and pass `--data-dir <folder>`.
- The driver tests (`cargo test -p open-controller-core -- --ignored --test-threads=1
  hidhide_round_trip hiding_thread_round_trip plug_on_the_bus`) hide, reveal and plug on the
  real drivers and undo it. Run them with OpenController closed.

What to try, and what it taught us (DualShock 4 and 8BitDo Ultimate 2, November 2026):

- **Idle.** `hidprobe watch 15`. Wireless pads report all the time when still (8BitDo over
  Bluetooth about 120 a second, DualShock 4 about 32), but a counter or the motion sensor makes
  every report differ: "same report twice" never happens on them.
- **Switched off while holding a stick or a button.** No pad sends a "released" report: it just
  stops. The 8BitDo over Bluetooth went silent for about 1.5 s, then reads failed. The DualShock 4
  went silent with no error at all, and the same handle worked again when it came back. SDL lets
  go of a PlayStation pad after 500 ms; for others the state holds until the device is removed.
- **Its dongle.** The 8BitDo's dongle in XInput mode is a different device (2dc8:310B, with
  keyboard and mouse collections that answer "access denied", which is normal) and, with no pad,
  another one again (2dc8:6013). Switched off, it kept giving the last state for about 7 s.
- **Unplugged and plugged back,** and **switched off and on**: watch `xinputwatch` to check that
  the virtual pad returns to rest, keeps its slot, and starts centred.
- **Through OpenController.** Run `xinputwatch` (and `hidprobe watch` on 054c for a virtual
  DualShock 4) while OpenController runs and the pad is switched off: what games get is what
  counts. Press a button on another pad meanwhile: if its virtual pad keeps changing,
  OpenController did not hang.
- **Gyro and rumble across a reconnect.** `padcheck`, then switch the pad off and on. This is how
  SDL 3.4.18 was caught losing the 8BitDo's gyro and rumble after a Bluetooth reconnect, and
  starting its virtual pad off centre; 3.4.20 fixed both.

## Safety

Keep each download in its own new folder and run Python with `-I`, from outside that folder: an
interpreter loads modules from the folder it starts in.
