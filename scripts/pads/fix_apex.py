"""Apex 5 rear: the paddles the manual draws beside their slots, moved up into them, and the left
one (under the blue arrow in the manual) as the mirror of the right: fix_apex.py IN OUT DY"""
import json
import sys

sys.path.insert(0, __file__.replace("\\", "/").rsplit("/", 1)[0])
import geom  # noqa: E402

p = json.load(open(sys.argv[1], encoding="utf-8"))
dy = float(sys.argv[3])
x0, _, x1, _ = geom.box(p["body"])
cx = (x0 + x1) / 2
rp2 = next(b for b in p["buttons"] if b["input"] == "rp2")
rp2["d"] = geom.transform(rp2["d"], lambda x, y: (x, y - dy))
p["buttons"].insert(p["buttons"].index(rp2) + 1, {"input": "lp2", "d": geom.mirror(rp2["d"], cx), "label": "M4"})
json.dump(p, open(sys.argv[2], "w", encoding="utf-8", newline="\n"), indent=1)
print("centre", round(cx, 2))
