"""Builds the icon files from their SVGs (needs Python with Pillow, and Microsoft Edge):

- assets/icon.ico: the app icon, for Explorer, the taskbar and the window. Sizes up to 32 come
  from icon-small.svg, drawn for small sizes; larger ones from icon.svg.
- assets/icon-256.png, for the README.
- assets/tray-dark.ico and assets/tray-light.ico: the notification-area icon for a dark and a
  light taskbar, without a tile.

Run scripts/icon_small.py first after changing the small drawings.
"""

import io
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from render_svg import render  # noqa: E402

ASSETS = Path(__file__).resolve().parent.parent / "assets"


def ico(images: list) -> bytes:
    """An ICO file with one PNG entry per image, as Windows Vista and later read them."""
    blobs = []
    for im in images:
        buf = io.BytesIO()
        im.save(buf, "PNG")
        blobs.append(buf.getvalue())
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for im, blob in zip(images, blobs):
        w = im.width if im.width < 256 else 0
        out += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    return out + b"".join(blobs)


def main() -> None:
    small, large = ASSETS / "icon-small.svg", ASSETS / "icon.svg"
    app = [render(small if s <= 32 else large, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)]
    (ASSETS / "icon.ico").write_bytes(ico(app))
    app[-1].save(ASSETS / "icon-256.png")
    for theme in ("dark", "light"):
        tray = [render(ASSETS / f"tray-{theme}.svg", s) for s in (16, 20, 24, 32, 40, 48)]
        (ASSETS / f"tray-{theme}.ico").write_bytes(ico(tray))
    print("icon.ico, icon-256.png, tray-dark.ico, tray-light.ico")


if __name__ == "__main__":
    main()
