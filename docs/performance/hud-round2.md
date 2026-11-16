# HUD round 2 — 2026-11-15/16 UTC

Base: `origin/main` at `74fd5d8f`. Branch: `perf/hud-cost`.

Unchanged text now retains its parts, content key, icon lookup and own-label
word tokens. Token widths still come from the current renderer, so font resource
reloads keep the original per-token positioning behavior. FPS and ping retain unchanged value strings; clock and date
format only at local minute/day boundaries. Chroma uses scalar color values
instead of allocating seven arrays per color. Mutable slot changes, preview changes, changed values, font mode
and GUI scale invalidate the relevant caches. Layout signatures use two reused
arrays; a changed drawing reuses its saved rectangle. Disabled widgets do no
content/size/draw work. Tab-hidden widgets keep their layout footprint but skip
drawing; recomputing their changing width remains necessary to preserve the
positions of neighboring widgets.

Legacy pixel-rounded fills, outlines and discs submit the original rectangles
in one immediate-mode quad batch per shape. Text order and widget layers stay
unchanged; cross-widget text reordering was not introduced. Modern adapters
retain Minecraft's existing batching and recorded GUI drawing path.

## Shared CPU allocation workload

24 stable text widgets, 20,000 frames per pass, three passes, OpenJDK 25.
The last two passes are reported. Gfx is a lightweight stand-in; these are not
Minecraft frame times or the older reported 1.9 ms HUD overhead.

| Metric | Before | After |
| --- | ---: | ---: |
| Allocated bytes/frame | 5,184 | 0 |
| CPU ns/frame, pass 2 | 1,864 | 562 |
| CPU ns/frame, pass 3 | 2,166 | 561 |

The shared code is identical on 1.8.9, 1.21.11, 26.2 and 26.3. Real adapter
measurements are recorded separately below; do not extrapolate the allocation
workload to GPU time.

## Validation

Core regressions cover stable-content identity, in-place slot mutation,
preview/value changes, own-label colors/spacing, GUI-scale remeasurement,
local minute/day boundaries (including fractional-minute UTC offsets and
clock rollback), 36,864 HSV color comparisons with Java's reference conversion,
layout invalidation, disabled and Tab-hidden widgets, unchanged rectangle
coordinates and closing a failed fill batch.

The full core suite exposed an existing badge test failure, reproduced on an
untouched `origin/main` worktree. Its fake server returned the old badge after
accepting a save; its error assertion also depended on another test loading
English first. The fixture now retains the saved choice and compares the
translated message after waiting for it. No production badge code changed.

Builds and adapter tests run for the four requested targets; core tests run
separately because adapter builds do not include the core test source set.

For direct CPU timing use the separate [HUD agent](../../mod/tools/hudbench/README.md).

## Direct adapter CPU timings

Baseline production jars were built in a separate `origin/main` worktree;
modified jars were built from this branch. Both run with Polarium's existing
terrain bench, RD8, the same default Loom window/GUI scale, vanilla menu style,
pixel-rounded radius-2 panels, and the same enabled FPS/ping/coordinates/
clock/date/CPS/memory/GPU/speed/direction/biome/server settings. Other default
widgets remain present. Replay and video capture are disabled for both jars
so their readback costs cannot be mistaken for HUD costs.

The timing agent measures `Hud.render` CPU submission after 20 seconds of HUD
rendering, retaining up to 1,000 calls. One run per cell; this shared CPU-only
host has run-to-run variability. Baseline and modified runs were serial.
The 1.8.9 view is frozen; modern views rotate as the terrain bench specifies.
26.3 uses Minecraft's software Vulkan fallback. These are neither GPU times
nor full-mod overhead, and do not reproduce the older 1.9 ms measurement.

| Target | Before mean ms | After mean ms | p95 before → after ms | Calls before / after |
| --- | ---: | ---: | ---: | ---: |
| 1.8.9 | 0.454 | 0.315 | 0.571 → 0.362 | 992 / 887 |
| 1.21.11 | 0.181 | 0.131 | 0.227 → 0.163 | 701 / 867 |
| 26.2 | 0.168 | 0.108 | 0.213 → 0.146 | 714 / 861 |
| 26.3 | 0.179 | 0.127 | 0.225 → 0.164 | 654 / 790 |

These final-code runs show about 28–35% less CPU submission time. Earlier
optimization passes were noisier and included flat/slower modern samples;
there is no claim that the historical 1.9 ms full-mod overhead was reproduced
or eliminated, or that GPU time fell by the same percentage. Resource-pack
font-width queries remain in own-label drawing to preserve its existing
positioning behavior across reloads.

Final checks: all 31 Fabric/Legacy Fabric target builds and Forge 1.8.9
built successfully. The separate core suite ran 366 tests with zero failures
and two existing skips. The four requested adapter builds and their tests
also passed. No Rust source or release pack was changed.
