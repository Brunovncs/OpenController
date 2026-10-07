#!/bin/sh
# Installs OpenController for the current user, in ~/.local, and its udev rule, which needs
# your password once. ./install.sh --no-rule skips the rule (the window can install it later).
# Run with sudo, it installs for the user who ran sudo.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
id=io.github.brunovncs.open-controller
rules=/etc/udev/rules.d/70-open-controller.rules
modules=/etc/modules-load.d/open-controller.conf

owner=""
if [ "$(id -u)" = 0 ]; then
    if [ -z "${SUDO_USER:-}" ] || [ "$SUDO_USER" = root ]; then
        echo "Run ./install.sh as the user who plays, not as root. It asks for the password when it needs it." >&2
        exit 1
    fi
    home=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    if [ -z "$home" ] || [ ! -d "$home" ]; then
        echo "Could not find the home folder of $SUDO_USER." >&2
        exit 1
    fi
    owner="$SUDO_USER:$(id -gn "$SUDO_USER")"
    prefix="$home/.local/share"
else
    home=$HOME
    prefix="${XDG_DATA_HOME:-$home/.local/share}"
fi
bin="$home/.local/bin"

as_root() {
    if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi
}

# mkdir -p that hands the folders it creates to the user when run through sudo.
make_dir() {
    set -- "$1"
    while [ ! -d "$1" ]; do set -- "$(dirname "$1")" "$@"; done
    shift
    for d in "$@"; do
        mkdir "$d"
        [ -z "$owner" ] || chown "$owner" "$d"
    done
}

put() {
    install -m "$1" "$2" "$3"
    [ -z "$owner" ] || chown "$owner" "$3"
}

for d in "$bin" "$prefix/applications" "$prefix/icons/hicolor/512x512/apps" "$prefix/icons/hicolor/scalable/apps"; do
    make_dir "$d"
done
put 0755 "$here/open-controller" "$bin/open-controller"
put 0755 "$here/open-controller-ui" "$bin/open-controller-ui"
sed "s|^Exec=.*|Exec=$bin/open-controller-ui|" "$here/$id.desktop" > "$prefix/applications/$id.desktop"
[ -z "$owner" ] || chown "$owner" "$prefix/applications/$id.desktop"
put 0644 "$here/icon-512.png" "$prefix/icons/hicolor/512x512/apps/$id.png"
put 0644 "$here/icon.svg" "$prefix/icons/hicolor/scalable/apps/$id.svg"
if [ -z "$owner" ] && command -v update-desktop-database >/dev/null; then
    update-desktop-database "$prefix/applications" || true
fi
echo "Installed to $bin."

# The window needs a few desktop libraries; the resident process needs none of them.
if command -v ldd >/dev/null; then
    missing=$(ldd "$bin/open-controller-ui" 2>/dev/null | awk '/not found/ { print $1 }')
    if [ -n "$missing" ]; then
        echo "The window can't open until these libraries are installed:"
        for lib in $missing; do
            case "$lib" in
                libxkbcommon-x11.so*) echo "  $lib: libxkbcommon-x11-0 on Ubuntu and Debian, libxkbcommon-x11 on Fedora and Arch" ;;
                libxkbcommon.so*) echo "  $lib: libxkbcommon0 on Ubuntu and Debian, libxkbcommon on Fedora and Arch" ;;
                libxcb-xkb.so*) echo "  $lib: libxcb-xkb1 on Ubuntu and Debian, libxcb on Fedora and Arch" ;;
                libxcb.so*) echo "  $lib: libxcb1 on Ubuntu and Debian, libxcb on Fedora and Arch" ;;
                *) echo "  $lib" ;;
            esac
        done
    fi
fi

if [ "${1:-}" != "--no-rule" ]; then
    if [ "$(id -u)" != 0 ]; then
        echo "Installing the udev rule (virtual controllers and controller access) needs your password."
    fi
    if ! { as_root install -m 0644 "$here/70-open-controller.rules" "$rules" &&
        printf 'uinput\n' | as_root tee "$modules" >/dev/null; }; then
        echo "The udev rule was not installed. Run ./install.sh again, or set it up in the window under Settings, Requirements." >&2
        exit 1
    fi
    as_root modprobe uinput 2>/dev/null || true
    if ! { as_root udevadm control --reload-rules && as_root udevadm trigger; }; then
        echo "The udev rule is installed but udev did not reload it. Restart the computer, then open OpenController." >&2
        exit 1
    fi
    if [ ! -c /dev/uinput ]; then
        echo "This system's kernel has no uinput module, so OpenController can't create controllers. This happens on WSL and some custom kernels." >&2
        exit 3
    fi
    echo "Done. Reconnect your controllers, then open OpenController from your applications."
fi
