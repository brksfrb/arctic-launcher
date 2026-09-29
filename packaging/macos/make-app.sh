#!/usr/bin/env bash
# Build the macOS downloads from the Apple Silicon and Intel release builds
# (run on a Mac, after `cargo build --release --target aarch64-apple-darwin`
# and `--target x86_64-apple-darwin` for arctic-app and arctic-cli):
#
#   dist/arctic-launcher-macos.zip   Arctic Launcher.app (what people download)
#   dist/arctic-launcher-macos       the launcher program alone (what updates replace)
#   dist/arctic-macos                the command line tool
#
# Everything is universal (both kinds of Mac) and ad-hoc signed; without a
# paid Apple Developer ID, macOS asks once before opening it.
#
# Usage: packaging/macos/make-app.sh <version>
set -euo pipefail

VERSION="${1:?version, like 0.2.0}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
OUT="$ROOT/dist"
APP="$OUT/Arctic Launcher.app"
ARM="$ROOT/target/aarch64-apple-darwin/release"
INTEL="$ROOT/target/x86_64-apple-darwin/release"

mkdir -p "$OUT"
rm -rf "$APP"
lipo -create "$ARM/arctic-launcher" "$INTEL/arctic-launcher" -output "$OUT/arctic-launcher-macos"
lipo -create "$ARM/arctic" "$INTEL/arctic" -output "$OUT/arctic-macos"
codesign --force --sign - "$OUT/arctic-launcher-macos"
codesign --force --sign - "$OUT/arctic-macos"

mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$OUT/arctic-launcher-macos" "$APP/Contents/MacOS/arctic-launcher"
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# The icon, drawn by the launcher itself at every size macOS wants.
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  "$OUT/arctic-launcher-macos" --write-icon "$ICONSET/icon_${size}x${size}.png" "$size"
  "$OUT/arctic-launcher-macos" --write-icon "$ICONSET/icon_${size}x${size}@2x.png" "$((size * 2))"
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

codesign --force --deep --sign - "$APP"
(cd "$OUT" && rm -f arctic-launcher-macos.zip && ditto -c -k --keepParent "Arctic Launcher.app" arctic-launcher-macos.zip)
echo "Built $OUT/arctic-launcher-macos.zip"
