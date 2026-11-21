"""Traces a maker's rear view into a pads.json look on the front look's grid:
trace_back.py CONFIG.json OUT.json OVERLAY.png

Same flood-fill tracing as scripts/pads/trace.py, but every shoulder part is given by hand with
the input it stands for (a rear view shows the controller's left trigger on the right), and the
grid placement comes from the front look so both views line up:

  align: {"front": FRONT.json, "T": {"s":..,"dx":..}}   same scale as the front's trace (same
         manual page scale and clip width): x as the front, y so the grips' bottoms meet
  align: {"front": FRONT.json, "bbox": true}            scaled so the body is as wide as the
         front's body, centred on it, the grips' bottoms meeting
  triggers / bumpers: {"lt": part, "rt": part} / {"lb": .., "rb": ..}
  buttons: [{input, seed|seeds|poly|at, label?, circle?}]
  panels: [{seed|poly|circle, style}], dots: [[x, y, r]] or [[x, y]]
  cut: [part, ...] regions taken out of the body besides the shoulders
  part: {"seed": [x, y]} | {"seeds": [[x, y], ...]} | {"poly": [[x, y], ...]} | {"tone": t, "seed": ..}
"""
import json
import math
import sys

import cv2
import numpy as np

sys.path.insert(0, __file__.replace("\\", "/").rsplit("/", 1)[0])
from source import render  # noqa: E402

cfg = json.load(open(sys.argv[1]))
img = render(cfg["source"], cfg.get("width", 1600))
rgb = np.array(img)
H, W = rgb.shape[:2]
if cfg.get("ink_mode") == "photo":
    ink = (rgb.min(axis=2) < cfg.get("ink", 215)).astype(np.uint8)
else:
    ink = (rgb.max(axis=2) < cfg.get("ink", 110)).astype(np.uint8)
line = cfg.get("line", 6)
for x0, y0, x1, y1 in cfg.get("erase", []):
    ink[y0:y1, x0:x1] = 0
for x0, y0, x1, y1 in cfg.get("wall", []):
    cv2.line(ink, (x0, y0), (x1, y1), 1, max(2, line // 2))


def flood(seed, base=None):
    free = (1 - ink) if base is None else base
    x, y = int(seed[0]), int(seed[1])
    if free[y, x] == 0:
        ys, xs = np.nonzero(free[max(0, y - 12): y + 13, max(0, x - 12): x + 13])
        if xs.size == 0:
            raise SystemExit(f"seed {seed} sits in ink")
        k = int(np.argmin((xs - min(x, 12)) ** 2 + (ys - min(y, 12)) ** 2))
        x, y = int(xs[k]) + max(0, x - 12), int(ys[k]) + max(0, y - 12)
    m = np.zeros((H + 2, W + 2), np.uint8)
    work = free.copy()
    cv2.floodFill(work, m, (x, y), 2)
    return (work == 2).astype(np.uint8)


def fill_holes(mask):
    cs, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    out = np.zeros_like(mask)
    cv2.drawContours(out, cs, -1, 1, -1)
    return out


def widen(mask, px):
    if px <= 0:
        return mask
    k = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * px + 1, 2 * px + 1))
    return cv2.dilate(mask, k)


def tone(seed, tol):
    m = np.zeros((H + 2, W + 2), np.uint8)
    work = np.ascontiguousarray(rgb[:, :, :3])
    t = (tol, tol, tol)
    cv2.floodFill(work, m, (int(seed[0]), int(seed[1])), (0, 0, 0), t, t, 4 | cv2.FLOODFILL_FIXED_RANGE | cv2.FLOODFILL_MASK_ONLY | (255 << 8))
    out = (m[1:-1, 1:-1] > 0).astype(np.uint8)
    return widen(fill_holes(out), round(line / 2))


def part(p):
    """A part's mask, widened by half a line so its edge sits on the line's middle."""
    if "stadium" in p:
        x0, y0, x1, y1 = p["stadium"]
        r = min(x1 - x0, y1 - y0) / 2
        m = np.zeros_like(ink)
        cv2.rectangle(m, (int(x0 + r), int(y0)), (int(x1 - r), int(y1)), 1, -1) if x1 - x0 >= y1 - y0 else cv2.rectangle(m, (int(x0), int(y0 + r)), (int(x1), int(y1 - r)), 1, -1)
        for cx, cy in (((x0 + r, (y0 + y1) / 2), (x1 - r, (y0 + y1) / 2)) if x1 - x0 >= y1 - y0 else (((x0 + x1) / 2, y0 + r), ((x0 + x1) / 2, y1 - r))):
            cv2.circle(m, (int(cx), int(cy)), int(r), 1, -1)
        return m
    if "poly" in p:
        m = np.zeros_like(ink)
        cv2.fillPoly(m, [np.array(p["poly"], np.int32)], 1)
        return m
    if "at" in p:
        m = np.zeros_like(ink)
        x, y, r = p["at"]
        cv2.circle(m, (int(x), int(y)), int(r), 1, -1)
        return m
    if "tone" in p:
        return tone(p["seed"], p["tone"])
    seeds = p["seeds"] if "seeds" in p else [p["seed"]]
    m = np.zeros_like(ink)
    for sd in seeds:
        f = flood(sd)
        if f.sum() > 0.4 * H * W:
            raise SystemExit(f"seed {sd} leaks into the background")
        m |= f
    if len(seeds) > 1:
        k = 2 * line + 3
        m = cv2.morphologyEx(m, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
    m = fill_holes(m)
    return widen(m, round(line / 2) + p.get("grow_px", 0))


def contour(mask):
    cs, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    c = max(cs, key=cv2.contourArea)
    c = cv2.approxPolyDP(c, cfg.get("epsilon", 1.2), True)
    return [tuple(map(float, p[0])) for p in c]


T = {"s": 1.0, "dx": 0.0, "dy": 0.0}


def g(x, y):
    return round(x * T["s"] + T["dx"], 2), round(y * T["s"] + T["dy"], 2)


def control(prev, p, nxt, sign):
    tx, ty = (nxt[0] - prev[0]) / 6 * sign, (nxt[1] - prev[1]) / 6 * sign
    if sign < 0:
        tx, ty = -tx, -ty
    seg = math.hypot(nxt[0] - p[0], nxt[1] - p[1])
    a = (p[0] - prev[0], p[1] - prev[1])
    b = (nxt[0] - p[0], nxt[1] - p[1])
    la, lb = math.hypot(*a), math.hypot(*b)
    if la and lb and (a[0] * b[0] + a[1] * b[1]) / (la * lb) < 0.55:
        return p
    lt = math.hypot(tx, ty)
    if lt > seg / 3 and lt:
        tx, ty = tx * seg / 3 / lt, ty * seg / 3 / lt
    return (round(p[0] + tx, 2), round(p[1] + ty, 2))


def path(points, corners=False):
    pts = [g(*p) for p in points]
    n = len(pts)
    d = [f"M{pts[0][0]} {pts[0][1]}"]
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        if corners:
            d.append(f"L{p2[0]} {p2[1]}")
            continue
        c1 = control(p0, p1, p2, 1)
        c2 = control(p3, p2, p1, -1)
        d.append(f"C{c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]} {p2[1]}")
    return "".join(d) + "Z"


def circle_of(mask):
    (x, y), r = cv2.minEnclosingCircle(cv2.findNonZero(mask))
    return x, y, r


def gcircle(x, y, r):
    gx, gy = g(x, y)
    return [gx, gy, round(r * T["s"], 2)]


overlay = rgb.copy()


def show(mask, color):
    edge = cv2.morphologyEx(mask, cv2.MORPH_GRADIENT, np.ones((3, 3), np.uint8))
    overlay[edge > 0] = color


# The body: what the outside cannot reach, less the shoulders and whatever else is cut.
outside = np.zeros_like(ink)
bk = cfg.get("body_close", 0)
body_ink = cv2.morphologyEx(ink, cv2.MORPH_CLOSE, np.ones((bk, bk), np.uint8)) if bk else ink
for x, y in [(x, y) for x in range(0, W, 8) for y in (0, H - 1)] + [(x, y) for y in range(0, H, 8) for x in (0, W - 1)]:
    if body_ink[y, x] == 0 and outside[y, x] == 0:
        outside |= flood((x, y), (1 - body_ink).astype(np.uint8))
solid = fill_holes(1 - outside)
shoulders = {}
for key in ("triggers", "bumpers"):
    for name, p in cfg.get(key, {}).items():
        shoulders[name] = part(p)
        print(" ", name, "area", int(shoulders[name].sum()))
cuts = [part(p) for p in cfg.get("cut", [])]
body = solid.copy()
for m in list(shoulders.values()) + cuts:
    body[widen(m, 2) > 0] = 0
for p in cfg.get("add", []):
    body |= part(p)
k = cfg.get("body_open", 11)
body = cv2.morphologyEx(body, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
n, lab, stats, _ = cv2.connectedComponentsWithStats(body)
body = (lab == 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))).astype(np.uint8)
bpts = contour(body)

al = cfg["align"]
front = json.load(open(al["front"], encoding="utf-8"))


def path_box(d):
    import re
    nums = [float(v) for v in re.findall(r"-?\d*\.?\d+", d)]
    xs, ys = nums[0::2], nums[1::2]
    return min(xs), min(ys), max(xs), max(ys)


fx0, fy0, fx1, fy1 = path_box(front["body"])
bx = np.array(bpts)
x0, y0 = bx.min(axis=0)
x1, y1 = bx.max(axis=0)
if "T" in al:
    T["s"], T["dx"] = al["T"]["s"], al["T"]["dx"]
    if al.get("center"):
        T["dx"] = (fx0 + fx1) / 2 - (x0 + x1) / 2 * T["s"]
else:
    T["s"] = (fx1 - fx0) / (x1 - x0) * al.get("scale", 1.0)
    T["dx"] = (fx0 + fx1) / 2 - (x0 + x1) / 2 * T["s"]
T["dy"] = fy1 + al.get("dy", 0.0) - y1 * T["s"]

out = {"body": path(bpts)}
show(body, (255, 0, 0))
for key, names in (("triggers", ("lt", "rt")), ("bumpers", ("lb", "rb"))):
    if all(n in shoulders for n in names):
        out[key] = [path(contour(shoulders[n]), cfg.get(key, {}).get(n, {}).get("corners", False)) for n in names]
        for n in names:
            show(shoulders[n], (0, 160, 255))

buttons = []
for b in cfg.get("buttons", []):
    e = {"input": b["input"]}
    m = part(b)
    if b.get("circle", False):
        x, y, r = circle_of(m)
        e["circle"] = gcircle(x, y, r * b.get("grow", 1))
        cv2.circle(overlay, (int(x), int(y)), int(r), (200, 0, 200), 2)
    else:
        e["d"] = path(contour(m), b.get("corners", False))
        show(m, (200, 0, 200))
    for k in ("label", "icon"):
        if k in b:
            e[k] = b[k]
    buttons.append(e)
out["buttons"] = buttons
panels = []
for p in cfg.get("panels", []):
    m = part(p)
    panels.append({"style": p.get("style", "shade"), "d": path(contour(m), p.get("corners", False))})
    show(m, (0, 120, 120))
if panels:
    out["panels"] = panels
dots = []
for sd in cfg.get("dots", []):
    if len(sd) == 3:
        dots.append(gcircle(*sd))
        cv2.circle(overlay, (int(sd[0]), int(sd[1])), max(2, int(sd[2])), (255, 0, 120), 2)
        continue
    m = widen(fill_holes(flood(sd)), round(line / 2))
    x, y, r = circle_of(m)
    dots.append(gcircle(x, y, r))
    cv2.circle(overlay, (int(x), int(y)), max(2, int(r)), (255, 0, 120), 2)
if dots:
    out["dots"] = dots
out["glyphs"] = cfg.get("glyphs", front.get("glyphs", "xbox"))
if front.get("tint"):
    out["tint"] = True
json.dump(out, open(sys.argv[2], "w", encoding="utf-8", newline="\n"), indent=1)
cv2.imwrite(sys.argv[3], cv2.cvtColor(overlay, cv2.COLOR_RGB2BGR))
print("scale", round(T["s"], 4), "dx", round(T["dx"], 2), "dy", round(T["dy"], 2), "body box", g(x0, y0), g(x1, y1), "front box", (fx0, fy0, fx1, fy1))
