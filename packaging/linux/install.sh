#!/bin/sh
# Installs OpenController for the current user, in ~/.local, and its udev rule, which needs
# your password once. ./install.sh --no-rule skips the rule (the window can install it later).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
id=io.github.brunovncs.open-controller

mkdir -p "$bin" "$prefix/applications" "$prefix/icons/hicolor/512x512/apps" "$prefix/icons/hicolor/scalable/apps"
install -m 0755 "$here/open-controller" "$here/open-controller-ui" "$bin/"
sed "s|^Exec=.*|Exec=$bin/open-controller-ui|" "$here/$id.desktop" > "$prefix/applications/$id.desktop"
install -m 0644 "$here/icon-512.png" "$prefix/icons/hicolor/512x512/apps/$id.png"
install -m 0644 "$here/icon.svg" "$prefix/icons/hicolor/scalable/apps/$id.svg"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/applications" || true
echo "Installed to $bin."

if [ "${1:-}" != "--no-rule" ]; then
    echo "Installing the udev rule (virtual controllers and controller access) needs your password."
    sudo install -m 0644 "$here/70-open-controller.rules" /etc/udev/rules.d/70-open-controller.rules
    echo uinput | sudo tee /etc/modules-load.d/open-controller.conf >/dev/null
    sudo modprobe uinput || true
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    echo "Done. Reconnect your controllers, then open OpenController from your applications."
fi
