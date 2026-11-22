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

## Safety

Keep each download in its own new folder and run Python with `-I`, from outside that folder: an
interpreter loads modules from the folder it starts in.
