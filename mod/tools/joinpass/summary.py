"""summary.py <prefix>...: mean over reps per (version, arm, mode) of join ms, first-60s FPS, 1% low, time to stable, client cpu."""
import sys, pathlib, re, statistics, collections
sys.argv += []
import analyze
W = pathlib.Path('/home/claude/joinpass-work/runs')
groups = collections.OrderedDict()
for prefix in sys.argv[1:]:
    for d in sorted(W.glob(prefix + '-*')):
        m = re.match(r'(.+)-(189|1211|263)-(\w+)-(mp|sp)(\d+)$', d.name)
        if not m or not (d / 'DONE').exists(): continue
        s = analyze.stats(d.name)
        if 'error' in s: continue
        groups.setdefault((m.group(1), m.group(2), m.group(3), m.group(4)), []).append(s)
print(f"{'set':10s} {'ver':5s} {'arm':4s} {'mode':4s} n  join_ms(each)            fps60  fps10  1%low  stable_s  cpu_ms  gc60")
for (p, v, a, mode), ss in groups.items():
    f = lambda k: statistics.mean(x[k] for x in ss)
    joins = '/'.join(f"{x['join']:.0f}" for x in ss)
    print(f"{p:10s} {v:5s} {a:4s} {mode:4s} {len(ss)}  {f('join'):6.0f} ({joins:14s}) {f('fps'):6.1f} {f('fps10'):6.1f} {f('low1'):6.1f} {f('stable'):6.1f}  {f('cpu'):6.2f} {f('gc'):5.0f}")
