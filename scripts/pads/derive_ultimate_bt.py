"""The 8BitDo Ultimate Bluetooth: the first Ultimate's shell, with Nintendo's letters on its face
buttons (B at the bottom, A on the right), as the Switch-made model prints them.

    python derive_ultimate_bt.py ../../assets/pads.json out/ultimateBluetooth.json
"""
import json
import sys

bt = json.load(open(sys.argv[1], encoding="utf-8"))["ultimate"]
bt["glyphs"] = "nintendo"
json.dump(bt, open(sys.argv[2], "w", encoding="utf-8"), indent=1)
print("ultimateBluetooth from", sys.argv[1])
