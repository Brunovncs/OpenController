"""Shapes on the 400 x 280 grid as masks: path to mask, mask to path, mirror and move."""
import math
import re

import cv2
import numpy as np

K = 20  # mask pixels per grid unit
GW, GH = 400, 280


def polys(d):
    """The closed outlines of an absolute M/L/H/V/C/Z path, curves flattened."""
    out, cur, x, y, sx, sy = [], [], 0.0, 0.0, 0.0, 0.0
    for cmd, args in re.findall(r"([MLHVCZmlhvcz])([^MLHVCZmlhvcz]*)", d):
        n = [float(v) for v in re.findall(r"-?\d*\.?\d+(?:e-?\d+)?", args)]
        if cmd == "M":
            if cur:
                out.append(cur)
            x, y = n[0], n[1]
            sx, sy = x, y
            cur = [(x, y)]
            for i in range(2, len(n), 2):
                x, y = n[i], n[i + 1]
                cur.append((x, y))
        elif cmd == "L":
            for i in range(0, len(n), 2):
                x, y = n[i], n[i + 1]
                cur.append((x, y))
        elif cmd == "H":
            x = n[-1]
            cur.append((x, y))
        elif cmd == "V":
            y = n[-1]
            cur.append((x, y))
        elif cmd == "C":
            for i in range(0, len(n), 6):
                x1, y1, x2, y2, x3, y3 = n[i:i + 6]
                for t in np.linspace(0, 1, 9)[1:]:
                    a = (1 - t) ** 3 * x + 3 * (1 - t) ** 2 * t * x1 + 3 * (1 - t) * t * t * x2 + t ** 3 * x3
                    b = (1 - t) ** 3 * y + 3 * (1 - t) ** 2 * t * y1 + 3 * (1 - t) * t * t * y2 + t ** 3 * y3
                    cur.append((a, b))
                x, y = x3, y3
        elif cmd in "Zz":
            if cur:
                out.append(cur)
            cur = []
            x, y = sx, sy
    if cur:
        out.append(cur)
    return out


def mask(d):
    m = np.zeros((GH * K, GW * K), np.uint8)
    for p in polys(d):
        cv2.fillPoly(m, [np.round(np.array(p) * K).astype(np.int32)], 1)
    return m


def circle_mask(x, y, r):
    m = np.zeros((GH * K, GW * K), np.uint8)
    cv2.circle(m, (round(x * K), round(y * K)), round(r * K), 1, -1)
    return m


def rect_mask(x0, y0, x1, y1, r=0):
    m = np.zeros((GH * K, GW * K), np.uint8)
    r = min(r, (x1 - x0) / 2, (y1 - y0) / 2)
    a = lambda v: round(v * K)
    cv2.rectangle(m, (a(x0 + r), a(y0)), (a(x1 - r), a(y1)), 1, -1)
    cv2.rectangle(m, (a(x0), a(y0 + r)), (a(x1), a(y1 - r)), 1, -1)
    if r:
        for cx, cy in ((x0 + r, y0 + r), (x1 - r, y0 + r), (x0 + r, y1 - r), (x1 - r, y1 - r)):
            cv2.circle(m, (a(cx), a(cy)), a(r), 1, -1)
    return m


def grow(m, units):
    px = round(abs(units) * K)
    if px == 0:
        return m
    k = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * px + 1, 2 * px + 1))
    return cv2.dilate(m, k) if units > 0 else cv2.erode(m, k)


def _control(prev, p, nxt, sign):
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


def smooth(points):
    """A closed Catmull-Rom path through grid points, as trace.py writes them."""
    pts = [(round(x, 2), round(y, 2)) for x, y in points]
    n = len(pts)
    d = [f"M{pts[0][0]} {pts[0][1]}"]
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        c1 = _control(p0, p1, p2, 1)
        c2 = _control(p3, p2, p1, -1)
        d.append(f"C{c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]} {p2[1]}")
    return "".join(d) + "Z"


def to_path(m, eps=0.06):
    """The largest piece of a mask as a smooth path; eps in grid units."""
    cs, _ = cv2.findContours(m.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    c = max(cs, key=cv2.contourArea)
    c = cv2.approxPolyDP(c, eps * K, True)
    return smooth([(p[0][0] / K, p[0][1] / K) for p in c])


def transform(d, fx):
    """Every coordinate pair of a path through fx(x, y)."""
    out = []
    for cmd, args in re.findall(r"([MLCZ])([^MLCZ]*)", d):
        n = [float(v) for v in re.findall(r"-?\d*\.?\d+(?:e-?\d+)?", args)]
        pairs = [fx(n[i], n[i + 1]) for i in range(0, len(n), 2)]
        out.append(cmd + " ".join(f"{x:.2f} {y:.2f}" for x, y in pairs))
    return "".join(out)


def mirror(d, cx=200.0):
    return transform(d, lambda x, y: (2 * cx - x, y))


def box(d):
    xs, ys = zip(*[p for poly in polys(d) for p in poly])
    return min(xs), min(ys), max(xs), max(ys)
