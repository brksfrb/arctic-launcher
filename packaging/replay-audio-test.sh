#!/usr/bin/env bash
# Disposable CI only. No release, account, user's instances, or desktop access.
set -euo pipefail
mkdir -p audio-test-results
export PATH="$PWD/audio-test-bin:$PATH"
root="$RUNNER_TEMP/arctic-audio"
arctic() { command arctic --data-dir "$root" "$@"; }
# Minecraft 26.3 falls back to Vulkan when SDL cannot use Xvfb's GLX visuals.
# Select the software driver explicitly instead of a runner's unused hardware ICD.
export VK_DRIVER_FILES
VK_DRIVER_FILES=$(find /usr/share/vulkan/icd.d -name 'lvp_icd*.json' -print -quit)
test -n "$VK_DRIVER_FILES"
vulkaninfo --summary > audio-test-results/vulkan.txt 2>&1
glxinfo -B > audio-test-results/opengl.txt 2>&1
pulseaudio --start --exit-idle-time=-1
pactl load-module module-null-sink sink_name=ci
save_results() {
  if [ -n "${game:-}" ]; then
    mkdir -p "audio-test-results/$version"
    for dir in replay_videos replay_recordings crash-reports; do
      if [ -d "$game/$dir" ]; then cp -a "$game/$dir" "audio-test-results/$version/"; fi
    done
  fi
}
trap save_results EXIT
for version in 1.8.9 26.2 26.3; do
  game=""
  id="audio-$version"
  arctic instances create "$id" --version "$version" --loader fabric
  arctic instances edit "$id" --arctic-client on --performance off --memory 2G --jvm-args='-Darctic.selftest=replay'
  timeout 900 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline AudioTest --memory 2G --dry-run > "audio-test-results/$version-prepare.log" 2>&1
  game=$(arctic --json instances show "$id" | jq -r '.game_dir')
  # Each test gets a separate data directory and disposable game settings.
  mkdir -p "$game"
  # Xvfb has no window manager to grant focus, so legacy Minecraft must not open its pause menu.
  printf 'pauseOnLostFocus:false\nsoundCategory_master:1.0\nsoundCategory_music:0.0\nsoundCategory_blocks:1.0\n' >> "$game/options.txt"
  command timeout 420 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline AudioTest --memory 2G --wait > "audio-test-results/$version-game.log" 2>&1 &
  game_pid=$!
  (sleep 180; if kill -0 "$game_pid" 2>/dev/null; then pkill -QUIT -x java || true; fi) &
  diagnostic_pid=$!
  result=0
  wait "$game_pid" || result=$?
  kill "$diagnostic_pid" 2>/dev/null || true
  wait "$diagnostic_pid" 2>/dev/null || true
  if [ "$result" != 0 ]; then tail -n 80 "audio-test-results/$version-game.log"; exit "$result"; fi
  grep -F 'replaytest: PASSED' "audio-test-results/$version-game.log"
  save_results
  python3 packaging/verify-replay-audio.py "audio-test-results/$version/replay_videos"
done

# Check Arctic's default multiplayer browser and preserved third-party menus.
id=menu-compat
arctic instances create "$id" --version 26.3 --loader fabric
arctic instances edit "$id" --arctic-client on --performance off --memory 2G
arctic mods install modmenu --instance "$id"
arctic mods install flashback --instance "$id"
timeout 900 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline MenuTest --memory 2G --dry-run > audio-test-results/menu-prepare.log 2>&1
menu_game=$(arctic --json instances show "$id" | jq -r '.game_dir')
modmenu_file=$(find "$menu_game/mods" -maxdepth 1 -type f -iname 'modmenu-*.jar' -printf '%f\n')
flashback_file=$(find "$menu_game/mods" -maxdepth 1 -type f -iname 'flashback*.jar' -printf '%f\n')
test -n "$modmenu_file" && test -n "$flashback_file"
for variant in plain modmenu flashback both; do
  expected=""; menu=off; replay=off
  case "$variant" in
    modmenu) menu=on; expected=modmenu ;;
    flashback) replay=on; expected=flashback ;;
    both) menu=on; replay=on; expected=modmenu,flashback ;;
  esac
  arctic mods toggle "$modmenu_file" --instance "$id" "$menu"
  arctic mods toggle "$flashback_file" --instance "$id" "$replay"
  arctic instances edit "$id" --jvm-args="-Darctic.selftest=title-compat -Darctic.selftest.titleMods=$expected"
  timeout 240 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline MenuTest --memory 2G --wait > "audio-test-results/menu-$variant.log" 2>&1
  grep -F 'titlecompat: passed' "audio-test-results/menu-$variant.log"
  if grep -F 'titlecompat: FAILED' "audio-test-results/menu-$variant.log"; then exit 1; fi
done
cp -a "$menu_game/screenshots" audio-test-results/menu-screenshots
