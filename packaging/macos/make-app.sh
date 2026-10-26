#!/usr/bin/env bash
# Build the macOS downloads from the Apple Silicon and Intel release builds
# (run on a Mac, after `cargo build --release --target aarch64-apple-darwin`
# and `--target x86_64-apple-darwin` for arctic-app and arctic-cli):
#
#   dist/arctic-launcher-macos.zip   Arctic Launcher.app (what people download)
#   dist/arctic-launcher-macos       the launcher program alone (what updates replace)
#   dist/arctic-macos                the command line tool
#
# Everything is universal (both kinds of Mac). With APPLE_SIGN_IDENTITY set (the release workflow sets it
# from its Apple secrets) it is signed with that Developer ID and the app is notarized by Apple and stapled
# (APPLE_API_KEY_PATH, APPLE_API_KEY_ID and APPLE_API_ISSUER too); without them it is ad-hoc signed and macOS
# asks once before opening it.
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

# Sign with the Developer ID (hardened runtime, as notarization needs) or ad hoc.
sign() {
  if [ -n "${APPLE_SIGN_IDENTITY:-}" ]; then
    codesign --force --options runtime --timestamp --sign "$APPLE_SIGN_IDENTITY" "$@"
  else
    codesign --force --sign - "$@"
  fi
}

mkdir -p "$OUT"
rm -rf "$APP"
lipo -create "$ARM/arctic-launcher" "$INTEL/arctic-launcher" -output "$OUT/arctic-launcher-macos"
lipo -create "$ARM/arctic" "$INTEL/arctic" -output "$OUT/arctic-macos"
sign "$OUT/arctic-launcher-macos"
sign "$OUT/arctic-macos"

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

sign --deep "$APP"
(cd "$OUT" && rm -f arctic-launcher-macos.zip && ditto -c -k --keepParent "Arctic Launcher.app" arctic-launcher-macos.zip)

# Notarization: Apple scans the app; the ticket is stapled to it so it opens without any warning, offline too.
if [ -n "${APPLE_SIGN_IDENTITY:-}" ] && [ -n "${APPLE_API_KEY_PATH:-}" ]; then
  echo "Notarizing (Apple's scan can take a few minutes)"
  result="$OUT/notarization.json"
  xcrun notarytool submit "$OUT/arctic-launcher-macos.zip" --key "$APPLE_API_KEY_PATH" \
    --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" --wait --output-format json > "$result" || true
  cat "$result"
  if ! grep -q '"status" *: *"Accepted"' "$result"; then
    id=$(sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' "$result" | head -1)
    [ -n "$id" ] && xcrun notarytool log "$id" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" || true
    echo "Notarization was not accepted" >&2
    exit 1
  fi
  xcrun stapler staple "$APP"
  (cd "$OUT" && rm -f arctic-launcher-macos.zip && ditto -c -k --keepParent "Arctic Launcher.app" arctic-launcher-macos.zip)
fi
echo "Built $OUT/arctic-launcher-macos.zip"
