#!/bin/sh
# Builds the macOS release files into dist/:
#   NickelInkSync-<version>-macos.dmg   the desktop client as a universal
#                                       .app (arm64 + x86_64, macOS 11+),
#                                       with kepubify inside
#   nickelinksync-<version>-macos.zip   the CLI, universal, with the licence
# Needs Xcode's command line tools (lipo, iconutil, hdiutil, codesign),
# rsvg-convert (`brew install librsvg`), both Rust targets
# (`rustup target add aarch64-apple-darwin x86_64-apple-darwin`) and curl.
# Not notarized: no Apple Developer ID (see the README on Gatekeeper).
set -eu

cd "$(dirname "$0")/../.."
root=$(pwd)

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
gui_version=$(sed -n 's/^version = "\(.*\)"/\1/p' gui/Cargo.toml | head -1)
if [ "$version" != "$gui_version" ]; then
  echo "Cargo.toml ($version) and gui/Cargo.toml ($gui_version) differ" >&2
  exit 1
fi

# kepubify (MIT), a pinned release checked against these checksums.
kepubify_version=v4.0.4
kepubify_arm64=6467d44439ee899113c7f710b509ef9e5ce65e8df711c85192b9ea5b683594b7
kepubify_x86_64=851afab0b83ecaf11f6965c901483eed3e74a6b41a3ab0a68f7321bc48bac4a3

work=$root/target/package-macos
dist=$root/dist
rm -rf "$work"
mkdir -p "$work" "$dist"

export MACOSX_DEPLOYMENT_TARGET=11.0
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --locked --target "$target" -p nickelinksync -p nickelinksync-gui
done
universal() { # name -> $work/name
  lipo -create -output "$work/$1" \
    "target/aarch64-apple-darwin/release/$1" "target/x86_64-apple-darwin/release/$1"
}
universal nickelinksync
universal nickelinksync-gui

fetch() { # asset sha256 -> $work/asset
  curl -fsSL -o "$work/$1" "https://github.com/pgaskin/kepubify/releases/download/$kepubify_version/$1"
  echo "$2  $work/$1" | shasum -a 256 -c - >/dev/null || {
    echo "$1: checksum differs from the pinned one" >&2
    exit 1
  }
}
fetch kepubify-darwin-arm64 "$kepubify_arm64"
fetch kepubify-darwin-64bit "$kepubify_x86_64"
lipo -create -output "$work/kepubify" "$work/kepubify-darwin-arm64" "$work/kepubify-darwin-64bit"
chmod 755 "$work/kepubify"

# The icon, from the SVG.
iconset=$work/AppIcon.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  rsvg-convert -w "$size" -h "$size" packaging/icon.svg -o "$iconset/icon_${size}x${size}.png"
  double=$((size * 2))
  rsvg-convert -w "$double" -h "$double" packaging/icon.svg -o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$work/AppIcon.icns" "$iconset"

# The app bundle. kepubify sits next to the program, where the app looks
# for it first.
app=$work/dmg/NickelInkSync.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses"
cp "$work/nickelinksync-gui" "$app/Contents/MacOS/NickelInkSync"
cp "$work/kepubify" "$app/Contents/MacOS/kepubify"
cp "$work/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
cp LICENSE "$app/Contents/Resources/licenses/NickelInkSync-LICENSE.txt"
cp packaging/licenses/kepubify-LICENSE.txt "$app/Contents/Resources/licenses/"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
# Ad hoc signature: an arm64 Mac runs no unsigned code, and lipo and the
# bundle changed what the linker signed.
codesign --force --sign - "$app/Contents/MacOS/kepubify"
codesign --force --sign - "$app"

ln -s /Applications "$work/dmg/Applications"
cp packaging/licenses/kepubify-LICENSE.txt "$work/dmg/"
cp LICENSE "$work/dmg/LICENSE.txt"
dmg=$dist/NickelInkSync-$version-macos.dmg
rm -f "$dmg"
hdiutil create -quiet -volname "NickelInkSync $version" -srcfolder "$work/dmg" -ov -format UDZO "$dmg"

# The CLI, zipped so it keeps its executable bit.
cli=$work/cli/nickelinksync-$version-macos
mkdir -p "$cli"
cp "$work/nickelinksync" "$cli/"
codesign --force --sign - "$cli/nickelinksync"
cp LICENSE "$cli/LICENSE.txt"
zip=$dist/nickelinksync-$version-macos.zip
rm -f "$zip"
(cd "$work/cli" && ditto -c -k --norsrc --keepParent "nickelinksync-$version-macos" "$zip")

echo "$dmg"
echo "$zip"
