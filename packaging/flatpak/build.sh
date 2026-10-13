#!/usr/bin/env bash
set -euo pipefail

# Package the native Linux binaries built on the Ryzen runner. The runtime supplies the
# desktop/graphics/audio libraries; Minecraft and its Java runtimes live in the app's data dir.
input=$(realpath "${1:?native artifact directory}")
output=$(realpath -m "${2:?new output directory}")
base_url=${3:?public repository base URL}
base_url=${base_url%/}
key=${4:?signing fingerprint}
: "${GNUPGHOME:?isolated signing keyring}"
app=com.arcticlauncher.ArcticLauncher
runtime=25.08
here=$(cd -- "$(dirname -- "$0")" && pwd)
test ! -e "$output" || { echo "Output already exists: $output" >&2; exit 1; }
mkdir -p "$output/site" "$output/downloads"
build="$output/build"
repo="$output/site/repo"
version=$(cat "$input/version.txt")
date=$(cat "$input/date.txt")

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --assumeyes --noninteractive flathub \
  "org.freedesktop.Platform//$runtime" "org.freedesktop.Sdk//$runtime"
flatpak build-init "$build" "$app" org.freedesktop.Sdk org.freedesktop.Platform "$runtime"
install -Dm755 "$input/arctic-launcher" "$build/files/bin/arctic-launcher"
install -Dm755 "$input/arctic" "$build/files/bin/arctic"
install -Dm644 "$input/icon.png" "$build/files/share/icons/hicolor/256x256/apps/$app.png"
install -Dm644 "$here/$app.desktop" "$build/files/share/applications/$app.desktop"
install -Dm644 "$here/../../LICENSE" "$build/files/share/licenses/$app/LICENSE"
# Minecraft's narrator loads Flite dynamically; it is not in the base runtime.
mkdir -p "$build/files/lib"
cp -a /usr/lib/x86_64-linux-gnu/libflite*.so* "$build/files/lib/"
ln -s libflite.so.1 "$build/files/lib/libflite.so"
ln -s libflite_cmu_us_kal.so.1 "$build/files/lib/libflite_cmu_us_kal.so"
install -Dm644 /usr/share/doc/libflite1/copyright "$build/files/share/licenses/flite/copyright"
mkdir -p "$build/files/share/metainfo"
sed -e "s/@VERSION@/$version/g" -e "s/@DATE@/$date/g" \
  "$here/$app.metainfo.xml" > "$build/files/share/metainfo/$app.metainfo.xml"
desktop-file-validate "$build/files/share/applications/$app.desktop"
appstreamcli validate --no-net "$build/files/share/metainfo/$app.metainfo.xml"
appstreamcli compose --prefix=/ --origin="$app" --components="$app" \
  --data-dir="$build/files/share/app-info/xmls" \
  --icons-dir="$build/files/share/app-info/icons/flatpak" "$build/files"
test -s "$build/files/share/app-info/xmls/$app.xml.gz"

# Java games still need X11 on Wayland desktops. Network is also used by login and local
# game/voice bridges. File picking uses XDG portals.
flatpak build-finish "$build" --command=arctic-launcher \
  --share=network --share=ipc --socket=wayland --socket=x11 --socket=pulseaudio \
  --device=dri \
  --talk-name=org.kde.StatusNotifierWatcher \
  --talk-name=org.a11y.Bus --filesystem=xdg-run/discord-ipc-0
flatpak build-export --gpg-sign="$key" "$repo" "$build" stable
flatpak build-update-repo --gpg-sign="$key" --generate-static-deltas \
  --title='Arctic Launcher' --default-branch=stable "$repo"
flatpak build-bundle --repo-url="$base_url/repo/" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  --gpg-keys="$here/public-key.gpg" "$repo" \
  "$output/downloads/arctic-launcher-linux-x64.flatpak" "$app" stable

public_key=$(base64 -w0 "$here/public-key.gpg")
cat > "$output/site/arctic.flatpakref" <<EOF
[Flatpak Ref]
Name=$app
Branch=stable
Title=Arctic Launcher
Url=$base_url/repo/
RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo
IsRuntime=false
GPGKey=$public_key
EOF
cat > "$output/site/arctic.flatpakrepo" <<EOF
[Flatpak Repo]
Title=Arctic Launcher
Url=$base_url/repo/
Homepage=https://arcticlauncher.com
Description=Official Arctic Launcher updates
GPGKey=$public_key
EOF
cp "$here/public-key.gpg" "$output/site/public-key.gpg"
cp "$here/index.html" "$output/site/index.html"
cp "$input/icon.png" "$output/site/icon.png"
cp "$output/site/arctic.flatpakref" "$output/downloads/arctic.flatpakref"
(cd "$output/downloads" && sha256sum -- arctic-launcher-linux-x64.flatpak arctic.flatpakref > FLATPAK-SHA256SUMS.txt)
