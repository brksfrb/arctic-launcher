#!/usr/bin/env bash
set -euo pipefail
results=${1:?test results directory}
app=com.arcticlauncher.ArcticLauncher
capture_results() {
  flatpak kill "$app" 2>/dev/null || true
  if [ -n "${title_game_dir:-}" ] && [ -d "$title_game_dir/screenshots" ]; then
    cp -a "$title_game_dir/screenshots" "$results/title-screenshots"
  fi
}
trap capture_results EXIT
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

title_game_dir=${3:?title compatibility game directory}
modmenu_file=$(find "$title_game_dir/mods" -maxdepth 1 -type f -iname 'modmenu-*.jar' -printf '%f\n')
flashback_file=$(find "$title_game_dir/mods" -maxdepth 1 -type f -iname 'flashback*.jar' -printf '%f\n')
test -n "$modmenu_file" && test -n "$flashback_file"
title_failed=false
for variant in plain modmenu flashback both; do
  expected=""
  menu=off
  replay=off
  case "$variant" in
    modmenu) menu=on; expected=modmenu ;;
    flashback) replay=on; expected=flashback ;;
    both) menu=on; replay=on; expected=modmenu,flashback ;;
  esac
  flatpak run --command=arctic "$app" mods toggle "$modmenu_file" --instance flatpak-title "$menu"
  flatpak run --command=arctic "$app" mods toggle "$flashback_file" --instance flatpak-title "$replay"
  flatpak run --command=arctic "$app" instances edit flatpak-title \
    --jvm-args="-Darctic.selftest=title-compat -Darctic.selftest.titleMods=$expected"
  if ! timeout 240 flatpak run --command=arctic "$app" --json launch --instance flatpak-title --offline FlatpakTest --memory 2G --wait > "$results/title-$variant.log" 2>&1; then
    title_failed=true
    flatpak kill "$app" 2>/dev/null || true
  fi
  if ! grep -F 'titlecompat: passed' "$results/title-$variant.log" || grep -F 'titlecompat: FAILED' "$results/title-$variant.log"; then
    title_failed=true
  fi
  cp -a "$title_game_dir/screenshots" "$results/title-$variant-screenshots"
done

# Record, save and play back a real world with and without Flashback. Run both
# cases even if one fails so a compatibility regression is distinguishable.
replay_failed=false
flatpak run --command=arctic "$app" mods toggle "$modmenu_file" --instance flatpak-title off
flatpak run --command=arctic "$app" instances edit flatpak-title --jvm-args='-Darctic.selftest=replay'
for variant in plain flashback; do
  enabled=off
  if [ "$variant" = flashback ]; then enabled=on; fi
  flatpak run --command=arctic "$app" mods toggle "$flashback_file" --instance flatpak-title "$enabled"
  if ! timeout 420 flatpak run --command=arctic "$app" --json launch --instance flatpak-title --offline FlatpakTest --memory 2G --wait > "$results/replay-$variant.log" 2>&1; then
    replay_failed=true
    flatpak kill "$app" 2>/dev/null || true
  fi
  if ! grep -F 'replaytest: PASSED' "$results/replay-$variant.log" || grep -E '(replaytest|worldtest): FAILED' "$results/replay-$variant.log"; then
    replay_failed=true
  fi
done
test "$title_failed" = false && test "$replay_failed" = false

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
