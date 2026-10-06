"""Rasterises an SVG at exact pixel sizes with Microsoft Edge in headless mode, which every
Windows 10 and 11 machine has, so the icons need no graphics library. Used by render_icon.py.

    python scripts/render_svg.py icon.svg 16 32 256   ->   icon-16.png, icon-32.png, icon-256.png
"""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

EDGE = [
    Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Microsoft/Edge/Application/msedge.exe",
    Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "Microsoft/Edge/Application/msedge.exe",
]
# Headless Edge will not make a window smaller than this; the picture is cropped out of it.
CANVAS = 600


def render(svg: Path, size: int) -> Image.Image:
    edge = next((e for e in EDGE if e.exists()), None)
    if edge is None:
        sys.exit("Microsoft Edge was not found")
    with tempfile.TemporaryDirectory() as tmp:
        page = Path(tmp) / "page.html"
        shot = Path(tmp) / "shot.png"
        page.write_text(
            f'<html><body style="margin:0;background:transparent">'
            f'<img src="{svg.resolve().as_uri()}" width="{size}" height="{size}" style="display:block"></body></html>',
            encoding="utf-8",
        )
        subprocess.run(
            [
                str(edge),
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                "--default-background-color=00000000",
                f"--window-size={CANVAS},{CANVAS}",
                f"--user-data-dir={tmp}\\profile",
                f"--screenshot={shot}",
                page.as_uri(),
            ],
            check=True,
            capture_output=True,
            timeout=60,
        )
        return Image.open(shot).convert("RGBA").crop((0, 0, size, size))


if __name__ == "__main__":
    src = Path(sys.argv[1])
    for s in map(int, sys.argv[2:]):
        out = src.with_name(f"{src.stem}-{s}.png")
        render(src, s).save(out)
        print(out)
