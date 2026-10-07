"""The DualSense Edge: the DualSense's body, with its Fn buttons below the sticks on the lower edge."""
import json
import re
import sys

ds = json.load(open(sys.argv[1]))
nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", ds["body"])]
pts = list(zip(nums[0::2], nums[1::2]))
edge = json.loads(json.dumps(ds))
for side, st in zip(("lp2", "rp2"), ds["sticks"]):
    x, y, r = st["circle"]
    # The body's lower edge right under the stick, between the grips.
    below = [py for px, py in pts if abs(px - x) < 4 and py > y]
    ey = min(below)
    w, h = 15.0, 6.0
    cy = ey - h / 2 - 1.5
    d = f"M{x - w / 2 + 2} {cy - h / 2}H{x + w / 2 - 2}C{x + w / 2} {cy - h / 2} {x + w / 2} {cy + h / 2} {x + w / 2 - 2} {cy + h / 2}H{x - w / 2 + 2}C{x - w / 2} {cy + h / 2} {x - w / 2} {cy - h / 2} {x - w / 2 + 2} {cy - h / 2}Z"
    edge["buttons"].append({"input": side, "d": d})
json.dump(edge, open(sys.argv[2], "w"), indent=1)
print("edge from", sys.argv[1])
