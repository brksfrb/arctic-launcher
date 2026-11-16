"""window.py <run> <from_s> <to_s> [N] [--incl PATTERN]: client-thread profile between times relative to in-game (leaf + inclusive Arctic/Polarium + inclusive top-level game phases)."""
import sys, evs, csv, pathlib, collections
W = pathlib.Path('/home/claude/joinpass-work/runs')
run, a, b = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]); N = int(sys.argv[4]) if len(sys.argv) > 4 and not sys.argv[4].startswith('--') else 30
pat = sys.argv[sys.argv.index('--incl') + 1] if '--incl' in sys.argv else None
ev = evs.events(W / run / 'events.csv')
t0 = ev['ingame']; leaf = collections.Counter(); incl = collections.Counter(); n = 0
for line in open(W / run / 'stacks.txt'):
    p = line.rstrip('\n').split(';'); t = (float(p[0]) - t0) / 1000
    if not (a <= t < b): continue
    n += 1; fr = p[1:]
    if fr: leaf[fr[0]] += 1
    for f in set(fr):
        if pat is None or pat in f: incl[f] += 1
print(f'{run} [{a},{b}) s: {n} samples')
print(' leaf:'); [print(f'  {100*c/n:5.1f}% {f}') for f, c in leaf.most_common(N)]
print(' inclusive:'); [print(f'  {100*c/n:5.1f}% {f}') for f, c in incl.most_common(N)]
