#!/usr/bin/env python3
"""Time one headless game start from a launcher-planned command.

run.py <command.json> <game_dir> <label> [--jar PATH | --no-arctic] [--pack NAME] [--timeout S]

Replaces mods/arctic*.jar with --jar (or removes it), selects resourcepacks/<NAME>,
starts Xvfb + Mesa llvmpipe, waits for ModernFix's "Game took" line (or the Arctic
"ready" timeline), then stops the game. Prints one RESULT line. One game at a time.
"""
import json, os, re, shutil, subprocess, sys, time
from pathlib import Path

args = sys.argv[1:]
cmd = json.loads(Path(args[0]).read_text())
game = Path(args[1]); label = args[2]
def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default
jar = opt('--jar'); extra = opt('--extra', '').split(); pack = opt('--pack'); timeout = float(opt('--timeout', '300'))
mods = game / 'mods'
for p in mods.glob('arctic*.jar'):
    p.unlink()
if jar and '--no-arctic' not in args:
    shutil.copy(jar, mods / opt('--jar-name', 'arctic-mod.jar'))
# Same options every run: small window, no music, the pack (or none).
opts = game / 'options.txt'
lines = [l for l in (opts.read_text().splitlines() if opts.exists() else []) if not l.startswith(('resourcePacks:', 'renderDistance:', 'soundCategory_master:', 'pauseOnLostFocus:', 'fullscreen:'))]
lines += ['resourcePacks:' + json.dumps(['vanilla', 'file/' + pack] if pack else ['vanilla']),
          'renderDistance:4', 'soundCategory_master:0.0', 'pauseOnLostFocus:false', 'fullscreen:false']
opts.write_text('\n'.join(lines) + '\n')
cmd = [('-Xmx2g' if a.startswith('-Xmx') else a) for a in cmd]
# The class-list recording of the launcher's startup cache differs per start: leave it out of
# timed runs. Without the client, the launcher would not add its agent either.
cmd = [a for a in cmd if not a.startswith(('-Xlog:class+load', '-Darctic.classlog', '-Darctic.classlist', '-Darctic.classkey'))]
if '--no-arctic' in args:
    cmd = [a for a in cmd if not (a.startswith('-javaagent:') and 'arctic' in a) and not a.startswith(('-Darctic.preload', '-Darctic.mixincache'))]
cmd[1:1] = extra
display = ':197'
xvfb = subprocess.Popen(['Xvfb', display, '-screen', '0', '1280x720x24', '-nolisten', 'tcp'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(0.7)
env = {k: v for k, v in os.environ.items() if not k.startswith('ARCTIC_')}
env.update(DISPLAY=display, LIBGL_ALWAYS_SOFTWARE='1', ALSOFT_DRIVERS='null', MESA_GL_VERSION_OVERRIDE='4.5')
out = Path(os.environ.get('STARTUP_RUNS', 'startup-runs')) / f'{label}.log'
out.parent.mkdir(exist_ok=True)
start = time.monotonic(); took = ready = proc = None
try:
    with out.open('w') as log:
        proc = None
        proc = subprocess.Popen(cmd, cwd=game, env=env, stdout=log, stderr=subprocess.STDOUT)
        while time.monotonic() - start < timeout and proc.poll() is None:
            text = out.read_text(errors='replace')
            m = re.search(r'Game took ([0-9.]+) seconds', text)
            r = re.search(r'\[Arctic startup\][^\n]* ready (\d+) ms', text)
            if r: ready = int(r.group(1)) / 1000
            pr = re.search(r'STARTUP_PROBE_READY (\d+)', text)
            if pr and not m and '--probe' in args:
                took = int(pr.group(1)) / 1000; wall = time.monotonic() - start; time.sleep(3); break
            if m:
                took = float(m.group(1)); wall = time.monotonic() - start
                time.sleep(3)  # let the Arctic timeline line land
                text = out.read_text(errors='replace')
                r = re.search(r'\[Arctic startup\][^\n]* ready (\d+) ms', text)
                if r: ready = int(r.group(1)) / 1000
                break
            time.sleep(0.2)
        else:
            wall = time.monotonic() - start
        if shutil.which('import'):
            subprocess.run(['import', '-display', display, '-window', 'root', str(out.with_suffix('.png'))], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if proc.poll() is None:
            proc.terminate()
            try: proc.wait(15)
            except subprocess.TimeoutExpired: proc.kill(); proc.wait()
finally:
    if proc is not None and proc.poll() is None:
        proc.kill(); proc.wait()
    xvfb.terminate(); xvfb.wait()
text = out.read_text(errors='replace')
print(f'RESULT {label} game_took={took} wall_to_took={round(wall, 2) if took else None} arctic_ready={ready} '
      f'early_stage_errors={text.count("was not loaded in early stage")} space_warnings={text.count("Invalid type: space")}', flush=True)
