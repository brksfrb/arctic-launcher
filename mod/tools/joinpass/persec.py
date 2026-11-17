"""persec.py <run>...: per-second FPS (and client cpu ms/frame) from 5 s before in-game to +75 s."""
import sys, evs, csv, pathlib
W = pathlib.Path('/home/claude/joinpass-work/runs')
for run in sys.argv[1:]:
    ev = evs.events(W / run / 'events.csv')
    t0 = ev['ingame']; b = {}; c = {}
    for r in csv.DictReader(open(W / run / 'frames.csv')):
        k = int((float(r['ms']) - t0) // 1000)
        if -5 <= k < 75: b[k] = b.get(k, 0) + 1; c[k] = c.get(k, 0) + float(r['cpu_ms'])
    print(run, ' '.join(f'{b.get(k,0)}' for k in range(-5, 75)))
    print(' cpu%', ' '.join(f'{c.get(k,0)/10:.0f}' for k in range(-5, 75)))
