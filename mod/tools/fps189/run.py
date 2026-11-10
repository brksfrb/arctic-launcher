#!/usr/bin/env python3
"""Run an instrumented 1.8.9 jar with an existing launcher dry-run command (offline only)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import uuid


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--launch-plan', type=Path, required=True)
    p.add_argument('--jar', type=Path, required=True, help='Client jar with Fps189Benchmark and runQueued hook')
    p.add_argument('--polarium', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True, help='New directory for this run')
    p.add_argument('--java', help='Override Java; JFR requires a supporting runtime')
    p.add_argument('--vm-arg', action='append', default=[])
    p.add_argument('--jfr', action='store_true')
    args = p.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    game = root / 'game'
    (game / 'mods').mkdir(parents=True)
    (game / 'config').mkdir()
    command = shlex.split(args.launch_plan.read_text())
    command = [s for s in command if not s.startswith(('-javaagent:', '-Darctic.selftest=', '-Darctic.preload='))]
    if '-DFabricMcEmu=' in command:
        i = command.index('-DFabricMcEmu=')
        command[i:i + 2] = ['-DFabricMcEmu= net.minecraft.client.main.Main ']
    if args.java:
        command[0] = args.java
    command = ['-Xmx1800M' if s.startswith('-Xmx') else s for s in command]
    name = 'FPS189Offline'
    offline_uuid = uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:' + name).encode()).digest(), version=3).hex
    values = {'--username': name, '--uuid': offline_uuid, '--gameDir': str(game), '--width': '1280',
              '--height': '720', '--accessToken': '0', '--userProperties': '{}', '--xuid': '0',
              '--clientId': '0', '--userType': 'legacy'}
    for key, value in values.items():
        if key in command:
            command[command.index(key) + 1] = value
    if '--accessToken' not in command or command[command.index('--accessToken') + 1] != '0':
        raise SystemExit('Expected a Minecraft launch command with --accessToken')
    if any(s.startswith(('--server', '--quickPlay')) for s in command):
        raise SystemExit('Multiplayer/quick-play arguments are forbidden in this offline benchmark')
    vm = ['-XX:ActiveProcessorCount=8', '-Dorg.lwjgl.opengl.Display.allowSoftwareOpenGL=true',
          '-Dfps189.bench=true', '-Dfps189.output=' + str(root / 'results.txt')]
    if args.jfr:
        vm.append('-XX:StartFlightRecording=filename=' + str(root / 'profile.jfr')
                  + ',settings=profile,delay=50s,duration=35s')
    command[1:1] = vm + args.vm_arg
    shutil.copyfile(args.jar, game / 'mods/arctic.jar')
    shutil.copyfile(args.polarium, game / 'mods/polarium.jar')
    (game / 'options.txt').write_text('renderDistance:4\nguiScale:2\nmaxFps:260\nenableVsync:false\n'
        'useVbo:false\nfullscreen:false\npauseOnLostFocus:false\nchatWidth:0.6\nchatHeightFocused:1.0\n'
        'chatHeightUnfocused:0.44366196\nchatScale:1.0\nfancyGraphics:true\n')
    (game / 'config/arctic.json').write_text(json.dumps({'fancy': True, 'fontAntialias': True,
        'replayEnabled': False, 'replayVideoEnabled': False, 'legacyVboOn': True, 'dynamicFps': False}))
    env = dict(os.environ, LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='8', ALSOFT_DRIVERS='null')
    with (root / 'game.log').open('w') as log:
        subprocess.run(command, cwd=game, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=210, check=True)
    log = (root / 'game.log').read_text()
    if any(s in log for s in ('Cannot place blocks outside', 'Cannot summon the object', 'Unbalanced brackets', 'FPS189 TIMEOUT')):
        raise SystemExit('Scene setup failed; reject this run (see game.log)')
    results = (root / 'results.txt').read_text()
    if results.count('FPS189 RESULT') != 2:
        raise SystemExit('Expected both open-chat and closed-chat measurements')
    print(results, end='')


if __name__ == '__main__':
    main()
