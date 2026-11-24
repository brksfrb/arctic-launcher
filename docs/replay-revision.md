# Replay revision: audit and plan

Date: 2026-11-24. Branch `replay/audit-plan`. Audited source: `main` at `8082da81` (the replay code is
identical at `4f3fdae5`, the tip when this was written).

Burak's report: the Arctic Client replay "lags like crazy", is "extremely buggy" and "really hard to use".
This document measures the first claim, lists what is broken behind the second, reviews the third against
ReplayMod and Lunar, and recommends what to build.

## 0. Verdict

- **Lag.** In every scenario measured here, recording and playback are *not* a steady frame-rate cost:
  replay code is 0.0–0.5% of render-thread samples on 1.8.9 and 26.3, and the background threads use
  10–70 ms of CPU per second. What does cost is (a) gameplay video capture on 26.3 (6–16% fewer frames),
  (b) a single frozen frame of 100–250 ms every time a replay opens or jumps backwards, which on 1.8.9 grows
  with how long you had been on the server, and (c) a compaction thread whose cost on 1.8.9 grows with
  session length (11 → 73 ms/s in six minutes, still rising). I could not reproduce continuous heavy lag,
  and section 1.6 says exactly what was not measurable here.
- **Bugs.** Thirteen confirmed, most of them "it does not work at all" rather than "it is slow". Both
  known 1.8.9 bugs are reproduced and root-caused (section 2). On 26.3 with the launcher's own mod set,
  **no replay can be opened** (B5).
- **Recommendation.** A hybrid: keep the recorder, the file format and the camera maths; rebuild the take
  lifecycle, the opening/compaction model, the playback transport and the viewer UI. Not a rewrite: the
  measured costs do not justify one, and a rewrite would re-risk 31 version targets. Not only targeted
  fixes: three of the defects are structural (section 4).
- **Small fixes now.** The two known 1.8.9 bugs have small root-caused fixes with tests. They are on
  `fix/replay-1.8.9` as a separate PR, #196 (section 2.4).

## 1. Measured causes of the lag

### 1.1 What was run

Real clients, offline, under their own Xvfb with software GL (llvmpipe), 1280x720, GUI scale 2, the same
pattern as the armorbox runs. Everything is in `mod/tools/replayaudit/`:

- `agent/`: a Java agent that hooks the buffer swap (LWJGL 2 `Display.update`, `glfwSwapBuffers`,
  `SDL_GL_SwapWindow`) and writes every frame (time, render-thread CPU), a stack sample every 5 ms of the
  render thread and of every runnable Netty/`arctic-*` thread, per-thread CPU once a second, and GC time.
- `run.py`: one client start; in `mp` mode against a local offline vanilla server (the join-pass lobby
  world, plus N wandering mobs inside the walled lobby as steady packet load). At most two JVMs.
- `-Darctic.selftest=replayaudit`: a dev-scene (only compiled with `-Pdevelopment_scenes=true`) added to
  `LegacyReplayTest` and `ReplayTest`. It runs timed phases through the same entry points as the keys
  (`Replays.toggleRecording`, `Replays.saveMoment`, `Replays.watch`, `ReplayViewer.seek`):
  `off` or `buffer → manual recording (with an F9 clip in the middle) → gameplay video → save → open →
  playback → six seeks → paused`. A second flow (`-Darctic.audit.render=true`) presses "Render" on a clip
  saved six seconds after joining.
- `analyze.py` cuts the samples by phase; `mcpr.py` prints what a saved replay contains.

Versions: **1.8.9** (Legacy Fabric, launcher instance mods: Polarium) and **26.3** (Fabric, launcher
performance set: Sodium/Polarium, Lithium, Fabric API, …; "Arctic alone" where noted). Render distance 10.
Runs (label → what): `189-off`, `189-on`, `189-long` (300 mobs, 180 s phases), `189-sp-rd20-off`
(singleplayer, render distance 20), `189-render`, `189-render-watch`, `263-off`, `263-on`, `263-long`
(Arctic alone), `263-render` (Arctic alone), `189-perf`, `263-perf` (section 1.7). Per-phase output of the six profiling runs is in
`docs/replay-revision/runs/`.

FPS on this box is software rendering and is **not comparable between runs** (the same scene gives
25–40 FPS on 1.8.9). Compare phases inside one run, render-thread CPU per frame, the share of samples
inside replay code, and per-thread CPU.

### 1.2 Steady state: recording and playback cost almost nothing

| run | phase | FPS | frame ms mean / p99 / max | render CPU ms/frame | render samples in replay code | compact / writer / Netty CPU ms/s |
|---|---|---|---|---|---|---|
| 189-off | Replay off | 40.1 | 25.0 / 28.8 / 32 | 15.27 | 0.0% | 0 / 0 / 6 |
| 189-off | same run, switched on and recording | 37.3 | 26.8 / – / 49 | 16.87 | – | – |
| 189-on | buffer | 26.8 | 37.3 / 46.8 / 54 | 23.73 | 0.0% | 11 / 3 / 8 |
| 189-on | manual recording | 27.0 | 37.1 / 47.0 / 223 | 23.60 | 0.2% | 23 / 10 / 7 |
| 189-on | playback | 28.8 | 34.8 / 47.9 / 58 | 24.54 | 0.3% | – |
| 189-long | buffer (300 mobs) | 27.4 | 36.5 / 48.7 / 60 | 22.74 | 0.0% | 39 / 5 / 13 |
| 189-long | manual recording | 28.9 | 34.7 / 45.9 / 201 | 20.93 | 0.1% | 65 / 10 / 12 |
| 189-long | playback | 32.6 | 30.7 / 39.0 / 44 | 21.09 | 0.5% | – |
| 263-off | Replay off | 25.4 | 39.4 / 57.7 / 72 | 33.99 | 0.1% | 0 / 0 / 6 |
| 263-on | buffer | 30.5 | 32.8 / 38.2 / 50 | 28.53 | 0.1% | 10 / 0 / 6 |
| 263-on | manual recording | 30.7 | 32.6 / 39.0 / 66 | 28.35 | 0.1% | 20 / 7 / 0 |
| 263-on | **gameplay video** | 28.8 | 34.7 / 56.2 / 104 | 29.82 | **3.7%** | 20 / 0 / 0 |
| 263-long | buffer (300 mobs, Arctic alone) | 19.4 | 51.7 / 73.7 / 87 | 43.57 | 0.1% | 20 / 2 / 6 |
| 263-long | manual recording | 17.5 | 57.0 / 77.2 / 151 | 48.70 | 0.1% | 23 / 4 / 5 |
| 263-long | playback | 15.9 | 63.0 / 78.4 / 100 | 47.45 | 0.5% | – |

Reading it:

- The recorder is cheap where it matters. On the network thread the replay code (re-encoding every packet,
  `Recorder.Take.packet`) is 1–3 ms/s at up to 3,900 packets/s. The writer is 2–10 ms/s. On the render
  thread there is one `SelfSample` per tick and nothing else.
- Inside one run on 1.8.9, turning Replay on and recording moved render CPU from 15.27 to 16.87 ms/frame
  (40.1 → 37.3 FPS). That 12-second window also contains the world snapshot, a clip save and a recording
  save; with software GL the save threads compete with the four rasteriser threads for the same six cores.
  Treat it as an upper bound of about 10%, not as a steady cost.
- Playback renders at the same cost as live play. `ReplayViewer.frame` (feed, self, camera, HUD) is
  0.3–0.5% of render-thread samples.
- The frame maxima of 201–223 ms in the "manual" phases are the test's own screenshot at the phase
  boundary (`LegacyScreenshots.write`, 120 ms of samples) and software rendering while a save thread
  compresses, not replay code on the render thread. The F9 clip itself: 69 ms worst frame on 1.8.9
  (`Thumbnail.png` visible in three samples), 151 ms on 26.3.

### 1.3 Hot path: the opening grows with the session (1.8.9 badly, 26.3 little)

A clip or recording starts with "the world as it was": every earlier packet that `SeekPlan` cannot prove
superseded. Entity metadata (`ENTITY_OTHER`), relative moves, block updates and everything classed
`ALWAYS` are never superseded (`SeekPlan.java:47-97`), so the opening keeps growing for as long as you
stay connected. `mcpr.py` on the files from the long runs:

| file | packets | in the opening (first timestamp) | biggest kinds in the opening |
|---|---|---|---|
| 1.8.9 "last 30 s" clip, saved 4.5 min after joining | 394,871 | **276,983 (70%)** | set_entity_data 247,494; move_entity_pos 24,284 |
| 1.8.9 manual recording, started 3 min after joining | 940,930 | 230,177 (24%) | set_entity_data 193,544; move_entity_pos 31,433 |
| 1.8.9 manual recording, started 55 s after joining (120 mobs) | 144,870 | 41,940 (29%) | set_entity_data 30,867; move_entity_pos 8,356 |
| 26.3 "last 30 s" clip, saved 4.5 min after joining | 66,864 | 22,073 (33%) | ids 0x36, 0x56, 0x08 |
| 26.3 manual recording, started 3 min after joining | 240,632 | 23,166 (10%) | ids 0x36, 0x56, 0x08 |

On 1.8.9 with 300 mobs that is about 1,280 opening packets per second of session. It has three measured
costs:

1. **Frozen frame on open and on every backward jump.** The viewer applies the whole opening inside one
   frame (`ReplayViewer.skipBootstrap`, `seekWorld`; a backward jump restarts from an empty world,
   `ReplayViewer.java:437`).

   | | opening packets | open: worst frame | jump back to start: packets applied, worst frame |
   |---|---|---|---|
   | 1.8.9, 3-min session (300 mobs) | 230,175 | 248 ms | 230,175 in 164–191 ms; frame 179–204 ms |
   | 1.8.9, 55-s session (120 mobs) | 41,938 | 157 ms | 41,938 in 79–108 ms; frame 99–128 ms |
   | 26.3, 3-min session (300 mobs) | 23,128 | 244 ms | 23,128 in 127–164 ms; frame 178–220 ms |

   During those frames in the long 1.8.9 run 63–76% of render-thread samples are replay code, and the largest single
   item is not applying packets but **reading them**: `ReplayData.packet` (one `ByteBuffer.allocate` and
   one positioned read system call per packet, `ReplayData.java:213`) is 27–52% of the samples;
   `LegacyReplayPlayback.apply` is 14–23%. On 26.3 it is the other way round (`ReplayPlayback.apply`
   40–48%, reads 2–6%; packets cost about 6 µs each there against under 1 µs on 1.8.9). Forward jumps are
   cheap on both (10–96 ms).
2. **Compaction CPU that climbs.** Every 10 s the whole ring is read, re-planned and re-compressed on
   `arctic-replay-compact` (`Recorder.maintain`, `copyRange`). CPU per second of that thread in 30-second
   buckets from the join, 1.8.9: `11, 35, 42, 52, 45, 49, 66, 62, 63, 62, 65, 73`. 26.3: flat at
   `21–26`. It is a minimum-priority thread, so it shows as frame loss only when the cores are busy.
3. **File size.** A "30-second" clip after 4.5 minutes is 27.8 MB of packets (2.2 MB zipped), 70% of it
   history.

Extrapolating the 1.8.9 slope to an hour on a busy server gives multi-second freezes on open and on every
backward jump, and a compaction thread using most of a core. That is an extrapolation from a 6-minute
run, not a measurement; the growth itself is measured.

### 1.4 Hot path: gameplay video capture (26.3)

With "Capture gameplay video" on, each captured frame (30/s) creates a full-resolution GPU buffer, copies
the colour texture into it, maps it and downsamples it pixel by pixel on the render thread
(`ReplayFrames.capture`, `ReplayFrames.java:31`; `LiveVideoSession.offer`, `LiveVideoSession.java:69`).
Measured twice on 26.3: 30.7 → 28.8 FPS (−6%) and 29.3 → 24.7 FPS (−16%), frame p99 39 → 56 ms, with
3.4–3.7% of render-thread samples in capture (`offer` 2.2–2.7%, readback 0.9–1.1%). The encoder itself is
an FFmpeg child process (x264 ultrafast, two threads); its CPU was **not** measured.

On 1.8.9 the same switch does nothing: capture never starts (bug B6), so there was nothing to measure.

### 1.5 What is *not* a cause (measured)

- Recording on the network thread, including re-encoding: 1–3 ms/s.
- The per-tick self sample and the HUD line: 0.0–0.2% of render samples.
- Normal playback, pausing, forward jumps.
- Garbage: GC time did not differ between off and recording phases beyond run noise (0.2–1.2 ms/s on
  1.8.9; 26.3 sits at 2–19 ms/s in every phase, including Replay off, because the game alone fills the
  2 GB test heap).

### 1.6 What could not be measured here

- A real GPU and Windows. llvmpipe is vertex-bound and shares cores with the game, so GPU readback cost
  (video capture, thumbnails, export) and all absolute FPS numbers do not transfer.
- Real servers: Hypixel-scale traffic, BungeeCord/Velocity server switches, anti-cheat and custom payload
  traffic, resource-pack prompts. The load here is 120–300 wandering mobs on a vanilla server.
- Sessions longer than six minutes (growth is extrapolated beyond that), and chunk-heavy travel on a
  server (section 1.7 covers singleplayer only).
- Forge 1.8.9, and every version other than 1.8.9 and 26.3.
- FFmpeg's own CPU, export throughput, and audio (no OpenAL device under Xvfb).
- 26.3 playback with the launcher's mod set: it cannot open a replay at all (B5), so 26.3 playback was
  measured with Arctic alone. Bots on 26.3 need a third JVM (ViaProxy), over the two-JVM limit.
- Lunar Client itself. Section 3 uses Lunar's published pages only.

### 1.7 Chunk-heavy singleplayer session

The existing `replayperf` scene (singleplayer, 40 teleports between two areas 3,000 blocks apart, so the
integrated server streams chunks for 128 s; then save a 10-second clip, open it, jump forward and back),
run with the agent attached and Replay on (`189-perf`, `263-perf`, 26.3 with Arctic alone):

| | 1.8.9 | 26.3 |
|---|---|---|
| render-thread samples in replay code during the session | 13 of 21,181 | 17 of 23,677 |
| `arctic-replay-compact` + `arctic-replay-writer` CPU | 41 + 24 ms/s | 65 + 35 ms/s |
| re-encoding on the local network thread | about 2 ms/s | about 3 ms/s |
| saved clip: packets, of which opening | 4,324; 1,468 | 16,952; 7,224 |
| open: total, worst frame | 274 ms, 66 ms | 949 ms, 244 ms |
| jump back: worst frame | 35 ms | 112 ms |

Chunks are superseded properly (40 teleports leave 506 chunk packets in the 1.8.9 opening), so travel
alone does not make the opening grow; entities and block changes do (1.3). In singleplayer the packets
never existed as bytes, so the recorder encodes every one; that is the 2–3 ms/s line.

## 2. Bugs

"Confirmed" means reproduced in a real client run here (run label given) or, where marked *by code*,
read directly from an unconditional code path. "Suspected" means found by reading and not reproduced.

### 2.1 The two known 1.8.9 bugs

**Known bug 1: recording/saving is rejected while already in a world.** Three separate causes.

- **B1 (confirmed, 1.8.9, run `189-sp-rd20-off`). The world snapshot trips the recorder's 64 MB limit.**
  Repro: singleplayer, render distance 20, Replay off; join; switch Replay on in the Replays tab; press
  F10. Result: log `replay: stopped: replay data arrived faster than it could be processed (64 MB queued)`,
  status "Replay buffer stopped", F10 answers "Cannot start recording: This version can't record a world
  you were already in", and F9 saves a 251 KB clip of 76 packets lasting 17 ms. Root cause: when no buffer
  exists, `LegacyReplaySnapshot.write` encodes the whole loaded world on the game thread and queues it in
  one go; `Recorder.Take.packet` counts queued bytes and ends the buffer above `MAX_PENDING_BYTES`
  (`Recorder.java:27,185`). That limit exists for a network that outruns the disk; a snapshot at a far
  render distance is larger than it by construction: about 50 KB per chunk here (441 chunks = 21.8 MB at
  distance 10), so by arithmetic around 1,300 chunks, render distance 17 with everything loaded. That
  threshold is an estimate; the failure at 20 is measured (the fixed build wrote 83.3 MB there).
- **B2 (confirmed, 1.8.9, run `189-off`). Recordings started inside a world on a busy server cannot be
  opened.** Repro: join a server with entities moving nearby with Replay off; switch it on; F10; record;
  stop; Watch. Result: `replay: packet 0 failed: … unknown packet 28`, `stopped feeding packets at cursor
  16 after 16 consecutive failures`, notice "Can't open this replay", back at the title. `mcpr.py --head`
  shows the file starting with play packets (ids 0x1C, 0x12, 0x15) *before* the login. Root cause:
  `LegacyReplayRecording.beginInWorld` attaches the take to the connection and then builds the snapshot;
  the network thread records live packets in between, ahead of the snapshot's login, and playback reads
  them in the login phase. On a quiet server (few packets in that window) it works, which is why the
  existing `replaymid` scene passes. The same misplaced packet 0 appeared in singleplayer (`189-sp-rd20`).
- **B3 (confirmed, 26.3, run `263-off`; by code for 1.9–26.2). There is no in-world start at all outside
  1.8.** `Platform.beginReplayInWorld` defaults to false (`Platform.java:60`), `FabricPlatform` does not
  override it, and `LegacyReplaySnapshot.supported()` is false from 1.9. Repro: join with Replay off,
  switch it on, press F9/F10. Result on 26.3: `take=false`, F10 does nothing useful, status stays "Join a
  world to buffer instant clips". The Replays tab says "applies from the next world join", so this is a
  missing feature the UI half-admits rather than a regression, but it is the same complaint.

**Known bug 2: loading a replay gets stuck forever on "Downloading terrain".**

- **B4 (confirmed, 1.8.9 run `189-render` and 26.3 run `263-render`; control `189-render-watch` is fine).
  "Render" on a clip whose window starts at the replay's first moment never leaves the terrain screen.**
  Repro: join a server; within the clip length (30 s by default) press F9; leave; Replays tab → **Render**
  on that clip. Result: "Downloading terrain" forever (`screen=DownloadingTerrainScreen`, `ready=false`,
  `exporting=true`, `time=0`, `start=83` for 30 s, then the test gave up); on 26.3 the same on
  `LevelLoadingScreen`. Screenshot: `docs/replay-revision/189-render-stuck.png`. "Watch" on the same clip
  works. Root cause, two steps in `ReplayViewer`:
  1. The Replays tab starts the export in the same call that opens the replay
     (`Replays.watch(file, v -> ReplayMenu.exportClip(...))`), before the world is joined. `seek()` clamps
     the target to the timeline start, which is still 0 then. A clip saved before the first compaction
     keeps its real timestamps (login at 72–83 ms into the take), so the target lies before the first
     packet. `seekWorld` treats that as a backward jump, restarts the connection, and has nothing to
     re-apply (`ReplayViewer.java:437-449`): the new connection never logs in, `ready()` stays false, and
     the terrain screen of the first join stays up.
  2. Even with a correct target, an export holds the clock at its first frame until the world is ready
     (`time = export.frameTime()`, `ReplayViewer.java:168`; `afterFrame` returns while `!ready()`,
     line 194). The packet that closes the terrain screen is the first position after the login, which is
     recorded a few milliseconds to seconds later. Nothing feeds it while the clock is held.
  Clips saved later than one clip length after joining, and manual recordings, have their whole opening on
  one timestamp and do not hit this; clips saved earlier (the file is then a raw copy with real
  timestamps), including clips from a fresh in-world snapshot, do.

### 2.2 Other confirmed bugs

- **B5 (26.3 with the launcher's performance mod set, run `263-on`, twice). No replay opens.** Watch →
  `Replay protocol handover failed (protocol=777 phase=CONFIGURATION packetId=3): IllegalArgumentException:
  Duplicate handler name: fabric:merger` → "Can't open this replay". With Arctic alone the same recording
  flow opens. Root cause: playback decodes packets itself and calls `packet.handle(listener)` directly
  (`ReplayPlayback.java:208`), outside the channel pipeline. Fabric API's networking module adds a
  `fabric:merger` handler after `decoder` on every `setupInboundProtocol`, and that handler removes itself
  only when a terminal packet passes *through* it (`FabricPacketMerger.decode`, fabric-networking-api-v1
  6.4.1). The replay's packets never do, so the handler from login → configuration is still there when
  configuration → play adds the next one. Arctic already works around the same class of problem for
  vanilla's `bundler` by hand (`ReplayCodec.java:87-90`). The launcher installs Fabric API in its 26.3
  performance set, so this is the default configuration. Other modern versions are not tested (S7).
- **B6 (1.8.9, run `189-on`, twice). Gameplay video never starts.** With capture on, status stays
  "Gameplay video ready" and `capturing=false` after 25 s. Root cause: `LiveVideo.frame` takes a
  `capturing` latch and releases it only inside the frame callback (`LiveVideo.java:104-125`); the legacy
  `LiveFrames.capture` primes its two pixel buffers on the first call and returns `true` without calling
  the callback (`LiveFrames.java:62,84`), so the latch is never released and no second capture happens.
  Introduced with the pixel-buffer readback (commit `3e6e37c1`). The file is shared by 1.8.9–1.12.2.
- **B7 (all versions, measured in section 1.3). The opening of every clip and recording grows with the
  session.** A design defect rather than a slip: `SeekPlan` supersedes chunks, teleports, head rotation
  and velocity, but not entity metadata, relative moves, block updates or anything classed `ALWAYS`.
- **B8 (all versions; seen in `189-sp-rd20-off`, rest by code). A stopped buffer stays stopped and lies
  about why.** Once a take has failed (B1, a full disk, the size limit) it remains the current take until
  you leave the world: `bufferInWorld` returns early because a take exists (`Replays.java:169`),
  `startRecording` refuses a failed take (`Recorder.java:286`), and the notice chosen is "This version
  can't record a world you were already in" (`Replays.java:187-189`). F9 still announces "Saving clip:
  last 30s" and saves whatever was written before the failure.
- **B9 (1.8.9, file inspection of the `189-off` clip plus code). In-world snapshots are incomplete.** The
  snapshot writes chunks, block entities, players, living entities and items. It does not write the tab
  list beyond visible players, scoreboard and teams, your inventory, vehicles, paintings, item frames,
  projectiles or any other non-living entity (`LegacyReplaySnapshot.java:113-121`), potion effects,
  passengers or the world border. First person in such a replay has an empty hotbar.
- **B10 (all versions, by code). The in-game list never shows the thumbnail it saves.** Every clip stores
  `arctic/thumb.png` (and pays a full-resolution readback for it); only the launcher reads it
  (`crates/arctic-core/src/replays.rs`).
- **B11 (latent, confirmed in a test run). `PacketSorter.nameOf` depends on translations.** It cuts
  `"Clientbound"` and `"Packet"` off class names using the *lengths of translated strings*
  (`PacketSorter.java:130-133`); "Packet" is "Paket", "Pakket", "パケット"… in the language files, and
  with no language loaded the key itself is used. Running the replay tests alone
  (`gradlew -p core test --tests '*replay*'`) fails `namesComeFromMojangClassNames` with `unk_wi`; the full
  suite passes only because an earlier test class happens to load English. No production caller today.
- **B12 (26.3, section 1.4). Video capture does its work on the render thread**, with a new GPU buffer per
  captured frame.
- **B13 (all versions, section 1.3). Opening and backward jumps do all their work in one frame**, reading
  each packet with its own allocation and system call.

### 2.3 Suspected (found by reading, not reproduced)

- **S1.** Follow mode never notices its target leaving: `ReplayViewer.camera` does nothing in `FOLLOW`
  (`ReplayViewer.java:349`), and the "target removed" check lives in `backend.camera`. After a jump,
  `follow(id)` can resolve to nothing while the label still says "Following …" and the free camera stays
  disabled.
- **S2.** 1.8.9: after a non-export jump to the very start, the recorded position packet arrives outside
  the seek and vanilla calls `setScreen(null)`, closing the replay menu you were scrubbing in.
- **S3.** In-world start: packets received but not yet applied by the game thread when the snapshot is
  taken are in neither the snapshot nor the recording (a frame's worth of drift). Remains after the fix
  for B2.
- **S4.** Two clocks: packets follow `ReplayViewer.time` (frame delta capped at 250 ms), game ticks follow
  `ReplayClock` (uncapped). After a hitch they disagree until the next jump.
- **S5.** `Replays.list()` opens every new `.mcpr` as a zip on the game thread (cached afterwards).
- **S6.** Memory for long recordings: about 25 bytes of index per packet plus two boxed maps; 940,930
  packets is roughly 24 MB, an hour on a busy 1.8.9 server would be several hundred. The "not enough
  memory to open it" notice exists because this has been hit.
- **S7.** B5 very likely affects every version whose Fabric API has the merger handler; only 26.3 was run.
- **S8.** `Replays.connectionClosed()` clears the viewer but leaves the replay's world up; F9/F10 there
  would snapshot the replay world.
- **S9.** Forge 1.8.9 shares these code paths and was not run.
- **S10.** A clip export starts at take time 0 even when the world first shows later (3.2 s into the take
  on 26.3 in `263-render`), so the video opens on a still. Seen in the fixed run's timeline; the output
  was not reviewed frame by frame.

### 2.4 Fixes shipped separately (`fix/replay-1.8.9`)

Only for the two known 1.8.9 bugs, each with a unit test, verified in the real client with the scenes
above. Not on this branch: <https://github.com/brksfrb/arctic-launcher/pull/196>.

| bug | change | where |
|---|---|---|
| B4 | a pending jump is clamped to the timeline start when it runs, not when it was asked for; and once an export has waited a second behind the terrain screen, packets arrive at their recorded pace until it closes | `ReplayViewer` (core, so every version) |
| B1 | the snapshot uses a new `Take.packetWhenRoom`, which waits for the writer instead of tripping the pending limit; the network path keeps the limit | `Recorder`, `LegacyReplaySnapshot` |
| B2 | live packets of a take are held while its snapshot is written and released after it, in order | new `SnapshotGate` (core), `LegacyReplayRecording` |

Verification with the fixed build (same scenes, same machine):

| run | before | after |
|---|---|---|
| `189-render` (1.8.9, Render on an early clip) | stuck on "Downloading terrain" for the whole 30 s | world ready within 2 s, export finished, 10.1 s MP4 written |
| `263-render` (26.3, Arctic alone) | stuck on `LevelLoadingScreen` | world ready within 2 s, export finished, MP4 written |
| `189-off` (1.8.9, in-world start on the busy lobby) | recording starts with play packets, "Can't open this replay" | recording starts with the login, opens, plays |
| `189-sp-rd20-off` (1.8.9 singleplayer, render distance 20) | "Replay buffer stopped", F10 refused, 17 ms junk clip | buffer ready, F10 records, 83.3 MB world written, replay opens |

Cost of the B1 fix at render distance 20: one 561 ms frame while the 83 MB world is encoded and handed to
the writer (the old build ended the buffer instead after a 191 ms frame). That frame is the snapshot's own
price and belongs to decision 4.3-1. Core tests: 447 run, 0 failed, including the three new ones.
The full `replayaudit` scene (record, save, open, play, six jumps) passes on 1.8.9 with the fixed build;
1.8.9, 1.12.2 and 26.3 compile. Forge 1.8.9 was not run.

Deliberately not in that PR: B3 (needs the design decision in section 4.3), B5 and B6 (not the two known
bugs; each wants its own PR and a version matrix), B8's wording (new strings in 14 languages).

## 3. UX review

### 3.1 Sources

- **ReplayMod**: source at commit `13ca34329be20fd3f3d2b2ccb4512c7d3f028871` (branch `stable`, version
  2.6.28) and its `docs/content.md`; ReplayStudio at `a2e312635cd305d0dd57218b810c6088b59c18b0`. Paths
  below are under `src/main/java/com/replaymod/`. The lines quoted for recording controls, the dropped
  packet list, jumping and the FAQ were re-read for this document; the rest comes from the same checkout.
- **Lunar Client**: Lunar's own pages, fetched 2026-11-24. Lunar ships two different things: upstream
  ReplayMod bundled as a module, and its own recorder/editor, Rewind.
  - <https://www.lunarclient.com/news/replay-mod-is-back>
  - <https://www.lunarclient.com/news/how-to-use-rewind-on-lunar-client>
  - <https://www.lunarclient.com/news/lunar-client-rewind-editor-content-creators>

  I did not run Lunar Client. Everything about Lunar below is what those pages state; nothing about how
  Rewind works internally, how it performs or what it does in edge cases is claimed.

### 3.2 How the three behave

| | Arctic today (code) | ReplayMod | Lunar (published) |
|---|---|---|---|
| Starting a recording | Off by default (`ClientConfig.replayEnabled`). Must be on **before** joining; then F9 saves the last 10–120 s, F10 starts/stops a recording. | Records from login by default (`recording/Setting.java`: `autoStartRecording`, `recordServer`, `recordSingleplayer` all true). | Replay Mod: "will start recording when joining a server (if the setting is turned on)" or from the pause menu. Rewind: "press Shift + R", or the pause menu, or "automatically record upon joining a server or world". |
| Starting mid-session | 1.8 only, by synthesising the world (B1, B2, B9); nowhere else (B3). | Not possible, and says so: packets are always captured from login; Start/Stop/Pause in the pause menu only place cut markers that are applied after disconnecting (`recording/gui/GuiRecordingControls.java:55-81`, `editor/gui/MarkerProcessor.java`). FAQ: "you need to start recording when joining a Server or World - otherwise some data which the Replay Files need is missing." | Rewind has a pause-menu start and Shadow Rewind ("continuously saves the last set amount of time in the background, anywhere from 30 seconds to 10 minutes"). How it obtains the world is not published. |
| Pause, markers | None. | Pause/resume (as cuts); marker key `M` storing time and camera (`PacketListener.java:464-493`). | Rewind: "pause recordings and add markers"; "you'll need to manually set keybinds for these features". |
| Knowing what is happening | "REC time · size" while a manual recording runs. Nothing on screen says the buffer is on, off or stopped; that is a line in the Replays tab. Results arrive as toasts. | Indicator icon with "RECORDING"/"PAUSED" (`recording/gui/GuiRecordingOverlay.java:179-193`). | Not described. |
| Finding recordings | A text list inside the Arctic menu: name, date, length, version, size; Watch, Render, Delete. No thumbnail (B10), rename, search or sort. A replay only opens in the Minecraft version it was recorded on. | Replay Viewer on the title screen: thumbnails, rename, delete, filter field, open folder (`replay/gui/screen/GuiReplayViewer.java`). | Replay Mod: "The Replay Viewer is located on the Main Menu". Rewind: the Rewind Editor "where you'll see all of your previously recorded Rewinds as well as projects". |
| Timeline | A bar that hides after 3 s and cannot be clicked. To scrub you press Esc for a second panel; the jump happens on release, with no preview. ←/→ 5 s, Ctrl 30 s. | Always-visible overlay: play/pause button, speed slider, clickable timeline with markers (`replay/gui/overlay/GuiReplayOverlay.java`). | Rewind: a timeline editor with tracks; details below. |
| Speed | Seven steps 0.25×–16× on ↑/↓. | Slider, 37 steps from 0.1× to 7.75× (`GuiReplayOverlay.java:137-144`). | Replay Mod: "adjustable playback speeds". |
| Jumping back | Rebuilds the world inside one frame; 100–250 ms freeze measured, growing with the opening (1.3). | Under 5 s forward: fast-forward. Otherwise a "please wait" screen and everything is resent from the start (`replay/ReplayHandler.java:700-740`). Optional Quick Mode (`Q`) precomputes entity positions and chunk state for instant jumps at reduced fidelity (`replaystudio/rar/RandomAccessReplay.java`). | Not described. |
| Camera | Free, first person (recorded player), follow (pick from at most 8 names, `ReplayMenu.java:126`). | Spectator camera entity; click a player or mob to spectate it; roll keys. | Rewind: "POV, which shows exactly what you saw during recording, including your HUD", "Free Camera", "Follow", forced perspectives; "Flight mode allows you to fly around your Rewind clip without impacting the camera path". |
| Camera paths | One kind of keyframe holding position and replay time together (`Keyframe.java`); Catmull-Rom only; add at "now", remove the nearest, clear. No selecting, dragging, preview, or speed control along the path. | Separate position (`I`) and time (`O`) keyframes on one timeline (`simplepathing/SPTimeline.java:59-61`), so the camera can move while time is slowed or stopped; Catmull-Rom, cubic or linear; path preview (`H`); keyframe repository. | Rewind: "Keyframes give you precise control over how properties change over time"; "To add a keyframe, select a point on the timeline and click the diamond icon next to any property." |
| Editing | None. | Cut and split through markers, applied on save. | Rewind: "You can trim and cut clips using the cutting tool, split and unlink media"; three tracks. |
| Export | 30 FPS fixed (`ReplayMenu.java:26`), at most 720p (`VideoExport.outputSize`), no options; FFmpeg fetched on demand. A second, separate system records the screen as MP4 while playing. | Frame-indexed render through an FFmpeg pipe with resolution, FPS, bitrate, presets, anti-aliasing, 360°/stereo methods, nametag and chroma-key options (`render/RenderSettings.java`). | Replay Mod: "You will need ffmpeg in order to render". Rewind: "File Format gives you six options: MP4, MKV, MOV, AVI, WebP, and WebM."; "Codec has four options, H.264, HEVC, VP9, and AV1." |
| Stated limits | "Applies from the next world join." | "Travelling backwards in time is not supported during Camera Paths" (docs). | "Note that UI elements only appear when the camera is set to POV or Force First Person mode." |
| Versions | 1.8.9–26.3 from one code base. | 1.7.10–26.3 through a preprocessor (`root.gradle.kts`). | Replay Mod on listed versions as a module; "Rewind is available across all Minecraft versions on Lunar Client". |

### 3.3 Why Arctic's is hard to use, in order of damage

1. **It fails at the first step too often.** Replay off by default plus "must be on before you join" plus
   B1–B3 means the natural first try (see something, press F9) answers "Nothing to save yet". On 26.3
   with the default mods, the second step (Watch) fails too (B5). ReplayMod avoids the first problem by
   recording from login by default and being explicit that mid-session start is impossible.
2. **Three products behind two keys.** F9 produces a 3D clip *and*, if video capture is on, an MP4; F10 a
   3D recording and another MP4; "Render" produces a third kind of MP4 from the 3D data. The settings for
   them share one tab and partly apply "from the next world join". Nothing on screen tells which of them
   is running.
3. **The viewer hides its controls.** The always-visible bar is display-only and disappears; the real
   controls are behind Esc; scrubbing has no preview and freezes on release; speed has no slider; the
   keys are fixed (`Replays.java:32-38`) and clash with nothing only because they cannot be rebound.
4. **Camera paths cannot be edited.** One keyframe type, no selection, no preview, no time control. The
   feature exists but cannot produce the shots it is for (orbit a frozen moment, slow-motion pass).
5. **Export has no choices** and its failure mode is a frozen loading screen (B4).
6. **The list is bare** and replays are locked to the version that recorded them, with a toast telling you
   to switch instance.
7. **Failures are toasts.** A stopped buffer, a failed save and a replay that cannot be read all appear
   for a few seconds and leave no state you can look at afterwards.

## 4. Recommendation

### 4.1 Hybrid, not rewrite, not patches only

Keep (measured cheap or simply sound):

- `Recorder`'s shape: capture on the network thread, one writer thread, compressed streams, saves on
  their own thread. 1–10 ms/s at 3,900 packets/s.
- The file: a `.mcpr` zip with `metaData.json` and `recording.tmcpr` as `[int ms][int length][packet]`,
  which is ReplayMod's container layout (`ReplayStudio`, `io/ReplayOutputStream.java:150-168`), plus
  Arctic's own entries. Existing replays must keep opening.
- The self track (`SelfSample`), `CameraPath`/`FreeCamera`, audio capture and mixing, FFmpeg handling.
- The core/adapter split and classifying packets by protocol name (`PacketSorter`), which is what lets one
  core serve 31 targets.

Rebuild (each is a confirmed structural defect):

1. **Take lifecycle** (B1, B3, B8).
2. **Opening and compaction model** (B7, B13, S6).
3. **Playback transport and clock** (B4, B5, S2, S4).
4. **Gameplay video capture** (B6, B12).
5. **Viewer and list UI** (section 3.3).

Why not a rewrite: nothing measured points at the recorder or the per-frame viewer loop; the version
adapters (`ReplayCompat`, `ReplayCodec`, `LegacyReplayCodec`: about 1,000 lines of per-version detail that
already work on 31 targets) are the expensive part to get right and the part a rewrite would put at risk.
Why not patches only: B5 is a consequence of bypassing the pipeline, B3 of synthesising worlds per
version, B7 of what the plan can supersede. Patching each symptom leaves the next one in place.

### 4.2 Architecture

```
            network thread                      game thread                     workers
  ┌──────────────────────────────┐   ┌──────────────────────────────┐   ┌────────────────────┐
  │ connection (unchanged hook)  │   │ Replays (facade)             │   │ writer             │
  │   └ encode → Take.packet     │──▶│   TakeState machine          │──▶│ state compactor    │
  └──────────────────────────────┘   │   OFF│STATE_ONLY│BUFFERING   │   │ saver              │
                                     │      │RECORDING │STOPPED(why)│   └────────────────────┘
                                     └──────────────────────────────┘
  playback:
  ReplayData (mapped file + index + state checkpoints)
      └▶ ReplaySource (core: one clock, budgeted feed, seek planner)
            └▶ adapter: channel.writeInbound-style delivery *before* the game's handler
                  └▶ ReplayFilter handler (drop / rewrite) └▶ packet_handler (vanilla, mods)
```

**A. Take state machine.** One object owns "what is the buffer doing and why": `OFF`, `BUFFERING`,
`RECORDING`, `STOPPED(reason)`. Every key press and menu line reads it. A stopped take can be replaced
without leaving the world. Bulk producers get backpressure instead of the pending limit (the B1 fix,
generalised). The limit on the network path stays, but degrades (drop the rolling window, keep the manual
recording) before it stops. Messages name the real reason.

**B. Bounded opening.** Replace "keep everything that is not provably replaced" with "keep the last
write per key, for every kind we can key":

- entity metadata by (entity, data index) — the adapter already holds the decoded packet, so it can
  report the indices; this is the dominant term on 1.8.9 (84–89% of the opening);
- block updates by block position instead of only by chunk;
- named `ALWAYS` packets that are idempotent (time, weather, world border, tab header/footer, experience,
  boss bar by id, scoreboard score by objective and name, held slot);
- relative moves: they already end at the next absolute position (`teleport_entity` does arrive on 1.8:
  1,593 times in the 180 s recording; modern `entity_position_sync` is why 26.3 stays flat), so the
  remaining work is small once metadata is keyed.

Unknown packets stay `ALWAYS`, as today. Compaction then runs on a bounded set, and its interval can
follow the clip length instead of 10 s. Acceptance: in the 300-mob scene the opening of a 30-second clip
at 30 minutes is within 10% of the one at 3 minutes; the compaction thread stays flat.

**C. Playback through the pipeline.** Deliver recorded packets into the replay connection's channel ahead
of the game's handler, the way ReplayMod does (`FullReplaySender` fires them into the pipeline;
`ReplayHandler.java:351-375` builds decoder → sender → `packet_handler`), with Arctic's drop/rewrite logic
as a handler in that pipeline. Vanilla's own protocol switching, Fabric API's merger, Forge's dispatcher
and the bundler then see ordinary traffic, and `ReplayCodec.beforeHandle/afterHandle` (the hand-rolled
handover) can be deleted. This removes the class of bug B5 belongs to.

**D. One clock, budgeted work.** A single replay clock drives packets and game ticks. Waiting for the
world to be ready is a state of that clock, not a side condition (B4 generalised). Opening and jumping
spend a per-frame budget (about 8 ms) with a progress line instead of one long frame, and read packets
from a memory-mapped file in bulk (the 27–52% item in 1.3). With B in place a backward jump applies a
bounded opening; optional state checkpoints every N seconds, built in the background at open, turn a
backward jump into "nearest checkpoint + forward", which is ReplayMod's Quick Mode idea without its loss of
fidelity because the checkpoint is a packet list, not a reconstructed world.

**E. Video capture.** One path for all versions: downscale on the GPU into a persistent target, read back
through persistent pixel buffers one frame late, hand the mapped bytes to the encoder thread in one copy,
convert there. 1.8.9–1.21.4 already has most of it (`LiveFrames`); B6 is a one-line latch fix; 1.21.5+
needs the equivalent with the new GPU API instead of a buffer per frame.

**F. UI.** Section 4.4, phase 4.

### 4.3 Decisions needed from Burak

These change defaults or behaviour users see, so they are not made here.

1. **Recording while Replay is "off".** B3 has three possible answers: (a) keep a state-only buffer
   (bounded by B, no rolling window, roughly the cost of the "buffer" rows in 1.2) whenever a world is
   joined, so switching Replay on works instantly on every version with no world synthesis; (b) port the
   1.8 snapshot to every version (large, and B9 shows how much state it has to cover); (c) keep "from the
   next join" and say so at the key press. Recommended: (a), with the 1.8 snapshot kept only as a
   fallback. It makes "off" mean "nothing saved, nothing kept on disk after leaving" rather than "nothing
   observed".
2. **Default.** Replay on by default (ReplayMod's choice) or off (today).
3. **One video system or two.** Keep live MP4 capture next to 3D clips, or make 3D the product and video
   an export of it.

### 4.4 Phases

Each phase ends with the audit scenes green on 1.8.9, 1.12.2, 1.16.5, 1.20.1, 1.21.11 and 26.3, with and
without the launcher's mod set, plus the full 31-target build and mixin check. The scenes run nightly in
CI from phase 0 on; B5 reached users because the existing scenes run without Fabric API.

| phase | content | closes | acceptance |
|---|---|---|---|
| 0. Now | `fix/replay-1.8.9` (B1, B2, B4). Separate small PRs: B6 latch; B5 stopgap (remove `fabric:merger` at each handover, as is done for `bundler`), run against Fabric API on every 1.20.5+ target; B8 wording; B11 (use literals). Audit scenes into CI. | B1, B2, B4, B5 (stopgap), B6, B8 (wording), B11 | scenes `replayaudit` off/on/render pass on 1.8.9 and 26.3 with launcher mods |
| 1. Lifecycle | Take state machine; restartable buffer; decision 4.3-1 implemented for every version; status line on the HUD for buffer/recording/stopped. | B3, B8, B9 (if 4.3-1a) | F9/F10 work or say exactly why on all six versions, joined with Replay off or on |
| 2. Transport and clock | Pipeline delivery; one clock; budgeted open/seek with progress; mapped reads; follow-mode fixes. | B5 (properly), B13, S1, S2, S4, S8 | open and jump-back worst frame under 50 ms in the 1.8.9 and 26.3 lobby scenes; opens with Fabric API and on Forge 1.8.9 |
| 3. Bounded opening | Keyed supersession (B above); compaction interval; index memory; optional checkpoints. | B7, S6 | opening size independent of session age within 10%; compaction thread flat; a one-hour recording opens in a 2 GB heap |
| 4. Viewer | Always-visible interactive timeline (click, drag with live preview on forward moves, markers, keyframes you can select and drag); speed slider; rebindable keys through the normal keybind screen; markers while recording; list with thumbnails, rename, search, "open in the right version" through the launcher; separate time keyframes and a path preview; persistent error panel instead of toasts. | 3.3 items 2–7, B10 | walkthrough script per version with screenshots |
| 5. Video | Shared capture path (E); export options (resolution, FPS, bitrate, range, camera); measure FFmpeg CPU on real hardware. | B12 | capture under 1% of render samples and under 3% FPS in the harness; confirmed on a Windows GPU machine |

Order matters: 1 and 2 remove "does not work"; 3 removes the growth; 4 is only worth doing on a viewer
that opens every time.

### 4.5 Parity, 1.8.9 to 26.3

Target: every row identical on every version. Where it cannot be, the game reason is given.

| feature | 1.8.9 | 1.9–1.12.2 | 1.13–1.20.1 | 1.20.2–1.21.4 | 1.21.5–26.1 | 26.2–26.3 | after the plan |
|---|---|---|---|---|---|---|---|
| Buffer and recording from the join | yes | yes | yes | yes | yes | yes | same |
| Start inside a world | yes, buggy (B1, B2, B9) | no (B3) | no | no | no | no | all, by 4.3-1 |
| Open and watch | yes | not run | not run | not run | not run | Arctic alone yes; with Fabric API no (B5) | all |
| Render from the first moment | no (B4) | by code no | by code no | by code no | by code no | no (B4) | all (fixed in core) |
| Gameplay video capture | no (B6) | by code no (same file) | pixel-buffer path | pixel-buffer path | buffer per frame (B12) | buffer per frame (B12) | one path |
| Video export under VulkanMod | n/a | n/a | blocked | blocked | blocked | allowed | unchanged below 26.3: frames are read back with OpenGL, which VulkanMod does not draw with (`ReplayPlayback.exportBlocked`) |

Game reasons that stay as adapter detail and are not user-visible differences:

- 1.8 sends chunks in bulk packets and unloads with an empty chunk (`map_chunk_bulk`); handled in
  `PacketSorter`.
- 1.20.2+ has a configuration phase; 1.19.4+ bundles packets; both live in the adapter and go away as a
  special case once delivery is through the pipeline (C).
- Frame readback differs by graphics API generation (GL texture read to 1.21.4, `GpuDevice` from 1.21.5,
  renderpearl from 26.3).
- The frame-end hook differs (`WindowFrameMixin` below 26.1, `runTick` from 26.1).
- Mixins added by the plan use MixinExtras `@WrapOperation`/`@WrapMethod` or `@Inject`, never `@Redirect`.

## 5. Reproducing this audit

```
J=$(mktemp -d -p /home/claude/work-tmp replayaudit.XXXXXX); export REPLAYAUDIT_JOB=$J
sh mod/tools/replayaudit/agent/build.sh $J/agent
# dev jars (JDK 25): -Pdevelopment_scenes=true, copied to $J/arctic-189-dev.jar and $J/arctic-263-dev.jar
python3 -I mod/tools/replayaudit/run.py 189 189-on --mode mp
python3 -I mod/tools/replayaudit/run.py 189 189-off --mode mp --props=-Darctic.audit.off=true
python3 -I mod/tools/replayaudit/run.py 189 189-render --mode mp --props=-Darctic.audit.render=true
python3 -I mod/tools/replayaudit/run.py 263 263-long --mode mp --mobs 300 --mods none \
        --props="-Darctic.audit.phase=180 -Darctic.audit.novideo=true -Darctic.waitlimit=300"
python3 -I mod/tools/replayaudit/analyze.py $J/out/189-on
python3 -I mod/tools/replayaudit/mcpr.py "$J/game/189-on/replay_recordings/<file>.mcpr" --head 10
```

The harness uses the launcher data root at `/home/claude/fl-run/data`, the argument files in
`/home/claude/v4-output/logs/dry-run-{189,263}.txt` and the server worlds from
`/home/claude/joinpass-work/srv`. Game logs, frame/stack/CPU files and screenshots of the runs behind
this document are kept in `/home/claude/work-tmp/replayaudit/`.
