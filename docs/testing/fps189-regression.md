# Minecraft 1.8.9 FPS investigation — 2026-11-10

The reported Windows GPU regression is **not reproduced or explained by these measurements**. The first
valid v4/v5 pair was effectively tied on llvmpipe. Later, the *unchanged v4 jar* went from 25.22 to
124.48 ms/frame in the open-chat scene as this shared host became busy. Single-run ablation differences
therefore cannot establish causality. No 3–5× Windows recovery is claimed.

One small, measurable inefficiency was addressed: the unmerged sharp-font feature read the model-view,
projection and viewport separately for shadow and foreground layers. They now share a snapshot within
one string draw. Separate draws still read actual GL state, including direct transforms from other mods;
each layer still uses its own anchor for the perspective divide. The GUI atlas is initialized before its
pixel-scale divisor is used. Wrapped/RTL text's private line helper also starts a snapshot. Font layout,
glyph metrics, size buckets and texture ownership are preserved.

The source mitigation is `69a06461` on `fix/189-fps-regression-nametag`, based on
`origin/fix/nametag-sharp-font` (`a8164b3e`). The exact v5 integration is `a53542e6` on
`fix/189-fps-regression-v5`, based on `a3f920df`. The requested `fix/189-fps-regression` branch is based on
`origin/main` (`35f6682b`, which advanced since the reported `a1ed86d7`) and carries the external benchmark,
work-count regression check, and this report. Main does not contain the font readback implementation;
introducing that unmerged implementation into main solely to optimize it would add runtime work to main.
The v5 fix branch additionally bundles rebuilt Fabric and Forge 1.8.9 jars in `a9941672`. Only three
font class entries changed in each of those two target jars; every entry in the other 30 targets remains
identical to v5, and the external fixture is absent throughout. These are mitigation builds, not a verified
Windows FPS recovery. No other feature branch has an evidenced root-cause fix to port. Only `fix/*` branches are pushed.

## Build and bundle evidence

The exact requested revisions `8e9cc6fb`, `c5dba469`, and `a3f920df` were rebuilt for Legacy Fabric 1.8.9
using JDK 25, offline Gradle and two workers. All archive entry bytes matched the corresponding embedded
release jars reconstructed from `mod/dist/arctic-client.pack` (ZIP metadata/compression were ignored).
The November 8 origin branch currently points to the older `ab8e79a8`; the measured November 8 client is
the explicitly supplied `8e9cc6fb`, not that older remote tip.

All three builds embed the identical Polarium 1.8.9 Git blob
`f7e75d8b53c3d3d36be6aefe0abb3ed7f7cdcd40`. Neither v4 nor v5 bundles the unmerged
`polarium-189-cpu-perf` changes at `5974754`. That commit is not an ancestor of Polarium `origin/main`.
The bundled mixin list lacks the CPU branch's pipeline, face-bucket, opaque-cutout and vignette additions.
The local Windows artifacts `/home/claude/v4-output/Arctic Launcher.exe` and
`/home/claude/v5-output/Arctic Launcher.exe` both contain that entire identical jar (SHA-256
`0adf5bc86ccc101f7dbc72dc609fe6eef0da764859fadaa42c27efd24ae56cea`). Their source release packs also
match the corresponding output packs and occur intact inside the executables. Thus this check covers
those actual built executables as well as the repository. There is no v4-to-v5 loss of that patch. The skin-atlas guard changed the modern Polarium jars; the 1.8.9
jar did not change.

`PERSONAL_BUILD` is a launcher updater switch, not a client renderer/debug switch. v5's embedded 1.8.9 jar
contains no Test/Proof helper classes. Release preprocessing excludes development scenes and chat-copy
proofs. v4 contains older proof classes, but `LegacySelfTest` returns unless its explicit system property
is supplied. No proof/self-test property was supplied in these runs. No new per-frame logging or GL
validation layer was found. Startup-cache/preload agents are not used by the launcher's normal Java 8
path; the profiler runs also excluded those startup agents.

## Fixture and limits

Every command/process was pinned to CPUs `8-15,24-31`. No DRM/NVIDIA device or `vglrun` was available.
The renderer was `llvmpipe (LLVM 19.1.7, 256 bits)` under a dedicated Xvfb, with eight raster threads.
Absolute FPS has no bearing on Burak's GPU. The selected CPUs were heavily contended; observed host load
rose from roughly 60–70 to over 90. Other jobs were not stopped or modified. Some builds/checks overlapped
later runs, which are retained as raw observations, not causal performance estimates.

The external fixture uses offline `FPS189Offline`, a fresh flat world with seed 20261108, camera
(0.5,100,0.5), yaw/pitch 0, fixed noon/weather, a stone platform, glass wall, ten named armour stands and
100 chat lines. Teleport/chunk loading precedes placement, and summon commands fit the 1.8.9 100-character
packet limit. The scene was visually inspected before accepting results and after the font change.

Settings: 1280×720, render distance 4, GUI scale 2, VBOs off with Polarium, Fancy font/AA on, vsync off,
Dynamic FPS off, replay/video capture off, persistence/copy/timestamps/CTM on when available. Twenty seconds
of scene warmup precede 15 seconds measured with chat open; a five-second transition and refreshed chat
precede 15 seconds with chat closed. Closed-chat measurements include normal eventual chat fading. Each
run has its own game directory. Normal runtime tests use Legacy Fabric; the launch plan actually loaded
Fabric Loader 0.20.1. The profiles use Java 21.0.13; separate Java 8u202 comparisons are identified below.
The production Forge jar is built/checked separately, not GPU-benchmarked.

The fixture is injected into `LegacyHooks.runQueued`; it is never packed into release jars. Font reversion
uses the rebuilt v4 `LegacySmoothFont`, `ScaledGlyphs`, smooth-font mixin and `GlScaleMixin` together, with
the latter restored to the mixin configuration. Other ablations use their real config switches. The
`nodiag` case disables the per-frame heartbeat method, not the startup/shutdown diagnostics machinery.
The 15 FPS unfocused-cap trial and early trials with failed placement/summon commands are excluded.

## Profiles and bounded method costs

JFR `profile` recordings last 35 seconds, starting 50 seconds after process launch. The first v4/v5
recordings include the full 15-second open-chat interval and about 11.0/10.1 seconds of closed chat,
respectively, plus transition/warmup. Each had 448 client-thread samples:

| Client-thread leaf | v4 | v5 |
| --- | ---: | ---: |
| `GL11.nglDrawArrays` | 67.19% | 66.52% |
| `LinuxContextImplementation.nSwapBuffers` | 13.39% | 14.51% |
| `GL11.nglCallList` | 2.23% | 2.46% |
| `GL11.nglTexSubImage2D` | 1.34% | 1.34% |

None of the suspect Java methods dominated either profile. Timestamp formatting had one sample around
the between-phase chat refill; it is not performed for every rendered line. Host-delayed startup means the after recording includes about 11.3 seconds of open-chat measurement,
plus warmup, and no closed-chat interval. Its 456 client samples were again dominated by `nglDrawArrays`
(57.02%) and swap (13.38%); its different coverage prevents a direct percentage comparison. These are sample proportions,
not isolated GPU-pass timings. The first recordings had one GC pause each: v4 17.49 ms and v5 12.07 ms;
this is not evidence of a new continuous GC problem.

A separate timed probe counts client-thread calls only during the sample intervals. The open-chat results
were:

| Work | Original v5 | Integrated mitigation |
| --- | ---: | ---: |
| Font GL snapshots/frame (three reads each) | 84.01 | 51.51 |
| Snapshot method time, ms/frame | 0.086986 | 0.041884 |
| Glyph draws/frame | 1242.90 | 1243.01 |
| Diagnostics heartbeat, ms/frame | 0.000766 | 0.001129 |
| Persistence restore check, ms/frame | 0.000334 | 0.000351 |
| Timestamp formatting / archive additions / copy clicks | zero | zero |

This is a 38.7% reduction in snapshot calls at the same glyph workload, and roughly 0.045 ms/frame saved
in that method on this software driver. It does not explain a several-millisecond Windows regression.
The before/after client CPU time in these instrumented runs was 4.20/3.96 ms/frame; their wall time is
mostly elsewhere. Probe overhead means their FPS is excluded from the normal result table.

CTM work is at mesh generation/model lookup, not neighbour walking for every visible glass block each
frame. Its client-thread `begin` calls were zero in the original steady probe and about 4.7/frame
(0.000427 ms/frame) in the fixed probe while remaining work was processed; meshing workers are outside
this client-thread timing scope. Persistent history writes occur synchronously when a message arrives,
so a chat burst can stutter. The 100-line insertion log spans ranged from tens to over 100 ms in these
runs; this is a separate event cost, not a steady per-frame write. It was not established as Burak's
reported sustained FPS cause, and persistence was not changed speculatively.

## Validation and artifacts

`mod/tools/fps189/` contains the reusable offline fixture, release-pack extractor, minimal bytecode
injector, timed probe, runner and `compare_probe.py`. The work-count regression check passes: at least
25% fewer snapshots with glyph work unchanged within 1%. All five legacy targets compile and pass mixin
checks (1.8.9, 1.9.4, 1.10.2, 1.11.2, 1.12.2). All three existing `TextPixelScaleTest` cases pass. Forge 1.8.9 builds successfully and its Forge-owned target check passes. The
integrated v5 1.8.9 production jar has no benchmark fixture, and its changes are confined to the font
implementation/mixin (plus compiler-generated inner-class metadata).

Raw logs, JFR recordings, screenshots, reconstructed baseline jars and fixed jars are retained outside
build directories at `/home/claude/fps189-evidence/`. In particular: `final-v4-1.jfr`, `final-v5-1.jfr`,
`after-fixed-1.jfr`, `probe-v5.log`, `probe-fixed.log`, and the before/after scene screenshots. The CSV
beside this report preserves frame counts and mean/median/p95/p99 for every accepted run. Generated
worktree build outputs and local Gradle caches are removed after recording results; shared Gradle caches
and other jobs' outputs are retained.

A Windows same-scene comparison is still needed to close the actual regression: record the F3 renderer,
loader/OptiFine/Patcher/runtime and replay settings, test Fancy off, and profile v4/v5 on that same setup.
The mitigation should be tested there before calling the reported FPS loss fixed.

## Complete frame-time observations

Mean **wall milliseconds per frame**, open/closed chat, in chronological run order. These rows are raw
observations on a contended software renderer, **not a performance ranking**. Repeated unchanged baselines
show that run-to-run load overwhelms the feature effects. See [the CSV](fps189-results.csv) for counts,
medians and tails. Probe runs are excluded.

| Build / variant | Java | Open chat, ms | Closed chat, ms |
| --- | --- | ---: | ---: |
| v5 baseline 1 | 21.0.13 | 25.30 | 24.81 |
| v4 baseline 1 | 21.0.13 | 25.22 | 24.21 |
| v5 with complete legacy-font reversion | 21.0.13 | 28.76 | 26.95 |
| v5 heartbeat no-op | 21.0.13 | 18.29 | 13.09 |
| v5 CTM off | 21.0.13 | 29.10 | 40.58 |
| v5 persistence off | 21.0.13 | 43.37 | 47.72 |
| v5 chat copy off | 21.0.13 | 66.09 | 50.05 |
| v5 timestamps off | 21.0.13 | 42.97 | 49.94 |
| v5 baseline 2 | 21.0.13 | 59.37 | 50.03 |
| v4 baseline 2 | 21.0.13 | 124.48 | 86.40 |
| v5 baseline 3 | 21.0.13 | 71.68 | 61.12 |
| v5 mitigation 1 | 21.0.13 | 65.75 | 65.06 |
| Nov 8 exact 8e9cc6fb | 21.0.13 | 111.06 | 127.60 |
| v5 mitigation 2 | 21.0.13 | 90.65 | 107.08 |
| v5 baseline 4 | 21.0.13 | 159.36 | 111.15 |
| v4 Java 8 | 8u202 | 140.31 | 95.11 |
| v5 Java 8 | 8u202 | 72.11 | 62.60 |
| v5 mitigation Java 8 | 8u202 | 78.04 | 72.51 |

The adjacent Java 21 v5/mitigation pair was 71.68/61.12 → 65.75/65.06 ms (open/closed). The bundled Java 8
v5/mitigation pair was 72.11/62.60 → 78.04/72.51 ms. Neither comparison establishes an overall improvement.
Only the reduction in redundant font work is demonstrated. The reported Windows cause remains open.
