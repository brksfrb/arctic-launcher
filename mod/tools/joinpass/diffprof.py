"""diffprof.py <runA> <runB> <from_s> <to_s> [N] [--leaf]: inclusive ms/frame per method (non-GL, non-idle samples), B minus A."""
import sys, collections, analyze
def spec(x, a, b):
    p = x.split(':')
    return (p[0], float(p[1]), float(p[2])) if len(p) == 3 else (x, a, b)
a_s, b_s = float(sys.argv[3]), float(sys.argv[4])
SA, SB = spec(sys.argv[1], a_s, b_s), spec(sys.argv[2], a_s, b_s)
a_run, b_run = SA[0], SB[0]
N = int(sys.argv[5]) if len(sys.argv) > 5 and not sys.argv[5].startswith('--') else 40
LEAF = '--leaf' in sys.argv
def prof(sp):
    run, a_s, b_s = sp
    d, ev, frames, gc = analyze.load(run)
    t0 = ev['ingame']; a, b = t0 + a_s * 1000, t0 + b_s * 1000
    c = collections.Counter(); n = 0
    for line in open(d / 'stacks.txt'):
        p = line.rstrip('\n').split(';'); t = float(p[0])
        if not (a <= t < b): continue
        n += 1; fr = p[1:]
        if not fr or fr[0].startswith(analyze.GL) or fr[0].startswith(analyze.IDLE): continue
        if LEAF: c[fr[0]] += 1
        else:
            for f in set(fr): c[f] += 1
    nf = len([f for f in frames if a < f[0] <= b])
    k = (b - a) / n / nf
    return {m: v * k for m, v in c.items()}
A, B = prof(SA), prof(SB)
rows = sorted(set(A) | set(B), key=lambda m: -(B.get(m, 0) - A.get(m, 0)))
print(f'ms/frame  {a_run} -> {b_run}  [{a_s},{b_s}) s')
for m in rows[:N]:
    print(f'  {A.get(m,0):6.3f} {B.get(m,0):6.3f}  {B.get(m,0)-A.get(m,0):+6.3f}  {m}')
