#!/bin/sh
# Packages a release build for Linux or macOS into dist/, with a .sha256 next to it.
#   packaging/package.sh linux 0.2.0 target/release
#   packaging/package.sh macos 0.2.0 target/aarch64-apple-darwin/release arm64
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
    rm -rf "$name" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$build/open-controller" "$build/open-controller-ui" "$app/Contents/MacOS/"
    sed "s/VERSION/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
    set=$(mktemp -d)/AppIcon.iconset && mkdir -p "$set"
    for s in 16 32 128 256 512; do
        sips -z $s $s "$root/assets/icon-512.png" --out "$set/icon_${s}x${s}.png" >/dev/null
        d=$((s * 2)); [ $d -le 512 ] && sips -z $d $d "$root/assets/icon-512.png" --out "$set/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$set" -o "$app/Contents/Resources/AppIcon.icns"
    # Not notarised: signed ad hoc, so it runs once the quarantine is lifted (right-click, Open).
    codesign --force --deep --sign - "$app"
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
