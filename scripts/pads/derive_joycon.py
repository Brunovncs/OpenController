"""A Joy-Con on its own, held sideways with its rail up: one half of the `joycons` drawing turned a
quarter (the left one anticlockwise, the right one clockwise) and fitted to the grid. SDL reads it
that way: the four buttons by their place once turned, SL and SR as the bumpers, the shoulder
button and trigger on the end as paddles, - or + as Start and Capture or Home as Guide.

    python derive_joycon.py ../../assets/pads.json out/joyconLeft.json out/joyconRight.json
"""
import json
import re
import sys

W, H = 400.0, 280.0
# How much of the grid the turned Joy-Con may take.
FIT_W, FIT_H = 330.0, 200.0

pair = json.load(open(sys.argv[1], encoding="utf-8"))["joycons"]


def turn_path(d, f):
    nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", d)]
    pts = iter(f(x, y) for x, y in zip(nums[0::2], nums[1::2]))
    out = []
    for cmd in re.findall(r"[MCZ][^MCZ]*", d):
        n = {"M": 1, "C": 3, "Z": 0}[cmd[0]]
        out.append(cmd[0] + " ".join(f"{x:.2f} {y:.2f}" for x, y in (next(pts) for _ in range(n))))
    return "".join(out)


def points(d):
    nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", d)]
    return list(zip(nums[0::2], nums[1::2]))


def rrect(x, y, w, h, r):
    k = r * 0.4477
    return (f"M{x + r:.2f} {y:.2f}C{x + w - r:.2f} {y:.2f} {x + w - r:.2f} {y:.2f} {x + w - r:.2f} {y:.2f}"
            f"C{x + w - k:.2f} {y:.2f} {x + w:.2f} {y + k:.2f} {x + w:.2f} {y + r:.2f}"
            f"C{x + w:.2f} {y + h - r:.2f} {x + w:.2f} {y + h - r:.2f} {x + w:.2f} {y + h - r:.2f}"
            f"C{x + w:.2f} {y + h - k:.2f} {x + w - k:.2f} {y + h:.2f} {x + w - r:.2f} {y + h:.2f}"
            f"C{x + r:.2f} {y + h:.2f} {x + r:.2f} {y + h:.2f} {x + r:.2f} {y + h:.2f}"
            f"C{x + k:.2f} {y + h:.2f} {x:.2f} {y + h - k:.2f} {x:.2f} {y + h - r:.2f}"
            f"C{x:.2f} {y + r:.2f} {x:.2f} {y + r:.2f} {x:.2f} {y + r:.2f}"
            f"C{x:.2f} {y + k:.2f} {x + k:.2f} {y:.2f} {x + r:.2f} {y:.2f}Z")


bodies = ["M" + s for s in pair["body"].split("M")[1:]]


def side(right):
    body = bodies[1] if right else bodies[0]
    near = (lambda x: x > 200) if right else (lambda x: x < 200)
    ends = [pair["bumpers"][right], pair["triggers"][right]]
    every = points(body) + [p for d in ends for p in points(d)]
    xs, ys = [p[0] for p in every], [p[1] for p in every]
    cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
    # Turned: the left one anticlockwise, so its rail (its right side) is on top; the right one
    # clockwise, so its rail (its left side) is.
    if right:
        def rot(x, y):
            return cx - (y - cy), cy + (x - cx)
    else:
        def rot(x, y):
            return cx + (y - cy), cy - (x - cx)
    turned = [rot(x, y) for x, y in every]
    tx, ty = [p[0] for p in turned], [p[1] for p in turned]
    k = min(FIT_W / (max(tx) - min(tx)), FIT_H / (max(ty) - min(ty)))
    ox, oy = W / 2 - k * (min(tx) + max(tx)) / 2, H / 2 - k * (min(ty) + max(ty)) / 2

    def f(x, y):
        rx, ry = rot(x, y)
        return ox + k * rx, oy + k * ry

    def circle(c):
        x, y = f(c[0], c[1])
        return [round(x, 2), round(y, 2), round(c[2] * k, 2)]

    look = {"body": turn_path(body, f), "glyphs": "nintendo"}
    body_pts = [f(x, y) for x, y in points(body)]
    left, right_x = min(p[0] for p in body_pts), max(p[0] for p in body_pts)
    top = min(p[1] for p in body_pts)
    length = right_x - left
    # SL and SR on the rail, near its ends.
    look["bumpers"] = [rrect(left + length * f0, top - 5, length * 0.16, 9, 4) for f0 in (0.2, 0.64)]
    stick = next(s for s in pair["sticks"] if near(s["circle"][0]))
    look["sticks"] = [{"circle": circle(stick["circle"]), "well": round(stick["well"] * k, 2)}]
    # The end's shoulder button and trigger, as SDL gives them: paddles.
    paddles = ("rp1", "rp2") if right else ("lp1", "lp2")
    look["buttons"] = [{"input": p, "d": turn_path(d, f)} for p, d in zip(paddles, ends)]
    # Each button by where it ends up once turned, with what is printed on it: X, A, B and Y go
    # round a quarter clockwise, the arrows a quarter the other way and point where they now do.
    if right:
        faces = {"north": ("east", "X"), "east": ("south", "A"), "south": ("west", "B"), "west": ("north", "Y")}
        small = {"start": "start", "guide": "guide"}
    else:
        faces = {"up": ("west", "◀"), "right": ("north", "▲"), "down": ("east", "▶"), "left": ("south", "▼")}
        small = {"back": "start", "misc1": "guide"}
    for b in pair["buttons"]:
        was = b["input"]
        if was in faces:
            now, label = faces[was]
            look["buttons"].append({"input": now, "circle": circle(b["circle"]), "label": label})
        elif was in small:
            nb = {"input": small[was]}
            if b.get("circle"):
                nb["circle"] = circle(b["circle"])
            else:
                nb["d"] = turn_path(b["d"], f)
            if b.get("icon"):
                nb["icon"] = b["icon"]
            look["buttons"].append(nb)
    return look


json.dump(side(False), open(sys.argv[2], "w", encoding="utf-8"), indent=1, ensure_ascii=False)
json.dump(side(True), open(sys.argv[3], "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print("joyconLeft and joyconRight from the joycons drawing")
