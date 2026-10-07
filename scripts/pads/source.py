"""Renders a drawing source to a grayscale PNG at a fixed width, and a copy with a coordinate grid
for picking seeds: source.py SPEC OUT_PREFIX [WIDTH]

SPEC is `pdf:FILE:PAGE:x0,y0,x1,y1` or `svg:FILE:x0,y0,x1,y1` (clip in document units) or
`img:FILE:x0,y0,x1,y1` (clip in pixels).
"""
import sys

import numpy as np
import pymupdf
from PIL import Image, ImageDraw


def solid_svg(path: str, page: int, clip, flags=("nodash",)) -> bytes:
    """The page's vector drawings that cross the clip, without dashed lines or text, as SVG."""
    p = pymupdf.open(path)[page]
    x0, y0, x1, y1 = clip
    rect = pymupdf.Rect(x0, y0, x1, y1)
    rgb = lambda c: "none" if not c else "rgb(%d,%d,%d)" % tuple(int(v * 255) for v in c)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0} {y0} {x1 - x0} {y1 - y0}" width="{x1 - x0}" height="{y1 - y0}"><rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" fill="white"/>']
    for dr in p.get_drawings():
        if not dr["rect"].intersects(rect):
            continue
        # Callouts run out to their labels, beyond the drawing.
        if "within" in flags and not rect.contains(dr["rect"]):
            continue
        if "nodash" in flags and str(dr.get("dashes")) not in ("[] 0", "None"):
            continue
        colour = dr.get("fill") if dr["type"] == "f" else dr.get("color")
        if "nogray" in flags and colour and max(colour) - min(colour) < 0.05 and 0.25 < sum(colour) / 3 < 0.9:
            continue
        # White strokes are knock-outs that part callouts from the lines under them.
        if "nowhite" in flags and dr["type"] in ("s", "fs") and dr.get("color") and min(dr["color"]) > 0.95:
            continue
        d, last = [], None
        for it in dr["items"]:
            k = it[0]
            if k == "l":
                a, b = it[1], it[2]
                if last is None or abs(last.x - a.x) > 0.01 or abs(last.y - a.y) > 0.01:
                    d.append(f"M{a.x} {a.y}")
                d.append(f"L{b.x} {b.y}")
                last = b
            elif k == "c":
                a, c1, c2, b = it[1:5]
                if last is None or abs(last.x - a.x) > 0.01 or abs(last.y - a.y) > 0.01:
                    d.append(f"M{a.x} {a.y}")
                d.append(f"C{c1.x} {c1.y} {c2.x} {c2.y} {b.x} {b.y}")
                last = b
            elif k == "re":
                q = it[1]
                d.append(f"M{q.x0} {q.y0}H{q.x1}V{q.y1}H{q.x0}Z")
                last = None
            elif k == "qu":
                q = it[1]
                d.append(f"M{q.ul.x} {q.ul.y}L{q.ur.x} {q.ur.y}L{q.lr.x} {q.lr.y}L{q.ll.x} {q.ll.y}Z")
                last = None
        if dr.get("closePath"):
            d.append("Z")
        fill = rgb(dr.get("fill")) if dr["type"] in ("f", "fs") else "none"
        stroke = rgb(dr.get("color")) if dr["type"] in ("s", "fs") else "none"
        rule = ' fill-rule="evenodd"' if dr.get("even_odd") else ""
        out.append(f'<path d="{"".join(d)}" fill="{fill}"{rule} stroke="{stroke}" stroke-width="{dr.get("width") or 0}" stroke-linecap="round" stroke-linejoin="round"/>')
    out.append("</svg>")
    return "\n".join(out).encode()


def render(spec: str, width: int) -> Image.Image:
    kind, rest = spec.split(":", 1)
    if kind in ("pdfsolid", "pdfvec"):
        if kind == "pdfvec":
            path, page, clip, flags = rest.rsplit(":", 3)
            flags = tuple(flags.split(","))
        else:
            path, page, clip = rest.rsplit(":", 2)
            flags = ("nodash",)
        clip = tuple(map(float, clip.split(",")))
        doc = pymupdf.open(stream=solid_svg(path, int(page), clip, flags), filetype="svg")
        z = width / (clip[2] - clip[0])
        pm = doc[0].get_pixmap(matrix=pymupdf.Matrix(z, z), alpha=False)
        return Image.frombytes("RGB", (pm.width, pm.height), pm.samples)
    if kind in ("pdf", "svg"):
        if kind == "pdf":
            path, page, clip = rest.rsplit(":", 2)
            p = pymupdf.open(path)[int(page)]
        else:
            path, clip = rest.rsplit(":", 1)
            p = pymupdf.open(path)[0]
        x0, y0, x1, y1 = map(float, clip.split(","))
        z = width / (x1 - x0)
        pm = p.get_pixmap(matrix=pymupdf.Matrix(z, z), clip=pymupdf.Rect(x0, y0, x1, y1), alpha=False)
        return Image.frombytes("RGB", (pm.width, pm.height), pm.samples)
    path, clip = rest.rsplit(":", 1)
    x0, y0, x1, y1 = map(int, clip.split(","))
    im = Image.open(path).convert("RGB").crop((x0, y0, x1, y1))
    return im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)


if __name__ == "__main__":
    spec, prefix = sys.argv[1], sys.argv[2]
    width = int(sys.argv[3]) if len(sys.argv) > 3 else 1600
    im = render(spec, width)
    im.save(prefix + ".png")
    g = im.copy()
    d = ImageDraw.Draw(g)
    for x in range(0, g.width, 50):
        d.line([(x, 0), (x, g.height)], fill=(255, 0, 0) if x % 200 == 0 else (255, 170, 170), width=1)
        if x % 100 == 0:
            d.text((x + 2, 2), str(x), fill=(200, 0, 0))
    for y in range(0, g.height, 50):
        d.line([(0, y), (g.width, y)], fill=(255, 0, 0) if y % 200 == 0 else (255, 170, 170), width=1)
        if y % 100 == 0:
            d.text((2, y + 2), str(y), fill=(200, 0, 0))
    g.save(prefix + "-grid.png")
    print(im.size)
