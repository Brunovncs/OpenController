"""An 8BitDo front with L4 and R4 peeking above the inner ends of the bumpers, as the triggers
peek above their outer ends: derive_four_front.py FRONT.json BACK.json OUT.json [rise]

Their width and place across come from the rear view (mirrored), their top `rise` units above
the bumper; the part behind the bumper and the body is cut away."""
import json
import sys

import numpy as np

sys.path.insert(0, __file__.replace("\\", "/").rsplit("/", 1)[0])
import geom  # noqa: E402

front = json.load(open(sys.argv[1], encoding="utf-8"))
back = json.load(open(sys.argv[2], encoding="utf-8"))
rise = float(sys.argv[4]) if len(sys.argv) > 4 else 5.0
names = sys.argv[5].split(",") if len(sys.argv) > 5 else ["lp1", "rp1"]
behind = geom.mask(front["body"])
for b in front["bumpers"]:
    behind |= geom.mask(b)
out = json.loads(json.dumps(front))
for name in names:
    bb = next(b for b in back["buttons"] if b["input"] == name)
    if "bumpers" not in front:
        raise SystemExit("no bumpers")
    x0, y0, x1, y1 = geom.box(bb["d"])
    # Seen from the front, the controller's left is on the left again.
    fx0, fx1 = 400 - x1, 400 - x0
    mid = round((fx0 + fx1) / 2 * geom.K)
    col = behind[:, mid]
    top = np.nonzero(col)[0][0] / geom.K
    tab = geom.rect_mask(fx0 + 1, top - rise, fx1 - 1, top + 8, 2.5)
    # Only what shows above the bumper and the body, column by column.
    first = np.where(behind.any(axis=0), behind.argmax(axis=0), behind.shape[0])
    rows = np.arange(behind.shape[0])[:, None]
    tab[rows >= first[None, :]] = 0
    e = {"input": name, "d": geom.to_path(tab)}
    out["buttons"].append(e)
    print(name, "x", round(fx0, 1), round(fx1, 1), "bumper top", top)
for k in ("_T", "dpad_center"):
    out.pop(k, None)
json.dump(out, open(sys.argv[3], "w", encoding="utf-8", newline="\n"), indent=1)
