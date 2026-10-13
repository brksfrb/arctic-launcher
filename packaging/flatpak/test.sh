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

xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash "$here/test-windows.sh" "$results"
echo 'Flatpak installation, signed update, launcher, and Fabric game checks passed.'
