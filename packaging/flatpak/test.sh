#!/usr/bin/env bash
set -euo pipefail

# Run only on a disposable Linux CI worker: installs, updates, and launches the app.
output=$(realpath "${1:?package output directory}")
key=${2:?signing fingerprint}
here=$(cd -- "$(dirname -- "$0")" && pwd)
app=com.arcticlauncher.ArcticLauncher
results="$output/test-results"
mkdir -p "$results"
exec > >(tee "$results/checks.log") 2>&1

# Use a copy so the artificial second version never reaches users.
cp -a "$output/site/repo" "$output/test-repo"
cp -a "$output/build" "$output/test-build"
flatpak remote-add --user --gpg-import="$here/public-key.gpg" arctic-test "file://$output/test-repo"
flatpak update --user --appstream arctic-test
find "$HOME/.local/share/flatpak/appstream/arctic-test" -name appstream.xml.gz -exec gzip -cd {} + | grep -F "<id>$app</id>"
flatpak install --user --assumeyes --noninteractive arctic-test "$app"
flatpak run --command=arctic "$app" --version
flatpak run --command=arctic "$app" --json update | jq -e '.managed_by == "flatpak"'
root=$(flatpak run --command=arctic "$app" --json paths | jq -r 'select(.name == "root") | .path')
test "$root" = "$HOME/.var/app/$app/data/ArcticLauncher"
flatpak run --command=sh "$app" -c 'test -f /.flatpak-info && test ! -w /app/bin/arctic-launcher'

before=$(flatpak info --user --show-commit "$app")
mkdir -p "$root"
printf 'persistent data\n' > "$root/flatpak-update-test.txt"
printf 'test update\n' > "$output/test-build/files/share/flatpak-update-test.txt"
flatpak build-export --gpg-sign="$key" "$output/test-repo" "$output/test-build" stable
flatpak build-update-repo --gpg-sign="$key" "$output/test-repo"
flatpak update --user --assumeyes --noninteractive "$app"
after=$(flatpak info --user --show-commit "$app")
test "$before" != "$after"
flatpak run --command=sh "$app" -c 'test -f /app/share/flatpak-update-test.txt'
test "$(cat "$root/flatpak-update-test.txt")" = 'persistent data'
printf 'Verified signed update: %s -> %s\n' "$before" "$after"

# Restore the exact candidate for graphics/game tests.
flatpak uninstall --user --assumeyes --noninteractive "$app"
flatpak remote-modify --user --url="file://$output/site/repo" arctic-test
flatpak install --user --assumeyes --noninteractive arctic-test "$app"
test "$(flatpak info --user --show-commit "$app")" = "$before"
test "$(cat "$root/flatpak-update-test.txt")" = 'persistent data'

flatpak run --command=arctic "$app" instances create flatpak-test --version 1.21.4 --loader fabric
flatpak run --command=arctic "$app" instances edit flatpak-test --arctic-client on --performance off --memory 2G
timeout 900 flatpak run --command=arctic "$app" launch --instance flatpak-test --offline FlatpakTest --memory 2G --dry-run > "$results/prepare.log" 2>&1

# Exercise the custom pack picker on the version that exposed Fabric's internal packs.
flatpak run --command=arctic "$app" instances create flatpak-packs --version 1.21.11 --loader fabric
flatpak run --command=arctic "$app" instances edit flatpak-packs --arctic-client on --performance on --shaders on --memory 2G \
  --jvm-args='-Darctic.selftest=packs -Darctic.selftest.requireHidden=true -Darctic.selftest.shader=Arctic-test-shader'
timeout 900 flatpak run --command=arctic "$app" launch --instance flatpak-packs --offline FlatpakTest --memory 2G --dry-run > "$results/packs-prepare.log" 2>&1
game_dir=$(flatpak run --command=arctic "$app" --json instances show flatpak-packs | jq -r '.game_dir')
mkdir -p "$game_dir/shaderpacks/Arctic-test-shader/shaders"
printf '# Minimal shader fixture for the Iris load/unload check.\n' > "$game_dir/shaderpacks/Arctic-test-shader/shaders/shaders.properties"

# Real title-screen integrations: test the event-based Mod Menu and Flashback's applied mixin,
# separately and together, so neither can accidentally cover a broken detection path in the other.
flatpak run --command=arctic "$app" instances create flatpak-title --version 26.3 --loader fabric
flatpak run --command=arctic "$app" instances edit flatpak-title --arctic-client on --performance off --memory 2G
flatpak run --command=arctic "$app" mods install modmenu --instance flatpak-title
flatpak run --command=arctic "$app" mods install flashback --instance flatpak-title
timeout 900 flatpak run --command=arctic "$app" launch --instance flatpak-title --offline FlatpakTest --memory 2G --dry-run > "$results/title-prepare.log" 2>&1
title_game_dir=$(flatpak run --command=arctic "$app" --json instances show flatpak-title | jq -r '.game_dir')

xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash "$here/test-windows.sh" "$results" "$game_dir" "$title_game_dir"
echo 'Flatpak installation, signed update, launcher, and Fabric game checks passed.'
