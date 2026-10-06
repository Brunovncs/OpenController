#!/bin/sh
# Removes what install.sh put in place. Settings stay in ~/.local/share/io.github.brunovncs.open-controller.
set -u
id=io.github.brunovncs.open-controller
prefix="${XDG_DATA_HOME:-$HOME/.local/share}"
"$HOME/.local/bin/open-controller" --quit 2>/dev/null
rm -f "$HOME/.local/bin/open-controller" "$HOME/.local/bin/open-controller-ui" "$prefix/applications/$id.desktop" \
    "$prefix/icons/hicolor/512x512/apps/$id.png" "$prefix/icons/hicolor/scalable/apps/$id.svg" \
    "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/$id.desktop"
if [ -f /etc/udev/rules.d/70-open-controller.rules ]; then
    echo "Removing the udev rule needs your password."
    sudo rm -f /etc/udev/rules.d/70-open-controller.rules /etc/modules-load.d/open-controller.conf
    sudo udevadm control --reload-rules
fi
echo "OpenController is uninstalled."
