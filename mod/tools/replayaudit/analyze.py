"""analyze.py <run dir> [--top N] [--phase NAME]: per audit phase (from "audit: PHASE x BEGIN/END @ms" lines in
client.log): frames, frame-time percentiles, render-thread CPU per frame, CPU per thread, GC, and where the sampled
stacks were (share of render-thread samples inside replay code, top replay frames, top leaf frames)."""
import sys, re, pathlib, collections, statistics
d = pathlib.Path(sys.argv[1]); top = int(sys.argv[sys.argv.index('--top') + 1]) if '--top' in sys.argv else 8
only = sys.argv[sys.argv.index('--phase') + 1] if '--phase' in sys.argv else None
log = (d / 'client.log').read_text(errors='replace')
phases = []; begin = {}
for m in re.finditer(r'audit: (PHASE (\S+) (BEGIN|END)|SEEK (BEGIN|END) to=(\S+)|OPEN (BEGIN|END))[^@\]]*@(\d+)', log):
    t = int(m.group(7))
    name = m.group(2) or ('seek ' + m.group(5) if m.group(5) else 'open'); edge = m.group(3) or m.group(4) or m.group(6)
    if edge == 'BEGIN': begin[name] = t
    elif name in begin: phases.append((name, begin.pop(name), t))
frames = [tuple(map(float, l.split(','))) for l in (d / 'frames.csv').read_text().splitlines()[1:]]
cpu = [l.split(',') for l in (d / 'cpu.csv').read_text().splitlines()[1:]]
gc = [tuple(map(float, l.split(','))) for l in (d / 'gc.csv').read_text().splitlines()[1:]]
stacks = []
for l in (d / 'stacks.txt').read_text(errors='replace').splitlines():
    p = l.split('\t')
    if len(p) == 4: stacks.append((float(p[0]), p[1], p[2], p[3].split(';')))
render = collections.Counter(s[1] for s in stacks).most_common(1)[0][0] if stacks else '?'
# the render thread is the one sampled regardless of state: it has the most samples
def pct(v, q): v = sorted(v); return v[min(len(v) - 1, int(len(v) * q))] if v else 0
REPLAY = ('com.arcticlauncher.client.replay.', 'com.arcticlauncher.legacy.replay.', 'com.arcticlauncher.mod.replay.')
def group(name): return re.sub(r'[ #-]*\d+$', '', name)
for name, a, b in phases:
    if only and name != only: continue
    f = [x for x in frames if a <= x[0] <= b]; secs = (b - a) / 1000.0
    print(f'== {name}: {secs:.1f} s')
    if f:
        dt = [x[1] for x in f]; cp = [x[2] for x in f]
        print(f'   frames {len(f)} ({len(f) / secs:.1f}/s)  frame ms mean {statistics.mean(dt):.1f} p50 {pct(dt, .5):.1f} p99 {pct(dt, .99):.1f} max {max(dt):.0f}'
              f'  render-thread CPU ms/frame {statistics.mean(cp):.2f}  CPU ms/s {sum(cp) / secs:.0f}')
    per = collections.Counter()
    for t, th, v in cpu:
        if a <= float(t) <= b: per[group(th)] += float(v)
    print('   CPU ms/s by thread: ' + ', '.join(f'{k} {v / secs:.0f}' for k, v in per.most_common(top)))
    g = [x for x in gc if a <= x[0] <= b]
    if len(g) > 1: print(f'   GC {g[-1][1] - g[0][1]:.0f} ms ({(g[-1][1] - g[0][1]) / secs:.1f} ms/s), heap {min(x[2] for x in g):.0f}-{max(x[2] for x in g):.0f} MB')
    rs = [s for s in stacks if a <= s[0] <= b and s[1] == render]
    inrep = [s for s in rs if any(fr.startswith(REPLAY) for fr in s[3])]
    if rs:
        print(f'   render-thread samples {len(rs)}, inside replay code {len(inrep)} ({100.0 * len(inrep) / len(rs):.1f}%)')
        firsts = collections.Counter()
        for s in inrep:
            entry = [fr for fr in s[3] if fr.startswith(REPLAY)]
            firsts[entry[0].split('.', 3)[-1] + ' <- ' + entry[-1].split('.', 3)[-1]] += 1
        for k, v in firsts.most_common(top): print(f'      {100.0 * v / len(rs):5.1f}%  {k}')
        leaf = collections.Counter(s[3][0] for s in inrep)
        for k, v in leaf.most_common(5): print(f'      leaf {100.0 * v / len(rs):5.1f}%  {k}')
    oth = collections.Counter(); osamp = collections.defaultdict(collections.Counter)
    for s in stacks:
        if a <= s[0] <= b and s[1] != render and any(fr.startswith(REPLAY) for fr in s[3]):
            entry = [fr for fr in s[3] if fr.startswith(REPLAY)]
            oth[group(s[1])] += 1; osamp[group(s[1])][entry[0].split('.', 3)[-1]] += 1
    for k, v in oth.most_common(top):
        print(f'   {k}: {v} runnable samples in replay code (~{v * 5 / secs:.0f} ms/s): ' + ', '.join(f'{n} {c}' for n, c in osamp[k].most_common(4)))
