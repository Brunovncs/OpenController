"""The Logitech Extreme 3D Pro, seen from above as in Logitech's own top view of it
(3dpro-gallery-3.png on logitechg.com): the base with its six buttons round the stick and the
throttle at the front, and the grip with the hat, buttons 3 to 6, the trigger at its tip and the
thumb button on its side. Measured on that picture and drawn by hand, as the photo has no lines to
trace. The grip moves with the stick and turns with its twist (`grip` on the stick, `grip` on the
buttons it carries); the throttle fills from the back as it is pushed forward (`levers`).

    python derive_flightstick.py out/logitechExtreme3d.json
"""
import json
import math
import re
import sys

# The stick's pivot on the grid, where the shaft meets the base.
CX, CY = 200, 161
GRIP_DROP = 6


def rrect(x, y, w, h, r):
    k = r * 0.4477
    return (f"M{x + r} {y}H{x + w - r}C{x + w - k} {y} {x + w} {y + k} {x + w} {y + r}V{y + h - r}"
            f"C{x + w} {y + h - k} {x + w - k} {y + h} {x + w - r} {y + h}H{x + r}C{x + k} {y + h} {x} {y + h - k} {x} {y + h - r}"
            f"V{y + r}C{x} {y + k} {x + k} {y} {x + r} {y}Z")


def f(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def poly(points):
    return "M" + "L".join(f"{f(x)} {f(y)}" for x, y in points) + "Z"


def sector(r0, r1, a0, a1, steps=6):
    """A key on the arc round the stick, between two radii and two angles (degrees, y down)."""
    outer = [(CX + r1 * math.cos(math.radians(a)), CY + r1 * math.sin(math.radians(a)))
             for a in (a0 + (a1 - a0) * i / steps for i in range(steps + 1))]
    inner = [(CX + r0 * math.cos(math.radians(a)), CY + r0 * math.sin(math.radians(a)))
             for a in (a1 - (a1 - a0) * i / steps for i in range(steps + 1))]
    return poly(outer + inner)


def pill(cx, cy, w, h, turn):
    """A rounded key of w by h turned by `turn` degrees about its centre, as a polygon."""
    pts = []
    r = w / 2
    for end, sign in ((-1, -1), (1, 1)):
        oy = end * (h / 2 - r)
        for i in range(7):
            a = math.radians(180 + 180 * i / 6) if sign < 0 else math.radians(180 * i / 6)
            pts.append((r * math.cos(a), oy + r * math.sin(a)))
    t = math.radians(turn)
    return poly([(cx + x * math.cos(t) - y * math.sin(t), cy + x * math.sin(t) + y * math.cos(t)) for x, y in pts])


def lower(d, dy):
    """An absolute path of M, L, H, V, C and Z moved down by dy."""
    out = []
    for cmd, args in re.findall(r"([MLHVCZ])([^MLHVCZ]*)", d):
        n = [float(v) for v in re.findall(r"-?\d*\.?\d+", args)]
        if cmd in "MLC":
            n = [v + dy if i % 2 else v for i, v in enumerate(n)]
        elif cmd == "V":
            n = [v + dy for v in n]
        out.append(cmd + " ".join(f(v) for v in n))
    return "".join(out)


def arms(cx, cy, s, w):
    h = w / 2
    t = h * 0.9
    return {
        "up": f"M{cx - h} {cy - s}H{cx + h}V{cy - t}L{cx} {cy - 1}L{cx - h} {cy - t}Z",
        "down": f"M{cx - h} {cy + s}H{cx + h}V{cy + t}L{cx} {cy + 1}L{cx - h} {cy + t}Z",
        "left": f"M{cx - s} {cy - h}V{cy + h}H{cx - t}L{cx - 1} {cy}L{cx - t} {cy - h}Z",
        "right": f"M{cx + s} {cy - h}V{cy + h}H{cx + t}L{cx + 1} {cy}L{cx + t} {cy - h}Z",
    }


# The base: a cushioned frame with a foot at each corner, cut short at the corners.
body = ("M108 38H292C306 38 316 42 324 50C332 58 336 68 336 82V232C336 246 332 256 324 264C316 272 306 276 292 276"
        "H108C94 276 84 272 76 264C68 256 64 246 64 232V82C64 68 68 58 76 50C84 42 94 38 108 38Z")
# The grip from above: the head with the hat and four buttons, the neck and the palm rest.
grip = ("M200 12C214 12 226 18 231 34C236 52 236 84 232 100C229 112 224 120 222 132C220 150 226 168 226 186"
        "C226 204 214 214 200 214C186 214 174 204 174 186C174 168 180 150 178 132C176 120 171 112 168 100"
        "C164 84 164 52 169 34C174 18 186 12 200 12Z")

look = {
    "body": body,
    "panels": [
        # The silver deck inside the rubber frame.
        {"style": "line", "d": ("M118 58H282C298 58 316 70 316 90V226C316 244 300 256 282 256H118C100 256 84 244 84 226"
                                "V90C84 70 102 58 118 58Z")},
        # The four feet's openings.
        {"style": "shade", "d": pill(86, 60, 9, 34, 45)},
        {"style": "shade", "d": pill(314, 60, 9, 34, -45)},
        {"style": "shade", "d": pill(86, 254, 9, 34, -45)},
        {"style": "shade", "d": pill(314, 254, 9, 34, 45)},
        # The throttle's slot, + at the front.
        {"style": "part", "d": rrect(160, 224, 36, 44, 8)},
    ],
    "levers": [{"input": "rt", "d": rrect(166, 228, 24, 36, 6)}],
    "sticks": [{"circle": [CX, CY, 15], "well": 44, "grip": grip, "twist": 25}],
    "buttons": [
        # On the base, in two arcs round the stick: 7, 9 and 11 outside, 8, 10 and 12 inside.
        {"input": "back", "d": sector(73, 90, 214, 238), "label": "7"},
        {"input": "start", "d": sector(52, 68, 216, 236), "label": "8"},
        {"input": "lstick", "d": sector(73, 90, 188, 212), "label": "9"},
        {"input": "rstick", "d": sector(52, 68, 190, 212), "label": "10"},
        {"input": "lt", "d": sector(73, 90, 158, 184), "label": "11"},
        {"input": "misc1", "d": sector(52, 68, 160, 184), "label": "12"},
        # On the grip: the trigger at its tip, the thumb button on its left, 3 and 4 below the hat
        # in a V, 5 and 6 either side of it.
        {"input": "south", "d": rrect(192, 4, 16, 12, 5), "label": "1", "grip": True},
        {"input": "east", "circle": [170, 106, 5.5], "label": "2", "grip": True},
        {"input": "west", "d": pill(191, 90, 7, 17, -38), "label": "3", "grip": True},
        {"input": "north", "d": pill(209, 90, 7, 17, 38), "label": "4", "grip": True},
        {"input": "lb", "d": pill(182, 66, 7, 16, 0), "label": "5", "grip": True},
        {"input": "rb", "d": pill(218, 66, 7, 16, 0), "label": "6", "grip": True},
    ],
    "glyphs": "numbers",
}
for name, d in arms(200, 68, 11, 7).items():
    look["buttons"].append({"input": name, "d": d, "grip": True})
# The grip sits a little lower than measured, so its tip stays in the picture when pushed forward.
look["sticks"][0]["grip"] = lower(grip, GRIP_DROP)
for b in look["buttons"]:
    if b.get("grip"):
        if "d" in b:
            b["d"] = lower(b["d"], GRIP_DROP)
        else:
            b["circle"][1] += GRIP_DROP
json.dump(look, open(sys.argv[1], "w"), indent=1)
print("logitechExtreme3d")
