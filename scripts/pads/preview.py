"""Draws every look in pads.json as the app does, side by side: preview.py PADS.json OUT.png [look ...]

A wheel is drawn as it stands and turned by STEER degrees, when that is set in the environment."""
import json
import os
import re
import sys

import pymupdf

pads = json.load(open(sys.argv[1]))
names = sys.argv[3:] or list(pads)
LETTERS = {"south": "A", "east": "B", "west": "X", "north": "Y"}


def points(d):
    """The x and y of every point of an absolute path of M, L, H, V, C and Z."""
    xs, ys, x, y = [], [], 0.0, 0.0
    for cmd, args in re.findall(r"([MLHVCZ])([^MLHVCZ]*)", d):
        n = [float(v) for v in re.findall(r"-?\d*\.?\d+", args)]
        if cmd == "H":
            x = n[-1]
        elif cmd == "V":
            y = n[-1]
        elif n:
            x, y = n[-2], n[-1]
        if cmd != "Z":
            xs.append(x)
            ys.append(y)
    return xs, ys


def panels(ps, edge, soft, part):
    o = []
    for pn in ps:
        st = pn["style"]
        if st == "line":
            o.append(f'<path d="{pn["d"]}" fill="none" stroke="{soft}" stroke-width="1.2"/>')
        elif st == "glow":
            o.append(f'<path d="{pn["d"]}" fill="none" stroke="#60cdff" stroke-opacity="0.55" stroke-width="2.2"/>')
        elif st == "light":
            o.append(f'<path d="{pn["d"]}" fill="#60cdff" fill-opacity="0.28" stroke="#60cdff" stroke-opacity="0.5"/>')
        elif st == "part":
            o.append(f'<path d="{pn["d"]}" fill="{part}" stroke="{edge}"/>')
        else:
            o.append(f'<path d="{pn["d"]}" fill="black" fill-opacity="0.28" stroke="{soft}"/>')
    return o


def svg(p):
    o = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 280" width="800" height="560">',
         '<defs><linearGradient id="b" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3d4048"/><stop offset="1" stop-color="#2a2c32"/></linearGradient></defs>',
         '<rect width="400" height="280" fill="#1a1b1f"/>']
    edge, soft, part = "#5a5d63", "#44474d", "#16171a"
    for d in p.get("triggers", []):
        o.append(f'<path d="{d}" fill="{part}" stroke="{edge}"/>')
    for d in p.get("bumpers", []):
        o.append(f'<path d="{d}" fill="{part}" stroke="{edge}"/>')
    o.append(f'<path d="{p["body"]}" fill="#34363c" stroke="{edge}"/>')
    o += panels([pn for pn in p.get("panels", []) if not pn.get("grip")], edge, soft, part)
    for x, y, r in p.get("dots", []):
        o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{soft}"/>')
    if "touchpad" in p:
        o.append(f'<path d="{p["touchpad"]}" fill="{part}" stroke="{edge}"/>')
    for lv in p.get("levers", []):
        o.append(f'<path d="{lv["d"]}" fill="{part}" stroke="{edge}"/>')
    for s in p.get("sticks", []):
        x, y, r = s["circle"]
        if "ring" in s:
            o.append(f'<circle cx="{x}" cy="{y}" r="{s["ring"][0]}" fill="none" stroke="{soft}" stroke-width="{s["ring"][1]}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{s.get("well", r * 1.18)}" fill="black" fill-opacity="0.28" stroke="{soft}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="#3d4048" stroke="{edge}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{r * 0.62}" fill="none" stroke="{soft}"/>')
    grips = [s for s in p.get("sticks", []) if "grip" in s]
    o += buttons([b for b in p.get("buttons", []) if not b.get("grip")], edge, part)
    for s in grips:
        x, y, _ = s["circle"]
        turn = float(os.environ.get("STEER", 0)) if "steer" in s else 0
        o.append(f'<g transform="rotate({turn} {x} {y})">')
        o.append(f'<path d="{s["grip"]}" fill="url(#b)" stroke="{edge}"/>')
        o += panels([pn for pn in p.get("panels", []) if pn.get("grip")], edge, soft, part)
        o += buttons([b for b in p.get("buttons", []) if b.get("grip")], edge, part)
        o.append("</g>")
    o.append("</svg>")
    return "\n".join(o)


def buttons(bs, edge, part):
    o = []
    for b in bs:
        if b.get("label") and "d" in b:
            o.append(f'<path d="{b["d"]}" fill="{part}" stroke="{edge}"/>')
            xs, ys = points(b["d"])
            x, y, r = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2, min(max(xs) - min(xs), max(ys) - min(ys)) / 2
            o.append(f'<text x="{x}" y="{y + r * 0.36}" text-anchor="middle" font-size="{r}" font-family="Arial" font-weight="bold" fill="#aeb6c1">{b["label"]}</text>')
            continue
        if "circle" in b:
            x, y, r = b["circle"]
            o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{part}" stroke="{edge}"/>')
            if b.get("label"):
                o.append(f'<text x="{x}" y="{y + r * 0.36}" text-anchor="middle" font-size="{r}" font-family="Arial" font-weight="bold" fill="#aeb6c1">{b["label"]}</text>')
            elif b["input"] in LETTERS:
                o.append(f'<text x="{x}" y="{y + r * 0.36}" text-anchor="middle" font-size="{r}" font-family="Arial" font-weight="bold" fill="#aeb6c1">{LETTERS[b["input"]]}</text>')
            elif b.get("icon"):
                o.append(f'<text x="{x}" y="{y + r * 0.3}" text-anchor="middle" font-size="{r * 0.8}" font-family="Arial" fill="#aeb6c1">{b["icon"][0]}</text>')
        elif "d" in b:
            o.append(f'<path d="{b["d"]}" fill="{part}" stroke="{edge}"/>')
    return o


from PIL import Image  # noqa: E402

ims = []
for n in names:
    doc = pymupdf.open(stream=svg(pads[n]).encode(), filetype="svg")
    pm = doc[0].get_pixmap(alpha=False)
    ims.append(Image.frombytes("RGB", (pm.width, pm.height), pm.samples))
cols = min(3, len(ims))
rows = (len(ims) + cols - 1) // cols
sheet = Image.new("RGB", (cols * 800, rows * 560), "black")
for i, im in enumerate(ims):
    sheet.paste(im, ((i % cols) * 800, (i // cols) * 560))
sheet.save(sys.argv[2])
