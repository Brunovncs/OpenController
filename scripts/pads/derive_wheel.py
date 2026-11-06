"""The Logitech G29, seen from the driver's seat as in Logitech's own front view of it
(g29-playstation-front-angle-gallery-8.png on logitechg.com): the rim with its three openings,
the paddles behind it, the plate with the d-pad, the PlayStation buttons, L2, L3, R2 and R3, the
plus and minus rocker, the red dial with its enter button, and Share, Options and PS at the foot of
the hub. Measured on that picture (1200 by 1037 pixels, the rim's centre at 601, 515 and its
radius 374) and drawn by hand, as a black wheel on a black base has no lines to trace.

The wheel turns with the left stick's x, the steering, by up to `steer` degrees (`grip` on the
stick, the panels and the buttons it carries); the base stays put. The brake and the throttle are
two pedals in the bottom corners, filled from below as they are pressed (`levers`).

    python derive_wheel.py out/logitechG29.json
"""
import json
import math
import sys

# The picture's pixels onto the grid: the rim's centre at 200, 140 and its radius 135.
PX, PY, PR = 601, 515, 374
CX, CY = 200, 140
S = 135 / PR
# The rim's inner edge, a circle about the same centre.
INNER = 292


def f(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def g(x, y):
    """A point of the picture on the grid."""
    return CX + (x - PX) * S, CY + (y - PY) * S


def mirror(pts):
    return [(2 * PX - x, y) for x, y in pts]


def poly(points):
    return "M" + "L".join(f"{f(x)} {f(y)}" for x, y in (g(*p) for p in points)) + "Z"


def arc(r, a0, a1, steps):
    """Points of the circle of radius r about the rim's centre, from a0 to a1 degrees (y down)."""
    return [(PX + r * math.cos(math.radians(a)), PY + r * math.sin(math.radians(a)))
            for a in (a0 + (a1 - a0) * i / steps for i in range(steps + 1))]


def angle(x, y):
    return math.degrees(math.atan2(y - PY, x - PX))


def circle(x, y, r, ccw=False):
    """A circle in the picture's pixels as a path on the grid; `ccw` winds it the other way, for
    a hole that stays one whatever the fill rule."""
    x, y = g(x, y)
    r *= S
    k = r * 0.5523
    if ccw:
        return (f"M{f(x + r)} {f(y)}C{f(x + r)} {f(y - k)} {f(x + k)} {f(y - r)} {f(x)} {f(y - r)}"
                f"C{f(x - k)} {f(y - r)} {f(x - r)} {f(y - k)} {f(x - r)} {f(y)}"
                f"C{f(x - r)} {f(y + k)} {f(x - k)} {f(y + r)} {f(x)} {f(y + r)}"
                f"C{f(x + k)} {f(y + r)} {f(x + r)} {f(y + k)} {f(x + r)} {f(y)}Z")
    return (f"M{f(x + r)} {f(y)}C{f(x + r)} {f(y + k)} {f(x + k)} {f(y + r)} {f(x)} {f(y + r)}"
            f"C{f(x - k)} {f(y + r)} {f(x - r)} {f(y + k)} {f(x - r)} {f(y)}"
            f"C{f(x - r)} {f(y - k)} {f(x - k)} {f(y - r)} {f(x)} {f(y - r)}"
            f"C{f(x + k)} {f(y - r)} {f(x + r)} {f(y - k)} {f(x + r)} {f(y)}Z")


def rrect(x, y, w, h, r):
    """A rounded key, centred on x, y in the picture, w by h with corners of r pixels."""
    pts = []
    for cx, cy, a0 in ((x + w / 2 - r, y - h / 2 + r, -90), (x + w / 2 - r, y + h / 2 - r, 0),
                       (x - w / 2 + r, y + h / 2 - r, 90), (x - w / 2 + r, y - h / 2 + r, 180)):
        pts += [(cx + r * math.cos(math.radians(a0 + 90 * i / 4)), cy + r * math.sin(math.radians(a0 + 90 * i / 4)))
                for i in range(5)]
    return poly(pts)


def arms(x, y, s, w):
    """The d-pad's four arms about x, y in the picture, each s long and w wide."""
    h, t = w / 2, w / 2 * 0.9
    shapes = {
        "up": [(x - h, y - s), (x + h, y - s), (x + h, y - t), (x, y - 2), (x - h, y - t)],
        "down": [(x - h, y + s), (x + h, y + s), (x + h, y + t), (x, y + 2), (x - h, y + t)],
        "left": [(x - s, y - h), (x - s, y + h), (x - t, y + h), (x - 2, y), (x - t, y - h)],
        "right": [(x + s, y - h), (x + s, y + h), (x + t, y + h), (x + 2, y), (x + t, y - h)],
    }
    return {k: poly(v) for k, v in shapes.items()}


# The top opening: the rim's inner edge over the top, from the left thumb rest to the right one,
# then back along the plate's top edge and the LED strip in its middle.
top_left = 180 + math.degrees(math.asin((PY - 455) / INNER))
plate_top = [(808, 455), (808, 405), (790, 372), (752, 368), (702, 380), (647, 393), (601, 386)]
plate_top += mirror(plate_top[::-1])[1:]
top_hole = arc(INNER, top_left, 540 - top_left, 48) + plate_top
# The lower openings, between the thumb rest, the pod under the plate and the lower spoke.
low = angle(321.7, 600)
spoke = angle(520, 795.5)
lower_left = [(321.7, 600), (395, 590), (445, 596), (455, 622), (472, 650), (499, 658), (526, 650),
              (548, 630), (552, 700), (540, 745)] + arc(INNER, spoke, low, 20)[:-1]
rim = circle(PX, PY, PR)
holes = [poly(top_hole[::-1]), poly(lower_left[::-1]), poly(mirror(lower_left))]
wheel = rim + "".join(holes)

# The plate: the silver spokes and the lower spoke, the dark wing with the d-pad and the PlayStation
# buttons above them, the hub, the LED strip, the rocker's and the dial's pods.
silver = [(395, 455), (395, 590), (445, 596), (520, 592), (557, 598), (557, 712)]
silver += mirror(silver[::-1])
wing = [(395, 470), (395, 405), (406, 384), (425, 371), (450, 368), (500, 380), (555, 393), (601, 386)]
wing += mirror(wing[::-1])[1:]
wing_bottom = [(808, 470), (740, 476), (680, 470), (601, 466), (522, 470), (462, 476)]
leds = [(548, 403), (575, 397), (601, 395), (627, 397), (654, 403)]
stripe = [(583, 142), (619, 142), (619, 218), (583, 218)]

look = {
    # The base under the wheel, its two feet showing below the rim.
    "body": poly([(258, 648), (944, 648), (952, 660), (952, 765), (940, 772), (940, 885), (845, 885), (842, 772),
                  (360, 772), (357, 885), (262, 885), (262, 772), (250, 765), (250, 660)]),
    "panels": [
        {"style": "line", "d": poly(silver), "grip": True},
        {"style": "shade", "d": poly(wing + wing_bottom), "grip": True},
        {"style": "part", "d": poly(stripe), "grip": True},
        {"style": "line", "d": "M" + "L".join(f"{f(x)} {f(y)}" for x, y in (g(*p) for p in leds)), "grip": True},
        {"style": "part", "d": circle(601, 502, 92), "grip": True},
        {"style": "line", "d": circle(601, 502, 74), "grip": True},
        {"style": "shade", "d": circle(601, 502, 54), "grip": True},
        {"style": "part", "d": circle(460, 435, 40), "grip": True},
        {"style": "part", "d": circle(499, 611, 46), "grip": True},
        {"style": "part", "d": circle(703, 611, 50), "grip": True},
        {"style": "shade", "d": rrect(601, 655, 44, 96, 12), "grip": True},
    ],
    # The brake and the throttle, in the bottom corners.
    "levers": [
        {"input": "lt", "d": "M20 210C20 206 22 204 26 204H46C50 204 52 206 52 210V258C52 262 50 264 46 264H26C22 264 20 262 20 258Z",
         "handle": False},
        {"input": "rt", "d": "M354 198C354 194 356 192 360 192H376C380 192 382 194 382 198V258C382 262 380 264 376 264H360C356 264 354 262 354 258Z",
         "handle": False},
    ],
    "sticks": [{"circle": [CX, CY, 1], "well": 1, "grip": wheel, "steer": 450}],
    "buttons": [
        # The paddles behind the rim, their blades showing above the thumb rests.
        {"input": "lb", "d": poly([(398, 458), (400, 372), (396, 354), (384, 345), (370, 348), (352, 366), (338, 410), (330, 458)]),
         "grip": True},
        {"input": "rb", "d": poly(mirror([(398, 458), (400, 372), (396, 354), (384, 345), (370, 348), (352, 366), (338, 410), (330, 458)])),
         "grip": True},
        {"input": "south", "circle": [*g(743, 457), 16 * S], "grip": True},
        {"input": "east", "circle": [*g(771, 429), 16 * S], "grip": True},
        {"input": "west", "circle": [*g(715, 429), 16 * S], "grip": True},
        {"input": "north", "circle": [*g(743, 401), 16 * S], "grip": True},
        {"input": "misc3", "d": rrect(472, 508, 28, 34, 7), "label": "L2", "grip": True},
        {"input": "misc2", "d": rrect(730, 508, 28, 34, 7), "label": "R2", "grip": True},
        {"input": "lstick", "d": rrect(424, 565, 42, 28, 7), "label": "L3", "grip": True},
        {"input": "rstick", "d": rrect(778, 565, 42, 28, 7), "label": "R3", "grip": True},
        {"input": "misc4", "circle": [*g(499, 592), 17 * S], "icon": "plus", "grip": True},
        {"input": "misc5", "circle": [*g(499, 631), 17 * S], "icon": "minus", "grip": True},
        {"input": "misc6", "circle": [*g(703, 611), 30 * S], "grip": True},
        {"input": "back", "d": rrect(601, 624, 32, 17, 5), "grip": True},
        {"input": "start", "d": rrect(601, 648, 32, 17, 5), "grip": True},
        {"input": "guide", "circle": [*g(601, 680), 15 * S], "icon": "ps", "grip": True},
    ],
    "glyphs": "ps",
}
for name, d in arms(460, 435, 30, 20).items():
    look["buttons"].append({"input": name, "d": d, "grip": True})
for b in look["buttons"]:
    if "circle" in b:
        b["circle"] = [round(v, 2) for v in b["circle"]]
json.dump(look, open(sys.argv[1], "w"), indent=1)
print("logitechG29")
