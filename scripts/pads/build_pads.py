"""Joins the traced looks into the one geometry file the app and the site draw from:
build_pads.py OUT.json name=TRACED.json ...
"""
import json
import sys

KEYS = ["body", "bumpers", "triggers", "panels", "touchpad", "dots", "sticks", "buttons", "glyphs", "tint", "stick_rings"]
out = {}
for arg in sys.argv[2:]:
    name, path = arg.split("=", 1)
    look = json.load(open(path))
    out[name] = {k: look[k] for k in KEYS if k in look and look[k] not in (None, [], {})}
text = json.dumps(out, separators=(",", ":"))
# One look per line keeps diffs readable.
text = "{\n" + ",\n".join(json.dumps(k) + ":" + json.dumps(v, separators=(",", ":")) for k, v in out.items()) + "\n}\n"
open(sys.argv[1], "w", newline="\n").write(text)
print(len(out), "looks,", len(text), "bytes")
