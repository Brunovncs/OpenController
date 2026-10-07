# Where the controller table's ids come from

Most rows of `crates/open-controller-core/src/models.rs` come from SDL's own lists. The block
marked "From OpenController's own research" adds the controllers SDL does not name: pads sold
today in Brazil, the US and Europe (GameSir, PowerA, Turtle Beach, Razer, SCUF, Nacon, HORI,
8BitDo, Redragon, EasySMX, MSI, Leadership, Havit), retro and old PC pads (Atari, Sega and SNK
minis, Logitech, Saitek, Thrustmaster, SideWinder, Gravis, the original Xbox controller) and
adapters for old consoles' controllers.

`sources.json` lists, for each of those ids, the model or models it belongs to, the name the
device reports, and the sources that tie the id to the model: SDL_GameControllerDB (the GUID holds
the ids), Linux's `xpad.c` and `hid-ids.h`, RetroArch's joypad autoconfig files, MiSTer's latency
catalogue, usb.ids, and users' own logs in GitHub issues and forums. Only ids marked confirmed or
likely there made it into the table. `left_out` says which ids were found and left out, and why:
flight sticks, wheels, and chip makers' ids that unrelated devices share.

`LAYOUTS` in the same file draws a pad whose family does not say its shape: a generic pad with
its sticks side by side like a PlayStation one, or a pad with no sticks at all.

Many cheap pads copy someone else's ids: Xbox 360 clones are 045e:028e, PlayStation 4 clones
054c:05c4 or 09cc (the Redragon Darkflame G820 among them), basic PC pads 0079:0006 or 0810:0001.
They cannot be told apart from the original by id, and get the original's row.

A controller that is not in the table can be sent from the window: Report a problem, on its
Information page, sends its ids, its SDL mapping and its place in the device tree to the
maintainer. That is what it takes to add a row here.
