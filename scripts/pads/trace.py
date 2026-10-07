"""Traces a controller drawing into OpenController's pad geometry: trace.py CONFIG.json OUT.json OVERLAY.png

The source (a manufacturer's line drawing, rendered big) is split into regions by its ink: each
part is the region a seed point floods to, widened by half a line so its edge sits on the line's
middle. Round parts become circles; others become smooth paths. Everything is moved onto the
400 x 280 grid the app and the site draw on.

Config:
  source, width, ink (darkest grey that is not a line), line (line width in pixels)
  bumpers: [[x, y], [x, y]]          seeds of the left and right bumper
  sticks:  [{seed, ring}]           seed inside the cap; ring: a grey ring is measured round it
  buttons: [{input, seed, circle?, label?, icon?, color?}]
  dpad:    {seed, center?}          the cross, split into four arms
  touchpad, panels: [{seed, style}] regions drawn as they are
  dots:    [[x, y]...]              small round lights, measured by seed
  fit:     max content width and height on the grid (default 356 x 236)
"""
import json
import math
import sys

import cv2
import numpy as np

sys.path.insert(0, __file__.rsplit("\\", 1)[0].rsplit("/", 1)[0])
from source import render  # noqa: E402

cfg = json.load(open(sys.argv[1]))
img = render(cfg["source"], cfg.get("width", 1600))
rgb = np.array(img)
gray = cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY)
H, W = gray.shape
# Dark in every channel: a coloured fill as dark as a line is not one.
if cfg.get("ink_mode") == "photo":
    # A product shot: anything clearly off the white backdrop, light colours included.
    ink = (rgb.min(axis=2) < cfg.get("ink", 215)).astype(np.uint8)
else:
    ink = (rgb.max(axis=2) < cfg.get("ink", 110)).astype(np.uint8)
if "callouts" in cfg:
    # Callout lines drawn over the art in one grey: taken out, and the lines they crossed joined
    # up again inside their strips.
    lo, hi = cfg["callouts"]
    # Only thick runs of that grey: the soft edges of black lines share it.
    callouts = ((gray >= lo) & (gray <= hi)).astype(np.uint8)
    callouts = cv2.morphologyEx(callouts, cv2.MORPH_OPEN, np.ones((7, 7), np.uint8))
    callouts = cv2.dilate(callouts, np.ones((7, 7), np.uint8))
    ink[callouts > 0] = 0
    k = cfg.get("bridge", 25)
    closed = cv2.morphologyEx(ink, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
    strip = cv2.dilate(callouts, np.ones((7, 7), np.uint8))
    ink = np.where(strip > 0, closed, ink).astype(np.uint8)
line = cfg.get("line", 6)
if cfg.get("debug_ink"):
    cv2.imwrite(cfg["debug_ink"], (1 - ink) * 255)
for x0, y0, x1, y1 in cfg.get("erase", []):
    ink[y0:y1, x0:x1] = 0
if cfg.get("erase_red"):
    # Red callouts and numbers drawn over a photo.
    red = (rgb[:, :, 0].astype(int) > 140) & (rgb[:, :, 1] < 110) & (rgb[:, :, 2] < 110)
    red = cv2.dilate(red.astype(np.uint8), np.ones((5, 5), np.uint8)) > 0
    # As part of whatever they cross: inside the body they close up; outside, the opening and the
    # largest piece leave them behind.
    ink[red] = 1
# Shadows: only the grey, unsaturated pixels of the box.
spread = rgb.max(axis=2).astype(int) - rgb.min(axis=2)
for x0, y0, x1, y1 in cfg.get("erase_grey", []):
    box = (spread[y0:y1, x0:x1] < 14) & (rgb.min(axis=2)[y0:y1, x0:x1] > 150)
    ink[y0:y1, x0:x1][box] = 0
for x0, y0, x1, y1 in cfg.get("wall", []):
    cv2.line(ink, (x0, y0), (x1, y1), 1, max(2, line // 2))


def flood(seed, base=None):
    """The open (not ink) region round a seed."""
    free = (1 - ink) if base is None else base
    x, y = int(seed[0]), int(seed[1])
    if free[y, x] == 0:
        # On a line: the nearest open pixel instead.
        ys, xs = np.nonzero(free[max(0, y - 12) : y + 13, max(0, x - 12) : x + 13])
        if xs.size == 0:
            raise SystemExit(f"seed {seed} sits in ink")
        k = int(np.argmin((xs - min(x, 12)) ** 2 + (ys - min(y, 12)) ** 2))
        x, y = int(xs[k]) + max(0, x - 12), int(ys[k]) + max(0, y - 12)
        print(f"  seed {seed} was on a line, moved to {[x, y]}")
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
    """The part of one colour round a seed: a button filled in its own grey."""
    m = np.zeros((H + 2, W + 2), np.uint8)
    work = np.ascontiguousarray(rgb[:, :, :3])
    t = (tol, tol, tol)
    cv2.floodFill(work, m, (int(seed[0]), int(seed[1])), (0, 0, 0), t, t, 4 | cv2.FLOODFILL_FIXED_RANGE | cv2.FLOODFILL_MASK_ONLY | (255 << 8))
    out = (m[1:-1, 1:-1] > 0).astype(np.uint8)
    return widen(fill_holes(out), round(line / 2))


def pick(entry, key="seed"):
    return tone(entry[key], entry["tone"]) if "tone" in entry else region(entry[key])


def region(seed, holes=True):
    if seed and isinstance(seed[0], (list, tuple)):
        # A region cut by callout lines: its pieces joined, and the lines between them closed.
        m = np.zeros_like(ink)
        for sd in seed:
            m |= flood(sd)
        k = 2 * line + 3
        m = cv2.morphologyEx(m, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
        if holes:
            m = fill_holes(m)
        return widen(m, round(line / 2))
    m = flood(seed)
    if m.sum() > 0.5 * H * W:
        raise SystemExit(f"seed {seed} leaks into the background")
    if holes:
        m = fill_holes(m)
    return widen(m, round(line / 2))


def contour(mask):
    cs, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    c = max(cs, key=cv2.contourArea)
    c = cv2.approxPolyDP(c, cfg.get("epsilon", 1.2), True)
    return [tuple(map(float, p[0])) for p in c]


# The grid transform, from the body and bumpers once they are known.
T = {"s": 1.0, "dx": 0.0, "dy": 0.0}


def g(x, y):
    return round(x * T["s"] + T["dx"], 2), round(y * T["s"] + T["dy"], 2)


def control(prev, p, nxt, sign):
    """A Catmull-Rom control point next to p, towards nxt, kept within a third of the segment and
    flat at a sharp corner, so long straight edges do not overshoot."""
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
    """A closed path through the points: Catmull-Rom curves, or straight lines for corners."""
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


# Body: everything the outside cannot reach, less the bumpers.
# The outside, from every point of the edge: callouts reaching the edge cut it into pieces.
outside = np.zeros_like(ink)
edge_pts = [(x, y) for x in range(0, W, 8) for y in (0, H - 1)] + [(x, y) for y in range(0, H, 8) for x in (0, W - 1)]
for x, y in edge_pts:
    if ink[y, x] == 0 and outside[y, x] == 0:
        outside |= flood((x, y))
solid = fill_holes(1 - outside)
bumpers = [tone(s["seed"], s["tone"]) if isinstance(s, dict) else region(s) for s in cfg.get("bumpers", [])]
if "bumper_band" in cfg:
    # Shoulders drawn as part of the silhouette: what lies above the line, on each side.
    cut = int(cfg["bumper_band"])
    top = solid.copy()
    top[cut:, :] = 0
    for half in (slice(0, W // 2), slice(W // 2, W)):
        b = np.zeros_like(top)
        b[:, half] = top[:, half]
        b = cv2.morphologyEx(b, cv2.MORPH_OPEN, np.ones((9, 9), np.uint8))
        n, lab, stats, _ = cv2.connectedComponentsWithStats(b)
        bumpers.append((lab == 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))).astype(np.uint8))
body = solid.copy()
for b in bumpers:
    body[widen(b, 2) > 0] = 0
# Leader lines and other thin marks touching the outline from outside come off here.
k = cfg.get("body_open", 11)
body = cv2.morphologyEx(body, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
if "body_tone" in cfg:
    # The body by its colour: what a drawing hangs off it (rails, tabs) stays out.
    body = np.zeros_like(ink)
    for bt in cfg["body_tone"]:
        body |= tone(bt["seed"], bt["tone"])
n, lab, stats, _ = cv2.connectedComponentsWithStats(body)
# Usually one piece; the Joy-Con are two.
order = 1 + np.argsort(-stats[1:, cv2.CC_STAT_AREA])
pieces = [(lab == i).astype(np.uint8) for i in order[: cfg.get("parts", 1)]]
pieces.sort(key=lambda m: np.nonzero(m)[1].mean())
body = np.zeros_like(ink)
for pc in pieces:
    body |= pc
body_contours = [contour(pc) for pc in pieces]
body_pts = [p for c in body_contours for p in c]

allpts = np.array(body_pts + [p for b in bumpers for p in contour(b)])
x0, y0 = allpts.min(axis=0)
x1, y1 = allpts.max(axis=0)
fw, fh = cfg.get("fit", [356, 236])
s = min(fw / (x1 - x0), fh / (y1 - y0))
T.update(s=s, dx=200 - (x0 + x1) / 2 * s, dy=cfg.get("center_y", 146) - (y0 + y1) / 2 * s)

out = {"body": "".join(path(c) for c in body_contours)}
show(body, (255, 0, 0))

if bumpers:
    out["bumpers"] = [path(contour(b)) for b in bumpers]
    for b in bumpers:
        show(b, (0, 160, 255))
    # Triggers peek out behind the bumpers: the same shape, raised and a little narrower.
    tr = []
    for b in bumpers:
        pts = contour(b)
        cx = sum(p[0] for p in pts) / len(pts)
        cy = sum(p[1] for p in pts) / len(pts)
        # Pulled towards the outer end of the bumper, where the trigger sits.
        rise = cfg.get("trigger_rise", 0.035) * (y1 - y0)
        ox = min(p[0] for p in pts) if cx < W / 2 else max(p[0] for p in pts)
        sx, sy = cfg.get("trigger_scale", [0.62, 0.85])
        tr.append(path([(ox + (x - ox) * sx, cy + (y - cy) * sy - rise) for x, y in pts]))
    out["triggers"] = tr

sticks = []
for st in cfg.get("sticks", []):
    if "at" in st:
        x, y, r = st["at"]
        m = np.zeros_like(ink)
        cv2.circle(m, (int(x), int(y)), int(r), 1, -1)
    else:
        m = pick(st)
        x, y, r = circle_of(m)
    entry = {"circle": gcircle(x, y, r)}
    if "well" in st:
        if isinstance(st["well"], (int, float)):
            wx, wy, wr = x, y, float(st["well"])
        else:
            wx, wy, wr = circle_of(region(st["well"]))
        entry["well"] = round(wr * s, 2)
        cv2.circle(overlay, (int(wx), int(wy)), int(wr), (0, 120, 0), 2)
    if st.get("ring"):
        # A grey ring round the stick: its middle radius, from the grey pixels near it.
        ys, xs = np.nonzero((gray > cfg.get("ink", 110)) & (gray < 215))
        dist = np.hypot(xs - x, ys - y)
        near = dist[(dist > r * 1.02) & (dist < r * 2.0)]
        if near.size:
            lo, mid, hi = np.percentile(near, [8, 50, 92])
            entry["ring"] = [round(float(mid) * s, 2), round(float(hi - lo) * s, 2)]
            cv2.circle(overlay, (int(x), int(y)), int(np.median(near)), (0, 200, 0), 2)
    sticks.append(entry)
    show(m, (0, 200, 0))
out["sticks"] = sticks

buttons = []
for b in cfg.get("buttons", []):
    e = {"input": b["input"]}
    if "at" in b:
        x, y, r = b["at"]
        e["circle"] = gcircle(x, y, r)
        cv2.circle(overlay, (int(x), int(y)), int(r), (200, 0, 200), 2)
        for k in ("icon",):
            if k in b:
                e[k] = b[k]
        buttons.append(e)
        continue
    if "poly" in b:
        e["d"] = path([tuple(map(float, q)) for q in b["poly"]], b.get("corners", True))
        poly_m = np.zeros_like(ink)
        cv2.fillPoly(poly_m, [np.array(b["poly"], np.int32)], 1)
        show(poly_m, (200, 0, 200))
        buttons.append(e)
        continue
    m = pick(b)
    if b.get("circle", True):
        x, y, r = circle_of(m)
        e["circle"] = gcircle(x, y, r)
        cv2.circle(overlay, (int(x), int(y)), int(r), (200, 0, 200), 2)
    else:
        e["d"] = path(contour(m), b.get("corners", False))
        show(m, (200, 0, 200))
    for k in ("label", "icon", "color"):
        if k in b:
            e[k] = b[k]
    buttons.append(e)

if "dpad" in cfg:
    dp = cfg["dpad"]
    cross = fill_holes(region(dp["seed"]) | (region(dp["center"]) if "center" in dp else 0))
    cx, cy = dp.get("pivot") or circle_of(region(dp["center"]))[:2] if "center" in dp else (None, None)
    if cx is None:
        ys, xs = np.nonzero(cross)
        cx, cy = xs.mean(), ys.mean()
    yy, xx = np.mgrid[0:H, 0:W]
    ang = np.degrees(np.arctan2(-(yy - cy), xx - cx))
    arms = {"up": 90, "right": 0, "down": -90, "left": 180}
    for name, a in arms.items():
        diff = (ang - a + 180) % 360 - 180
        arm = (cross & (np.abs(diff) < 45)).astype(np.uint8)
        arm = cv2.morphologyEx(arm, cv2.MORPH_OPEN, np.ones((3, 3), np.uint8))
        buttons.append({"input": name, "d": path(contour(arm), dp.get("corners", True)), "arrow": dp.get("arrows", False)})
        show(arm, (255, 140, 0))
    out["dpad_center"] = list(g(cx, cy))
out["buttons"] = buttons

panels = []
for p in cfg.get("panels", []):
    if "circle" in p:
        x, y, r = p["circle"]
        m = np.zeros_like(ink)
        cv2.circle(m, (int(x), int(y)), int(r), 1, -1)
    elif "poly" in p:
        m = np.zeros_like(ink)
        cv2.fillPoly(m, [np.array(p["poly"], np.int32)], 1)
    else:
        m = pick(p)
    if p.get("hull"):
        # A convex part cut by callouts: the hull of its pieces.
        hull = cv2.convexHull(cv2.findNonZero(m))
        m = np.zeros_like(ink)
        cv2.fillPoly(m, [hull], 1)
    panels.append({"style": p.get("style", "shade"), "d": path(contour(m), p.get("corners", False))})
    show(m, (0, 120, 120))
out["panels"] = panels
if "touchpad" in cfg:
    m = region(cfg["touchpad"])
    out["touchpad"] = path(contour(m))
    show(m, (120, 0, 255))
dots = []
for sd in cfg.get("dots", []) + [("ink", p) for p in cfg.get("ink_dots", [])]:
    if sd[0] == "ink":
        # A filled mark: the ink round the seed.
        m = flood(sd[1], ink.copy())
    else:
        m = region(sd)
    x, y, r = circle_of(m)
    dots.append(gcircle(x, y, r))
    cv2.circle(overlay, (int(x), int(y)), max(2, int(r)), (255, 0, 120), 2)
out["dots"] = dots
for k in ("glyphs", "tint", "stick_rings"):
    if k in cfg:
        out[k] = cfg[k]

json.dump(out, open(sys.argv[2], "w"), indent=1)
cv2.imwrite(sys.argv[3], cv2.cvtColor(overlay, cv2.COLOR_RGB2BGR))
print("scale", round(s, 4), "content", round((x1 - x0) * s, 1), "x", round((y1 - y0) * s, 1))
