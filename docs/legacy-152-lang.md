# Crash group 240: legacy language selection

## Cause

`class_244` is StringTranslate (`bp` in Mojang's 1.5.2 client).
`method_633` at line 64 opens `/lang/<selected>.lang`; `method_631`
at line 100 calls it for the selected non-English language. The initial
English load and `/lang/languages.txt` load have already succeeded.
This is not evidence of a missing English resource or a broken Knot jar.

Arctic's shared settings (`shared::sync_options`) and captured defaults
copy `lang` verbatim between game versions. A modern `lang:en_us` therefore
reaches a game that requires `en_US`. Pre-1.6 StringTranslate passes the
missing resource stream to InputStreamReader without a null check.
Languages introduced after the target release cause the same exception.
Existing options and user-disabled sharing can retain the bad selection.
The supplied crash alone does not identify which unavailable language
the affected user's options contained.

The downloaded 1.5.2 jar's SHA-1 is
`465378c9dc2f779ae1d6e8046ebc46fb53a57968`, matching Mojang's metadata.
It contains `lang/en_US.lang`, not `lang/en_us.lang`. Both original and
loader-profile client jars contain the resources. No jar rewriting,
extra classpath resource directory, or assetsDir change is needed.

## Fix

After downloads and asset layout, before constructing the launch plan,
inspect the installed client's actual resource layout. For clients with
`lang/en_US.lang`, resolve the options selection case-insensitively to
an available jar language. Fall back to `en_US` when none matches.
This runs after defaults and sharing, for vanilla and explicit loaders,
and repairs existing options even with sharing disabled. Other options
remain intact. Missing options use the game's own default. Resource-pack
clients (including 1.6.4) are left alone.

No profile crossover was found: saved loader IDs include the game version,
and the merge retains the matching vanilla parent's asset index. A
regression test caches the reported `1.7.3` label alongside 1.5.2, 1.4.7,
and 1.6.4 and verifies each intermediary and asset index independently.
The two install passes in `prepare` also account for multiple classpath
entries being logged during preparation; an earlier log line alone does
not establish that its profile was used for the later launch.

## Validation (ovh1, 2026-11-09)

Builds and games restricted to two CPUs (8 and 9), with nice level 10.
Isolated data: `/home/claude/legacy152-data`; logs and screenshots:
`/home/claude/legacy152-proof`. Games ran sequentially under a private
Xvfb display with software OpenGL and the downloaded Linux jre-legacy
Java 8u202, Legacy Fabric Loader 0.20.1, and no user mods.

| Case | Before, `lang:en_us` | After preparation |
| --- | --- | --- |
| 1.5.2, Knot | Exact reported Reader:78 / InputStreamReader:97 / class_244:64,100 crash | Main menu, `en_US` |
| 1.4.7, Knot | Equivalent class_244:56,91 crash | Main menu, `en_US` |
| 1.6.4, Knot | Starts successfully | Main menu; options unchanged |
| 1.3.2, Knot | Not run | Main menu, `en_US` |
| 1.5.1, Knot | Not run | Main menu, `en_US` |
| 1.5.2, pure vanilla LaunchWrapper | Not run | Main menu, `en_US` |

Arctic Client currently has no target below 1.8.9 (`mod/targets.json`),
so there is no separate 1.5.2 Arctic mod to validate. No versions were
removed. Other pre-1.6 releases use the same resource-layout detection;
the live matrix is representative, not an exhaustive test of every release
or snapshot. Windows 8.1 / Java 8u51 was not available for a native rerun.
Headless audio initialization and the old S3 resource downloader emit
nonfatal errors; the rendered main menus were verified in screenshots.

To repeat: build `arctic-core` examples with `offline-accounts`; use an
isolated `ARCTIC_DATA_DIR` and `loader_smoke fabric:1.5.2:0.20.1` (or the
other versions). Seed the resulting instance's `minecraft/options.txt`
with `lang:en_us`, then run preparation again. Run the printed offline
command from that game directory under Xvfb. For the pure vanilla case,
disable performance, shaders, and Arctic Client in the isolated vanilla
instance, then use `smoke_launch 1.5.2`. Never seed real user settings.

Automated checks: `cargo fmt --check`,
`cargo clippy -p arctic-core --all-targets --features offline-accounts -- -D warnings`,
and `cargo test -p arctic-core --features offline-accounts` (442 passed).
Tests cover language casing, unavailable/empty selections, preservation
of unrelated settings, idempotence, absent options, resource-pack clients,
and isolation of cached legacy profiles.
