"""The small versions of the icon, drawn on a 16-unit grid rather than shrunk from the 1024 one:
the same two pads, an outline behind and a solid one in front, with the front pad larger so it
still reads at 16 to 24 pixels. Writes `assets/icon-small.svg` (on the dark tile, for Explorer
and the taskbar) and `assets/tray-dark.svg` / `assets/tray-light.svg` (no tile, for the
notification area on a dark or a light taskbar)."""

from pathlib import Path

ASSETS = Path(__file__).resolve().parent.parent / "assets"

# The front pad: a top bar with rounded shoulders and two grips.
FRONT = (
    "M6.3 5.6H12.2C13.6 5.6 14.3 6.4 14.6 7.9L15.2 11.6C15.4 13.1 14.1 13.8 13.1 12.9"
    "L11.9 11.6H6.6L5.4 12.9C4.4 13.8 3.1 13.1 3.3 11.6L3.9 7.9C4.2 6.4 4.9 5.6 6.3 5.6Z"
)
# The d-pad and two face buttons, cut out of it.
CUTS = (
    '<rect x="6.35" y="7.3" width="1.3" height="3.4" rx=".3"/>'
    '<rect x="5.3" y="8.35" width="3.4" height="1.3" rx=".3"/>'
    '<circle cx="11.6" cy="8.3" r=".85"/><circle cx="12.7" cy="9.9" r=".85"/>'
)


def svg(front: str, back: str, tile: bool) -> str:
    # The pad behind, 2.4 units up and to the left: only its upper left shows, kept clear of the
    # front pad by a gap, as in the large icon.
    back_pad = f'<g transform="translate(-2.4 -2.6)"><path d="{FRONT}" fill="none" stroke="{back}" stroke-width="1.25" stroke-linejoin="round"/></g>'
    tile_svg = (
        '<defs><linearGradient id="t" x1=".2" y1="0" x2=".8" y2="1"><stop offset="0" stop-color="#272B32"/>'
        '<stop offset="1" stop-color="#0F1013"/></linearGradient></defs>'
        '<rect x=".5" y=".5" width="15" height="15" rx="3.6" fill="url(#t)"/>'
        if tile
        else ""
    )
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">'
        f"{tile_svg}"
        '<mask id="gap" maskUnits="userSpaceOnUse" x="-4" y="-4" width="24" height="24">'
        f'<rect x="-4" y="-4" width="24" height="24" fill="#fff"/><path d="{FRONT}" fill="#000" stroke="#000" stroke-width="2.2" stroke-linejoin="round"/></mask>'
        '<mask id="cut" maskUnits="userSpaceOnUse" x="-4" y="-4" width="24" height="24">'
        f'<rect x="-4" y="-4" width="24" height="24" fill="#fff"/><g fill="#000">{CUTS}</g></mask>'
        # Up half a unit, so the pair sits in the optical centre.
        '<g transform="translate(0 -.45)">'
        f'<g mask="url(#gap)">{back_pad}</g>'
        f'<path d="{FRONT}" fill="{front}" mask="url(#cut)"/>'
        "</g>"
        "</svg>\n"
    )


if __name__ == "__main__":
    for name, front, back, tile in (
        ("icon-small.svg", "#60CDFF", "#8D97A5", True),
        ("tray-dark.svg", "#FFFFFF", "#9AA0A6", False),
        ("tray-light.svg", "#1B1B1F", "#6B7178", False),
    ):
        (ASSETS / name).write_bytes(svg(front, back, tile).encode())
    print("icon-small.svg, tray-dark.svg, tray-light.svg")
