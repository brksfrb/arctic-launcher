# Experiment: AppCDS archive of the previous launch

Branch `experiment/launch-appcds`. **Off by default**; nothing changes unless
`ARCTIC_EXPERIMENT_APPCDS=1` is set in the launcher's environment.

## What it does

`crates/arctic-core/src/launch/cds.rs`, hooked at the end of `launch::plan`:

- Per game folder, `.arctic-startup/appcds.jsa` plus `appcds.key` (next to the existing
  class-list cache in `startup_cache.rs`).
- The key is a SHA-256 over: game version, loader kind, Java major, the java binary
  (path/size/mtime), the runtime's `release` file, every classpath entry
  (path/size/mtime), every `mods/*.jar`, and the JVM flags that shape loading
  (`-javaagent`, `--add-*`, module path, `-XX:+Use*`, ...).
- Key mismatch or no archive: the old archive is deleted and this start records a new one.
  Key match: the archive is used.
- Java 19+: `-Xshare:auto -XX:+AutoCreateSharedArchive -XX:SharedArchiveFile=...`. The JVM
  dumps at exit and re-validates the file itself.
- Java 13-18 (the launcher's 17): `-XX:ArchiveClassesAtExit=` on the recording start,
  `-XX:SharedArchiveFile=` afterwards.
- Java 8 (1.16.5 and older): no dynamic CDS exists; nothing is added.
- Skipped when the user's JVM args already carry `-Xshare`, `SharedArchiveFile`,
  `ArchiveClassesAtExit`, `AutoCreateSharedArchive` or `DumpLoadedClassList`.

Never fatal: `-Xshare:auto` makes the JVM ignore an archive it rejects, and any problem
computing the key adds no flags at all.

## Why CDS (and not a heap/process snapshot)

CDS is the only documented, cross-platform JVM snapshot. Leyden AOT cache (JDK 24+/25:
`-XX:AOTCache`) would also keep linked/profiled state, but the launcher's runtimes are
Java 8/17/21/25 and the player's Java may be any, so dynamic AppCDS is the common
denominator. CRaC/CRIU snapshots a whole process (GL context, native handles): unusable
for a game client.

## Measured / estimated benefit

Not measured on the real game: I did not launch Minecraft (needs an account, assets and a
game JVM; the harnesses are in the memory notes). A toy JDK 25 program that loads Swing, HTTP
and XML classes showed no gain (~33 ms either way; the JDK's default base archive already
covers those), so that run proves only that the flags work and fall back. The first start
with the archive costs more (dump at exit: ~60 ms on the toy, longer on a full game).

Expected: AppCDS typically saves 10-25% of class-load time for jar classes loaded by the
app loader (vanilla's own classes, libraries such as Netty, Guava, LWJGL). Classes defined
by Fabric's Knot / Forge's modular layers / Mixin-transformed classes are **not** archived
(custom loaders, transformed bytes), which is most of a modded start. The existing
`Preloader` and `MixinCache` cover that part. Realistic gain: a second or less on vanilla
or lightly modded; needs a real A/B (see below) before any decision.

## Limitations

- Forge/NeoForge (JPMS layers, `--module-path`): module classes from custom layers are not
  archived; the flags go in the key, so changing them just rebuilds.
- Agents: `-javaagent` works with dynamic dump, but classes the agent transforms at load
  are excluded. The agent flag is part of the key.
- mtime based fingerprints, not content hashes: hashing hundreds of MB per start would eat
  the gain. A jar replaced with the same size and mtime would not be noticed by the
  launcher; the JVM's own classpath validation (size/mtime too) is the second line.
- Windows/macOS: same flags, but untested here. Archive paths use forward slashes. On
  Windows an antivirus scanning the .jsa may slow the first map. A game killed during the
  exit dump leaves a partial file; the JVM's header validation rejects it and (19+) re-creates it.
- Archive size: tens of MB per game folder.
- `-XX:+UseZGC` etc. are in the key; heap size is not (dynamic archives don't embed heap objects).

## Try it

```
ARCTIC_EXPERIMENT_APPCDS=1 arctic-launcher   # launch twice; second start uses the archive
```
Compare "Game took" / startup timing (see the startup-timing harness) with and without,
several runs each. Delete `.arctic-startup/appcds.*` to reset.

## Tests

`cargo test -p arctic-core cds`: off by default, Java 8 / user CDS skipped, record-then-use,
and a stale archive is deleted for each of: a mod added/changed, a classpath jar changed,
Java `release` changed, java binary changed, game version changed, agent flag changed.
