# Controller drawings

`assets/pads.json` holds the drawing of each kind of controller the window and the website show:
the outline, bumpers, triggers, sticks and every button, on a 400 by 280 grid. Each one is traced
from its maker's own front view, so the proportions and the places of the controls are the real
ones; nothing of the source picture is kept, only the shapes.

| Look | Traced from |
| --- | --- |
| `ultimate2`, `ultimate2c`, `ultimate`, `sn30pro` (Pro 2), `retro` (SN30) | The vector drawings in 8BitDo's manuals, download.8bitdo.com/Manual/Controller/ |
| `dualsense` | The front view in Sony's DualSense manual (CFI-ZCT1) |
| `dualsenseEdge` | The DualSense, with the Edge's Fn buttons (`derive_edge.py`) |
| `ds4`, `ds3` | "Dualshock 4 Layout.svg" and "Dualshock3 Layout.svg" on Wikimedia Commons, by Tokyoship, CC BY 3.0 |
| `joycons` | "Nintendo Switch Joy-Con illustration.svg" on Wikimedia Commons, public domain |
| `xbox` | The front product shot on xbox.com's Xbox Wireless Controller page |
| `switchPro` | The diagram on Nintendo's support page for the Pro Controller |
| `generic`, `handheld` | The Ultimate 2C's outline with neutral buttons, and a handheld drawn by hand (`derive_misc.py`) |

`looks/*.json` says, for each, where its source is and where each part is (a seed point inside it,
or its circle). `trace.py` floods each part from its seed, places it on the grid and writes the
look; `build_pads.py` joins them:

    python trace.py looks/ds4.json out/ds4.json out/ds4-overlay.png   # the overlay shows what was found
    python build_pads.py ../../assets/pads.json dualsense=out/ds5.json ...

It needs numpy, opencv-python-headless, Pillow and PyMuPDF. After changing `pads.json`, copy it to
the website's `data/pads.json`.
