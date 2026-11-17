"""run.py <ver 189|1211|263> <label> [--arctic JAR] [--polarium JAR] [--mode mp|sp] [--bots N] [--rd N] [--lat MS] [--cfg JSON]
 [--perf] [--delay S] [--extra JVMARGS]
One real client start under Xvfb + llvmpipe; the join agent connects to a local offline server (mp) or opens a saved
world (sp) once the game has settled, then logs frames/screens/stacks until 75 s after the player is in game.
Out: runs/<label>/ (events.csv, frames.csv, stacks.txt, gc.csv, client.log, server.log)."""
import os, sys, pathlib, subprocess, shlex, uuid, hashlib, shutil, time, json, random, argparse

W = pathlib.Path('/home/claude/joinpass-work')
ap = argparse.ArgumentParser()
ap.add_argument('ver'); ap.add_argument('label')
ap.add_argument('--arctic'); ap.add_argument('--polarium'); ap.add_argument('--mode', default='mp')
ap.add_argument('--bots', type=int, default=80); ap.add_argument('--rd', type=int, default=10)
ap.add_argument('--lat', type=int, default=150); ap.add_argument('--cfg', default='{}')
ap.add_argument('--perf', action='store_true', help='add the launcher performance mod set (modern)')
ap.add_argument('--pack', action='store_true', help='the server sends a 32 px resource pack (accepted)'); ap.add_argument('--size', default='854x480'); ap.add_argument('--delay', type=int, default=0); ap.add_argument('--extra', default='')
ap.add_argument('--noop', action='store_true', help='GALLIUM_NOOP=1: no-op GPU, exposes CPU cost')
ap.add_argument('--looks', default='fake', help='fake | real | off (unreachable)')
o = ap.parse_args()
V = {'189': dict(mc='1.8.9', args='189', inst='fl-1-8-9', old=True),
     '1211': dict(mc='1.21.11', args='12111', inst='fl-1-21-11', old=False),
     '263': dict(mc='26.3', args='263', inst='fl-26-3', old=False)}[o.ver]
OLD = V['old']
PORT = {'189': 25611, '1211': 25612, '263': 25613}[o.ver]
LOOKS_PORT = 18461 + int(o.ver) % 7
out = W / 'runs' / o.label; game = W / 'game' / o.ver; srv = W / 'srv' / o.ver
shutil.rmtree(out, ignore_errors=True); out.mkdir(parents=True)
NAME = 'JoinTest'
BURAK = dict(showCosmetics=True, fullbright=True, connectedGlass=True, zoomEnabled=True, freelookEnabled=True, snaplookEnabled=True,
 hotbarScrollStop=True, chatTimestamps=True, chatCopy=True, chatStack=True, chatMentions=True, lowFire=True, itemPhysics=True, fancy=True,
 fontAntialias=True, ownNametag=True, optifineCapes=True, tabPing=True, noHurtCam=True, dynamicFps=False, tntTimer=True, portalScreens=True,
 badgeNametags=True, fovNoSpeed=True, fovNoBow=True, legacyVboOn=True, confirmLeave=True, minimapRound=True, minimapRotate=True,
 minimapPlayers=True, minimapCoords=True, autoReconnect=True, tierTags=True, tierNametags=True, tierTab=True, pingNametags=True,
 tabGlowSelf=True, chatKeep=True, chatPersistent=True, lanAuto=True, chatPlaceholders=True, scoreboardNumbers=True, scoreboardBackground=True,
 lowHealthBeep=True, containerSearch=True, shulkerPeek=True, commandPalette=True, saturationBar=True, tooltipScroll=True, deathNotice=True,
 copyScreenshots=True, waypointsInWorld=True, replayEnabled=True, replayVideoEnabled=False, chatEmoji=True,
 hud={'minimap': {'enabled': True}, 'watermark': {'enabled': True}})
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
server = None
def cmd(s, pause=0.0):
    server.stdin.write(s + '\n'); server.stdin.flush()
    if pause: time.sleep(pause)

base = shlex.split(pathlib.Path(f'/home/claude/v4-output/logs/dry-run-{V["args"]}.txt').read_text())
JAVA = base[0]
P = 120  # platform y
def server_props(rd):
    lt = 'DEFAULT' if OLD else 'minecraft\\:normal'
    return (f'online-mode=false\nserver-ip=127.0.0.1\nserver-port={PORT}\nlevel-seed=20261116\nlevel-type={lt}\nspawn-protection=0\n'
            f'view-distance={rd}\nsimulation-distance=8\nmax-players=200\ngamemode=creative\nforce-gamemode=true\nallow-flight=true\n'
            'network-compression-threshold=256\nplayer-idle-timeout=0\nspawn-monsters=false\nmotd=join test\nsync-chunk-writes=false\n'
            'enable-command-block=false\nwhite-list=false\nenforce-whitelist=false\n')

def build_world():
    """Generate the busy world once: a glass lobby over normal terrain with named armor stands, villagers and animals."""
    global server
    shutil.rmtree(srv / 'world', ignore_errors=True)
    server = start([JAVA, '-Xms1G', '-Xmx2G', '-jar', 'server.jar', 'nogui'], 'server-gen', srv, stdin=subprocess.PIPE)
    waitfor(out / 'server-gen.log', 'Done (', proc=server, timeout=600)
    R = 24
    g = (lambda b: b) if OLD else (lambda b: 'minecraft:' + b)
    cmd('gamerule doDaylightCycle false', .2); cmd('gamerule doMobSpawning false', .2); cmd('gamerule spawnRadius 0', .2)
    cmd('gamerule doWeatherCycle false', .2) if not OLD else None
    cmd(f'setworldspawn 0 {P + 1} 0', .3)
    cmd(f'fill {-R} {P} {-R} {R} {P} {R} {g("glass")}', 1)
    for z in (-R, R):
        cmd(f'fill {-R} {P + 1} {z} {R} {P + 5} {z} ' + ('stained_glass 3' if OLD else 'minecraft:light_blue_stained_glass'), 1)
    for x in (-R, R):
        cmd(f'fill {x} {P + 1} {-R} {x} {P + 5} {R} ' + ('stained_glass 14' if OLD else 'minecraft:red_stained_glass'), 1)
    rng = random.Random(11)
    def pos(): return rng.uniform(-R + 2, R - 2), P + 1, rng.uniform(-R + 2, R - 2)
    for i in range(60):
        x, y, z = pos()
        if OLD: cmd('summon ArmorStand %.1f %d %.1f {CustomName:"Stand %d",CustomNameVisible:1}' % (x, y, z, i), .03)
        else: cmd('summon minecraft:armor_stand %.1f %d %.1f {CustomName:"Stand %d",CustomNameVisible:1b}' % (x, y, z, i), .03)
    for i in range(60):
        x, y, z = pos()
        cmd(('summon Villager %.1f %d %.1f {NoAI:1}' if OLD else 'summon minecraft:villager %.1f %d %.1f {NoAI:1b}') % (x, y, z), .03)
    for kind in (['Cow', 'Sheep', 'Pig'] if OLD else ['minecraft:cow', 'minecraft:sheep', 'minecraft:pig']):
        for i in range(20):
            x, y, z = pos()
            cmd(('summon %s %.1f %d %.1f {NoAI:1}' if OLD else 'summon %s %.1f %d %.1f {NoAI:1b}') % (kind, x, y, z), .03)
    time.sleep(3)
    cmd('save-all', 5); cmd('stop')
    server.wait(timeout=120); procs.remove(server)
    shutil.rmtree(srv / 'world-pristine', ignore_errors=True)
    shutil.copytree(srv / 'world', srv / 'world-pristine')

status = 'failed'
try:
    (srv / 'eula.txt').write_text('eula=true\n')
    props = server_props(o.rd)
    if o.pack:
        import hashlib as _h
        zp = W / 'packs' / o.ver / 'pack.zip'
        pport = PORT + 100
        start([sys.executable, '-I', '-m', 'http.server', str(pport), '--bind', '127.0.0.1', '--directory', str(zp.parent)], 'packsrv', W)
        props += f'resource-pack=http\\://127.0.0.1\\:{pport}/pack.zip\nresource-pack-sha1={_h.sha1(zp.read_bytes()).hexdigest()}\nrequire-resource-pack=false\n'
    (srv / 'server.properties').write_text(props)
    if not (srv / 'world-pristine').exists():
        build_world()
    shutil.rmtree(srv / 'world', ignore_errors=True)
    shutil.copytree(srv / 'world-pristine', srv / 'world')
    for f in ['ops.json', 'usercache.json', 'banned-players.json', 'whitelist.json']:
        if (srv / f).exists(): (srv / f).unlink()
    if o.looks == 'fake':
        start([sys.executable, '-I', '/home/claude/badgefix/tools/fakesrv.py', str(LOOKS_PORT), str(out / 'looks.log'), str(o.lat)], 'fakesrv', W)
    if o.mode == 'mp':
        server = start(['taskset', '-c', '0-1', JAVA, '-Xms1G', '-Xmx2G', '-jar', 'server.jar', 'nogui'], 'server', srv, stdin=subprocess.PIPE)
        waitfor(out / 'server.log', 'Done (', proc=server)
        if o.bots:
            bport, bv = PORT, V['mc']
            if o.ver == '263':
                # The bot library stops at 26.1: bots talk 1.21.11 to ViaProxy, which talks 26.3 to the server.
                bport, bv = PORT + 10, '1.21.11'
                (W / 'viaproxy/run').mkdir(parents=True, exist_ok=True)
                via = start(['taskset', '-c', '0-1', JAVA, '-Xmx1G', '-jar', str(W / 'viaproxy/ViaProxy.jar'), 'cli', '--bind-address', f'127.0.0.1:{bport}',
                             '--target-address', f'127.0.0.1:{PORT}', '--target-version', '26.3', '--auth-method', 'NONE'], 'viaproxy', W / 'viaproxy/run')
                waitfor(out / 'viaproxy.log', 'started', proc=via, timeout=120)
            bargs = ['taskset', '-c', '0-1', 'node', str(W / 'bots.js'), str(bport), str(o.bots), bv]
            bots = start(bargs, 'bots', W)
            waitfor(out / 'bots.log', 'BOTS STARTED', proc=bots, timeout=180)
            time.sleep(2)
            colors = ['red', 'blue', 'green', 'yellow']
            for c in colors:
                if OLD: cmd(f'scoreboard teams add {c}'); cmd(f'scoreboard teams option {c} color {c}')
                else: cmd(f'team add {c}'); cmd(f'team modify {c} color {c}')
            cmd('scoreboard objectives add kills dummy' + (' Kills' if OLD else '')); cmd('scoreboard objectives setdisplay sidebar kills')
            rng = random.Random(5)
            for i in range(o.bots):
                n = f'Bot_{i:03d}'
                cmd(f'tp {n} {rng.uniform(-20, 20):.1f} {P + 1} {rng.uniform(-20, 20):.1f}', .02)
                cmd((f'scoreboard teams join {colors[i % 4]} {n}' if OLD else f'team join {colors[i % 4]} {n}'), .01)
                if i < 12: cmd(f'scoreboard players set {n} kills {i * 3}')
    # ---- client ----
    game.mkdir(parents=True, exist_ok=True)
    for d in ['mods', 'saves', 'logs', 'config/arctic', 'arctic', 'replays']:
        shutil.rmtree(game / d, ignore_errors=True)
    for f in ['config/arctic.json']:
        if (game / f).exists(): (game / f).unlink()
    (game / 'mods').mkdir()
    if o.arctic: shutil.copy2(o.arctic, game / 'mods/arctic.jar')
    if o.polarium: shutil.copy2(o.polarium, game / 'mods/polarium.jar')
    if o.perf and not OLD:
        src = pathlib.Path('/home/claude/fl-run/data/profiles/default/instances') / V['inst'] / 'minecraft/mods'
        for jar in src.glob('*.jar'):
            if jar.name not in ('arctic-mod.jar', 'polarium-arctic.jar'): shutil.copy2(jar, game / 'mods' / jar.name)
    if o.mode == 'sp':
        (game / 'saves').mkdir(exist_ok=True)
        shutil.copytree(srv / 'world-pristine', game / 'saves/joinworld')
    opts = f'renderDistance:{o.rd}\nguiScale:{1 if o.size != "854x480" else 2}\nmaxFps:260\nfullscreen:false\npauseOnLostFocus:false\nenableVsync:false\n'
    opts += 'soundCategory_master:0.0\n' if OLD else 'soundCategory_master:0.0\nsimulationDistance:8\nonboardAccessibility:false\ntutorialStep:none\nskipMultiplayerWarning:true\njoinedFirstServer:true\ninactivityFpsLimit:"minimized"\n'
    opts += 'fancyGraphics:true\nao:2\n' if OLD else 'graphicsMode:1\nao:true\n'
    (game / 'options.txt').write_text(opts)
    if o.arctic:
        cfg = dict(BURAK); cfg.update(json.loads(o.cfg))
        (game / 'config').mkdir(exist_ok=True); (game / 'config/arctic.json').write_text(json.dumps(cfg))
    a = [x for x in base if not x.startswith(('-Darctic.selftest=', '-Darctic.preload=', '-javaagent:', '-Xms', '-Xmx'))]
    if '-DFabricMcEmu=' in a:
        i = a.index('-DFabricMcEmu='); a[i:i + 2] = ['-DFabricMcEmu= net.minecraft.client.main.Main ']
    for key, val in [('--username', NAME), ('--uuid', uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:' + NAME).encode()).digest(), version=3).hex),
                     ('--gameDir', str(game)), ('--width', o.size.split('x')[0]), ('--height', o.size.split('x')[1]), ('--accessToken', '0'), ('--userType', 'legacy'),
                     ('--userProperties', '{}'), ('--xuid', '0'), ('--clientId', '0')]:
        if key in a: a[a.index(key) + 1] = val
    action = f'mp:127.0.0.1:{PORT}' if o.mode == 'mp' else 'sp:joinworld'
    delay = o.delay or (30 if OLD else 45)
    tmp = W / 'tmp'; tmp.mkdir(exist_ok=True)
    jv = ['-Xms512M', '-Xmx2048M', '-javaagent:' + str(W / 'agent/joinagent.jar') + f'={out};{action};{delay * 1000};5',
          '-Djava.io.tmpdir=' + str(tmp)]
    jv += ['-Xloggc:' + str(out / 'gcjvm.log'), '-XX:+PrintGCDetails', '-XX:+PrintGCApplicationStoppedTime'] if OLD else ['-Xlog:gc*:file=' + str(out / 'gcjvm.log')]
    if o.looks == 'fake': jv.append(f'-Darctic.cosmetics.url=http://127.0.0.1:{LOOKS_PORT}')
    elif o.looks == 'off': jv.append('-Darctic.cosmetics.url=http://127.0.0.1:9')
    a[1:1] = jv + shlex.split(o.extra)
    if o.ver == '263': a += ['--graphicsBackend', 'opengl']
    (out / 'command.json').write_text(json.dumps(a, indent=1))
    display = ':' + str(700 + int(hashlib.md5(o.label.encode()).hexdigest(), 16) % 200)
    start(['Xvfb', display, '-screen', '0', o.size + 'x24', '-nolisten', 'tcp'], 'xvfb', W)
    time.sleep(1)
    env = dict(os.environ, DISPLAY=display, LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='4', TMPDIR=str(tmp))
    if o.ver == '263': env.update(SDL_VIDEO_FORCE_EGL='1', MESA_GL_VERSION_OVERRIDE='4.6', MESA_GLSL_VERSION_OVERRIDE='460')
    if o.noop: env['GALLIUM_NOOP'] = '1'
    env.pop('ARCTIC_SESSION', None); env.pop('ARCTIC_TOKEN', None)
    client = start(['taskset', '-c', '2-7'] + a, 'client', game, env=env)
    waitfor(out / 'DONE', '', proc=client, timeout=420)
    status = 'ok'
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
    if (game / 'logs/latest.log').exists(): shutil.copy2(game / 'logs/latest.log', out / 'latest.log')
    print('RUN', o.label, status, flush=True)
