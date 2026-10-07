"""Joins the traced looks into the one geometry file the app and the site draw from:
build_pads.py [--add] OUT.json name=TRACED.json ...

With --add, the looks already in OUT.json are kept and the ones given are added or replaced, so a
new drawing does not need every other look traced again.
"""
import json
import sys

KEYS = ["body", "bumpers", "triggers", "panels", "touchpad", "dots", "sticks", "buttons", "glyphs", "tint", "stick_rings"]
args = sys.argv[1:]
add = args[0] == "--add"
if add:
    args = args[1:]
out = json.load(open(args[0], encoding="utf-8")) if add else {}
for arg in args[1:]:
    name, path = arg.split("=", 1)
    look = json.load(open(path, encoding="utf-8"))
    out[name] = {k: look[k] for k in KEYS if k in look and look[k] not in (None, [], {})}
# One look per line keeps diffs readable.
text = "{\n" + ",\n".join(json.dumps(k) + ":" + json.dumps(v, separators=(",", ":")) for k, v in out.items()) + "\n}\n"
open(args[0], "w", newline="\n", encoding="utf-8").write(text)
print(len(out), "looks,", len(text), "bytes")
