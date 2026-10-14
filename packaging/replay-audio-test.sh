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
for version in 26.3 26.2 1.8.9; do
  game=""
  id="audio-$version"
  arctic instances create "$id" --version "$version" --loader fabric
  arctic instances edit "$id" --arctic-client on --performance off --memory 2G --jvm-args='-Darctic.selftest=replay'
  timeout 900 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline AudioTest --memory 2G --dry-run > "audio-test-results/$version-prepare.log" 2>&1
  game=$(arctic --json instances show "$id" | jq -r '.game_dir')
  # Each test gets a separate data directory and disposable game settings.
  mkdir -p "$game"
  printf 'soundCategory_master:1.0\nsoundCategory_music:0.0\nsoundCategory_blocks:1.0\n' >> "$game/options.txt"
  command timeout 420 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline AudioTest --memory 2G --wait > "audio-test-results/$version-game.log" 2>&1
  grep -F 'replaytest: PASSED' "audio-test-results/$version-game.log"
  save_results
  python3 packaging/verify-replay-audio.py "audio-test-results/$version/replay_videos"
done
