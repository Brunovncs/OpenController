"""The two drawings that stand for many controllers: a neutral pad from the Ultimate 2C's outline,
and a handheld PC drawn by hand in the same terms."""
import json
import sys

u2c = json.load(open(sys.argv[1]))
generic = json.loads(json.dumps(u2c))
keep = []
for b in generic["buttons"]:
    if b["input"] == "back":
        b["icon"] = "view"
    elif b["input"] == "start":
        b["icon"] = "menu"
    elif b["input"] == "guide":
        b["icon"] = "ring"
    elif b["input"] in ("none", "misc1"):
        continue
    keep.append(b)
generic["buttons"] = keep
generic["dots"] = []
json.dump(generic, open(sys.argv[2], "w"), indent=1)


def rrect(x, y, w, h, r):
    k = r * 0.4477
    return (f"M{x + r} {y}H{x + w - r}C{x + w - k} {y} {x + w} {y + k} {x + w} {y + r}V{y + h - r}"
            f"C{x + w} {y + h - k} {x + w - k} {y + h} {x + w - r} {y + h}H{x + r}C{x + k} {y + h} {x} {y + h - k} {x} {y + h - r}"
            f"V{y + r}C{x} {y + k} {x + k} {y} {x + r} {y}Z")


def arms(cx, cy, s, w):
    h = w / 2
    t = h * 0.9
    return {
        "up": f"M{cx - h} {cy - s}H{cx + h}V{cy - t}L{cx} {cy - 1}L{cx - h} {cy - t}Z",
        "down": f"M{cx - h} {cy + s}H{cx + h}V{cy + t}L{cx} {cy + 1}L{cx - h} {cy + t}Z",
        "left": f"M{cx - s} {cy - h}V{cy + h}H{cx - t}L{cx - 1} {cy}L{cx - t} {cy - h}Z",
        "right": f"M{cx + s} {cy - h}V{cy + h}H{cx + t}L{cx + 1} {cy}L{cx + t} {cy - h}Z",
    }


handheld = {
    "body": rrect(14, 78, 372, 150, 46),
    "bumpers": [rrect(38, 70, 70, 12, 6), rrect(292, 70, 70, 12, 6)],
    "triggers": [rrect(46, 63, 52, 11, 5), rrect(302, 63, 52, 11, 5)],
    "panels": [{"style": "part", "d": rrect(104, 88, 192, 130, 7)}],
    "sticks": [{"circle": [58, 116, 13], "well": 17}, {"circle": [342, 186, 13], "well": 17}],
    "buttons": [
        {"input": "south", "circle": [342, 132, 7.2]},
        {"input": "east", "circle": [356, 118, 7.2]},
        {"input": "west", "circle": [328, 118, 7.2]},
        {"input": "north", "circle": [342, 104, 7.2]},
        {"input": "back", "circle": [84, 96, 4.6], "icon": "view"},
        {"input": "start", "circle": [316, 96, 4.6], "icon": "menu"},
    ],
    "glyphs": "xbox",
    "tint": True,
}
for name, d in arms(58, 184, 15, 9.5).items():
    handheld["buttons"].append({"input": name, "d": d})
json.dump(handheld, open(sys.argv[3], "w"), indent=1)
print("generic and handheld")
