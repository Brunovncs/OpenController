# Changelog

Each release's section here is its release notes on GitHub. It says what changed for someone who
plays with OpenController, and whether settings carry over.

## 0.7.0

A controller on its receiver no longer shows up three times. Windows sometimes lists an Xbox-style
controller a second way while it connects, and those copies appeared as extra controllers with no
button layout. Receivers that only say the controller is plugged in no longer show it as charging
at 100%. The window has a new layout: everything is centered, the controllers fill the width of
the window, players are listed in order, the battery reads 80% with a bolt while charging, and
connecting a controller is a button at the top. Settings that a newer version wrote can no longer
wipe your profiles when you go back to an older one; what can be read is kept, and the original
file is saved next to it as settings.json.bad. Connecting a controller in the middle of a game no
longer makes the others freeze for a moment: creating its Xbox controller and hiding the original
now happen apart from everyone's input. Settings carry over.

## 0.6.1

Updating or uninstalling right after OpenController opens works every time. A copy that was
still starting could miss the request to quit, so the uninstaller left the program behind and an
update could fail to replace it. Settings carry over.

## 0.6.0

Pick the language in Settings, English or Portuguese. OpenController starts in English and keeps
your choice across updates, so after updating it opens in English once until you pick again.
Uninstalling now asks whether to delete your settings and controller profiles too; choose No to
keep them for a future installation. Settings carry over.

## 0.5.0

The name is now written as one word, OpenController, in the window, the installer and the
shortcuts. The old Start menu shortcut is replaced and "Start with Windows" keeps working. The
window shows its version at the top, and the settings have a back button. Settings carry over.

## 0.4.1

"Update now" works. Until now the window closed before it could start the installer, so the
update was downloaded but never installed. From 0.3.0 or 0.4.0, download this version from the
website or this page once; from here on the button installs the next one by itself. Settings
carry over.

## 0.4.0

Controllers look like themselves. The window draws each one in its own shape, with its controls
where the real one has them: the DualSense with its touchpad and light strips, the DualShock 4,
the Xbox and Switch Pro controllers, Joy-Cons, the 8BitDo Ultimate with its star and the 8BitDo
SN30 Pro, among others. The icons are drawn instead of taken from a Windows font, so they show
on Linux and macOS too. Settings from 0.3.0 carry over.

## 0.3.0

A Windows installer, and updates. `open-controller-0.3.0-windows-x64-setup.exe` installs for your
user without administrator rights, appears in the Start menu and in Apps, where it uninstalls, and
updates an existing installation in place (including one made with `install.ps1`), keeping your
settings. When a newer version is out, the window says so in a bar at the top: on Windows "Update
now" downloads the installer, checks it and runs it; on Linux and macOS it opens the download page.
The check asks GitHub once when the window opens and can be turned off in Settings. The zip stays
for running without installing. Settings from 0.2.0 carry over.

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
