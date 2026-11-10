# Offline Minecraft 1.8.9 frame benchmark

This is an external test fixture, never part of a release pack. It uses the same remapped fixture with each
rebuilt or shipped client jar. The ASM injector adds one call to `LegacyHooks.runQueued` and the fixture class;
all other client bytecode is preserved. The optional `nodiag` injector argument makes only the watchdog's
`frame()` method a no-op for ablation. Config toggles are `-Dfps189.noctm=true`, `nopersist`, `nocopy`,
`notimestamps`, and `plain` (each with the `fps189.` prefix).

Scene: offline `FPS189Offline`, flat seed 20261108, camera (0.5,100,0.5), yaw/pitch 0, fixed noon and weather,
stone platform, glass wall, ten named armor stands, and 100 chat messages. Wait for teleport chunks before
building. Keep summon commands under the 1.8.9 chat packet's 100-character limit. Warm up 20 seconds, measure
15 seconds with chat open, then wait 5 seconds and measure 15 seconds with chat closed. Report mean, median,
p95 and p99 of complete frame intervals from `System.nanoTime`. Vsync, Dynamic FPS and replay capture are off;
1280x720, render distance 4, GUI scale 2, Fancy/AA on, VBOs off with Polarium. Chat persistence, copying,
timestamps and CTM are enabled when present. Closed-chat samples include vanilla's eventual chat fade.

Pin **all commands** to `taskset -c 8-15,24-31` on ovh1. Use JDK 25 and Gradle `--max-workers=2` for builds.
To prepare the fixture, temporarily copy `Fps189Benchmark.java` into the legacy adapter's main Java package
in a disposable source checkout. Build the 1.8.9 adapter using `bash mod/gradlew --offline --no-daemon
--max-workers=2 -p mod/versions/legacy build -PchatcopyDev=false`. Save the remapped jar as `harness.jar`,
then remove that temporary source. Build each baseline without the fixture and preserve the jars separately.
Verify their entries against the 1.8.9 jar reconstructed from `mod/dist/arctic-client.pack`.

Compile `Instrument.java` with ASM 9.9 on its classpath (already in Gradle's cache). Run:

```
java -cp "$BENCH_CLASSES:$ASM_JAR" Instrument baseline.jar harness.jar instrumented.jar
```

Run `run.py --launch-plan <launcher-dry-run.txt> --jar instrumented.jar --polarium <bundled-polarium.jar>
--output <new-result-directory> --java <jdk21>/bin/java --jfr` under `xvfb-run -s '-screen 0 1280x720x24'`.
For Java 21 add `--vm-arg=--add-opens=java.base/java.lang=ALL-UNNAMED` if required by your loader/runtime.
The dry-run command supplies the game's libraries, natives and assets; all account fields are replaced with
offline values. The fixture never joins a multiplayer server. Each run gets a fresh game directory; failed
scene setup is rejected. Inspect a screenshot before accepting a new fixture revision.

For ablation use one flag at a time with `--vm-arg=-Dfps189.noctm=true`, etc. Revert the complete legacy font
implementation, its mixin and `GlScaleMixin` together when testing the old font: reverting just the GL reads
would discard projected nametag sizing. Alternate baseline and variant runs, repeat pairs, and inspect JFR
`jdk.ExecutionSample`/`jdk.NativeMethodSample` stacks. Frame times from this busy software-rendering host
cannot establish Windows GPU performance, especially for small effects. Never interpret absolute FPS as a
GPU result. Preserve results and profiles outside build directories, and remove only your own build outputs.

To test the font snapshot optimization without mistaking host contention for a speedup, also compile
`Fps189Probe.java` with `--release 8` into the tool classpath. Inject with a final `probe` argument and run
both jars in the same scene. The probe counts/times client-thread methods only, during measured intervals;
worker-thread chunk meshing is outside its timing scope. It also reports client-thread CPU time per frame.
Probe runs carry instrumentation overhead and must not be used as the normal FPS result. Run:

```
python mod/tools/fps189/compare_probe.py before-game.log after-game.log
```

This checks for at least 25% fewer matrix/viewport snapshots at an unchanged glyph workload (within 1%).
The open-chat phase is used because closed chat eventually fades. Each snapshot makes three GL reads;
shadowed strings should share their snapshot, while separate draws still observe fresh GL state. Both
public string drawing and Minecraft's private wrapped/RTL line helper must start a snapshot. In addition
to this work-count test, inspect projected nametags and run the normal frame benchmark and mixin checks.

`extract_pack.py mod/dist/arctic-client.pack <output-directory> [--version 1.8.9]` reconstructs the
embedded release jar(s). Compare archive **entry bytes** with a source rebuild, since ZIP timestamps and
compression need not match. To publish a partial pack update on a fix branch, extract all entries, replace
only the rebuilt target jars, and use the normal `mod/build.py --pack` path. Verify that every other target
still has identical entry bytes and that none contains the external benchmark fixture.
