"""Shared: events of a run, with 'ingame' = the first null screen after the last loading-terrain screen (within 20 s of the action)."""
import csv
TERRAIN = ('class_483', 'class_3928', 'LevelLoadingScreen', 'ReceivingLevelScreen', 'class_434')
def events(path):
    rows = [(float(r['ms']), r['event'], r['detail']) for r in csv.DictReader(open(path))]
    ev = {}
    for t, e, d in rows: ev.setdefault(e, t)
    act = ev.get('action')
    if act is None: return ev
    after = [(t, e, d) for t, e, d in rows if t >= act and t <= act + 60000]
    terr = [t for t, e, d in after if e == 'screen' and d.endswith(TERRAIN)]
    start = terr[-1] if terr else act
    nulls = [t for t, e, d in after if e == 'screen' and d == 'null' and t >= start]
    if nulls: ev['ingame'] = nulls[0]
    else: ev.pop('ingame', None)
    return ev
