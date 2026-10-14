#!/usr/bin/env bash
# Disposable CI only. No release, account, user's instances, or desktop access.
set -euo pipefail
mkdir -p audio-test-results
export PATH="$PWD/audio-test-bin:$PATH"
root="$RUNNER_TEMP/arctic-audio"
arctic() { command arctic --data-dir "$root" "$@"; }
pulseaudio --start --exit-idle-time=-1
pactl load-module module-null-sink sink_name=ci
for version in 26.3 26.2 1.8.9; do
  id="audio-$version"
  arctic instances create "$id" --version "$version" --loader fabric
  arctic instances edit "$id" --arctic-client on --performance off --memory 2G --jvm-args='-Darctic.selftest=replay'
  arctic launch --instance "$id" --offline AudioTest --memory 2G --dry-run > "audio-test-results/$version-prepare.log" 2>&1
  game=$(arctic --json instances show "$id" | jq -r '.game_dir')
  # Each test gets a separate data directory and disposable game settings.
  mkdir -p "$game"
  printf 'soundCategory_master:1.0\nsoundCategory_music:0.0\nsoundCategory_blocks:1.0\n' >> "$game/options.txt"
  command timeout 420 "$PWD/audio-test-bin/arctic" --data-dir "$root" launch --instance "$id" --offline AudioTest --memory 2G --wait > "audio-test-results/$version-game.log" 2>&1
  grep -F 'replaytest: PASSED' "audio-test-results/$version-game.log"
  mkdir -p "audio-test-results/$version"
  cp -a "$game/replay_videos" "$game/replay_recordings" "audio-test-results/$version/"
  python3 packaging/verify-replay-audio.py "audio-test-results/$version/replay_videos"
done
