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

HEADER = """# Third-party notices

Open Controller is MIT licensed (see [LICENSE](LICENSE)). Its programs include the code below,
each under its own license. ViGEmBus, HidHide, DsHidMini and BthPS3 are not included: Open
Controller downloads their official installers from Nefarius' GitHub releases when you ask it to.

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

## Rust crates

Each crate's license text is in its source, at the repository listed. Where a crate offers a
choice of licenses, Open Controller uses it under the first one that applies.

| Crate | Version | License |
|---|---|---|
"""


def main() -> None:
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"],
            cwd=ROOT,
            check=True,
            capture_output=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    members = set(meta["workspace_members"])
    # Everything the two programs link, following normal dependencies only (no build scripts,
    # no tests).
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
