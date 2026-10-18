"""Writes THIRD_PARTY_NOTICES.md: every crate compiled into the two programs, with its version and
license, and the notices of the C code linked in (SDL and the controller mapping database).
Run it when dependencies change:

    python scripts/third_party_notices.py
"""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRAMS = ("open-controller", "open-controller-ui")
# Every target a release is built for.
TARGETS = (
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)

HEADER = """# Third-party notices

OpenController is MIT licensed (see [LICENSE](LICENSE)). Its programs include the code below,
each under its own license. ViGEmBus, HidHide, DsHidMini and BthPS3 are not included: Open
Controller downloads their official installers from Nefarius' GitHub releases when you ask it to.
Neither are usbip-win2 and VIIPER, which the experimental VIIPER option downloads the same way.

## SDL 3

[SDL](https://github.com/libsdl-org/SDL) is compiled into `open-controller.exe` and linked
statically. It is under the zlib license:

> Copyright (C) 1997-2026 Sam Lantinga <slouken@libsdl.org>
>
> This software is provided 'as-is', without any express or implied warranty. In no event will
> the authors be held liable for any damages arising from the use of this software.
>
> Permission is granted to anyone to use this software for any purpose, including commercial
> applications, and to alter it and redistribute it freely, subject to the following
> restrictions:
>
> 1. The origin of this software must not be misrepresented; you must not claim that you wrote
>    the original software. If you use this software in a product, an acknowledgment in the
>    product documentation would be appreciated but is not required.
> 2. Altered source versions must be plainly marked as such, and must not be misrepresented as
>    being the original software.
> 3. This notice may not be removed or altered from any source distribution.

## SDL_GameControllerDB

The Windows entries of [SDL_GameControllerDB](https://github.com/mdqinc/SDL_GameControllerDB)
are bundled in `open-controller.exe`, under the same zlib license as SDL.

## Controller drawings

The DualShock 4 and DualShock 3 drawings in `assets/pads.json` are traced from "Dualshock 4
Layout.svg" and "Dualshock3 Layout.svg" by Tokyoship on Wikimedia Commons, under
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/); the Joy-Con, from "Nintendo Switch
Joy-Con illustration.svg", in the public domain. `scripts/pads/README.md` lists every source.

## GamepadMotionHelpers

The way OpenController works out a DualShock 3's tilt from its accelerometer (`motion::TiltRate`)
follows the sensor fusion step of [GamepadMotionHelpers](https://github.com/JibbSmart/GamepadMotionHelpers),
under the MIT license:

> Copyright (c) 2020-2023 Julian "Jibb" Smart
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## VIIPER

[VIIPER](https://github.com/Alia5/VIIPER) is an experimental option on Windows, off by default.
Its server, `viiper.exe`, is not part of OpenController. When you choose VIIPER and click
Install, OpenController downloads the official v0.8.2 release from GitHub, checks it against
the hash it had when this version was made and unpacks it next to itself, with the
`licenses.txt` it comes with. It runs as a separate program that OpenController talks to over a
local TCP connection; OpenController includes none of its code and stays MIT.

> VIIPER - Virtual Input over IP EmulatoR
>
> Copyright (C) 2025-2026 Peter Repukat
>
> This program is free software: you can redistribute it and/or modify it under the terms of the
> GNU General Public License as published by the Free Software Foundation, either version 3 of
> the License, or (at your option) any later version.

The source of that version is at https://github.com/Alia5/VIIPER/tree/v0.8.2 and the license at
https://www.gnu.org/licenses/gpl-3.0.html. [usbip-win2](https://github.com/vadimgrn/usbip-win2),
the driver VIIPER needs, is installed with its own installer, under its own license.

## Rust crates

Each crate's license text is in its source, at the repository listed. Where a crate offers a
choice of licenses, OpenController uses it under the first one that applies.

| Crate | Version | License |
|---|---|---|
"""


def linked(target: str) -> tuple[dict, set, set]:
    """The packages, the workspace members and what the two programs link when built for `target`,
    following normal dependencies only (no build scripts, no tests)."""
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
            cwd=ROOT,
            check=True,
            capture_output=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    members = set(meta["workspace_members"])
    todo = [i for i in members if packages[i]["name"] in PROGRAMS]
    seen = set()
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                todo.append(dep["pkg"])
    return packages, members, seen


def main() -> None:
    # Every archive ships the same notices, so they cover what any of the builds links.
    packages, members, seen = {}, set(), set()
    for target in TARGETS:
        p, m, s = linked(target)
        packages |= p
        members |= m
        seen |= s
    rows = sorted(
        {
            (packages[i]["name"], packages[i]["version"], packages[i].get("license") or "see repository", packages[i].get("repository") or "")
            for i in seen - members
        }
    )
    lines = []
    for name, version, license, repo in rows:
        crate = f"[{name}]({repo})" if repo else name
        lines.append(f"| {crate} | {version} | {license.replace('/', ' OR ')} |")
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text(HEADER + "\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(rows)} crates")


if __name__ == "__main__":
    main()
