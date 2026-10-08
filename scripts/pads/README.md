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
| `g820` | Redragon's front render of the Darkflame G820 on its Brazilian store (`hg04f-927-7k115gx8ht.png`), laid on white; a black pad on a black body has no lines to flood, so every part is measured |
| `gamesirG7` | GameSir's render of the G7 SE on gamesir.com |
| `gamesirNova` | GameSir's render of the Nova Lite on gamesir.com |
| `gamesirCyclone` | The front view in GameSir's Cyclone 2 manual |
| `redragonHarrow` | Redragon Brazil's store render of the Harrow G808 V2 |
| `redragonSaturn` | Redragon Brazil's store photo of the Saturn G807 |
| `razerWolverineV3` | Razer's product image of the Wolverine V3 Pro on razer.com |
| `victrixBfg` | Turtle Beach's store image of the Victrix Pro BFG Reloaded for Xbox |
| `naconRev5Pro` | Nacon's product image of the Revolution 5 Pro |
| `turtleBeachStealthUltra` | Turtle Beach's store image of the Stealth Ultra |
| `xbox360` | The front product shot in Microsoft's datasheet for the Xbox 360 Controller for Windows |
| `xboxS` | "XboxOriginalController.jpg" on Wikimedia Commons, by Swaaye, CC BY-SA 3.0 (no official front view is left online) |
| `logitechF310` | Logitech G's product image of the F310 |
| `horiOcta` | HORI Europe's product image of the Fighting Commander OCTA for Xbox |
| `atariVcs` | Atari's product photo of the VCS Modern Controller |
| `n64` | The vector drawing in 8BitDo's 64 Bluetooth Controller manual |
| `genesis3b` | Sega's drawing of the 3-button pad in the Mega Drive Mini manual |
| `megadrive6b` | Sega's drawing of the 6-button pad in the Mega Drive Mini manual |
| `gamesirG7Pro` | The device layout drawing in GameSir's G7 Pro manual |
| `flydigiVader` | The front drawing in Flydigi's Vader 5 Pro manual |
| `flydigiApex` | The front drawing in Flydigi's Apex 5 manual, its filled shapes only, so the grey callouts drop out |
| `onikumaC1` | A retailer's front photo of the Onikuma C1 (compumarts.com), its watermark and the JPEG noise under the outline erased |
| `psClassic` | "PlayStation Controller transparent.png" on Wikimedia Commons, CC BY-SA 3.0 (Sony publishes no straight front view) |
| `generic`, `handheld` | The Ultimate 2C's outline with neutral buttons, and a handheld drawn by hand (`derive_misc.py`) |

`looks/*.json` says, for each, where its source is and where each part is (a seed point inside it,
or its circle). `trace.py` floods each part from its seed, places it on the grid and writes the
look; `build_pads.py` joins them:

    python trace.py looks/ds4.json out/ds4.json out/ds4-overlay.png   # the overlay shows what was found
    python build_pads.py ../../assets/pads.json dualsense=out/ds5.json ...
    python build_pads.py --add ../../assets/pads.json gamesirG7=out/gamesirG7.json   # one look, the rest kept

The looks traced from one model's pictures are given to its USB ids in `DRAWINGS`, in
`crates/open-controller-core/src/models.rs`; the rest are drawn by family. A model that copies
another's ids (the Onikuma C1 reports itself as a Switch Pro Controller) is in `ALIASES` instead,
and is drawn as itself once its owner picks it on its Information page. The site gets the same
from `models.json`.

It needs numpy, opencv-python-headless, Pillow and PyMuPDF. After changing `pads.json`, copy it to
the website's `data/pads.json`.
