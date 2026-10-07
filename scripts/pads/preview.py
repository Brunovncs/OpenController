"""Draws every look in pads.json as the app does, side by side: preview.py PADS.json OUT.png [look ...]"""
import json
import sys

import pymupdf

pads = json.load(open(sys.argv[1]))
names = sys.argv[3:] or list(pads)
LETTERS = {"south": "A", "east": "B", "west": "X", "north": "Y"}


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
    for pn in p.get("panels", []):
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
    for x, y, r in p.get("dots", []):
        o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{soft}"/>')
    if "touchpad" in p:
        o.append(f'<path d="{p["touchpad"]}" fill="{part}" stroke="{edge}"/>')
    for s in p.get("sticks", []):
        x, y, r = s["circle"]
        if "ring" in s:
            o.append(f'<circle cx="{x}" cy="{y}" r="{s["ring"][0]}" fill="none" stroke="{soft}" stroke-width="{s["ring"][1]}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{s.get("well", r * 1.18)}" fill="black" fill-opacity="0.28" stroke="{soft}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="#3d4048" stroke="{edge}"/>')
        o.append(f'<circle cx="{x}" cy="{y}" r="{r * 0.62}" fill="none" stroke="{soft}"/>')
    for b in p.get("buttons", []):
        if "circle" in b:
            x, y, r = b["circle"]
            o.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{part}" stroke="{edge}"/>')
            if b["input"] in LETTERS:
                o.append(f'<text x="{x}" y="{y + r * 0.36}" text-anchor="middle" font-size="{r}" font-family="Arial" font-weight="bold" fill="#aeb6c1">{LETTERS[b["input"]]}</text>')
            elif b.get("icon"):
                o.append(f'<text x="{x}" y="{y + r * 0.3}" text-anchor="middle" font-size="{r * 0.8}" font-family="Arial" fill="#aeb6c1">{b["icon"][0]}</text>')
        elif "d" in b:
            o.append(f'<path d="{b["d"]}" fill="{part}" stroke="{edge}"/>')
    o.append("</svg>")
    return "\n".join(o)


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
