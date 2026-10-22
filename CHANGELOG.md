# Changelog

Each release's section here is its release notes on GitHub. It says what changed for someone who
plays with OpenController, and whether settings carry over.

## 0.8.11

PC pads that show up as Microntek USB Joystick (0079:0006), like the KTS ones shaped as a
DualShock, now work as controllers. Before, they showed up as not mapped.

A controller whose buttons OpenController cannot read can now be mapped on its page. Map the
buttons asks for each part in turn, lights it on the drawing, and the controller then works as an
Xbox controller. Map again, under Information, starts over. A controller like this also shows its
model's name and drawing when its ids are known.

Settings carry over.

## 0.8.10

The Logitech Extreme 3D Pro now has its own drawing, with the grip leaning and turning as you
move it and the throttle filling as you push it. It is left to flight games as it is. To play a
gamepad game with it, turn off Keep native on its page and it becomes an Xbox controller: the
stick and its twist as the sticks, the throttle as the right trigger, the trigger and the thumb
button as A and B.

Flight sticks, throttles and racing wheels are no longer turned into Xbox controllers on their
own, so flight and racing games keep reading them with their twist, throttle and force feedback.
One that OpenController does not know says so on its page, with a way to send its details.

A report on a controller OpenController does not know now records the buttons, axes and hats it
sends, and names the Linux drivers as they are.

Settings carry over.

## 0.8.9

Updating HidHide now warns that its installer asks to restart Windows and opens again afterwards
to finish. That second window is expected.

Twin USB Joystick adapters that report the id 0815:0005 now work as controllers. Before, they
showed up as not mapped.

Windows on ARM gets its own build, and an update moves an ARM PC to it. HidHide has no ARM
version yet, so there the original controller can't be hidden. Linux also gets an ARM build, and
macOS is now one app for both Apple Silicon and Intel Macs.

The Windows build no longer needs the Visual C++ runtime installed to start.

If ViGEmBus stops or is updated while OpenController runs, the controllers come back on their
own once it is there again. Before, they stayed gone until OpenController was restarted. Adding a
controller over VIIPER that fails no longer freezes the other controllers for seconds.

With more than one Windows user signed in, only the one at the screen gets the controllers.

The window no longer closes when OpenController stops unexpectedly: it waits and connects again.
If the window and OpenController are different versions, it says so and offers to restart.
Renaming a profile takes accents, paste and the cursor keys like any other text box.

A macro now plays without holding up the keys of other buttons, and pressing it again while it
plays does nothing. Two buttons holding shortcuts that share a key, like Ctrl+C and Ctrl+V, no
longer let go of Ctrl when one of them is released. On Linux, keys from extra buttons work as
soon as device access is set up, without signing out first.

On macOS, profiles now switch with the program in front, and Start at login uses Login Items.
The Mac app now keeps the same signature across updates, so the Accessibility permission carries
over from here on. Updating to this version asks for it once more.
On Linux, Start with the system restarts OpenController if it stops, where the desktop allows it.
On a Steam Deck, setting up device access says to set a password first.

When OpenController quits, it leaves HidHide on if another program is still hiding a controller
with it. Uninstalling keeps your settings if a hidden controller could not be shown again. A
crash is written to crash.log in the settings folder, and the hidden controllers come back.

Settings are safer. If the file can't be read at startup, it is no longer replaced by the
defaults. A value this version can't read costs only that value, not the controller's profiles,
and settings from a newer version are kept when an older one saves. Settings carry over.

## 0.8.8

Joy-Cons can now be two controllers instead of one. On a pair's page, Split Joy-Cons makes each
Joy-Con a controller of its own, held sideways: the stick on the left, the four buttons on the
right, SL and SR as the bumpers, and the shoulder button and trigger on the end free to assign.
Join Joy-Cons puts them back together. The left Joy-Con keeps the pair's player, so games see no
controller leave. This is experimental.

A face button, bumper or any other regular button set to a key or a macro now types it. Since
0.8.6 it did nothing. Two controllers can no longer swap players when one is shown to games as a
DualShock 4 and the other as an Xbox controller, as each ended up as the other kind. When another
program held HidHide at the wrong moment, a controller could stay visible to games twice, or
hidden. OpenController now tries again until it works.

Updating from the window and installing drivers no longer fail on a slow connection. Start with
Windows now shows as off when it was turned off in Task Manager, and turning it on here works
again. Settings that an antivirus held for a moment at sign-in are no longer replaced by the
defaults. Report a problem no longer includes the address of a Bluetooth LE controller. The window
does less work while a game is in front of it. Settings carry over.

## 0.8.7

Sticks get more settings. Full tilt at makes a worn stick that no longer reaches its edge count as
pushed all the way, and the response curve slows the middle of the travel for fine aiming, or
speeds it up. Every stick setting also takes any value typed in, not only the ones offered. Each
stick can have its own settings, such as a precise curve on the right one only, and the page shows
live what games get from each stick. Measure finds where a drifting stick rests and how far it
reaches, then sets its centre, deadzone and full tilt from that, so a stick that always leans the
same way no longer needs a large deadzone. Settings carry over. If you go back to 0.8.6, both
sticks keep the left one's deadzones.

## 0.8.6

Every button can now be remapped, not only the extra ones. On a controller's Buttons page, All
buttons lists the face buttons, bumpers, stick clicks, d-pad and the middle buttons, and each can
become another Xbox button, a key or a macro. Games then stop seeing it as itself. It is kept per
profile, so a profile tied to a game gives that game its own layout without Steam. On a Switch
controller, Use the controller's letters makes the B at the bottom B in games, as printed. A tile
says Remapped while its profile changes one of these buttons, and Restore defaults puts everything
back. A touchpad click given something else no longer clicks the touchpad of a virtual DualShock 4
too. Settings carry over. If you go back to 0.8.5, it keeps them but presses both buttons.

## 0.8.5

The DualShock 3 gets more out of DsHidMini's SXS mode, all of it experimental. Its
pressure-sensitive buttons can move the triggers, as on the PlayStation 2 and 3: on its Buttons
page, pick which button moves LT and which moves RT, such as Cross to accelerate in a racing game.
Its gyro can now aim too. It only turns sideways, so tilting up and down comes from the
accelerometer. If something reads wrong, Report a problem on its Information page sends what the
controller measured.

On Windows, a controller can now be shown to games as a DualShock 4 instead of an Xbox controller,
so Steam Input and games that read a gyro get its motion sensors, the DualShock 3's included. Games
that only know Xbox controllers don't see it, so it stays off until you turn it on, under
Information or Motion. This is experimental too.

Any controller can also have a stick press the d-pad, for games played on it. It is under Sticks.
Settings carry over.

## 0.8.4

The LeadJoy Xeno Pro and Xeno Plus are now known by name and drawn as themselves. In Switch mode
the Xeno Pro reports itself as a Switch Pro Controller, so pick it under Model on its Information
page. Settings carry over.

## 0.8.3

Some controllers copy another one's USB id, so OpenController can't tell them apart. A controller
that does can now be told which model it is, under Model on its Information page, and from then
on it shows with its own name and drawing. The Onikuma C1, which reports itself as a Switch Pro
Controller, is the first one listed.

Reporting a problem with a controller that has a gyro now also sends what the gyro does while
the report is open, so a gyro that doesn't aim can be looked into without the controller in hand.
Turn the controller around a bit before pressing Send. Settings carry over.

## 0.8.2

Each controller can now be kept native from its page. Games then see the real controller, with
its own button prompts, adaptive triggers and, over the cable, haptic feedback. This also lets
OpenController work alongside a DS5Dongle. Profiles, gyro and remapping are off while a controller
stays native. Close the game before switching.

Gyro aiming has finer tuning: its own sensitivity while aiming with LT, a separate up and down
speed, a button that pauses it while held, and a press to turn it on and off instead of holding.
Under Advanced there is acceleration for fast turns, steadying for slow movements and an
adjustable anti-deadzone.

On Windows, Settings has a new Advanced section where the virtual controllers can be made with
VIIPER instead of ViGEmBus. It is experimental and off by default, and ViGEmBus stays the
recommended choice. Trying the other one can help if a game doesn't see your controller or your
PC gets blue screens. With VIIPER chosen, Requirements installs usbip-win2 and the VIIPER server.
If VIIPER does not start, OpenController goes back to ViGEmBus and tells you.

Linux got a round of fixes. A DualSense connected after OpenController starts, as over Bluetooth,
is now hidden from games. On GNOME and other Wayland desktops that leave the frame to the app, the
window has its own title bar to move, resize, maximize and close it. Players are numbered past
four. The window says when the kernel has no uinput, as on WSL, and when it couldn't ask for the
password. install.sh and uninstall.sh say when something failed, work under sudo and list missing
libraries. Setting up device access no longer writes a file to /tmp first. DualSense buttons are
right when read through their event node. The dock shows the right icon, a window that fails to
open says so, and quitting leaves no files behind.

On macOS, closing the window now ends it, as on Windows and Linux. Only the small part that keeps
your controllers working stays in the background. OpenController's own files on macOS moved to a
shorter folder, since the long one could keep it from starting. A controller slow to take rumble
no longer freezes the others. Settings carry over.

## 0.8.0

A controller that misbehaves, or that OpenController does not know, can now be reported from the
window. Report a problem, on its Information page, sends its technical details so it can be added
or fixed without having it in hand. You see everything before it is sent, and you can add the
model and what goes wrong. Each controller now shows how sure OpenController is that it works:
verified, compatible, works with a note, or not on the list yet. 320 more controllers are
recognised by name: GameSir, PowerA, Turtle Beach, Razer, SCUF, Nacon, HORI, Redragon, EasySMX
and MSI pads, the Atari VCS controllers, the Sega and NeoGeo mini consoles' pads, old Logitech,
Saitek, Thrustmaster and SideWinder pads, the original Xbox controller, and adapters for
PlayStation, GameCube, N64, Saturn and Wii controllers. 22 controllers now have a drawing of their
own, traced like the others from pictures of the real thing: the Xbox 360 controller and the original
Xbox Controller S, GameSir G7 SE, G7 Pro, Nova Lite and Cyclone 2, Flydigi Vader and Apex, Redragon
Harrow, Saturn and Darkflame G820, Razer Wolverine V3, Victrix Pro BFG, Nacon Revolution 5 Pro,
Turtle Beach Stealth Ultra, Logitech F310, HORI Fighting Commander OCTA, Atari VCS, 8BitDo 64, the
Sega Genesis and Mega Drive pads and the PlayStation Classic controller. Their buttons light up as
they are pressed, white and black, C and Z included. Settings carry over.

## 0.7.1

Every controller is drawn as it really looks. The drawings are traced from each maker's own
pictures, so a DualSense, a DualShock 4, an 8BitDo Ultimate 2 or a Switch Pro Controller has its
real outline and its buttons where they really are. The 8BitDo Ultimate 2C and the first Ultimate
have drawings of their own. Settings carry over.

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
