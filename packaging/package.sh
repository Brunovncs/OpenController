#!/bin/sh
# Packages a release build for Linux or macOS into dist/, with a .sha256 next to it.
#   packaging/package.sh linux 0.2.0 target/release
#   packaging/package.sh macos 0.2.0 target/universal/release universal
# MACOS_SIGN_IDENTITY names a code signing identity in the keychain for the macOS app; without it
# the app is signed ad hoc.
set -eu
os=$1 version=$2 arch=${4:-x64}
root=$(cd "$(dirname "$0")/.." && pwd)
# Absolute, since the packaging happens inside dist/.
build=$(cd "$3" && pwd)
mkdir -p "$root/dist"
cd "$root/dist"

case "$os" in
linux)
    name="open-controller-$version-linux-$arch"
    rm -rf "$name" && mkdir "$name"
    cp "$build/open-controller" "$build/open-controller-ui" "$name/"
    "$build/open-controller" --udev-rule > "$name/70-open-controller.rules"
    cp "$root/packaging/linux/"* "$root/assets/icon-512.png" "$root/assets/icon.svg" \
        "$root/README.md" "$root/LICENSE" "$root/THIRD_PARTY_NOTICES.md" "$root/CHANGELOG.md" "$name/"
    chmod +x "$name/install.sh" "$name/uninstall.sh"
    tar -czf "$name.tar.gz" "$name"
    sha256sum "$name.tar.gz" > "$name.tar.gz.sha256"
    ;;
macos)
    name="open-controller-$version-macos-$arch"
    app="$name/OpenController.app"
    rm -rf "$name" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Library/LaunchAgents"
    cp "$build/open-controller" "$build/open-controller-ui" "$app/Contents/MacOS/"
    sed "s/VERSION/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
    cp "$root/packaging/macos/io.github.brunovncs.open-controller.agent.plist" "$app/Contents/Library/LaunchAgents/"
    set=$(mktemp -d)/AppIcon.iconset && mkdir -p "$set"
    for s in 16 32 128 256 512; do
        sips -z $s $s "$root/assets/icon-512.png" --out "$set/icon_${s}x${s}.png" >/dev/null
        d=$((s * 2)); [ $d -le 512 ] && sips -z $d $d "$root/assets/icon-512.png" --out "$set/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$set" -o "$app/Contents/Resources/AppIcon.icns"
    # Not notarised, so macOS blocks the first start until the user allows it in System Settings,
    # Privacy & Security, Open Anyway. The same identity on every release keeps the Accessibility
    # permission across updates; an ad hoc signature is new with every build, and the permission
    # has to be granted again. The resident process is signed first, as code of its own, then the
    # bundle around it.
    identity=${MACOS_SIGN_IDENTITY:--}
    codesign --force --sign "$identity" --identifier io.github.brunovncs.open-controller.resident "$app/Contents/MacOS/open-controller"
    codesign --force --sign "$identity" "$app"
    codesign --verify "$app"
    cp "$root/README.md" "$root/LICENSE" "$root/THIRD_PARTY_NOTICES.md" "$name/"
    ditto -c -k --keepParent "$name" "$name.zip"
    shasum -a 256 "$name.zip" > "$name.zip.sha256"
    ;;
*)
    echo "usage: package.sh linux|macos VERSION BUILD_DIR [ARCH]" >&2
    exit 2
    ;;
esac
echo "dist/$name"
