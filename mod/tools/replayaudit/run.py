"""run.py <189|263> <label> [--mode mp|sp] [--selftest NAME] [--props "-Dk=v ..."] [--rd N] [--mobs N] [--timeout S]

One real client start under its own Xvfb (software GL, 1280x720, GUI scale 2) with the audit agent attached; in mp
mode against a local offline vanilla server (the join-pass lobby world plus --mobs wandering mobs for packet load).
Everything lives under $REPLAYAUDIT_JOB (a mktemp dir in /home/claude/work-tmp); nothing is written to /tmp.
Out: $REPLAYAUDIT_JOB/out/<label>/ = game.log, server.log, frames.csv, stacks.txt, cpu.csv, gc.csv, *.png.
At most two JVMs run: the client and, in mp mode, the server.
"""
import os, sys, pathlib, subprocess, shlex, uuid, hashlib, shutil, time, json, argparse

JOB = pathlib.Path(os.environ['REPLAYAUDIT_JOB'])
assert str(JOB).startswith('/home/claude/work-tmp/'), JOB
ap = argparse.ArgumentParser()
ap.add_argument('ver'); ap.add_argument('label'); ap.add_argument('--mode', default='mp')
ap.add_argument('--selftest', default='replayaudit'); ap.add_argument('--props', default='')
ap.add_argument('--rd', type=int, default=10); ap.add_argument('--mobs', type=int, default=120)
ap.add_argument('--timeout', type=int, default=600); ap.add_argument('--cfg', default='{}')
ap.add_argument('--noagent', action='store_true'); ap.add_argument('--jar', default='dev', help='which built jar: arctic-<ver>-<jar>.jar in the job dir')
ap.add_argument('--mods', default='instance', help='instance: the launcher performance set | none: Arctic alone'); ap.add_argument('--keepgame', help='reuse this label\'s game dir')
o = ap.parse_args()
V = {'189': dict(mc='1.8.9', inst='fl-1-8-9', old=True, port=25711),
     '263': dict(mc='26.3', inst='fl-26-3', old=False, port=25713)}[o.ver]
OLD, PORT = V['old'], V['port']
SRC_SRV = pathlib.Path('/home/claude/joinpass-work/srv') / o.ver
INST = pathlib.Path('/home/claude/fl-run/data/profiles/default/instances') / V['inst'] / 'minecraft'
out = JOB / 'out' / o.label; game = JOB / 'game' / (o.keepgame or o.label); srv = JOB / 'srv' / o.ver
shutil.rmtree(out, ignore_errors=True); out.mkdir(parents=True)
tmp = JOB / 'tmp'; tmp.mkdir(exist_ok=True)
NAME = 'ArcticDev'
procs = []
def start(args, name, cwd, env=None, stdin=None):
    p = subprocess.Popen(args, cwd=cwd, stdin=stdin, stdout=open(out / (name + '.log'), 'w'), stderr=subprocess.STDOUT, env=env, text=True)
    procs.append(p); return p
def waitfor(path, needle, timeout=300, proc=None):
    for _ in range(timeout * 4):
        if path.exists() and needle in path.read_text(errors='replace'): return
        if proc is not None and proc.poll() is not None: raise RuntimeError(f'{path.name}: exited {proc.returncode} before {needle!r}')
        time.sleep(0.25)
    raise RuntimeError('Timed out: ' + str(path) + ' ' + needle)

base = shlex.split(pathlib.Path(f'/home/claude/v4-output/logs/dry-run-{"189" if OLD else "263"}.txt').read_text())
JAVA = base[0]
server = None
def cmd(s, pause=0.0):
    server.stdin.write(s + '\n'); server.stdin.flush()
    if pause: time.sleep(pause)
status = 'failed'
try:
    if o.mode == 'mp':
        if not srv.exists():
            srv.mkdir(parents=True)
            for item in SRC_SRV.iterdir():
                if item.name in ('world', 'logs'): continue
                (shutil.copytree if item.is_dir() else shutil.copy2)(item, srv / item.name)
        lt = 'DEFAULT' if OLD else 'minecraft\\:normal'
        (srv / 'server.properties').write_text(
            f'online-mode=false\nserver-ip=127.0.0.1\nserver-port={PORT}\nlevel-seed=20261116\nlevel-type={lt}\nspawn-protection=0\n'
            f'view-distance={o.rd}\nsimulation-distance=8\nmax-players=20\ngamemode={1 if OLD else "creative"}\nforce-gamemode=true\nallow-flight=true\n'
            'network-compression-threshold=256\nplayer-idle-timeout=0\nspawn-monsters=false\nmotd=replay audit\nsync-chunk-writes=false\n'
            'white-list=false\nenforce-whitelist=false\n')
        (srv / 'eula.txt').write_text('eula=true\n')
        shutil.rmtree(srv / 'world', ignore_errors=True)
        shutil.copytree(srv / 'world-pristine', srv / 'world')
        server = start(['taskset', '-c', '22-23', JAVA, '-Xms1G', '-Xmx2G', '-Djava.io.tmpdir=' + str(tmp), '-jar', 'server.jar', 'nogui'], 'server', srv, stdin=subprocess.PIPE)
        waitfor(out / 'server.log', 'Done (', proc=server)
        # Wandering mobs inside the walled lobby: steady entity movement for the recorder to write.
        import random
        rng = random.Random(7)
        kinds = ['Chicken', 'Pig', 'Sheep', 'Cow'] if OLD else ['minecraft:chicken', 'minecraft:pig', 'minecraft:sheep', 'minecraft:cow']
        cmd('time set 6000', .2)
        for i in range(o.mobs):
            cmd('summon %s %.1f 121 %.1f' % (kinds[i % 4], rng.uniform(-20, 20), rng.uniform(-20, 20)), .02)
    if not o.keepgame:
        shutil.rmtree(game, ignore_errors=True)
    (game / 'mods').mkdir(parents=True, exist_ok=True); (game / 'config').mkdir(exist_ok=True)
    for old in (game / 'mods').glob('*.jar'): old.unlink()
    for jar in (INST / 'mods').glob('*.jar'):
        if jar.name != 'arctic-mod.jar' and o.mods == 'instance': shutil.copy2(jar, game / 'mods' / jar.name)
    shutil.copy2(JOB / f'arctic-{o.ver}-{o.jar}.jar', game / 'mods/arctic-mod.jar')
    for d in ('screenshots', 'logs'): shutil.rmtree(game / d, ignore_errors=True)
    opts = f'renderDistance:{o.rd}\nguiScale:2\nmaxFps:260\nfullscreen:false\npauseOnLostFocus:false\nenableVsync:false\n'
    opts += 'soundCategory_master:0.0\nfancyGraphics:true\n' if OLD else ('soundCategory_master:0.0\nsimulationDistance:8\nonboardAccessibility:false\n'
            'tutorialStep:none\nskipMultiplayerWarning:true\njoinedFirstServer:true\ninactivityFpsLimit:"minimized"\n')
    (game / 'options.txt').write_text(opts)
    cfg = dict(dynamicFps=False); cfg.update(json.loads(o.cfg))
    (game / 'config/arctic.json').write_text(json.dumps(cfg))
    a = [x for x in base if not x.startswith(('-Darctic.selftest=', '-Darctic.preload=', '-javaagent:', '-Xms', '-Xmx'))]
    if '-DFabricMcEmu=' in a:
        i = a.index('-DFabricMcEmu='); a[i:i + 2] = ['-DFabricMcEmu= net.minecraft.client.main.Main ']
    for key, val in [('--username', NAME), ('--uuid', uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:' + NAME).encode()).digest(), version=3).hex),
                     ('--gameDir', str(game)), ('--width', '1280'), ('--height', '720')]:
        if key in a: a[a.index(key) + 1] = val
    jv = ['-Xms512M', '-Xmx2048M', '-Djava.io.tmpdir=' + str(tmp), '-Darctic.selftest=' + o.selftest, '-Darctic.ffmpeg=/usr/bin/ffmpeg',
          '-Darctic.cosmetics.url=http://127.0.0.1:9', '-XX:ActiveProcessorCount=6']
    if not o.noagent: jv.insert(0, '-javaagent:' + str(JOB / 'agent/auditagent.jar') + f'={out};5')
    if o.mode == 'mp': jv.append('-Darctic.audit.server=true')
    a[1:1] = jv + shlex.split(o.props)
    if o.mode == 'mp':
        a += ['--server', '127.0.0.1', '--port', str(PORT)] if OLD else ['--quickPlayMultiplayer', f'127.0.0.1:{PORT}']
    if not OLD: a += ['--graphicsBackend', 'opengl']
    (out / 'command.json').write_text(json.dumps(a, indent=1))
    display = ':' + str(900 + int(hashlib.md5(o.label.encode()).hexdigest(), 16) % 90)
    start(['Xvfb', display, '-screen', '0', '1280x720x24', '-nolisten', 'tcp'], 'xvfb', JOB)
    time.sleep(1)
    env = dict(os.environ, DISPLAY=display, LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='4', TMPDIR=str(tmp))
    env.pop('JAVA_TOOL_OPTIONS', None)
    if not OLD: env.update(SDL_VIDEO_FORCE_EGL='1', MESA_GL_VERSION_OVERRIDE='4.6', MESA_GLSL_VERSION_OVERRIDE='460')
    for k in ('ARCTIC_SESSION', 'ARCTIC_TOKEN'): env.pop(k, None)
    client = start(['taskset', '-c', '16-21'] + a, 'client', game, env=env)
    if o.mode == 'mp':
        # Onto the lobby platform, in creative (the world spawn can be under water).
        waitfor(out / 'server.log', NAME + ' joined the game', proc=client, timeout=240)
        cmd(f'gamemode {1 if OLD else "creative"} {NAME}', .3); cmd(f'tp {NAME} 0.5 121 0.5', .3)
    try:
        rc = client.wait(timeout=o.timeout); status = f'exit {rc}'
    except subprocess.TimeoutExpired:
        status = 'timeout'
except Exception as e:
    print('RUN FAILED', repr(e), flush=True)
finally:
    if server is not None and server.poll() is None:
        try: cmd('stop', 3)
        except Exception: pass
    for p in reversed(procs):
        if p.poll() is None: p.terminate()
    for p in procs:
        try: p.wait(timeout=30)
        except subprocess.TimeoutExpired: p.kill()
    if (game / 'logs/latest.log').exists(): shutil.copy2(game / 'logs/latest.log', out / 'game.log')
    shots = sorted((game / 'screenshots').glob('*.png')) if (game / 'screenshots').exists() else []
    for i, s in enumerate(shots): shutil.copy2(s, out / f'shot-{i:02d}.png')
    print('RUN', o.label, status, 'shots', len(shots), flush=True)
