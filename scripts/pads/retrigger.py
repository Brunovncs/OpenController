"""Rebuilds a look's triggers from its bumpers, as trace.py makes them, with another trigger_rise
and trigger_scale, for a look whose source is not at hand:

    python retrigger.py ../../assets/pads.json dualsense 0.02 0.8 1.6 [dualsenseEdge ...]

The bumpers' points on the grid stand for the traced ones (the grid is a uniform scale of the
trace), and the height the rise is a share of is the body and bumpers' on the grid."""
import json
import math
import re
import sys


def points(d):
    """A path's on-curve points: where it starts and where each segment ends."""
    out = []
    for cmd, args in re.findall(r"([MLC])([^MLCZ]*)", d):
        nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", args)]
        out.append((nums[-2], nums[-1]))
    # The last point closes back on the first.
    if len(out) > 1 and out[-1] == out[0]:
        out.pop()
    return out


def control(prev, p, nxt, sign):
    """As in trace.py."""
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


def path(pts):
    """As in trace.py, on points already on the grid."""
    pts = [(round(x, 2), round(y, 2)) for x, y in pts]
    n = len(pts)
    d = [f"M{pts[0][0]} {pts[0][1]}"]
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        c1 = control(p0, p1, p2, 1)
        c2 = control(p3, p2, p1, -1)
        d.append(f"C{c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]} {p2[1]}")
    return "".join(d) + "Z"


def retrigger(look, rise, sx, sy):
    body = points(look["body"])
    bumpers = [points(b) for b in look["bumpers"]]
    ys = [y for _, y in body + [p for b in bumpers for p in b]]
    lift = rise * (max(ys) - min(ys))
    tr = []
    for pts in bumpers:
        cx = sum(p[0] for p in pts) / len(pts)
        cy = sum(p[1] for p in pts) / len(pts)
        ox = min(p[0] for p in pts) if cx < 200 else max(p[0] for p in pts)
        tr.append(path([(ox + (x - ox) * sx, cy + (y - cy) * sy - lift) for x, y in pts]))
    look["triggers"] = tr


pads_path, rest = sys.argv[1], sys.argv[2:]
keys = [a for a in rest if not re.fullmatch(r"-?\d+\.?\d*", a)]
nums = [float(a) for a in rest if re.fullmatch(r"-?\d+\.?\d*", a)]
rise, sx, sy = nums
pads = json.load(open(pads_path, encoding="utf-8"))
for k in keys:
    retrigger(pads[k], rise, sx, sy)
    print(k, "triggers rebuilt")
# Written as build_pads.py writes it: one look a line.
text = "{\n" + ",\n".join(json.dumps(k) + ":" + json.dumps(v, separators=(",", ":")) for k, v in pads.items()) + "\n}\n"
with open(pads_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(text)
