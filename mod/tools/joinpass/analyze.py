"""analyze.py <run>... [--stacks] [--window S]: join time, first-60s FPS, 1% lows, time to stable, client CPU, GC per run."""
import sys, evs, pathlib, collections, statistics, csv
W = pathlib.Path('/home/claude/joinpass-work/runs')
args = [a for a in sys.argv[1:] if not a.startswith('--')]
STACKS = '--stacks' in sys.argv
WIN = 60.0

def load(run):
    d = W / run
    ev = evs.events(d / 'events.csv')
    frames = [(float(r['ms']), float(r['dt_ms']), float(r['cpu_ms'])) for r in csv.DictReader(open(d / 'frames.csv'))]
    gc = [(float(r['ms']), float(r['gc_ms_total']), float(r['heap_used_mb'])) for r in csv.DictReader(open(d / 'gc.csv'))]
    return d, ev, frames, gc

def stats(run):
    d, ev, frames, gc = load(run)
    if 'ingame' not in ev: return dict(run=run, error='never in game')
    t0, act = ev['ingame'], ev['action']
    win = [f for f in frames if t0 < f[0] <= t0 + WIN * 1000]
    tail = [f for f in frames if t0 + WIN * 1000 < f[0] <= t0 + 75000]
    dts = sorted(f[1] for f in win)
    avg = len(win) / (sum(dts) / 1000)
    p99 = dts[int(len(dts) * 0.99)]
    worst1 = dts[int(len(dts) * 0.99):]
    low1 = 1000 / (sum(worst1) / len(worst1))
    # Time to stable: per-second FPS; reference = median of the 60-75 s seconds; stable = from the first second after
    # which every second reaches 85 % of the reference.
    def per_sec(fr, start, n):
        b = [0] * n
        for t, dt, c in fr:
            k = int((t - start) / 1000)
            if 0 <= k < n: b[k] += 1
        return b
    secs = per_sec(frames, t0, 75)
    ref = statistics.median(secs[60:75]) if len(secs) >= 75 else statistics.median(secs[-10:])
    stable = 0
    for k in range(60):
        if secs[k] < 0.85 * ref: stable = k + 1
    cpu = sum(f[2] for f in win) / len(win)
    g = [x for x in gc if t0 <= x[0] <= t0 + WIN * 1000]
    gcms = (g[-1][1] - g[0][1]) if len(g) > 1 else 0
    gj = [x for x in gc if act <= x[0] <= t0]
    gcjoin = (gj[-1][1] - gj[0][1]) if len(gj) > 1 else 0
    first10 = [f for f in frames if t0 < f[0] <= t0 + 10000]
    fps10 = len(first10) / (sum(f[1] for f in first10) / 1000) if first10 else 0
    spikes = sum(1 for f in win if f[1] > 100)
    j10, j60, jst = javams(run, 0, 10), javams(run, 0, 60), javams(run, 60, 70)
    return dict(run=run, j10=j10, j60=j60, jst=jst, join=t0 - act, fps=avg, fps10=fps10, low1=low1, p99=p99, worst=dts[-1], stable=stable, ref=ref,
                cpu=cpu, gc=gcms, gcjoin=gcjoin, spikes=spikes)

GL = ('org.lwjgl.opengl.', 'org.lwjgl.system.JNI', 'org.lwjgl.sdl.', 'org.lwjgl.glfw.', 'org.lwjgl.opengl.LinuxContext')
IDLE = ('java.lang.Thread.sleep', 'java.lang.Thread.yield', 'jdk.internal.misc.Unsafe.park', 'java.lang.Object.wait', 'java.lang.Thread.onSpinWait')
def javams(run, a_s, b_s):
    """Client-thread ms per frame spent outside GL/driver calls and idle waits, between a_s and b_s after in-game."""
    d, ev, frames, gc = load(run)
    t0 = ev['ingame']; a, b = t0 + a_s * 1000, t0 + b_s * 1000
    n = busy = 0
    for line in open(d / 'stacks.txt'):
        i = line.find(';'); t = float(line[:i])
        if not (a <= t < b): continue
        n += 1
        top = line[i + 1:].split(';', 1)[0]
        if not top.startswith(GL) and not top.startswith(IDLE): busy += 1
    fr = [f for f in frames if a < f[0] <= b]
    if not n or not fr: return float('nan')
    return (b - a) * busy / n / len(fr)

def stacks(run, top=25):
    d, ev, frames, gc = load(run)
    act, t0 = ev['action'], ev.get('ingame', ev['action'])
    phases = {'join': (act, t0), 'first60': (t0, t0 + WIN * 1000)}
    for name, (a, b) in phases.items():
        leaf = collections.Counter(); incl = collections.Counter(); n = 0
        for line in open(d / 'stacks.txt'):
            p = line.rstrip('\n').split(';')
            t = float(p[0])
            if not (a <= t <= b): continue
            n += 1
            fr = p[1:]
            if fr: leaf[fr[0]] += 1
            seen = set()
            for f in fr:
                if ('arcticlauncher' in f or 'polarium' in f) and f not in seen:
                    seen.add(f); incl[f] += 1
        print(f'--- {run} {name}: {n} samples ({(b - a) / 1000:.1f} s)')
        print('  leaf:')
        for f, c in leaf.most_common(top): print(f'   {100 * c / max(n, 1):5.1f}% {f}')
        print('  arctic/polarium inclusive:')
        for f, c in incl.most_common(top): print(f'   {100 * c / max(n, 1):5.1f}% {f}')

if __name__ == "__main__":
    print('run                          join_ms  fps60  fps10  1%low  p99ms  worst  stable_s  ref  cpu_ms  gc60ms gcjoin spikes>100  javams0-10 0-60 60-70')
    for r in args:
        s = stats(r)
        if 'error' in s: print(r, s['error']); continue
        print(f"{r:28s} {s['join']:7.0f} {s['fps']:6.1f} {s['fps10']:6.1f} {s['low1']:6.1f} {s['p99']:6.1f} {s['worst']:6.0f} {s['stable']:8d} {s['ref']:4.0f} {s['cpu']:7.2f} {s['gc']:7.0f} {s['gcjoin']:6.0f} {s['spikes']:5d}  {s['j10']:6.2f} {s['j60']:6.2f} {s['jst']:6.2f}")
        if STACKS: stacks(r)
