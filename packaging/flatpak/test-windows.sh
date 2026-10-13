#!/usr/bin/env bash
set -euo pipefail
results=${1:?test results directory}
app=com.arcticlauncher.ArcticLauncher
trap 'flatpak kill "$app" 2>/dev/null || true' EXIT
pulseaudio --start --exit-idle-time=-1
pactl load-module module-null-sink sink_name=ci

flatpak run "$app" --no-intro > "$results/launcher.log" 2>&1 &
pid=$!
ready=false
for ((i=0; i<45; i++)); do
  kill -0 "$pid"
  if xwininfo -root -tree | grep -q 'Arctic Launcher'; then ready=true; break; fi
  sleep 1
done
test "$ready" = true
sleep 5
kill -0 "$pid"
import -window root "$results/launcher.png"
flatpak kill "$app"
wait "$pid" || true

timeout 240 flatpak run --command=arctic "$app" --json launch --instance flatpak-packs --offline FlatpakTest --memory 2G --wait > "$results/packs.log" 2>&1
grep -F 'packstest: passed' "$results/packs.log"
if grep -F 'packstest: FAILED' "$results/packs.log"; then exit 1; fi
cp -a "${2:?packs game directory}/screenshots" "$results/packs-screenshots"

flatpak run --command=arctic "$app" --json launch --instance flatpak-test --offline FlatpakTest --memory 2G --wait > "$results/game.log" 2>&1 &
pid=$!
ready=false
for ((i=0; i<240; i++)); do
  kill -0 "$pid"
  if grep -q '"event":"window_ready"' "$results/game.log"; then ready=true; break; fi
  sleep 1
done
test "$ready" = true
sleep 20
kill -0 "$pid"
xwininfo -root -tree > "$results/game-windows.txt"
grep -Ei 'minecraft|arctic' "$results/game-windows.txt"
if grep -E 'Error starting SoundSystem|Error while loading the narrator' "$results/game.log"; then exit 1; fi
import -window root "$results/game.png"
flatpak kill "$app"
wait "$pid" || true
