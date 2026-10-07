#!/bin/sh
# Removes what install.sh put in place. Settings in ~/.local/share/io.github.brunovncs.open-controller
# stay unless you choose to delete them. Run with sudo, it uninstalls for the user who ran sudo.
set -u
id=io.github.brunovncs.open-controller
rules=/etc/udev/rules.d/70-open-controller.rules
modules=/etc/modules-load.d/open-controller.conf

if [ "$(id -u)" = 0 ]; then
    if [ -z "${SUDO_USER:-}" ] || [ "$SUDO_USER" = root ]; then
        echo "Run ./uninstall.sh as the user who installed OpenController, not as root." >&2
        exit 1
    fi
    user=$SUDO_USER
    home=$(getent passwd "$user" | cut -d: -f6)
    prefix="$home/.local/share"
    config="$home/.config"
else
    user=""
    home=$HOME
    prefix="${XDG_DATA_HOME:-$home/.local/share}"
    config="${XDG_CONFIG_HOME:-$home/.config}"
fi
bin="$home/.local/bin"

as_root() {
    if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi
}

if [ -x "$bin/open-controller" ]; then
    if [ -n "$user" ]; then
        sudo -u "$user" XDG_RUNTIME_DIR="/run/user/$(id -u "$user")" "$bin/open-controller" --quit 2>/dev/null
    else
        "$bin/open-controller" --quit 2>/dev/null
    fi
fi
rm -f "$bin/open-controller" "$bin/open-controller-ui" "$prefix/applications/$id.desktop" \
    "$prefix/icons/hicolor/512x512/apps/$id.png" "$prefix/icons/hicolor/scalable/apps/$id.svg" \
    "$config/autostart/$id.desktop"
# Left by versions before 0.8.1, which staged the rule there.
rm -rf "${TMPDIR:-/tmp}/open-controller-setup" 2>/dev/null
# Only the folders that are empty now.
for d in "$bin" "$prefix/applications" "$prefix/icons/hicolor/512x512/apps" "$prefix/icons/hicolor/512x512" \
    "$prefix/icons/hicolor/scalable/apps" "$prefix/icons/hicolor/scalable" "$prefix/icons/hicolor" "$prefix/icons" \
    "$config/autostart"; do
    rmdir "$d" 2>/dev/null
done

status=0
if [ -f "$rules" ] || [ -f "$modules" ]; then
    if [ "$(id -u)" != 0 ]; then
        echo "Removing the udev rule needs your password."
    fi
    if as_root rm -f "$rules" "$modules" && [ ! -f "$rules" ]; then
        if ! { as_root udevadm control --reload-rules && as_root udevadm trigger; }; then
            echo "The udev rule is removed, but udev did not reload. It stops applying after a restart." >&2
            status=1
        fi
    else
        echo "The udev rule is still in $rules. Remove it with: sudo rm $rules $modules" >&2
        status=1
    fi
fi

data="$prefix/$id"
if [ -d "$data" ] && [ -t 0 ]; then
    printf "Do you also want to delete your settings and controller profiles? [y/N] "
    read -r answer
    case "$answer" in
        [yY]*) rm -rf "$data" ;;
    esac
fi
if [ "$status" = 0 ]; then
    echo "OpenController is uninstalled."
else
    echo "OpenController is uninstalled, except for the udev rule."
fi
exit "$status"
