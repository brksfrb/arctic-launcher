#!/usr/bin/env python3
"""Check actual GL snapshot work in two identical open-chat benchmark scenes."""
import re
import sys
from pathlib import Path


def metric(log, name, field):
    line = re.search(r'^FPS189 PROBE ' + re.escape(name) + r' .*$', log, re.M)
    if line is None:
        raise SystemExit('Missing open-chat probe: ' + name)
    return float(re.search(re.escape(field) + r'=([0-9.]+)', line[0])[1])


def main():
    if len(sys.argv) != 3:
        raise SystemExit('compare_probe.py before.log after.log')
    before, after = [Path(p).read_text() for p in sys.argv[1:]]
    old = metric(before, 'font.prepare', 'calls_per_frame')
    new = metric(after, 'font.prepare', 'calls_per_frame')
    old_glyphs = metric(before, 'font.glyph', 'calls_per_frame')
    new_glyphs = metric(after, 'font.glyph', 'calls_per_frame')
    # Use method counts, not FPS, so contention on the test host cannot masquerade as a fix.
    if old_glyphs <= 0 or abs(new_glyphs / old_glyphs - 1) > 0.01:
        raise SystemExit('Glyph workload differs by more than 1%; reject this comparison')
    if old <= 0 or new / old > 0.75:
        raise SystemExit('Expected at least 25% fewer driver snapshots in the shadow-heavy open-chat scene')
    print(f'PASS: snapshots/frame {old:.2f} -> {new:.2f}; '
          f'glyphs/frame {old_glyphs:.2f} -> {new_glyphs:.2f}')
    print('Three driver reads per snapshot. Wall-frame speedup still requires a GPU benchmark.')


if __name__ == '__main__':
    main()
