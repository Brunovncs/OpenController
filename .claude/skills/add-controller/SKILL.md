---
name: add-controller
description: How to add a controller to OpenController after someone reports or requests it (from the app's Report a problem, the site's "Ask for it", an email or an issue). Covers checking whether it is already known, finding its USB ids and chip, deciding between a row of its own and an alias of the model it copies, tracing its drawing, and syncing the website. Use whenever a controller has to be added, renamed, given a drawing, or looked into because it is "not on the list".
---

# Adding a controller

A report usually arrives as a brand and model name, sometimes with ids. The work is always the
same: find out what the controller tells the computer it is, then make OpenController show it
under its own name and drawing. Read this whole file before touching code.

The app is this repository. The website is the sibling repository `open-controller-site`
(Next.js). They share two generated files: `data/models.json` (from `models.rs`) and
`data/pads.json` (a copy of `assets/pads.json`). A controller is not done until both are in sync.

## 1. Check it is not there already

Search before researching. Names vary between stores, so try the brand alone, the model number,
regional names (the Redragon G820 is also "Darkflame"), and the ids if you have them.

- `crates/open-controller-core/src/models.rs`: `MODELS` (name and ids), `ALIASES`, `DRAWINGS`, `LAYOUTS`.
- `scripts/models/sources.json`: the `models` list of each row names every product known to use
  that id, and `left_out` lists ids found and rejected on purpose, with the reason. A controller
  in `left_out` needs a new fact to come back, not a second guess.
- `assets/pads.json` keys and the table in `scripts/pads/README.md`, for an existing drawing.

If it is there, say so and stop, or fix what the report says is wrong.

## 2. Collect what the report gives

A report from the app ("Report a problem") has the vendor and product ids, the release number,
SDL's GUID and joystick name, its mapping, the connection and the device path. That is solid
evidence: a controller's ids come from the controller itself.

A request from the website may carry the ids the browser read in the controller test, or only
the name the person typed. If there are no ids, they must come from research (step 3). Never
invent an id or assume one from the brand. If research cannot settle the ids, stop and tell the
maintainer what to ask the reporter for: the app's Report a problem, once in each connection mode.

## 3. Research the ids and the chip

Look for every mode the controller has. Most pads have several (Switch, PC/XInput, D-input,
Android, iOS), picked by a button held at power-on or a switch, and each mode can show different
ids. The manual says how modes are chosen; the ids come from elsewhere. Sources, roughly by
weight:

1. SDL_GameControllerDB (`gamecontrollerdb.txt`): the GUID holds the ids, `03000000VVVV0000PPPP...`
   with both ids little-endian. Also SDL's own `controller_list.h` and `usb_ids.h`.
2. Linux `drivers/input/joystick/xpad.c` and `drivers/hid/hid-ids.h`.
3. RetroArch's `retroarch-joypad-autoconfig`, MiSTer's input latency catalogue.
4. FCC ID filings (fccid.io, the FCC's own search): the grantee names the real maker, which is
   often an OEM shared by several brands, and the internal photos show the chip.
5. usb.ids, devicehunt.com, linux-hardware.org probes.
6. Users' own logs: GitHub issues on SDL, Steam Input, DS4Windows, Ryujinx and the like, Reddit
   and forums. Weigh these by how precise they are: a pasted `lsusb` or GUID beats "it shows up as".

Note the chip when you find it, and other products built on the same chip or by the same OEM:
they usually share ids and firmware, and one finding then covers several controllers.

Rate each id `confirmed` (two independent sources, or the reporter's own app report), `likely`
(one good source) or a guess. Only confirmed and likely ids go in the table. A guess goes in the
reply to the maintainer, not in code.

## 4. Decide which case it is

**It copies another controller's ids.** Very common with cheaper pads, which use the same chip
as a known controller and answer its protocol: Xbox 360 clones are `045e:028e`, PlayStation 4
clones `054c:05c4` or `054c:09cc`, Switch Pro clones `057e:2009`, basic PC pads `0079:0006` or
`0810:0001`. Nothing SDL reads tells the copy from the original, and it already works as that
controller. Add it to `ALIASES` in `models.rs`:

    (vendor, product, "brand-model", "Brand", "Brand Model", "drawingKey")

The key is what the owner's settings keep once they pick it under Model on the Information page,
so it is permanent: lowercase, hyphenated, never renamed afterwards. Extend the
`aliases_are_of_known_models` test with the new entry. The site lists it under its own name with
"It reports itself as..." automatically, through `models.json`.

**It has ids of its own that are not in the table.** Add a row to the research block of `MODELS`
(between "From OpenController's own research" and "End of the research rows"), sorted by id, and
a matching row in `scripts/models/sources.json` with `models`, `reported`, `modes`, `confidence`
and `sources`. The `Family` decides the extra buttons' names, gyro, light bar and advice, so pick
it by the protocol the controller speaks, not by its looks: a pad that speaks Switch Pro is
`SwitchPro` whatever its shape. Use `Family::Other` when no family fits. A pad in `Other` whose
shape is not Xbox-like (sticks side by side, or no sticks) needs a `LAYOUTS` entry.

**Both.** One controller can copy ids in one mode and have its own in another. Do both, and say
in `sources.json` which mode shows which ids.

**An id shared by unrelated devices** (a chip maker's default id used by many different
products): do not put one product's name on it. Leave it out with the reason, or alias it only if
the product is clearly one of a family of look-alikes.

Unless someone tested the controller itself with OpenController, it is not `VERIFIED` in
`rating.rs`. The grade follows from the family and hints on its own.

## 5. Draw it

Every controller added gets a drawing of its own, unless an existing look is truly the same
shell (then reuse that key in `DRAWINGS` or in the alias). `scripts/pads/README.md` explains the
tools; in short:

1. Find a straight front view. In order of preference: the vector drawing in the maker's manual
   (`pdf:` source, cleanest), the maker's own render on its site or store, a retailer's photo,
   Wikimedia Commons with its license noted. Angled shots and lifestyle photos do not work.
2. Keep downloaded pictures outside the repository (the session's scratch folder). Nothing of the
   source picture is committed, only the shapes traced from it.
3. `python source.py SPEC out/prefix` renders it with a coordinate grid for picking seeds.
4. Write `looks/<key>.json`, with the key in camelCase brand plus model (`onikumaC1`,
   `gamesirG7Pro`). Start from the look of the most similar controller; `onikumaC1.json` is a
   photo traced with `ink_mode: "photo"`, `flydigiVader.json` a manual drawing.
5. `python trace.py looks/<key>.json out/<key>.json out/<key>-overlay.png` and look at the
   overlay. Every button, stick, bumper and the d-pad must sit where the picture has them. Fix
   seeds and trace again until they do.
6. `python build_pads.py --add ../../assets/pads.json <key>=out/<key>.json` adds it and keeps the
   rest. `python preview.py ../../assets/pads.json out/preview.png <key>` draws it as the app does.
7. Add a row to the table in `scripts/pads/README.md` saying exactly where the source came from.
8. Point the ids at it in `DRAWINGS`, or put the key in the alias.

Check the face button letters too. A Switch-style pad has B and A on the bottom and right, and the
look's `glyphs` decide what the window and the site print on them.

## 6. Sync the website and the notes

From the app folder, in Git Bash (its rustup override only applies there; from elsewhere cargo
picks the MSVC toolchain and fails. PowerShell's `>` writes CRLF):

    cargo run -p open-controller-core --example models_json > ../open-controller-site/data/models.json
    cp assets/pads.json ../open-controller-site/data/pads.json

Both files use LF line endings: check the diff only shows the new controller. Then in the site:

- `lib/models.ts` `BRANDS`: add the vendor id if it is a new maker (aliases carry their brand).
- `lib/families.ts`: only if a family was added. A new family also needs a page in `lib/guides.ts`.
- `lib/faq.ts`: the "another one's name" answer cites an alias as its example; keep it accurate.

Add a line for players to `## Unreleased` in `CHANGELOG.md`: which controller, and for an alias,
that its owner picks it under Model. Public text is short, plain, without em dashes or a list of
bullets.

## 7. Verify

- App: `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --all`.
- Look at the drawing in the window: `target\debug\open-controller-ui.exe --demo` (capture with
  `PrintWindow` like `scripts/screenshot.ps1`, not a screen grab).
- Site: `npx tsc --noEmit`, `npm run lint`, `npm run build`, then `npx next start` and search the
  controller in the list. Do not run `next dev`, it rewrites `AGENTS.md`.

Never commit or push unless the maintainer asks for it in that same message.

## 8. Report back

End with a short account for the maintainer: the ids and how sure each one is, the case chosen
and why, the chip or OEM if found, where the drawing came from, the sources, and what is still
uncertain with what would settle it. If the reporter left an email, suggest the one-line answer
to send them.
