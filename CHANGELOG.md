# Changelog

Each release's section here is its release notes on GitHub. It says what changed for someone who
plays with Open Controller, and whether settings carry over.

## 0.2.0

Linux and macOS. On Linux every controller becomes an Xbox 360 controller through the kernel's
uinput, hidden from games with an exclusive grab, with the same window, extra buttons, gyro,
profiles and lights as on Windows; a tarball installs it for the user along with a udev rule. On
macOS, which lets no app create controllers, Open Controller adds keys and macros for extra buttons,
the light bar and the battery to controllers games already read.

Also new on every system: drag one controller onto another to swap players, gyro aiming added to
the right stick (always, while aiming or while a button is held), the touchpad's halves and
two-finger touch as buttons, a stick deadzone and anti-deadzone, profiles that switch with the
program in front, a light bar that blinks below 15 % battery, 268 more known controllers (602 in
all), and on Windows the extra buttons of AYANEO, ZOTAC Zone, OneXPlayer and Legion Go handhelds as
keys and macros. Settings from 0.1.0 carry over.

## 0.1.0

The first release. Any controller SDL 3 reads (PlayStation 3 to 5, Switch, 8BitDo, Steam, Flydigi
and hundreds of generic pads, over USB, Bluetooth or a receiver) becomes an Xbox 360 controller
through ViGEmBus, keeps its player slot across cables and pairings, and can be hidden from games
with HidHide. Extra buttons (back paddles, L4 and R4, Capture, the touchpad click) can press an
Xbox button, hold a key or type a macro, in up to eight profiles per controller, and DualShock 4
and DualSense light bars take the player's colour, a chosen one or the battery's. The window shows
every controller drawn live, and installs the drivers it needs when asked, checked against pinned
hashes. Tested on hardware with an 8BitDo Ultimate 2 Wireless.
