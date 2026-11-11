# Command line (`arctic`)

`arctic.exe` is a headless companion to the launcher. It uses the same data folder,
accounts, settings and downloads as the GUI, so you can script installs and launches,
make desktop shortcuts, or run Minecraft on a machine you only reach over SSH.

```powershell
arctic launch                       # latest release as the active account
arctic launch 1.21.4
arctic launch latest --account Alex --memory 6G --wait
arctic launch --instance fabric     # an instance, with its own version and mods
arctic open --launch 1.20.1         # open the launcher window and start 1.20.1
```

Download `arctic-windows-x64.exe` or `arctic-linux-x64` from the
[latest release](https://github.com/brksfrb/arctic-launcher/releases/latest) and put it on
your `PATH` as `arctic`.

## Global options

| Option | Meaning |
|---|---|
| `-p, --profile <NAME>` | Work in this profile instead of the active one (also `ARCTIC_PROFILE`) |
| `--data-dir <DIR>` | Use another data folder (also `ARCTIC_DATA_DIR`) |
| `--json` | Machine-readable output: one JSON object per line on stdout |
| `-v, --verbose` | Print launcher log messages to stderr |

**Exit codes:** `0` success, `1` error, `2` bad usage. `launch --wait` returns the game's own
exit code.

## Commands

### `arctic versions`

```text
arctic versions [--all] [--installed] [-n <LIMIT>]
```

Lists releases, newest first. `--all` includes snapshots and old alpha/beta versions.
`--installed` shows only downloaded versions.

```text
26.3                   2026-09-15  installed  latest
26.2                   2026-06-16
```

JSON: `{"id","type","released","installed","latest"}` per version.

### `arctic install [VERSION]`

Downloads everything a version needs (Java runtime, libraries, assets) without starting it.
It uses the same parallel downloader as the launcher. `VERSION` may be an id, `latest`
(default) or `latest-snapshot`.

### `arctic launch [VERSION] [options]`

Downloads whatever is missing, then starts Minecraft.

| Option | Meaning |
|---|---|
| `-i, --instance <NAME>` | Launch an instance (id or name). Custom instances use their own Minecraft version |
| `-a, --account <NAME>` | Saved account by username, UUID or id (default: the active account) |
| `-m, --memory <SIZE>` | Max heap, e.g. `4G`, `6144M`, `6144` |
| `--width <PX>` / `--height <PX>` | Window size |
| `--fullscreen` | Start fullscreen |
| `--server <ADDRESS>` | Join this server right away (`host` or `host:port`) |
| `--world <FOLDER>` | Open this singleplayer world right away (Minecraft 1.20+) |
| `--java <PATH>` | Use this `java(w).exe` instead of the managed runtime |
| `-w, --wait` | Stay attached: print the game log, exit with the game's code |
| `--dry-run` | Prepare everything and print the command line (token redacted) |

Without `--wait`, the game is started **detached**: it keeps running after `arctic` exits,
and its output goes to `profiles\<profile>\logs\game-vanilla.log`. Microsoft sessions are refreshed
automatically when needed.

### `arctic instances`

```text
arctic instances list
arctic instances create <NAME> [--version <VERSION>] [--loader <LOADER>] [--loader-version <V>]
arctic instances show <INSTANCE>
arctic instances edit <INSTANCE> [--name N] [--version V] [--loader L] [--loader-version V]
    [--memory 4G|default] [--icon classic|stellar|dendrite|plate|star|crystal] [--color #rrggbb|default]
    [--arctic-client on|off] [--java PATH|default] [--jvm-args "FLAGS"] [--performance on|off] [--shaders on|off]
    [--game-dir FOLDER|default [--keep-files]]
arctic instances remove <INSTANCE>
```

`edit` changes only what's given. Changing the version or loader picks the newest stable
loader build unless `--loader-version` says otherwise. The Vanilla instance follows the
version picked to play, so its version and loader can't be pinned (make another instance).

`--loader` is `vanilla` (default), `fabric`, `quilt`, `neoforge` or `forge`. Without
`--loader-version`, the newest stable loader build for that Minecraft version is used.
Removing an instance moves its folder (worlds included) to `instances/.trash`.

`--game-dir` puts the instance's game (worlds, mods, packs, options) in a folder on any drive.
Its files move there (the folder must be empty) unless `--keep-files`, which uses the folder as
it is, for one that already holds a game. `default` brings it back inside the instance's folder.
A game folder elsewhere is left alone when the instance is removed.

```text
arctic instances create "Fabric" --version 26.3 --loader fabric
Created Fabric (Fabric 0.19.5, Minecraft 26.3). Start it with: arctic launch --instance fabric
```

JSON (`list`): `{"id","name","version","loader","loader_version"}` per instance.

### `arctic mods`

Mods come from [Modrinth](https://modrinth.com) and are matched to the instance's
Minecraft version and loader. Required dependencies are installed too.

```text
arctic mods search <QUERY> --instance <INSTANCE> [-n <LIMIT>]
arctic mods install <SLUG_OR_ID> --instance <INSTANCE>
arctic mods list --instance <INSTANCE>
arctic mods toggle <FILE> on|off --instance <INSTANCE>
arctic mods remove <FILE> --instance <INSTANCE>
arctic mods check --instance <INSTANCE>
```

```text
arctic mods install sodium-extra --instance fabric
Installed Sodium Extra 0.6.1
Installed Sodium 0.6.13 (dependency)
```

JSON: `search` prints `{"id","slug","title","author","downloads","description"}`,
`install` prints one `{"event":"installed",...}` per file, `list` prints
`{"file","enabled","title","size"}`.

`check` lists what would stop the game while loading (the same mod twice, a mod that
breaks another, a missing or too old requirement, a mod for another Minecraft version)
with the fixes the launcher offers, and exits with 1 when it finds any. The launcher runs
the same check when you press Play. JSON: one `{"event":"clash","problem","fixes"}` each,
or `{"event":"clean"}`.

### `arctic modpacks`

```text
arctic modpacks search [QUERY] [-n <LIMIT>]
arctic modpacks install <SLUG_OR_ID | FILE.mrpack>
```

Each modpack becomes a new instance with its loader, mods and config.

```text
arctic modpacks install fabulously-optimized
Installed Fabulously Optimized (Fabric 26.3). Start it with: arctic launch --instance fabulously-optimized
```

### `arctic skins`

Your skin library (the skins in the launcher's Cosmetics tab) and the community gallery.

```text
arctic skins list
arctic skins add <PNG> [--name N] [--model classic|slim]
arctic skins rename <SKIN> <NAME> | model <SKIN> classic|slim | remove <SKIN> | export <SKIN> <PNG>
arctic skins wear <SKIN|none> [--account A]      (your Arctic look; `none` = your Minecraft skin)
arctic skins gallery [QUERY] [--new] [--page N]
arctic skins take <GALLERY_ID> [--wear] | share <SKIN> [--name N] | report <GALLERY_ID>
```

`SKIN` is a library skin's id (or its first characters) or name.

### `arctic screenshots`, `replays`, `logs`

```text
arctic screenshots list [--instance I] [-n N] | remove <FILE>
arctic replays list [--instance I] | remove <NAME> [--instance I]
arctic logs [--instance I | --launcher] [-n N] [--level warn|error] [--follow] [--path]
```

Removed screenshots and replays go to a `.trash` folder next to them. Watch a replay with
`arctic launch --instance I --replay FILE`. `logs` shows an instance's last game log
(`--follow` keeps printing new lines while the game runs).

### `arctic accounts`

```text
arctic accounts list
arctic accounts login [--browser]     # device code by default (works over SSH)
arctic accounts use <ACCOUNT>
arctic accounts remove <ACCOUNT>
```

`<ACCOUNT>` is a username (case-insensitive), a Minecraft UUID (with or without dashes),
or the internal id.

### `arctic profiles`

```text
arctic profiles list
arctic profiles create <NAME> [--switch]
arctic profiles use <PROFILE>
arctic profiles rename <PROFILE> <NEW_NAME>
arctic profiles remove <PROFILE>        # folder moves to profiles/.trash
```

Every command works on the active profile unless you pass `-p, --profile <NAME>` (or set
`ARCTIC_PROFILE`), e.g. `arctic -p Speedruns launch 1.16.1`.

### `arctic migrate`

Brings instances over from other launchers and clients: version, loader, mods, configs,
worlds, resource and shader packs, options and servers. Originals are never changed and
accounts never come along. Supported: Minecraft Launcher, TLauncher, Prism, PolyMC,
MultiMC (with `--folder`), Modrinth App, CurseForge, ATLauncher, GDLauncher (new and
old), and the own mods of LabyMod, Lunar and Feather. LabyMod and Lunar HUD layouts and
keys can be applied too.

```text
arctic migrate list [--folder <DIR>]
arctic migrate instance <KEY_OR_NAME> [--skip mods,worlds,resourcepacks,shaderpacks,settings,screenshots] [--name <NAME>]
arctic migrate all [--skip ...]
arctic migrate hud <KEY> [--instance <INSTANCE>] [--on fps,coords,...]
```

While importing, mods are checked the way the loaders check them: mods their own files
say won't run on that Minecraft version come switched off, a required mod the pack is
missing is fetched from Modrinth, and mods are matched to Modrinth by hash so they can be
updated and shared. Lunar saves only settings that differ from its defaults, so widgets
without a saved on/off state are listed; `--on` picks them (default: the ones that were
moved somewhere).

### `arctic share` and `arctic import`

Share an instance (version, loader and the exact Modrinth versions of its mods, no jar
files), a HUD layout, a crosshair, all Arctic Client settings, or a whole profile.

```text
arctic share instance|hud|crosshair|client|profile [--instance <INSTANCE>] [--as code|text|file] [--out <FILE>]
arctic import <CODE | TEXT | FILE> [--instance <INSTANCE>]
```

A code (`abcd-efgh`) is stored on the Arctic server and needs a signed-in account; text
(`arctic1.…`) and `.json` files work without it. Accounts, the proxy, Java paths and JVM
arguments are never included, and everything imported is checked value by value first.

### `arctic bedrock`

Minecraft: Bedrock Edition (Minecraft for Windows). `--preview` works on Minecraft Preview
instead, and `--account <ID>` picks an Xbox account's folder for worlds and servers (the ids
are in `status`; the default is the one played last).

```text
arctic bedrock status | play
arctic bedrock play --native-client <BUNDLE_DIRECTORY>
arctic bedrock offline <ADDRESS> --name <NAME> [--skin PNG] [--cape PNG] [--slim] [--listen-port PORT] [--version VERSION] [--play]
arctic bedrock versions [--all] | install <VERSION|latest>
arctic bedrock worlds list | import <MCWORLD_OR_FOLDER> | export <WORLD> [--to FILE] | backup <WORLD> | remove <WORLD>
arctic bedrock packs list | import <MCPACK_OR_MCADDON> | remove <NAME|UUID>
arctic bedrock servers list [--no-ping] | ping <ADDRESS> | add <ADDRESS> [--name N] | remove <ADDRESS>
```

`install` switches the installed game to any version since 1.21.120, older or newer: Windows
downloads the package from Microsoft and installs it over the current one, keeping worlds,
packs and settings. The game must be owned on the Microsoft account signed in to the
Microsoft Store, and the Store may update it again later. Removed worlds and packs go to
`com.mojang\arctic-trash`; backups go to `bedrock\backups` in Arctic's folder.

`offline` runs a local protocol connector for servers that explicitly accept unsigned
Bedrock players (for example, a PocketMine/Nukkit server configured without Xbox auth).
It connects upstream with the chosen name, an offline identity and no Xbox credentials.
`--skin` supplies a 64×64 or legacy 64×32 PNG, `--slim` selects slim arms, and `--cape`
supplies a static 2:1 PNG (power-of-two width 64–1024). HD capes are reduced to 64×32.
The server receives the chosen appearance; server policy controls whether it is shown.
Reconnect to change it. Animated capes, 3D cosmetics and local player rendering still
need native game hooks. Offline UUIDs follow Java's `OfflinePlayer:<name>` convention.
Join the printed `127.0.0.1:19142` address from Bedrock; keep the command open while playing.
The target and the game must speak the same protocol. `--version` selects an explicitly
supported protocol data version when discovery is unavailable. Node.js 24+ with npm is
required; the first run downloads a pinned protocol helper into Arctic's data folder.
The game must allow joining a local server: this connector does not modify the client's
sign-in screens or Windows license checks, and it cannot join servers that require Xbox auth.

`--native-client` attaches the experimental native adapter built with
`python mod/versions/bedrock/build.py --java-home <JDK> --game-version <VERSION>`.
It reuses the shared Java client menus and HUD drawing. In-world skins, capes, cosmetics,
emotes, game state, replay, and Xbox identity bindings are still pending; this is not a
feature-complete Bedrock port. `--smoke` on the build script tests native injection and
rendering in a disposable hidden DirectX 11 host.

The native client's source adds **Mods → Bedrock Cosmetics**, an experimental
opt-in for the body/cape/wing/emote rendering path. It defaults off through
`bedrockNativeCosmetics` in the shared client config. Cape animation and emote
wheel key/hold settings are available on that card. The paired Java/native
render-thread bridge applies changes and clears new readiness when disabled;
actual model/pose/texture/draw requirements still apply. This does not establish
full feature parity, enable probe camera/clock controls or certify deterministic
video. No builds, tests or Bedrock launch were performed for this source change.

Recorded appearance histories now include a bounded six-part native body-pose
track in addition to cape ticks and wing samples. Replay rendering and immutable
native emote programs use the recorded body reference at the requested time.
Absent samples, actor gaps and missed ticks block pose-dependent indexed capture
readiness. Older archives remain usable for preview, with that readiness check
unsatisfied. This source port does not verify native animation interpolation,
equipment fidelity or repeatable MP4 output; it was not built or tested.

Indexed frame markers now retain `timeRemainder` and `timeDenominator` beside
the whole `timeMs`. Recorded body/wing interpolation, cape physics and emote
phase use that fraction, while recorded packet selection keeps the whole-ms
floor. The capture receipt must match the fraction actually used by the Java
world layer, and the export range is checked against the appearance history
without flooring the final frame. These paired Java/native source changes have
not been built, installed or tested.

New appearance snapshots/histories retain loaded native skin PNGs and the
observed arm model as `bedrockSkin` actor data. Recorded Arctic skins take
priority; native fallback skins are loaded from the archive for bodies and
emotes. An explicit unknown skin in a new take cannot use the viewer's current
skin or pass indexed capture readiness. This does not establish persona or
animated native-skin support, attachment replay or complete rendering parity.
No builds, tests or Bedrock launch ran for this source change.

Appearance histories now include a bounded attachment-palette track when native
observation is active. Its loader preserves gaps and samples matching recorded
layouts at fractional frame times. A leased source consumer maps matching
archived palettes into owned native copy outputs, checking the replay marker,
native tick/partial and indexed layout. Those CPU checks are not a GPU/coverage
receipt, and the equipment/emote export restriction remains in place. This source
change was not built or tested.

Body history version 2 also preserves the model basis and world origin from the
observed native draw. Replay bodies/capes/cosmetics and matching native equipment
use the recorded root rebased to the current camera. Version 1 tracks retain
preview support, with the new root-dependent capture readiness unsatisfied.
Reflected, sheared or degenerate root interpolation remains a gap; this source
port does not verify complete native simulation, culling or rendered video.
No builds, tests or Bedrock launch ran.

#### Bedrock packet recordings

`arctic bedrock offline HOST --name NAME --record-to FILE.abr` records the unsigned
relay session. `arctic bedrock replay FILE.abr --speed 1 --start-ms 0` serves it on
loopback port 19143 (`--listen-port` changes it). Type `stop` and Enter to finish
the session cleanly. The recording is created exclusively; existing files are
refused. Playback restores world packets and the recording player's movement.
It requires matching Bedrock protocol data and uses no Xbox authentication.

`arctic bedrock replay FILE.abr --nethernet` selects the local LAN-world transport
on `127.0.0.2:7551`; `--listen-port` can select another free local port. It advertises
the recording as **Offline playback**, verifies self-signed P-384 identities and
the session nonce locally, and uses loopback WebRTC without STUN/TURN services.
This preview path was ported through static edits and has not been run or built.
It does not establish installed-client playback or deterministic video parity.
The launcher has a camera keyframe editor for protocol 2193 (data version 1.26.51),
including JSON path save/open and export to a new `.abr` with a baked camera track.
Real-time MP4 recording is available through the loaded native bundle. The indexed
MP4 source path and embedded resource-pack transfer have also been ported. Complete
packet/terrain barriers, direct singleplayer capture and deterministic rendered
video parity remain unfinished. These newer ports have not been built or run.

While the CLI replay is running, stdin accepts `pause`, `resume`, `state`,
`camera clear`, `stop`, and JSON controls (one line per command):

```json
{"op":"speed","speed":0.5}
{"op":"seek","timeMs":30000}
{"op":"reset","timeMs":1000}
{"op":"step","generation":2,"sequence":1,"timeMs":1500}
{"op":"camera","pose":{"x":12,"y":80,"z":15,"yaw":45,"pitch":20,"fov":90}}
{"op":"camera_path","path":{"format":1,"keyframes":[{"timeMs":0,"pose":{"x":0,"y":80,"z":0,"yaw":0,"pitch":0,"fov":70},"ease":"smooth"},{"timeMs":1000,"pose":{"x":10,"y":85,"z":10,"yaw":90,"pitch":10,"fov":90}}]}}
{"op":"export","file":"D:\\replays\\edited.abr","fps":60}
```

For protocol 1.26.51 on the local RakNet listener, `reset` also accepts
`"reconnect":true`. It requests a transfer to the same owned loopback endpoint,
then waits in reported `worldReset` stages for a new viewer connection with the
same offline profile, its initialization acknowledgement and packet catch-up.
The reset command's immediate reply only acknowledges the request; inspect
`worldReset.stage == "ready"`, the new generation/viewer ID and packet readiness
before feeding the new world. A 60-second deadline reports `replay_reset_failed`.
No automatic helper/game restart is attempted. Unsupported transports reject
the automatic request before resetting; omitted/false `reconnect` keeps manual
rejoin. Installed-client behavior and native terrain/presentation readiness are
unverified. This source change was not built or tested and did not open Bedrock.

Keyframe times must increase and fit the replay. Interpolation is `catmull_rom`
(Java's uniform camera curve), `linear`, `smooth` or `hold` from each keyframe to
the next. New launcher keyframes use the Java curve; existing paths without an
`ease` field retain linear interpolation. The launcher can also import the camera
keyframes from an extracted Java replay `arctic.json` (extras version 1). It sorts
the keys and keeps the last edit at each timestamp, as the Java editor does.
This transfers the camera edit, not the Java world's replay packets.
`export` uses the applied camera path, or
an optional `path` object in that command. The output must be a new absolute path.
Optional `fps` selects 1–120 camera samples/s (default 25). The same recording,
path and rate produce byte-identical `.abr` output, with source metadata preserved
and integer-indexed sample times. This does not make MP4 capture deterministic.

The static native-camera port also exposes a scriptable consumer API:

```text
arctic bedrock native-camera --bundle D:\arctic\dist --session native_PID_EPOCH --sequence 1 --target D:\replays\native-camera.json
arctic bedrock native-camera --bundle D:\arctic\dist --session native_PID_EPOCH --sequence 2 --release
```

The target JSON has `position` (three world coordinates), `orientation` (four
native XYZW quaternion values) and `fieldOfView` (5–170 degrees). Use the actual
camera basis reported by the adapter, not a guessed Java-to-native rotation.
The live session is in `arctic-bedrock-PID.camera.json` in the loaded bundle.
Commands never launch the game or switch accounts. They require its existing
exact-image camera binding. A consumer acknowledgement confirms scoped component
restoration and, for a set, unchanged player pose; it does not confirm terrain,
GPU presentation or deterministic video. These camera-port additions have only
been reviewed statically, without builds, tests or a Bedrock launch.

Portable appearance snapshots have a separate source control channel:

```text
arctic bedrock native-appearance --bundle D:\arctic\dist --session native_PID_EPOCH --sequence 1 D:\replays\appearance-operation.json
```

The operation file contains one of these JSON objects:

```json
{"op":"save","path":"D:\\replays\\start.arctic-looks.zip"}
{"op":"load","path":"D:\\replays\\start.arctic-looks.zip","owner":"editor"}
{"op":"time","timeMs":30000,"owner":"editor"}
{"op":"state"}
{"op":"clear","owner":"editor"}
```

Each line above is a separate example file, with a strictly increasing command
sequence. The session and latest sequence are published in
`arctic-bedrock-PID.appearance.json`. Save requires a new file. Load checks the
complete archive before mounting it; a failed preparation leaves the previous
mount intact. An optional owner is a 1–64 character alphanumeric/underscore/hyphen
token; the same token is required to replace, retime or clear an owned mount.
Omit it for an unowned mount. Time cannot precede the snapshot or exceed 24 hours,
and must match an active export dispatch marker. Without a marker, time stays
held until explicitly changed. State returns the manifest identity and owner.

These commands require the existing loaded native client/world and never launch
Minecraft or sign in. Queue admission expires after two seconds; once executing,
a host timeout does not cancel file I/O. Load has a 20-second pre-commit preparation
deadline. Completion acknowledges Java appearance state; it does not establish
native suppression or presented pixels. No commands were run for this static port.

Appearance history uses the same command with these operation-file examples:

```json
{"op":"history_begin","path":"D:\\replays\\looks.arctic-looks.zip","timeMs":0,"owner":"take"}
{"op":"history_finish","owner":"take"}
{"op":"history_cancel","owner":"take"}
```

Begin samples the current loaded looks and arms observation of later changes on
client frames. `timeMs` assigns the first sample's timeline origin; later times
come from elapsed native world clock. A separate cosmetic clock preserves cape
and idle animation phase. Repeating an emote start, stopping it, changing its held
phase, changing a look/fit or adding/removing an actor creates an event; advancing
animation time alone does not. Assets are copied on one bounded writer, with a
32-job / 8 MiB pending-state queue, 4,096 observed-change limit, bounded 1 MiB index
and 256 MiB raw budget. State JSON is stored as checked asset entries referenced
by timestamp/hash; it shares the 4,096 raw-file budget with images and model data.
Queue overflow, missing assets, backward clocks and world/local identity changes
fail the take. Playback mounts and history recording cannot own the client together.

Finish acknowledges queued finalization. Poll `state` with increasing sequences
until `history.stage` is `complete` or `failed`; only `complete` reports a finished
archive. Cancel can leave an output if finalization was already writing it; inspect
the reported state. The staging cache contains only raw appearance assets. No auth
envelopes are recorded. The selected appearance file may be a format-1 snapshot
or format-2 history; a history must cover the entire MP4 frame range.

New histories also carry completed-tick endpoints of Arctic's cape filter,
including the first-person player's state. Separate compressed blocks hold at
most 64 ticks / 128 actors and inflate to at most 2 MiB. Their data shares the
raw-file, byte, queue and manifest budgets. Missing tick/actor samples remain gaps;
the recorder fails on inconsistent same-tick data. Playback of a physics-bearing
history uses those endpoints and rejects cape export frames without a recorded
sample, rather than using the current viewer's filter. All blocks are checked
before mount, with one decoded-block cache during playback.

The live `offline ... --record-to take.abr` CLI also accepts paired recording
controls after the existing client joins and initializes its world:

```json
{"op":"native_record_begin","bundle":"D:\\arctic\\dist"}
{"op":"native_record_finish"}
```

`native_record_cancel` (with no additional fields) releases this CLI's owned
appearance take while packet recording continues. A begin timeout retains its
recovery lease if cleanup was not acknowledged; the failure event includes
`retainedLease`. Retry finish or explicitly cancel that owner before beginning
another take. Cancellation checks fresh Java ownership and terminal history
state, and never cancels another recording's owner.

`state` reports the relay take's ID and initialization status. Begin queues a new
local marker, records its actual packet-queue timestamp and waits for the exact
native dispatcher/post-tick acknowledgement. The native consumer checks the live
receipt and derives the first appearance time from that tick clock; it refuses
stale anchors. No connection, game launch or sign-in is performed by these controls.
Finish stops both capture streams and waits for both files to finalize, then creates
`take.abr.arctic-looks.zip` and `take.abr.appearance.json` beside the packet recording.
The descriptor binds their recording ID, file hashes and appearance manifest.
Existing unrelated outputs are refused. A successful finish may be retried with
the same lease. `stop` and stdin EOF finalize an owned pair before stopping its
helper; failed paired finalization reports an error/nonzero exit and preserves files.

MP4 export with no explicit appearance file discovers a valid adjacent companion.
It hashes the replay's actual opened descriptor, checks the adjacent source/archive
bytes and rechecks the selected manifest/ID after native mount. Replacement files,
foreign IDs, corrupt links, escaping paths and insufficient appearance time ranges
fail export. Explicit appearance selection keeps its existing behavior. Move all
three files together to preserve the adjacent pairing.

This source port associates an initial queue marker with a completed native tick.
It does not prove continuous alignment under lag/pause/rate changes, coverage of
the earlier packet prefix, packet application or terrain readiness. Native wings,
armor/held-item posing and full draw fidelity remain unfinished; alignment/native/
video verification flags stay false. None of these operations has been built,
run or tested.

CLI replay also accepts an indexed export request on stdin:

```json
{"op":"native_export","options":{"bundle":"D:\\arctic\\dist","ffmpeg":"D:\\tools\\ffmpeg.exe","output":"D:\\replays\\clip.mp4","fps":30,"frames":300,"settleFrames":3,"appearanceSnapshot":"D:\\replays\\start.arctic-looks.zip","cameraPath":{"format":1,"keyframes":[{"timeMs":0,"pose":{"x":0,"y":80,"z":0,"yaw":0,"pitch":0,"fov":70},"ease":"catmull_rom"}]}}}
```

Omit `appearanceSnapshot` to use a valid adjacent recorded-appearance companion
when present, otherwise current appearance. A selected snapshot must be
at or before the export origin, and cannot replace an existing mount.
Export owns its temporary mount and clears it during cleanup. Each selected-snapshot
frame requires matching manifest/time metadata and visible-actor body/cape draw
readiness alongside the clock, packet marker and camera requirements. Older draw
plans, unsupported attachment poses and custom capes during elytra flight cannot
satisfy that requirement. Use `cancel export` for cooperative cancellation or `stop`
to cancel and stop playback after cleanup. The output and its `.frames.jsonl`
manifest must be new files. This source path has not been built or exercised;
native-world and deterministic-video verification flags remain false.

An optional `startMs` in the export options requests a fresh-world export from
that timestamp (clamped to the recorded initialization checkpoint). The launcher
exposes it as **Reload world before export** plus **Start at**. Omit it to keep
the current-world/playhead workflow. Rebuild export currently requires an existing
initialized RakNet protocol 1.26.51 viewer, a matching current native local UUID,
idle clock/packet/appearance bindings and no other appearance owner.

Export first reserves the native capture slot without opening FFmpeg. It then
requests the local transfer, checks the replacement viewer/initialization and
packet catch-up, and requires a different native world epoch in the same process.
Only the matching unexpired reservation can activate indexed capture.
Indexed exports also require matching observations of an empty
subchunk insertion queue, zero in-flight tasks and an unchanged observed task
activity revision across the captured native world pass. Missing or busy observations time out; this condition does
not prove complete terrain, lighting, mesh or packet application readiness.
The capture-control fields `requireSubchunkIdle`, `requireNetworkIdle`,
`requireRendererIdle` and `requireFrameInput` are optional for manual capture.
The indexed export job sets all four automatically.
The network condition requires the active session's pending chunk and paused
callback maps, plus its I/O task group, to be observed idle with unchanged task
activity during the same captured world pass. It also requires the owned legacy
insertion queue to have equal issued/consumed sequence counts and an empty ready
vector, unchanged across that pass. An empty vector alone does not admit issued
decode work that has not completed. Consumed includes native discards and does
not prove successful application. Missing network ownership, older receipts or
busy work time out; complete terrain readiness remains unverified.
The renderer condition requires the actual primary renderer's background and
main-thread task groups to have zero in-flight tasks, running group state and
unchanged observed task activity across the same world pass. Renderer/client/
level ownership must agree. Missing observations reject the frame; idle groups
do not prove complete visibility, geometry, lighting or terrain GPU uploads.
The same condition also requires empty pending visibility, actor-notification
and block-entity-notification maps in the camera dimension's owned render-chunk
coordinator. It matches the actual camera dimension, validates the coordinator
owner and list links, and rejects unsupported or missing observations. This
reader supports at most eight owned dimension coordinators; it does not invoke
engine getters, drain maps or advance extra ticks to make a frame pass.
The frame metadata names these map-key counts `visibilityDirtyChunks`,
`actorNotificationChunks` and `blockEntityNotificationChunks`. A chunk key may
hold multiple messages; these counts do not report individual notification
messages or the total number of visible chunks.
`sharedChunkWork` also counts live, expired and empty weak chunk entries and
observed geometry/sort and visibility-build states. `requireRendererIdle`
requires zero active build/sort and visibility-build counts, with matching native
chunk identities and state metadata across the captured pass. Merely dirty
cached chunks are reported separately; their membership in the visible frame
has not been established. The reader accepts at most 32,768 chunk keys and
rejects unsupported, unreadable or changing snapshots. These conditions still
do not certify complete visible geometry, lighting or terrain GPU completion.
`requireFrameInput` also requires the actual world callback's frame argument and
one primary, non-shadow view for client 0. Its camera position/target must match
the current primary renderer. The same before/after snapshot carries bounded
view, chunk-queue and 51 terrain-layer queue counts, plus their bounded native
entries. It requires matching count partitions, valid chunk indices, finite native
ready-time values, supported mesh/index selectors, live shared controls, valid
index-range arithmetic and High/Low LOD values. Potentially drawable entries must
have mesh and material pointers. Direct resources must also satisfy the native
queue producer's availability predicate: vertex data, attributes for split
streams, and direct index data when that variant is selected. Availability is
reported as empty/unavailable/available partitions for both direct resource
types; these describe native optional fields, not GPU completion. Ordinary meshes
use their native backend-handle or CPU-position/index fallback conditions,
including the native attribute-buffer condition. A separate ordinary index
override must have its handle tracking flag set. Capture also validates the
ordinary availability partitions and embedded shared-control counts. The limits
are 32,768 chunks and 262,144 total layer entries; larger, missing, unsupported
or changing snapshots reject capture.
The same snapshot follows up to 16,384 distinct layer `MaterialPtr` addresses,
copies their shared pairs and rechecks positive controls. Null material pointers,
empty pairs and owned-null pairs are counted separately. Potentially drawable
entries must have a nonempty material-info reference, and both capture matchers
check the material count partitions. These references do not prove a resolved
shader or completed native material/resource binding.
This observes metadata and references. Buffer contents/bounds, material ownership,
projection, visible-set completeness and GPU execution remain unverified. Input,
resource and allocator addresses are not archived. CPU fallback availability
does not establish successful upload or buffer contents. Material-resource
ownership remains unverified; the material-info object is not dereferenced.
The reservation renews while waiting and releases
on error/cancellation; other capture owners cannot
consume the reserved slot. The outcome includes a `worldRebuild` identity receipt.
Cancellation addresses only that reset's generation and original viewer ID; an
already-sent transfer cannot be undone. No helper/client launch or authentication
is performed. Rebuilding does not prove terrain completeness, deterministic
simulation, decoded-video repeatability or complete Java parity. This source
workflow has not been built, run or tested.

The static native clock port exposes a separate script command:

```text
arctic bedrock native-clock --bundle D:\arctic\dist --session native_PID_EPOCH --sequence 1 D:\replays\clock-operation.json
```

The operation JSON is, for example:

```json
{"op":"freeze","partial":0.0,"realTimer":true,"leaseMs":5000}
```

Operations are `freeze`, `step`, `sample`, `renew` and `resume`. Use an increasing
sequence for each command. `step` advances one primary-client world tick;
`sample` changes its fractional tick without advancing it. `renew` retains the
same fraction and timer mode while extending the lease. Fractions must be in
`[0,1)` and leases 500–30000 ms (default 5000). `resume` releases both clocks.
The selected client/real timer rates must match when controlling both.

Commands use the already-loaded adapter and never launch the game or sign in.
The adapter publishes `arctic-bedrock-PID.clock.json`, with a live-binding flag,
timer output, held tick and a request-correlated completion receipt. A successful
`step` must observe the actual primary level tick and requested real-timer step,
rather than just calculating a timer result. Lease expiry, lost ownership,
unexpected world ticks and a two-second completion timeout release control.
These source changes have not been built or tested. This does not freeze an
integrated server, acknowledge queued replay packets, prove terrain readiness,
or establish deterministic rendered MP4 export.
Seek advances an existing connection. `reset` pauses playback, retires its viewer,
increments the generation and waits for a fresh game connection, reconstructing
from the recording through the requested timestamp. Read `generation` from the
`replay_reset` or `replay_state` event instead of assuming the example's value.
Automatic installed-game reconnect and native terrain readiness remain unfinished.

`step` requires paused playback, a connected viewer, the current generation and a
strictly increasing sequence. It queues eligible recorded world updates through exactly `timeMs`,
waiting for all batches at that timestamp, then emits `replay_step` with matching
generation/sequence and `packets_queued: true`. This acknowledges packet queuing;
`native_world_verified` remains false. `packet_ready` in state events describes
packet catch-up, not rendered terrain. A held camera is sent when its pose/time
changes, without periodic messages while paused. Reset, disconnect, stop and
timeout cancel pending steps. Step errors preserve generation/sequence correlations.
Pause suspends packet delivery and path time, while client-side animation can continue.
Camera paths see only recorded chunks; `.abr` export is not MP4/video capture.

#### Comparing finished indexed exports

```text
arctic bedrock compare-exports first.mp4 second.mp4
```

This reads two finished indexed MP4 exports and each file's adjacent
`.frames.jsonl` and `.decoded.jsonl` audits. It starts no game, account session,
encoder or decoder. The saved decoded rows are revalidated; they are not decoded
again. The MP4 hash must still match the completed audit, and every captured
RGBA row and encoder timing/color request is checked again. Inputs are opened
read-only, with writer/deletion sharing denied on Windows, and checked for changes.

The captures must have different IDs and matching frame counts, dimensions and
FPS. The report compares source RGBA and decoded NV12 hashes frame by frame,
rational presentation timing, observed camera poses, relative native tick/partial
time, indexed replay time, cosmetic/appearance intent and encoding requests. It
reports the first differing zero-based frame and fields, retaining both audited
MP4/input/decoded sequence hashes in the structured result. Exit status is 0 when
those observations match, 1 when they differ; malformed, changed or incomplete
artifacts produce an error. MP4 container-byte equality is reported separately.

New indexed exports also save `.export.json`. Before activating the encoder or
freezing the native clock, the job binds the helper's open replay descriptor to
a locally hashed, read-locked `.abr` file. It checks that source again after the
last captured frame. The helper acknowledges a binary fingerprint of the accepted
camera path and an increasing revision; the feeder requires both to stay unchanged.
The receipt retains the recording digest/size/protocol/optional recording ID,
camera path, frame range, settling count, encoder choice and appearance manifest.
It also binds the finished MP4, captured-frame manifest and decoded-audit hashes.
Existing receipt paths are refused, and failed jobs retain partial artifacts.

When a receipt is present, comparison validates its intent digest and every
artifact binding. Bound frame time, cosmetic/appearance intent and settling
requirements must agree with the capture rows. `recordingSourceIdentical` and
`inputIntentIdentical` compare the two saved bindings; they are null when either
export predates the receipt format. `intentDifferences` identifies source, camera,
range, appearance, encoder or settling differences. Differing known intents also
return exit status 1 even when their captured pixels happen to match.

Use `--json` to retain the structured `exports_compared` event. Agreement covers
these saved artifacts and their bound/observed inputs. It does not attest that
all recorded packets, game resources or shader state were applied. Broad
`sameRecordingVerified`, `nativeWorldVerified` and `deterministicVideoVerified`
flags remain false; matching receipts alone cannot establish Java parity.
This source implementation has not been built or run.

#### Native MP4 recording

```text
arctic bedrock capture --bundle BUNDLE_DIRECTORY --ffmpeg FFMPEG_EXECUTABLE clip.mp4 --seconds 10 --fps 30
```

The selected bundle must already be loaded into the running game. This command
does not launch Minecraft or select an account. It records presented DX11/DX12
pixels and the Arctic overlay to a new MP4/H.264 file, without audio. Frame rates
are 1–60, duration 1–7200 seconds, maximum resolution 3840×2160 with SDR RGBA/BGRA.
Keep the game's resolution fixed during the clip. Existing outputs are refused;
encoding or rendering failures are reported and partial files are preserved.
This uses wall-clock playback; it is not the shared Java replay exporter's fixed
simulation/frame stepping. The GUI exposes the same feature under **Arctic Client**
and the replay camera editor.

### `arctic worlds`, `waypoints`, `servers`, `friends`, `defaults`, `skin`, `look`, `proxy`, `settings`, `crash`, `together`

```text
arctic worlds list|sources [--instance <I>]
arctic servers list [--instance <I>] [--no-ping] | ping <ADDRESS> | add <ADDRESS> [--name N] [--instance <I>]
arctic servers move <ADDRESS> [--before <ADDRESS>] [--instance <I>]
arctic servers browse [--premium|--cracked] [--random]
arctic servers submit <ADDRESS> --name N [--description D] [--tag T]... | verify <ID>
arctic servers review [--state pending] [--approve|--reject|--remove <ID>]   (needs ARCTIC_ADMIN_KEY)
arctic friends list | add <CODE|NAME> | accept <NAME> | remove <NAME> [--account A]
arctic friends recovery | restore <RECOVERY_CODE> [--account A]
arctic friends chat <NAME> [--send TEXT]
arctic defaults show | set <KEY> <VALUE> | unset <KEY> | capture [--instance I] | apply [--instance I] | clear
arctic share options   (default game settings as a code)
arctic friends invite <NAME> --server <ADDRESS> | --together <CODE>
arctic friends profile [--name N] [--share-online B] [--share-server B] [--invites B] [--show-accounts B] [--link ACCOUNT] [--unlink ACCOUNT]
arctic worlds import <FOLDER_OR_ZIP> | backup <WORLD> | remove <WORLD> [--instance <I>]
arctic waypoints list [--world W] | add <NAME> <X> <Y> <Z> --world W [--dim D] | remove <NAME> --world W [--instance <I>]
    (W is a server address, or sp:<world name>; the game's Compass and Minimap show them)
arctic skin show | set <PNG> [--model classic|slim] | reset | cape <NAME|none>
arctic look show | skin <PNG|none> | cape <PRESET|PNG|none> | wear [IDS...] | fit <ID> [--up N] [--forward N] [--mirror]
arctic proxy show | set <HOST> [PORT] [--username U] [--password P] | off | test
arctic settings show | get <KEY> | set <KEY> <VALUE>
arctic crash [FILE] [--instance <I>]
arctic together host [--port N] | join <CODE>
```

### `arctic java <VERSION>`

Prints the Java executable that version uses, installing the runtime if needed. This is
handy for running tools against the same JVM.

### `arctic paths`

Shows the data folder layout: settings, accounts, versions, libraries, assets, runtimes,
instances and logs.

### `arctic storage`

```text
arctic storage show
arctic storage move <FOLDER|default>
arctic storage use <FOLDER>
```

Where Arctic keeps everything (Settings → Storage in the launcher). `move` copies it all to an
empty folder (another drive, say) and uses it from then on; the old copy is removed the next
time the launcher starts. Close the launcher first. `default` moves back to the usual place.
`use` switches to a folder that already holds Arctic's files, copying nothing. Not available
when `ARCTIC_DATA_DIR`, `--data-dir` or portable mode (`portable.txt`) sets the folder.

### `arctic update [--beta]`

Checks GitHub Releases for a newer launcher. It only checks; installing happens in the
GUI.

### `arctic open [options]`

Starts the launcher window, detached. Options are passed through to the GUI:

| Option | Meaning |
|---|---|
| `--launch <VERSION>` | Start that version as soon as the version list loads |
| `--account <NAME>` | Select this account first |
| `--tab <TAB>` | `play`, `accounts`, `instances`, `logs`, `settings`, `about` |
| `--no-intro` | Skip the intro animation |
| `--profile <NAME>` | Open in this profile |

The launcher exe is looked up next to `arctic.exe` (as `arctic-launcher.exe` or
`Arctic Launcher.exe`) or via `ARCTIC_LAUNCHER_EXE`.

## GUI flags

The same flags work on the launcher itself, which is useful for desktop shortcuts:

```text
"Arctic Launcher.exe" --launch 1.21.4 --account Steve --profile Speedruns --no-intro
```

## JSON output

With `--json`, every line on stdout is one JSON object. Long operations emit progress
events:

```json
{"event":"progress","stage":"Downloading game files","files_done":812,"files_total":5634,"bytes_done":201326592,"bytes_total":695500800}
{"event":"launched","version":"26.3","account":"Steve","pid":12345,"log":"C:\\...\\logs\\game-vanilla.log"}
```

With `launch --wait --json` you also get the parsed game output and the exit:

```json
{"event":"log","level":"info","text":"[Render thread/INFO] Setting user: Steve"}
{"event":"window_ready"}
{"event":"exited","code":0}
```

Errors are reported as `{"event":"error","message":"…"}` with exit code 1.
