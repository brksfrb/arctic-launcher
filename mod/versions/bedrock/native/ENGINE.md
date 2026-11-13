# Engine renderer input, version 1

The DLL exports the input functions in `include/arctic_bedrock_engine.h`. There is
currently a limited exact-build producer for standard standing cosmetic
attachments. It requires a verified player identity, native timer/camera and
matching observed DX12 depth. Full body/cape replacement and other body states remain
unfinished. The hidden test host
publishes an explicit fixture; it does not demonstrate Minecraft integration.
Do not suppress the game's renderer just because the DLL loaded successfully.

A version-specific engine adapter must acquire the actual camera, interpolated
actor model/limb poses and player UUIDs on the appropriate engine thread, upload
skin pixels, and resolve the current terrain depth to the queried overlay size.
Use the minimum depth of each source pixel footprint when downsampling so walls
do not acquire holes. Depth is conventional DirectX z/w in [0,1], near=0, far=1;
reverse-Z must be converted. The matrix and depth must describe the same camera
and frame. Clear the snapshot on disconnect, dimension change, and shutdown.
The installed `GameRenderer` projection uses OpenGL clip Z. Its native producer
converts the entire clip-Z row with `(Z + W) / 2` before publishing DirectX depth;
reverse-Z normalization follows this conversion. The recorded native and GPU
projections in `inspection/world-projection.json` corroborate the conversion,
including the 0.025 near plane. Direct engine API inputs already use DirectX
projection and must not receive this native-producer conversion again.

`arctic_bedrock_world_v1` takes this UTF-8 JSON shape (no trailing NUL):

```json
{
  "schema": 1,
  "session": "connection-unique-id",
  "sequence": 1,
  "clockMs": 125,
  "width": 800,
  "height": 450,
  "server": "localhost:19132",
  "dimension": "overworld",
  "dayTime": 0,
  "localPlayer": "b50ad385-829d-3141-a216-7e7d7539ba7f",
  "camera": [0, 0, 0, 0, 0, 70],
  "viewProjection": [1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0.01, 0, 0, 0.5, 0.5, 1],
  "players": [{
    "id": "b50ad385-829d-3141-a216-7e7d7539ba7f",
    "name": "Notch",
    "position": [0, 0, 0, 0, 0],
    "model": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0],
    "skin": "engine:fixture",
    "visible": true,
    "slim": false,
    "replaceBody": true,
    "replaceCape": true
  }]
}
```

The example matrix is orthographic fixture data. Supply the real game's
column-major view-projection matrix in gameplay. `model` is a row-major 3x4
transform from Java model coordinates in blocks (head pivot origin, y down,
face toward -z) to the world space expected by that matrix. `position` is feet
x/y/z and yaw/pitch in degrees; `camera` is x/y/z, yaw/pitch and FOV in degrees.
Optional `partPivots` and `partRotations` are six three-element arrays in
HEAD, BODY, RIGHT_ARM, LEFT_ARM, RIGHT_LEG, LEFT_LEG order, in Java model pixels
and radians respectively. Supplying rotations preserves the engine head look.
Optional `partMatrices` supplies six rigid row-major 3x4 part-to-model transforms
in blocks, in the same order. Matrices preserve the native orientation directly
when no emote is active. Emotes begin with the matrix's pivot/rotation baseline,
so bones they leave unposed keep their native pose. Scale, shear, reflection,
non-finite values and malformed matrix arrays are rejected.
Optional `availableParts` is a six-bit mask in the same order (default 63 for
existing adapters). Unavailable attachments are not drawn. Body replacement needs
all six parts; cape replacement needs the body part. The installed-build producer
can enable all six attachments when a single native hierarchy matches the
standard standing pivots, identity torso, and interpolated head look with rigid
matrices. Otherwise it retains the limited head/body path for supported standing
poses. It keeps the original game body/cape, and disables those attachments for
other poses.

Native actor identity, state vector, rotation, bounds and death state are joined
through checked EnTT storage and the current level's player list UUIDs. Server
ticks from an integrated single-player server do not advance the primary client
clock. Native animation output is sampled after the original actor callback;
two bounded bone snapshots are included in diagnostic reports. The installed
`BoneOrientation::updateBoneTransform` at `0x1BC3A30` stores the completed
bone-to-entity matrix at `+0x38` (`0x1BC3F22` through `0x1BC3F2E`). Recorded standard
player matrices match Arctic's attachment pivots and head look after pixel-to-
block translation conversion. `attachmentSamples` reports the live mapping
result without requiring or fabricating depth. Visible attachment alignment,
other body states and renderer replacement still need installed-world validation.
The automatic installed-game run `fedbde4ea1be42faab4a5a1b93af2215` verified the
live mapping: `availableParts=63`, six finite native matrices, 107 world passes,
289 actor calls, 813 completed ticks, 14 owned skin copies and 64 owned animation
copies. The game exited cleanly. Its missing `terrainDepth` prevents publishing
these samples as a render frame; `cosmeticsBound` remains false.

Optional player `cape` is an uploaded texture key, used when Arctic has no cape
override. `elytra: true` suppresses cape drawing. `capeMotion` carries the actual
interpolated Java-style cloak displacement and gait:

```json
{"delta":[0,0.4,-0.5],"bodyYaw":0,"walkDistance":0.2,"bob":0.1,"fallFlyingScale":0}
```

`delta` is chasing position minus player position in world blocks; yaw is degrees,
walk distance is the Java gait accumulator, and bob/flight scale are in [0,1].
The Java 26.2 cape math (including its sine table) and X-Z-Y cape transform are
implemented. `CapePhysics.Chaser` ports the 20 Hz following filter and teleport
reset for engines without a Java cloak component. Advance it on actual game ticks,
then interpolate; do not tick it from render callbacks. Crouch/emote body poses
come from `partPivots`/`partRotations`. No motion data is fabricated while offline.

The installed-build producer now includes `capeMotion` sampled from its own
tick-driven native cache. Native actor identity/generation and current level are
rechecked on every tick. This provides the Java following filter and bob/flight
inputs. The producer now permits an additive Arctic cape on a corroborated
standing rig only when the current native classic appearance has no cape and
the chest slot matches the game's canonical empty item. Native cape suppression
and support while wearing chest armor remain unfinished.

`replaceBody` attests that the engine adapter suppressed its original body for
that actor and frame. `replaceCape` requires either native cape suppression or
checked absence of the native cape. Without them Arctic renders only worn
cosmetics; it does not replace the underlying skin or emote the game's limbs.
Engine skins use `engine:` keys; Arctic textures/models retain their shared core
names. Upload engine skins at 64x64 after legacy skin normalization. Textures are
bounded to 128 entries / 64 MiB, evicted oldest first. Snapshots are owned copies,
must match the overlay surface, and expire after 500 ms. Sequence numbers must
increase within a session; use a new session on a new connection.

The installed-build producer copies classic RGBA8 images from the current
player-list skin reference into owned, content-addressed `engine:skin/` and
`engine:cape/` textures. Persona/custom geometry and unknown arm/image formats
are refused. Legacy skin mirroring and base/overlay alpha use the same shared
Java `SkinFormat` implementation as the Java client; memory accounting uses the
normalized image size. This enables native skin/cape preview inputs while
`replaceBody` remains false until suppression is verified. `replaceCape` can be
true for the checked absence case described above.
Frames exceeding 200,000 clipped triangles are rejected and cleared; shared menu
rendering remains available after a rejected frame.

The backend implements shared cuboid and glTF mesh geometry, vertex colors,
glow layers, alpha blending, skin outer layers, classic/slim arms, limb bends,
expressive emote poses and animated cape texture selection. Software shading
is a preview approximation. Cape physics is implemented for supplied tick data;
identity/limb mapping, matching depth, equipment rendering and game-controlled camera
remain unfinished. The offline connector can record/play Bedrock network sessions,
and its launcher editor can send public camera packets and export a new `.abr`
with baked camera keyframes. That packet route does not provide this native API's
view/depth/actor data, the Java replay backend or rendered video capture.
Explicit cape and mesh clocks support deterministic animation sampling independently
of the connector's packet recording/playback.
Emote playback also uses the adapter's animation clock. Bedrock supplies the
current world frame's clock, clears playback when the session changes, and
rejects pending remote-emote replies from a previous session. Recording/restoring
emote events and deterministic native simulation stepping are still required
for complete replay export.
Completed primary-client ticks accumulate time at their observed tick rate;
changing TPS does not rescale elapsed animation time. Interpolation uses the
current partial tick. The test covers rate changes, foreign server ticks and
world switches; live timing still requires a playable installed-game world.

## Presented-frame MP4 capture

`arctic_bedrock_capture_v1` accepts bounded control JSON, with status through
`arctic_bedrock_capture_status_v1` and `arctic-bedrock-PID.capture.json` beside
the DLL. The same directory accepts atomically published
`arctic-bedrock-PID.capture-request.json` from the launcher/CLI. Start requests
specify `op`, an absolute `ffmpeg` executable, a new absolute MP4 `output`,
`fps` (1–60) and `frames` (up to two hours); stop is `{"op":"stop"}`.

Capture reads the real DX11/DX12 swap-chain image after overlay submission.
DX12 uses the swap chain's validated direct queue, transitions PRESENT to
COPY_SOURCE and back, and retires the copy before mapping. Captures support SDR
RGBA/BGRA up to 3840×2160. Resizing or unsupported formats fail explicitly.
GPU timeout disables DX12 capture until process restart and retains in-flight
resources until safe retirement. Encoder work is bounded and runs off the render
thread. This is real-time H.264/MP4 without audio, not deterministic replay export.
The isolated installed 1.26.5203.0 game has completed a four-frame capture with
network and GDK-user denial active, after the guard's anonymous-pipe handling
was corrected. Local-world rendering and deterministic frame stepping remain
separate checks.

Indexed capture uses the same API with `mode:"indexed"`, a native `session`,
and optional `clean_view:true`. It performs no wall-clock repetition or gap filling.
Each `sample` command supplies the next zero-based `index`, `min_sequence`,
`ticks`, `partial`, `clock_ms`, and six-value `camera`. Acceptance requires a newly
uploaded Arctic world layer for that session, the exact tick/fraction/clock, a
newer world sequence than the previous accepted sample, and matching position,
angles and FOV (0.001 tolerance). Missing samples time out instead of silently
substituting pixels. The native renderer reads the actual post-overlay swap chain.

A sample's optional `frames` (default 1, maximum 120 and bounded by remaining
output frames) requests successive fresh native world layers at the same pose and
time. Every accepted layer receives a distinct index and must have a newer world
sequence; this never duplicates a previous image to fill gaps. Native receipts
include `gpu.api`, a local renderer-callback `gpu.sequence`, and the actual DX12
`gpu.bufferIndex`. These describe the captured image, independently of its world
sequence. Native timing diagnostics also include the owned game renderer's tick,
last clock time, and the world screen context's interpolation value.

Indexed requests default to `settle_frames:3` (range 1–8). At a fixed requested
camera/time, the sampler waits for identical complete RGBA images from fresh
world sequences, including coverage of every DX12 swap-chain buffer. Pixel
changes restart the settle count; repeating a world sequence never advances it.
The accepted image is the actual final readback. Receipts/manifests include its
`settled.frames`, buffer count and coverage mask. Unsettled samples time out;
no filtering, averaging or pixel changes are applied. `settle_frames:1` retains
the raw first-matching-frame diagnostic path. Real-time capture is unchanged.

Status `receipt` identifies owned/enqueued pixels; `complete` confirms encoder
success. The new sibling `*.frames.jsonl` records each index, native frame metadata,
dimensions, and SHA-256 of raw RGBA bytes before encoding. Existing outputs and
manifests are rejected. `clean_view` hides Arctic UI and its cursor; it does not
change Minecraft's native HUD. Native simulation, camera control, replay reset,
and HUD control must be coordinated separately by a playback/export driver.

`test-world.ps1 -IndexedExportProbe -HeadlessPresent -Ffmpeg ABSOLUTE_EXE` adds an installed-game
test that freezes the native timers, hides/restores the native HUD through the
owned private window, and exports the same eight-sample camera path twice. It
checks manifest/receipt identity, decodes both MP4s, and compares raw and decoded
frame hashes exactly. Evidence is retained in `indexed-export-evidence.json` on
success or failure. This diagnostic does not establish replay reset or full
deterministic replay parity.

Installed-game diagnostic `efa1ef622d70408281a614f830500db7` reached all eight
indexed camera sample receipts, with real native timing and clean Arctic world
layers, after the body/cape/emote/movement/camera/clock checks passed. Encoding
failed with Windows broken-pipe error 109; the manifest contains only its first
attempted frame and the MP4 is empty. This is retained failure evidence, not an
export pass. The same FFmpeg 7.1 binary and arguments encoded eight frames in an
ordinary headless process. Subsequent two-frame encoder-only preflights isolated
the failure to the test debugger: FFmpeg's `0x406D1388` thread-name notifications
were delivered as unhandled exceptions and terminated the encoder. Ordinary,
isolated and packaged runs passed; debugged runs failed with exit 1080890248.
The harness now consumes only first-chance x64 thread-name metadata from the
explicitly configured encoder executable. Other child/game exceptions retain
normal handling. The corrected private-desktop preflights completed in both
ordinary and packaged contexts, with five notifications and encoder exit zero.
This protocol follows Microsoft's documented native thread-naming convention:
https://learn.microsoft.com/en-us/visualstudio/debugger/how-to-set-a-thread-name-in-native-code

`encoder_host` provides these small synthetic diagnostics independently of the
game. `--debug-private OUTPUT_DIR FFMPEG --isolate` exercises the debugger, a
private desktop, isolation and the actual capture worker without starting
Minecraft. Indexed world tests now run this packaged preflight before game
startup. It verifies encoder plumbing only. Capture errors retain the encoder
exit status, and the debugger records child exit codes.
For the failed world run, `diagnosticIndexedGpuExportObserved` and
`deterministicVideoVerified` remain false. The owned game process was retired.

Installed-game run `55af1a0c87d941278c38e4456806206f` then completed two real
eight-frame indexed H.264 MP4s at 24 FPS (1920x1009 GPU input, padded to
1920x1010 for encoding). Both decoded with the expected frame count. The debugger
handled ten encoder thread-name notifications, and both encoder children exited
zero. Every paired camera pose, tick, fraction and clock matched; all paired
native view/projection matrices also matched. Exact raw/decoded frame hashes did
not all match, so the repeatability gate correctly failed.

The frozen-frame comparison localized variation to native water. In its
1,265,664-pixel comparison crop, 252,636 pixels changed, with maximum summed RGB
difference 11; terrain and Arctic geometry outside the water stayed stable.
`pixel-variation-analysis.json`, `frozen-rgb-difference.png`, the raw frame
manifests and both clips retain the evidence. This is successful native encoding,
not a deterministic replay export pass.

The renderer now coordinates the native world screen context's interpolation
argument with the controlled timer fraction and reads the constructed value back
at `ScreenContext::a` (+8). The exact constructor at `0x4815ed0` copies this
argument independently of the clock fields. Run `f8e0485b40aa43a3a5060fa51ffe8ab9`
verified native context fraction 0.25 and matching hidden-output return, and again
encoded/decoded both clips successfully. Its incoming fraction was already 0.25;
this coordination did not resolve the water variation. Only frame pairs 1, 4 and
7 had identical raw hashes. Texture animation/render preparation still needs
investigation. All preceding checks passed, including that run's frozen-frame
pair, but the indexed repeatability gate remained false. Both owned game
processes were retired; full replay playback/reset and normal-launch integration
remain unfinished.

Run `6c9d27254bff4634a59d68ff41dc5391` separated camera changes from steady
rendering. Its raw twelve-frame fixed-pose sequence had one RGBA hash across all
three DX12 buffers, with renderer tick 872 and interpolation 0.25 throughout.
The moving paths still differed. This supports waiting for native render
preparation after a camera change; it does not identify the internal cause or
prove that the settled export gate passes.

The first settled-path run, `48ef0fa163eb475ba9b45a208738c6be`, accepted three
frames with identical-image coverage of all three buffers, then failed. A
player-directory refresh gap longer than 500 ms incorrectly advanced the native
session epoch during the same world and released the controlled clock/camera.
The partial clip is failure evidence, not a completed export.

Player-directory lifetime now uses the live local player's EntityRegistry key:
the registry address and native construction ID at owner+0x160. The installed
constructor at `0x25bfc50` obtains the ID with atomic fetch-add at `0x25bfcd5`
and stores it at `0x25bfcdd`; native EntityContext comparisons also read it at
`0x19fcdbd` and `0x19fcdca`. Cache age no longer creates a new world session.
Each refresh revalidates the primary client, active level, weak local-player
entity and registry generation. Missing ownership or a failed directory copy
invalidates usable samples without manufacturing a new lifetime. The existing
200/500 ms sample freshness checks remain, and reused registry memory with a
different native ID creates a new epoch. Unit tests cover stale data, invalid
copies and address reuse. Installed run `0d58205d933a419781bd260e1c04a4e3`
retained session `native_33680_1` and registry ID 2 through both eight-frame
exports and the twelve-frame held-pose sequence. All sixteen path frames had
identical-image coverage of all three buffers, and both MP4s encoded/decoded.
Raw frame pairs 2, 6 and 7 matched; the other five still differed despite equal
native matrices/timing. The held-pose sequence again had a single raw hash.
The frozen-frame clock comparison also failed on native water in this run.
The session fix therefore has installed-game evidence; render preparation alone
has not resolved exact repeatability. The owned game process was retired.

The isolated body probe additionally observes
`FlipbookTexture::createDescription` at `0x661b740`, forwarding all four native
arguments and the complete hidden-output return. Only the exact ticking-stage
caller (`0x46e8e53`) with a live controlled renderer clock is recorded.
`renderCurrentFrame` copies its new context to GameRenderer+0x270 at `0x47a8f55`
through `0x4460390`; that alternative requires matching copied fields, excluding
padding. Run `a7d64acc0b0f49648764384a4cd12997` observed 135,290 native calls
but accepted no exact context copies; no texture phases were inferred from that
run. The read-only diagnostic now follows the constructor's clock reference at
ScreenContext+0xb0 (`0x4816018`) across context copies and render threads. It
requires the exact live controlled renderer+0x1f0 reference and matching native
session/time. `exactContextCopy` separately reports the stronger header match.
These observations explicitly set `frameOwnershipVerified:false` and do not
authorize GPU delivery or change capture matching. The observer records the
native texture phase/count/period, the actual
context fraction and the returned frame UV/blend values at +0x20/+0x24/+0x28.
It is bounded to 128 descriptions per context with an explicit omission count.
Results appear in `nativeTiming.rendererState.flipbooks`; the observer does not
alter texture animation or claim to fix water.

Installed run `7640fc50a006408c9e407f09f0c2987d` verified the clock-reference
observer: 142,843 native calls, 57,519 controlled-clock matches and 44,352 copied
descriptions. All 64 recorded textures had matching phases and returned blend
values between paired camera views, including all five views with different raw
pixels; the recorded contexts also passed the stronger exact-copy comparison.
There were 19 omitted descriptions per context, so this does not rule out every
texture. `texture-phase-analysis.json` retains paired coverage, matrix equality
and differences. The native frozen-frame clock comparison and indexed pixel
repeatability checks still failed; all other requested world diagnostics passed.
The owned game process was retired. Complete texture coverage and shader-input
comparison remain next steps; no animation state was changed to force a match.

The next trace raises the description bound to include the complete observed
83-texture set. It also adds `nativeTiming.rendererState.shaderInputs`, a read-only
copy of named CPU constants from the same controlled renderer's global buffers.
The reader validates each native name, primitive tag, scalar-array count and
stable header before copying its data pointer (+0x40). Unknown/missing constants
are listed explicitly. Native `getShaderConstants` at `0x595ce10` establishes the
per-frame slots and float tags; `0x595a0d2` checks matrix tag 11. Stores at
`0x595ae59` and `0x47a9336` corroborate the indirect data layout.
In particular, `renderCurrentFrame` writes `TOTAL_REAL_WORLD_TIME` through
global-buffer+0x238 at `0x47a933a`, independently of the mClock fields already
controlled. This is a candidate input to investigate, not a demonstrated cause
of the pixel mismatch. The reader reports `gpuSubmissionVerified:false`; CPU
constant values do not prove what a deferred GPU draw consumed.

Run `593136d92d544eeab1b27ab3ac2957ea` recorded all 83 textures, with no omissions
and identical paired texture phases. Its first constant reader rejected all
entries. Detailed diagnostics in `e1a36c74516e40d0a9f97f96118e4bcf` established
the installed non-array shape: the declared element-count field is zero for all
sixteen recognized constants. The reader now requires that observed value,
alongside the exact name/type and a stable header. Other counts remain rejected.
CPU global matrix defaults are not the per-draw native camera matrices; this
trace must not be presented as the GPU uniform stream.

The isolated clock controller now coordinates `TOTAL_REAL_WORLD_TIME` at the
exact first fmod return (`0x47a92fe`) in renderCurrentFrame. The pinned image's
own import thunk is `0xe5a6ab0`, with IAT `0xf1044e8` and hint/name `0xf10bcd0`
identifying `fmod`. The original double arguments are forwarded exactly once.
An owned controlled context on that thread, a fresh primary binding and native
3600-second divisor are required before substituting the controlled time modulo
one hour. The second fmod call and other callers keep their native result;
last-time bookkeeping is preserved. The hook is installed only for the explicit
isolated clock probe. `nativeRenderClock.totalTime` retains its actual native and
controlled results. Installed run `e1a36c74516e40d0a9f97f96118e4bcf` verified
those results and matching CPU TIME/TOTAL_REAL_WORLD_TIME values. Its repeated
camera exports still differed, so this alone is not the water fix.

Native preRenderUpdate at `0x4685680` reads the context clock at `+0xb0` and only
updates camera `mLastTime` (+0x404) and `mDeltaTime` (+0x400) for a positive
difference (`0x46856d7` through `0x46856e4`). A held time therefore retained its
previous positive delta. The isolated controller now rebases the owned primary
camera at the start of controlled time and clears that delta before the native
method. The original method computes positive step deltas normally. The expected
native output is checked after return. Foreign/uncontrolled cameras retain their
normal call, and backward controlled time is reported as requiring world
reconstruction rather than silently reanchoring an active timeline.

Run `8b40010a2fca4a909ec39c70c799cc61` verified 1,576 controlled camera updates,
with final native delta 0 and last time 56.5625. Both CPU time constants matched
the controlled timeline, all sixteen named globals were readable, and all 83
texture descriptions were recorded. Its strict frozen native GPU comparison
passed. Both eight-frame path MP4s encoded/decoded, but only paired frame 2 had
an identical raw hash, so indexed repeatability remained false. All other
requested world checks passed. `camera-time-observations.json` and
`camera-time-and-constants-analysis.json` retain the native results and paired
differences. CPU fog globals vary during the camera path; those globals are not
proof of the actual deferred material inputs. The owned game process was retired.
Per-draw GPU constants and complete camera/world render-state control remain
unfinished, along with replay playback/reset and normal-launch integration.

The isolated diagnostic now also observes the public D3D12 resource Map/Unmap
methods, retaining at most 64 mapped CPU-visible buffer resources and 256 MiB.
Upload/GPU-upload heaps and custom CPU-visible heaps are recognized; the latter
are needed by BGFX's GetCustomHeapProperties allocation path. Readable ranges
must include a complete aligned 512-byte prefix; ambiguous mappings are rejected,
and Unmap removes their CPU pointers before the native method is forwarded.
Only the frozen-clock probe samples draw data. Root CBVs are retained from exact
world-tagged draw commands on valid closed command lists, then the bounded first
and last packets are copied immediately before queue submission. The samples
are exposed as `nativeTiming.submittedShaderConstants` on that same world scope.
They include hashes/raw prefixes and explicit omission counts, rather than names
inferred from the CPU global defaults. These prefixes have no verified shader
layout and do not prove GPU execution or deterministic pixels. The original game
buffers, native draw calls and all depth ownership/freshness checks are preserved.
The real D3D12 test covers standard/custom heap mappings, writes after Map,
queue-specific sampling, immutable snapshots, overlap rejection and Unmap.

First installed diagnostic `03d03d28f4464dc4bb255cd4720da420` reached a world and
delivered 42 matching layers. It timed out waiting for a cosmetic layer, before
replay export. Its initial upload-only filter yielded no buffer prefixes; no
success claim is based on that run. The controller retired the owned game.
Run `0c944e8f0186406e9c699f860f1bca9d` verified the corrected observer in the
installed game. It observed custom write-combined and write-back heaps, retained
four CPU-visible buffers (58,869,120 bytes), and reported no capacity omissions.
Native world reports contain readable vertex/fragment prefixes from submitted
draws. All requested body/cape/emote/movement/camera/clock checks passed, including
the strict frozen native image comparison. Both eight-frame MP4s encoded and
decoded; paired raw frames 2, 6 and 7 matched, so repeatability remained false.
`submitted-constants-analysis.json` retains the bounded comparisons. Different
draw ordinals may represent different chunks/materials, and these reports precede
the accepted export frame; neither is a verified shader identity or causal proof.
The owned process was retired automatically; the empty user provider, loopback
network guards and unchanged license state were retained.

The next capture revision binds immutable diagnostics to the native publisher's
session and world sequence, retaining at most 32 frame samples. Indexed receipts
and encoded frame manifests carry `submittedShaderConstants` only when that exact
pair has a native diagnostic. The D3D12 test verifies this association survives
resource Unmap and rejects another session or sequence; installed-game validation
of the receipt attachment remains pending. This does not authorize a frame or
relax its existing camera/clock/depth/pixel checks.

Native layer diagnostics must be distinguished from the game's presented scene:
the layer can use delayed depth from a different native submission. The new
`presentedShaderConstants` receipt field requires world draws targeting the actual
DX12 back buffer on the presented queue. RTV creation attaches a unique lifetime
tag through the public resource private-data API; no back-buffer reference is
retained. Recorded targets carry those generation IDs through valid closed lists
to queue submission. A clear or invalid trace removes the association, and each
present consumes it once even if no capture is wanted. Missing evidence remains
null; it never borrows the most recent unrelated world scope. Later compositing,
all shader execution and complete scene input coverage remain unverified.

`nativeShaderLayout` records content hashes of VS/PS bytecode from successful
graphics PSO creation (including validated PSO streams). Small reflection layouts
retain declared buffer registers, spaces, variable names/types/offsets and sizes;
stripped/unsupported reflection is reported explicitly. Shader hashes follow the
PSO's COM lifetime via private data. A separately deserialized root signature must
establish root parameters 2/3 as the vertex/pixel CBVs before those declarations
are associated with the captured prefixes. Draw ordinals alone are not shader or
chunk identities. This is diagnostic metadata, not an engine-state override.

Run `229498dc533f4b28a70aa92ba133ea07` verified both diagnostic attachments in
14 of its 16 encoded frames: native publisher and presented-target scopes matched
where both were available. Each receipt was preserved unchanged in the encoder
manifest. The target observer retained no GPU resource references. Both MP4s
completed; raw pairs 2, 6 and 7 matched, and all other requested world checks
passed. The strict repeatability gate therefore still failed and the controller
retired the game. That revision's legacy FXC reflection rejected the actual DXIL
shaders and its root reader required stage-exclusive visibility, so it established
no named GPU-input mapping.

All six sampled shader hashes were then matched byte-for-byte to DXIL containers
in the installed `RenderChunk.material.bin`. Their local Windows SDK DXC reflection
was checked through the same reader; the declarations are retained locally under
that probe's `installed-shaders/`, outside the distributable bundle. The probe now
loads DXC dynamically from an already installed x64 Windows SDK when legacy FXC
cannot reflect a DXBC container. Ordinary launches do not use this diagnostic
compiler. Root visibility ALL is accepted when it includes the required stage;
actual register/space values still come from deserialization, never from a guessed
BGFX layout. Live run `f064559a9fbc4816b5a2e4dd75bcda63` has already reported
ten mapped root signatures, with vertex/pixel CBVs at registers 0/1, space 0,
both visible to all stages. That run finished with both eight-frame MP4s complete
and both diagnostic attachments present in all sixteen encoded frames. Publisher
and presented-target scopes matched for each frame; receipt metadata matched the
worker manifests. All requested body/cape/emote/movement/camera/clock checks and
the strict frozen native-image comparison passed. Only raw pair 0 differed, so
the repeatability check still failed and the owned process was retired.
`named-shader-inputs-analysis.json` records mapped DXC layouts and bounded input
comparisons: sampled time, projection, view and jitter fields matched; differing
observed value sets were FogColor, RenderChunkFogAlpha and per-draw u_model data.
Draw coverage is partial, and varying chunk sets/texture contents prevent a
causal claim. No renderer-state override was added to force these inputs equal.

`render-clock-epoch-analysis.json` establishes a separate controller bug. Before
control, the first world sample had client time 21.969 s, renderer time 262.309 s
and its fourth clock field 267.744 s. The controller later set both native fields
to client-relative 39.5125 s. The current direct assignment therefore rewinds
native clock domains and loses their existing phase. Native render-clock anchors
now preserve both original epochs and apply requested replay-time deltas, with
piecewise scale changes and explicit freeze generations. Camera-time validation
uses the mapped scaled clock. The total-time fmod input retains its own epoch.
Run `acdbffe611f04d83a35fcdb14f9b4717` verified the mapping throughout freeze,
step and hold, including no rewind at the first freeze. Its indexed camera paths
kept native clock fields 308.6302185 / 324.7326355 throughout both runs. All other
requested live checks and the strict held-image comparison passed. Both MP4s
encoded/decoded, but only raw pairs 2, 6 and 7 matched; repeatability remains false.
Reflected draw inputs still show varying FogColor, including pairs whose pixels
match. This neither proves that fog explains every mismatch nor establishes full
draw/texture coverage. The controller retired the owned process.

The pinned native fog setup routine at `0x4682200` also contains fixed per-render
blends, independent of zero camera delta (for example `0x4683d34` through
`0x4683d82`). The isolated replay controller now evaluates that original routine
from the same scalar starting state on repeated calls at one replay timestamp.
Advancing time takes the previous output as the next starting state. World session,
freeze generation and owned renderer camera identify the history; backward time
is refused. Guarded scalar ranges exclude ParticleRenderData, resource pointers,
camera transforms and terrain data. No shader constant is assigned a canned value.
The original method still runs once with its native arguments, calculating the
current view's fog. `nativeFog` and per-world `fogEvaluation` report baseline/output
hashes, ownership and repeated evaluation; these are not deterministic-video proof.
Run `270c586e9364474fb67d9c6624933b93` verified 952 native evaluations, including
946 repeats. All sixteen accepted camera sample requests retained the same fog
starting-state hash and valid native clock mapping; the offline postcheck is in
`replay-fog-state-analysis.json`. All requested live body/cape/emote/movement,
camera and clock checks passed. Both eight-frame MP4s completed; raw pairs 2, 3,
5, 6 and 7 matched, so repeatability still failed. The twelve-frame held-pose
sequence had one raw hash across all three swap-chain buffers. Native fog outputs
and sampled FogColor still varied despite a fixed starting-state hash, showing
that restoring these scalar blends does not control every input. Differences in
match counts across runs are not evidence of a causal improvement. The controller
retired the game. Full replay reset/playback, complete camera/world render-state
control and deterministic export remain unfinished.

Indexed automation now also requires a current mapped native clock and verified
fog evaluation from the same starting-state hash for every requested view. Its
per-request evidence retains both mappings. The new gate was checked against all
sixteen requests in this installed run after completion; this run's already-loaded
automation predates that gate. Native tests pass 109 cases (two manual installed
shader checks remain ignored in the ordinary suite), and Python automation passes
21. These checks do not establish full Java/Bedrock feature parity.

The local `.abr` playback helper now provides explicit reset generations and exact
paused packet steps. Reset retires the old peer and waits for a fresh connection;
step acknowledgements correlate generation/sequence and require all eligible world
updates at the requested timestamp to leave the playback cursor. They acknowledge
transport queuing, not native client processing, terrain readiness or pixels.
Owned-peer cleanup also frees the single-viewer slot and prevents a peer disconnect
from closing the entire pinned JavaScript transport. Eleven encrypted-wire tests,
four Rust offline tests and the built-CLI replay smoke passed. The latter used a
completed cache with an identical pinned lock after a cold dependency-install
startup timed out. Evidence and cache provenance are under
`replay-reset-cli-0310352026ba4cc18d33e947ac7313d3` in the local reference output.
The native replay backend, installed-game automatic reconnect and terrain-ready
binding are still missing; this helper work does not change deterministic-video
or full-parity readiness.

### Installed-world offline packet recording probe

`test-world.ps1 -LanRecordProbe -HeadlessPresent` is a separate opt-in automatic
test using a fresh copy of a completed private offline world. It enables only
that copy's multiplayer/LAN flags; Xbox and platform broadcast intents remain
zero. Normal probes retain their previous settings, including muted audio. The
controller checks the owned PID's IPv4/IPv6 UDP endpoints through `GetExtendedUdpTable` and
obtains a fresh native world sample before starting the standalone recorder.

Installed runs `73f02b9d4a1e46bebd254e6bca9e51af` and
`309ac5d95a7349eeb4c8d53c3839268e` exposed NetherNet on loopback port 7551,
rather than RakNet on 19132. Their real version-7 advertisements identify protocol
2193 / game 1.26.52, accept self-signed authentication and disable online
authentication. The second recording attempt closed before receiving any game
packets. Advertisement discovery does not establish login or replay playback.

The recorder uses the pinned pure-JavaScript NetherNet/WebRTC implementation with
no STUN/TURN services, local ICE interfaces/candidates, and offline identity only.
Diagnostics retain bounded signalling types, error codes and RTC/ICE states,
excluding SDP and identity assertions. Each attempt gets a distinct recording,
report and log. On failure the controller keeps the same owned game alive for at
most 600 seconds; it launches another recorder only when the combined SHA-256
of `record-installed-world.cjs` and `lan-transport.cjs`
changes, after refreshing native/session and kernel ownership witnesses. A
`lan-record-stop.request` in that probe directory ends this diagnostic window.
The controller still retires its owned processes on completion or error.

The loopback guard permits exact four-byte boolean `SO_BROADCAST` and
`IPV6_V6ONLY` configuration calls, the read-only IPv4/IPv6 address-list query,
and an `IPV6_V6ONLY` status read. A dual-stack wildcard bind uses mapped IPv4
loopback, preserving IPv4 reception; IPv6-only wildcard binds still use `::1`.
Only explicit LAN probes translate other interface binds to loopback. Ordinary
loopback probes continue to reject those binds. Connect/datagram destination
checks remain independent and reject external destinations in every mode.
Disposable isolated-process checks verify these cases, native IPv4-to-dual-stack
reception, IPv6 round trips, TCP/ConnectEx, source-profile preservation, user
denial and child pipes. All three modes passed in
`isolation-address-sort-b1e23b361b0b40a883e2097319de04e7`. Native unit tests pass 111 cases
(two ignored), Python tests pass 25, and release Clippy is clean. Two Node tests
exercise real IPv4/IPv6 loopback sockets and reject datagrams from the wrong
source port. These socket checks do not run Minecraft.
This probe does not establish native replay playback, deterministic export or
cosmetic parity. The distributed DLL has not been replaced with this test build.

Run `948cc7f7307648f5b4afe8103d49f5ff` subsequently passed installed-world
recording on the private desktop. The official 1.26.52 game's version-7 local
advertisement accepts self-signed identities and supplies a nonce. The recorder
now signs that nonce in its own client data and uses the same locally generated
offline transport identity throughout signalling and login. No Xbox token or
account is requested. Attempt 2 sealed `installed-world-002.abr` (5,196,991 bytes),
including 121 level chunks and 121 subchunk packets with successful, nonempty
terrain payloads, plus spawn, entities, movement and block updates. Modern
request-mode chunks use `highest_subchunk_count`; their zero `sub_chunk_count`
does not mean no terrain. The owned game was retired after the test.

The real recording exposed a camera-export decoder bug: its standalone parser
did not initialize the world's shield runtime ID from `item_registry`, so later
recipe items were decoded at the wrong offsets. Export now applies recorded
clientbound registries before reading subsequent packets. A regression failed
before the fix and passes after it. Real-recording exports under
`camera-export-installed-0f9cfca9-b3f3-4c4e-9c91-8aaa8e9126f8` preserve all other
world packet bytes/timestamps and repeat byte-for-byte at 30 Hz (158 camera
samples, output SHA-256
`b83a6c3ccc663de7da56ad39d7dedd190d61631d4e3aa35a7465eeb5ab968704`).
`replay-installed-wire-74ca7a30-0a06-4496-b787-43cd7675ccb1` verifies that a fresh
encrypted offline protocol client receives every eligible recorded packet,
including all terrain and camera samples. Queue completion can precede reception;
the check waits for actual received packet hashes. This is not installed-client
rendering or MP4 evidence. The Node suite passes 14 tests, Rust offline tests pass
four, and the rebuilt CLI's warm-cache recording/export/reset/step smoke passes
under `camera-registry-cli-22cdc97c517b41a689b875f4720c02bd`.

`test-world.ps1 -ReplayFile ABSOLUTE_RECORDING.abr -HeadlessPresent` starts the
separate installed-client replay probe. It uses an owned local playback helper
and matches the engine's own discovered LAN entry. It retains hidden
desktop input, empty user provider, loopback guards and muted audio. Native
join/world/depth observation is required before its rendering flag can pass.
Native reset integration and deterministic video remain separate gates.

Installed replay attempt `1c3d9be16a8d4624a706c78f91da1c6c` did not connect.
Queued mouse input did not switch the menu; owned-window keyboard navigation
did reach Servers. Its saved `replay-servers-enter.png` shows the signed-out
Microsoft-account requirement. The playback server received no viewer login,
and neither world nor deterministic-video readiness passed. The game and helper
were retired. The automation now uses the observed two Right/Enter sequence and
recognizes that banner immediately instead of waiting through a diagnostic
timeout. Banner recognition was checked against the actual saved Servers and
Worlds frames; the updated complete navigation sequence has not been rerun.

Read-only inspection of the pinned image locates the OreUI
`vanilla.networkWorldJoiner` registration at RVA `0xbbf50e0`, its external-server
method at `0xbbf5c80`, and its LAN method at `0xbbf7030`. The latter forwards a
copied native string to `0x1b4a7b0`; the external-server path reaches
`Social::IUserManager` through `0x1b49540`. These are research locations, not
validated callable bindings. A local native connection route, object lifetime,
thread ownership and replay-world reset still require live validation. The
signed-out Servers menu is not evidence that the native LAN route is unavailable.

Run `8bf9525069894decb95aa6e154ca1b32` observed a live native join-facet context,
its task state and shared-state reference counts. The native LAN getter uses a
vector at facet offset `0x20`, with `0xc0`-byte `OreUI::LanWorld` entries. The
binding publishes fields at offsets `0x00` (ID), `0x20` (world name), `0x40`
(owner name), `0x60` (address) and `0x80` (displayed port). The join callback
received a live context on the same thread used by the LAN-list binding; no
retained engine pointer was used. No join method was invoked in this run.

An additional loopback-only NetherNet advertisement in that same run reached
the installed Worlds page: `replay-lan-discovery.png` shows **Offline playback**
as a LAN world owned by **Arctic Bedrock Replay**. Its sender bound
`127.0.0.2:7551` and sent only to the owned game's `127.0.0.1:7551`. This was
discovery evidence only; the sender was not a playback server. The user stopped
Minecraft and the owned helpers exited. It does not establish native replay
playback, reset, terrain readiness or rendered export.

### Static NetherNet replay port (2026-10-26)

At the user's request, the following work consists only of source edits and
static review. No tests, builds, helper processes or Minecraft launches were run.

`offline/replay-nethernet.cjs` adds an offline NetherNet playback listener with a
fresh discovery nonce. Its own verifier validates the client's signed P-384
`cpk`, client-data signature, offline identity and nonce. Native self-signed
tokens do not need the generic client's `x5u` header; no OIDC fallback runs.
The per-instance RTC backend restricts candidate gathering and remote candidates
to loopback and configures no STUN/TURN services. Local announcements use unicast
to the native discovery listener. Peer retirement preserves the listener across
reset generations and cannot close a replacement viewer's slot.

The Rust helper bundle includes both new transport modules in its source digest
and writes them into the versioned cache. The launcher exposes a LAN-world
preview option, and CLI `--nethernet` selects `127.0.0.2:7551` by default. The
ordinary direct replay remains on `127.0.0.1:19143`. Startup checks the endpoint
against the selected transport. The isolated replay controller now requests
NetherNet, and the native matcher checks **Offline playback** and **Arctic
Bedrock Replay** in their separate world/owner fields, with port 7551.

These changes have not been compiled or executed. The native call, full replay
world lifecycle, deterministic MP4, and complete cosmetic parity remain
unverified; no capability/readiness flag is promoted by this static port.

Run `542857359b4d4dc3b64de4e161f9ac0c` loaded the installed world and passed live
world/depth observation but found no owned IPv4 LAN endpoint. This run also
allowed pre-bind `IPV6_V6ONLY` configuration, unlike the earlier runs with a
working IPv4 advertisement. That unproven guard allowance was reverted; the
diagnostics/retry controller and separate broadcast option remain. The test
did not reach a recorder connection and supplies no new replay evidence.

Run `8dd6bd1e8edf4d50a8ce6e40d4074287` restored the owned IPv4 NetherNet endpoint
and retained successful world/depth observation. Its first recorder attempt
received a real `CONNECTRESPONSE`, then timed out in ICE (`connecterror:9`)
without game packets. The response contained no ICE candidates. The trace also
revealed pinned Werift's fallback to all adapter candidates when an interface
filter contains only normally excluded loopback addresses. A per-instance
candidate-gather restriction fixed that behavior before sockets/candidates are
created. Attempt 2, in the same game process, offered only `127.0.0.1` and `::1`
and received another response with no candidates, but still timed out without
terrain. The changed-worker retry and fresh-session/kernel witnesses worked;
the game was retired after its bounded diagnostic window. Recording, native
playback and deterministic export remain unverified. A modern offline login's
locally self-signed token is now verified against its own P-384 key, issuer and
offline identity claims instead of incorrectly requiring the token to be empty.
This identity check has not been reached by an installed-game successful login.

Run `7c26529ad3674b499b84c1e0432d4d6b` proved IPv6 discovery against the installed
world: the owned endpoint was `::1:7551`, its version-7 advertisement was parsed,
and the game returned a real connection answer. That answer again had no ICE
candidates. During negotiation, rejected IPv4 binds rose from zero to six and
denied address-list queries rose from 62 to 66. No world packets arrived. This
identified two restrictions in the test instrumentation; it does not establish
the ordinary game's behavior or prove those were the only restrictions.

The direct Winsock query failed under the previous guard in
`address-query-before-548d3d5cef4840c9a7b220c4b62114b4`, then passed with the exact
read-only allowance. The mapped-bind check also caught this Windows provider's
one-byte boolean result length; the guard now accepts one or four bytes from
that status query and verifies the returned boolean before mapping. The installed
run above was explicitly stopped for a fresh native build; PID 31232 retired.
`lan_probe.py` now witnesses both address families and has a bounded JSON CLI for
the recorder's before/after endpoint diagnostics. Mapped dual-stack sockets are
reported in this Windows installation's IPv4 owner table. Native network reports
also retain denied-operation counters and rejected-bind families.

The 512-byte prefix covers the observed 480-byte vertex declarations as well as
the smaller fragment declarations. Older 256-byte traces remain partial and can
include bytes beyond a shader's declared fragment buffer. Those trailing bytes
must not be interpreted as shader inputs or an uncontrolled clock.

The first run's cosmetic report reached disk 35 ms before the harness recorded
its timeout. World-report polling now performs its final read before declaring
timeout and caps its last sleep to the remaining wait. The boundary regression
passes without extending native snapshot freshness or issuing another request.

`inspect-installed.ps1` uses a windowless package-context diagnostic executable
to read the installed image without starting Minecraft. It confirms unique
candidate signatures and PE/text identity, but does not install world hooks or
validate live object layouts. Runtime observation and tests are described below.

## Exact-build world observers

After installing the graphics hooks, the DLL attempts three pass-through detours
only inside `Minecraft.Windows.exe`: world rendering, actor rendering and completed
`MultiPlayerLevel::_subTick` callbacks. The PE machine/timestamp/image size and the
entire executable section's SHA-256 must match `inspection/1.26.5203.0.json`.
Different or already modified images are refused. The hooks keep original arguments,
return values and rendering; they do not suppress skins, capes or limbs.
Read-only relocation inspection found no relocations inside this build's `.text`;
ASLR therefore does not change the bytes covered by its code fingerprint.

`arctic-bedrock-PID.engine.json` reports callback counts, bounded actor render inputs
and copied camera origin/view/projection candidates using guarded memory reads.
Actor tokens are local to each world pass. Player samples additionally carry UUIDs
from the native player list, corroborated against the identity component. Nested passes are kept
separate, stale samples expire after 500 ms, and completed tick callbacks are counted
without assuming a fixed frequency. Camera data remains provisional until checked
in an actual world; matrices use the SDK's camera-relative convention. The observer
does not manufacture terrain depth. The limited producer publishes a `WorldFrame`
only when its camera, clock, component samples and depth agree.
`cosmeticsBound` and `deterministicVideoReady` remain false.
`activeHookMask` uses bits 0/1/2 for world/actor/tick observers; partial installation
errors are reported separately and already enabled observers retain their trampolines.

Native tests install actual MinHook detours in permanent fixture functions and check
forwarding, actor capture, tick ordering, and safe copied camera reads. Those tests
run headlessly without a game or saved identity. An isolated signed-out run of the
installed build has now executed 847 world callbacks, 1,664 actor callbacks and
1,305 completed level callbacks. Its primary-client clock reached 573 ticks, and
the copied player directory contained the isolated ArcticTest player. Camera
matrices/origin were finite, but the local-player sample was absent and actor
samples were rejected. Identity/pose binding, matrix semantics and matching DX12
terrain-depth extraction still require validation; these counts do not prove
cosmetics parity.

The checked component reader follows the installed EnTT hash/sparse storage and
rejects stale entity generations, broken hash chains and unavailable pages. It
reads actor type, UUID, head/body yaw, ground state, flags and bounds without
invoking the engine's throwing component getters. The active client/level/timer
read also checks client-to-renderer back references and level-control liveness.

The DX12 observer traces descriptor creation/copies, ordinary depth bindings,
render-pass clears/preservation/discards, viewports, draws and legacy barriers.
States are applied in actual queue submission order.
The adapter observes `D3D12CreateDevice` after hudhook's own discovery and binds
observers to the actual returned device before its first PSO/DSV allocations.
Late injection retains present-time discovery with its trace limitations. A
headless WARP test creates the device through this export hook and verifies
depth-writing/read-only pipelines and queue ordering before any swap chain exists.
Graphics and stream-created PSOs carry observed depth-enable/write-mask facts in
COM private data, so pointer reuse cannot attach old state to a new PSO. Unknown
pipelines are refused. A current-frame full clear plus an observed depth-writing
draw is required; a depth-read-only draw cannot claim a world-pass buffer.
Readback serializes with submissions, restores depth plane zero's state, and
leaves stencil untouched. D16/D24/D32/D32S8 are covered by actual WARP GPU tests,
including reverse-Z and padded rows. A matching world-pass scope and full-size
viewport are required; ambiguous buffers and untracked/enhanced traces are refused.
Enhanced barriers, split/local-only render passes, MSAA depth, indirect drawing,
bundles and some late-attachment cases still need coverage. Untraced indirect
draws/bundles invalidate their list. These gaps are not parity claims.
Frame transforms stay camera-relative to preserve precision; reverse-Z conversion
is applied to both depth and projection. The skin/cape renderers remain enabled.

## Installed-process initialization probe

After building the bundle, run `native/inspect-initialization.ps1`. It creates a
fresh ArcticTest configuration with a loopback cosmetics endpoint, starts the
installed executable under package identity on an unshown private desktop, and
traps its entry point after system-loader initialization. It restores the
original entry byte, holds that thread, and loads Arctic through the ordinary
development loader. Only the owned debuggee and its children are terminated.
No installation files are modified. The EXE entry point is not resumed.

This probe has verified shared-Java-client startup, the exact executable hash,
all three engine detour installations, and early DX12 device observation in the
installed 1.26.5203.0 process. Its world/actor/tick counts remain zero. It does not
verify an offline Bedrock world, native pose semantics, body/cape suppression or
deterministic replay video. Those checks still require an isolated running game.

`native/inspect-initialization.ps1 -IsolateProfile` additionally checks test-only
profile/network/user guards while keeping entry held. `-IsolatedBoot` resumes
only after those guards and all engine observers report initialization; it
observes for 180 seconds by default (`-BootSeconds 15..3600`). The desktop is
never shown. The probe records bounded raw RGBA frames with JSON dimensions in
its owned `game-profile` directory and terminates its own process tree afterward.
`-SeedProbe` can reuse game data and shader caches from a completed sibling
isolated probe, after checking its report and process lifetime. Copies reject
reparse points and enforce depth, file-count and byte limits. Original user
profiles and credentials are never seed sources. Test options retain the
`ArcticTest` name, disable LAN visibility and disable focus-loss pausing.
The isolated profile selects Simple graphics and 1x MSAA for the initial
world-hook/depth check; this does not verify Vibrant Visuals or MSAA support.

The running inspector accepts bounded keyboard/navigation requests through
`probe-input.json`, targeting only its owned private-desktop game window. A
queued Windows message is not proof that the game consumed it; screenshots
verify navigation. It does not activate a foreground window or move the system
cursor. `game-profile/probe-snapshot.request` requests a frame within the same
256-frame limit, retaining only the latest sixteen raw frames and their metadata.
Automatic startup snapshots stop after twelve frames; later frames are requested
explicitly. Sequence numbers remain unique across raw-frame slot reuse.
`python native/control-probe.py PROBE_DIRECTORY key 13` queues Enter and saves
the resulting GPU readback as PNG; use `snapshot` without a key to observe later
UI transitions. The controller needs Pillow and never reads the desktop.

The `inspect_live` example accepts that exact probe directory, game PID and
owning inspector PID. It verifies the injected DLL path, isolation report and
private startup desktop before sampling bounded module/RVA stacks. It reads no
heap dump or account data and downloads no symbols. Its optional `--stop` stops
only that verified process handle for a rebuild; the owning debugger records
the early exit and cleans up its children. Such an early exit is not a passed
full-duration boot test.
Its `--wake` option queues activation/focus lifecycle messages only to the
verified private window. Queueing is reported separately from consumption;
it never switches desktops or activates the user's foreground window.
`--focus-private` additionally joins the verified private window's input queue
briefly to establish real active-window/keyboard focus on that same unshown
desktop. It verifies those handles, detaches, and restores the helper thread's
desktop. It never calls `SwitchDesktop` or `SetForegroundWindow`, and does not
request desktop-switch access. This distinguishes actual focus from merely
posting lifecycle messages; neither proves that a world has loaded.

The opt-in sidecar enables `probe_isolation.rs` only inside this test process.
It redirects source-profile native file opens, attribute queries, deletions and
rename/link destinations to a newly created profile; profile environment values
are also local to that process. The fresh game options name is `ArcticTest`.
Master, music, sound and speech audio levels are forced to zero after seeding, so private
desktop runs do not play background audio. The user's ordinary profile is untouched.
AFD endpoint creation and network IOCTLs are denied, including on pre-existing
sockets. The GDK user provider returns access denied before contacting its
implementation. Other GDK providers retain their original calls. There are no
machine firewall, account, ACL or entitlement changes. This is test instrumentation,
not a production offline-launch mode or a general security sandbox.

`-Loopback` opts into a separate test transport for local worlds and the replay
connector. Winsock bind/connect/datagram calls validate copied IPv4/IPv6 addresses;
wildcard binds are restricted to loopback. Connected sends verify the real peer,
including when Winsock ignores an explicit destination on a stream. AFD data
operations require the checked socket/call scope or an actual loopback peer;
unknown operations remain denied. GDK user denial and file isolation stay active.
Disposable tests cover local TCP/UDP round trips, wildcard-bind restriction,
external-address denial, and encoder pipe/child operation. Installed local-world
entry with this mode now reaches real world/actor/client-clock callbacks after
the NSI enumeration fix described below. ConnectEx without a checked call
scope fails closed. The guard discovers the system provider implementation
through a synchronous `WSAIoctl` request for `WSAID_CONNECTEX`, before hook
activation, using a temporary loopback socket. A destination-checking detour on that implementation
also covers function pointers cached before activation. This preserves real
overlapped completion and initial payload transfer to loopback; external
destinations remain denied. Other provider implementations remain unsupported
and fail closed at native I/O. The fixture uses a previously cached pointer to
check a real overlapped connection, its initial bytes, subsequent connected
bytes, and external-address denial.
Three read-only interface parameter/enumeration queries (`0x120007`, `0x12000F`,
`0x12001B`) on an
object verified as `\\Device\\Nsi` are also permitted in this mode. The fixture
checks a real `GetIfEntry` before and after guard installation while still
rejecting external sends/connections. Other network device IOCTLs remain denied.
The initialized-GDK fixture also exercises enumeration through a real connectivity
hint query. Denying `0x12001B` previously made that query fail with access denied;
admitting it on the verified NSI object allows initialization to complete.

`-EmptyUserProvider` selects an explicit test double instead of failing the
user-provider factory. It exposes an empty user set, never calls the actual
XUser provider, and completes additions through the real GDK XThreading async
machinery with `E_GAMEUSER_NO_DEFAULT_USER` for silent lookup. Requests that
would display sign-in UI complete as canceled (`E_ABORT`) in this headless
test. No user handle or authentication
token is created. Store and entitlement providers are unchanged. The
separate GDK device-association provider also exposes an empty test object in
this mode: device lookups return `E_GAMEUSER_USER_NOT_FOUND` with null handles,
and change registrations complete without retaining callbacks. In the default
deny-provider mode its factory is denied because it can return user handles. The
`async_host ROOT --empty-user` fixture verifies callback delivery, failed
result retrieval, null user outputs, and empty device lookup/registration;
ordinary `async_host ROOT` separately
checks the real threading machinery without a user provider. Both manual and
background-thread completion queues are tested; `--cancel-ui` exercises UI
cancellation. This mode is
test instrumentation, not an authenticated or licensed account implementation.

Earlier isolated signed-out runs reached local-world loading, but two
world attempts timed out with `Prerequisites-32` before a playable world started.
The published disconnect enums identify stage `Prerequisites` and reason
`Timeout` (32); this is not a NetherNet session-error code.
Loopback UDP discovery was present; this does not prove a playable world.
The empty-provider experiment distinguished incomplete sign-out initialization
from transport failure. After the NSI enumeration fix, sign-out completes and
the installed client renders a local world using the isolated ArcticTest profile.
The exact-image probe also passively observes NetherNet's session-error
message formatter at RVA `0xD547D0`. It forwards arguments/output unchanged and
records only call count and integer error code. It does not alter prerequisite
checks, identity, transport selection, or connection outcomes.
Two additional passive observers cover this build's
`GameServerConnectProgressHandler::onStart`/`tick` at RVAs `0x1574400` and
`0x1574430`. They copy only validated progress flags and a module-relative
callback code address while the callback argument is live, then forward the
original call. The installed instructions corroborate the offsets. These
diagnostics distinguish startup prerequisites from actual transport negotiation;
they never satisfy a prerequisite or start a game operation themselves.
Before that fix, the observed loading handler ticked while `initialized` remained
false and its `onStart` observer did not fire. The session-error formatter did not
fire either. Two further passive observers cover the progress controller tick
at `0x4FCDB10` and its startup predicate at `0x4938850`, preserving the full dirty
flag and boolean return ABIs. A completed 900-second isolated run observed
`hasTicked=true`, `initiated=false`, and a startup predicate that remained true.
Its virtual target is the client user getter at `0x5DC29E0`; the user-state
predicate at `0xE53F30` saw a pending operation. The Xbox Live provider
had already left its sign-in state, while PlayFab remained busy. The copied
provider vtable RVAs and the exact type accessors, checked against the published
`Social::IdentityType` enum, distinguish these providers without account data.
The owned sign-in worker was waiting on an unfinished future. Changing only
the GDK XUser result or presentation status has not resolved this operation.
No predicate or pending flag is changed by these diagnostics.
A GDK connectivity-hint query before runtime initialization returned success with
a null provider. `async_host ROOT --empty-user --network --loopback` now explicitly
initializes the runtime using the version arguments and mode observed in this
build's initializer (`0xE021F0C` / `0xE0246C4`), then queries the real provider.
Manual and background completion queues and UI cancellation pass with
`networkInitialized=true`; no user handle is created. The exact-image observers
also forward the game's connectivity getter at `0xE0235DC` and its registered
change callback at `0x8EE60`, copying only initialization/level diagnostics.
The installed game has reported initialized connectivity through both paths
after the NSI enumeration fix. Connectivity hints and authentication results
remain unchanged; initialization does not prove world entry.
Subsequent world entry is verified separately through live callbacks and GPU
frames. The first automatic run saved the local world and third-person ArcticTest
model with 108 world passes, 198 actor calls and 896 completed level callbacks.
The local-player getter observed at client vtable slot `0x1F` resolves to
`0x5DAED30`. It uses a weak entity reference at client + `0x248` and component
lookups rather than a plain pointer getter. The exact-build reader now checks
the live control block, entity generation/packed range, LocalPlayer tag and
ActorOwner payload, and the returned actor's registry/entity/type back-references.
It rechecks liveness before returning and never executes an engine getter or
changes engine reference counts. An installed-game automated run resolved all
226 weak lookups and joined ArcticTest's native UUID to the player directory.
`weakLocalPlayer` reports attempts, successes and the current validation step
(8 means all checks passed); a transient step alone is not a success claim.
Body/cape replacement remains disabled.
The actor callback's camera-target input is optional diagnostic data: the pinned
dispatcher does not consume that argument before clobbering R9. Missing data
there must not discard otherwise valid actor/position/rotation samples. The
actual world camera is acquired independently.
The latest automated run, with the starting perspective reset to first person
before its F5 step, observed 120 world passes, 210 actor calls and 240 successful
weak-player reads. It copied the actual third-person ArcticTest actor, UUID,
position, rotation, AABB and movement flags. Classic skin and skeletal animation
samples were absent in that run. A subsequent installed-game run copied 62
classic 64x64 RGBA skins after accepting the native 2D image's zero depth as well
as one; depths greater than one and incorrect pixel lengths remain rejected.
The reader copies owned pixels, which alone does not prove visible replacement.
The animation reader copied all 17 bone names, affine matrices and transforms,
but rejected their default parent links. The installed constructors at
`0x1BC28F0`, `0x1BC2A60` and `0x1BC3960` initialize those links to bone `-2` and
hierarchy `Unknown` (`-1`). These observations are now retained with
`parentLinksResolved=false`; unresolved links never become invented roots or
authorize attachment traversal. Cycles, malformed links, storage changes and
foreign contexts remain rejected. Matrix space and full limb mapping still
require corroboration. `skinReader`/`animationReader` retain bounded layout and
validation evidence without native addresses or identity-provider data.
The callback uses the SDK's void return ABI and its
last boolean is `ignoreLighting`.
The depth selection trace shows the expected 1920x1009 buffer, current frame,
clear and queue, but no associated world scope. None of 48,294 observed depth
draws carried a world-hook scope. This narrows the remaining depth work to the
engine render/command-recording handoff; dimensions alone do not authorize the
buffer. `lastCapture` and `drawScopes` retain bounded selection evidence without
COM addresses. No synthetic depth or relaxed scope fallback is used.
Up to eight full-viewport depth-writing calls following observed world activity
can additionally collect bounded executable-relative caller RVAs. These locate
the deferred rendering handoff; recent world activity only gates diagnostics
and never supplies a missing scope.
An installed-game run captured two such stacks, both entering the native batch
flusher at `0xDFA47D0` from the backend submit routine at `0xDFAA6E0`. The sequence
of constant-buffer/vertex/index binds and indexed draws matches BGFX's
`BatchD3D12::flush`; its outer execution occurs in a deferred task, outside the
world observer. Read-only `.pdata` inspection now supplies exact function ranges
for these caller RVAs. A frame/task association remains necessary before depth
can be accepted; a timestamp or the most recent world pass is insufficient.

### Native submission bridge

`render_bridge.rs` observes the pinned backend routine at `0xDFAA6E0`, forwarding
all six pointer-width arguments, including both stack arguments. The third
argument (R8), saved at `+0x168` and used by the native `Frame::m_debug` load at
`0xDFAABD7`, is the BGFX Frame. The second argument is a separate backend context.
The reader copies bounded rectangles and finite view/projection matrices from
the frame's 256 views at `+0x300`, stride `0x200`, and reads the corroborated
resolution, render-item count and frame number. Header values are rechecked
after copying. Reports include sample age and native submission sequence.
No native address, callback, user-data value, or resource contents are reported.
The installed-game run `749ee87678ed4aa6be156f0e0a9898a7` observed frame 1984,
351 render items and nine perspective views. All 44,266 depth draws occurred
inside backend submissions; none occurred inside the original world hook.
Eight views matched the native world orientation and its depth-converted
projection. This spatial agreement alone does not establish a frame association.

The native view-transform setter at `0xB737CB0` forwards its eight arguments,
including the flag byte and four stack arguments. An installed-game run observed
6,623 setter calls, all outside the world observer. Its deferred caller chain
begins at `0x530C951`, then `0xE007AE3`/`0xE006931`. Direct frame-view matches
inside world callbacks remained zero; no scope was inferred from these calls.

Native view setters at `0x534F59B`/`0xB77E49B` load the same graphics-context
global at `0x11F2E860` and use its view array at `+0x1EC3080`. The inspected
Context declaration matches this layout: embedded Frames at `+0x13D40`, stride
`0xE809C0`, render/submit pointers at `+0x1D150C0`/`+0x1D150C8`, and EncoderImpl
at `+0x13D10`. The installed frame/swap routines at `0xB7274B0`/`0xB7278D0` and
encoder assignment at `0xB725DA4` corroborate these fields. The producer reader
requires distinct owned render/submit frames, the encoder's matching Frame
pointer, and stable pointer/counter rereads. World entry/exit must see the same
Frame and generation; multiple world scopes in one frame are marked ambiguous.
Backend diagnostics compare this exact frame key with up to eight recorded world
frames and their matrices. Installed-game evidence is recorded below.
`worldScopeAuthorized` remains false: view-specific draw/depth association and
safe buffering of CPU/GPU frame lag must be established before publishing depth.
The subsequent automatic run `993ba0174e954dcda458b73efeab1bee` verified the
native ownership link: 163 stable producer reads out of 230 attempts, GPU frame
1933 matched to recorded world scope 107, and all eight world views matched that
scope's view and depth-converted projection. View 9 had a different projection
and was correctly distinguished. The current CPU scope was already 115; report
sampling age and deferred rendering must not be confused with zero-lag ownership.
All 47,851 depth draws remained backend-recorded and unscoped. The run completed
115 world passes, 427 actor calls and 773 ticks, and its owned process exited.
The association now has installed-game evidence; per-view depth selection and
buffered world snapshots remain unfinished. Frame-crossing callbacks poison
both affected keys as ambiguous, including a previously recorded key, rather
than retaining a misleading unique match.

### Buffered world snapshots

`world_history.rs` now retains up to 32 owned snapshots within the existing
500 ms freshness window and a 32 MiB CPU-state budget. Camera, actor, skeletal
matrices, texture keys, cape motion and native clock values stay with their
original scope. Cape motion is sampled at world completion rather than from
the newer live physics cache when a delayed frame presents. Actor snapshots
clear their native skin read hint after copying the owned appearance.
Depth selection searches actual submitted scopes that still have a valid
snapshot; it no longer asks for the latest CPU scope. The selected snapshot
owns at most one full-resolution depth image, cleared at the start of each
present even when device discovery or capture fails. Unknown, expired, evicted,
cross-session and oversized snapshots cannot receive depth. Missing scoped
depth still clears the renderer's world input.
The automatic installed-game run `22485ade7d5a4923bda2d2af4f45c25f` completed
106 world passes, 326 actor calls and 791 ticks. Its history held scopes 91–106
(16 snapshots, 94,288 owned bytes), with no oversized snapshots rejected. No
depth scope was authorized and no world frame was published. Tests separately
verify selecting scope 107 while CPU scope 115 is newest, single-image ownership,
present clearing, epoch changes, age, count eviction and memory limits.

For the remaining per-view association, static inspection located
`ViewState::reset` at `0xDF86090`, called with its stack-local object at
backend stack base `+0x1090` (`0xDFAA801`), and the current view index stored at
base `+0x1A8`. The four-argument vertex-buffer filler at `0xDFA4590` returns a
u32 stream count. Its callers write CBV/CBF addresses before filling batch
packets: non-indexed at `+0x50/+0x58` (stride `0x70`), indexed at `+0x60/+0x68`
(stride `0x88`). Native batch counts/arrays are `+0x90/+0x98` and `+0x94/+0xA0`
on the backend's first argument. The actual command-list pointer is at `+0x30`
on the filler's first argument. The implemented binding observer below verifies
packet ownership and the view before joining actual D3D12 root CBV/CBF bindings;
these sites alone do not authorize a depth draw. No mid-function register patch or
unwind-sensitive assembly shim has been installed.

### Per-view batch bindings

`native_batch.rs` now implements that join. Its pass-through reset/fill detours
preserve the byte flag, output buffers and full u32 stream-count return. A
submission-local context retains the checked native Frame key and transient
ViewState location only while the original backend routine executes. The packet
must be the current last slot in the correct native indexed/non-indexed array,
with bounded count and aligned nonzero CBV/CBF addresses. The current view must
have the full native viewport and match the recorded world frame's orientation
and depth-converted projection, with a fresh owned snapshot still available.
D3D12 root-CBV and root-signature observers retain actual command-list bindings;
changing the signature clears the binding evidence. A draw gets a world scope
only when command list, Frame generation and both bound GPU addresses match.
Bindings shared with a non-world view become permanently ambiguous for that
submission. Nested submissions and capacity overflow fail closed.

The automatic installed-game run `55cf88defe6c437682187e5c673abd31` completed
113 world passes, 303 actor calls and 755 ticks. It verified all 62,843 packet
ownership checks, 8,547 world-view bindings and 8,446 actual scoped depth draws,
with no binding conflicts. This is real per-view GPU evidence. Presentation-time
depth remained unavailable: the native resource's final scope was null and its
full-viewport evidence had been reset. A later clear or write is suspected;
`scopedDepthLoss` counters now distinguish those causes. The next depth work must
preserve the real world contents before that mutation, in GPU command order,
with resource-state restoration and fence-safe lifetimes. No synthetic depth or
scope fallback has been introduced, and no cosmetic frame was published in that
run. `worldProducer` counters separately report native frame acceptance and
successful delivery to the shared Java renderer; they are not visible cosmetics
or deterministic-video completion flags.

If the pending loss trace confirms a depth clear after scoped writes, preserve
the contents by inserting an owned GPU copy into the same recording command list
before the original clear. The candidate must still have a full-viewport scoped
last depth writer and known state; an intervening unscoped depth write invalidates
it. Copy/restore transitions must leave the native resource in its expected state.
Retain snapshot resources in a bounded pool through recording, queue submission,
an owned fence signal after the actual ExecuteCommandLists, and consumption or
expiry. A completed copy fence alone does not authorize overwriting a snapshot
that presentation still needs. Reset of a never-submitted list must cancel its
recording reservation. Multiple copies of one source/scope require execution-order
selection; distinct matching sources remain ambiguous. Never inject a separate
queue copy after the native clear, split/reset the game's command list, reuse a
resource while GPU work can access it, or treat unknown/failed fences as complete.
Renderer diagnostics retain the actual non-TEST Present HRESULT;
the private desktop has returned `DXGI_STATUS_OCCLUDED` while GPU readbacks still
show actual startup frames.

`-HeadlessPresent` is a separate opt-in experiment requiring `-IsolatedBoot`.
After isolation and silent profile setup it translates only DXGI's positive
`DXGI_STATUS_OCCLUDED` result to `S_OK` for the test game, including TEST presents.
Both original Present methods still execute exactly once, and renderer diagnostics
retain the original non-TEST HRESULT. `headlessOcclusionAdjustments` counts the
translations. All device errors and other status codes pass through unchanged;
ordinary bundles and default probes install no filter. This only tests whether
an unshown desktop interrupts the application's frame pump; it does not establish
world entry or deterministic replay. The progress observer also retains a single
bounded stack of game-module RVAs from its first tick, without symbols or heap data.
An installed-game run with this adjustment still timed out before world entry,
with no connection-handler start callback. Occlusion alone did not explain that
startup failure.

The standalone `isolation_host` example verifies absolute and native relative
file opens, rename/delete destinations, preserved source sentinels, fresh and
pre-existing socket denial, and user-provider denial in a disposable process.
It also checks anonymous pipes and a real child stdout round trip under the
guard. Native borrowed file names may have MaximumLength zero; the guard reads
their bounded Length rather than rejecting them. Root resolution recognizes
FILE_DEVICE_NAMED_PIPE separately from filesystem and network devices, allowing
encoder pipes without admitting sockets or unresolved filesystem paths.
An installed-game run has reached the signed-out ArcticTest main menu and local
world-creation settings with Arctic HUD drawing and those guards active. The
fixed DX12 upload path has completed over 12,000 real adapter frames without a
render error or device reset. A later run with corrected NSI enumeration passed
the previously pending PlayFab sign-out operation and reached real world/actor/tick
callbacks. Full live cosmetics must still be verified. Diagnostics identified a
2x-MSAA startup depth buffer as unsupported; it is refused rather than replaced
with fabricated far depth. The observed 280-list pool fits the 512-list cache.

### Automatic isolated world test

From the repository root, subsequent runs need only:

```powershell
./mod/versions/bedrock/native/test-world.ps1
```

This selects the newest completed automatic offline world test with a stopped
owned process, then builds, hooks and drives a fresh muted private-desktop run.
The runner uses an optimized release build by default; `-DebugBuild` selects the
debug adapter. The lower-level inspector accepts `-Release` explicitly.
`-PlanOnly` prints the selected seed and parameters without starting Minecraft;
`-SeedProbe` selects a specific completed automatic test. The native loader
independently revalidates the seed before copying its allowed test files.

To test an actual attachment through the ordinary Arctic lookup and asset path:

```powershell
./mod/versions/bedrock/native/test-world.ps1 -CosmeticsProbe
```

This opt-in mode starts a temporary IPv4 loopback asset service before the game
starts. It supplies one diagnostic magenta hat for the isolated test identity,
without a sign-in endpoint or token. A headless JVM preflight exercises the
production HTTP client under the network guard first; failures stop before
Minecraft boots. The normal JVM configuration is unchanged. Only isolated probes
select IPv4 because their wildcard-bind guard restricts IPv6 to `::1`, which
cannot connect to an IPv4-mapped peer. The automated controller requests the
world-only transparent render layer and correlates its attachment pixels with
the actual GPU screenshot. It writes `cosmetic-evidence.json` only on success.
`cosmeticsParityVerified` remains false: one diagnostic attachment cannot prove
all assets, poses, cape replacement, emotes or replay parity.

The first asset run `a624335996d44cb48ca71f7c9305688a` correctly failed with zero
loaded cosmetics despite visible native player transforms and sustained world
delivery. The Java lookup failed under the probe's original network guard.
The standalone `java_lookup_host` reproduced that failure without starting
Minecraft. Windows uses a provider AFD helper handle for this connection: its
bounded request includes the original checked socket and destination. The guard
now accepts only that exact request within the synchronous checked connect call.
It separately admits bounded `SO_ERROR`/`SO_TYPE` reads, and scalar TCP_NODELAY,
keepalive and buffer options only on an actual loopback peer. General transport
IOCTLs and remote destinations remain denied. Device classification reads the
canonical object name directly, avoiding recursive filesystem path resolution.
Three consecutive headless Java HTTP round trips passed with this configuration;
the subsequent installed-game attachment evidence is recorded below.

The opt-in Java probe writes bounded `probe-client.log` diagnostics and
`probe-world-N.json/png` on request. These contain copied native frame data and
actual rendered pixels; they never supply a world, camera, pose, texture or depth
frame. The metadata includes lookup URL/status and loaded model IDs so failures
can be diagnosed without navigating menus manually.

```powershell
./inspect-initialization.ps1 -IsolatedBoot -Loopback -EmptyUserProvider -AutoWorld `
    -BootSeconds 600 -SeedProbe ABSOLUTE_COMPLETED_PROBE
```

`auto-world.py` waits for initialized hooks and completed sign-out, recognizes
the pinned English test menus from actual GPU readbacks, focuses only the owned
private window, and sends Enter/Down through the debugger's bounded input channel.
Each menu transition must be observed before another key is sent. Unknown screens
time out without further navigation. A seed is required so this flow only opens
the existing isolated test world; it does not create/delete worlds or sign in.
The controller waits for real world/actor callbacks and the primary-client clock,
saves world and third-person frames plus engine diagnostics, and reports through
`auto-world.json`. The debugger independently checks the live counters before
ending the owned process early. Only this opt-in mode bypasses the usual full
observation duration. `cosmeticsVerified` and `deterministicVideoVerified` remain
false: entering a world does not prove either feature. No interactive desktop,
global input, system cursor or real account is used.
Report publication retries atomic replacement for up to two seconds when a
Windows reader temporarily denies delete sharing, preserving the previous
complete JSON throughout. `python -m unittest discover -s . -p test_auto_world.py`
tests this with an actual Windows file handle. Eligible seed caches include the
isolated profile's `AppData/LocalLow/NVIDIA/DXCache`; no startup speedup has been
established from that addition.
The subsequent automatic run completed without recovery: 103 world passes,
284 actor calls, 743 completed tick callbacks and 14 owned classic skin copies
were saved with live-world and third-person frames. All 69 animation read
attempts in the final native report produced owned observations with unresolved
parent links explicitly marked. The controller exited successfully and the
debugger retired its owned process. This validates automated test entry and data
capture; it does not establish visible cosmetics or deterministic video parity.

The automatic run `8b7e59bd8a8542a4911f5fd1b353c24f` completed with 106 world
passes, 397 actor calls and 730 completed tick callbacks. Native batch binding
identified 6,590 actual world-scoped GPU draws. Depth diagnostics recorded 14
clears and 72 unscoped overwrites of scoped depth; no world depth reached the
cosmetics producer. This is evidence of automatic navigation and live hooks,
while visible cosmetics and deterministic video remain unverified.

The depth observer now records private plane-zero copies in the original native
command list before a locally verified scoped writer is followed by an unscoped
writer, a clear, or list closure. It restores the source state immediately and
leaves the private destination in COPY_DEST, so the same closed native list can
execute again. Submission-order metadata must independently prove the source's
scope, clear, state, dimensions and full viewport before a copy is eligible.
Later scoped writes invalidate earlier copies of that source; presentation
selects the last eligible copy for its exact buffered world observation.

The pool is bounded to 16 textures and 256 MiB. A recording remains associated
with its native list until a successful reset or a new list at that address.
Reuse additionally requires completion of a fence signaled after the actual
native queue submission. Reset and new-list bookkeeping share the submission
gate, preventing a reset from retiring ownership before that signal is queued.
Readback leases prevent reuse during capture, failed slots remain retained,
and compatible retired slots can be reallocated after a resolution change.
The real WARP GPU test draws geometry, preserves its depth, clears the original,
and verifies both pixel arrays. It also repeats the same closed list and uses a
queued GPU wait to prove a reset cannot recycle a pending copy. It also verifies
retaining 0.25 world depth before an unscoped native draw overwrites the original
pixels with 0.75. These tests validate the mechanism; installed-game acceptance
and visible cosmetics still
require their own live evidence.

Installed-game run `597f58598e8f4b80a71ef599e542baaa` verified the submission
gate and bounded pool in a real isolated world: 35 copies were recorded and
accepted, seven were read back, and six world frames reached the shared Java
renderer. All 16 slots were used and later safely reused, with three reusable
at the final sample; there were no exhausted or failed copies, unknown native
submissions, rendering errors or device resets. This debug run completed 67
world passes and 186 actor callbacks. Private readback barriers are now excluded
from the native-resource cache, preventing those textures from evicting actual
game depth. No native body/cape replacement or deterministic-video completion
claim follows from world-frame delivery.

The first optimized run `f0729b7c913649fe92c844a231272e5a` delivered 13 native
world frames and verified 10,510 world-scoped draws. It accepted 86 copies but
also refused 209 reservations after all 16 slots were occupied. The recording
pool now coalesces copies of the same source on the same recording list into
one texture. Each new copy advances its generation, so earlier markers cannot
authorize bytes overwritten later in that list. The WARP test records two world
scopes with different actual occluder footprints, verifies one owned texture,
rejects the superseded scope, and reads only the final footprint. The current
automatic installed-game run is checking this change; world-frame delivery
still does not prove native cosmetic replacement.

Run `4730841083584a0bb639d1444df09af7` delivered 12 frames but still exhausted
the pool 260 times: all 16 slots remained owned despite coalescing. Final native
list destruction is now observed through the actual graphics-list interface's
public Release entry (canonical IUnknown can have a separate tear-off). Before
calling the original, the hook captures only bounded slot generations; after a
zero return it publishes lock-free retirement events without dereferencing the
destroyed object. Submission-gated retirement happens after native work has its
fence signal queued. Old delayed events cannot retire a newer reservation, and
fence completion plus the readback lease remain mandatory for reuse. A real WARP
test destroys a submitted list, verifies its owner is retired, and still reads
the original copied geometry. Installed-game verification follows separately.

Run `a1a62cceaa7b4e2d89ec72c11d41a90f` delivered 12 world frames but observed
no final-release retirements; all 16 slots were owned and 127 copies were refused.
The actual game therefore still needs a lifecycle fix. Bounded diagnostics now
report each owned list's closed/unknown state, event count, list/copy frame ages,
and whether a draw arrived after observed closure. For the first eight such
draws, only booleans comparing the live public Reset/Release entries with the
installed targets are retained. This distinguishes retained pooled lists from
alternate interface implementations without guessing when old GPU commands
cease to be executable. The pool cap and lifetime requirements remain enforced.

### Native pooled-list retirement

Run `90fa9319568f411b911d07d96fa417c1` observed zero draws after an observed
close. Its retained, closed list owners therefore do not indicate a missed
Reset call. Static inspection of the pinned image located allocation at
`0xDFA3D60`, queue completion at `0xDFA1540`, and fence polling at `0xDFA4480`.
Completion removes submitted contexts whose fence is finished from the used
ring and pushes them into the free ring. Allocation pops that free ring and
unconditionally resets the allocator and native list before recording again.
Both rings hold at most `0x110` entries, with an inclusive head/tail and the
`0x110` empty sentinel. The actual public command queue is the first pointer
in this native queue object, corroborated by its ExecuteCommandLists call at
`0xDFA42C6`; the field at `+0x2F8B790` is resource-state management instead.

`depth_queue.rs` observes the completion function without changing its timeout,
fence argument or return byte. It requires a stable used-ring observation,
known actual submitting queue, submitted context, and current reservation
generation before the native call. After success, the context must be absent
from the used ring, present in a stable free ring, retain its list/fence pair,
and have a completed native fence. Only then can submission-gated bookkeeping
retire that generation. A later reset, resubmission or reservation invalidates
the saved ticket. Reuse still waits for Arctic's own GPU fence and any active
readback lease. A completed fence without the ring transition does not retire
ownership. This is a version-specific native ownership contract, independently
of the public COM lifetime path.

The ABI/ring tests and real GPU depth test pass in debug and release (47 native
tests total). The latter verifies that completion alone leaves ownership intact,
that a used-to-free handoff retires a still-live list, and that an old ticket
cannot retire a newer reservation. These are mechanism checks; the automatic
installed-game run must separately establish retirement and sustained delivery.

Installed-game run `3509d7461f8d4e268a30aaf63a6e7d79` verified 128 native
handoffs with zero rejected handoffs, exhausted reservations, failed copies,
rendering errors or device resets. It recorded 146 copies, accepted 139 final
markers and delivered 25 world frames to the shared renderer. Thirteen slots
were sufficient (201,477,120 estimated bytes); the final sample retained two
reusable slots. The run completed 101 world passes and 285 actor callbacks and
its owned process exited. This verifies recycling through the installed game's
native free ring; it does not establish visible cosmetics or deterministic video.

Reservations now stamp their actual submitted generation. A context can enter
the native used ring just before public submission; its ticket may wait for
exactly that first submission, but cannot authorize an unsubmitted generation or
another execution. The real GPU test rejects retirement before submission.

Automatic world tests now observe native delivery for ten seconds after F5
(`-WorldObserveSeconds` changes the bounded duration). Completion requires at
least five new deliveries, a delivery within the last two seconds, and no new
pool exhaustion. The inspector independently checks the delivery counters and
reported delta before early cleanup. The final report includes the observation
duration, baseline/final delivery and exhaustion counts, and last-delivery gap.
`worldDepthObserved` is distinct from the still-false cosmetics/video flags.

Run `0f0af9f8488a4319a27b93c454e58715` passed this stronger test with 102 new
deliveries during ten seconds (six initially, 108 finally), a zero-millisecond
last sampled delivery gap, and zero exhaustion throughout. It verified 590
native handoffs without rejections, used 14 slots within the existing budget,
and ended with nine reusable slots. There were 331 world passes, 1,152 actor
callbacks and no copy failures, render errors or device resets. The inspector
checked the counters and retired its owned process. Native visible cosmetics,
full pose/replacement support and deterministic replay video remain unfinished.

The full 240,802,816-byte text fingerprint now uses Windows CNG streaming SHA-256
with the same expected digest. In the current development build, the read-only
`hash_profile` probe measured 22,153 ms for the previous Rust SHA-256 path and
687 ms for CNG. Native RGBA texture content keys use the same backend; standard
vectors and multi-chunk input are tested against SHA-256 reference results.

Installed-game run `903bbc7454d948be81c894dde75991a8` completed automatically
with the diagnostic asset service. Its four HTTP requests fetched the real
in-world UUID's lookup, catalog, geometry and texture through the shared Java
client. `probe-world-2.png/json` recorded 45,922 nontransparent world-layer pixels
and the loaded `arctic_probe_hat`; the GPU frame contained 136,104 matching magenta
pixels within its scaled/dilated attachment footprint. Visual inspection confirms
the cuboid above the native player's head. This is actual standing attachment
evidence, with native world camera/pose/depth, rather than a supplied fixture frame.

The ten-second depth observation delivered 89 new frames (3 initially, 92 finally)
with zero exhaustion. The final sample reached 116 deliveries, 388 world passes,
1,234 actor callbacks and 696 native queue handoffs, with zero failed copies,
render errors or device resets. The owned game process exited; the helper service
was cleaned up. The test profile remained ArcticTest, muted, with an empty user
provider and loopback-only transport. The native library's 49 tests, Java's 26
tests, shared-client smoke, standalone isolation test and release Clippy passed.
Full body/cape replacement, other poses/assets, emotes and deterministic replay
video remain unfinished; the full-parity flags deliberately remain false.

### Animated cape observations

`test-world.ps1 -CapeProbe` adds a two-frame red/green cape to the isolated asset
service and requires each phase to reach both the actual world layer and GPU
capture. It also runs the diagnostic head-attachment check. The metadata records
the selected shared-core texture, native client clock, attachment conditions and
tick-derived cape motion. A later feature failure preserves completed world and
depth checks in the report, while the overall state remains an error.

The native absence check requires stable zero-sized/zero-byte cape image data,
an empty cape ID and cleared persona/cape-on-classic flags on a recognized classic
skin. Empty images in this installed build retain an allocated buffer, so a
non-null pointer with zero length is permitted. This is independent of failed
image decoding: unreadable or inconsistent state cannot authorize drawing.

The armor reader checks `ActorEquipmentComponent` and the actual container's
`getItem` vtable entry. The observed player getter at `0x19DF740` forwards slot
zero specially; chest slot one uses its bounded vector at `+0x1A0/+0x1A8` with
`0x98`-byte ItemStacks. The other corroborated getters at `0x19DF320` and
`0x25A5100` use their inspected vector offsets. All return the canonical empty
item at `0x11D65D50` for invalid slots. The reader compares the chest's visual
header with that live empty reference and rechecks the container/vector/item;
it never invokes an inventory method or changes equipment. Unknown containers
and nonempty chest slots keep cape rendering disabled.

Runs `e33bb19425184548baf143ff276174f2` and
`437121178b4547d79cb6298957c26791` failed the cape check and identified the empty
blob representation and actual armor getter. Run
`50b76261c2f4469fbd28641ee636d99d` passed with native cape absence, an empty chest,
six native part matrices and both GPU-observed animation phases. Its green phase
at native clock 35,182 ms contained 245 world-layer pixels and 862 matching GPU
pixels; the red phase at 36,064 ms contained 183 and 489 respectively. A nearby
zombie obscured most of the player. This establishes animated delivery through
real scene depth, while complete placement/alignment and moving physics remain
unverified. `cape-evidence.json` explicitly leaves `movingCapePhysicsVerified`
and full parity false.

This run delivered 87 new world frames during the ten-second observation and
308 by final cleanup, with 1,872 native queue retirements, zero exhausted/failed
depth copies, zero render errors and zero device resets. Its owned process
exited, and the loopback service was stopped. The profile stayed muted and
signed out. Replacing an existing native cape, other body states, all armor,
native emotes and deterministic replay video still require implementation and
their own installed-game verification.

The installed `query.model_scale` callback (`0x221A9A0`, callable entry in table
`0xE801840`) reads `RenderParams+0x110`. The initializer at `0x1BBB780` stores the
model-scale argument there, separately from frame alpha at `+0x108`. The live
value is `0.0625` (model pixel units); this does not prove the global player scale.
Mojang's public player entity uses a separate `scripts.scale` of `0.9375`.
The current world root still derives scale from bounds; observing the installed
global model transform is the next alignment requirement, rather than treating
the unit conversion or a public default as the evaluated native transform.


### Automated native model transform observation

The scripts now prepare only the fresh isolated test world's `level.dat` before
world entry: peaceful difficulty, forced creative mode, and mob spawning off.
A bounded little-endian NBT parser preserves all other bytes and refuses malformed,
duplicate or escaping paths. The source seed is unchanged.

Run `6e77eed8ec484bad91f2cacbfda5aee8` captured the actual player renderer map
and `ActorRenderData::applyGlobalAnims` at RVA `0x1D5CF90`. That callback is before
resource scale and cannot supply the final cosmetic root by itself. Run
`735d8a4a27b24fbb9df469d915ff81d4` automatically installed both fixed observers,
including the five-argument `DataDrivenModel::render` at `0x1C35FF0`, and captured
503 owned final matrices. Its renderer vtable/render RVAs were `0xE942B70` and
`0x558D300`; the bounded map resolved on its first attempt.

The draw observer validates actor/context/component/model/renderer ownership,
UI/item exclusion and world scope. It copies the native camera's current matrix
stack tops while the draw owns them. Its three- and five-argument trampolines
preserve native arguments and output. Only the independently hashed installed
image and fixed prologues can enable them; the temporary arbitrary-RVA request
channel has been removed.

The recorded draw used model-pixel scale 0.0625 and evaluated root scale 0.9375.
The previous AABB-derived estimate was about 0.086 blocks too high in this pose.
The live producer now converts the actual draw matrix from pixel to block units
and preserves its camera-relative translation. It requires the same view and
projection as the owned world snapshot; a failed match hides attachments.
`inspection/native-transform.json` retains the installed observation for checking
pixel/block vertex equivalence and foreign-camera rejection. Native bodies remain
enabled. This does not yet prove all poses, armor, body replacement or parity.

`test-world.ps1 -MovementProbe` includes cape and final-transform checks, then
uses the owned private window's message queue for bounded movement keys. Each
key is released before acknowledgement. The script requires native displacement
and nonzero cape motion input, records evidence and refuses to call a queued
message consumed. It never uses the system keyboard or interactive desktop.


Run `15a15d29a961407cae729ac508489a7d` passed the complete automated movement
probe on the muted private desktop with an empty user provider. The shared world
input used `modelSource: native-model-draw`, scale 0.9375, all six part matrices,
native cape absence and an empty chest. Both animated cape phases passed static
GPU checks. The ten-second depth check delivered 87 new frames without pool
exhaustion; 493 final model matrices were copied during the complete run.
Forward input was obstructed, so the script automatically tried sideways input;
the observed player displacement was 1.0539 blocks with nonzero cape displacement
0.04899. This establishes private scripted movement consumption and cape input,
not moving cape GPU alignment or physics parity. The movement screenshot does
not establish visible cape rendering; `movingCapePhysicsVerified` stays false.
The owned game process was confirmed stopped after the automatic run.

The current native suite has 56 passing tests; the Java suite and shared-client
smoke, four Python checks and release Clippy also pass. Body suppression, other
pose/armor coverage and deterministic native replay/video export remain pending.


### Exact-frame moving cape evidence

Probe layer requests now receive a marker only when that layer is included in
`NativeBridge.frame`'s returned pixels. The JNI bridge retrieves it with those
pixels; the native renderer publishes it only after successful texture upload
and consumes it at the immediate post-render GPU readback. The isolated frame
metadata records `worldRequestSequence`. The script requires the same marker as
its world-layer report, preserving matching GPU metadata beside each PNG.
It does not use a later screenshot as evidence for an earlier moving pose.
The headless Java smoke checks capture, next-frame reset and stale-request
rejection. A Python regression rejects a newer frame with the wrong marker.

Run `815fea59ad404484acf6ab9ae51678cb` passed static hat, both animated cape
phases, final native root and exact-frame moving cape checks. Sideways movement
was 1.1299 blocks, cape delta 0.05865. The tagged movement layer contained 24,484
cape pixels, with 96,847 matching GPU pixels in a 99,936-pixel dilated footprint.
This resolves the earlier movement screenshot's timing ambiguity. Full physics
coverage and cosmetics parity remain unverified.

The held-key worker now runs outside the debugger event pump. Previously a
2-second sleep there could stop game execution at the next debug event; earlier
world reports advanced only 592 and 1,074 native milliseconds over those holds.
The movement verifier now also requires at least 1,000 native milliseconds of
clock advancement for its 2-second held-input attempt.

The cape chaser now uses native feet Y from the AABB, retaining state-vector X/Z.
A change in eye height alone must not add vertical cape velocity. A regression
checks stationary stance changes and true feet motion separately; the standing
movement and interpolation filter tests remain in place.


Run `4e0f304b67af4182a9e3d247e4a1751c` also passed with the input-worker and
feet-height fixes. A 2-second forward hold advanced the native clock by 2,298 ms
and moved the player 1.01295 blocks. Walk distance reached 0.60777, matching the
Java-style 0.6 factor. Nonzero cape delta 0.07534 was rendered: 13,973 cape pixels
in the owned layer, 55,107 matching GPU pixels in a 58,260-pixel footprint, linked
by request marker 2001. Static hat, both cape phases, final root and sustained
depth also passed. No pool exhaustion or render errors were reported. The owned
process was retired after the automatic run. All 57 native tests, five Python
checks, the Java tests/marker smoke and release Clippy pass. The complete-pose
physics/parity and deterministic replay/video flags remain false.

### Automated navigation and native skin submission tracing

`test-world.ps1 -MovementProbe` runs navigation and held input entirely through
the owned inspector. `auto-world.json` now includes automation/input mode and
per-stage elapsed times; the PowerShell controller prints stage changes. Each
recognized menu's saved GPU frame is the frame used to choose its action, avoiding
a redundant capture and a possible transition mismatch. Bootstrap failures are
reported before a live PID/engine report is available.

The exact-image `Model::dataDrivenDraw` at `0x1c44f70` takes seven pointer-width
arguments. Its root RenderParams/ActorRenderData/component links, non-UI/non-item
ownership, geometry hierarchy key and complete standing pose are checked before
classifying the call. The installed geometry alias is `default`; hierarchy 54
is an opaque map key, not a small vector index. The normal asset API has delivered
the diagnostic skin to Java's loaded texture registry with the actual world UUID.
Experimental replacement readiness is session-bound and expires after 250 ms.

Run `e422320b031f487ebf1c34dd4131eef8` failed its body check. It suppressed 160
CPU skin draws; all 160 actor guards finished without ambiguity, and body records
survived in the owned CPU history. None reached a matching selected GPU world
frame. This rules out loss in the actor guard, but does not establish body
replacement. The experimental CPU skip stays confined to explicit BodyProbe.

A passive `EncoderImpl::submit` observer at `0xb722470` now forwards all six
arguments unchanged and traces calls made within the root skin method. Only a
stable producer Frame key, exactly one global render-item allocation and exactly
one encoder submission authorize an item. The installed copy is
`Frame+0xc0700+index*0x80`; an actual backend batch draw pointer and world scope
must match before it counts as a GPU packet match. Concurrent, dropped, reused
and nested unrelated submissions cannot acquire the root actor's ownership.
This trace changes no native draw, binding, packet or return value.

Java world state now distinguishes a temporary missing matched depth frame from
a level disconnect. Missing depth hides geometry and readiness while preserving
the session/playback state. A real disconnect or session change still clears
playback. Headless Java tests cover both paths; live emote continuity remains
unverified.

Run `d8dc356f618743b28de686be05a7f9ae` passed the fully scripted movement
check with the passive submission observer installed. Its seven acknowledged
inputs selected the seeded world, changed camera and held forward movement.
Total controller time was 244.7 seconds: initialization 63.2 seconds, menu
navigation 17.6 seconds and world loading 147.4 seconds. The ten-second depth
check delivered 92 frames with no exhaustion. Movement advanced the native clock
2,295 ms, moved the actor 1.00399 blocks and produced cape delta 0.03960. Marker
2001 joined 13,522 layer cape pixels to 53,184 matching GPU pixels in a 56,440
pixel footprint. Static hat, both cape phases and final native root also passed.
Owned PID 13616 was confirmed stopped after the automatic run.

The basic encoder observer recorded 156,731 calls but none inside the 428
observed root model calls; no render-item ownership or GPU body match was
established. The body path may submit indirectly or defer to another callback.
This negative observation must be resolved before enabling GPU body suppression.
The current 62 native tests, 7 Python checks, Java tests/marker smoke, hidden DX11
ABI/skin/depth/cape/disconnect smoke and release Clippy pass. The preview DLL was
rebuilt with SHA-256
`1cd17d4f11f0468176d938c4905d6827c8d752b5e83522022b6b3885c8a71fec`.
Complete cosmetics, body pose coverage and deterministic native replay/video
remain unverified.

Run `a26aa70638b8410494bb4c699715e235` passed the automatic movement/cape/root
checks with a temporary observer on the older sorted-list bone-vector copy.
That copy received zero calls, and the basic BGFX submit received zero calls
inside the world callback. The older `SortedMeshDrawList` branch is therefore
not a demonstrated bridge for this installed renderer; the temporary hook was
removed. It must not be described as live skin submission ownership.

The current experiment hooks the skinned draw-description constructor at
`0x5e9e4e0`, called at `0x12eabf6`. Its 21 machine arguments, stack-widths and
native this-return are corroborated by the installed call and constructor.
The output contains the evaluated world matrix at +0, mesh at +0xb0, vertex
count at +0xb8 and copied shader color at +0x17c. The constructor reads that
color from the actual ScreenContext shader-color reference. A candidate must
match the active actor's final root matrix, input mesh/count and source color.

The root model method is now always forwarded. Explicit BodyProbe readiness
may reduce only the owned output description's color alpha (+0x188);
no global shader color, native bone, packet binding or queue is changed. The
write is read back and committed only if its actor's body record is accepted;
rejection or Rust unwinding restores the old alpha. Ordinary launches provide
no replacement candidate. A successful fixture alone does not prove that this
native material respects that alpha or that body pixels are replaced.

Run `e935b171546942a38cc5c26b1135b1c6` reached the new constructor 3,400
times within actor ownership. Static hat/cape/root checks passed, but its body
check failed before the output guards accepted a description; no alpha writes
were applied. PID 26256 was confirmed stopped. The output diagnostics now retain
the native input/copied count and return/mesh match booleans before evaluating
the matrix and color checks. The count is an opaque native draw parameter: it
is compared bit-for-bit and never sizes a read/allocation or changes a draw.
The 21-argument regression now installs a real detour and checks all argument
widths and the native return through its trampoline. Alpha rollback is checked
for both ownership rejection and unwinding. The body verifier explicitly keeps
`nativeBodyAbsenceVerified` false: seeing blue replacement pixels alone cannot
prove that every original-body pixel/depth contribution has disappeared.

Run `2673150f2a094a059fd1e20292c3f50a` established 2,725 exact root/color
matches. Its opaque native draw count was 0, and the source color was white with
alpha 0.65. The alpha guard initially required exactly 1 and applied no writes;
the body check failed. The owned observation is retained in
`inspection/skinned-description.json`; rollback now covers its exact partial
alpha bits as well. PID 6404 was confirmed stopped.

Run `405b17f305c044c086bbc80150b78a18` reduced 750 owned descriptions to alpha
zero and finished 150 corresponding actor guards without ambiguity. Body records
survived in CPU history, but no body-tagged GPU world frame reached the verifier.
No body GPU image was produced. PID 27332 was confirmed stopped. Exact-zero
opacity may be culled before the scoped GPU draw. The current probe tests
positive alpha 0.0001 while preserving native mesh/count/matrices and the same
depth-ownership requirements. Pixel absence and full body replacement remain
unverified; the numerical alpha write alone is not evidence for either claim.

Run `0b72334cf5eb4d94813cc35d8fd7133e` applied 790 positive-alpha changes but
still selected no matching body frame. Readiness was then found to be coupled
to depth availability: `bodyPlan()` returned no loaded skins during every missing
depth frame. The asset getter now retains loaded texture readiness from the
same live session's retained world. Rendering still requires `WORLD.renderable`;
disconnect/session changes and native session/pose/readiness checks remain.

Run `125b8237baa548fe94a6a47629094cc0` passed the complete automatic BodyProbe
after that readiness fix. Marker 1500 joined 46,779 blue body layer pixels to
183,725 matching GPU pixels in a 194,644-pixel footprint. Its actor body record
contained five owned draw descriptions with the loaded diagnostic skin. The
installed observer matched 2,045 descriptions and changed 1,880 after readiness
became available. Copied native current/previous bone spans were observed too;
the last mesh had two matrices in each span, matching their source bytes.

The same run passed hat, both cape phases, final root and movement checks.
Forward input advanced the native clock 2,179 ms and moved 1.00859 blocks, with
cape delta 0.08396. Marker 2001 joined 14,078 cape layer pixels to 55,337 matching
GPU pixels in a 58,664-pixel footprint. The ten-second depth check delivered
76 new frames with no exhaustion. The body image was inspected: custom blue
skin, diagnostic hat and cape are visible. This does not establish absence of
all original-body pixels/depth; that flag remains false, alongside all-pose,
cosmetics-parity and deterministic-video flags.

The new optional BodyAbsenceProbe uses acknowledged fixed private-JVM modes.
Native mode clears replacement readiness; suppression mode retains readiness
but omits the Arctic world layer; replacement mode restores normal diagnostics.
The alpha parameter is independently bounded to [0,0.01] in Java and native
readiness validation. No arbitrary address, camera, pose or depth input is
accepted. Opt-in `depth: true` world-layer requests save a copied f32le input
depth plane beside the PNG/JSON with the same request marker. The comparison
requires stable session/camera/root and records RGB/depth changes under an
eroded body mask. Its partial diagnostic does not set the full-body absence or
all-pose replacement verification flags.

Run `11822ee4572144e583db554824a60a43` passed the replacement skin check but
failed the new native-only/suppression-only comparison. The native body remained
visible in the captured suppression image: only 2,335 of 41,462 body-mask depth
pixels moved farther, 5,015 moved nearer, and only 1,428 of 165,508 GPU-mask
pixels changed. Alpha 0.0001 and the native enabled dithering flag did not remove
the original geometry. PID 25920 was confirmed stopped. This is a failed
suppression strategy, not evidence for native-body absence.

The next explicit BodyProbe method is `bones`. Current and previous borrowed
matrix spans at description offsets +0xc0/+0xd0 must match the constructor's
actual source spans. Their counts remain unchanged and bounded to 256; only the
copied pointer fields are substituted with a stable, aligned 16 KiB palette.
Each matrix collapses vertices to a point while keeping homogeneous w=1. Native
world/bounds/mesh/count/color fields and source animation matrices are preserved.
The allocation lives for the process lifetime for deferred consumers. Rejection
or unwinding restores both copied pointers. Unit tests cover alignment,
unchanged sources, acceptance, rejection and unwinding. GPU color/depth absence
is still unproven until a live comparison passes; all parity flags remain false.

Run `e0f664fe967841c287c8174f2c979443` passed the automatic BodyAbsenceProbe
with the `bones` method. Under the eroded 45,818-pixel body mask, 45,204 depth
samples moved farther and zero moved nearer. Of 182,932 GPU-mask pixels,
171,175 changed. The suppression-only image was inspected and showed the world
behind the actor with no original body. The replacement image was inspected
and showed the blue skin, magenta hat and animated cape without Steve's cuffs.
The native observer matched 2,145 descriptions and changed 1,895. These are
bounded standing-pose observations, not all-pose or full parity verification.

That same run passed movement (0.97332 blocks), native clock advancement
(2,305 ms), moving cape input (delta 0.083945) and matching cape GPU pixels
(57,316 in a 60,732-pixel footprint). The ten-second depth observation delivered
71 frames with zero exhaustion. All navigation and eleven acknowledged actions
were scripted; 62.1 seconds were startup and 146.6 seconds world loading out of
247.8 seconds total. PID 25652 was confirmed stopped. The native, Java and Python
suites passed 65, 31 and 8 checks respectively; release Clippy and the hidden
renderer/JVM smoke passed. All-pose, cosmetics-parity and deterministic-video
flags remain false.

### Static recorded appearance mount and export wiring (2026-10-26)

The source now includes the portable archive reader and replay appearance mount
that were still missing in the earlier snapshot-export entry. ZIP entries are
read without extraction, with bounded indexes/bytes, SHA-1 content addresses and
SHA-256 descriptors. Models, images, actor IDs and recorded look/emote state are
prepared privately before replacing the active mount. Local-player remapping is
explicit; live catalog fetches cannot overwrite mounted textures or definitions.
Recorded cape frames, fitted cosmetics, idle animation and emote phase use the
checked export dispatch time, otherwise an explicitly held snapshot time.

The PID-scoped appearance mailbox and C ABI serialize save/load/time/clear/state
on the Java client thread. Receipts retain 32 terminal requests and identify world,
sequence, request and operation. Owner tokens are validated at the host and native
boundary and compared on Java replacement/time/clear. Host completion checks the
returned owner. Load preparation expires before commit after 20 seconds; executing
file I/O is not cancelled by a host timeout. A failed load preserves its old mount.

The editor's optional snapshot selection is copied into the immutable video job.
The job checks that no appearance is mounted, loads under a private owner, rejects
a snapshot later than the playhead, and adds manifest/time requirements to each
frame's existing clock/dispatch/camera fence. Java only emits that identity when
the world layer rendered. Both native and host fences additionally require its
`renderReady` flag: visible body/cape plans must acknowledge the requested keys,
arm model and complete attachment matrices; old replacements cannot qualify.
Missing native replacement, partial poses and custom cape/elytra combinations
remain unready. This is a bounded draw-state requirement, not pixel/terrain proof.
Export pins the final appearance time before resuming
native clocks and clears its own mount during cleanup, comparing the owner first.

Explicit unmount/replacement retains immutable texture pixels for buffered native
draws. Admission budgets the union of old/new keys and retained texture history
to 64 MiB, including eventual clear. It refuses further changes on exhaustion;
session end releases history. This avoids dropping an acknowledged image before
native history finishes delivering it, without removing the existing native
body/cape ownership or experimental suppression gates.

All additions in this entry were source edits and source inspection only. No
tests, builds, helper scripts, dialogs or Bedrock process were run. Automatic
appearance-history capture/recording association, all native poses, packet/terrain
application barriers and deterministic moving MP4 evidence remain incomplete.

### Static skin geometry, glow ordering and input continuation (2026-10-26)

`PlayerSkin` supplies continuous standard Java-layout head/body/limb boxes for
replacement rendering. `AvatarRenderer` selects shared `SkinLimbs` split geometry
only for an emote's bending joints. Classic/slim arm UVs, normalized HD texture
coordinates and outer-layer inflation remain in the checked shared geometry
path. Native pose, appearance ownership and suppression gates are unchanged.

`Raster3D` now resolves cosmetic base transparency before its separate emissive
queue, following Java's ordered glow layer. The pass still uses copied terrain
and opaque geometry depth; it does not claim native light-map equivalence.
Native cape entries advance at most once for a completed tick, ignore older
callbacks and reset their chase endpoints when intermediate ticks are missing.

The bridge now distinguishes Windows virtual keys from shared GLFW UI codes for
function keys, punctuation, navigation, keypad digits/operators, locks and both
modifier sides. Native key messages resolve Control/Alt using the extended-key
bit; middle and side mouse press/releases join the existing input queue. Binding
names and held-state lookup share the same table. Keypad Enter cannot yet be
distinguished from ordinary Enter by the current 256-key native state packet.

This continuation used source inspection and edits only. No build, test, helper
or Bedrock process was run. Full feature parity, native all-posture rendering,
armor/held-item attachment posing and deterministic rendered export remain
unfinished; verification flags are unchanged.

### Static recorded resource-pack replay port (2026-10-26)

The helper now indexes `resource_packs_info`, `resource_pack_data_info` and
`resource_pack_chunk_data` before the recording's first `start_game`. It validates
UUID/version identity, descriptor sizes/counts, complete chunk coverage and the
descriptor's SHA-256 over ordered archive bytes. Per-chunk payload and packet
hashes detect changes after indexing. It retains offsets/descriptors/hashes,
without expanding archives or fetching external resources. Incomplete archives
fail startup before the replay server advertises readiness.

Each fresh viewer negotiates the original pack offer/stack and requests its own
chunks. The loopback server clears per-pack CDN URLs, sends the recorded download
descriptor and drains a bounded request queue; it does not replay old chunk
requests or send old transfer packets on the world timeline. Client completion
releases the existing pack-handshake wait. Refusal, invalid identity/index,
conflicting descriptors/chunks, timeout and exhausted retries stop that viewer.
World reset creates a fresh per-viewer negotiation state using the same checked
archive index. The Rust helper fingerprint and file installation include the new
module; replay state forwards archive counts/sizes to the launcher editor.

The archive budget is 64 packs, 512 MiB per pack, 1 GiB combined and 65,536 chunks;
the containing recording remains subject to its 2 GiB limit. This only restores
download bytes actually present in the recording. CDN-only or cached packs are
still missing unless their bytes were recorded. Native world reconstruction,
Arctic cosmetic/emote state capture, resource audio export and repeatable moving
video remain incomplete. This continuation was source-only: no build, test,
helper or game was run, and broad verification flags remain false.

### Static replay self visuals and cosmetic frame clock (2026-10-26)

`replay-self.cjs` projects the recording connection's visual state onto the fresh
viewer. It handles legacy/current player-list layouts, correlates the bootstrap's
unique entity ID as well as the recording UUID, remaps only self entries and
preserves their existing `Skin` payloads. Pre-bootstrap skin, held-item and native
emote-list state is coalesced until the local world exists. Timed self skin,
equipment, supported arm animations and native Bedrock emote events then enter
the same outbound queue before the replay step's marker. Remote actor IDs and
ordinary world packets retain their existing path; arbitrary client requests
are not replayed as server operations. World reset creates a new adapter.

The live offline relay now maps an owned `player_skin` update from the incoming
game UUID to its selected offline upstream UUID. Its recorder serializes that
normalized update rather than retaining a pre-normalization wire buffer. Custom
appearance selection still suppresses replacement updates. Authentication
envelopes remain excluded. This preserves protocol appearance data that actually
exists; absent initial skins and Arctic HTTP catalog/emote history are not
reconstructed or claimed as captured.

`CosmeticClock` derives cape/idle time from the exact dispatch-only receipt in a
held native sample. It requires matching session, completed tick/fraction, live
clock ownership fields, both controlled timers, native dispatch completion and
a bounded recorded timestamp. Other samples retain the native world's clock.
World geometry, preview and cape readiness all select cape frames using this
time. Live Arctic emote start times and native cape physics retain their existing
clock domains, pending actual recorded emote/physics state reconstruction.

The Java capture tag adds `controls.cosmeticClockMs` only after rendering the
world layer. Host and native `ControlFence` implementations accept an optional
matching cosmetic clock requirement, constrained to the same dispatch-only
marker timestamp and both held timers. Protocol indexed export requires it, so
an old Java bundle, skipped world layer or mismatched animation phase cannot
satisfy that capture request. Required and observed controls remain in the
existing indexed manifest. Helper installation/fingerprinting includes the new
self-state module.

Only static source reads and edits were performed. No tests, builds, helper
programs or Bedrock processes were run. Native application/terrain completion,
Arctic appearance-history capture, held-item/armor emote posing and repeatable
moving-video verification remain incomplete; broad verification flags are false.

### Static appearance asset retention and portable snapshot export (2026-10-26)

Bedrock opts into the shared `AppearanceAssets` store through `Platform`.
Normal skin/cape and cosmetic model/texture/animation fetches reuse only raw
files whose SHA-1 equals the catalog's content address. Downloads are checked
before caching; skin normalization and cape preparation retain their existing
paths after fetching. Local launcher textures are copied onto the existing
background worker. Other adapters default to no store. Managed cache files have
strict key names, bounded reads, atomic replacement and recency-based eviction:
4 MiB per file, 4,096 files and 256 MiB total. Cache availability is separate from
successful live asset loading.

`AppearanceSnapshot.write` captures a point in a logical appearance timeline,
including explicit null state, wearer UUID/local-player identity, source session,
timestamp, skin/cape hashes, arm model, cosmetic fits, loaded catalog definitions,
cape playback rate, reduce-motion setting and emote phase/hold state. A loaded
mesh or the actual loaded cuboid fallback determines the required file set.
Cache-only look/item/emote access does not initiate network work while capturing.
Required assets must already be available; incomplete captures fail rather than
substituting a fresh catalog or a different model.

The archive writer preflights each required file, records size and SHA-256,
rechecks content while streaming and writes a new `.arctic-looks.zip` using
sorted asset names, fixed entry times and stored payloads. It has a 1 MiB
manifest limit and the same 256 MiB asset budget. Existing output is refused;
failure removes only the new output created by that call. Account tokens,
authentication envelopes, source paths and creation wall time are not included.
`NativeBridge.saveAppearance` supplies the current validated world/session and
its selected cosmetic clock on the serialized client thread.

The automatic recording association, archive reader/playback mount and native
control/launcher route for this API are still required. These files are not
evidence of a verified native draw or a completed appearance-history recording.
All work in this continuation was static source inspection and editing: no
builds, tests, helper programs or Bedrock processes were run. Broad parity and
deterministic rendered-video verification flags remain false.

### Static replay editor export worker integration (2026-10-26)

The replay editor now offers Export MP4 (preview) through the composed core
job, with independent FPS/frame-count controls and duration from the playhead.
The action requires an initialized, caught-up single viewer and a valid path.
Its immutable request snapshot owns a UUID, edition/path/frame parameters and
shared cancellation flag. Source UI text distinguishes fixed-frame export,
real-time recording and the remaining unverified replay accuracy/audio support.

The worker owns the already-running OfflineConnector through file selection
and rendering, returning it on success, dialog cancellation, ordinary failure
or a caught unwind. Progress overwrites a per-job shared slot and requests a
repaint, avoiding per-frame UI event accumulation while minimized. Final events
carry the UUID. The UI releases stale returned connections on a worker instead of
blocking the event thread or replacing a newer session. A returned connection's
readiness is refreshed from actual helper events before enabling another export.

During export the offline panel retains the local address, progress and Cancel
export/Stop local session controls. Stop marks cancellation and defers helper
shutdown until export cleanup returns its owner. New helper startup and native
load/record actions are gated while the export owns the connection. Launcher
exit and export-state destruction signal cooperative cancellation. The worker
does not launch the game or a helper; all paths still require the existing
connected client and loaded bundle.

No UI, file dialog, build, test, helper, export, injection or Bedrock launch was
run. These changes are source-only; native dispatch/application/terrain, world
lifecycle, repeatable moving video and full cosmetics/feature parity remain
unverified or unfinished, and no broad readiness flag was promoted.

### Static owned indexed replay export job (2026-10-26)

Native marker requests now declare completion `client_tick` (default) or
`dispatch`. A dispatch-only marker retires its pending gate after the observed
primary-thread return and retains a receipt only for that same held tick. It
explicitly reports postClientTickCompleted=false; final tick waiting rejects
that mode. Capture packet fences carry a declared stage: dispatch-only fences
require exact held-tick equality, while post-tick fences retain their completed
later-tick checks. No asynchronous chunk/application/terrain flag is inferred.

`NativeMarkerFeed` now sends dispatch-only markers before every output frame,
including zero/fractional frames, without adding simulation ticks. Those real
receipts are required by the protocol-camera capture path and retained with
actual frame metadata. Pre/post-tick markers remain separate for scheduled
simulation ticks. Reports expose current command sequences and process-wide
usedMarkers; the job checks remaining sequence/marker capacity at the observed
tick rate before rendering its requested range.

The new `bedrock::export_job` source API owns the complete indexed-video flow
for an existing helper/native connection: metadata/output validation, capture
reservation, replay pause/forward origin alignment, camera path, zero-fraction
dual-timer freeze, ordered packet/marker/tick/frame sampling, camera-view/GPU
fences and encoder completion. It releases its clock once all pixels are queued
before waiting on FFmpeg. Result flags remain nativeWorldVerified=false and
deterministicVideoVerified=false, even if encoding succeeds.

Cleanup checks the original viewer/generation and last owned replay sequence,
cancels only that pending step, clears the protocol camera, stops only the
owned capture on failure and releases the acknowledged clock. It runs after
ordinary errors and caught unwinds; failures retain output paths and cleanup
diagnostics. A completed/error capture accepts an idempotent owned stop. Clock
leases still expire independently if an acknowledgement/owner disappears.

The existing CLI replay session recognizes native_export with typed options
and emits progress/saved/failure events. Its input reader sets cancellation for
cancel export/stop while the worker is inside bounded operations; export errors
do not automatically destroy the helper connection. No Minecraft/helper launch,
world selection, sign-in or license modification is performed by the job.

These are source changes only. No build, test, job, export, helper, injection or
Bedrock launch was executed. The GUI action and native replay lifecycle are
still unfinished; dispatch/thread behaviour, complete packet/terrain state,
moving-frame repeatability, audio and full cosmetic/feature parity remain
unverified. A successful future encode alone must not promote those flags.

### Static protocol-camera presented-view capture (2026-10-26)

The helper now returns the requested float32 wire camera pose with rational
editor steps. `camera.wirePose` normalizes equivalent yaw turns before float32
conversion, and canonicalizes a rounded +180 to -180. Position, rotation and FOV
use the pinned packet schema's float fields. These receipts remain queue data;
they are not native camera application claims. The core feeder retains the
latest acknowledged pose and clears/unexposes it after path changes/failure.

Capture's `CameraFence` accepts either the original scoped command identity or
a distinct presented-view requirement with native session and six camera
values. Controlled commands retain matrix-consumption, restoration, unchanged
player and producer-frame checks. Presented requirements compare the actual
world tag camera within 0.001 per component, using wrapped angular differences,
at the same acknowledged held-clock sample. They do not fabricate command or
restoration receipts. Malformed/ambiguous camera objects are rejected.

`NativeExportClock::capture_protocol_frame` consumes only its latest uncaptured
frame receipt after the feeder queued the editor instruction. It verifies the
frame-index timing and held state, renews the lease, requires matching observed
camera/clock/rate/session and a fresh GPU-tagged presentation, then uses the same
indexed settling/readback/encoder path as scoped capture. No camera component
is patched and no extra tick is requested. Expected controls are retained in
the manifest beside actual presented-world values. Returned broad world/video
verification fields remain false, independent of caller-supplied metadata.

Native application/terrain completeness, installed-game camera conventions and
full deterministic moving exports remain unverified. The complete launcher job
and replay lifecycle are still unfinished. This continuation contains only
source edits and inspection: no build, test, export artifact, helper, injection
or Bedrock launch was executed.

### Static fractional export camera sampling (2026-10-26)

The helper's optional `step.cameraTime` contains a sub-millisecond numerator
and denominator relative to the packet step's integer timeMs. Values must be
finite bounded integers with denominator 1–1000 and remainder in [0,denominator),
and the combined camera time cannot exceed the recording. Rational export
steps cannot move the viewer's camera target backward.

Each viewer retains a separate camera target and the timestamp of its last
queued editor camera instruction. Recorded packet catch-up still uses only
the integer target; path sampling uses the exact fractional target. A rational
step with an active path/fixed camera cannot complete before that instruction
was queued. Its receipt preserves the original timeMs/remainder/denominator and
cameraQueued flag; recorded-camera mode explicitly leaves that flag false.
Legacy commands retain their existing step payload and sampling behavior.

`ReplayQueue` sends the timeline's rational remainder and verifies it in the
correlated result. Correlated state includes viewer-bound packet/camera targets
without changing periodic status. Attaching during a fractional playhead queues
forward to ceil(max(packet_target,camera_target)) and exposes that integer
position as the new export origin, rather than seeking back to a displayed
floored timestamp. Baked .abr camera poses likewise sample origin+index*1000/fps
before writing each integer-timestamp packet, avoiding a second pose-rounding
step while retaining the existing recording format and packet order.

These are camera/transport source changes, not native application or terrain
receipts. No export artifact, build, test, helper, injection or game launch was
executed. Full camera/video/native-world/cosmetic parity remains unproven.

### Static planned-tick replay marker coordination (2026-10-26)

The host separates marker admission, primary-thread dispatch return and later
completed-client-tick waiting. `packet::wait_dispatch` shares the bounded,
fresh-file/session/request/replay identity checks with the existing final wait,
but does not require or advance a tick. The CLI exposes it as
`native-packet --wait-dispatch`, mutually exclusive with `--wait`.

`NativeExportClock` now calls `FeedPhase::AfterTick` after each exact native
step acknowledgement. `ReplayQueue::queue_marked` requires an already-admitted
arm with the current replay generation, next step sequence and exact pre-tick
timestamp. It validates the queued marker and original viewer in the correlated
helper reply. `confirm` checks viewer/sequence/position after a tick without
queuing another copy of the data; queueing an AfterTick point is rejected.

`export_feed::NativeMarkerFeed` supplies the complete callback sequence to the
clock controller. It waits for dispatch at target_tick-1 before returning from
BeforeTick. After the clock's real step, it requires the same native observation
and receipt at exactly target_tick, and reconfirms the original helper viewer.
An early completion, extra tick, changed observation, reset, reconnect or wait
failure disables this feeder. It requests no simulation step of its own.

BeforeFrame remains a queue-only phase. Its newly queued fractional-frame
packets are not covered by the preceding tick marker's receipt. Full packet
application and asynchronous terrain barriers are still required, as is proof
that the installed client's frozen main loop can satisfy the dispatcher gate.
No such runtime behavior is claimed. All these changes are source-only; no
tests, builds, helpers, live injection or Bedrock launch were performed, and
native-world/deterministic-video/full-parity flags remain false.

### Static correlated replay export feeding (2026-10-26)

Optional helper `requestId` values produce a dedicated `replay_control` event
after the operation resolves. Its `result` and newly sampled `state` are kept
together, including the current `lastStepSequence`. The helper does not change
legacy step/result payloads or add that sequence field to periodic status.
Asynchronous command errors return the same request identity.

`OfflineConnector::request_control` sends on the existing owned playback pipe
and waits on its non-coalesced event channel for that exact ID/operation. It
bounds the wait to at most 30 seconds, detects helper/output termination and
preserves unrelated lifecycle events in a bounded deferred queue. Legacy step
duplicates and replies from earlier timed-out requests cannot complete a new
command. Non-playback sessions reject this API before writing the pipe.

`bedrock::export_feed::ReplayQueue` pauses and binds to one initialized, caught-up
viewer. Its identity includes replay generation, connection UUID and player
UUID, not just the offline name. It takes the next actual helper step sequence
from the correlated snapshot and queues each `export_clock::FeedPoint` without
rounding its packet timestamp forward. Replies must match generation, sequence,
position, paused/caught-up state and that same viewer; the rational clock and
tick positions must not move backward. Errors disable the feeder. Camera-path
configuration uses the existing public camera packet route; no native camera
quaternion is guessed from editor yaw/pitch.

The queue API deliberately returns transport receipts. The native packet
application/terrain barriers, replay world lifecycle and launcher export action
remain unfinished. No game, helper, build or test was executed for this static
source continuation, and no broad parity/video verification flag was enabled.

### Static frame-indexed native export clock (2026-10-26)

The host's `bedrock::export_clock` module attaches to an acknowledged explicit
freeze of both native timers at zero interpolation. `Timeline` derives each
frame from its integer index, preserving sub-millisecond rational timestamps
and using quotient/remainder for native ticks and alpha. It never accumulates
rounded frame durations. FPS and observed integral TPS are independent: no
tick is added merely because another output frame or packet marker needs one.

`advance_frame` calls a supplied replay feeder before every native tick and
again at the output timestamp, then waits for each actual single-tick receipt.
Intermediate ticks are retained at low output FPS. Native session, exact held
tick, original freeze sequence, both timer rates and alpha must match each
acknowledgement. A partially failed frame cannot be retried on the same object.

`capture_frame` additionally applies the supplied native camera quaternion,
waits for component restoration/player ownership, renews the clock lease and
requires a later presented layer containing those actual clock/camera request
identities at the scheduled tick, alpha and rate. The scoped indexed encoder
then performs its existing settling/readback/manifest path for that index.
The adapter copies actual TPS into capture controls and rejects a rate that no
longer describes the completed native tick or disagrees with the real timer.
Capture fences may require that rate explicitly.

The caller must own the already-running replay connection, feeders, native
camera targets, indexed capture lifecycle and cleanup. This API does not start
Bedrock or a helper. The launcher export action is not wired to it yet. A
packet feeder's queue receipt does not prove application or terrain readiness,
and no deterministic moving-frame/video flags are promoted. All changes here
are source-only: no build, test, helper execution or game launch was performed.

### Static replay packet-marker correlation (2026-10-26)

The helper accepts an optional canonical `37c1`-prefixed 64-bit marker on an
exact paused step. It requires one initialized viewer and a fresh marker,
retained across resets. Once world packets and camera instructions reach that
step's queue position, it queues `network_stack_latency` with `needs_response`
true in the same connection queue. The marker uses BigInt serialization;
recorded connection timing probes remain filtered. `replay_step` gains marker
fields only when one was requested, and still asserts queue progress only.
The pending marked step retains its original viewer object; a replacement
connection cannot complete it even if that connection is also initialized.

`arctic_bedrock_packet_arm_v1` and the existing PID-scoped control worker arm a
bounded native marker. Requests carry native session, increasing command
sequence, unique request ID, marker, replay generation/sequence/time and timeout.
Markers are not reused across native world transitions; the process-wide set is
bounded at one million. Arming clears the preceding capture receipt. Only a
later observed matching request probe on the bound primary client thread can
reach `dispatch_returned`. A subsequent completed primary level tick in the
same native context reaches `client_tick_completed`. An observed session change
or timeout fails the pending request. Temporary context-copy contention does
not erase a pending marker, and the host requires a currently live context
before accepting a receipt. Neither command admission nor a transport echo can
produce this completion; no native tick is advanced automatically.

The Rust `bedrock::packet` API and `bedrock native-packet` CLI publish requests
exclusively/atomically, correlate reports and bound waits. Arm and wait are
separate operations so the host can queue the returned replay command and
permit its required client tick. `PacketArm::replay_command` preserves the arm's
generation, replay sequence, timestamp and marker exactly. The native worker
refreshes unchanged reports at least every 500 ms while it is serviced. Host
waits read freshness metadata from the same opened report and ignore files
older than two seconds; a cached live-context flag alone cannot qualify.

Completed receipts can be copied into native world capture controls only at or
after their post-marker client tick and while that native context remains live.
`ControlFence::with_packet` adds an optional `packetDispatch` requirement to
indexed samples; native readback/queueing and host observation match its
identities, explicit native session, replay metadata, dispatcher checks and tick boundary. Those actual
receipts remain in frame manifests. This does not establish prior asynchronous
packet application, chunk completion, terrain readiness or deterministic moving
video; all broad verification flags remain false. This is source-only, without
builds, tests, helper execution, live packet injection or Bedrock launches.

### Static native packet-dispatch observer (2026-10-26)

Previously copied exact-image sections resolve the unique packet factory at
`0x16ad560`. Its case for packet 115 (`network_stack_latency`) calls the helper
at `0x16bac90`. Constructor stores bind packet vtable `0xe7b0fd0`, dispatcher
singleton `0x1185b5b0`, timestamp at Packet+`0x30` and response byte at +`0x38`.
The packet ID accessor at `0x3c40700` returns 115. The dispatch singleton's vtable
`0xe7bfea0`, slot 1, selects `0x1733ce0`, which forwards callback/network ID and
the shared packet to callback virtual offset `0x2b0`. The pinned public SDK
independently supplies the four-argument borrowed-shared-pointer dispatcher ABI.
`inspection/native-packet.json` retains these static facts without game bytes.

`native_packet` installs a pass-through observer only under the existing exact
image hash and matching code/vtable guards. Before forwarding it checks the
dispatcher singleton, packet type/vtable, native shared-pointer header and live
reference counts; only the timestamp and response byte are copied. After the
original returns, no packet/shared-pointer memory is dereferenced. The callback
arguments are forwarded once, unchanged; Rust does not construct a packet,
invoke a discovery virtual, increment references, or alter network behavior.

Completed primary-client level ticks establish their actual thread/session.
An observation reports a primary-thread return only when the same level/epoch
survives the callback and the thread matches. Engine reports expose eight recent
observations and separate entered/forwarded/observed/rejected counters. These
are dispatch-return diagnostics, not a replay barrier: earlier packet work can
be asynchronous, and terrain completion has not been established. Replay steps
retain `packetsQueued` with `nativeWorldVerified=false`. Native packet
application, terrain and deterministic-video readiness flags remain false.

This source port used existing copied sections. It performed no build, tests,
helper execution, game launch, packet injection or live hook validation.

### Static local movement/emote cancellation port (2026-10-26)

The Bedrock platform previously inherited `localMoving() = false`, bypassing
the shared client's walk/jump/sneak emote cancellation. `native_input` now
copies game-processed local input through the checked actor/EnTT context. It
requires the actual LocalPlayer tag and supported exact image; it rechecks the
entity generation, component location and payload before accepting the copy.
Movement axes must be finite and bounded, and copied native booleans must be
valid. Unused bitfield padding is not interpreted as additional flags.

`inspection/native-input.json` records the static layout evidence. Previously
saved package-inspection sections match the existing 1.26.5203.0 profile/hash.
The native accessors index MoveInputComponent (`0x018b1887`) with stride `0x64`,
read jump bit 4 at `+0x60`, and inspect direction bits 13–16 plus movement axes
at `+0x24/+0x28`. These accesses corroborate the pinned public SDK declarations.
The current installed file still denies direct reads; this work used existing
copied sections and started no game or package helper.

Actor/world snapshots retain the copied movement state. The Java platform uses
the local player's directional, analog, jump and sneak values for the existing
shared emote stop behavior. Velocity, knockback and fixed physical key bindings
are not input substitutes. A retained world during a missing native depth/frame
has no current input availability, preventing cancellation on stale input.
This is source-only: no builds, tests or game launches were performed. Live
keyboard/controller/remap behavior, complete emote/cosmetics parity, native
packet/terrain hooks and deterministic replay export remain unverified.

### Static native capture/control correlation (2026-10-26)

The held-clock observer copies an operation identity only after its world tick
and requested timer work have completed. It requires current ownership, a live
lease, the exact held client tick/fraction, matching native timer readback and
no queued/pending operation. World collection retains that identity only if it
matches before and after the native world callback. Real-timer control is
reported from the actual bound timer, independently of the client fraction.

The camera controller records a compact receipt only after the native matrix
consumer ran, the component bytes were restored and the player pose was
unchanged. Its producer-frame key must remain stable across that consumer.
Eight receipts are retained for deferred rendering; conflicting commands in
one producer frame mark that frame ambiguous. The world-to-producer mapping
must also be stable, unique and current. A global last-command receipt cannot
stand in for this association.

`live_frame` carries the copied clock and resolved producer-camera identities
through `CaptureFrame` into the presented world tag. An optional scoped sample
`controls` requirement matches both operation sequences/request IDs, the held
tick/fraction, completed camera checks and optionally the real timer (default
required). Native matching occurs both before readback and before queueing the
pixels. Actual receipts reach the indexed manifest. Existing unfenced indexed
samples retain their previous matching policy.

The Rust `ControlFence::from_receipts` and `IndexedSample::from_observation`
helpers join acknowledged controls to a later, fresh presented world sequence.
They do not advance clocks, renew leases, apply replay packets or establish
terrain readiness. Native packet/terrain hooks, Java replay-backend integration
and deterministic moving-video verification remain pending. This is source-only:
no builds, tests, helper execution or Bedrock launches were performed.

### Static owned indexed-capture API (2026-10-26)

The existing indexed native capture sink now accepts an optional paired
`captureId`/`requestId` envelope. Start binds ownership; later sample/stop commands
must match it, and a scoped capture rejects legacy unscoped sample/stop input.
Legacy unscoped captures retain their command format. Reports distinguish the
latest admitted command from each frame's capture/request identity. Those IDs
also reach the encoder's RGBA-hashed frame manifest. Encoder completion remains
separate from admission or queueing a presented frame.

An active indexed sink observes fresh presented world/GPU tags even when no
readback is pending. Reports include their age; observation does not encode a
frame. The new Rust `bedrock::capture::control` API and CLI `bedrock native-capture`
provide typed start/observe/sample/stop/finish operations, bounded JSON and
report reads, exclusive atomic request publication, unique request correlation,
bounded waits and withdrawal of the caller's unconsumed request on failure.
Finish requires successful native encoder completion, the exact frame count
and nonempty MP4/manifest files. No game launch or account action is included.

Sample matching still uses the actual native tick, float fraction, clock time,
camera, minimum world sequence and clean-view state, with fresh world layers,
pixel settling and DX12 buffer coverage where requested. Observation does not
prove packet application or terrain readiness, and the API does not coordinate
clock leases, camera ownership, world reset or replay stepping. Deterministic
moving-video verification and Java replay-backend registration remain pending.
There were no builds, tests, helper execution or game launches for this port.

### Static selected-skin pose port (2026-10-26)

`animation_pose::standard_parts` reads the hierarchy index actually selected by
the bound native skin draw. Six named bones must have the standard rest
translations, pivots, zero default rotations and unit default scale. Completed
matrices must be finite, affine, rigid and preserve handedness. Their current
translations and rotations are copied directly; parent defaults are not
traversed and a standing torso/head-look constraint is not imposed on this path.

`native_body` records this owned `SkinPose` only for the standard geometry aliases
under the existing data-driven-model, actor, context and non-UI draw checks.
`native_transform` binds it to that world's actor scope and animation frame,
rejects conflicting selected poses and requires it for body/cape receipts.
`live_frame` accepts its matrices only with the matching native root/camera,
animation frame and original appearance. Posture-related AABB changes cannot
rescale this native-root path. The older standing observation remains the
restricted fallback when the selected skin pose is unavailable.

Standard-rig cosmetics can therefore consume current native posture/attack
matrices in source, including when another copied hierarchy would make a
name-only search ambiguous. Replacement retains its experimental isolated gate;
cape ownership/physics/chest-slot checks still apply. Fall-flying state is also
forwarded to the Java cape renderer. No tests, builds, helper execution or game
launches were performed. Broader posture, slim, armor/held-item, persona and
elytra behavior remain unverified; all-pose/parity flags remain false.

### Static replay initialization boundary (2026-10-26)

The helper's `join` event is an encryption-handshake boundary. Replay state now
reports the separate local-player initialization packet with viewer UUID,
connection ID and generation. The runtime entity must match the recorded
StartGame entity; a mismatch closes the viewer and cancels a pending step.

NetherNet replay defaults to initialization-gated playback. It requires the
recording's original client initialization checkpoint, queues bootstrap packets
in bounded batches through that checkpoint and then waits for the new viewer's
acknowledgement. Paused replay still feeds that bootstrap. Wall time spent
waiting does not become simulation catch-up. Packet-ready and replay-finished
states cannot pass this boundary early; missing checkpoints, mismatched entity
IDs and initialization timeouts fail explicitly. Reset and exact-step lower
bounds use the recorded initialization timestamp. Legacy RakNet playback keeps
its existing policy unless the helper caller opts into `waitForClientInit`.

The launcher exposes initialization separately from queued packet readiness,
and its replay MP4 button requires both. The pinned protocol client can emit
this acknowledgement merely on receiving the spawn status; it cannot establish
that the native client's terrain, main-thread packet application or GPU work has
completed. Those native fences remain pending. This is a source-only port;
there were no tests, builds, helper execution or Bedrock launches.

### Static native-cape replacement path (2026-10-26)

This port contains source edits and static inspection only. The installed game
directory and resource archive headers were read without executing the game or
an analysis helper. No tests, builds or Bedrock launches were performed. Those
archive headers do not establish the native cape draw hierarchy or invocation.

The readiness payload now has a separate bounded `capes` list alongside skin
plans. Each entry identifies an actor, an Arctic cape SHA-1, its loaded static or
animated frame key, and the original native cape SHA-256 key. The source planner
requires a complete supported rig, a loaded texture, empty chest slot and native
cape motion before proposing a replacement.

Native draw ownership distinguishes a six-part skin hierarchy from a hierarchy
containing only `cape` and an optional `root`. It does not assume that a logical
geometry alias identifies the cape. The selected geometry must belong to the
actual actor animation component; the root animation frame, current appearance,
standing-pose checks, scoped world transform, copied mesh/count and native color
checks still apply. An unrecognized hierarchy follows the native path.

Skin and cape submissions now carry distinct typed replacements. Only a matching
cape skinned-description can use the existing borrowed-palette collapse path,
and it records a separate `Global.cape` receipt after ownership is accepted.
That receipt includes the cape physics sampled from the primary client's actual
completed tick; neither draw nor presentation advances the simulation. Repeated
mesh receipts must agree on the accepted hash, native key, frame and physics.

The world producer carries `nativeCape`, `nativeCapeSuppressed`, `capeHash` and
the accepted frame texture only when the scoped receipt still matches the actor
and animation frame. The Java boundary checks the complete pose, native cape
presence, chest slot and physics. Rendering uses the accepted hash's loaded
frames at the world clock instead of switching to a newly edited look before a
native draw accepts it. Existing proven-absence cape attachment remains separate.

This is an unverified experimental route, not installed-game evidence. In
particular the real cape hierarchy and use of this skinned-description path are
not yet established. The existing isolated replacement gate is retained; native
cape, all-pose and deterministic-video verification flags remain false.

### Static native clock API and completed-tick fence (2026-10-26)

This continuation consists only of source edits and static review. No builds,
tests, analysis helpers or Bedrock launches were performed.

The clock controller now has a bounded one-request queue, an in-process
`arctic_bedrock_clock_v1` entry point and PID-scoped file requests serviced by the
existing control worker. The Rust API and `bedrock native-clock` CLI publish
atomically and correlate results by session, sequence and unique request ID.
Regular commands require expiring 500–30000 ms leases; callers can renew a held
sample without changing its fraction or timer mode. Requests expire before an
unresponsive timer consumer can apply them. A missing completion, stale native
owner or expired lease releases both clock controls without waiting for another
request. Legacy probe files remain restricted to the isolated probe mode.

`step` now records the next expected primary-client level tick. Its response
waits for the real level-tick detour to return, update the primary client clock
and finish cape simulation. If controlling a distinct real timer it must finish
that step too. Timer arithmetic and packet-queue progress are insufficient. The
held expected tick remains checked after completion so an unrequested extra
primary-world tick releases control. Session changes clear sequence, freeze
generation, counters, pending replies and leases. Responses retain calculated
timer counts separately from `completedTick`/`worldTickComplete` and `heldTick`.

The exact-image renderer clock, camera delta and fog hooks can now service the
regular clock-control route, still requiring an owned frozen world context.
GPU shader/presentation diagnostics keep their isolated-probe gate. This port
does not establish renderer reset repeatability or deterministic video, and does
not implement integrated-server stepping, packet-application/terrain fences or
the Java replay backend. All broad verification/readiness flags remain false.

## Native client timer stepping probe

The exact installed image has `Timer::advanceTime(float)` at RVA `0x47323b0`,
called for MinecraftGame's timer pair at `0x7d9f8d`/`0x7d9f9c`. Its native
stepping field is +0x3c: the branch at `0x4732440` emits no tick and copies a
fraction below 1 into alpha/passedTime, while +1 or greater emits one tick and
subtracts 1. The read constants at RVAs `0xe5a7060` and `0xe5e9578` are 1 and -1.
The API/layout declarations are cross-checked with the linked Timer/Minecraft
headers in THIRD_PARTY.md; the branch behavior comes from the installed image.

`native_clock` forwards every ordinary call and checks a 16-byte prologue after
the executable hash check. Its opt-in probe binds only the live primary client's
actual simulation timer and level/session, using the existing checked client
back-references. Normal wall-clock bookkeeping runs first; native stepping then
overwrites its unconsumed calculated outputs using the engine's own stepping
branches. The original stepping field is restored before returning. A native
step already owned by another engine component is rejected. Session changes or
an expired world binding release probe control.

`test-world.ps1 -ClockProbe` includes body/emote/movement checks and then tests
freeze, fractional samples, two separate one-tick requests and resume. Commands
are bounded, sequenced and session-bound; only explicit isolated body probes
enable them. A timer calculation acknowledgement does not attest completed
simulation: the script separately checks completed primary-client ticks,
elapsed time and interpolation copied into the actual world frame. This is not
integrated-server stepping, native replay playback or deterministic video export.
Those verification flags remain false pending their separate implementation.

## Body and emote verification follow-ups

Run `c0cfe1e3781649e99fe907080c41e2fd` again showed the replacement skin and
an empty native body in the suppression-only GPU image, but failed the strict
depth comparison: both owned depth files were byte-identical, with zero farther
or nearer samples, while 173,091 of 185,956 GPU-mask pixels changed. The native
world sequences/scopes differed (372 and 384), and the original shader alpha
was still partial. This run does not establish a repeatable depth-absence
comparison and stopped before the new emote test. PID 14992 was confirmed
stopped. The positive standing-pose result above remains bounded to its run.

The automation now settles three matching native snapshots after each body mode
switch. Independent remaining body-absence, emote and movement diagnostics are
collected even if an earlier comparison fails. Failures remain explicit in
`failedChecks` and still fail the overall run; this avoids spending another full
startup merely to obtain the later diagnostic evidence. Nine Python checks
cover report/input ownership and continuing diagnostics after a failed check.
The opt-in EmoteProbe serves a catalogued two-second arm animation through the
normal asset protocol, invokes `Looks.playEmote` on the render thread and
captures two actual clock-driven poses plus their matching GPU markers. Its
control accepts only play/stop for the fixed diagnostic emote in an explicit
isolated body probe; no camera, poses, clock or depth are supplied by that file.

Run `de25f77e1b70466998a8f096e4e2b551` passed all automatic checks, including
the settled body comparison and the new emote check, with an empty failedChecks
list. Under a 47,451-pixel body mask, 45,594 depth samples moved farther, zero
moved nearer, and 172,985 of 189,464 GPU-mask pixels changed. This second positive
standing-pose comparison does not erase the intervening inconclusive depth run.

The ordinary local emote advanced from 0.279 s / arm -33.480 degrees to 10.735 s
/ arm -88.200 degrees on the actual native clock. The two exact marked GPU
frames matched 235,275 and 322,388 blue skin pixels respectively, and 40,520
body-mask pixels changed. Both images were inspected: the arms visibly move,
with the diagnostic skin, hat and cape present. Stop acknowledgement was followed
by a native world capture with no playing emotes. The script then moved the
native player 1.02485 blocks, advanced its clock 2,355 ms, observed cape delta
0.03007 and matched 51,330 cape GPU pixels. The sustained depth check delivered
65 frames without exhaustion. All thirteen acknowledged inputs/controls were
automatic, and PID 13988 was confirmed stopped after 271.7 seconds. The runner
now checks the native arm angle before requesting another expensive GPU capture.

The current suites pass 65 native, 32 adapter Java and nine Python tests, with
release Clippy and the hidden renderer/JVM smoke also passing. Live emote
animation is demonstrated for this diagnostic rig; other body poses, equipment,
native lighting/material fidelity, complete cape physics parity, native replay
playback/camera and deterministic video export remain unfinished. Verification
flags for all-pose body replacement, cosmetics parity and deterministic video
remain false.

## Native clock live verification

Run `1a677d0a657b48578741828dcd06af13` passed all automatic checks including
the first native client-clock probe. Completed client tick 488 and interpolation
0.25 held the actual world clock at 24,412 ms across the frozen captures. One
native step produced tick 489 / 24,462 ms. Sampling 0.75 retained tick 489 and
produced 24,487 ms; the next step produced tick 490 / 24,537 ms. Resume reached
tick 504 / 25,210 ms. The timer's original -1 stepping field was restored after
each control. These are completed world-tick observations, beyond the separate
timer calculation acknowledgements. The run completed in 249.7 seconds with
no failed checks, and PID 26396 was confirmed stopped.

An additional comparison found that frozen body pose, root, camera, projection
and cape texture were identical, but the Arctic world layer changed 22,751
pixels. Cape motion had become null: its previous 500 ms wall-clock tick expiry
discarded valid paused physics. Cape entries now carry the exact completed
primary-client tick, checked against the camera's tick instead of elapsed wall
time. Live actor observation freshness and session/UUID checks remain; changing
the native actor address invalidates the old simulation. A regression verifies
paused-tick retention and rejection of newer ticks, other sessions and absent
actors. The ClockProbe now requires byte-identical decoded RGBA world layers
at the same frozen sample. Its repeat live result follows below.

The rebuild's hidden renderer smoke hit an intermittent duplicate-loader check
before any game launch. Its diagnostic now retains the actual loader status and
stderr. Module enumeration retries only Windows `ERROR_BAD_LENGTH`, bounded to
two seconds, following the documented
[CreateToolhelp32Snapshot behavior](https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-createtoolhelp32snapshot).
The retry is conditional; the old diagnostic did not establish that this was
the specific cause of the failed attempt. The subsequent hidden smoke passed
duplicate rejection, ABI/skin/depth, cape, disconnect and JVM/menu checks.

Run `7d7eb353008f4b158a7186e66c44f1c7` passed the stronger ClockProbe and all
preceding body/emote/movement checks with no failed checks. The actual client
held tick 489 / partial 0.25 / 24,462 ms across both frozen captures. One tick
advanced to 490 / 24,512 ms; fractional sampling produced 24,537 ms without a
tick, and the next tick produced 491 / 24,587 ms. Normal time resumed. Both
decoded RGBA cosmetic layers had SHA-256
`a92b70bea9411b48797c57767e8ccf12444aad0c14cd54d6389bdc223c984072`.
Their cape motion values were identical and remained present. The held native
GPU image was inspected and retained the blue skin, diagnostic hat and cape.
PID 26584 was confirmed stopped after 276.7 seconds; its profile kept audio
muted and the empty user provider/unchanged license guards remained active.

The complete native frame is still not deterministic. Comparing the saved
frozen/held native GPU images inside a crop excluding the HUD edges found
171,853 changed pixels (RGB absolute-difference sum >12) out of 1,293,240.
The camera and cosmetic layer were stable; world rendering continued changing.
`frozen-native-frame-evidence.json` retains this negative observation. Future
ClockProbe reports also retain cropped native-frame hashes separately from the
cosmetic-layer hashes. The next native renderer/simulation clock work must
resolve these changes, and replay camera/reset/playback still need bindings.
Client timer stepping and frozen cosmetics do not establish deterministic MP4
export or full parity. The suites now pass 69 native, 32 adapter Java and ten
Python checks, plus release Clippy and hidden renderer/JVM smoke.

## Native renderer clock binding

The installed `GameRenderer::renderCurrentFrame(float)` spans RVA `0x47a8ca0`
through `0x47ac05c`. At `0x47a8e99` it obtains wall time directly, then updates
the renderer clock at +0x1f0 (accumulated time), +0x1f4 (delta), +0x1f8 (delta
squared) and +0x1fc (unscaled time). Scale/pause are +0x200/+0x204 and the
wall-time bookkeeping field is +0x218. This clock advances independently of the
native client timer and must be sampled separately for replay rendering.
The offsets are corroborated by these installed instructions, not named fields
in the public untyped Clock declaration.

`native_render_clock` hooks `makeScreenContext` at `0x47adaf0`, preserving its
this/output/float arguments and native return. Only the actual direct native
return site `0x47a8f24` in renderCurrentFrame can authorize a change; its other
UI/item call sites pass through. The renderer must match the primary client's
fresh level/timer/session binding. The current completed client ticks and native
fraction provide the sample; four renderer-clock values are updated before the
native frame context is built. Scale, pause and the wall-time bookkeeping field
are preserved. Failed writes/readback restore the original values. Ordinary
launches and non-frozen probe frames retain native behavior.

The world timing record now includes the renderer clock read from that same
world pass. `-RenderClockProbe` includes all preceding checks and requires the
frozen native image crop as well as the cosmetic layer to match exactly; its
clock samples must match the requested native tick/fraction. A failed full-frame
comparison remains a failed test even if the clocks and cosmetics are stable.
No native replay reset, camera or deterministic-video readiness is asserted by
the new hook. The live results below distinguish clock ownership from pixel stability.

Run `59838726288a4b45a73b8c6aec923094` established the new hook's live ownership:
49 world-context calls applied 49 clock overrides, while other contexts passed
through. At frozen client tick 487 / fraction 0.25, both world snapshots read
renderer clock `[24.362499,0,0,24.362499]`. The cosmetic layers were identical.
The strict native-image comparison nevertheless failed: 174,631 of 1,265,664
cropped pixels changed by RGB absolute-difference sum >12, in the water region.
The renderer-clock binding is demonstrated; it did not resolve the remaining
texture/world animation by itself. All prior body/emote/movement/client-clock
checks passed. PID 29068 was confirmed stopped with the overall test failed.

The next RenderClockProbe coordinates the primary client's simulation and real
timers. The real timer is obtained from the same verified Minecraft instance
at +0xe8 and validated independently; pointer changes invalidate the binding.
Only matching native tick rates permit coordinated steps. A command requesting
both timers is acknowledged only after both actual timer callbacks apply their
outputs. World timing snapshots also copy the real timer, and the probe requires
its sampled fraction to match. Each timer still performs normal wall bookkeeping
before controlled stepping and restores the original stepping field afterward.
This extends explicit isolated control; integrated-server timing, replay reset,
camera and deterministic-video readiness remain unverified.

Run `366e78321f47465cb9d8b5c2c7aa36c6` passed the strict RenderClockProbe and
all preceding checks. The primary simulation and real timer each received two
individual native steps; 667 callbacks matched each timer. Commands were
acknowledged only after both timers applied their outputs. At frozen tick 531,
both actual world snapshots retained fraction 0.25, 26,562 ms and renderer clock
`[26.5625,0,0,26.5625]`; the real timer also retained alpha/passedTime 0.25 and
zero ticks/timestep. Steps reached ticks 532 and 533, fractional sampling added
25 ms without a tick, and both timers resumed with their stepping fields at -1.

The frozen cosmetic layer hashes matched. The native GPU crops also matched
exactly, with SHA-256
`b09688bd1483f8023161ea37abf957f584deed7d0d1ac4d647a7c7171e61c662`.
An additional comparison of the whole native images found only 224 changed RGB
pixels inside `[91,19,105,37]`, the FPS-counter digits. All other pixels,
including the water region that changed previously, were identical. The held
native image was inspected and retained the custom skin, hat and cape. This
demonstrates controlled sampling of this installed scene; it does not establish
repeatability after native replay resets or across recordings.

The run completed in 244.0 seconds, delivered 67 depth frames in its sustained
observation without exhaustion, and retained an empty failedChecks list. PID
4840 was confirmed stopped; the test profile's four audio values remained 0,
the empty user provider was active and the license state was unchanged. Current
native/Java/Python checks pass 71/32/10, with release Clippy and hidden renderer
smoke passing. Native simulation/replay/video readiness flags remain false.

Camera research is retained in `inspection/camera-candidates.json`. The installed
image has one matching `_updatePlayer` candidate at `0x6733360`; its third
argument supplies registry/entity values at +0x10/+0x18. A getter candidate at
`0x5dba620`, twelve vtable slots after the known local-player getter, chooses a
weak reference at client +0x218 or falls back to +0x248. These are static
candidates requiring live ownership/layout/ABI validation. No camera control is
enabled by the candidate file; camera position/rotation/FOV, replay reset and
deterministic video export remain unfinished alongside the broader parity work.

`native_camera` now installs a passive updater detour at `0x6733360`. It forwards
the native camera/component/player arguments exactly once and never changes
camera or player fields. Samples require the freshly checked primary client,
level, player entity generation and session. Independently, the checked camera
getter slot selects its live weak entity and looks up the camera component by
the installed type hash and `0x120` stride. Reports distinguish matching the
player target from matching that live camera entity's component. Quaternion,
position, aspect/FOV and clipping values are copied and bounded; stale samples
expire. The native forwarding and malformed-memory tests pass, without asserting
that a fixture proves ownership in Minecraft.

Run `native/test-world.ps1 -CameraProbe` for automatic private-desktop offline
entry, all preceding cosmetic/clock diagnostics, and back/front/first-person
camera observations. It sends keys only through the owned inspector, captures
the exact marked GPU frame, compares copied camera values with the rendered
view, and restores the initial view before clock checks. Camera observations
from another player/session fail the probe. Failure evidence survives view
restoration errors. This is an observation probe; native camera control and
deterministic replay export remain disabled pending live implementation.

Run `c13bbdb8bb78425286a14a9c3a9f3825` passed the original observation probe
and every preceding diagnostic in 276 seconds, using 21 scripted inputs and no
manual navigation. Native startup/menu entry/world loading occupied 245.8 seconds;
the camera view check itself took 1.4 seconds. There were 704 valid primary-player
updater observations. In back, front and first-person views, the copied camera
position matched the rendered position exactly; FOV was `1.1519173` radians
(66 degrees), near plane `0.025`, far plane `2500`. The fallback weak entity did
not own the component, so this establishes target-player association and matching
rendered values, without camera-entity membership or control. Both frozen cosmetic
layers and native GPU crops were identical. The owned PID 27556 was stopped,
all four profile audio settings remained zero, the empty user provider and
loopback-only guards were active, and license state was unchanged.

The next observer also scopes the actual camera sparse-iterator callback at
`0x676c930`. Its native closure supplies the registry owner and current camera
entity. The observer checks owner generation, packed membership, component type,
stride and exact incoming pointer, then rechecks them inside the updater. The
thread-local scope is restored even when rejected. Its native three-argument
forwarding and wrong-component/expired-generation tests are independent of game
evidence. `-CameraProbe` now requires component membership in all three views,
in addition to primary-player/session ownership. This scope still writes no
camera or actor fields.

Run `360aaba1a63c41d184789c88cb3216b2` established that scope in the installed
game: all 648 accepted updater observations also matched the native registry
component. The first/back/front cameras are entities 1/2/3, distinct from the
primary target player entity 10. Every perspective's component position exactly
matched its rendered view, and all three membership checks passed. Ordinary
fallback weak-entity selection still resolved the target player, as expected
from the earlier negative component lookup.

The overall run nevertheless FAILED its strict frozen-native-image comparison.
The clocks and camera values were identical at client tick 521 / fraction 0.25,
and the cosmetic layers were byte-identical, but 185,349 of 1,265,664 cropped
native pixels differed. The maximum RGB absolute-difference sum was 5; none
exceeded 12. This small variation remains a failure, without relaxing the hash
comparison or claiming deterministic video. Native server/world time and other
render state still need investigation. All other diagnostics passed, both timers
resumed, PID 16608 was stopped, and the isolation/license guards were retained.
The run took 267.8 seconds with 21 scripted inputs. Validation now covers 75
native tests (74 in the full run plus the additional source-membership test),
11 Python tests, release Clippy, and the hidden renderer/JVM smoke.

`native_camera_control` adds opt-in camera control scoped to the native camera
matrix consumer. It requires an isolated body probe, the fresh primary client and
session, and the exact live registry component supplied to the owned updater.
Commands contain a monotonic sequence, current session, absolute position,
normalized quaternion and vertical FOV in degrees. Unknown fields, invalid
values, old sequences and foreign sessions are rejected.

The patch changes only orientation/position/FOV; aspect, near/far clipping,
cached matrices and player components retain their native values. The native
camera consumer runs normally and the original camera bytes are restored afterward,
including on rejected paths. An unexpected player-camera update during the
scope first restores the native values and aborts control. The controller also
compares the player's position/velocity/rotation and body/head yaw before/after
camera emission. These guards do not by themselves establish that camera changes
reach the native world image.

`native/test-world.ps1 -CameraControlProbe` runs the existing observations, then
freezes both timers and tests position/FOV and quaternion changes against the
actual rendered world camera and marked GPU pixels. It releases the camera,
checks the restored view and resumes clocks on both success and failure.
`camera-control-evidence.json` preserves acknowledgements, actual world reports,
GPU comparisons and failures. Native replay camera/video readiness remains false
until installed-game evidence and the replay backend establish the full path.

The first control attempt wrapped `GameRenderer::renderCurrentFrame` at
`0x47a8ca0`. Run `2eae0d1b86ca4a2094a5764c9f43be5b` rejected that approach:
61 owned patches restored successfully and preserved every sampled player
position/rotation value, but the actual rendered position and FOV remained
unchanged. Component write/readback alone is not camera control. All other
diagnostics passed, including the strict frozen native-frame comparison, and
the owned PID 17844 stopped after 276.9 seconds.

The second attempt scoped the actual event dispatcher at `0x25c6a90`, independently
checking its native three-pointer/byte ABI and exact prologue. The native player
camera updater runs first with unmodified inputs. Only after it returns can its
checked primary-player component receive the replay view; it remains scoped
through subsequent native camera systems in that event and restores on event
exit. Nested events retain the outer scope. A repeated player-camera updater
restores native inputs and rejects that control sample. No render matrices are
fabricated or substituted in the overlay to make this test pass. Run
`324bc41c1a44482dbcc19be1424a2a6b` also rejected this scope: all 65 patches restored
and preserved player state, but the rendered view remained unchanged. All other
diagnostics, including the strict native frozen-frame comparison, passed.

Executable analysis then located the actual matrix consumer at `0x46cff30`.
It resolves the camera component by the native type hash/stride, computes view
and projection, and copies component position to its renderer's +0x660 field,
the same field independently read by the existing world producer. Its matrix
helper at `0x9f4eb0` reads camera quaternion/aspect/FOV/clipping and produces the
native saved model-view/projection. The controller now wraps that consumer,
requiring its `this` to match the primary level renderer's player-renderer
pointer at +0x468. The fresh updater component must come from the same native
thread and still match its registry generation.

A separate hook observes the matrix helper. An exact input-pointer match to the
owned component must have occurred before any control is allowed; each controlled
sample requires the same match during the actual native calculation. Neither
matrix helper nor consumer arguments are replaced. Their two/three-pointer
forwarding is tested independently. This path still requires live view/GPU
evidence before claiming camera control.

Run `52c6aed6ea5747dea42e887a4611e59d` reached this consumer but recorded no
owned matrix-input matches, so it applied zero patches and rejected the control
command. All other checks passed, including byte-identical frozen native crops.
The native resolver at `0x9ed0a0` explains the selection: it takes the first
packed `MinecraftCamera::RenderCameraComponent` tag (`0x119e772b`) from the camera
registry, checks its generation and resolves that entity's CameraComponent.
The updater's input-camera membership cannot substitute for this selection.

The binding now follows that exact render-tag lookup read-only, validates the
packed/sparse storage and registry-owner generation, then resolves the render
camera's component. The matrix helper must observe this exact component before
and during control. The last successful binding step is reported explicitly;
none of these checks are relaxed to obtain a passing screenshot. Full native
checks pass 81 tests, Python automation passes 13, and release Clippy passes.

Run `435ade09440c4cbb885ca5899398224e` established the corrected binding at all
seven validation steps and observed 545 exact native matrix-input matches. It
applied no patches: the run failed sustained depth delivery before reaching the
camera commands (five total delivered depth frames, no pool exhaustion). This
negative run is retained. The following run used the identical native build and
a longer 20-second observation window, preserving the ownership/freshness gates.

Run `d093d341e3ec465fadacbf7beda0fc59` PASSED the complete CameraControlProbe in
251.8 seconds, using 27 scripted inputs. The render camera was entity 8, distinct
from the input-camera entities. Its native helper received the exact checked
component 1,960 times. All 146 applied camera patches restored their original
bytes, without changing the sampled player position/velocity/rotation or body/
head yaw. A +0.5/+0.2 position shift and 50-degree FOV reached both the native
world camera and GPU image; 1,122,272 cropped native pixels changed. The second
target reached its requested position and 60-degree FOV, and its world-Y
quaternion rotation changed rendered yaw by 15 degrees, with 1,116,862 changed
pixels. The actual native view/projection and all six camera values were exactly
equal to baseline after release. Baseline/control/restoration images were inspected.

The same run passed all prior body, cape, emote, movement and camera-observation
checks, delivered 132 depth frames during the 20-second observation without
exhaustion, and passed client/real/renderer-clock stepping plus byte-identical
frozen cosmetic and native GPU crops. The native crop SHA-256 was
`96d36ecedd03e8174dfb3e9e700b5c98c187f10ca51211f011414ce3c5cd208a`.
PID 26488 was confirmed stopped; the empty user provider, loopback-only guards,
unchanged license state and four zero audio settings were retained.

This proves scoped native camera movement in the tested scene. It does not
establish replay-backend integration, native replay reset/playback, all camera
rotations/postures or deterministic rendered-video export. Their readiness flags
remain false. `-CameraControlProbe` now also requires exact view/projection
restoration; the saved native reports from this run satisfy that stronger check.

### Static camera editor and native command port (2026-10-26)

No builds, tests, helper processes or Bedrock launches were performed for these
source changes, following the user's static-only instruction. Earlier evidence
above does not validate this new command route.

The Bedrock packet editor now ports Java `CameraPath`'s uniform Catmull–Rom
position/yaw/pitch curve, endpoint control duplication, float yaw unwrapping,
pitch clamp and linear FOV. The launcher defaults new keyframes to this curve
and imports keys from Java extras version 1 `arctic.json`, with the same stable
sort/last-duplicate behavior. Existing linear/smooth/hold files remain readable.
Live playback and baked exports share this sampler.

`native_camera_control` now accepts a bounded single-request queue through
`arctic_bedrock_camera_v1` and a PID-scoped file mailbox serviced on the existing
control worker. Normal requests do not require the body probe. The checked
consumer still owns all component access and restoration. Admission requires
fresh matrix observations and exact world-session/sequence validation; stale
queued requests expire before touching memory. Session changes reset command
state. A failed restoration blocks further writes within that session.

The host's typed `bedrock::client::camera_control` API and `bedrock native-camera`
CLI publish atomically, correlate replies with a distinct request ID, and wait
for restoration/player-pose receipts. They do not launch Minecraft. The legacy
probe command remains restricted to probe mode. Reports distinguish queued,
rejected and completed commands, without promoting parity or deterministic-video
flags. Native world reconstruction, terrain readiness, capture presentation
fences and registration of the Java replay backend are still required.

### Static native-skin emote port (2026-10-26)

This continuation contains only source edits and static inspection. No tests,
builds, helper processes or Bedrock launches were performed.

The native world renderer now registers visible actor UUIDs with the existing
shared remote-emote watcher. Previously only Java's render adapters did so,
leaving the Bedrock watch list empty even while remote players were visible.
The body readiness planner can now choose a loaded native game skin for an
active emote when no Arctic custom skin is ready. Readiness accepts only an
Arctic SHA-1 texture key or an owned native skin SHA-256 key. Before suppression,
an engine key and arm model must still equal the same live actor's observed
appearance; arbitrary engine keys and other actors' skin images cannot qualify.

`live_frame` retains `nativeSkin`/`nativeSlim` independently of the replacement
texture reported by the scoped draw. `WorldFrame` uses the acknowledged skin and
arm model when rendering that replacement, avoiding a race where a new look
download could change the rendered arm model before the native draw accepted it.
Native images in an active body plan are pinned against bounded-cache eviction;
uploads are declined when no unpinned resident can make room. Repeated immutable
native image uploads refresh recency without decoding the same image again.

The shared rig adds `poseMatrices`, preserving native baseline rotations directly
while applying the same six-part, root, torso and split chest/pelvis hierarchy.
Bedrock uses it for emote rendering, including the body/cape/cosmetic attachment
poses. It no longer decomposes all native matrices into Euler angles whenever
one emote starts. Untouched six-part matrices and position-only track rotations
are retained; expressive transforms compose in matrix space. Native component
bytes remain unchanged by this Java renderer.

The existing exact-image ownership, supported standing pose, fresh readiness and
experimental isolated body-suppression gates remain in place. This source port
does not establish game-skin emote rendering, broader pose coverage, native cape
replacement or deterministic-video parity in the installed game. Verification
flags remain false.

### Static equipped-elytra observation (2026-10-26)

`native_equipment` now returns one coherent chest observation with occupancy
and an optional owned canonical item ID. The existing exact-image getter/vector
guards remain. Nonempty slots may resolve the public SDK's weak Item reference;
local name, namespace and namespaced name must agree, both the Item header and
referenced string bytes must remain unchanged, and the slot/vector/container
must survive the final recheck. Failure to decode a name preserves unknown identity
without turning an occupied chest into an empty one. No inventory methods run.

Actor and world snapshots carry that name and nullable `elytraEquipped`, separate
from the legacy `elytra` gliding flag. Empty chest means known unequipped; a named
nonempty chest identifies elytra or another item; unreadable identity stays null.
Java bounds the ID and rejects contradictions between item, occupancy and equipped
state. Preview hides cape geometry when folded wings are equipped, as Java does;
world rendering keeps its existing empty-chest and native receipt requirements.
Recorded-appearance readiness also rejects equipped custom wings without requiring
the actor to start gliding first.

The optional Item name layout comes from the attributed SDK declarations and has
not been validated against installed runtime memory. Native wing geometry, wing
poses and replacement remain unported. No tests, builds, helper scripts, dialogs
or Bedrock process were run, and no parity verification flag was enabled.

### Static appearance-history recording and timed restoration (2026-10-26)

`AppearanceSnapshot.capture` now separates owned logical state/references from raw
file I/O. Its change key includes emote start identity and held phase, omits
advancing time, and records loaded drawable state without scheduling downloads.
The ordinary single-snapshot writer retains its strict required-asset behavior.
`AppearanceHistory` observes changed states through the existing Java frame path;
one writer copies content-addressed assets into a private staging cache and seals
a format-2 `.arctic-looks.zip`. It coalesces multiple changes at one held instant.
Missing raw assets, backward clocks, identity/world changes and queue/state/asset
overflow fail the take rather than omitting a change and claiming completeness.

The queue holds at most 32 jobs / 8 MiB pending state, at most 4,096 observed changes
are admitted, and full state JSON lives in content-addressed asset entries. The
timestamp/hash index and asset descriptors must fit the 1 MiB manifest; raw assets stay within
4,096 files / 256 MiB. Finalization is asynchronous; only history stage `complete`
reports a finished archive. Cancellation marks failure and can leave an output
already being finalized, without overwriting a pre-existing file. Cleanup touches
only the managed files in the take's own temporary directory.

`history_begin`, `history_finish` and `history_cancel` share the existing serialized
native appearance channel and require an owner. Begin supplies an explicit timeline
origin; subsequent times derive from elapsed native world clock. Recording and
mounted playback are mutually exclusive. A replay-dispatch clock cannot be used as
a live recording clock. World loss cancels rather than silently sealing a prefix.
These controls require an already-loaded exact-image client; they launch nothing.

Each state also records the original cosmetic clock, independently of the assigned
timeline origin. This preserves animated cape and idle phase instead of restarting
them at frame zero. The reader validates ordered events and one world/local identity,
shares immutable decoded assets across states with one preparation budget (also
accounting for parsed state bytes), and
selects the latest recorded state at the checked export frame time. Local-player
remapping is checked against every event before mounting. Single-state saves of a
mounted history preserve the selected state's elapsed emote and cosmetic phase.
Export rejects a range beyond the history end and retains the same manifest/time/
draw-readiness fences. Owned cleanup can clear directly if state inspection fails
on an out-of-range sample; the Java owner check still protects unrelated mounts.

This records observed logical appearance, not native suppression or cape simulation.
Automatic `.abr` association and packet-clock alignment, physics history, armor/
held-item emote poses, native wings and deterministic GPU/video evidence remain
unfinished. No tests, builds, helper scripts or Bedrock process were executed.

### Static completed-tick cape history (2026-10-26)

The native cape cache now exposes owned filter snapshots for its exact completed
tick and session: old/current feet and cloak positions, body-yaw endpoints, walk,
bob and flight counter. Actor/world history retains these arrays with the matching
camera/depth scope. Camera sampling also registers the checked local actor, which
can be absent from model-draw callbacks in first person. The local fallback carries
its matching motion and endpoints. Actor memory accounting includes the newly
owned chest-item name. No native entity or inventory fields are mutated.

The Java snapshot sampler preserves the native interpolation/subtraction order
and shortest-yaw/flight blend. Recording compares a supplied motion sample against
its reconstructed endpoints, and rejects contradictory same-tick data. Relative
time subtracts the large elapsed clock before combining its fractional phase,
preserving small initial fractions in older sessions. No render callback advances
the chasing filter.

`CapeHistory` records a separate `bedrock_cape_ticks_v1` track in appearance
archives. Zlib blocks contain a magic/version, tick/start/rate and bounded UUID /
endpoint records. Blocks have at most 64 ticks, 128 actors and 2 MiB inflated data;
they share the serialized writer's 8 MiB queue and archive byte/file/index budgets.
The reader preflights every block's hashes, sizes, ordering, values, duplicate
actors, compressed-stream completion and index bounds before committing a mount.
Playback caches only one decoded block, with an additional preparation reservation,
and rechecks the source manifest when opening another block. Raw source pointers
and engine identities are never stored in that binary data.

Selected-time playback maps the recorded local UUID and reconstructs its sample
within that recorded tick. Gaps, absent actors or changing source archives return
no motion; the cape renderer hides the missing layer and draw readiness stays
false. Native cape readiness likewise declines to suppress a cape when recorded
motion is unavailable. Old appearance-only archives retain their previous behavior.
Status distinguishes a present physics track from coverage or verified output.

These endpoints belong to Arctic's Java-style filter, driven by observed game
ticks, rather than a newly inspected native engine cloth solver. Packet-clock
alignment, unseen-actor/tick coverage, arbitrary same-tick changes, native wings,
armor/held-item poses and repeatable GPU/video proof remain incomplete. Existing
experimental suppression gates and false verification flags are unchanged. No
tests, builds, helpers or Bedrock process were run for this source port.

### Static paired packet/appearance recording (2026-10-26)

New live packet takes receive canonical recording UUIDs. Packet-only camera export
preserves existing IDs and does not invent one for legacy inputs, retaining the
existing deterministic header behavior. The owned relay adds correlated recording
state, marker and finish controls. A marker is serialized into the take at its actual
queue timestamp and queued only to the initialized local viewer; its echo is removed
before upstream forwarding/recording. Playback continues filtering recorded latency
probes, so an old nonce cannot satisfy a new native arm.

Native post-tick receipts now copy the matching owned client clock. An optional
history-begin anchor names the exact marker/request/sequence/generation and recording
ID. The consumer requires its current primary-client completion, at most three
seconds old in wall time, before calling Java. Java waits for a sufficiently recent
matched world frame and derives the first sample time from that tick's elapsed clock.
The association is stored in the archive; arbitrary manual origins remain separate.
This names a queue/post-tick relation, not completion of the preceding world packets.

`recording::begin`/`finish` use the existing helper and adapter, with no game launch,
world join or account action. They refuse busy appearance owners, preserve their
lease for failed finalization retries, stop both capture streams before waiting for
writer flushes, and publish a new adjacent descriptor only after both report finish.
The CLI exposes native_record_begin/native_record_finish; live state reports the
recording identity/readiness. Stop and stdin EOF attempt paired finalization first;
a failed save produces a nonzero exit. Unexpected helper loss cancels the owned
appearance take where its native context remains available.

The descriptor binds the recording ID, packet bytes, appearance bytes and manifest
hash. The helper hashes the replay's original open descriptor on demand and rejects
source metadata changes after indexing; finalization also checks its original created
file identity. Auto-selection compares that digest against the adjacent source and
descriptor, rejects foreign IDs, changed/escaping files and corrupt/disappearing links,
then checks the mounted manifest and recording ID again. An explicit appearance file
keeps its prior behavior. Hashing happens while the export viewer is paused; ordinary
recording close does not incur the paired digest operation. The helper fingerprint
and installation include recording-control.cjs.

Appearance receipts now have a bounded history of up to 32 / 16 KiB and a report-size
fallback for long diagnostic paths/errors, preserving the existing 64 KiB host reader.
These limits do not change request correlation or make a timeout cancellation.

The source workflow provides one initial marker anchor and exact-content pairing.
It does not prove continuous clock alignment under lag, pause or rate changes,
unobserved prefix/tick/actor coverage, packet application, terrain, native wing or
armor poses, or repeatable GPU/video output. Alignment/native/video verification
flags stay false. All work in this entry was source editing/inspection only; no
builds, tests, helpers, pairing commands or Bedrock launch were executed.

### Static wing rendering port (2026-10-26, unbuilt and untested)

`native_elytra` observes only an actual selected `geometry.elytra` hierarchy with
the three vanilla rest bones and a known equipped `minecraft:elytra` chest item.
It checks finite affine matrices, positive bounded scales, orthogonal axes and
matching native actor/render/animation ownership. Completed wing matrices are
copied to the owned world frame; unresolved parent links are never traversed.
The separate affine contract preserves native nonuniform wing scaling instead
of treating a wing as one of the six rigid skin parts.

The Java renderer has left/right mirrored wing cuboids using the cape atlas.
Completed native wing poses remain under their independent entity model root;
they are no longer reparented through a humanoid emote torso. Previews can use
the observed wing matrices with their native controller posture. Live draws
require a separate Elytra replacement plan and native suppression receipt,
matching the ready cape hash/frame, actor scope and animation frame. The
existing descriptor/bone-copy suppression path owns and rolls back its edits;
the replacement cannot borrow an unrelated body or cape receipt. World-frame
parsing checks the receipt, equipped item, texture identity and complete parts.
Owned frame-history accounting also includes cape, skin-pose and wing strings.
The standard-part rest check now selects the vanilla slim shoulder pivots and
offsets from the observed native arm model, including their half-pixel shift;
it no longer compares every slim skin against the wide-arm rest metadata.

Wing suppression additionally requires `elytra_probe: true` in an already
authorized isolated body-probe configuration. It stays disabled by default;
this source change does not create or enable any probe configuration. Native
wing/atlas/glint/lighting alignment remains unverified, and custom wing glint is
not ported. Export readiness continues refusing custom capes on equipped wings:
live controller matrices are not a recorded deterministic wing-animation track.
All native wing and deterministic wing-export verification flags remain false.
Only source/installed-asset reads and source edits were performed. No builds,
tests, project helpers, game launch, process interaction or UI actions were run.

### Static paired-recording launcher integration (2026-10-26)

The Bedrock Offline card now routes paired appearance begin/save/cancel through
the same core marker/recording API as the CLI. A background job exclusively owns
the connector during each operation and returns it with any outstanding lease
on failure. Saving keeps the connection alive; stopping finalizes the pair before
helper teardown. Selecting another edition is disabled while a session/job owns
it. A failed begin retains its lease before sending a request that may commit
without returning an acknowledgement; only acknowledged cleanup clears it.
The CLI exposes retained leases and an explicit `native_record_cancel` operation.

Cleanup checks a fresh serialized Java State receipt, including explicit absent
history, actual owner and terminal status. An old world epoch can be released
when Java acknowledges its take is terminal; an active foreign history is never
cancelled. Live native context is still required for an authoritative receipt.
Save failures keep the connector/owner available for retry. A normal launcher
close waits for the paired save and relay shutdown, and aborts closing on save
failure. Bedrock ownership prevents tray handover/background update restart;
relay polling continues outside the Bedrock tab. Unconsumed result payloads
attempt finalization/owned cancellation on a background cleanup thread.

These paths do not implement the Java raw-packet ReplayBackend for Bedrock:
Java .mcpr and Bedrock .abr formats remain distinct. Native world reset/join,
continuous recording clock alignment, wing replay state, armor/held items,
audio and repeatable presented/video output remain incomplete. Forced shutdown
and persisted crash recovery are not established by the best-effort cleanup.
All work here was source inspection/editing only, without builds, tests, helper
execution, game launch, process interaction or UI automation. Full parity and
native/deterministic video verification remain unproven.

### Static protocol-driven world reset (2026-10-26)

`reset` can opt into `reconnect: true` on the pinned 1.26.51 RakNet listener.
It pre-serializes the official TransferPacket fields before changing generation,
targets only its existing loopback endpoint with reload-world set, flushes old
queued packets before transfer, and retires the old peer after delivery grace.
One active world is admitted; a second transport slot allows only the retiring
peer/fresh handshake overlap. Automatic replay reconnect is not enabled for
NetherNet or other protocol versions without matching transport evidence.

The reset owns a bounded transition with a 60-second deadline: awaiting a fresh
connection, initializing, catching up, then ready/failed. It checks a new viewer
ID, the previous offline UUID/name, the actual local-player initialization packet
and complete packet queuing to the paused target. Mutating playback/camera/export
controls are rejected while the automatic transition is pending. Old-generation
peer events are ignored by the editor and cannot clear a newer coalesced state.
Timeout/failure does not automatically start another helper or launch a game.
The manual reset route and late manual rejoin remain available.

The launcher exposes backward Seek and Restart world through these controls and
reports the same stages. Neither a transfer send nor a ready reset receipt proves
the installed engine actually tore down its old world, applied terrain, rendered
the requested scene or produced repeatable MP4 pixels. Export still needs its
existing native session/clock/camera/readback fences. Native reset repeatability,
NetherNet reconnect, continuous clock mapping, full appearance/armor/audio parity
and deterministic video remain incomplete. Only source/reference reads and edits
were performed; no builds, tests, helper execution or Bedrock launch occurred.

### Static fresh-world indexed export (2026-10-26)

The capture manager now supports scoped reserve/renew/activate. Reservation is
independent of world epochs, has a bounded 5–120 second lease and opens no files
or encoder. Other owners cannot start/stop/sample that slot. Activate requires
the matching live reservation; expiry cannot silently become an ordinary start.
Stop releases an unused reservation. Status reports remaining lease time and
the C status adds reserved/cancelled codes 6/7. Existing start behavior remains
available for the current-world workflow.

`ExportOptions.startMs` and the launcher's Reload world before export control
route through that reservation before changing playback. The worker checks one
initialized/caught-up local RakNet viewer, target/range bounds and a current Java
world snapshot with the same local UUID and no active appearance owner. It retains
reset cancellation authority before sending the transfer. It waits for the fresh
viewer/generation and exact paused packet target, then requires another native
world epoch and idle bindings in the same client process. Capture activation and
the existing appearance/clock/camera/marker/presentation fences follow only then.
The result retains old/new viewer and native-world identities in worldRebuild.

The worker renews the slot while waiting and cancels only its reset generation
and original viewer ID on failure. A late response cannot authorize resetting a
different owner. Cancellation cannot retract a transfer already sent. No new
helper, client or account session is created. Native initialization reports now
include current-world/local-player identity for this preflight; they do not
attest complete terrain or repeatability. Source inspection/editing only: no
builds, tests, helper execution, Bedrock launch or runtime capture was performed.
All broad native-world/deterministic-video verification flags remain false.

### Static equipment snapshot and replay observation port (2026-10-26)

`ActorEquipmentComponent` is read once for its armor/hand container pointers.
Known installed-image getter RVAs select the bounded vector layout; armor reads
four slots from the known 4–6 slot container and hands require exactly two.
Each group copies and rechecks headers, canonical item identity, validity/count,
vector bounds and vtable. The component bytes and ECS lookup are checked again
before publication. Missing groups remain unknown, with no fabricated empties,
durability, icon or inventory-total values. A separate chest observation remains
available if a neighboring armor slot prevents copying the full group. Native
history accounting includes every owned equipment string and duplicated chest.

Java world-frame parsing validates slot/count/empty consistency and cross-checks
the grouped chest against its existing observation. Armor rows now carry these
owned slot records; game-widget capability and icon rendering remain pending.
Appearance history includes bounded per-actor bedrockEquipment state, including
explicit unknown groups, in change detection. Core snapshot extension copies
adapter JSON, limits it to 8 KiB per actor and rejects fields/actors it cannot
associate. The existing 4096-event/file and raw-byte bounds still apply; frequent
equipment changes can exhaust that budget and fail the take explicitly.

Replay parsing validates the optional equipment state before mount, remaps it
with the actor and compares its observed fields against actual native equipment
at the selected timeline event. Missing observations do not become assertions.
Mismatch prevents render-ready acknowledgement and appears in status. A known
native body pose alone no longer attests a complete emote with unposed native
armor/held items: export currently requires known empty attachment containers.
Native equipment emote posing, item icons/durability, complete inventory data,
and broader Java parity remain incomplete. Only source/reference reads and edits
were performed; no builds, tests, helpers, process interaction or game launch.

### Static archive-backed item sprites (2026-10-26)

PackAssets reads only explicit absolute directory-pack roots, highest priority
first. It resolves each resource's directory archive, validates the v1 magic,
version, bounded descriptor count/name/offset/length and source identity, and
reads only the requested payload. Binary PNG/MCB bytes remain binary; no archive
extraction or decompression is performed. Declared stubs can use a loose file;
conflicting loose/archived payloads are rejected. Resources must remain inside
their canonical pack root. No root discovery or active-pack precedence is inferred.

ItemSprites resolves observed item keys on one daemon worker with 32 queued jobs,
up to 64 ordered roots and 4096 optional explicit canonical-ID/atlas-key bindings.
Native Slot.icon_key copies Item's atlas/name strings from the same stable prefix
as the canonical ID, then rechecks referenced bytes. It remains optional and
unverified; explicit and observed keys must agree. ItemAtlas reads one requested
key without constructing every definition, bounds and rejects duplicate keys,
accepts catalog comments and omitted texture_name, and rejects a conflicting
explicit atlas name. It masks unsupported higher-priority definitions, decodes
only unambiguous static PNGs, and retains bounded 16 MiB cache/16 MiB pending
pixel budgets. Per-file byte/dimension limits apply. Unsupported MCB/variant,
tint, glint, 3D item or durability behavior is not fabricated. NativeBridge exposes
session-bound bind/state entry points. The native active-stack observer and
restack invalidation source wiring are described below; they remain unbuilt and
unverified. The new archive reader/loader was never executed.

The GUI draws these loaded sprites and observed stack counts; icon-only calls
omit overlays. Armor is individually registered through supportsGameWidget,
whose default preserves other adapters' hasFeatures behavior. All occupied armor
slots must be known and resolved before emitting a live row set. World changes
clear resources and queued reads, and generation/session checks discard late
asynchronous work. Status exposes cache/upload bytes and pending/failed keys.
This does not establish correct active-pack selection, GPU/lighting parity or
complete item rendering. No builds, tests, helpers, Bedrock launch or runtime
render/capture was performed. Full native and deterministic-video parity remains
incomplete and unverified.

### Recorded wing render samples (2026-10-26, static source only)

WingHistory observes completed native wing draws alongside appearance recording,
using the cape track's precise anchored clock but a distinct render-sample format.
Each frame stores a double time, completed native tick, rate, and up to 128 UUIDs.
Each actor stores a rigid torso reference plus two affine wing matrices (36
floats). Up to 64 frames are zlib-compressed per content-addressed block, with
2 MiB inflated bounds and the shared AppearanceHistory queue/archive limits.
All-empty blocks become explicit index gaps. Duplicate times must have identical
owned samples; clock reversals or ambiguous same-time poses fail recording.

Mount validates all chunk descriptors, exact sample order, tick monotonicity,
matrix bounds/handedness/scale, index agreement, decompression completion and
trailing bytes. The reader pins the archive manifest hash, caches at most two
blocks, and reserves 16 MiB in the shared preparation budget. UUID remapping uses
the same recorded-local/viewer identity policy as cape and equipment history.
It samples the recorded curve with shortest-path rotation slerp and linear
positive scales/translations, preserving exact endpoints. Missing actors,
skipped ticks, gaps longer than two observed tick periods and track endpoints
without a following sample never use a live controller or extrapolated pose.

The saved torso reference remains validated metadata; independent wing rendering
does not apply an emote torso delta against it (wing-layer-root-source.json).
Old archives lacking the track cannot authorize replay wing replacement. The
existing native receipt, experimental isolated gate and texture ownership checks
remain required. This sampled curve does not prove the native controller's
between-sample output, lighting/glint or GPU parity; render-ready still rejects
equipped custom wings for deterministic export. Status reports track presence,
sample errors and wingInterpolationVerified=false. Only source reads/edits were
performed; no build, test, helper, game launch or runtime hook was executed.

### Native directory-pack stack bridge (2026-10-27, static source only)

Offline analysis of the existing cached executable establishes the primary
ClientInstance table at RVA 0xe9731b0 and its resource delegate at 0x5dbdfb0.
The delegate reads game at +0x1a8 and selects game vtable +0x260. The observer
decodes only in-image mov/lea pointer/address getters, with at most three direct
jump thunks; it never invokes the getter. The resulting manager must have table
0xe7b7f10, its full stack at +0xb8 must have table 0xe7c84b0, and its vector uses
0x128-byte PackInstances with the ResourcePack pointer at +0x100 and selected
subpack index at +0x120. The index and default-load implementations traverse
the vector forwards. The adjacent revision/pointer bytes contribute opaque
identity data; they are not treated as independently verified counters.
Mutable PackStats and padding are excluded from the identity hash so ordinary
asset loads do not continuously invalidate the sprite cache.

ResourcePack::getResource (0x1931650) first tries its selected child with index
-1, then an optional own overlay strategy, then its base Pack strategy. Client
wrapper/implementation indirection is read from the actual code rather than
assuming the public server-side direct-member layout. Only Directory strategy
table 0xe7c9f40 is supported here. Its getAsset (0x19b9540) prefixes resource
paths with the resolved directory string at +0x70. The observer requires that
actual string to be absolute; it does not map filesystem aliases or construct
an install path. Exact field/address evidence is retained in
inspection/resource-packs-1.26.5203.0.json. No game functions or SDK implementations
are copied into the adapter.

Every reference, vector/header, string header and referenced path allocation is
copied and checked again. There are at most 64 instances/64 roots, 4096 UTF-8
bytes per path, 64 KiB of total paths, 2048 guards and 256 KiB of guarded bytes.
A pass-through _composeFullStack hook at 0x14c8d20 maintains an in-flight count
and revision before/after the original call. Copies spanning a restack fail;
buffered world draws re-observe the current primary context and publish an
unavailable descriptor if their identity/revision expired. This also catches
selected-subpack or instance identity changes that did not compose a new vector.
The hash-matched install path registers this observer; it does not enable any
cosmetics suppression probe. Owned snapshots and their heap capacities join the
existing bounded world/GPU handoff history.

Java validates the revision, all groups and path budgets before asynchronously
preparing resources. PackAssets preserves per-instance groups: an atlas file in
a selected child masks the parent's entire file for that instance, while keys
still merge across instances. PNG resource lookup follows the same grouped
order. Native revisions supersede compatibility bindings; restack/error/world
gaps clear caches and stale queued work. A fresh frame retries preparation after
a gap. ZIP/encrypted/custom entries, unreadable or relative directories, unknown
getters and unstable memory invalidate the whole stack, never exposing lower
vanilla resources as a substitute.

The observer, hook ABI, resolved paths, subpack precedence, in-place reload
coverage (including files changing without metadata updates) and live pixels
remain unverified. Status keeps
activeEngineStackVerified=false. This turn ran source reads/edits and offline
byte disassembly only: no project code, builds, tests, game/test helpers, Bedrock
launch or process interaction. Full Java/native rendering and deterministic
video parity remain incomplete.

### Observed static icon frames (2026-10-27, static source only)

Offline Item-table analysis locates getIconInfo at virtual slot 0x75. Its base
implementation, RVA 0x24c2530, queries components through slot 0x18 and tests
isHumanoidArmor through slot 10. If both select constant-zero method 0x76c20,
the stack/component/trim branches cannot run. With alternate atlas-name length
at Item+0x98 equal to zero, the ordinary branch copies Item+0x68 and frame
Item+0x58 into an Icon-type result. No stack/animation argument or aux/damage
value chooses the frame on that constrained branch. Other item overrides, the
humanoid selector, component-selected keys, raw UVs, block icons and alternate
atlases remain unsupported. Evidence is retained in
inspection/item-icons-1.26.5203.0.json; no getter is invoked by the adapter.

Slot.icon_frame is optional and accompanies the independently checked canonical
ID/atlas/icon-name strings. The observer requires matching copied/current vtable,
exact method targets, expected selector prologue and constant-zero code bytes,
a nonnegative u16 frame and a final stable Item prefix/weak holder. A raw icon
name without that constrained receipt no longer authorizes native PNG lookup.
Explicit adapter keys retain the compatibility path for singleton textures.
The frame is serialized in equipment history, validated as optional bounded
metadata and compared during replay when known. Earlier archives remain readable.

ItemAtlas streams up to 65536 variant paths, selecting exactly the observed
index. Missing/unknown/out-of-range variants, nonzero indices on singleton
strings and unsupported higher-priority definitions fail without clamping or
falling through. Sprite caches include the selected frame in their identity.
This is base PNG art only; tint, glint, 3D item rendering and durability are not
attested. Status retains iconFrameLayoutVerified=false and
nativeItemRenderingVerified=false with staticBaseLayerOnly=true. No build, test,
project code, game/test helper or Bedrock process was executed. Full native
world cosmetics and deterministic replay/video parity remain incomplete.

### Shared emote target math at native draw time (2026-10-27, static source only)

EmotePrograms publishes actor/session-scoped immutable program states through an
AtomicReference. A program records animation identity, original phase/origin,
held state, clock domain and valid event range. Tokens remain stable while that
logical emote is unchanged; a stop, event switch or replacement invalidates the
old token. Replay range bounds exclude the next appearance event and preserve
the recorded local-player remapping. Live origins use Playing's existing native
animation clock. Publishers expire after 250 ms, support at most 128 actors, and
bound unique retained animation graphs to 16 MiB using a cached conservative
Animation.retainedBytes weight. Callbacks never query Looks/UI state or files.

Native skin readiness optionally includes emoteToken. The existing owned draw
observer first attests its selected standard SkinPose, then samples a checked
primary-client clock with the same interpolation bits. Timing contains completed
tick/elapsed/rate/partial values and the existing held-clock/dispatch receipts.
Replay math calls the same CosmeticClock validation as the world renderer; an
unqualified replay marker never falls back to the viewer's simulation origin.
JSON f32 timing values round through float before double clock arithmetic to
preserve native integer-ms boundaries. Clock/context copies are checked again
after the callback, and unknown/changed timing drops the target.

The process-lifetime bridge holds an Arc<JavaVM>/GlobalRef, attaches each caller
thread independently, and uses a bounded JNI local frame plus a reentry guard.
It calls NativeBridge.sampleEmotePose with copied baseline matrices. The Java
service validates six rigid parts, executes the shared Rig.poseMatrices code and
returns double[74]: an exact integer millisecond clock, original float phase,
and 72 original float matrix values. Native code rejects non-finite, lossy,
non-rigid, reflected or out-of-bound results. No game getter, constructor or
resource loader is called. Pending callback exceptions are cleared and reject
the target instead of crossing a game ABI boundary.

NativeTransform records a target only under the same actor/world scope,
animation frame and exact original SkinPose parts. Conflicting targets disable
the target for that pass. Only the dedicated player emotePose payload carries
the matrices to Java; nativeGlobal avoids duplicating them. Owned target string
capacity and inline matrices participate in the existing world-history budget.
The world parser validates frame identity and baseline agreement. The software
renderer consumes preposed parts only when its current token, animation, phase
and cosmetic time agree; knee/limb geometry still uses that same emote phase.
Absent/obsolete targets leave the existing shared rig renderer in place.

This does not retarget any native armor, held-item or other attachment palette.
No native mesh or palette is written by this module. Binding bone indices,
inverse-bind spaces, multi-pass coverage and deferred palette storage lifetime
remain required before native equipment posing can be enabled. The known-empty
attachment export gate remains unchanged, as do the isolated body-probe gates.
Status explicitly reports nativeEquipmentPoseApplied=false and
nativeEmotePoseVerified=false. Only source/reference reads and edits were
performed: no build, test, project helper, Bedrock launch or process interaction.
Full native world/cosmetics and deterministic video parity remain unverified.

### Owned emote palette candidates (2026-10-27, static source only)

The existing skinned-description observer now hands the original spans to
native_emote_palette before any experimental suppression. It first requires
the same actor/world scope, current emote target, animation frame, native root
matrix and tint. Both span descriptors must agree byte-for-byte with their
constructor arguments. Counts are bounded to 256 matrices per span. The reader
copies affine matrices, rereads their bytes and headers, and rechecks the actor
target. These checks detect observed changes; they do not lock the engine or
prove an atomic snapshot. Only the latest owned current/previous palettes and
current-pose candidate are retained. No native pointer escapes the copy and no
replacement storage is lent to a deferred renderer.

The candidate builder converts the shared six-part pose back from row-major
3x4 blocks to column-major 4x4 pixels. It requires unique exact element agreement
with a copied current reference part before selecting its shared emote target.
Unknown entries and identical reference matrices remain unresolved. It never
assumes palette order from entry count, assigns every identity to the torso, or
uses current emote targets for a previous-frame palette. Candidate matrices are
owned diagnostic data, not writes or proof of native bone identity. Status
reports match/ambiguity counts and owned matrix bytes without publishing raw
addresses, and keeps nativeEquipmentPoseApplied and paletteLifetimeVerified false.

Offline disassembly of the already cached pinned image traced owner vectors at
+0x510/+0x528 through the constructor at 0x5e9e4e0 and call at 0x12eabf6. The
append path at 0x12ecf20 assigns part palette/group indices at +0x88/+0x8c and
appends identity matrices. The Model::dataDrivenDraw group and owner preparation
paths are recorded in inspection/emote-palette-1.26.5203.0.json. Their engine
type names, palette deformation spaces, previous target pairing and renderer
retirement remain unresolved. No new hook or native
palette mutation is enabled. Source and cached bytes were read; no build, test,
project-code execution, game launch or game-process interaction was performed.

### Indexed native palette bindings (2026-10-27, static source only)

The cached image provides an explicit association beyond numeric candidates:
0x1883b0 selects the animation hierarchy using geometry+0xe8; 0x1c36670 selects
a 0x240-byte model part from geometry+0x38; and 0x1c21d4c uses part+0x90 as its
BoneOrientation index. The append path assigns that part's palette slot at
+0x88 and owner index at +0x8c. native_palette_binding copies and cross-checks
these indices through the geometry groups, owner entries and member-part vectors.
Each member must lie on a real part-vector element, be unique within its owner,
and agree with its exact palette slot and owner index. Both constructor spans
must identify exactly one owner before named bindings are returned.

Bounds are 256 geometry parts, 64 groups/owners, 512 total bound entries, 1024
guards and 64 KiB of guarded metadata. All relevant vector headers, member
pointers, index fields and the hierarchy index are reread. The binding lives in
a nested thread-local scope covering only the original dataDrivenDraw call.
It is removed from Sample before the sample is remembered, omitted from sample
serialization, and never added to native submission packets or world history.
Nested unknown draws mask the outer binding; an overflowing scope disables
lookup until that scope exits. Returned names, indices and matrices are owned.

The emote candidate builder uses this named index map when available, avoiding
identity-matrix ambiguity between the head and torso. Each palette matrix must
agree exactly with the indexed native bone matrix; inverse-bind/deformation
entries stay unresolved and never fall back to numeric matching. Known six-part
attachments use desiredPart * inverse(referencePart) * nativeAttachment in
entity/pixel space, preserving the native local offset and scale. Unknown bone
names remain unresolved. Rigid inversion rejects non-finite, non-affine,
non-orthogonal or reflected reference parts. The binding is checked again after
the candidate calculation. Status identifies the binding source, hierarchy,
indexed entry count and non-direct entries.

This remains an owned candidate calculation: no native palette is patched,
previous-pose target or retirement event is invented, and no equipment/export
readiness gate is relaxed. Engine bone binding, equipment application, palette
lifetime and complete replay/video parity remain unverified. Changes were read
back as source only; no build, test, project-code execution or Bedrock launch
was performed.

### Native owned-copy emote hook (2026-10-27, static source only)

The pinned image's 0x5eaad10 helper synchronously copies a count/pointer span
of 4x4 matrices into caller-owned storage and pads unused entries to eight with
identity matrices. Its body reads only rcx/rdx arguments and writes 64 bytes per
matrix. All six direct call sites were inspected; none consumes a return value.
The candidate rendering context table at 0xe9765c0 has 0x5e4b240 at virtual
slot +0x358. That method calls the eleven-tag visitor at 0x5e4b4f0; tag 7 enters
at 0x5e4b9d2, and its current-span path calls the copy helper at 0x5e4e3c5 after
loading the description's +0xc0/+0xc8 fields. These are static image facts and
inferences about the consumer, not a new runtime validation.

native_emote_upload hooks the copy helper using the pinned prologue and checked
image base. It forwards the original operation exactly once, then can modify
the completed native-owned output under the existing isolated body-probe gate.
It requires an indexed named binding and a description receipt established
after the observer's root/tint checks. Receipts disappear at the end of the
draw scope and are cleared before every subsequent description constructor.
Numeric-only candidates cannot authorize upload changes. Suppressed body/cape
descriptions point to a different palette and do not match an original-source
receipt.

The callback accepts only 1..8 matrices, rechecks source bytes/descriptors,
requires the full 512-byte output to match the native copy plus exact identity
padding, rejects source/output overlap, and recomputes the named current-pose
retarget. Only output values change; source palette bytes, span pointers,
counts, matrix order and padding are preserved. A failed or partial OS write,
readback mismatch, changed target or changed binding restores the original
native output through a scope guard. The source and actor/clock binding are
checked before and after the write. Exceptions do not cross the game ABI.

This avoids supplying deferred consumers with Rust-owned replacement memory;
the engine retains ownership of the same output and subsequent upload path it
already uses. It does not prove that every renderer path reaches this helper,
that every palette has direct bone-to-entity space, or that the selected draw
was visibly rendered. Previous-pose retargeting, attachment-name coverage,
multi-pass coverage and native equipment/export verification remain incomplete.
Status records owned output retargets separately from the false rendering and
deterministic-video verification flags. Normal-operation and export gates were
not relaxed. Only source and cached-image bytes were read and edited; no build,
test, project-code execution, Bedrock launch or game-process inspection occurred.

Further static tracing identified the standard palette writer. In 0x1c20f20,
part+0x84 selects the outer 0x60-byte group; part+0x90 selects BoneOrientation,
and its +0x38 matrix is passed directly to 0x12ee7f0 using the part's +0x88/+0x8c
palette/owner indices. That writer copies all sixteen values to owner+0x510's
indexed current entry. Previous at +0x528 uses a separately flagged matrix or
copies current as a fallback. The standard branch therefore has direct
bone-to-entity palette space; override branches and previous cosmetic timing
still need coverage. Owning current/previous vector regions must not overlap,
including across owners, before any upload receipt can be accepted. Unnamed
child bones, such as outer layers, still need their explicit model-part ancestry
mapped to the six-part rig; no name-based parent guess is enabled.

### Native child-part emote inheritance (2026-10-27, static source only)

The indexed binding reader now copies the actual model-part tree. Geometry+0x50
contains the root indices used at 0x1c36670; each part's +0x188/+0x190 vector
contains the child pointers followed by 0x1c21d20. The recursive call at
0x1c21dd0 receives the accumulated parent matrix after the native bone update.
Null child pointers are skipped exactly as in that traversal. All non-null
pointers must address complete 0x240-byte elements in the same part vector.
Root indices, child lists, vector headers and the part's +0x84/+0x88/+0x8c/+0x90
group/palette/owner/bone indices participate in the reread guards.

The graph must be a complete rooted forest: unique roots, one parent per child,
no repeated edges, cycles, self-links or unreachable parts. Part bone indices
must belong to the selected animation hierarchy. An iterative parent-before-child
pass resolves each part once. A six-part native bone anchors itself; an unnamed
descendant inherits its nearest explicit rig ancestor. A resolved unanchored root
remains unanchored. This does not use BoneOrientation's unresolved parent defaults
or infer parents from names such as jacket, sleeve or hat. Each palette member
also has to corroborate its outer group index, not just its owner and palette slot.

The copied binding retains the original bone name/index/matrix plus its rig role,
anchor bone index and ancestor depth. Retargeting still requires exact agreement
between the current palette and that original bone matrix. For a descendant it
uses desiredAncestor * inverse(referenceAncestor) * nativeChildMatrix, preserving
the child's own offset, rotation and scale. Both the diagnostic candidate and
the native owned-copy hook consume this same mapping. Upload diagnostics report
at most eight owned bone/anchor records; no native addresses are published.
Metadata was initially bounded to 64 KiB, with at most 2048 reread guards for the
additional tree/index fields. Scoped pointer lifetime and receipt clearing are
unchanged. Unanchored subtrees, override/deformation palettes and cross-geometry
attachment paths still reject retargeting, and previous-pose timing remains
unresolved. The inspected 0x1c21590 helper returns a part predicate rather than a
previous-pose timestamp, so it cannot supply that missing clock. No build, test,
project-code execution, Bedrock launch or game-process interaction was performed.
Full native world/equipment and deterministic replay/video parity remain unverified.

### Observed native previous=current fallback (2026-10-27, static source only)

native_palette_writer now observes 0x12ee7f0 with its checked 27-byte prologue.
Both direct callers, 0x1c20a71 and 0x1c211bf, pass seven Windows x64 arguments:
group, owner index, palette slot, byte update predicate, current matrix pointer,
32-bit draw count, and a pointer to a previous matrix plus its byte flag at +0x40.
The observer forwards every original argument once and never edits these inputs
or the owning palette vectors. It copies only the fields the selected branch
uses: current plus the flag, and previous matrix bytes only when that flag is 1.
The standard/legacy callers explicitly clear the no-history flag at 0x1c2114a
and 0x1c20a46. Other/override paths are not assumed to do so.

Before and after the original write, bounded metadata guards corroborate the
owner/member vectors, slot/owner indices, group/bone index fields, source values
and actual current/previous output entries. Only an observed update with flag 0
and exact expected output copies establishes previous=current. Flag 1 supplies
an explicit previous input with unproven history, even when the matrices are
identical. An update predicate of zero records no fallback and replaces any
older proof for that slot.
Failed observations invalidate the actor scope's fallback ledger.

Each unambiguous native ActorScope owns its ledger, bounded to 512 entries and
256 KiB including vector capacity and retained metadata bytes. Actor/context,
world pass and completed animation frame are rechecked when recording and when
qualifying an upload. Input matrix/flag addresses may point into a native caller's
stack and are dropped immediately after observing that call; only checked owning
heap metadata and copied matrix bytes remain until the actor scope ends. Ledgers
are absent from Global, submissions, JSON frames and persistent world history.
Nested actor writes invalidate matching owner/slot records in other active
scopes; an unidentifiable write invalidates those ledgers. This prevents a shared
geometry buffer overwritten by another actor from inheriting an older fallback
record merely because both actors were standing in the same pose. Skipped or
overflowing actor scopes mask outer ownership until their guard exits.

The owned-copy hook now recognizes a previous span only if every entry has this
fresh explicit fallback proof, its corresponding current description passed the
root/tint checks, and current/previous metadata and matrix bytes still agree.
It then applies the same current emote target that the engine's fallback chose.
The output must be disjoint from both owning source palettes as well as the
input span. Existing full-copy/padding validation and rollback checks remain.
Status separately counts previous-current-fallback outputs and retains false
historical-previous-pose, equipment-rendering and deterministic-video flags.
No older cosmetic time, previous render frame or historical emote phase is
invented. Genuine previous-pose timing, multi-pass/attachment coverage and full
replay/video parity remain incomplete. Normal-operation/export gates remain
unchanged. Only source and cached image bytes were read/edited: no build, test,
project-code execution, Bedrock launch or game-process interaction was performed.

### Native cache keys and current-upload pairing input (2026-10-27, static source only)

Further cached-image tracing identifies Actor+0x118 as a map keyed by the model
part's own HashedString at +0/+8. Its tree node stores the copied hash at +0x20,
string at +0x28 and group-vector header at +0x50. The part's +0x84 group selects
a 24-byte inner vector, then +0x88 selects a 64-byte matrix. This key is separate
from the animation hierarchy's bone name; no alias is invented between them.
The geometry binding now guards and copies these native keys, with a 128-byte
UTF-8 name limit, alongside the part's scalar current-scope identity. Additional
key/string guards raise the metadata byte limit to 128 KiB; the 2048-guard limit
and all existing topology, actor and palette checks remain.

The producer at 0x1c21700 checks the actual part predicate at 0x1c21742, obtains
the map entry at 0x1c21760, copies BoneOrientation+0x38 into the selected cache
matrix at 0x1c21bae..0x1c21c7d, then visits children at 0x1c21cc7. Its root wrapper
is called at 0x1cb8eb0 after the model draw. A separate consumer branch can set
the previous-input flag to 1 after copying the current bone matrix when there
is no suitable cached entry (0x1c210f3). Flag 1 alone therefore proves neither
an older matrix nor an older cosmetic clock. Existing upload qualification
continues to reject these inputs without explicit observed fallback evidence.

Each ActorScope now owns a separate bounded current-upload ledger (512 parts,
256 KiB including capacity and retained strings). After the original native
copy, the observer requires an attested named current span, complete 512-byte
copy/padding agreement, disjoint output, and stable source/binding/actor/frame.
It records the exact native matrix and the actual supplied matrix for each
part. An unchanged native draw is recorded even when no current emote exists;
successful emote retargets replace it with their token, time and phase. Later
unmodified copies replace earlier emote copies for the same part. Previous
fallback copies do not replace current-upload records.

Root/tint-checked descriptions may establish a named receipt independently of
an active emote; both copied description spans must match the original source
headers before and after attestation. Retargeting still requires a complete
valid named candidate.
Unidentified copies, failed retargets and inconsistent observations invalidate
the ledger. Nested uploads invalidate other active actor scopes, and masked
scopes cannot borrow outer ownership. Lookup requires the exact copied key,
current-scope part identity and bitwise native matrix agreement, returning only
owned pose values. No source matrix address is retained or dereferenced later,
and no upload record enters serialized Global, frame packets or history.

These records supply diagnostic native-cache write pairing, described below.
The historical consumer/upload implementation follows below; a matching idle
matrix must never manufacture a historical emote phase. Full native cosmetics,
multi-pass coverage and deterministic
replay/video parity remain incomplete. No gates or verification flags changed.
Only source and cached image bytes were read/edited: no build, test, project-code
execution, Bedrock launch or game-process interaction was performed.

### Observed cache-write pairing (2026-10-27, static source only)

native_emote_cache adds read-only observers for the pinned recursive cache writer
at 0x1c21700 and its actual predicate at 0x1c21590. Checked 21-byte/25-byte prologues
precede installation. Both original functions receive their native arguments
exactly once; the predicate's original byte result is returned unchanged. The
observer never calls the predicate a second time, edits a bone, changes a native
cache entry or replaces an output matrix. Callbacks use the existing isolated
body-probe gate, which remains disabled for normal operation.

Before the producer, guarded copies corroborate the current actor/component,
render params, geometry part vector, exact opaque part key and group/slot/bone
indices, bone span and BoneOrientation+0x38 matrix. The exact part/key/native
matrix must already have an owned current-upload record in the same actor draw
scope. A bounded TLS scope matches only this producer's part/params/predicate-key
triple to the actual predicate callback. Duplicate observations are ambiguous;
missing inputs, overflow or failed scope creation mask outer ownership. Native
hidden/false-predicate paths do not qualify as observed cache writes.

After an observed true result and the original producer return, the reader copies
the native map and corroborates the resulting selected matrix byte for byte.
Static insertion at 0x1c87d1e confirms the map's node count at +8. The bounded tree
walk checks that count, unique nodes, parent links, head/nil markers, unique exact
key match and group/matrix vector bounds. It rereads every copied guard (at most
256 nodes, 2048 guards and 128 KiB), rechecks all producer inputs and requires the
same current-upload record and actor/frame afterwards. It does not infer a write
from a before/after numeric difference; the observed native predicate and pinned
producer control flow provide the write event even when a standing pose is equal.

Status retains the latest owned diagnostic pair: actor UUID, world scope/frame,
copied key/group/slot, native/supplied matrices and optional sampled emote clock.
All native addresses and caller-stack span headers expire with their synchronous
producer scope. Monotonic observation sequences prevent a returning parent from
replacing a later child's diagnostic pair. These sequences do not establish GPU
or cross-thread rendering order. The pair never authorizes historical upload,
enters replay frames or supplies an old pose to the renderer. Bounded history
and destruction invalidation are described below. Other writer and consumer coverage,
multi-pass coverage and equipment/video parity remain incomplete and unverified.
No build, test, project-code execution, Bedrock launch or game-process inspection
was performed; this implementation has only been reviewed as source.

### Cache lifetime and retained emote poses (2026-10-27, static source only)

The cached owner constructor at 0x19e6890 allocates a 0x68-byte tree head, sets
its self-links and nil/color bytes, and stores it at owner+0x118 (0x19e69fa).
The owner destructor at 0x19e7930 first disconnects the animation component's
+0xc0 back-reference, then passes that tree's root to 0x1a77c60 at 0x19e7c4a
and frees the head at 0x19e7c5b. A map destructor at 0x19e6830 also calls the
same typed eraser. The eraser recursively visits right subtrees, iterates left
subtrees, releases every 24-byte inner vector in node+0x50/+0x58, releases the
outer vector and HashedString, then frees each 0x68-byte node. Its three direct
call sites are 0x19e683f, 0x19e7c4a and recursive 0x1a77ccd; the cached direct
branch scan found no E9 tail jumps to this entry. This establishes a destruction
boundary, not complete runtime coverage of every possible cache mutation.

native_emote_cache now observes that eraser with its exact 16-byte prologue and
one-pointer void ABI. It increments a process-local history generation before
forwarding the original function once, including nil-root calls from empty-map
destruction. History never dereferences retained native addresses: keys combine
the checked world epoch/UUID, an opaque randomized owner/context/level identity,
and the copied native cache key. A witness hashes the freshly guarded head,
node and vector backing identities. Destruction invalidates the history generation;
unknown, unobservable and probe-disabled producers discard records for their
checked native owner. An unreadable owner falls back to global invalidation,
while a known null owner follows the native no-write path. Unsupported draws
by other entities therefore do not erase unrelated player history. Matching
current-thread actor upload ledgers are invalidated too. Generation exhaustion
disables history, and frame-upload records carry the same global generation.

The producer observer no longer requires a current render receipt merely to
observe a write. After an actual true predicate result it reserves that slot
before returning to the native write, removing any older supplied pose even
when the new native matrix is numerically identical. Without a current upload,
the write leaves a slot without an eligible supplied pose. With a corroborated
upload and cache readback, a ticket commits the owned native/supplied matrices
and sampled emote clock. Failed/dropped tickets cancel their pending write.
Known hidden parts skip their own predicate without masking their valid child
observers. Unknown scopes mask outer ownership while independently validated
children can establish their own scope at the current mask depth.

The owned cache history is bounded to 4096 keys, 512 pending writes and 4 MiB,
including vector capacity, retained key strings and emote tokens. Completed old
slots may be evicted; no pending slot is evicted. Later child write sequences
cannot be overwritten by a parent's delayed return. Overlapping writes from
different native threads mark the slot conflicted until a subsequent uncontested
write. Historical lookup rejects active producers, pending/conflicted records,
future animation frames, different backing witnesses, changed generations and
non-bitwise native matrix matches. Unchanged uploads require native and supplied
matrices to match exactly when there is no emote clock.

This implements owned historical storage/lookup in source. The consumer below
uses explicit cache-branch provenance and writer copies; the previous-input flag
or matrix equality alone cannot authorize an old cosmetic phase. Replay,
normal-operation and export gates remain unchanged. Lifetime/mutation coverage,
native rendering and deterministic replay/video remain unverified. No build,
test, project-code execution, game launch or game-process inspection occurred.

### Historical previous-palette upload (2026-10-27, static source only)

The part preparer at 0x1c20f20 has six Windows x64 arguments: part, render params,
bone span, predicate key, geometry pointer, and byte previous-request. Its cached
path calls 0x1c87b10 three times, returning at 0x1c21388, 0x1c213ca and 0x1c21416.
Only after the third lookup does it copy the cache's selected 64-byte matrix.
It then calls the native predicate and returns at 0x1c2156a. A current-matrix
fallback instead returns from that predicate at 0x1c2110b, after zero, one or two
lookups. Both flag-one paths reach the palette writer at return RVA 0x1c211c4.
These exact call sites distinguish the copied source even when its values equal
the current pose. The lookup's existing-entry return clears its result flag at
0x1c87fb9; its native result pointer and caller-owned result storage are preserved.

Read-only prepare/lookup hooks use checked 19-byte/21-byte prologues. Nested TLS
consumer scopes match the actor, part/key, params, geometry group/owner/slot,
bone span and animation frame. Each lookup requires the expected first native
stack-frame RVA from the bounded stack capture. The third successful lookup
copies a guarded cache snapshot and obtains its owned historical receipt. The
actual predicate callback freezes this provenance before Molang executes, with
the exact fallback/cached predicate return site required as a second check.
Late lookups, duplicate predicate observations, missing/unexpected stack frames,
scope overflow and ambiguous contexts reject historical qualification. Snapshot
allocations are capped at 1 MiB per native thread and released after the writer
attempt or preparer return. No original function is called more than once.

At the writer, current/previous input bytes must match the checked source branch
and the original write must produce exactly those matrices in the owning palette.
Only then is the previous pose/clock sealed as owned data in the actor's writer
ledger. Caller-stack addresses and cache snapshots expire immediately. A sealed
copy is checked against its current owning palette/metadata on every use; it
does not acquire a newer clock when the native cache advances afterwards. Nested
writes still invalidate shared owner/slot receipts, and skipped writes cannot
inherit earlier proof. Flag-zero and explicitly observed flag-one current
fallbacks remain distinct from genuine cached history.

The eight-matrix owned-copy hook now handles mixed previous entries. A native
current fallback uses the current target only for that bound entry; a historical
entry uses the exact supplied matrix from its earlier observed upload, retaining
its own optional emote token/time/phase. It never substitutes the current clock
for old history or assumes all cached parts came from one render time. Historical
copies also work when the current emote has ended. Native output ownership,
full copy/padding checks, source/target/binding rechecks and write rollback remain.
Only current-source copies update the current-upload ledger.

A previous source can be identified without requiring its old cosmetic record.
An unavailable historical pose leaves that previous upload native, while the
current upload can still be recorded and paired with the next cache write. This
avoids a missing-history cycle that would otherwise prevent recovery after cache
invalidation. Diagnostics distinguish previous-current and historical outputs,
with at most eight per-entry clocks. Source implementation flags are separate
from the unchanged false native-rendering and deterministic-video verification
flags. This is not proof of full equipment/cosmetics, multi-pass or replay parity.
No tests, build, project-code execution, Bedrock launch or game-process inspection
was performed; changes were reviewed only as source and cached executable bytes.

### Built-in Windows MP4 encoder for Bedrock (2026-10-27, static source only)

Bedrock capture now declares an optional encoder kind: ffmpeg (the legacy
default) or arctic_builtin. The existing ffmpeg path field carries the selected
executable in both cases. Host indexed options and real-time recording omit the
default kind when publishing legacy FFmpeg requests, preserving their old JSON
shape. Indexed export passes an explicit non-default kind through capture
reservation/activation and reports it in the final outcome. Native capture
status and each indexed frame's manifest record the backend and padded encoded
dimensions separately from original presented dimensions and RGBA hash.

The launcher worker prefers its existing Windows H.264 encoder when available,
then an installed FFmpeg, with manual selection when needed. Replay controls
include Use FFmpeg for an explicit external-encoder preference. The immutable
export request retains that preference while its worker owns the connection.
No encoder download, helper/client startup or account/sign-in action is added.
Real-time MP4 recording uses the same selection path. Both encoding routes still
require the already-running, loaded native client when used by the product.

Native capture creates its new output file exclusively and redirects that exact
file handle to the child process. The internal --encode-video-stream entry point
runs before launcher UI, single-instance handover or data-directory setup. It
duplicates the inherited handle and wraps it in a seekable COM IStream with
serialized file access and independent clone cursors. The child never reopens,
deletes or replaces an output filename. Media Foundation wraps that IStream as
an IMFByteStream and receives the MP4 container attribute explicitly, following
[MFCreateMFByteStreamOnStream](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nf-mfidl-mfcreatemfbytestreamonstream)
and [MFCreateSinkWriterFromURL](https://learn.microsoft.com/en-us/windows/win32/api/mfreadwrite/nf-mfreadwrite-mfcreatesinkwriterfromurl).
The existing hardware/software encoder fallback resets this same owned stream
when retrying initialization. Java's filename-based encoder route is unchanged.

The stream accepts bounded 1..3840 by 1..2160 RGBA frames, FPS 1..60 and at most
two hours. Indexed mode requires exactly the requested complete frames and EOF;
truncated, extra and zero-frame streams fail. Real-time mode permits an earlier
complete-frame stop. Odd source dimensions are padded with black to the next
even encoder dimensions, preserving every original pixel. Conversion uses the
shared BT.709 limited-range NV12 path. Native queue bounds, frame/hash manifests,
capture ownership, hidden child window, watchdog and exit/diagnostic handling
remain in place. The launcher has no additional game control in this route.

This closes the source-level dependency gap between Java's bundled MP4 encoding
and Bedrock's prior FFmpeg-only capture. It does not implement replay audio,
terrain/application completion or prove repeatable decoded video. All broad
native-world, rendering, deterministic-video and full-parity gates remain false.
No encoder, game, helper, build, test, UI automation or project-code execution
was run; only source and SDK documentation were read and source files edited.

### Emote reference before the skin draw (2026-10-27, static source only)

The native palette path previously acquired its emote reference only when the
standard skin reached dataDrivenDraw. Equipment submitted earlier in the same
root model could therefore miss the current target. A new native_skin_selection
reader copies the root renderer's already-selected controller/geometry pairs.
It requires the current draw's exact pair in that list and one live geometry
whose explicit name matches SerializedSkinImpl's standard geometry name. It
checks that geometry's initialized hierarchy index, six-part rest metadata,
completed matrices, frame, actor, context and model binding. A generic default
name, duplicate candidate, expired geometry or unreadable metadata supplies no
early reference. It does not search unrelated hierarchies or reuse an index
from an earlier frame.

Cached executable evidence for 1.26.5203.0: Model::draw loads the component's
pair-vector begin/end at RVAs 0x1c369da..0x1c369e8, advances by 0x28 at 0x1c36a00,
copies each controller at 0x1c36a32 and geometry weak pointer at 0x1c36a56, then
calls the controller at 0x1c36c8e. That controller selects the geometry's +0xe8
hierarchy at 0x1cb8734 and passes the same geometry as dataDrivenDraw's fifth
argument at 0x1cb8b09..0x1cb8b1f. Installation now requires exact byte matches
at these additional layout sites. This is cached-byte analysis, not a running
game observation or proof of equipment rendering.

The reference owns its copied pose and bounded metadata guards inside the
current ActorScope. Each use rechecks selection metadata, native shared-pointer
liveness, the completed rig and frame. Native refcount increments may change a
positive count; expiration still invalidates the reference. A changed or
conflicting skin baseline clears the emote target for that actor scope. Cached
skin appearances also recheck geometry, persona and arm model before reuse.
No selection pointers are serialized, retained in sample history, or carried to
the next actor frame. No earlier upload destination is retained for later use.

This early reference can authorize the existing named, owned-upload emote path
for an earlier equipment draw whose native part topology is already bound. It
never fills Global::skin_pose: body, cape and elytra suppression still require
their actual draw receipts. It does not implement independent held-item model
ownership, unnamed/default geometry identification, or prove native armor,
multi-pass, GPU or replay parity. The experimental probe restriction and Java
equipment readiness gates remain in place, and all broad runtime verification
flags remain false. No build, test, game launch, game-process inspection or
project-code execution was performed; only source and cached bytes were read.

### Child equipment palette binding (2026-10-27, static source only)

Attachment rendering does not call the root model-render observer again. The
cached root renderer invokes the checked attachment wrapper at 0x1c38a10 for
the seven native slot indices; the accepted branch tail-calls 0x1c3ee90 at
0x1c38bb0. That path selects an 80-byte ChildAnimationComponentInfo from the
root component's +0x300 vector at 0x1c3f010..0x1c3f05a. Its getter at 0x1bc02e0
returns the child shared_ptr at entry+0x40. The attachment draws that child's
selected controllers using child+0x88 RenderParams, writes the copied slot index
at child+0x284, and calls the controller at 0x1c3f5f5. dataDrivenDraw consequently
has native return sites 0x1cb8b24 and 0x1c3f5fa. Additional exact-byte profile
checks cover these layout and call sites; no runtime observer was executed.

native_attachment_binding now handles this path separately when the strict root
model binding declines a draw. It requires the active world actor/root binding,
the exact native call chain, seven bounded child slots, unique ownership of the
selected child, live shared/weak references, matching child/root RenderParams,
an occupied item pointer, matching animation frames and the actual selected
controller/geometry pair in the child's list. It reads the child's hierarchy
through its own component rather than treating Actor+0x270 as that child's pose.
The intervening MolangVariableMap is mutable query state and is not mistaken for
stable slot identity. Slot keys and child shared-pointer fields are guarded.

The native view and projection matrices must match the root skin draw. The
model matrix must either match it or provide the validated coordinate change
described below. Every named rig anchor in the child hierarchy must match the
corresponding copied native skin matrix in that space, with at least one such
anchor. Identical spaces retain exact anchor equality. Its descendants are mapped
through the existing checked native part topology. This supplies a real named
palette to the existing owned-upload emote path, including its current/previous
source distinction and historical-pose requirements. No child is relabeled as
the player skin, and no body, cape or elytra suppression is requested for it.

The palette binding retains this child ownership/space receipt only during the
current dataDrivenDraw. Every later constructor-span attestation or upload-span
lookup rechecks it, resamples the root/child rigs, and rechecks palette metadata
afterwards. Expiration, slot replacement, matrix/frame changes, ambiguous slot
ownership or an unbound anchor leave the native upload unchanged. Diagnostics
retain only copied slot, UUID, geometry, hierarchy and frame values, with no
native addresses. Ordinary root-model acceptance remains unchanged.

Attachments without a checked local rig anchor or indexed root binding remain
unsupported by this path. Expression-selected held-item transforms still need
their actual native source established;
mapping CarriedItem/OffhandItem to a guessed hand is not sufficient. Child-cache
producer/consumer history has not been generalized from the existing root-only
cache proof, so an unavailable previous receipt still leaves the previous upload
native. Full attachment rendering, native equipment and deterministic video
verification flags remain false, and the Java export gate requiring empty
attachments for an emote remains in place. No build, test, Bedrock launch,
process inspection, UI automation or project-code execution was performed.

### Attachment draw-space conversion (2026-10-27, static source only)

The child-palette path now carries an owned coordinate change rather than
requiring the attachment's model matrix to equal the skin's model matrix.
Both are the observed native model matrices, mapping model pixels to the same
camera-relative world space. The native bone palette also uses pixel units,
as in the existing skin/owned-upload path. No actor bounding box, inferred
skin scale, screen projection or guessed hand side supplies the conversion.

For native skin matrix W and attachment matrix C, U = inverse(C) * W changes
root pixels into child pixels. A checked child rig anchor must agree with
U * R, where R is its copied root skin reference. A requested target T applies
to the child's existing palette matrix A as
U * T * inverse(R) * inverse(U) * A. This preserves the attachment's observed
local offset, scale and mirrored basis. Singular, badly conditioned,
non-affine, non-finite or excessively lossy conversions leave the upload native.
The conversion is an owned pair of matrices and carries no engine pointers.

native_emote_space inverts the three-dimensional affine basis with pivoting in
f64 and computes translation separately. World translations do not enter the
basis condition estimate. It validates the stored f32 forward/backward
conversions against both native model matrices and their mutual round trips.
Converted coefficients/translations remain bounded to the existing one-million
native-palette range. Reconstruction uses a 0.0001-world-unit translation bound;
rig-anchor and inverse-pair translations use a 0.002-pixel bound. Basis agreement
uses a 0.00001 absolute/relative bound. Identical model spaces keep exact anchor
matching and the existing retargeting arithmetic. An unchanged target preserves
the original matrix exactly, avoiding conversion drift for an unaffected limb.

Constructor-root attestation now uses the current checked child model matrix
when a child binding is active. It still compares the native constructor's
source and output, then attests both copied palette spans and their owner.
Named upload bindings include the owned coordinate change, so all later
before/after comparisons cover that change too. Current uploads and explicit
previous-current-fallback entries use it; a sealed historical entry keeps its
original supplied matrix and original emote clock. No current-frame conversion
is substituted for the history of an earlier frame.

This extends the source implementation for transformed attachments whose native
rig anchors are established. The indexed-parent extension below also supports
subtrees without local canonical names. Unobserved expression sources remain
unbound. Native child-cache
history, complete attachment coverage, GPU behavior and deterministic replay
video remain unverified; the isolated probe and export readiness restrictions
remain. No game, process, encoder, build, test, helper or project-code execution
was run. The work consists of source reads/edits and static mathematical review.

### Indexed native parent bindings (2026-10-27, static source only)

The child path now follows BoneBindingMethod::MatchingParentBoneName (2) using
its actual parent hierarchy/bone indices. The cached branch at 0x1c39f82 loads
the root animation component through child+0xa0, selects its hierarchy using
BoneOrientation+4, and selects the source element using BoneOrientation+0 with
the 0xe8 stride at 0x1c3a09c. It copies all sixteen matrix values starting at
0x1c3a0cc and sets the child's override-stack flag at +0xe6 at 0x1c3a17c. These
additional fixed sites are checked during profile installation. No mid-function
patch, expression evaluation or new engine call is needed to read this binding.

Animation sampling now copies the method and override flag as owned values,
without adding them to serialized pose/history data. A child's method-2 indices
are resolved in the root component's namespace, including when root and child
share the same numeric hierarchy key. Root and child frames must agree. Method-3
expression bindings explicitly remain unresolved: their matrix-copy branch at
0x1c3a53d does not promise to update those indices, so stale values are not
treated as ancestry. The sampler continues to reject invalid indices and cycles
and preserves unresolved constructor defaults rather than inventing parents.

For an indexed source, native_attachment_binding requires the native override
flag, full child/source matrix agreement and draw-space agreement. It then finds
one live, selected root geometry for that source hierarchy. Repeated controller
layers referencing the same geometry are allowed; different geometry trees with
the same hierarchy are ambiguous. That root hierarchy must have the checked
standard rest rig and match the current skin reference. A fresh guarded part-tree
reader resolves every root part's nearest canonical rig ancestor, including
helper bones that have no renderable mesh or palette owner. The source is chosen
by its explicit index; a helper's text name or a coincident standing matrix does
not select a hand.

The resulting root role seeds the child's bound bone and follows the child's
actual part edges to its descendants. This permits, for example, a child root
bound by index to a player item helper to inherit that helper's checked arm role
without requiring a bone named rightarm inside the item model. Explicit native
bindings take precedence over a local canonical-looking name. Unproved indexed
sources and unproved expression bindings form barriers: their subtrees cannot inherit an
unrelated local parent role. A separately proved indexed override can establish
its own role beyond a barrier. Root helper bones with an unproved explicit source
also stop role inheritance. Unbound palette entries remain native.

Each accepted source hierarchy is copied and rechecked along with its guarded
root geometry topology for every later palette receipt. Slot metadata and draw
space guards still apply. At most sixteen source rigs are retained within the
current draw, with bounded geometry/part counts and bounded foreign reads; none
survive the draw scope. Diagnostics retain only child/source indices and rig
roles. The shared owned-upload path applies the same clock and conversion rules
as before. Child cache provenance, dynamic expression-selected parents, differing
unproved root rigs, full held-item coverage and broad runtime parity remain
incomplete or unverified. No builds, tests, Bedrock launches, process inspection,
UI automation or project-code execution occurred; work used source and cached
executable-byte analysis only.

### Constant expression parent bindings (2026-10-27, static source only)

The expression branch now accepts an inspected ConstantExpression containing a
stored HashType64 value. native_molang_constant checks the exact pinned vtable
at 0xe7d6860. Its evalGeneric entry at 0xa4690 is just lea rax,[rcx+8]; ret, so
the existing native evaluation returns the stored MolangScriptArg without any
query or script call. The clone at 0x1ce7a60 allocates 0x68 bytes, initializes
that vtable, copies the argument type at +8 and its POD value at +0x10. Those
layout/code sites and all sixteen vtable entries are checked at installation.
The reader uses the stored type/hash directly; it does not invoke an evaluator,
clone, optional-value getter or any other engine method.

The native parent expression branch at 0x1c3a1c0 calls the root component's
getSkeletalHierarchiesToProcess and uses its first index. The inspected getter
at 0x185cd0 returns the vector at component+0x398/+0x3a0. The reader checks the
actual root vtable's getter entry and reads that bounded vector rather than
making the virtual call. Within the selected hierarchy it follows the native
first-hash-match order at 0x1c3a380. Bone name hashes are copied as owned u64
values and are excluded from serialized pose/history data. No hash algorithm,
string-to-hand alias, hierarchy-order guess or matrix search supplies a source.

A bounded reader captures the child's hierarchy-map membership and vector,
then the expression-owning bone's bytes and constant object's type/hash. It
requires the native expression variant, method 3, override-stack flag and the
same completed child matrix already sampled for that bone. The selected root
source matrix must still exactly match the copied child matrix and pass the
existing coordinate-space checks. Its checked root geometry/rig ancestry then
supplies the role, just as for indexed bindings. Parent-link diagnostics identify
method 2 versus method 3. Constant pointers and bone storage addresses remain
private metadata borrows within the current draw; every later palette receipt
rechecks those bytes, source hierarchies, native frame and ownership.

Dynamic, source-tree and compiled expression objects remain barriers, as do
float variants, unknown vtables, missing hashes and unproved source rigs. The
general animation sampler still reports expression ancestry as unresolved; it
does not turn old parent indices into evidence. The isolated probe restriction,
child-cache provenance requirements and broad false verification flags remain.
This is source support for literal-hash parent selection, not full expression,
held-item, GPU or replay parity. No build, test, game, encoder, helper, process
inspection or project-code execution was run. Work used source/SDK declarations
and cached executable-byte analysis only.

### Observed dynamic expression parents (2026-10-27, static source only)

native_expression_binding adds scoped observers for the existing native setup
at 0x1c38c50 and the evaluator dispatch at 0x7c9e00. Both originals are forwarded
once; no query is evaluated again and no evaluator result is replaced. The setup
observer preserves the seven-argument ABI. The evaluator dispatcher is shared,
so its leaf bridge first checks the original native return address. Only return
site 0x1c3a2d9 uses the typed two-argument observer. Other virtual calls tail-jump
directly to the original trampoline, retaining their integer/vector arguments,
stack slots and native return type. At the pinned binding call, the bridge also
copies the already-selected root hierarchy node from RBX and child bone from
RSI into otherwise unused argument registers. It does not patch the inlined
matrix-copy body. The bridge has no stack frame; its typed Rust callback uses
the ordinary Windows ABI/unwind convention, consistent with Microsoft's
[x64 exception-handling documentation](https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64?view=msvc-170).

Capture requires the current world ActorScope, checked root/child RenderParams,
unique native child slot, matching owning expression node, the actual root node
in its native hierarchy map, live weak/shared owners and a successful-resource
branch with a nonempty native geometry range. The source hash is accepted only
when the existing evaluator returned the child's known heap scratch argument
at component+0x210 with HashType64. Unknown return storage might be temporary
and is not retained. The source index follows native first-hash-match order
inside the root node selected before evaluation; neither the child's name nor
an equal standing matrix selects a parent. The copied source hashes, matrix,
owners and evaluator-result bytes are guarded until native setup returns.

Setup completion seals a source only when those guards still agree and the
native child has the expected method/override flags and all sixteen copied
matrix values. Pending heap borrows then disappear. ActorScope retains only
owned source values/indices and opaque identity keys, bounded to 512 sources.
At most eight nested setup scopes and two MiB of pending metadata per scope are
accepted. Each setup invalidates the slot's previous selection before executing;
unidentified or nested rendering also invalidates potentially shared outer
selections. New serials prevent an old selection from surviving a later setup
whose matrices happen to be identical. Later palette checks compare the sealed
selection again, including child/model identity, expression identity and serial.
Each setup can complete only once. Lookups require one completed owner and one
matching source from that owner's exact setup serial; ambiguous owners or
duplicate selections are rejected.
Nothing is serialized into persistent history and no native stack pointer is
retained beyond its call.

The completion receipt also records the actual native child frame and reload
marker. The cached setup tail at 0x1c3a7ec..0x1c3a833 can deliberately store an
invalidated frame value after updating the bones. A different child counter is
therefore accepted only with a completed setup in this same root ActorScope;
the sampler checks the root frame and the separately observed child frame.
It never substitutes a guessed root-minus-one value. Reload changes and later
setup invalidation revoke that receipt. The strict equal-frame path remains
available independently. All upload/history clocks still use the checked root
render time, not an invented time derived from the child counter.

The attachment reader prefers these observed outcomes, then its separately
checked constant-hash path. Existing source-rig/topology, frame, model-space,
constructor-span, owned-output and previous-pose rules still apply. The observer
installs only for the existing explicit isolated body probe. Native parent
transforms after the copied matrix, evaluator results in other storage, full
multi-pass/child-cache coverage and persistent replay of expression choices
remain incomplete. All broad rendering/equipment/deterministic-video flags
remain false. No build, test, Bedrock launch, process inspection, encoder,
helper or project-code execution was performed; only source and cached bytes
were inspected and source files edited.

### Child animation-cache ownership (2026-10-27, static source only)

The cache input reader now accepts a proved attachment component in addition to
the strict root component. Offline decoding of the cached controller function
at 0x1cb7f00 identifies the part preparer wrapper call at 0x1cb88cf, the draw at
0x1cb8b1f and the cache-producer traversal at 0x1cb8eb0. The wrapper calls the
existing six-argument preparer at 0x1c20ec6. The later traversal at 0x1c30260
passes the same model's RenderParams and copied bone span to the existing
producer at 0x1c302d6; recursive children preserve them at 0x1c21cc7. This work
adds no new native hook or call and changes none of those arguments or results.

native_cache_binding copies bounded heap ownership metadata. A root continues
to require model_binding's full root-world checks. A child must occupy exactly
one of the root's seven shared-pointer slots, with live owners, the matching
slot field, child/root/context/actor RenderParams identities, a live model and
an item parameter. The geometry must actually occur in that component's
selected controller/geometry list with live owners. Its hierarchy index selects
that component's own checked hierarchy vector. The cache caller's supplied span
must match that vector's exact count and pointer, rather than supplying an
unrelated vector with coincident matrix values. The shared hierarchy-map walker
checks bounded membership, links and unique hierarchy indices for both readers.

Before entering dataDrivenDraw, the palette binding can retain the same checked
owner for the duration of that draw. Named current uploads carry a private
component/model/geometry/hierarchy/frame/reload/slot identity. FrameUploads now
matches this owner as well as part identity, native key and matrix. Reusing a
geometry part across root and child components cannot pair the other model's
supplied pose with a later native cache write. Observed attachment setup serials
are included and rechecked even when native matrices/counters remain unchanged.
The separately observed child-frame lease is required for a different native
child counter, as in the attachment draw path. Caller stack span addresses stay
inside synchronous prepare/produce scopes; the draw owner retains heap metadata
only, and ActorScope upload entries retain scalar comparison keys and owned poses.

The existing preparer observer now freezes against that exact validated
RenderParams owner, not an assumed Actor+0x270 root. Both its historical branch
and producer completion revalidate the owner. Existing lookup order, predicate
and writer return sites, guarded cache-node witnesses, write reservations and
generation/overlap rules remain required. Historical previous entries retain
their original supplied local matrix and clock; current child draw-space
conversion is not applied to a historical matrix again.

Persistent history deliberately keeps the real native actor/key/group/palette
namespace. It does not invent separate native caches for each child. A later
root or child write to the same native key replaces the earlier supplied-pose
eligibility through the existing reservation serials, including a supported
write without a paired current upload. Unidentifiable writes still invalidate
that owner or the global generation. The new component identity is current-scope
upload provenance and is never serialized or retained in persistent history.

This extends the source path for child cache provenance; it does not establish
all attachment geometry/binding coverage or rendered previous-pose behavior.
Unproved child ancestry and unsupported expression result storage still leave
entries native. All broad equipment/rendering/deterministic-video verification
flags remain false, and the Java emote-export gate requiring known-empty
attachments remains. No build, test, Bedrock launch, process inspection, encoder,
UI automation or project helper/code execution occurred. Work used source edits
and offline disassembly of the previously cached executable bytes.

### Subchunk insertion work during indexed capture (2026-10-27, static source only)

native_world_work adds a read-only source for pending subchunk changes, queued
subchunk inserts and TaskGroup's in-flight count. The pinned ClientLevel table
at 0xe7adab0 contains subChunkTickAndSendRequests at 0x12f4940 and the existing
client-tick hook at 0x12f4b70. The inspected call at 0x12f4ad3 loads the object
from level+0x7b8, adds 0x48 and calls 0x6177840. That routine obtains its queued
count from the first vector using a 0x20-byte stride, loads its task count at
insert-manager+0xe0 under the native mutex, and checks leaving-game at +0x178.
The inlined ClientLevel constructor initializes the matching SubChunkManager
vtable 0xe7ae8b8, pending-change map and insert-manager member at +0x48; the
insert-manager constructor at 0x6177160 builds its TaskGroup at +0x18. TaskGroup
construction at 0x6b4080 installs vtable 0xe76b260 and initializes the task count
at group+0xc8. The direct connector getter at 0x6410f0 returns manager+0x370.
These code sites and table entries are checked only after the existing full
image fingerprint. This adds no native detour, getter call or scheduler call.

The reader requires the live primary client's player registry, session epoch,
completed client tick, exact level/manager/task-group vtables and stable owned
manager identities. It copies bounded map/vector headers and task metadata,
checks empty-map links, vector size/capacity/stride and finite count bounds,
rejects unknown task states, and rechecks the reads and active client. No queue
payload, arbitrary task object or guessed ownership-control layout is read.
At most 32 metadata reads / 2 KiB are retained per endpoint. All native addresses
remain private to the world callback; published values contain counts and
world/session/tick identities only.

During indexed capture, the existing world observer samples this reader before
and after the original world draw. Only equal, still-owned endpoint observations produce worldWork in
that pass's renderer timing. live_frame copies the recorded result rather than
querying current work when older pixels are later presented. The ordinary Java
capture metadata bridge carries it to the host/native capture matchers. A
requireSubchunkIdle control condition requires the same session, tick and exact
world sequence, matching endpoints, zero pending changes/queued/in-flight work,
a Running task group and a false leaving flag. NativeExportClock requests this
condition before every indexed export frame, and native admission/readback checks
it again for each actual tagged frame. Basic capture clients omit the condition
unless requested. A missing/changed/busy observation waits and eventually fails
the existing bounded capture wait; it never admits a frame based on elapsed
settling frames alone or advances extra simulation ticks to clear the queue.

Matching endpoint snapshots do not detect a task which begins and ends between
the reads. The reader does not establish full LevelChunk decoding/insertion,
network callbacks, lighting, visibility/geometry builds, terrain upload or GPU
completion. Task generations and the other queues still need to be covered for
complete world readiness. Its field is deliberately observedIdle, not a terrain
or packet-application verification flag. Those flags and deterministic-video
verification remain false; this is an additional observed-work capture condition,
not proof of deterministic replay or Java parity. No build, test, game, encoder,
process inspection, UI automation or project helper/code execution occurred.
Only source/SDK reads, cached executable-byte disassembly and source edits were
performed.

### Task activity between capture endpoints (2026-10-27, static source only)

The insertion-work reader now includes an activity witness from
native_task_activity. IBackgroundTaskOwner's declared virtual order and the
cached TaskGroup table at 0xe76b260 identify taskRegister at slot +0x18 / RVA
0x6b73d0 and taskComplete at slot +0x38 / RVA 0x6b7120. Independent decoding
finds the native task count increment at 0x6b74c5 and decrements at 0x6b728c /
0x6b72e3, all at TaskGroup+0xc8 under its native mutex. The observers forward
the existing two-argument calls once. taskRegister's shared_ptr value is passed
indirectly on Windows x64; Rust forwards that native reference without copying,
retaining or destroying it. taskComplete receives the native non-null task
pointer directly. Neither observer evaluates a task or changes native work.

The same activity scope wraps TaskGroup construction at 0x6b4080 and the common
destructor body at 0x6b4320. The constructor preserves all four arguments,
including the native by-value string's storage, and its observed this-return.
The deleting destructor at 0x6ba2c0 calls that same body at 0x6ba2cb, so embedded
and virtual deletion share the observed destruction path. Construction and
destruction update the record even if an allocator later reuses the group
address with identical vtable, queue headers and zero task count. Function
prologues, slots, counter stores, this-return and deleting-wrapper call are
checked against the pinned image before the activity gate becomes available.

A fixed 256-entry process-local ledger stores opaque group comparison keys,
monotonic activity revisions and active callback counts. Hot observer bookkeeping
does not allocate or retain native task/string/shared-pointer storage. Its mutex
is released before native code or native memory reads. Snapshot access uses a
nonblocking lock and rejects concurrent bookkeeping or active callbacks for
that group. Saturated tracking with no inactive eviction candidate keeps an
unknown-callback guard through the original and changes a global invalidation
generation. Eviction uses a new monotonic revision on reuse and cannot remove
active records. Poisoned state, arithmetic exhaustion or unmatched completion
disables witness admission rather than treating lost tracking as idle.

The work reader acquires its witness before copying queue/count fields and
rechecks it around later metadata and live-client checks. Both ends of a world
draw must have the same quiescent witness as well as equal native snapshots.
An observed task registration and completion between those reads now changes
the revision even when the final native count is zero. Published worldWork adds
taskActivityObserved and its revision/invalidation generation without native
addresses. Both host and native requireSubchunkIdle matchers require these
fields along with their existing session/tick/world-sequence and idle-count
checks. Per-PID engine diagnostics expose bounded tracking state and retain
taskLifecycleVerified=false and terrainReady=false.

This covers the observed callback/lifetime paths. It does not prove that every
native writer uses them, that a callback already executing before hook
installation was observed, or that queued changes and other native work cannot
change through unobserved paths. Full bootstrap, packet application, lighting,
visibility/geometry work, terrain uploads and repeatable rendered output still
need evidence. Broad readiness/parity flags remain false. No build, test,
Bedrock launch, process inspection, encoder, UI automation or project helper/code
execution occurred; work used source/SDK reads, cached executable-byte analysis
and source edits only.

### Owned network chunk I/O during capture (2026-10-27, static source only)

native_network_work adds pending-chunk and paused-callback map counts plus the
current network handler's I/O task count/activity to the world-work sample.
The cached client GameSession constructor at 0x26b26b0 stores discriminator 0
and the moved callback at session+0x50. The inspected getNetEventCallback branch
at 0x26b2b62 uses that same owned callback when session+0x48 has no server handler,
and copies its alive flag/shared owner at callback+8/+0x10. No getter is called
by Arctic. The final LegacyClientNetworkHandler table at 0xe7af110 is installed
at 0x132c6ad, following its base construction. The constructor store at
0x132bee1..0x132bee5 binds callback+0x58 to its IClientInstance. The table's
0x131f770 handler independently checks the alive flag through callback+0xb0
and obtains the level at +0xc0. These sites and table entries are pinned before
the new reader becomes available.

The reader starts with the current primary client for the selected live level,
then guards client+0x1b0 -> Minecraft+0xc8 -> GameSession, the client discriminator,
active level ownership, absence of the server callback and the owned legacy
callback. It requires the exact final callback vtable, matching primary-client
back-reference and live non-owner level reference. Shared owner controls must
stay live; their changing reference counts are not mistaken for stable metadata.
The existing primary-client helper now exposes this checked identity separately
from its live local-player-registry lookup.

The native constructor creates pending-chunk and paused-callback unordered maps
at callback+0xe8 and +0x128, with list heads at +0xf0 and +0x130. The reader copies
their bounded headers/counts and requires self-linked sentinel heads for zero
counts. Constructor code at 0x132c480..0x132c5f8 assigns the unique I/O TaskGroup
at callback+0x168 using the literal ClientNetworkHandler IO Group. The inspected
factory at 0xcf8c0 calls the same TaskGroup constructor used by the activity
observer. The reader requires its exact task-group vtable and name, acquires its
activity witness before copying pending work, then reads count at group+0xc8
and state at +0xb0. Unknown owners, names, states, counts or stale activity leave
the network sample unavailable; the legacy insertion queue is not guessed from
these counts. No packet/callback/task is run, drained or written by this reader.

Network samples are retained only alongside the existing before/after subchunk
samples inside one original world callback. Their owned-client identities,
guarded metadata, session/epoch/tick and I/O activity must still agree. A changed
or newly available/unavailable network sample rejects that pass's paired receipt.
An unsupported network path can remain null for manual subchunk-only capture.
Published networkChunkIO contains counts, state and activity revision/generation
only. Native/host requireNetworkIdle independently checks this data against the
same session, tick and exact world sequence already used by requireSubchunkIdle.
The indexed export clock requests both conditions for every captured frame;
basic capture defaults both optional flags off. Missing or busy data fails the
existing bounded wait rather than falling back to old work observations.

Each network endpoint uses the bounded metadata copier (32 reads / 2 KiB of
copied bytes and at most eight checked shared controls). Native addresses stay
private to that callback and are not archived. This extends observed-work
coverage, not full packet application or terrain readiness. Legacy insertion,
unobserved map writers/task paths, bootstrap, lighting, visibility/geometry work,
terrain uploads and repeatable rendered video remain incomplete or unverified.
All broad verification flags remain false. No build, test, Bedrock launch,
process inspection, encoder, UI automation or project helper/code execution
occurred. Work used source/SDK reads, file-backed inspection of cached executable
bytes and source edits only.

### Legacy network insertion accounting (2026-10-27, static source only)

The network work sample now also covers the legacy handler's owned
NetworkChunkInserter. Cached constructor code at 0x132c771..0x132c7ad allocates
0x30 bytes, stores the handler back-reference at queue+0, zeroes both sequence
counters at +8/+0x10, initializes the completed-chunk vector at +0x18/+0x20/+0x28
and assigns the unique pointer to handler+0x298. The destructor at 0x132d70a
destroys that vector through 0x136e0c0 and frees the queue. Its element stride is
0x18, with an owned chunk at element+8; Arctic does not read or retain chunks.

The LevelChunkPacket handler at final legacy vslot 0x5a8 -> 0x1346770 increments
queue+8 at 0x13471aa and gives the old sequence to the decode request. The
completed-chunk callback at 0x1357450 obtains the same owned queue at 0x13574ac,
appends sequence/chunk metadata at 0x13575d4 and orders ready chunks by sequence.
Its consume loop at 0x1357ab0 requires the first ready sequence to match
queue+0x10. Removing a ready entry updates vector end at 0x1357d91; completing
that consume step increments +0x10 at 0x1358130. Native discard paths also reach
that increment. This counter therefore means consumed, not successfully applied.
These constructor, ownership, request, vector and consumer sites are pinned
before enabling the reader; no callback or drain routine is invoked by Arctic.

Each network endpoint guards handler+0x298 and the complete 48-byte queue header,
requires the owner back-reference to match, bounds vector size/capacity and checks
24-byte element alignment. Consumed cannot exceed issued, outstanding work is
bounded, and the ready count cannot exceed outstanding work. Null vectors must
have all three pointers null. The same header and monotonic sequence values must
remain unchanged across the existing paired world-pass sample. This uses two
additional guarded reads within the existing 32-read/2-KiB endpoint budget.
There are no new native hooks or retained chunk pointers.

networkChunkIO now includes legacyInsertion with observed, issuedChunks,
consumedChunks, outstandingChunks, readyChunks and observedIdle. Its outer idle
value also requires issued == consumed and an empty ready vector. Native and
host requireNetworkIdle independently require the new observation, both zero
counts and the same issued/consumed values; missing older receipts cannot pass.
The indexed exporter already requests this condition on every sample. An empty
ready vector cannot now hide an issued decode request that has not completed.

This is static source coverage of observed queue accounting. It cannot prove
successful chunk application, full task/bootstrap coverage, lighting, visibility,
geometry, terrain upload or repeatable rendered video. Old early-return paths
that leave a sequence unconsumed can cause a bounded capture timeout; Arctic does
not skip, increment or repair native work. Full parity and broad verification
flags remain incomplete/false. No builds, checks, tests, Bedrock launches,
process access, UI automation or project code execution were performed. Work
used read-only cached-file disassembly and source reads/edits only.

### Renderer-owned task work during capture (2026-10-27, static source only)

native_render_work observes both TaskGroups uniquely owned by the actual
primary LevelRenderer. The cached constructor at 0x47e5420 installs its final
tables at 0xe8e3bf0 and 0xe8e3cf0. It constructs group+0x3d8 through factory
0xcf780 at 0x47e6345 with the literal A LevelRenderer group. The second factory
call at 0x47e6376 uses renderer+0x3e0, the main-thread scheduler and the literal
Main thread tasks group. Constructor stores at 0x47e6389/0x47e6390/0x47e63d8
bind game-renderer, level and client references at +0x3e8/+0x3f0/+0x408. The
destructor at 0x47efc60 destroys both owned groups. These independently decoded
sites and table entries are pinned before enabling the reader.

The visibility rebuild routine at 0x1c9ef80 follows coordinator+0x60 to this
LevelRenderer and obtains its background group at 0x1c9f4ae. This establishes a
renderer-work path beyond network and subchunk insertion. It does not establish
that every visibility, geometry, light or upload task uses these two groups.

The new endpoint reader requires both exact renderer tables, matching
client+0x1c0 -> renderer and renderer+0x408/+0x3f0 -> client/active level, distinct
owned groups, their exact TaskGroup tables and names, bounded native task counts
and recognized group states. Each group uses the existing observed-activity and
construction/destruction witness. Guarded metadata and both quiescent witnesses
must remain live with the same client/session/epoch/tick. Only bounded metadata
is copied; no native task, resource, renderer or scheduler is called or retained.

World-work sampling now receives the actual renderer argument at both ends of
the original world callback and guards the primary client's matching renderer
link even when the optional renderer-task reader is unavailable. Equal paired
samples publish rendererTasks with background/mainThread counts, states and
activity revisions, observedIdle and terrainReady=false. They remain part of
that exact world pass's capture controls rather than a later presentation-time
resample. No native address is published or archived.

Both native and host capture matchers implement requireRendererIdle, default
false for manual capture. Indexed replay export requests it alongside
requireNetworkIdle and requireSubchunkIdle for every sample. Both groups must
have zero tasks, running state and unchanged quiescent observed activity across
the draw. Missing, stale or busy observations fail the existing bounded wait;
they cannot reuse an older pass or silently omit the new condition.

This adds static source coverage, not verified renderer convergence. Unobserved
task writers, bootstrap, dirty/visible chunk state, geometry, lighting and terrain
GPU completion remain incomplete or unverified. Full cosmetics/world parity and
deterministic-video flags remain false. No build, check, test, Bedrock launch,
process access, UI automation or project code execution occurred; work used
public declarations, read-only cached executable disassembly and source edits.

### Active-dimension pending chunk changes (2026-10-27, static source only)

native_chunk_work adds the current camera dimension's pending visibility and
chunk-change maps to rendererTasks.chunkChanges. The cached LevelRenderer
coordinator lookup/factory at 0x47d8360 uses the map at renderer+0x28, with a
DimensionType key at node+0x10 and object/control at node+0x18/+0x20. It creates
an in-place coordinator at control+0x10, sets the owner table at 0xe6daf00 and
calls the constructor at 0x1c9a8b0. That constructor assigns coordinator table
0xe7d5b80, level/renderer references at +0x58/+0x60, dimension ID at +0x80 and
pending maps at +0x88/+0xc8/+0x108. Constructor/helper stores independently
establish each map's head at +8 and count at +0x10. These facts come from cached
code, not assumed SDK field order.

The reader selects the active dimension through the renderer's owned player
camera at renderer+0x468/+0x470. Its factory places the camera at control+0x10
and sets owner table 0xe8e4260; the final camera table is 0xe8de6b0. Base
construction at 0x466cb80 assigns camera level at +0x978, dimension at +0x990
and renderer at +0x9a0. Camera lookup code at 0x46716b0 calls the dimension's
virtual slot +0x10 before selecting a coordinator. The inspected method at
0x230e560 merely copies dimension+0x1a0 into its out-value. Arctic requires that
exact method in the actual slot, copies the same field and never calls it.
Factories, stores, lookup, getter bytes and camera/coordinator table entries are
pinned before the new read-only module becomes available.

Each endpoint independently requires matching primary client, renderer and
level links; exact object/control tables and factory object offsets; positive
shared controls; and matching camera/coordinator dimension IDs. It walks the
renderer-owned map's bounded list, checks every previous link, unique node and
dimension key, exact count and final sentinel/tail link, then selects the unique
matching dimension entry. It supports one to eight owned coordinators within
the existing 32-read/2-KiB copier budget (at most 30 guarded reads). It copies no
chunk payload, weak render-chunk map or notification vector contents. The three
pending-map counts are bounded and their empty sentinel links must be self-linked.
Unknown ownership, a missing dimension, larger maps, stale links or failed reads
leave the observation unavailable rather than supplying zero counts.

The new owned snapshot participates in both endpoints of the existing world-pass
pair, alongside renderer group activity/lifetime witnesses. Its metadata and
client/session/epoch/tick are rechecked before publishing scalar dimension/count
data. rendererTasks.observedIdle now also requires an available idle chunk-change
snapshot. Native and host requireRendererIdle independently require that nested
observation and all three pending counts to be zero. Missing older receipts fail
this condition. Indexed exports already request requireRendererIdle on every
frame; manual capture defaults it off. Nothing is drained or repaired, and no
additional simulation tick is advanced to make pending work disappear.

This extends observed pending-work coverage. Map counts do not prove all work
writers were observed, successful notification delivery, bootstrap convergence,
visible chunk state, geometry, lighting or GPU completion. Busy/deferred maps may
cause a bounded capture timeout; the source path has not been exercised. Full
parity, terrain readiness and deterministic-video verification remain false.
No build, check, test, Bedrock launch, process access, UI automation or project
code execution occurred. Work used public declarations, read-only cached-file
disassembly and source reads/edits only.

### Chunk-change consumption and geometry candidates (2026-10-27, static source only)

Cached consumer code at 0x1c9ce50 first checks pending visibility count at +0x98
and uses the notification maps at +0xc8/+0x108. It decrements visibility count
at 0x1c9d253. Consumed actor-notification keys decrement count +0xd8 and unlink
their list node at 0x1c9e354..0x1c9e36a; block-entity keys follow the corresponding
+0x118 decrement/unlink at 0x1c9e964..0x1c9e97a. The renderer routine at 0x47f4a70
iterates its owned coordinator list and calls this consumer at 0x47f4ae4. These
are pending maps whose processed keys are removed, rather than permanent counts
of cached chunks. The reader now pins the observed consumer entry, map selection
and decrement/unlink sites in addition to the constructor/factory evidence.

The counts describe map keys (subchunk positions), not the number of messages
stored in each notification vector. The chunkChanges receipt now names them
visibilityDirtyChunks, actorNotificationChunks and blockEntityNotificationChunks.
Native and host capture matchers both require those explicit zero-count fields;
older partial receipts cannot pass requireRendererIdle. No vector payload is
read or represented as an individual notification count. The consumer may defer
entries or return before delivery, so waiting for it remains part of capture and
does not constitute successful application or full terrain readiness.

Geometry-path investigation identified scheduleChunkBuild at 0x47d8820 from its
native signature literal and its renderer reference at builder+0x128. The
embedded builder begins at renderer+0x4a0. Two tempting count sources are
deliberately excluded: renderer+0x4a8 is assigned from +0x4a4 and the latter is
reset at 0x47f61d4..0x47f61e0, making +0x4a8 a previous-frame statistic; it is not
an outstanding-work counter. The routine at 0x47d23c0, called on renderer+0x4b8,
locks its object, trims a vector of reusable builders and unlocks it. It is not
a pending-work count or drain API. Neither source can satisfy capture readiness.
The remaining geometry port must establish actual build/sort lifecycle and
application state, together with visible geometry, lighting and GPU completion.

This work remains static and unverified at runtime. No build, check, test,
Bedrock launch, process access, UI automation or project code execution occurred.
Full parity, terrain readiness and deterministic-video verification remain false.

### Shared-chunk build and visibility state (2026-10-27, static source only)

native_shared_chunk_work observes the selected coordinator's weak RenderChunkShared
map at +0x18. The cached get-or-create routine at 0x1ca11a0 creates a 0xd0-byte
owner/object allocation, assigns control table 0xe77c3d0 at 0x1ca19c7, places the
object at control+0x10 and calls the shared-chunk constructor at 0x1ca6390. Its
weak assignment at 0x1ca1a2c..0x1ca1a4b increments the weak control and stores
object/control at node+0x20/+0x28, following the 12-byte SubChunkPos key. The
coordinator's own diagnostic traversal at 0x1ca02f5..0x1ca033a independently walks
that same map and checks strong liveness through node+0x28. These sites are
pinned before enabling the new reader.

The shared constructor stores minimum block position at object+0x14/+0x18/+0x1c
by shifting the supplied chunk coordinates left four bits. It initializes build
state at +4 to 1 and visibility state at +0x20 to 0. Constructor code at
0x1ca6417 atomically allocates an ID and stores it at object+0x80. The reader
checks each live object's position against its map key and guards that positive
constructor-assigned ID separately from reusable addresses and states. Build
start at 0x1ca7d92 changes +4 from 1 to 0. Public enum semantics and the native
visibility transitions at 0x1c9da18/0x1c9da31 distinguish Dirty=0, Building=1,
BuildingButDirty=2 and Ready=3. The snapshot counts build/sort state 0 and
visibility states 1/2 as active work. A merely dirty cached chunk is reported
separately, since visible membership has not been established.

The reader rechecks the coordinator table, level/renderer/dimension links and
complete weak-map header. It accepts at most 32,768 keys, traverses exact forward/
previous links and count, rejects duplicate position keys, and requires the final
head/tail links to agree. Empty weak pairs are counted explicitly. Nonempty
weak pairs require the exact shared owner table, object == control+0x10 and a
valid positive weak count. Live/expired classification is observed without
incrementing any native reference. Initially expired objects are not sampled for
payload state or IDs. Reference categories are rechecked before and after later
metadata validation; changing strong counts that remain positive are allowed.
This does not acquire a strong lease or prove complete lifetime/bootstrap coverage.

A separate packed metadata reader keeps the small network/coordinator copier's
budget unchanged. It allows at most four guarded reads per chunk plus eight
fixed reads, individual reads up to 64 bytes, and 112 bytes per maximum chunk
plus 512 bytes of packed data. Each observed live chunk contributes a map node,
control table, 36-byte state/position header and 8-byte native ID. Guards and
reference categories remain private to the current callback, are compared at
both world-pass endpoints and are discarded after producing scalar counts.
No native getter, constructor, build, sort, renderer or queue routine is invoked.
No chunk, geometry or GPU resource payload is copied or archived.

chunkChanges.sharedChunkWork reports key/live/expired/empty counts, active
build/sort and visibility-build counts, and dirty/ready visibility counts. Its
observedIdle requires zero active build/sort and visibility-build states.
requireRendererIdle now also requires this nested observation in both native
and host matchers. They independently check the count partitions and bounds,
including live + expired + empty == keys and, when no visibility build remains,
dirty + ready == live. Missing or older partial receipts cannot satisfy capture.
Unsupported ownership, a larger weak map, a changing ID/state/reference category
or an incomplete traversal leaves the observation unavailable and the indexed
export wait bounded; it never supplies synthetic idle counts.

This extends the static port beyond task/map emptiness to actual shared build
states. Visible geometry selection, unscheduled dirty visible chunks, all native
writers/bootstrap paths, lighting, GPU upload completion and repeatable rendered
video remain incomplete or unverified. visibleSetVerified, terrainReady and all
broad parity/deterministic-video verification flags remain false. No build,
check, test, Bedrock launch, process access, UI automation or project code
execution occurred; work used public declarations, cached executable disassembly
and source reads/edits only.

### Actual world-call frame input (2026-10-27, static source only)

native_frame_input observes the FrameRenderObject reference supplied in R8 to
the original world routine at 0x47f8570. Code at 0x47f85d4 saves that argument;
0x47f88f9 captures it alongside the render context. World-work sampling now takes
that same argument before and after forwarding the original once, with no ABI
change or additional engine call. The frame pointer remains a private endpoint
key; no asynchronous borrowed pointer or frame/allocator address is archived.

Cached frame destruction at 0x47aee60 independently establishes its shared
allocator prefix at +0/+8, view begin/end at +0x10/+0x18 and 0x3300 view stride.
Allocation at 0x4800fbb uses the same stride. The view move constructor at
0x48011e0 copies client ID at +0x8c and moves ChunkRenderObjectCollection from
view+0xe0, with shadows following at +0x5e0. Player view production at 0x46ac3a0
copies camera/target fields from renderer-camera+0x654..+0x668 into the view's
first six floats; its shadow flag is initialized false at 0x46b01d9. These sites
are pinned along with the constructor/queue evidence before enabling the reader.

The reader binds the actual primary client, renderer, active level and owned
player camera, checks their exact tables/back-references and positive camera/
allocator controls, then copies the view vector and bounded prefixes. It accepts
at most 16 active views, categorizes other-client views before client-0 shadow
views, and requires exactly one client-0 non-shadow view. The selected view's
camera position/target are compared bit-for-bit with the current renderer fields.
A mismatch is reported explicitly rather than presented as camera agreement.

Collection move code at 0x52eaf50 places the chunk queue at collection+0x18 and
51 layer-vector headers in +0x30..+0x4f8, followed by maximum vertex count at
+0x4f8. Chunk destruction at 0x52eaed0 advances by 0x48; layer append at
0x468f998 advances by 0x28. Each vector header is checked for canonical/aligned
addresses, ordered bounds, exact element stride and bounded count/capacity.
The initial reader copied the selected collection's header block and reported
chunk count, all 51 layer counts, their checked sum and maximum vertex count.
The entry reader described below extends that observation with borrowed records.

Each endpoint uses at most 64 guarded reads / 8 KiB of copied metadata, in
128-byte pieces, with at most four positive shared-control checks. Metadata and
owner controls are rechecked with the same client/session/epoch/tick. The
snapshot participates in the existing paired world-work observation, so its
frameInput receipt belongs to the captured world sequence rather than a later
presentation-time resample. Projection, resource-ownership, visible-set and
terrain verification flags remain false; entry observation is described below.

Both native and host ControlFence matchers add requireFrameInput, default false
and omitted when false in core-generated requests. Indexed replay export sets it
automatically with the three observed-work conditions. It requires the actual
argument observation, unique matching primary view, valid view-category counts,
matching finite camera/target arrays, bounded queue counts, 51 layer headers and
a matching layer-entry sum. Missing, ambiguous, changing or older partial frame
receipts fail the existing bounded wait. No frame input is synthesized or patched.

This adds static input/camera association. Equal metadata endpoints do not prove
that every input was consumed, all visible geometry is present, resource uploads
finished, projection is correct or GPU execution completed. Native world/cosmetics
parity and deterministic-video verification remain incomplete/false. No build,
check, test, Bedrock launch, process access, UI automation or project code
execution occurred; work used public declarations, cached-file disassembly and
source reads/edits only.

### Selected-view chunk and layer records (2026-10-27, static source only)

native_frame_chunks extends frameInput with the selected primary view's actual
ChunkRenderObjectCollection entries. Its layout is checked against public type
declarations and independently decoded cached producer/destructor instructions.
The 0x52eb319 stores place position at +0/+4/+8 and the native double ready-time
value at +0x10. Resource construction at 0x52eb43a and 0x52eb483 establishes
index object/control/selector at +0x18/+0x20/+0x28 and mesh object/control/selector
at +0x30/+0x38/+0x40. The destructor at 0x4703c80 independently tests both
selectors and releases the corresponding controls through count+8. These sites
are pinned before enabling this reader.

Selectors 0/1/2 mean monostate, shared ordinary resource and shared direct-chunk
resource. Monostate union bytes are uninitialized native storage and are never
interpreted as pointers. Shared default-empty and owned-null values are accepted;
unsupported non-owning aliases and unknown/valueless selectors are rejected.
Resource aliases may point away from control+0x10. Nonempty controls must have
positive strong and weak counts; unique controls are rechecked before and after
the guarded record reads. No reference count is modified, no strong lease is
acquired and no engine function is called. Resource availability metadata is
read as described below; resolved geometry/buffer payloads remain uninspected.
Native ready-time bits and signed positions are retained as private endpoint
keys; time must be finite but may be negative. No readiness/time control is
inferred from this field. One observed producer supplies the literal 2.0.

Layer production at 0x468f92f and its append at 0x468f981 establish the 0x28-byte
records: chunk index +0, material-pointer address +8, sorted index start/count
+0x10/+0x14, unsorted start/count +0x18/+0x1c, visible-camera mask +0x20,
fallback bool +0x24 and TerrainLayerLOD +0x25. Each chunk index must address the
copied chunk vector; both unsigned range additions must fit u32. The public LOD
enum has High=0, Low=1 and Count=2, so only 0/1 are supported. The collection's
three groups of 17 layer vectors are not equated to that two-value enum.
Material addresses are checked for canonicality but are not dereferenced or
treated as owned resources. A nonzero mask and either nonzero range count mark
a potentially drawable entry for diagnostics; they do not establish primary
camera visibility or the chosen fallback range. Missing mesh/material pointers
in such entries prevent required-frame-input capture. A separate index object
is not required because a mesh may supply its own index buffer.

Each endpoint accepts at most 32,768 chunks, 262,144 total layer records and
212,992 control guards, deduplicated within outer/client/material-reference
classes. Including direct/ordinary availability and material-reference metadata,
the packed guard store is capped at 18,121,984 copied bytes and 413,696 block
reads, each at most 16 KiB.
Active chunk batches contain
64 records and layer batches 128 records. Vector headers must match the parent
frame's counts and retain valid ordered/aligned bounds; reserved capacities
remain bounded separately. Larger, unsupported, unreadable or changing records
leave chunkPayload unavailable, rather than substituting empty queues. The
optional child participates in endpoint equality and liveness, nested under
the same client/session/epoch/tick and world sequence as its parent.

chunkPayload reports per-selector partitions, two LOD counts, layer counts,
shared-reference/unique-control counts, unique positions, referenced/unreferenced
chunks, nonzero masks, fallback flags and potentially drawable/missing-pointer
counts. Addresses and copied records are not serialized. Both capture matchers
now require this child when requireFrameInput is true, validate its bounds and
partitions against the parent, and reject missing potentially drawable mesh or
material pointers. Older header-only receipts cannot satisfy that condition.

queuePayloadObserved now means these records were copied and rechecked;
meshIndexOwnersObserved means their shared-control liveness was observed. Neither
means resources are retained, types are verified, buffer contents/bounds are
known, materials are owned, the visible set is complete or GPU uploads finished.
Those flags and all broad native-parity/deterministic-video flags remain false.
No build, syntax check, test, Bedrock launch, process access, UI automation or
project code execution occurred. Work used source reads/edits, public declarations
and disassembly of existing cached executable files only.

### Direct chunk-buffer availability (2026-10-27, static source only)

The chunk queue producer at 0x468fc20 independently establishes the direct
resource conditions before its call to addToChunkQueue at 0x468fe11. The vertex
variant-2 branch at 0x468fce7 requires a nonnull direct vertex object, its vertex
optional flag +0x68 equal to one, and, when split-stream bool +0x48 equals one,
its attribute optional flag +0x88 equal to one. The index variant-2 branch at
0x468fd99 requires a nonnull direct index object and nonzero optional flag +0x60.
Both byte sequences are pinned by native_frame_chunks::install. Public direct
resource declarations cross-check the field types; offsets come from these
installed producer instructions rather than inferred SDK layout alone.

The observer now follows only variant-2 nonnull objects whose enclosing shared
controls were checked. It reads a guarded 0x41-byte vertex metadata span starting
at +0x48, or one index availability byte at +0x60; bool representations outside
0/1 reject the child observation. The intermediate optional storage is copied
only as a private change guard. No ServerResourcePointer storage is interpreted,
no resolved resource/backend buffer is followed, and no native lease/refcount,
thread-local initialization or producer function is executed. Shared controls
and all metadata are rechecked by the existing endpoint liveness path.

Objects are deduplicated by resource class and address for bounded reads, while
empty/unavailable/available counts describe each chunk queue entry. A split
stream is available only with both vertex and attribute optionals engaged.
Index/vertex counts must partition their corresponding variant-2 totals. The
receipt also carries unique direct-resource and split-stream entry counts.
Potentially drawable layer entries with unavailable or empty selected direct
resources are counted separately. Both native and host requireFrameInput gates
require these counts, consistent partitions and zero such potential failures.
Older payload receipts without direct availability cannot satisfy that gate.

The separate direct-read allowance is at most one 65-byte span and one byte per
chunk, with at most two additional block reads per chunk. Together with bounded
queue batches, the direct-only allowance was 15,009,024 bytes / 69,632 reads;
the ordinary-resource extension below supplies the current combined budget. No
native address, optional storage or copied record is serialized. Limits do not
grow in response to an unsupported scene, and missing samples remain missing.

directAvailabilityObserved reports this producer predicate only. It does not
establish actual frame-graph consumption, resolved resource identity, buffer
bounds, lighting, uploads or GPU completion. Ordinary resource availability uses
a different producer path: the mesh branch calls 0x4b04330, which can accept a
CPU-data fallback as well as backend resource state, and the ordinary index
branch checks an internal handle flag. Those predicates are not replaced by a
direct optional-field check. MaterialPtr ownership is also still unported; a
nonnull layer material address is not evidence of a valid material resource.
All broad parity/deterministic-video verification flags remain false. No build,
test, syntax check, game launch, process access or project code execution was
performed; this work used public declarations and existing cached-file decoding.

### Ordinary chunk mesh/index availability (2026-10-27, static source only)

native_frame_ordinary ports the distinct variant-1 producer predicates. The
mesh queue branch calls the validator at 0x4b04330; it can accept native CPU
position/index data while a backend handle is unavailable. Requiring every
ordinary mesh to satisfy the direct-resource optional flags would change that
behavior. No native validator, getter, initializer or resource method is called.

The validator first reads its index discriminator at mesh+0x18 and its vertex
resource block at +0x1c0. A set tracking bit 0 at block+0x12 accepts that handle;
otherwise a nonempty position vector at mesh+0x60/+0x68 can supply the fallback.
Its position element stride is 12 bytes and the native count must fit u32.
The layout byte at mesh+0x25c is compared with one; only that equality requires
an available attribute block at +0x1d8. The public VertexLayout declaration is
opaque, so this byte is not assigned an invented bool/enum meaning. If the
index discriminator is nonzero, an available mesh index block at +8 or a
nonempty CPU index vector at +0xa8/+0xb0 is also required. A separate ordinary
override index object has a different queue branch at 0x468fd3c/0x468fd70:
its block+0x12 tracking bit must be set, with no CPU-vector fallback there.

The cached Mesh copy constructor at 0x4afd500 establishes vertex shared object/
control at +0x1c0/+0x1c8 and attribute object/control at +0x1d8/+0x1e0, including
native strong-count increments. Their common polymorphic ResourcePointer
header has table 0xe775460 and shared object/control fields at header+8/+0x10.
The table's destructor at 0x836b90 releases header+0x10 through strong count+8
and weak count+0xc. Constructor stores, copy operations, destructor and producer
predicates are pinned before enabling the reader. Header tables, canonical
addresses and positive strong/weak control counts are checked. Owned-null and
default-empty headers remain unavailable rather than fabricated valid buffers;
unsupported non-owning aliases reject the child observation.

Only the branches needed by that predicate are inspected. CPU vector headers
must have ordered bounds, correct stride/alignment and bounded u32 capacity;
their arrays are not read. Other tracking bits are preserved and its changing
validation counter at +0x10 is not copied or incremented. No native reference
count, ownership lease, TLS initialization or tracking state is changed. All
copied headers/flags and shared-control categories are rechecked by the same
world endpoint path. CPU fallback is reported separately from available backend
tracking state, and is never presented as completed upload.

Resources are deduplicated by class/address while receipts count queue entries.
ordinaryIndexAvailabilityCounts and ordinaryMeshAvailabilityCounts each partition
the corresponding variant-1 total into empty/unavailable/available. The receipt
also reports used CPU fallbacks, visited attribute-buffer conditions, unique
ordinary objects, embedded client-reference counts and unique client controls.
uniqueSharedOwners continues to count outer mesh/index owners only; embedded
client owners have their own count. Both capture matchers require this child
observation, consistent partitions/reference bounds and no potentially drawable
layer entries with unavailable selected ordinary resources. Older direct-only
receipts cannot satisfy requireFrameInput.

At most three embedded client references per ordinary mesh and one per ordinary
index object are followed. The mesh/index allowance has caps of 32,768 chunks,
262,144 layer entries, 196,608 control guards, 17,859,840 copied bytes and
397,312 guarded block reads before the material extension below. Those read/byte
caps allow the worst ordinary
fallback branches (ten blocks / 128 bytes per mesh, two blocks / 25 bytes per
index) or the smaller direct branches. Rechecks also read positive control
counts; these limits are not frame-time or performance guarantees. Native
addresses, headers and arrays are not archived.

This adds source coverage for both producer resource variants. It does not
prove material ownership, resource payload types/bounds, actual frame-graph
consumption, lighting, uploads, GPU completion or complete Java parity. All broad
native-world/deterministic-video flags remain false. No build, syntax check,
test, game launch, process access, UI automation or project execution occurred;
work used public declarations, source reads/edits and existing cached files.

### Actual layer material shared references (2026-10-27, static source only)

native_frame_material follows the MaterialPtr addresses actually supplied by
the selected view's terrain layers. Cached layer production looks up the
terrain-variation map by layer ID and LOD, takes its value at node+0x30 at
0x468f8c2, and stores that address after the chunk index at 0x468f92f/0x468f933.
The public TerrainMaterialVariationManager declaration cross-checks the map's
key/value types; no guessed global material, nearest draw or material-name
lookup is substituted for those actual frame references.

The camera constructor calls 0x52f1e00 for its named materials. That routine
zeroes a 16-byte MaterialPtr, obtains a returned shared pair through the native
group virtual slot, copies it into the destination and releases an old control
through pointer+8 / strong count+8 / weak count+0xc. These producer/constructor
sites are pinned before enabling the reader. The public MaterialPtr declaration
cross-checks its shared_ptr<RenderMaterialInfo> field. The observer copies only
that exact 16-byte pair, validates canonical nonnull pointers, and observes
positive strong/weak counts before and after guarded frame metadata. Aliasing
shared pointers are allowed without imposing an unproven control+0x10 layout.
Unsupported non-owning aliases reject the child; default-empty and owned-null
pairs are reported as unavailable material-info references.

Reads are deduplicated by MaterialPtr address, with a cap of 16,384 distinct
references per endpoint. Reference controls have a separate deduplication class
from outer mesh/index and embedded client controls. The guarded pairs and native
addresses remain private endpoint keys; only counts are serialized. The reader
originally did not dereference RenderMaterialInfo. The later backing-pointer
extension below adds one independently pinned field read. It still does not
invoke a lookup, acquire a native lease, modify ownership or follow shader/backend
resources. Equal pairs are not proof of resolved material state.

The receipt adds materialReferencesObserved/materialSharedOwnersObserved and
null-pointer, default-empty/owned-null/nonempty reference counts, shared-reference
counts, distinct MaterialPtr/info/control counts and missing potentially drawable
material-info counts. Both capture matchers require these observations, checked
partitions and zero missing material info among potentially drawable entries.
For example, a nonnull layer material pointer containing an empty shared pair
can no longer satisfy requireFrameInput. The existing materialOwnershipObserved
flag remains false because a live info reference does not establish resolved
native resource ownership, actual material consumption or shader/GPU readiness.

The extension adds at most 16 bytes and one guarded block read per distinct
material reference, plus its positive-control rechecks. Combined caps are
32,768 chunks, 262,144 layer entries, 16,384 material references, 212,992 control
guards, 18,121,984 copied bytes and 413,696 guarded block reads. Unsupported,
unreadable, oversized or changing observations remain absent rather than
synthesized. Existing world-work sampling is confined to indexed capture by
capture::frame_options; normal world draws do not start this traversal. These
caps are not a performance guarantee, and no timing claim has been made.

Actual material/shader binding, complete visible-set/resource/lighting coverage,
GPU completion, full native cosmetics parity and deterministic video remain
incomplete or unverified. All broad verification flags remain false. No build,
syntax check, test, game launch, process access, UI automation or project code
execution occurred; work used public declarations, source edits and existing
cached executable-file decoding only.

### Normal-session cosmetics activation (2026-10-27, static source only)

The previous activation path enabled native body/cape/emote work exclusively
from arctic-offline-probe.json. The existing rendering/retargeting code therefore
could not be requested in a normal client session. The source now exposes an
adapter-specific experimental Bedrock Cosmetics card in the shared Mods menu,
backed by ClientConfig.bedrockNativeCosmetics (default false). The card reuses
cape-motion and emote-wheel key/hold settings. Other adapters inherit a false
supportsNativeCosmeticsPreview capability, so the card is Bedrock-specific; this
capability exposes the setting and makes no runtime parity claim.

After each serialized Java UI frame, the JNI bridge queries
nativeCosmeticsRequested, applies native_body::set_user_enabled, and sends the
effective enabled state back through nativeCosmeticsMode before requesting
bodyPlan. Java readiness/emote-program publication requires that effective state
and the existing diagnostic body-control mode. The plan is refreshed while off
as well, to clear immutable emote programs and update bounded texture pins.
Already acknowledged engine-skin replacements keep their texture pinned during
a mode transition until the actual retained world frame no longer references it;
disabling does not remove pixels needed by an in-flight suppressed draw.

Native body policy now has independent probe and normal-user flags. An isolated
run ignores the normal-user flag and retains its explicit body/elytra probe
policy. Normal user opt-in also enables the existing owned-wing path. Turning
normal mode off immediately clears readiness; a failed Java frame clears normal
enable/readiness before reporting the error. Existing session, identity, loaded
texture, geometry, native pose, owned draw, actor scope and frame checks remain
in place. Enabling is a request for that checked path, not evidence that all its
bindings are present. No native clock/camera/flipbook probe flag is enabled by
this user preference, and no account or network policy is changed.

The exact-image attachment expression hooks now install even when the preference
starts disabled, so a later enable does not require reloading the native bundle
or leave the emote binding observer absent. Disabled setup forwards the original
once inside a None scope. That scope also quarantines nested evaluations if the
mode changes while the original executes; they cannot inherit an older outer
setup. The dispatcher keeps its existing ABI-preserving return-address gate.
Enabled setup still invalidates/seals the actual native binding evaluations and
never evaluates an expression twice. No new engine ABI, offset or hook target is
introduced by this activation wiring.

The shared emote-wheel input path already runs separately from the all-features
capability, so this change does not enable unrelated unfinished Bedrock gameplay
controls. Automatic third-person emote camera control still requires that broader
adapter capability and remains unavailable. Native/Java status reports expose
requested/user/probe/effective mode while all cosmetics/world/deterministic-video
verification flags remain false. Imported settings shares do not include the new
opt-in; existing configs keep its false default.

This is source activation and lifecycle work only. Full visible cosmetics,
equipment/emote/cape rendering, resolved materials, native GPU execution and
repeatable deterministic export remain incomplete or unverified. No compiler,
syntax check, test, helper, game process, UI automation or project code was run.
The installed native bundle and user config were not modified or launched.

### Recorded body references for replay (2026-10-27, static source only)

Appearance history now carries bedrock_body_samples_v1 alongside the cape-tick
and wing-sample tracks. BodySample owns six rigid row-major 3x4 matrices copied
from visible native-model-draw observations with all six parts available. These
are the original native reference matrices, before Arctic's emote posing; the
existing EmotePose validation checks its reference against the same player
matrices. No actor, model or native heap addresses enter the archive.

BodyHistory uses the cape writer's native elapsed/partial clock mapping, keeping
the fractional sample time rather than flooring every observation. A frame
contains time, completed tick, rate and up to 128 UUID/matrix entries. Blocks
flush at 64 frames or before exceeding 2 MiB, and use the existing serialized
compression, hashing, queue, asset and index budgets. Empty blocks remain gaps.
Changed observations at an identical recorded time fail recording rather than
silently choosing one pose. Times and ticks must remain ordered.

Mount validates every descriptor and compressed block before activation: bounded
counts and times, rigid matrices, unique actors, exact header/index agreement,
strict stream completion and no trailing data. Playback retains at most two
decoded blocks and reserves 16 MiB within the shared 64 MiB appearance budget.
An uncached read rechecks the source manifest and asset hashes. A sample can
interpolate only between adjacent observations of that actor with no missed
completed tick or long pause; crossing blocks requires exactly adjacent index
bounds. Actor disappearance, missing blocks and discontinuities never permit
holding or extrapolating the last pose. Exact observed endpoints are preserved.

Rendering substitutes the archived body parts for replay bodies and worn
cosmetics. Replay emote seeds carry a deep-copied immutable BodyHistory.Curve,
clipped to both the appearance event and the curve's whole-ms clock bounds,
with exact fractional membership checked again when the curve is sampled.
Curve retention is included in the bounded emote-program budget and changes the
program token when the source interval changes. The attached native callback
still validates its actual native baseline, token, world, actor, clock and
publisher lease. It samples only the published curve and animation, with no
archive access on the callback thread. Native retargeting retains the actual
native reference and uses the desired posed matrices computed from that recorded
baseline; the bridge ABI and hook targets are unchanged.

New archives with body data cannot publish fresh body/cape/wing replacement
readiness across a body sample gap. Already acknowledged buffered suppressed
draws retain their texture/body fallback. Capture readiness additionally requires
a recorded body sample for requested bodies, emotes, capes or worn cosmetics,
and a matching native emote receipt when an emote is requested. Legacy archives
without the track retain preview behavior but cannot satisfy that pose-dependent
capture check. Existing equipment and wing verification gates still apply.
Status exposes bodySamplesRecorded, lastBodySampleError, recordedBodyPrograms
and bodyInterpolationVerified=false; the native status report bounds the new
error string with the other appearance errors.

The curve specifies quaternion/translation interpolation of observed references.
It does not record dynamic Molang attachment choices, reproduce unobserved native
animation or establish complete root/world/equipment/material/lighting behavior.
Full cosmetics parity and deterministic rendered MP4 remain incomplete or
unverified. No build, parser, syntax check, test, helper, game process or UI was
run for this change. The installed native bundle was not changed.

### Fractional replay appearance samples (2026-10-27, static source only)

The indexed exporter already sampled protocol camera paths at rational times,
but the native dispatch marker and Java cosmetic clock discarded the remainder.
At 30 FPS, for example, the camera's 33 + 10/30 ms sample was paired with body,
wing, cape and emote math at 33 ms. The source now retains timeRemainder and
timeDenominator in PacketArm, the native marker request and every corresponding
receipt. Denominators are bounded to 1..1000, remainders must be smaller, and a
fraction past the 24-hour endpoint is invalid. Missing fields in legacy marker
input default to 0/1. Export feed compares the full pair to its FeedPoint before
queuing; endpoint waits also require that exact pair. Recorded packet selection
still uses the whole-ms floor, while the helper's existing cameraTime rational
fields continue to select the camera sample.

CosmeticClock now prepares an immutable Sample with the exact whole/remainder/
denominator identity after the existing session, dispatch, held tick, clock and
real-timer checks. Double conversion is confined to pose/animation math. Body
and wing curve lookup and cape-tick interpolation accept the fractional time;
appearance idle motion and replay emote phases use it as well. Appearance event
selection stays on ordered integer event keys. Curve program ranges retain
whole-ms bounds but exact sample membership is checked inside the immutable
callback, including the fractional portion before an interval's first integer.
An appearance sample beyond its end time is rejected even if its floor equals
the endpoint; export preflight compares the final frame's rational numerator to
that endpoint before freezing the world.

The emote math JNI method keeps its descriptor and 74-double shape, but the
first double now carries the actual sample clock, including a replay fraction.
Native Target retains sampleTimeMs separately from its whole timeMs identity;
WorldFrame validates their relationship and matches the exact double against
the current sample. This prevents a held emote with an unchanged seconds value
from accepting another fractional baseline within the same millisecond. Target
equality, actor-scope conflicts, current upload records and historical emote
palette records also retain this sample time. No foreign pointer or archive I/O
was added to the callback. The paired Java and native sources must be rebuilt
together; an older native callback rejects fractional first values.

CaptureFrame only adds cosmeticClockRemainder/cosmeticClockDenominator when it
actually includes the rendered world layer. Both native admission/readback and
host observation matchers compare that pair with the dispatch marker in the
same control fence. The protocol-frame exporter additionally checks the marker
pair against its FrameTime, so another valid rational point cannot satisfy the
nominal output index. Legacy 0/1 cosmetic receipts may omit both fields; a
fractional export cannot use that fallback. Frame manifests retain the resulting
control identities with their captured presentation as before.

This removes a source-level clock quantization gap. It does not prove packet
application, native animation interpolation, full equipment/material/lighting
coverage, exact cape fidelity or repeatable decoded video. Native/world/parity
and deterministic-video verification flags remain false. No build, parser,
syntax check, test, encoder, helper, game process or UI automation was run. The
installed game, bundle and user config were not changed.

### Native fallback skin archives (2026-10-27, static source only)

The live body/emote source can use an owned engine:skin image when no Arctic
skin is selected, but appearance snapshots previously recorded only the public
Looks skin reference. Replaying such an emote could therefore borrow the new
viewer's skin. NativeSkinCapture now attaches bedrockSkin actor state to both
single snapshots and appearance histories. A known state contains a texture
content hash and observed slim flag; absent native image/arm observations become
an explicit null. Only the prepared standard skin image already owned by the
Java engine-texture cache is read. This does not archive a native address,
texture handle, account envelope or authentication data.

Each take owns a bounded 128-entry image-to-asset memo. It compares both the
immutable engine key and the actual Java image object, keeping only a weak image
reference. A newly observed image is encoded as PNG and retained under its
computed SHA-1 texture key. The existing raw-file size, SHA-256 archive integrity,
asset count, queue, resident preparation and total archive budgets still apply.
Shared AppearanceAssets gains an overload for retaining adapter-owned bytes,
and Captured.withPlayerData can include those already retained references. The
history writer still verifies/copies the referenced files into its own spool;
missing/evicted assets fail the take instead of silently dropping a skin.
PNG encoding occurs once per newly observed image within the bounded memo, on
the serialized Java capture path, and no performance guarantee is made.

The replay loader strictly validates the two-field native skin descriptor and
decodes its texture through the existing bounded skin preparation path. Its
shared texture library deduplicates pixels across events and Arctic/native
references. Event change detection includes native skin content and arm-model
changes while preserving the existing stable emote-start key. Native fallback
states survive local actor remapping and snapshot re-saving through the existing
raw manifest/reference path.

Replay avatars and body readiness choose an explicit recorded Arctic skin first,
then the archived native skin. Both use the checked normal replacement path and
its acknowledged texture/slim receipt. A new explicit unknown cannot fall back
to the viewer's live engine skin for an emote, and pose-dependent indexed
appearance readiness rejects a missing effective recorded skin. A visible actor
absent from the selected appearance event also cannot pass that skin readiness
check. Legacy archives lacking bedrockSkin retain their prior preview fallback;
no historical data is invented for them. Status exposes nativeSkinStatesRecorded
and nativeSkinPixelsRecorded separately, so unknown observations are not counted
as archived pixels.

This preserves a missing input to replay bodies/emotes. Persona geometry,
animated native skin behavior, dynamic attachment-selection archives, complete
equipment/material/lighting rendering and repeatable deterministic video remain
unfinished or unverified. No build, parser, syntax check, test, encoder, helper,
game process or UI automation was run. Installed binaries and user config were
not changed. All broad parity and deterministic-video verification flags remain
false.

### Archived attachment palettes (2026-10-27, static source only)

The source now retains an owned attachment-palette sample after the existing
checked eight-matrix native copy helper. Named palette bindings expose their
guarded attachment slot and a deterministic skeleton/layout digest. The SHA-256
digest includes hierarchy index, bone names/hashes, binding methods, explicit
indexed parent links, rest transforms/pivots, part topology, owner/group/palette
membership and copied cache-slot hash/text. It excludes native addresses and
current animation matrices. This is a skeleton/layout identity, not a digest of
all mesh vertices, textures, resource packs or materials.

Space.root_matrix and Space.child_matrix convert owned affine matrices between
the checked root and child pixel spaces using the existing validated coordinate
change. Identity conversions preserve endpoint float bits. The new collector
only handles named child palettes; root body/cape palettes do not acquire an
attachment identity. A palette sample retains slot, geometry name, layout digest,
hierarchy/group/owner, ordered bone indices/names, resolved canonical rig anchors
and root-space values. Dynamic Molang choices reach the sample through the
already sealed native binding and final copied matrices. AST, child/model/part
pointers, cache-node identities and native reload addresses never enter it.

Each ActorScope owns a native_attachment_samples::Ledger, limited to 16 palettes,
128 bones and 256 KiB of retained reference/supplied values. A current copy with
an active emote opens a pending observation; only the checked committed upload
can supply its final posed values. Repeated native passes must agree on original
and final values. Changed keys/poses within that scope, mismatched animation
frames, unfinished copies, ambiguity, disabled observation and budget overflow
invalidate the collection. End-of-actor publication attaches the pointer-free
frame to the same actual standard skin draw, and live_frame publishes it only
with that matching six-part pose. No new hook address, native call signature or
borrowed pointer layout was introduced.

The frame's complete flag means that the collector's observed copies finished;
it does not enumerate every equipped/native draw. coverageVerified stays false,
including for a completed empty collection. Existing opaque current-upload/cache
records remain process-local; the archival values are a separate sanitized copy.
WorldFrame validates affine values, bounded text/counts/indices, unique palette
keys and same-draw provenance before recording.

AttachmentHistory writes bedrock_attachment_samples_v1 alongside cape, wing and
body tracks using the same native elapsed/partial clock mapping. Blocks hold at
most 64 frames and 2 MiB raw bytes, with at most 128 actors per frame; even one
oversized frame fails the take. Incomplete actors remain gaps, and all-empty
blocks are omitted. Changing pose data at an identical held time fails rather
than choosing an arbitrary render. Animation-frame counters are provenance,
so a counter increment with unchanged pose at that same time is not a change.
Compression, content hashes, spool/index/queue limits and archive integrity use
the existing AppearanceHistory path.

Mount validates every chunk before activation: exact descriptor/header agreement,
ordered finite clocks, strict compressed-stream termination, no trailing bytes,
unique actors/palettes, complete collections and bounded affine matrices. The
reader reserves 16 MiB within the shared appearance budget and retains at most
two decoded blocks; uncached reads recheck the mounted manifest and content
hashes. ReplayAppearance exposes sampled palettes through the existing local
actor-remapping rules. Missing actors, missed ticks, long pauses, chunk gaps and
layout/anchor changes cannot interpolate or extrapolate. Matching layouts use
sampled polar affine interpolation and preserve exact observed endpoints (see
the source follow-up below); this is not a claim that native animation uses
that interpolation. Status reports
attachmentSamplesRecorded and lastAttachmentSampleError with interpolation and
coverage verification both false.

The native playback consumer below now uses these archived palette values.
The existing emote capture gate requiring known-empty native attachments remains
unchanged, and no GPU/equipment/parity/deterministic-video verification flag was
enabled. Complete dynamic attachment fidelity, resource identity, all native
draws/materials/lighting and repeatable video remain unverified. No build, parser,
syntax check, test, encoder, helper, game process or UI automation ran; work was
limited to source reads and edits. Installed binaries and user config were not
changed.

### Post-export decoded video audit (source only)

The launcher export job now releases its owned native camera, appearance and
clock controls before decoding the completed video. A failed native cleanup
prevents that audit. The existing selected encoder executable also supplies the
decoder: Arctic's hidden --decode-video-frames mode for arctic_builtin, or FFmpeg
with passthrough frame timing and framehash v2 output. This path does not launch
the game or request game/account operations. It runs only when export is invoked;
no code was executed while implementing it.

The built-in decoder requests software NV12 output from Media Foundation. It
bounds geometry, buffers and frame count, removes stride padding, and supports
an integer even display aperture inside a larger coded canvas. Unsupported
fractional/odd apertures, pan-scan and ambiguous buffer layouts are errors rather
than implicit cropping or resampling. It emits actual decoded sample hashes,
timestamps and available durations through the same bounded parser as FFmpeg.

The audit requires the exact capture ID, frame index/count, original/padded
geometry, encoder name, RGBA input contract, requested color settings and rational
frame timing in the capture manifest. The capture worker closes its final video
and manifest write handles before publishing encoder completion. Windows readers
deny concurrent writes and deletion, with a brief retry for older worker close
timing. Manifest and MP4 length/mtime are checked as well. The audit creates a new
.decoded.jsonl exclusively; existing audits cannot be overwritten by export.

Decoder stdout uses a bounded channel and line limit, stderr retains a bounded
tail, and a stalled decoder times out. Cancellation/error cleanup kills and waits
only the owned decoder child, drops its receiver before joining blocked readers,
and retains the MP4 and partial audit. Thread creation failure follows the same
owned-child cleanup path.

Successful future execution requires real decoder EOF, successful exit, exact
NV12 byte/frame counts and strictly increasing presentation timestamps matching
index/FPS within two 100-ns units. Positive supplied sample durations must match
the same cadence; absent durations do not establish the final clip duration.
The complete record includes source dimensions, backend, encoded-file SHA-256,
and separately domain-separated input-RGBA and decoded-NV12 sequence hashes.
Sequence hashes exclude capture IDs, native addresses and codec timestamp
rounding. Compare decoded hashes only with the same decoding backend/version.

Input manifest color fields describe the encoder request, not verified decoded
color metadata. H.264 is lossy, so input and decoded hashes represent different
pixel domains. A single artifact audit cannot establish repeat-run native scene
or video determinism: all broad native-world, deterministic-video and parity
flags remain false. No compiler, parser, syntax check, build, test, encoder,
decoder, helper, game process or UI automation was run for these source edits.

### Indexed encoder input accounting and color profile (source only)

The native encoder worker now checks pixel byte count, nonzero repetitions,
remaining frame capacity and, for indexed capture, one frame per batch with the
next exact metadata index. Indexed rows are committed only after write_all has
submitted that frame's complete immutable RGBA buffer to the pipe. Short writes,
invalid batches, missing/reordered indices and excess/short totals fail the
capture. Manifest rows must equal the written indexed count. A second count
check in tick_manager also rejects an unexpected encoder completion result.
Realtime short clips and repeated gap frames retain their separate behavior.

Rows retain the existing RGBA hash and native receipts. An encoding object adds
the exact index/FPS fraction, floor/next-floor 100 ns timestamps, source layout
and requested output matrix/primaries/transfer/range/chroma/padding. It explicitly
marks encoderInputWritten=true and decodedFrameVerified=false. Pipe submission
is not evidence that a codec decoded the same frame or that a native scene was
deterministic. Input RGBA hashes remain distinct from lossy encoded-video pixels.

The worker syncs its manifest and, after successful child completion, the actual
originally created output descriptor, checking that it contains nonzero bytes.
No path is reopened, removed or replaced for this check. Errors retain partial
artifacts. Core's finish wait is now 90 seconds, longer than the native indexed
encoder's existing 60-second idle watchdog. Capture still requires successful
encoder exit; no native/world/video verification flag was enabled.

FFmpeg conversion now explicitly uses full-range RGBA to limited-range BT.709,
centered chroma coordinates, even black padding and matching output color tags.
The built-in converter already performs BT.709 NV12 conversion; its Media
Foundation media types now also declare centered, aligned progressive chroma to
match the existing 2x2 averaging. The implementation uses the documented
[FFmpeg scale controls](https://raw.githubusercontent.com/FFmpeg/FFmpeg/master/libavfilter/vf_scale.c),
[codec color options](https://www.ffmpeg.org/ffmpeg-codecs.html) and
[Media Foundation chroma flags](https://learn.microsoft.com/en-us/windows/win32/api/mfobjects/ne-mfobjects-mfvideochromasubsampling).
This aligns requested color interpretation, not the two codecs' compressed bytes
or decoded output. Native scene/resource readiness, full cosmetic/lighting
coverage, audio and decoded-video repeatability remain incomplete or unverified.
Work was limited to source edits and source/API-documentation reads. No build,
project parser/syntax check, test, encoder, helper, game process or UI automation
ran; installed binaries and configuration were not changed.

### Native replay attachment consumer (2026-10-27, static source only)

NativeBridge.attachmentPlan prepares the current archived palettes on the Java
client thread. The plan carries manifest/session, exact rational replay time,
native tick/partial and dispatch marker identities. Missing samples remain absent
actors/palettes. Active playback without a usable sample publishes blocked
ownership; it cannot silently use the viewer's live emote attachment pose. The
paired render bridge publishes plans after its normal uploaded client frame.
Native parsing bounds input to 8 MiB, retained data to 16 MiB, actors to 128,
palettes to 16 per actor and bones to 128 per actor. The immutable plan lease is
250 ms. Invalid input, frame/renderer failure or initialization invalidates ready
data; unknown control blocks changes until a fresh explicit plan resolves it.

native_attachment_replay matches the actual primary client timing against the
plan's session, marker/request/generation/replay/observation identity, held native
tick, exact float partial, controlled real timer and rational replay timestamp.
It matches the checked current palette's static layout key and ordered bone
indices/names/slots. The recorded baked root pose can retain a different resolved
parent from the viewer's current Molang selection. Only the existing checked
Space.child_matrix conversion maps those owned values back into native draw
coordinates. Callback sampling does no archive, network or Java/UI work.

The existing eight-matrix copy wrapper still forwards native code exactly once.
Replay replaces only an attested owned output after source/span/padding/alias
checks. Before and after the write it rechecks the source, writer/actor scope,
palette binding, active mode, publication revision, plan lease and native timing.
Failed writes/checks retain the existing restoration path. No descriptor is lent
Rust memory and no source palette or native hierarchy is rewritten. Publication
revision exhaustion blocks writes rather than wrapping an old identity.

Current-fallback previous entries use the current recorded target; genuine
historical entries keep their separately observed supplied matrices. Current
upload and native-cache records now distinguish replay manifest/rational-time
provenance from live emote provenance and include its retained string bytes in
their budgets. Actual historical cache keys and writer receipts still control
eligibility. Status reports owned CPU copies and the last matching identity,
with native equipment/GPU/deterministic-video verification false.

This is a source implementation of replay consumption, not runtime evidence.
The existing emote capture gate requiring known-empty native attachments remains.
Full resource/mesh/material identity, all writer coverage, interpolation fidelity,
GPU behavior and repeatable video remain incomplete or unverified. No build,
parser, syntax check, test, encoder, helper, game process or UI automation ran;
installed binaries and user config were not changed.

### Recorded actor model roots (2026-10-27, static source only)

Six archived body matrices did not preserve the actor's global placement. The
source now writes bedrock_body_samples_v2 with its own versioned block magic.
Each actor adds nine model-basis floats and three world-position doubles. Capture
requires native-model-draw and checks every model value against nativeGlobal's
actual Draw.matrix with the existing pixel-to-block basis conversion. It uses
neither AABB scale nor a reconstructed feet/yaw model. The native f32 camera
origin is recovered from its shortest JSON spelling before adding the original
camera-relative float translation in double precision.

ModelRootSample owns those values and subtracts the current native camera origin
before narrowing the relative translation for rendering. BodySample copies,
equality, immutable emote curves and their retained-data estimate preserve the
root. New block-size accounting includes its 60 bytes per actor. The reader
validates the correct version/magic, retains legacy v1 decoding and rejects
conflicting v1/v2 tracks. Existing integrity, timing, gap and allocation limits
remain. Position interpolation uses doubles; positive-handed orthogonal bases
use quaternion rotation/positive-scale interpolation. Exact observed endpoints
are retained even when a reflected, sheared or degenerate interval cannot be
interpolated. Unsupported intervals remain gaps, not collapsed turning models.

WorldFrame uses the recorded model for replacement bodies, capes, wings and worn
cosmetics. New v2 samples missing a usable root cannot authorize fresh native
replacement plans. Capture readiness requires the recorded root as well as body
parts for requested appearance, so v1 archives remain preview inputs without
inventing a recorded root. Status separates modelRootsRecorded from
modelRootInterpolationVerified=false.

Attachment plans now include the current f32 camera origin and optional rebased
actor model. Native validation bounds both. The consumer rechecks the origin
against the frozen clock's owned primary renderer and the actual camera field
already used by world observation. For a recorded root it converts block basis
columns back to native pixels, then constructs one checked Space between that
recorded model and the currently owned child model. Each archived root-space
bone thus maps directly through inverse(current child model) * recorded model,
with no intermediate narrowed current-root palette. Actor draw matrices and
camera origin are rechecked before/after copying alongside the existing clock,
marker, lease, scope, layout, source and output guards. CPU-copy status records
whether a recorded root participated. Legacy attachment plans without roots keep
their prior preview placement.

This ports observed visual placement; it does not rewrite native actor state,
physics or renderer culling. Interpolation, discontinuities, complete stock
appearance/material/lighting coverage, GPU execution and repeatable video remain
incomplete or unverified. No broad verification gate was enabled, and the native
equipment/emote export restriction remains. No build, parser, syntax check,
test, encoder, helper, game process or UI automation ran. Installed binaries and
user config were not changed.

### Native cape appearance and recorded absence (2026-10-27, source only)

The take-scoped native image collector now retains both prepared skins and raw
standard cape PNGs. Its weak-image memo is bounded to 256 entries for the two
roles across 128 actors; archive file/count/byte limits still apply. Each actor
declares bedrockCape as null for unavailable state, {present:false} for an
observed absence, or {present:true,texture:<SHA-1>} with retained loaded pixels.
Descriptors have strict field counts/types. Legacy missing fields keep their
preview behavior; an explicit unknown cannot pass cape capture readiness unless
an explicit recorded Arctic cape supplies the replacement.

Native cape images must have one standard 2:1 frame, width 64–1024 and a multiple
of 64. Loading checks dimensions before allocation and counts the image against
the shared preparation budget. Pixels keep their recorded alpha. They use the
separate look:<hash>/native key and never pass through CapeFormat.withElytra or
the animated-frame map. This also distinguishes identical content hashes used as
both native and Arctic art. Actor-specific classic cape selection prefers the
recorded Arctic choice, then the native image; elytra selection still requires an
explicit Arctic cape. Buffered raw-cape receipts keep their exact static key,
and capture compares the current and acknowledged raw/animated roles.

An explicit recorded absence can publish a bounded hiddenCapes readiness entry
containing only actor ID and its actual current native cape digest. Native parsing
rejects duplicate/conflicting classic cape entries. The existing 250 ms session
lease, active mode, exact cape mesh, complete standard skin pose, root animation
frame and known-empty chest checks apply. HideCape is a separate typed submission
plan and uses the existing checked root/color match, owned bone-span replacement
and rollback path. Its actor-scoped receipt rejects an already recorded positive
cape replacement and vice versa; it records native digest, geometry, frame and
draw count, with no replacement texture or invented positive hash.

live_frame publishes nativeCapeHidden only for the matching current native
appearance and actual animation frame with complete part data. It retains the
original nativeCape, clears cape/capeHash and disables the software cape for that
buffered draw. Java validates the receipt separately from nativeCapeSuppressed.
Recorded no-cape capture requires either that receipt or an actual observed
absence with no cape texture. Unknown archive state does not authorize hiding.
Capture with a visible recorded cape requires archived cape tick history; a
legacy preview's live viewer motion cannot attest indexed replay capture.
Status distinguishes recorded native cape state/pixel counts and native hidden
draw counts; rendering/hiding verification flags remain false.

This preserves standard cape art and explicit absence in the source replay path;
it does not record native cloth/bone animation, persona geometry, native cape
materials or wing art. Existing Java-style completed-tick cape motion and native
equipment/wing export restrictions remain. Full visual parity and deterministic
GPU/video export are incomplete or unverified. Only source reads and edits were
performed: no build, parser, syntax check, test, helper, encoder, game process or
UI automation ran. Installed binaries and user config were not changed.

### Attachment affine interpolation and camera float matching (source only)

AttachmentSample.between previously interpolated every matrix entry. Between
different rotations that can shrink or collapse the basis. The source now calls
AttachmentTransform for each owned root-space matrix. It converts column-major
native palettes to a row-major 3x3 basis, factors any negative determinant with
the same fixed X reflection, and computes a right polar decomposition through
at most 32 scaled Newton iterations. Finite inverses, orthogonality, positive
orientation/stretch and reconstruction error are checked. Quaternion sampling
uses the existing WingSample rotation path. The symmetric positive stretch is
interpolated separately and the fixed reflection restored; translation remains
linear in double precision before narrowing to the native float palette.

This supports affine shear and reflected intervals in addition to rigid scale
and rotation. Exact endpoints and identical matrices bypass decomposition,
including unchanged collapsed poses. Changing singular/ill-conditioned matrices
or determinant parity produce a sample gap through the existing replay error
path, rather than a linear fallback. Existing layout/anchor/tick/chunk bounds
and native consumer ownership checks remain. No archive encoding or reader
budget changed. Appearance status names polar_affine while retaining
attachmentInterpolationVerified=false and attachmentCoverageVerified=false.

Both core and native capture now recover native float32 positions before
comparing protocol camera targets. live_frame's shortest decimal serialization
can otherwise differ from the exact promoted wire float by more than .001 at
large world coordinates. Position comparison now requires the same recoverable
native float, also removing the previous .001 acceptance window between distinct
nearby floats. Signed zero is equivalent. Angles derived from the view matrix
and FOV derived from projection retain their bounded .001 comparison. Native
indexed-sample matching uses the same position rule. Core target normalization
also maps yaw rounded to positive 180 back to negative 180 like the public
camera packet's wirePose path.

These are source implementations, not runtime evidence. Matching sampled
transforms does not establish native hierarchy interpolation fidelity, full
equipment/material/GPU coverage, terrain readiness or repeatable MP4 export.
Broad verification flags remain false and the known-empty emote export gate
remains. Only source reads and edits were performed; no compiler, parser,
syntax check, test, encoder, helper, game process or UI automation ran.

### Buffered emote pose consumption (source only)

WorldFrame previously selected current menu emotes before rendering an already
buffered native body. A failed current-program match left the new emote free to
pose the software body independently of that frame's native target. Its wing
path also evaluated the emote again against the wing curve's separate torso.

The source now consumes the frame's owned six-part target at its exact recorded
clock. EmotePrograms retains lightweight token/actor/phase/animation records for
buffered rendering, independently of its 250 ms callback authority. These records
do not retain live Playing objects or body curves. The cache holds at most 512
records and 16 MiB, counts each shared immutable Animation once, touches all
current programs before evicting older records, and releases all data on a world
session change. The live State's 16 MiB accounting now reserves 1024 bytes per
program; the separate render cache uses that same conservative record weight.
Thus the two holders are bounded to 32 MiB combined, with shared animation data
counted conservatively in each. Native callback sampling still reads only the
immutable leased State and rechecks it after evaluation. Clearing that State
disables callbacks immediately; retained records cannot extend its lease.

Render lookup requires session, actor, token, whole/fractional sample clock,
program time/reference interval and exact phase. It returns the acknowledged
animation for matching elbow/knee geometry even if the menu has since changed or
stopped the emote. Actual owned body matrices remain usable if the bounded cache
has retired that animation, but its bends cannot then pass capture readiness.
Without an owned current target, world rendering uses the native baseline rather
than independently applying an unacknowledged emote. Direct model preview keeps
its existing independent animation path.

Wings now retain an independent model root rather than consuming the posed body
used by skin, classic capes and worn cosmetics. The older torso-reference product
was removed to follow Java WingsLayer (wing-layer-root-source.json). Native wing
observations keep their own controller/posture; Java-motion wings use the
literal layer translation before their independent part transforms. The
software renderer does not evaluate a second emote pose for wings.

Capture retains its fresh leased-program and requested-animation checks, now also
requiring the animation retained for that acknowledged receipt. A requested stop
waits until the native frame no longer contains an emote target. The existing
known-empty native attachment and unverified wing export gates remain. Buffered
render synchronization is a source implementation, not GPU, equipment, all-pose
or deterministic-video evidence. No build, parser, syntax check, test, encoder,
helper, game process or UI automation ran; installed binaries were not changed.

### Owned skin RGBA and body samples v3 (source only)

native_skinned_desc already compared the original native RGBA with its copied
description at +0x17c before suppression. It now forwards those same checked
values to the current actor's Global. No new native field, offset, hook or write
was introduced. Native submit tags carry the actual skin-mesh role from the body
observer; attachment-only tags cannot publish skin color even when a child uses
the same geometry name. Recording also requires the current world/actor scope,
actual draw and same-frame selected skin geometry. Only finite channels in
[0,1] are supported; a conflicting draw or unsupported color makes the frame's
skin color explicitly unknown. Conflicting skin poses clear it as well.

live_frame publishes nativeSkinColor only alongside the matching actual native
skin pose and appearance. Java validates the four channels and exact original
Global receipt. Raster3D captures tint/opacity values when making a sink, then
multiplies them into emitted vertex RGBA. Faded opaque textures join the sorted
translucent pass. Replacement skin receives the full native RGBA once; classic
capes, wings and worn base/glow layers inherit only actor opacity, restored in a
finally block between actors. Model preview defaults remain white/fully opaque.
This does not infer a native lightmap, shader state or separate cape/wing RGB.

New body archives write bedrock_body_samples_v3 with magic 0x415243424f440003.
Each actor appends a strict presence byte and, when present, four color floats
after the existing six parts and model root. BodySample clones, equality and
immutable curves preserve those values. Exact endpoints are kept; interior color
uses linear interpolation only when both endpoint colors are known, otherwise
it remains unknown without discarding valid body transforms. Block accounting
reserves the maximum extra 17 bytes per actor, and callback-curve retained-byte
accounting increases to 2816. Existing block/actor/inflate/index/cache budgets
remain. Version 1 and 2 formats retain their own magic/read paths and cannot be
combined with another body-track version in one mount.

Replay rendering takes color from the archived body sample and never borrows
the viewer's current tint for a missing color. Fresh replacement plans for v3
require known color; capture requires known archived color for requested body or
attached appearance. Older tracks remain preview inputs. Status distinguishes
the presence of a color-capable track from interpolation verification, which
remains false. Native materials, separate attachment colors, lightmaps, complete
GPU behavior and repeatable MP4 export remain incomplete or unverified. Only
source reads and edits were performed; no build, parser, syntax check, test,
encoder, helper, game process or UI automation ran. Installed files/config were
not changed.

### Independent cape/wing draw colors (source only)

Native submission tags now distinguish the selected Skin, Cape and Elytra
hierarchies from other attached geometry. Cape tags carry their actual native
texture digest and animation frame; wing tags carry the observed equipped wing
frame. The existing root/source-color-matched description forwards its original
RGBA before suppression through record_draw_color. Global keeps separate cape
and wing records/conflict flags, with skin RGBA retaining its prior path. This
uses the existing native fields and hooks without additional native writes.

Layer records require the current world/actor/draw scope and animation frame;
wings also match the actual WingPose geometry/frame. Repeated incompatible colors
or identities make only that layer unknown. live_frame publishes cape color only
for its actual native cape digest and wing color only beside the matching
equipped-item pose. Java checks bounded geometry, finite [0,1] RGBA, animation
frame, native appearance/equipment and the exact Global record. Armor/held-only
tags cannot supply any of these standard layer colors.

LayerColorHistory writes bedrock_layer_colors_v1, magic 0x415243434f4c0001.
Each actor encodes two independent 2-bit states (unknown, absent, observed).
Observed roles append a 32-byte SHA-256 geometry/native-cape fingerprint and four
floats. No animation-frame counter, native address or render resource pointer
enters the archive. The fingerprint is a sampled-layout identity, not a complete
mesh/material/shader lease. Packed samples use fixed-size owned arrays; frames
are bounded to 128 actors and blocks to 64 frames/1 MiB raw (the maximal encoded
actor is 113 bytes including UUID). Compressed blocks have a separate bound.

The reader validates strict state flags, float ranges, magic/index/count/order,
unique actors, native timing and complete deflate consumption. Source manifest
identity is checked on reopening. Its two-block cache evicts before decoding a
new block to bound retained plus transient allocation. An 8 MiB reservation is
included in the bounded 128 MiB aggregate appearance preparation budget,
alongside body/wing/cape/attachment/material tracks, optional glint history
and assets. The existing 64 MiB texture-transition budget is unchanged.

Sampling retains exact endpoints. Each role interpolates its RGBA linearly only
between matching observed fingerprints; stable absence remains absent, while
layout/presence/unknown transitions stay unknown. Actor, chunk, tick and clock
gaps remain gaps. Classic cape and wing roles do not discard each other's valid
samples. Existing take initialization, sampling and finalization now include the
track, and replay resolves it through the same recorded/live actor mapping.

The renderer uses the active layer's archived RGBA once, with no additional body
alpha multiplication. Worn cosmetics/glow still inherit body opacity, while a
layer without recorded native color retains a white/body-fade preview fallback.
New replay replacement plans wait through unknown layer colors; classic cape
capture also requires a color state consistent with the snapshot's known native
cape presence. Legacy explicit native-cape absence needs no native cape tint.
The unverified wing and known-empty emote export restrictions remain. Status
reports layerColorsRecorded and lastLayerColorError with interpolation verified
false. Full materials/lighting, separate armor/held colors, GPU behavior and
repeatable video remain incomplete or unverified. Only source reads and edits
were performed; no build, parser, syntax check, test, encoder, helper, game
process or UI automation ran. Installed binaries and configuration were not
changed.

### Native root cape palettes and attachment samples v2 (source only)

The existing root-selected geometry observer already classifies a hierarchy as
a cape only when it contains cape and no bones other than root/cape. When that
actual draw has no suppression replacement, the checked palette binding now
assigns archive role 7 and its observed native cape content key. This role is
distinct from native child-component slots 0 through 6; no child vector is read
at index 7. Other root geometry receives no attachment role. Binding still
requires the original model/RenderParams/actor identity, animation frame, exact
indexed geometry ownership and the constructor's root/tint/span receipts.

The native root and palette already share the same observed pixel space, so a
checked identity Space preserves the actual matrices. The existing eight-matrix
copy wrapper records only its current completed output, with its owner/group,
indexed bones and static layout digest. Suppressed cape draws cannot publish
native root cape samples. Native cape values are not independently retargeted
as six-part Java emote bones; active replay can instead supply their sampled
root-space palettes through the existing owned-output patch and rollback path.
No new native offset, hook, source-palette write or borrowed memory is introduced.

New archives write bedrock_attachment_samples_v2, magic 0x4152434154540002.
Each palette adds a strict presence flag and native cape content key before its
existing geometry/layout/bones. Role 7 requires that key; roles 0 through 6 forbid
it. V1 archives use their original magic and binary layout, with an explicit v1
reader; a mount cannot combine both track versions. The existing actor/palette,
bone and 2 MiB/64-frame block limits remain. The two-block cache now evicts before
decoding another block. Cape identity/layout transitions and recording gaps are
not interpolated through. Affine interpolation remains sampled reconstruction,
not an assertion of the engine's original cloth/controller evaluation.

Current native output samples also carry optional owned-copy provenance for the
archive manifest and exact rational replay time. Java copies and binary writing
discard this provenance; a loaded archive cannot supply an old acknowledgement.
Only a new successful current-output copy can publish it. Frame finalization
rejects invalid or duplicate structural identities before exposing a collection.

For a v2 recording's raw native cape, the replay plan selects only palettes whose
native content key matches the selected archived PNG's decoded dimensions/RGBA.
NativeImageIdentity reproduces native_skins::image's domain: little-endian u32
width/height followed by row-major RGBA. Capture checks loaded cape pixels against
their engine key once per retained image, and replay preparation hashes each raw
cape once within the shared resident budget. PNG compression, alpha preparation
and heap identity do not substitute for that comparison.

Matching raw cape art stays in the native mesh/material: the Java body plan does
not suppress it or draw the Java cape mesh. Explicit Arctic capes retain animated
art and Java physics; v1 archives retain their preview path. New raw-cape sample
gaps cannot fall back to movement-based Java physics. Indexed readiness requires
the matching native cape image, all expected current palette copy receipts at
the requested manifest/rational time, same-frame standard body/root and an owned
copied-description receipt for the archived layer color. The original source
color remains separate. Missing or failed tint writes block readiness; native
material/pass history is still not recorded.
Wing export and known-empty attachment emote restrictions remain unchanged.

Status reports nativeCapePaletteTrackAvailable separately from the unchanged
false nativeCapePaletteRenderingVerified. Original native geometry/materials are
reused, but GPU consumption, resource/mesh history, complete cloth coverage,
shader/lighting fidelity and deterministic video remain unverified or incomplete.
No compiler, parser, syntax check, build, test, encoder, decoder, helper, game
process or UI automation ran. Installed game/bundle/config files were not changed.

### Native cape copied-description RGBA replay (source only)

The leased attachment plan now carries at most 16 distinct cape colors per actor.
Each must match a sampled native cape geometry/content identity. Java resolves
RGBA only from a matching recorded LayerColorSample fingerprint, on the same
fractional replay clock as the palette. Native plan validation rejects duplicate
colors, unmatched identities and non-finite/out-of-range channels; retained-data
accounting includes the owned colors.

The existing checked skinned-description observer applies RGBA at its already
observed copied color field (+0x17c), after confirming the constructor result,
mesh/count, original root/tint and exact source spans. It never changes original
color storage. The output range must not alias the original color, root or span
headers. The transient color target rechecks the live plan lease/ownership,
dispatcher clock, camera origin, actor frame/draw and indexed palette binding.
Writes are read back; the source color is rechecked unchanged. A partial write,
changed binding, missing receipt or other failure restores the saved 16 bytes.
An unchanged target still needs the same checks and a fresh receipt.

The frame retains original capeColor separately from capeReplayColor, whose
source and applied RGBA, geometry/content key, animation frame and exact replay
identity are copied values. Conflicting source colors or a later failed/unplanned
raw cape description invalidate the frame's tint acknowledgement. Unattested
or failed raw cape descriptions also invalidate its attachment collection rather
than letting an earlier successful copy certify the whole collection.

Java validates that receipt against the exact native Global and original draw,
then capture requires both current palette receipts and the applied color at the
requested archive/rational time. A new recording observes the checked applied
RGBA when present, preserving the visible replay tint without archiving an old
acknowledgement. Existing layer-color binary layout stays unchanged.

This changes copied native field values, not visibility/culling, shader or blend
pass selection, material/lightmap history or GPU verification. All broad parity,
deterministic-video, nativeCapeTintReplayVerified and nativeCapeColorReplayVerified
flags remain false. No compiler, parser, syntax check, build, test, encoder,
decoder, helper, game process or UI automation was run; installed files were not
changed.

### Recorded native cape dithering (source only)

The checked 21-argument skinned-description constructor already exposes its last
u8 as the native dithering flag. The observer now checks that the owned output
byte at +1fe equals the constructor input, then keeps its known boolean value
beside the original cape/wing draw color. Bytes outside 0/1 remain unknown and are passed
through unchanged. A conflicting flag makes that actor/layer observation unknown.
Raw cape descriptions without their root/tint/span checks invalidate the source
color and palette collection instead of inheriting an earlier draw's receipt.

Version 2 recordings use bedrock_layer_colors_v2, magic 0x415243434f4c0002. Each
observed role appends one strict state byte after its existing layout fingerprint
and RGBA: 0 unknown, 1 disabled, 2 enabled. Absent/unknown roles carry no flag.
The worst-case actor grows from 113 to 115 bytes including UUID, so 128 actors
over 64 frames still fit the 1 MiB raw block bound. The existing compressed size,
index, source identity, two-block cache and 8 MiB preparation reservation remain.
The v1 reader retains its exact old layout and supplies unknown flags; mixed
layer-color track versions in one mount are rejected.

Exact endpoints keep their flags. Interior samples retain a flag only when both
known endpoint flags and layout identities agree. A flag change stays unknown
without discarding otherwise usable RGBA. The replay plan includes a known flag
only beside the matching recorded geometry/native-cape identity and color.

For a scoped raw cape draw, the constructor forwards all 21 original arguments
unchanged and is called once. After construction, the hook may copy the known
recorded boolean into owned output+1fe alongside RGBA at +17c. Both ranges are
checked against aliasing the original color, root and input span headers. The
existing plan lease, camera/dispatcher clock and indexed span binding still
apply. Output RGBA and the actual output flag are read back before acknowledgement;
unknown source bytes cannot be reinterpreted as booleans. Legacy plans without
a flag preserve that byte and may preview color, but cannot pass the known-flag
capture gate.

One rollback guard retains both originals, marks writes before attempting them,
and attempts to restore both fields on partial writes, failed reads, stale plans,
rejected acknowledgements or unwind. This is a synchronous owned-description
transaction, not an atomic GPU operation. The source LayerColor retains its
original flag; the replay receipt independently carries source and applied flag
values. Java validates source against the original draw and both receipt values
against the exact native Global. Capture requires the known archived flag at the
requested rational replay time. New recordings retain the checked applied flag.

layerDitheringTrackAvailable reports format capability, not proof of observed
values or shader execution. nativeCapeDitheringReplayVerified and all broader
world, parity and deterministic-video flags remain false. Lightmaps, native
visibility/culling, blend/pass/resource history and GPU results remain unfinished
or unverified. No compiler, parser, syntax check, build, test, encoder, decoder,
game process or UI automation ran; installed files were not changed. The later
field-copy revision used only source reads/edits and offline disassembly of
previously cached executable sections.

### Static actor shader-constant map (source only)

Cached text SHA-256
6fa218d247874940235aacb8fd7e3bbf0e3d1e259630d7f83fcb536f06733aeb
is the existing 1.26.5203.0 profile. Its ActorConstants getter starts at RVA
0x5958320; the initializer at 0x5959eda passes globals+c0. The getter validates
native names and scalar/vector primitive tags before storing the slots. The
native snapshot at 0x12ec803..0x12eca08 copies the data via constant+40. These
independent references identify the following global slots:

| Global offset | Name | Primitive | Float count |
| --- | --- | --- | --- |
| d0 | OVERLAY_COLOR | 4 | 4 |
| d8 | TILE_LIGHT_COLOR | 4 | 4 |
| e0 | TILE_LIGHT_INTENSITY | 2 | 2 |
| e8 | BLOCK_LIGHT_COLOR | 4 | 4 |
| f0 | CHANGE_COLOR | 4 | 4 |
| f8 | GLINT_COLOR | 4 | 4 |
| 100 | UV_ANIM | 4 | 4 |
| 110 | UV_OFFSET | 2 | 2 |
| 118 | UV_ROTATION | 2 | 2 |
| 120 | GLINT_UV_SCALE | 2 | 2 |

The existing controlled-context shader snapshot now reads these named slots,
checking name, primitive, non-array header, finite bounded data and unchanged
header. Missing or mismatched slots stay rejected. This does not establish that
each material consumes these values, archive per-actor lighting, change shaders,
or certify a GPU draw. No lighting formula is inferred from the names. Full
materials/lighting replay, native visibility, shader/resource identity and broad
parity remain incomplete or unverified; verification flags stay false.

The same cached constructor shows the final argument loaded at 0x5e9ea72 and
stored at output+1fe at 0x5e9eada before return. That static field-copy evidence
supports the owned-output replay revision above. No compiler, syntax check,
build, test, game process, encoder, decoder or UI automation was run. Only offline
binary inspection helpers read cached sections; installed files were not changed.

### Scoped layer shader inputs and version 3 history (source only)

The prior controlled-frame diagnostic is no longer the only reader of the ten
named ActorConstants above. The checked skinned-description observer now copies
their 32 floats while a selected cape/wing tag and its native palette-writer
stamp are active. It resolves ScreenContext from that actor context's +28, then
the constant group from screen+20. It checks every name, primitive, non-array
header and bounded finite data. After copying the whole set it rechecks every
slot and value by float bits, then the actor/pass/frame stamp and screen/group
references. These addresses stay local to the callback; published ActorInputs
owns only the 32 floats.

The original LayerColor includes the optional owned set. Every matching layer
description in the actor frame must supply identical inputs. A missing, rejected
or changed set remains unknown for the rest of that frame, without discarding
usable RGBA/dithering. A changed cape set also invalidates any earlier replay
color acknowledgement. Java compares each published set with that exact native
Global layer, rejects malformed/non-numeric/non-finite values and copies arrays
before retaining them. The role's geometry/native-cape fingerprint still binds
these values to the observed layout; it is not a material/resource identity.

LayerColorHistory now writes bedrock_layer_colors_v3, magic 0x415243434f4c0003.
After each observed role's v2 bytes it writes one strict presence byte and, when
present, 32 floats in the ordered ACTOR_FIELDS layout. Absent/unknown roles have
no payload. The maximum actor is 373 bytes including UUID; 16 frames of 128
actors plus headers fit below the existing 1 MiB raw block limit. Compressed
limits, two-block eviction-before-decode, 8 MiB preparation reservation and the
existing archive/index budgets remain. Version 1 and 2 readers preserve their
magic, 64-frame bounds and exact prior layouts; missing inputs remain unknown.
One mount rejects mixed layer track versions. The default legacy standalone
LayerColorSample read/write overloads still use version 1; the production v3
history passes version 3 explicitly.

Exact endpoints retain their owned shader inputs. Interior samples keep them
only when both sets and role identities are bit-identical. Lighting/UV changes
remain input gaps; shader-specific wrapping/interpolation is not guessed. RGBA
and dithering continue to use their independent interpolation rules.

Known raw-cape inputs enter the immutable native color plan as expectations.
Before modifying owned tint/dithering fields, the observer requires the current
set to match. After those copies it re-reads the selected layer set and checks
the plan again before recording a receipt. The receipt includes the checked set;
native/Java validation compares it with the original LayerColor. Indexed native
cape readiness requires known archived inputs at the exact rational replay time
and matching receipt values, alongside palette, RGBA and dithering receipts.
Legacy archives still preview, but cannot certify this new input requirement.

This captures CPU state and refuses a mismatched cape acknowledgement. It does
not apply archived lighting to the engine. In particular, cached disassembly
shows the actor constant-copy helper at 0x12ec730 called from the distinct
branches at 0x12e9214 and 0x12e95ab, while the existing skinned-description path
constructs at 0x12eabf6. Hooking that helper alone cannot be treated as complete
skinned-draw coverage. No new detour or global shader-buffer write was added.
Per-material application, original shader/resource identity, full native
lighting and deterministic GPU/video results remain incomplete or unverified.
layerShaderInputsTrackAvailable reports format capability only;
nativeLayerLightingReplayVerified and all broad parity flags remain false.

Only source reads/edits and offline disassembly of previously cached sections
were used. No compiler, syntax check, build, test, encoder, decoder, game process,
account operation or UI automation ran; installed files were not changed.

### Native setter/consumer trace and paired-export comparison (source only)

Further offline inspection of the same cached image identifies scalar/vector
constant setters that write through constant+40 and set byte constant+31 to 1.
Examples are 0x595aedf/0x595aee5 and 0x595ae9a/0x595aea4; the palette writer also
sets +31 at 0x12e9349. The skinned path constructs a variant containing its
description pointer and tag 7, then reads the current receiver's vtable+358 at
0x12eac78..0x12eac8c. Its receiver comes from the service pointer at 0x11d1ec68,
read at 0x12e96bb and assigned at 0x7f09c5. The service is supplied by an indirect
factory; its implementation and return ABI have not been resolved. These facts
are saved in inspection/actor-shader-setters-1.26.5203.0.json. No target is chosen
by a guessed table scan, and no global constant mutation or consumer detour was
added. Source-only inspection also listed installed module metadata; it did not
load or execute a game module.

The Rust export audit now exposes a read-only paired-artifact comparison through
`arctic bedrock compare-exports first.mp4 second.mp4`. Each MP4, captured-frame
manifest and completed decoded-audit file stays open during comparison. Windows
opens deny concurrent writers/deletion; length and modification metadata are
rechecked on both handles and paths. The comparison re-hashes the actual MP4,
revalidates the full capture manifest through the existing audit validator, and
streams every saved NV12 row to recompute its domain-separated sequence digest.
It requires a complete matching footer, exact counts/dimensions/FPS, valid hashes,
strict timestamp order, stable timebase, presentation cadence and distinct
capture IDs. Missing, extra, truncated or changed artifacts are rejected.

The comparison then streams both captures together to locate the first differing
zero-based frame in input RGBA, decoded NV12, rational presentation timing,
observed native camera/timeline or encoding request. Presentation offsets are
normalized to each video's first PTS, and absolute native tick origins are
normalized to each capture's first tick. Native tick/partial time may not run
backwards, and packet replay fractions must advance at the indexed frame cadence.
Cosmetic clock and appearance intent are checked when present. Volatile native
session, producer sequence and process identities are not equated across runs.
All reads are bounded by the existing per-line/frame/duration limits, with
cooperative cancellation between rows and file chunks. Container-byte equality
is reported independently of pixel/timing agreement.

The CLI emits the structured comparison and returns 0 for agreement of these
observations or 1 for differences. This revalidates saved decoded receipts;
it does not start a decoder or certify that arbitrary supplied receipts came
from Minecraft. The source recording and all resources/shaders are not yet bound
by these logs. sameRecordingVerified, nativeWorldVerified and
deterministicVideoVerified remain false even when both artifacts match. General
determinism and full Java-to-Bedrock parity remain unproven.

Only source reads/edits, installed-file metadata reads and offline disassembly
were performed. No compiler, syntax check, build, test, comparison command,
encoder, decoder, game process, account operation or UI automation was run.

### Recording and camera intent bound to finished exports (source only)

Indexed export now reserves its capture slot before preparation and activates
the encoder only after source, appearance and camera setup. Source hashing no
longer runs with an already-waiting encoder or frozen native clock. The worker
requests the existing helper's recording_source operation, then opens the named
finalized .abr read-only, validates its metadata and independently hashes its
contents. The digest and optional canonical recording ID must match the helper's
open descriptor. The source handle stays alive through the captured range and
encoder finish; Windows denies concurrent writers/deletion. File/path metadata
and the helper's descriptor identity are checked again after the last frame.
The helper's existing descriptor/stat checks remain in force between packet reads.

Rust CameraPath::sha256 and camera.cjs pathSha256 use the same binary layout:
domain arctic-bedrock-camera-path-v1 plus NUL, format/count as little-endian u32,
then each ordered key's u32 time, one easing byte (linear 0, smooth 1, hold 2,
catmull_rom 3) and six little-endian float64 pose components x/y/z/yaw/pitch/fov.
No object-key sorting, decimal rounding or guessed interpolation enters this
fingerprint. The core serde_json dependency enables its existing float_roundtrip
feature so a saved numeric camera path preserves those bits when read again.
No dependency versions or lockfile packages changed.

The helper computes the hash only when accepting a path and publishes it with
cameraRevision in correlated/periodic state. Fixed-camera/clear operations clear
the hash. The export feeder verifies the exact accepted fingerprint and revision,
then fences subsequent step, confirm and source replies to that identity. An
intervening camera change fails the feeder even if the viewer and packet sequence
are unchanged. Setter revision bounds reserve one final revision for cleanup.
Failed source-identity operations also invalidate the feeder; owned-camera cleanup
stays available after failure. Cleanup checks the same camera fingerprint and
revision before canceling/clearing, and verifies the next revision/cleared hash.
An unconfirmed or changed camera is retained with a cleanup error instead of
clearing another path. No account identity or session is created.

After the native controls have been released and the single-video decoded audit
has completed, the job writes a new .export.json with create_new and sync_all.
Its bounded intent contains source digest/byte length/protocol/optional
recording ID, the accepted camera path/fingerprint, origin/range/FPS, settling
count, encoder kind and optional appearance manifest hash. Schema-2 and schema-3 intent
also binds the observed native tick rate. A schema-specific
arctic-bedrock-export-intent-v1/v2 domain hash identifies the serialized typed intent.
The receipt binds the encoded MP4 hash, input RGBA and decoded NV12 sequence
digests, and hashes of both exact audit files. Those artifacts remain read-locked
while hashing/writing. It is emitted only after final source confirmation; errors
retain the MP4/audits/partial receipt and do not report successful completion.
ExportOutcome includes the new receipt path. GUI progress distinguishes source
binding and final receipt binding.

Paired-export comparison now reads and retains optional receipt locks, validates
the intent and capture/audit identity, and re-hashes both audit files. A present
malformed or mismatched receipt is rejected, not treated as legacy. Each bound
frame must match the intent's exact indexed replay range, cosmetic/appearance
request, clean-view state and requested settling minimum. Recording source,
camera, range, appearance, encoder and settling intent differences are reported
separately from pixel/timing differences and make matches() false. Legacy
exports without receipts still compare their available observations, with null
recordingSourceIdentical/inputIntentIdentical instead of inferred source identity.

These are content/intent bindings and saved artifact comparisons. They do not
authenticate arbitrary receipts as game-produced evidence, prove application of
every source packet, retain all game resources/shaders, or establish native-world
fidelity. sameRecordingVerified, nativeWorldVerified, deterministicVideoVerified
and all broad Java-to-Bedrock parity flags remain false.

Only source/dependency-manifest reads and source edits were performed for this
change. No compiler, syntax check, build, test, helper, encoder, decoder,
comparison command, game process, account operation or UI automation was run.

### Presentation-resolution world layer (source only)

The rendering bridge previously shared one reduced pixel surface between the
world geometry and Java GUI, halved each presentation dimension and skipped
updates for 33 ms. That let the native world advance while the uploaded
replacement bodies/capes/cosmetics retained earlier positions. The source now
invokes the Java frame path on every presentation and uses the presentation
pixel dimensions. A single uniform fit factor bounds oversized outputs to
3840x2160 without independently clamping their aspect ratio. The native surface
ABI, checked snapshot dimensions, depth element limit and Java snapshot parser
use the same maximum. The FFI signature and schema number are unchanged.

GUI dimensions remain ceil(pixel dimension / 2). Mouse coordinates are converted
with floor division, including negative positions outside the window. SoftwareGfx
retains logical dimensions and a two-pixel GUI transform; pixelScale now describes
that transform. World pixels are copied directly before GUI drawing, independent
of that transform and without a resampling/premultiplication round trip. Player
previews are also rasterized at their physical size and drawn into their logical
rectangle. A world/depth snapshot of a different size puts the client into a
temporary frame gap, retaining its session but withholding fresh replacement
readiness until a matching native observation arrives.

The depth footprint minimum now supports equal-size, smaller and larger output
surfaces up to that bound. Same-size planes validate and clone directly. Other
sizes retain every source pixel overlapped by each destination footprint; this
does not synthesize depth or prove that a selected native plane covers every
world material/pass. Invalid planes still reject the frame.

The local hudhook DX11/DX12 backends recreate a resized texture resource in its
existing ID/descriptor slot. They validate RGBA size before uploading. DX11
prepares the new owned texture/view before replacing the old pair. DX12 waits
for every outstanding overlay frame fence before overwriting the descriptor or
releasing its resource; its replacement starts in COPY_DEST with uploaded=false.
Repeated resizes do not accumulate permanent texture slots. Per-frame texture
uploads retain the existing actual render queue and completion fence path.

RGBA conversion now uses one scanline scratch buffer, preserving arbitrary image
raster layout while avoiding an additional full ARGB copy. The embedded headless
Java client keeps its 32 MiB initial heap and has a 512 MiB maximum to accommodate
the larger bounded images/depth. These source choices increase potential memory
and pixel work. No performance or resource-usage result is claimed.

Installed shader/material assets were inspected as files, but this change does
not implement their complete shader behavior or resolve the skinned consumer
ABI. Approximate software lighting, complete material/resource ownership and
GPU fidelity remain unresolved. No full parity or deterministic-rendering flag
was enabled. Only static file reads and source/documentation edits were used;
no build, syntax check, test, renderer, game, account or UI operation was run.

### Shared editor/export camera curve evaluation (source only)

The launcher has a pure Rust CameraCurve prepared from a validated CameraPath.
It copies bounded keys and prepares the float yaw track without changing saved
keyframes. Java-style segments retain each float rounding step in yaw unwrap,
including Math.round's positive-direction half-turn tie. They use duplicated
endpoint controls, uniform Catmull-Rom positions/yaw/pitch, clamped pitch and
linear FOV with a float FOV difference. Mixed linear/smooth/hold paths retain
the existing helper's rules, endpoint behavior and shortest-turn interpolation.
Fractional timestamps use the exact millisecond plus remainder/denominator
expression already accepted by the helper. A separate wire_pose operation
normalizes equivalent yaw turns before float32 conversion, including +180 to
-180. Curve overshoot follows the helper's supported-range rejection.

ReplayQueue retains this prepared curve only after the helper confirms its
accepted camera fingerprint and revision. It computes the expected wire pose
before queuing each export step, then compares all six returned camera values
exactly (numeric equality allows JSON's loss of a negative-zero sign). A helper
reply with the right path identity/time but a different sampled camera fails
the feeder. This is separate from the native presented-camera readiness check;
it does not establish actual terrain, material or GPU correctness.

The launcher editor includes a local clickable/draggable camera timeline with
keyframe marks, edit cursor and replay playhead; previous/next selection; a
button to sample the edit time; and a selectable X/Z curve overview with yaw.
Selecting a key copies its original pose, preserving saved double coordinates,
angles and FOV. Interior sampling uses CameraCurve. The bounded overview
includes each key and additional segment samples; hold jumps have no connecting
motion line, and rejected samples create gaps with a visible range error. These
views change local editor fields only. The existing explicit Preview position
button owns external camera requests. Updating an existing key remains possible
when all 4096 slots are occupied.

The cached-image static RTTI reader found 688 standard locator/vtable pairs,
with no matching named renderer/frame-builder/MinecraftGame table in that set.
This does not resolve the virtual skinned-description consumer at slot +358,
and no candidate vtable address was promoted into a native hook. The inspected
constructor also copies ScreenContext+68's 20-byte field to description+18c;
the cached MeshContext/ScreenContext declarations identify a candidate optional
normalized clip region there, with the adjacent flags matching the caller's
reads. Its coordinate convention/consumer remains unresolved. It is not used
as lighting data or replayed as opaque bytes.

Only source/documentation edits, file reads and disassembly of cached image
bytes were used. No project code, build, syntax check, test, game process, UI,
account, encoder, decoder or comparison command ran. The new sampler and editor
remain unbuilt/unverified; native hook/material parity and deterministic video
remain unfinished, and all corresponding verification flags remain false.

### Java worn-cosmetic pass semantics (source only)

The cached Java 26.2 client JAR has SHA-256
40896ee9f1e2bec3c934daac7e93d41e9e3d9c2f8ae0ca366d52ffbfd1afa290.
Static class-file decoding and shader text reads identify the material rules
used by Fabric CosmeticsLayer's entityTranslucent/entityTranslucentEmissive
calls. The bounded facts are recorded in
inspection/java-cosmetic-pass-26.2.json. No Minecraft classes were loaded or
executed to inspect them, and no shader/game files are distributed by this port.

Both pipelines define ALPHA_CUTOUT=0.1f. entity.fsh tests sampled texture alpha
before vertex colour/opacity and ColorModulator. Both use translucent blending
with culling disabled. ENTITY_TRANSLUCENT inherits DepthStencilState.DEFAULT:
GREATER_THAN_OR_EQUAL with depth writes in Java's reverse-Z convention.
ENTITY_TRANSLUCENT_EMISSIVE explicitly uses the same comparison with writes
disabled. Their setup builders sort on upload. MeshData.decodeQuadCentroids
uses the float midpoint of vertices 0 and 2, including triangle quads whose
fourth vertex duplicates their third. Perspective sorting uses descending
distance from the configured origin. DynamicTexture.createTexture selects
SamplerCache.getRepeat(FilterMode.NEAREST) for the images registered by Arctic.

Raster3D.cosmeticSink selects explicit Java base/glow rules for worn cuboids,
mesh base/emissive textures and mesh sheen. These materials always take the
ordered translucent path; texture alpha or glTF alpha labels no longer select
an opaque pass for these cosmetics. The software cutoff runs on the original
texel, before tint/fade. Surviving base fragments write depth even when the
post-tint alpha rounds to zero; glow only tests depth. These passes use the
exact conventional less-or-equal depth comparison instead of the generic
renderer path's existing depth slack. Texture coordinates wrap before integer
conversion, retaining negative, tiny and large UV behavior without saturated
integer indices. Generic skin/cape/low-level sinks retain their previous pass
rules; this is not a completed port of every player/native material.

An optional snapshot cameraRelative flag declares model/projection space. The
native live-frame builder supplies true, consistent with its existing
camera-relative matrices and roots. The Java frame renderer then supplies a
zero translucent sort origin; explicit false uses the observed absolute camera
position. Older providers omitting the flag retain projected-depth sorting.
When an origin is declared, every blended primitive uses the same distance
units. Both original quad triangles and all clipped fan triangles retain the
same pre-clipping quad sort key. Stable sorting retains their submission order
for equal keys. The software model-to-view arithmetic and GPU coverage are
still separate, unverified matters.

Java's current cuboid draw calls use the baked idle animation independently of
the cape/mesh reduce-motion toggle. Bedrock now does the same, but supplies the
recorded/current explicit cosmetic clock instead of sampling wall time inside
each piece. Mesh idle/sheens and cape frame motion keep their existing toggle
behavior. The existing cuboid fixture's source expectation was updated to this
behavior; no fixture or test was executed.

Only static file reads, class-file decoding, file hashing and source edits were
performed. No compiler, build, syntax check, test, application UI, game process,
account, encoder, decoder or export ran. Native lighting/material ownership,
per-face shader lighting, the unresolved skinned consumer and deterministic GPU
video remain unfinished. No native-rendering or full-parity verification flag
was enabled.

### Copied native dimension and Java cardinal lighting (source only)

Static disassembly of the pinned installed image identifies three related
accessors: 24c8000 returns [object+1c8]; 24c7c90 returns that object's +48 name
reference with a null fallback; 24c7cb0 returns its +78 integer ID with a zero
null fallback. The cached public Actor declaration attributes +1c8 to its shared
dimension. The old SDK's dimension-name +20 differs from the actual +48 code
and is not used. Facts are in inspection/dimension-1.26.5203.0.json.

native_dimension configures only after the executable text hash passes and
checks the exact bytes of all three accessors. It never calls these functions.
The reader copies the actor's shared dimension header, positive bounded native
reference counts, SSO name and ID, then rechecks their headers/characters/ID,
the actor registry key and its level pointer. It accepts only Overworld/0,
Nether/1 and TheEnd/2 pairs. A missing or inconsistent read stays unknown;
the accessor's null-pointer zero is never inferred to mean Overworld. No native
pointer is serialized, and these copies do not lease dimension/render resources.

Owned actor observations retain the optional canonical dimension. The frame
builder compares every available camera-local/rendered-local copy for its
actual local UUID and publishes a dimension only when they agree. It no longer
unconditionally writes unknown. The existing actor fixture's initializer was
updated with an absent dimension; no fixture was executed.

Java 26.2 Lighting.updateLevel supplies normalized (0.2,1,-0.7) and
(-0.2,1,0.7) directions for Default, with the second Y changed to -1 for Nether.
The cached JOML 1.10.8 classes establish the default non-FMA normalization order
x*x+(y*y+z*z), followed by 1f/(float)sqrt and float component multiplication.
The quad distance sum now uses that same default right-grouped sum. No promise
is made for custom Java FMA settings or identical GPU floating-point execution.

JavaEntityLighting selects the corresponding pair for known native dimensions
and declared world-coordinate frames. The worn-cosmetic vertex path computes
both front and back RGB using min(1,0.4+0.6*(max(0,dot0)+max(0,dot1))). Back
colours negate both dot products. Clipping interpolates both colour sets, and
rasterization selects them from winding before its traversal normalization.
This follows entity.vsh/entity.fsh PER_FACE_LIGHTING, including the emissive
pipeline's directional lighting; emissive bypasses the lightmap, not that stage.
Unspecified coordinate spaces and unknown dimensions retain the previous
fallback. Generic skin/cape passes and menu previews retain their earlier path.

This adds copied native world metadata and directional shading in source. It
does not apply archived ActorInputs to the native material consumer, provide
the world's sampled lightmap/fog, or prove GPU/native resource coverage. The
skinned consumer and deterministic video remain unresolved/unverified. All
parity/native-rendering verification flags remain false. Only file reads,
cached-byte disassembly/class decoding and source/documentation edits occurred;
no build, syntax check, test, application, game, account or export was run.

### Stripped Actor shader uniform metadata (source only)

The installed Actor.material.bin has SHA-256
8720b71a8e7b6c4d5e9c12b6a8293734919030559d1e3ca0ad452f532f3ae9e6.
Its inspected DXBC programs have stripped reflection, but their BGFX version 5
wrappers retain uniform names, type/array information, byte offsets and register
counts. Inspection facts and one complete wrapper layout are in
inspection/actor-shader-metadata-1.26.5203.0.json. Only file reads and Windows
system D3DDisassemble were used to inspect the instructions; no game, shader or
project code was executed.

native_shader_asset reads that bounded local asset after the executable's pinned
text check. A missing or different asset disables this optional index without
preventing hook installation. It validates wrapper ranges, identifiers, supported
types, non-overlapping uniforms, DXBC chunk ranges, stage/version and the sole
immediate CB0 declaration. The index keys exact bytecode SHA-256 values; conflicting
metadata for one hash is unavailable. It exports float4 variables only. Matrix
major order is absent from this wrapper, so matrices remain explicitly omitted.

PSO observation keeps real D3D reflection when available and otherwise attaches
this matching asset metadata, retaining reflectionVerified=false. Queue-time
capture retains its existing raw 512-byte prefix and separately copies named
float4 ranges only for an observed root CB0 in space zero with a matching shader
hash. The entire declared CB must fit in one uniquely matched mapped upload
resource, with even partial overlapping mappings rejected. Reads hold the
existing mapping lock across each owned copy, excluding
Unmap; aggregate vector bytes have a separate 512-byte limit per shader stage.
Larger/budget-exceeding vector arrays are omitted whole, with explicit counts.
No matrix, texture, root/global constant write or inferred register is introduced.
Asset indexing errors and ambiguous-layout counts appear in status.

Named values retain raw float bits and per-range byte hashes, represent nonfinite
floats as null, and report omitted matrices/vectors. Prefix and vector reads are
separate observations: concurrent native writers can change values between them,
and coherentSnapshotVerified remains false. The bounded named ranges can survive
a missing raw prefix when the full declared CB is smaller than that prefix.
Full input and actual GPU-consumption verification also remain false.

In one inspected vs_4_0 shader, FogControl is at byte 128, OverlayColor at 144,
FogColor at 416, SubPixelOffset at 944 and TileLightColor at 960 in a 976-byte
CB0. The first three fit inside the retained prefix; the last two do not. All
five vectors total 80 bytes and fit the separate named-vector budget. Its fog
instructions use projected Z with FogControl.xyz and forward FogColor.rgb.
This is a variant-specific observation, not a universal world-distance fog law.
The software renderer does not guess a projection/material mapping from it.

This source adds bounded shader-input identification, not the unresolved native
material consumer, complete fog/lightmap application or deterministic export.
Runtime material identity, complete input coverage, GPU execution and full parity
remain unverified. No build, test, syntax check or Bedrock launch was performed.

### Expressive limb material and packed normals (source only)

The existing Fabric CosmeticsLayer.drawLimbs draws SkinLimbs' bent halves through
Canvas.base, which selects the entityTranslucent material. The Bedrock avatar
path now uses JAVA_BASE for those bent halves too, retaining its native tint/fade
through an explicit colour overload. This carries the ported texture-alpha
cutoff, repeating nearest sampler, strict depth test/base depth writes and the
known world's front/back directional lighting into expressive limbs. Ordinary
unbent skin keeps its existing material path. Software capes and wings now use
the dedicated solid and armor-cutout passes described below.

Static decoding of Java 26.2 BufferBuilder.normalIntValue establishes float clamp
to [-1,1], multiplication by 127f, truncation to int and low-byte packing.
DefaultVertexFormat.NORMAL_FORMAT is RGBA8_SNORM. The entity vertex shader uses
these decoded normals directly without renormalizing them. Java cosmetic
base/glow passes now perform this signed-byte round trip before shading. This
applies to the vertex input after the current pose-normal calculation; recovering
Java's complete pose normal-matrix/trustedNormals history from a final native
affine pose is a separate unresolved problem.

These changes came from repository source and cached class/shader reads only.
No build, test, syntax check, Minecraft class loading or Bedrock launch occurred;
full native lighting, material consumption and parity remain unverified.

Additional cached native tracing finds both direct callers of the service
registration routine 7f0860: 7b898c and 7c134c. They obtain a provider through
virtual slot 6b0, call the returned factory's slot 38, store its result at owner
offset 888 and register that result. Candidate primary table e76e2d0 is stored
by constructor code at 768e61; its slot 6b0 targets getter 82c2e0, which returns
[this+728]. The actual callers' runtime vtables and the returned factory's
implementation remain unestablished. These bounded static facts are retained in
inspection/actor-shader-setters-1.26.5203.0.json; no consumer detour was selected
from the candidate pointer groups and no game was run to inspect them.

### Pinned local skinned-material replay copy (source only)

The consumer has now been resolved independently of the indirect factory. The
installed read-only data contains the profiler signature for the void
BgfxFrameBuilder::_insert(ItemDescription) method at ef84bc3. Its pointer record
at e9769b0 is referenced by 5e4b41c inside function 5e4b240..5e4b464. The vtable
entry e976918 contains that function; subtracting the observed slot 358 gives
e9765c0, corroborated by constructor store 5e0bd27. The public FrameBuilder
declaration is an attribution reference only: its slot ordering differs.

The installed wrapper copies its indirect 16-byte variant, calls visitor
5e4b4f0, and returns void. The visitor reads its tag at +8, indexes jump table
e977870, and tag 7 reaches the skinned branch at 5e4b9d2. The regular branch
copies named CPU constants into a caller-local snapshot at RBP+590, whose first
word is the description pointer. Call 5e4f967 passes that record in R8 to
5ee8e50. The helper's five-pointer ABI, stack fifth argument and void cleanup
return are corroborated by its sole direct call and installed instructions.
It reads the copied light, overlay, change, glint and UV values when populating
its material inputs. Details are in inspection/skinned-material-consumer-1.26.5203.0.json.

native_shader_consumer adds source hooks for that wrapper and material helper.
Installation checks the pinned vtable slot, direct material call and both
prologues after the existing executable hash gate. Ordinary/unplanned calls pass
through. A raw-cape replay description can prepare an immutable plan containing
its original and requested 32-float cohorts, exact description/root/tint/flag and
the existing checked actor, palette, session, clock and replay identity. This
temporary borrow is confined to the native submission guard; it never enters an
archive, published receipt or process-wide pending queue. New constructors reset
their pending description. Nested/skipped submissions and inserts mask parent
state, and a prepared description can reach the material copy only once.

The material hook requires the exact description in the copied snapshot, the
pinned FrameBuilder vtable, matching current actor/palette/replay guards and an
original cohort that matches the snapshot bit for bit. GetCurrentThreadStackLimits
must place all 180 observed bytes inside the native caller's current thread
stack. That prefix must not overlap the description. The override writes only
two float ranges, +8..+60 and +8c..+b4, totaling 128 bytes. Their field order maps
back to the existing ActorInputs archive layout; fog, render distance, matrices,
resource handles and dynamic containers are untouched. No shared shader global
is changed and no native ownership/reference count is transferred.

Writes and readbacks occur under a rollback guard. The native helper receives
all five original arguments and consumes the local override synchronously.
After it returns, the snapshot and replay guards are rechecked, original vector
bytes are restored and verified, and only then can the frame receive a local
copy receipt. Failed/duplicate applications invalidate replay readiness. Every
prepared description increments shaderInputDraws; each unique completed copy
increments shaderInputCopies. shaderInputsApplied is true only when every
prepared draw has a matching copy. A later successful draw cannot cover a missed
earlier description. Source shader inputs and requested shader inputs remain
separate in the receipt, allowing the recorded cohort to differ from live values.

The Java frame parser validates both cohorts, copy counts and the independently
owned native receipt. Raw native-cape indexed readiness now additionally requires
shaderInputsApplied and equal positive preparation/copy counts. Legacy receipts
remain readable for previews but cannot supply that new proof. Archive layout
and sampled interpolation rules are unchanged.

This is an unbuilt source implementation of one local material-copy path. Special
material branches can bypass the helper and remain gaps. Complete material/resource
identity, the remaining fog/light inputs, native GPU consumption, full-world
repeatability and deterministic MP4 fidelity are not established. All broad
native-material, GPU, deterministic-video and parity verification flags remain
false. No build, syntax check, test, project/application execution or Bedrock
launch occurred; only source/reference reads, cached-byte disassembly and edits.

### Draw-local tint/fog context and layer track version 4 (source only)

The same installed material copier has five additional named inputs. The
ActorConstants getter constructs a 32-byte name; loads at
59588b0/59588bb read its two literal halves from ef2cdb7/ef2cdc7.
The literal is MULTIPLICATIVE_TINT_CHANGE_COLOR, its primitive check at 59588f5
requires tag 4, and store 59588fd writes ActorConstants+48, or globals+108.
The material visitor copies it into local snapshot+60. That visitor independently
copies FOG_COLOR from globals+70 to local+70, FOG_CONTROL from +78 to local+80,
RENDER_DISTANCE from +80 to local+88 and HUD_OPACITY from +150 to local+b4.
The material helper reads these local fields, including at 5ee963c, 5ee9c32,
5eeb998 and 5eebf3d. Updated facts are in
inspection/skinned-material-consumer-1.26.5203.0.json.

ActorInputs keeps its original ordered 32 values and adds optional materialContext:
tint RGBA, fog RGBA, fog-control XY, render distance and HUD opacity (12 floats).
The scoped observer checks their names, primitive/non-array headers, finite
bounded values and slot identities alongside the actor cohort, then repeats the
reads and scope/global-pointer checks. Missing context is explicit. It does not
substitute a frame-end global sample, default fog, or a guessed matrix.

The local material-copy hook now bounds 184 caller-stack bytes. When both source
and requested contexts are known and the original snapshot matches the recorded
source observation, it replaces only +8..+b8: 44 typed floats, or 176 bytes.
The description pointer, matrices and resource/container fields remain outside
that write. Legacy inputs retain the two original 128-byte float ranges and do
not replace unknown context. Full before/after restoration and per-description
copy counts still apply. Missing context cannot authorize a known replay context.
No shared shader global, camera, native reference count or resource pointer is
modified.

LayerColorHistory now writes bedrock_layer_colors_v4 with magic 415243434f4c0004.
Each present shader cohort retains its 32 floats, then a strict context-presence
byte and, when present, 12 more floats. Readers explicitly retain versions 1,
2 and 3 without inventing the missing context. A fully populated version 4 actor
uses 471 bytes including its UUID. The existing 16-frame/128-actor maximum plus
headers remains below the unchanged 1 MiB inflated block bound; the 8 MiB
preparation reservation and two-block cache remain unchanged.

Native plan/receipt JSON preserves optional context, and Java validates its exact
shape and float bits. Raw-cape indexed readiness now requires that context as
well as equal positive preparation/copy counts. Legacy archives can preview but
cannot attest these newly required fields. Interior intervals still require
unchanged shader inputs; no interpolation law for changing fog/material state
is guessed. Archive status reports whether the version 4 context track exists.

When recording a replayed cape, LayerColorSample now retains the shader cohort
acknowledged by its completed material-copy receipt, alongside applied RGBA and
dithering. It previously kept the live source cohort even after replay applied
different shader values. A completed legacy core-only override retains the
checked source context that it leaves unchanged; an RGBA-only replay retains
matching unmodified shader values. An incomplete prepared material copy yields
unknown shader data.

This adds source storage and draw-local application of the identified fields;
it does not establish complete material/resource identity, software-cosmetic fog
shading, special material branches, full GPU coverage or deterministic video.
All broad native-rendering/parity verification flags remain false. Only cached
binary/source reads and source/document edits occurred: no build, syntax check,
test, application execution or Bedrock launch was performed.

### Derived normal-matrix float ordering (source only)

Static class-file decoding of cached JOML 1.10.8 identifies six intermediate
products and the determinant grouping in Matrix3f.normal. Given the native
row-major linear matrix [[a,b,c],[d,e,f],[g,h,i]], they are ae, db, gb, ah, dh
and ge, with determinant ((ae-db)*i+(gb-ah)*f)+(dh-ge)*c. JOML takes the float
reciprocal first and multiplies each cofactor by it to construct the matrix.
Its Matrix3f.transform uses nested fma calls; the default Math.fma fallback is
separate multiplication/addition, giving m0*nx+(m1*ny+m2*nz) for each row.
Vector3f.normalize similarly uses x*x+(y*y+z*z), a float inverse square root
and component multiplication. No JOML classes were loaded or executed.

JavaNormalMatrix now ports that calculation for the Java material draws,
including expressive split limbs, solid capes and armor-cutout wings.
Raster3D clones the 12-float draw pose before
validation and uses that same owned pose for positions and the cached normal
matrix. It no longer recomputes inverse-transpose cofactors for every cosmetic
normal, divides the transformed result by the determinant, or substitutes -Z
when a cosmetic matrix is small/singular. Singular/nonfinite intermediate
normals proceed to the already-ported signed-byte packing, where Java's float
to integer conversion handles NaN. The generic skin path retains its
previous normal calculation. No performance or pixel comparison was run.

This matches the inspected arithmetic for a derived final matrix, not the full
PoseStack history. The original separate normal matrix, trustedNormals flags,
custom joml.useMathFma=true settings and identical CPU/GPU floating-point
execution remain unresolved or unsupported. Framebuffer depth, fog, lightmaps
and broader native rendering are not established by this source math change.

The cape comparison identifies Java 26.2's separate material: its
CapeLayer.submit uses entitySolid/ENTITY_SOLID (initializer store offset 660),
which inherits entity shaders/depth state without an alpha-cutout define or
PER_FACE_LIGHTING. RenderPipeline.Builder.build defaults culling to true and
uses ColorTargetState.DEFAULT, whose optional blend is empty. The source now
models that solid pass for software capes as described below. Facts are retained
in inspection/java-cosmetic-pass-26.2.json.

This turn performed source/cache reads, static class decoding and source/document
edits only. No build, syntax check, test, game, UI, account or export ran. Full
parity and deterministic video remain incomplete and unverified.

### Vanilla cape geometry and solid pass (source only)

Static decoding of PlayerCapeModel.createCapeLayer identifies an uninflated
10x16x1 box with origin (-5,0,-1), cape-part offset (0,0,2), and body attachment.
CubeDefinition applies texture scale (1,.5) to its 64x64 layer, yielding a 64x32
atlas. ModelPart.Cube/Polygon supplies the six outward face corner sequences
and reversed V on the UP/max-Y face. Shared cosmetic cuboids have the opposite
winding and a different quad diagonal, so JavaCapeMesh emits the vanilla quads
directly through Xform without changing worn models or wing geometry.

Raster3D.JAVA_SOLID now culls back faces before screen-space winding is
normalized, uses front-normal cardinal lighting, repeat/nearest sampling,
strict depth comparison, no alpha cutout, and depth writes. With blending
disabled, Java writes sampled RGB even when sampled or tinted alpha is zero.
The overlay therefore stores those RGB writes with opaque coverage, so later
composition over the opaque native world does not erase them. Cape texture
registration uses decoded images/subimages without a premultiplied drawing
conversion, preserving RGB beneath transparent texels. Software cape draws
are submitted immediately before the deferred translucent cosmetic layers.
Raw capes which the world-frame replacement gates leave native remain native.

The baked cape-part +PI Y rotation and setupAnim's initial quaternion -PI Y
rotation cancel mathematically, but their rounded intermediate operations remain
observable. JavaCapePose now retains the full local sequence: Quaternionf's
Y(-PI), X(pitch), Z(roll), Y(yaw) composition, Matrix3f.rotationZYX(0,PI,0),
Matrix3f.rotate, getEulerAnglesZYX and Quaternionf.rotationZYX reconstruction.
Euler extraction uses float sqrt before atan2, without a fabricated clamp.
The final quaternion matrix follows Matrix4f.rotation/rotateAffine's field
arithmetic. Its m10 addition order differs from Matrix3f.rotate and is retained.

JOML 1.10.8 defaults joml.fastmath and joml.sinLookup to false. Its float sine
uses java.lang.Math.sin with float conversion; cosine derives from the float
sqrt of 1-sin*sin and a quadrant computed using float HALF_PI/TWO_PI and an
integer-truncated quotient. JavaCapePose uses that default path, rather than
Math.cos or algebraically removing the two half-turns. The existing native
body-parent composition remains separate; original PoseStack matrix property
branches, trusted-normal history and custom JOML option modes are not recovered.
Native pose handedness/front-face agreement, visibility/fade, lightmaps, fog,
sampler/colour-space details and pixel fidelity remain unverified.
No build, syntax check, test, game, UI, account action or export ran. Full parity
and deterministic replay/video remain incomplete.

### Wing cutout material and explicit mirroring (source only)

Cached Java 26.2 WingsLayer.submit passes the player's selected elytra texture
to EquipmentLayerRenderer.renderLayers. The vanilla elytra equipment asset's
wings layer has use_player_texture=true. That renderer selects
RenderTypes.armorCutoutNoCull, whose pipeline is ARMOR_CUTOUT_NO_CULL
(initializer store 0x5b9). It inherits ENTITY_SNIPPET and its depth writes,
defines ALPHA_CUTOUT=0.1f, NO_OVERLAY and PER_FACE_LIGHTING, disables culling,
and uses the default unblended colour target. The render setup separately
selects VIEW_OFFSET_Z_LAYERING and may submit armorEntityGlint for enchanted
equipment. Those separate operations are not established by the base material.

Raster3D.JAVA_WINGS now implements the base pass's sampled-texture cutoff before
tint/fade, repeat/nearest sampler, packed normals, known-dimension front/back
cardinal lighting, strict depth test and depth writes. Accepted texels overwrite
RGB with opaque overlay coverage; a partially transparent texel is not blended
into the native world. Like solid capes, these wing draws execute before the
deferred worn-cosmetic translucent and glow layers.

JavaModelBox factors the inspected ModelPart.Cube/Polygon face layout out of
JavaCapeMesh and reuses it for wings. Mirroring swaps box X endpoints, flips
the X face normals and reverses the complete UV-bearing vertex sequence.
This also fixes the old right-wing declaration's source bug: its mirror flag
was on a bone, whereas Geometry.parse reads mirror only from individual cubes.
No mirrored cube flag was reaching that old emitter. The new explicit right
box uses mirror=true, and both boxes have outward vanilla face order and the
max-Y face's reversed V. Shared worn-model geometry is unchanged.

The wing boxes still follow the existing native contract from Mojang Bedrock
Samples v1.26.50.4 geometry.elytra: size (10,20,2), UV (22,0), left/right origins
(-10,0,0)/(0,0,0), and rest pivot (0,24,0), converted to local Y-down bounds
4..24. The completed native bone matrices already carry controller transforms
and nonuniform scale. Java's ElytraModel instead has one-pixel deformation,
wing offsets +/-5 and its own animation state; applying those offsets directly
to the completed Bedrock matrices would not establish rig equivalence. The new
tick-endpoint Java rig path described below is independent of those matrices.
Glint, world lightmaps/fog and native face/pose agreement remain incomplete or
unverified. Native suppression/ownership gates and all parity/export verification
flags remain unchanged. No build, syntax check, test, game, UI, account action
or export ran; only source/reference reads, static class decoding and edits.

### Equipment projection layering from the owned camera pair (source only)

Cached class-file BootstrapMethods records map VIEW_OFFSET_Z_LAYERING to
LayeringTransform.lambda$static$0, which supplies +1f, and PERSPECTIVE to
ProjectionType.lambda$static$0. The latter postmultiplies the model-view matrix
by scale(1f-1f/4096f). ORTHOGRAPHIC maps to lambda$static$1 and postmultiplies
by translate(0,0,1f/512f). The forward variant is a separate -1f modifier.
RenderType.writeDynamicTransforms modifies RenderSystem's copied model-view
matrix before writing its dynamic uniform. LevelRenderer.render pushes that
stack and multiplies the camera view-rotation matrix before feature preparation;
GameRenderer.renderLevel supplies that matrix separately from the projection.
This establishes a matrix transform before projection, not a raster depth epsilon.

The native world-frame publisher now includes optional projectionContext beside
its combined viewProjection. Its fields are type="perspective",
clipDepth="negative_one_to_one", the raw column-major view and matrix arrays,
and explicit reverseZ from the copied depth surface. Both arrays are taken from
the same CameraSample used by the existing native combined-matrix calculation.
The current native publisher already requires a perspective projection; the
Java context parser additionally supports an explicitly declared orthographic
pair. It never infers a projection from FOV or decomposes a combined matrix.

WorldFrame checks cameraRelative=true, finite bounded matrix shapes, affine
view and matching projection type. It clones the pair and recomputes the
existing base viewProjection, including OpenGL-to-DirectX clip Z conversion
before view multiplication and the existing reverse-Z normalization afterward.
Every component must numerically equal the published float; only signed zero
is treated as equivalent. A provided inconsistent pair rejects the frame.
Missing context retains older providers' unlayered behavior and supplies no
equipment-layering evidence. No schema-v1 fixture was changed or executed.

JavaLayerProjection then copies the view, applies the reference scale to its
first three columns or the reference orthographic translation to its fourth,
and recomposes projection. The translation retains JOML's default non-FMA
rightward grouping. Raster3D owns the resulting equipmentMatrix and uses it
only for JAVA_WINGS vertices, before clipping/rasterization. Other materials,
normal transforms, sorting positions and terrain depth retain their existing
inputs. Off-axis projection coefficients are preserved. The old constructors
and preview path remain compatible without pretending they have a native pair.

This source addition has not established CPU/GPU floating-point agreement,
the installed view's complete semantic equivalence to Java's view rotation,
runtime frame coherence or layered native depth/pixel fidelity. The exact
cross-language float check also remains unbuilt and untested. Wing export and
all broad rendering/determinism/parity verification gates remain unchanged.
No build, syntax check, test, game, UI, account action, encoding or export ran.

### Java wing tick endpoints, adult rig and replay sampling (source only)

Cached ElytraAnimationState.tick stores old X/Y/Z angles, chooses standing,
crouching or gliding targets, then adds (target-current)*0.3f independently
per axis. Standing targets are (0.2617994f,0,-0.2617994f); crouching targets are
(0.6981317f,0.08726646f,-0.7853982f). Gliding lerps pitch 0.2617994f..0.34906584f
and roll -0.2617994f..-1.5707964f with phase 1f, or 1f-(float)pow(-normalizedY,1.5)
while descending. Vec3.normalize uses double left-associated squared length,
sqrt and division, returning ZERO below the double representation of 1e-5f.
getRotX/Y/Z uses previous+partial*(current-previous), not quaternion interpolation.

native_wing_animation is a submodule of the existing native_capes tick owner.
Its Filter advances only from Entry.advance after the completed client tick
and guarded actor re-read, with duplicate/backward ticks ignored by that owner.
It uses the owned velocity and ActorDataFlagComponent flags 32 (Gliding) and
1 (Sneaking), cross-referenced with the pinned ActorFlags declaration. These
flags' complete semantic agreement with Java is still unverified. Address/session
changes and missed ticks reset the filter; firstTick records where the observed
history begins. A per-axis cape follower snap preserves wing smoothing because wing
animation is independent of the cloak's chasing position. Render sampling and
replay seek never advance either filter.

Cape snapshots now optionally carry wingState: tick, firstTick, previous/current
three-float angles and the completed tick's crouching state. WorldFrame requires
the same completed tick and a finite native partial in [0,1] before live sampling.
The source port begins at zero angles when observation starts, matching the Java
class initializer but not recovering unobserved animation since entity creation.
firstTick is explicit provenance, not proof that an existing entity was just
created. Rust powf versus Java Math.pow, same-tick pose/flag changes, native flag
semantics and original entity lifecycle coverage remain fidelity gaps.

JavaWingModel implements the cached adult ElytraModel's own boxes: origins
(-10,0,0)/(0,0,0), size (10,20,2), one-pixel deformation, UV (22,0), 64x32 atlas,
mirrored right box, X offsets +/-5, and Y=3 while crouching (0 otherwise).
WingsLayer translates +2 pixels Z before drawing. Both wings keep sampled pitch;
the right wing negates yaw and roll. JavaCapePose.part reuses the already-ported
Quaternionf.rotationZYX/matrix arithmetic. The model attaches to the entity root,
as WingsLayer does, instead of inheriting the player's separate body-bone emote.
Existing native wing observations and suppression receipts are still required;
the Java target geometry never substitutes for that native ownership evidence.

CapeHistory now writes bedrock_cape_ticks_v2 (magic ending 0002), retaining an
explicit v1 reader. The v2 per-actor record appends a strict 0/1 wing-presence
byte and, when present, firstTick, six float angles and a strict 0/1 crouch byte.
The enclosing frame supplies tick. Conflicting v1/v2 tracks are rejected.
Maximum fully populated serialization is 1,426,828 bytes for 64 ticks and 128
actors, within the unchanged 2 MiB block limit; v2 mounting reserves 16 MiB for
decoded state/temporary buffers, while v1 keeps its previous reservation.

Replay samples the exact containing recorded tick using its stored elapsed
clock/rate and fractional requested time, then performs the Java scalar angle
lerps. It does not interpolate native wing matrices to approximate these angles.
Missing ticks, actors or v2 endpoints produce no Java wing target and cannot
authorize a new replay suppression plan. The existing sampled native wing track
is still required by the native replacement ownership path. Older v1 archives
keep their previous sampled-pose compatibility behavior. A v2 gap never borrows
this viewer's live filter or silently switches to that older behavior.

This adult rig/filter/recording source port is unbuilt and untested. It does not
cover the baby model variant, establish root/normal/GPU float equivalence,
recover pre-observation history, implement equipment glint or complete native
lightmaps/fog. Wing export and all native/parity/determinism verification gates
remain false or unchanged. No build, syntax check, test, game, UI, account action,
encoding or export ran; only source/reference reads, static class decoding and edits.

### Read-only enchantment-list metadata for equipment/replay (source only)

The installed function at 0x2488450 loads ItemStackBase user data at +0x10,
constructs the four-byte key ench (rdata 0xe8169a0), calls lookup 0x2548830,
and invokes the returned tag's virtual type getter at vtable+0x28. It returns
true exactly for type 9 (List), without examining the list length or contents.
An empty enchantment list therefore still satisfies this predicate. The pinned
public ItemStackBase implementation corroborates that meaning; it supplies no
installed offsets or function invocation to Arctic.

Cached lookup instructions establish the compound's map-head pointer at +8,
node left/parent/right links at +0/+8/+0x10, nil byte +0x19, std::string key
at +0x20 with length/capacity +0x30/+0x38, inline tag variant at +0x40, and
variant index +0x68 (0xff is valueless). Constructor 0x1d07e0 allocates a
24-byte compound, writes vtable 0xe821430 and initializes its map/count header;
it allocates a 0x70-byte sentinel, stores self links and flags 0x0101. The
compound type getter is 0xa45940 (b0 0a c3). List construction at 0x5624dc uses
vtable 0xe7d0200, whose type getter is 0x254f660 (b0 09 c3).

native_item_enchantments is now a source submodule of native_equipment. After
the existing exact-image gate, it checks lookup/predicate code and vtable slots,
requires the pinned compound type, and performs an ordered read-only key search.
It bounds root tags to 128, search depth to 32 and copied keys to 256 UTF-8 bytes.
Sentinel shape, counts, parent links, path cycles and ancestor ordering must agree.
A found variant must have a valid index and a literal native type getter whose
return agrees with it; List additionally requires the pinned List vtable/getter.
Unsupported or unreadable layouts stay unknown, never an invented false value.

Two complete observations must agree on root/sentinel headers, every visited
node's bytes, copied key text and result. The equipment observer then rechecks
stack/container headers and the optional slot metadata. The partial chest path
now rechecks optional identity/enchantment fields too. Canonical empty slots
report false only when their user-data pointer is null; otherwise this optional
metadata remains unknown. No native function is invoked, no inventory field is
modified, no list payload is traversed, and no native address/NBT payload is
published or retained in history. Read cost and runtime coherence remain untested.

Slot.enchanted is nullable in native JSON and WorldFrame.ItemSlot. Appearance
capture already serializes owned equipment into logical snapshots, so this field
is retained and changes participate in recorded state. The archive parser accepts
an optional boolean/null field and rejects true for an empty slot. When a recording
contains a known value, equipment matching requires the same known value; an
unreadable current stack cannot satisfy it. Older archives retain their missing
field compatibility and do not establish enchantment/glint fidelity.

This is the enchantment predicate, not a complete glint selector. The installed
item predicate at 0x24cda00 additionally ORs the result with bit zero at item+0x152.
That forced-item flag now has the source reader described below; runtime
agreement and custom predicates remain unresolved. Java's
EquipmentLayerRenderer instead uses ItemStack.hasFoil and may submit
ARMOR_ENTITY_GLINT. Cached reference data identifies a distinct unculled GLINT
pipeline with depth EQUAL/no writes and SRC_COLOR+ONE RGB blending while retaining
destination alpha. Its shader has no vertex lighting/tint; sampled colour times
ColorModulator is cut off below alpha .1, and RGB fades with total fog and GlintAlpha.
The armor texture is textures/misc/enchanted_glint_armor.png. TextureTransform
uses Util.getMillis times glintSpeed times 8, integer conversion, period 110000/30000
translations, a 0.17453292f Z rotation and 0.16f scale. Recording those options,
phase, texture/resource identity and fog, as well as implementing the pass, is
still required. Enchantment metadata does not enable a substitute glow pass or
relax native wing/export/parity gates. Facts are retained in
inspection/item-enchantments-1.26.5203.0.json and java-cosmetic-pass-26.2.json.

Only source/reference reads, cached static disassembly and source/document edits
occurred. No build, syntax check, test, game, UI, account action or export ran.
Glint rendering, native layout fidelity, full parity and deterministic video
remain incomplete or unverified.

### Selected base item glint predicate and replay metadata (source only)

The cached Latite Item declaration names virtual slot 0x28 as isGlint; the pinned
public Item declaration corroborates the mIsGlint bit following mMaxDamage.
Installed vtable 0xe816a90 has getter 0x24cda00 at slot 0x28 (entry 0xe816bd0),
and construction stores that table at 0x3ef5703. The adjacent item table
0xe816eb0 selects the same predicate, demonstrating a shared base implementation.
Its body reads the supplied stack's user-data pointer at 0x24cda17, performs the
enchantment lookup already inspected, and at 0x24cda60 ORs the result with
item+0x152 before ANDing with 1. The source observation therefore checks the
selected virtual, not an item-ID list or an assumed default for every Item.

native_equipment::base_forced_glint requires a stable item pointer/vtable matching
the canonical identity prefix, slot 0x28 equal to the pinned getter, and its
inspected stack-read and OR/mask code bytes. It reads the flag byte twice and
rechecks the vtable/slot. Existing identity-prefix/name, stack and container
rechecks still govern publication. Its binding is independent of the icon getter:
an item may override icon selection and still select the known base glint method.
An unknown glint override, failed read or changed flag remains absent/unknown.
No native function is invoked and no inventory/item field is edited.

Slot now publishes nullable forcedGlint and nativeGlint alongside enchanted.
For a known base selector, forcedGlint=true yields nativeGlint=true even if the
enchantment observation is unavailable. With forcedGlint=false, nativeGlint
equals the optional enchantment result. Without a known base selector, its
result remains unknown. Empty slots explicitly have no glint draw; their
nativeGlint is false, with no forced-item flag invented for a missing item.
The read counter is a CPU observation diagnostic, not a rendered-pass receipt.

WorldFrame validates these relationships, including rejecting glint on an empty
slot or a nonempty known result that cannot follow from its known inputs.
Appearance capture already includes the owned equipment object, so the nullable
fields persist in logical history and the strict parser now accepts them.
Replay equipment matching compares each known recorded field with the current
known field; a changed forced bit or unavailable predicate cannot silently match.
Older recordings with missing fields retain compatibility without glint proof.

This establishes source handling of the installed base predicate only. Custom
virtual overrides, Java ItemStack.hasFoil overrides, native material activation,
Java option/clock/fog observations and live glint rendering remain incomplete.
The following source-only section adds the software pass and reference asset
binding; it does not supply those observations. No export/parity
gate is relaxed. These source changes have not been built, syntax-checked or
tested, and no game, UI, account action, encoder or export was run.

### Java armor glint shader and local reference artwork (source only)

JavaArmorGlint reconstructs Java 26.2's armor TextureTransform and glint shaders
from cached class/shader data. It requires explicit Util.getMillis, glint speed,
strength, ColorModulator and environmental/render-distance fog inputs. It never
samples a wall clock or substitutes Java defaults for missing observations.
Util.getMillis delegates to the selected TimeSource through getNanos and divides
by 1000000; that clock is independent of the completed entity tick.

Raster3D.JAVA_ARMOR_GLINT uses the equipment camera, no culling, exact EQUAL depth
and no depth writes. It interpolates transformed UVs and spherical/cylindrical
Position distances, samples LINEAR/REPEAT at texel centres, applies the post-
modulator alpha cutoff and fog fade, then adds source RGB squared while retaining
destination alpha. Vertex tint, normals, lightmap and actor fade are absent from
the glint shader. JavaWingModel.emitGlint re-emits the same adult wing geometry
and poses after its base. This is an explicit-input entry point; live rendering
does not call it until the actual foil selector and coherent inputs are supplied.
No deterministic export gate accepts these unobserved inputs.

The software depth comparison and sampler arithmetic have not been compared to
native GPU quantization. The overlay representation also cannot generally express
an additive scene contribution where it has no opaque base coverage: this pass
retains destination alpha exactly. Native world blending remains separate work.

The installed vanilla Bedrock actor/item glint PNGs are also 128 by 128, but
independent decoded RGBA inspection finds different pixels from Java's armor
glint. Java's metadata selects blur=true, default clamp=false; ReloadableTexture
selects LINEAR/REPEAT and allocates one RGBA8_UNORM mip. Bedrock artwork is not
silently substituted.

An optional java_reference_jar absolute local file in arctic-bedrock.json is
validated by the launcher and native bundle and passed through the four-string
NativeBridge.init overload. Existing Java overloads remain available. The bundle
builder source accepts --java-reference-jar; it has not been run. JavaReferenceAssets
opens that archive only as ZIP data, bounds archive/entry/selected-resource sizes,
rejects duplicate selected entries, rechecks file identity/attributes, and pins
both texture and metadata hashes before decoding the 128 by 128 PNG. It never
adds the archive to a class path, loads its classes, extracts or copies its assets.
Absent/rejected artwork remains unavailable; appearanceState exposes a path-free
javaReferenceAssets status. An asset load does not attest a rendering pass.

The exact hashes, source contracts and remaining gaps are recorded in
inspection/java-cosmetic-pass-26.2.json. These changes are unbuilt and untested.
No game, project class, UI, account action, encoder or export was executed.

### Enchantment-list headers and the Java foil selector (source only)

Further cached Java 26.2 class decoding establishes a selector difference:
ItemStack.hasFoil first returns a non-null ENCHANTMENT_GLINT_OVERRIDE Boolean,
including explicit false. Without that component it delegates to the selected
Item.isFoil. The base implementation uses ItemStack.isEnchanted, which checks
whether the ENCHANTMENTS component (or ItemEnchantments.EMPTY) is nonempty.
The installed Bedrock predicate instead checks whether ench is a ListTag, even
when its list is empty. Neither that Boolean nor a nonempty unvalidated native
list alone supplies Java's component/override/selected-item semantics.

native_item_enchantments now also reports optional enchantmentListSize and
enchantmentListElementType. The node variant copy already contains the ListTag
header: begin/end/capacity at list+8/+0x10/+0x18 and element type at +0x20.
Separate code/vtable gates pin ListTag::toString's size calculation at 0x254f6a5
and its type read at 0x254f6c6. Constructor stores and ListTag::equals independently
support those offsets and the eight-byte vector stride. Header pointers remain
private metadata and are never dereferenced as entries.

The reader checks alignment, ordering, divisibility, legal type, at most 256
entries and 4096 allocated slots. A canonical null vector is size zero; an
allocated empty vector also retains size zero and its observed type. Nonempty
End-type lists and malformed/oversized headers leave size/type unknown while
preserving the independently known native list-presence result. Both complete
node observations and the enclosing slot/container rechecks must still agree.

WorldFrame requires size/type to be known together and consistent with a
nonempty equipment slot and the native list-presence Boolean. Appearance
snapshots/history retain both nullable fields through existing owned JSON
capture. ReplayAppearance validates strict optional integer types/ranges;
known recorded values require equal known current values. Old recordings that
omit both fields remain readable. Entry payload and registry/component
conversion, Java overrides, live foil selection and glint activation remain
unfinished; rendering/export/parity gates are unchanged. This source work has
not been built or tested, and Bedrock has not been opened.

### Bounded raw enchantment entries through appearance replay (source only)

native_item_enchantments now has an independently gated payload reader for at
most 64 native enchantment entries and 512 searched entry-map nodes per list.
The generic compound lookup retains the existing root/sentinel, parent, key,
ordering and cycle checks. Nonempty payloads require CompoundTag elements; entry
pointers are checked for alignment and aliases. Only id and lvl are searched.
Both field searches must agree on shared root/sentinel and overlapping copied
nodes. Two full observations compare pointer arrays, all searched compound data
and raw values before existing slot/container rechecks accept the copy.

Cached installed decoding identifies the list consumer at 0x519b500, reached
from 0x248859e. It uses the typed short lookup at 0x2548a30, with its index-2 filter
at 0x2548b08. ShortTag's table/type getter and copy routine independently establish
the short payload at object+8. Separate fingerprints gate this stage, so an
unsupported payload does not become a fabricated empty list. Runtime copies have
not been observed under the current source-only instruction.

The decoder narrows id to its low byte and skips values above 41; lvl is a signed
short. Missing/non-short fields default to zero in that decoder. Arctic retains
the original nullable signed shorts instead: no narrowing, defaulting, filtering,
registry mapping or level normalization is silently applied. Null fields mean a
coherently missing/non-short tag; a null entries collection means unavailable
payload data. A checked empty list carries an empty collection. All native
addresses remain private temporary observation data.

The nullable enchantmentEntries field is carried by native equipment, WorldFrame
and existing appearance snapshot/history capture. Each entry has exactly id and
level fields. WorldFrame checks list length/type consistency, the 64-entry bound
and signed-short ranges. ReplayAppearance additionally requires strict JSON
number/null types and exact entry shapes. Known recorded collections require
equal known current collections, preserving order and null fields; older archives
may omit the collection. Header-only observations remain independently useful.

The pinned Enchant::Type declaration and installed upper-bound check now provide
registry-conversion inputs. Java 26.2 ItemEnchantments class data shows a further
distinction: its constructor accepts levels 0 through 255, its JSON codec accepts
1 through 255, and its stream codec passes VAR_INT levels through construction.
Entry conversion and glint overrides therefore need separate handling; the next
section adds a supported vanilla projection. Raw list length or values are not
automatically treated as Java hasFoil. Live
glint activation, native/GPU fidelity and deterministic export gates remain
unchanged. No build, test, game, UI, account action, encoder or export was run.

### Vanilla Java enchantment-component projection and archive checks (source only)

JavaEnchantmentComponents projects supported raw native equipment observations
into an owned logical Java 26.2 enchantment map. The pinned native Enchant::Type
order supplies IDs 0 through 41; cached Java Enchantments.<clinit>/key supplies
the matching minecraft resource names, including Lunge. These are names, not
Java wire registry integers. No Minecraft class is loaded or ItemStack created.

Projection follows the inspected native decoder: missing/non-short id/lvl fields
default to zero, id narrows to its low byte, and IDs above 41 are skipped. Raw
captured fields remain unchanged. Empty equipment, checked list absence/non-list
type, checked list size zero and lists containing only skipped IDs project to an
empty map. Accepted entries retain levels in Java's constructor range 0 through
255, including zero; this portable adapter data is not passed off as Java's
stricter JSON component codec. Duplicate native instances and out-of-range levels
remain unavailable with explicit reasons. No first/last/max rule, clipping or
partial component map is substituted.

WorldFrame.ItemSlot.validate derives the projection after raw validation. Its
optional javaEnchantments field has schema 1, reference
java-26.2-bedrock-1.26.5203.0 and a sorted levels object. Snapshot/history capture
serializes this field through the existing owned equipment data. Recorded or
adapter-supplied projections must have valid schema/reference/names/ranges and
equal a fresh projection from their raw source; unavailable source cannot be
promoted by providing a map. Accepted DTOs are replaced with owned data whose
private fields and immutable map cannot be changed through the public accessor.
Old recordings may omit the field; derivation uses only their available inputs.

ReplayAppearance checks strict JSON integer types/ranges and exact projection
fields before deserialization. Equipment matching requires each known recorded
projection to equal a known current projection as well as retaining raw matching.
NativeBridge.appearanceState exposes javaEnchantmentComponents counts and reasons;
counts do not attest native or GPU behavior. baseItemFoil exposes the base Java
Item predicate only. Explicit ENCHANTMENT_GLINT_OVERRIDE and selected Item.isFoil
overrides remain unresolved, and no renderer uses this map as full hasFoil proof.
Live glint activation and deterministic export gates remain unchanged. These
source edits have not been built, syntax-checked or tested; Bedrock was not opened.

### Explicit Java foil selector and replay state (source only)

JavaItemFoil implements the decoded Java 26.2 ItemStack.hasFoil branch order.
An adapter-supplied javaFoilInputs object declares schema 1, reference java-26.2,
componentOverride (unknown/absent/true/false) and itemPredicate
(unknown/base/vanilla-elytra/true/false). Predicate true/false values are explicit
copied results, not an inference that an implementation is constant. Unknown
override never falls through. Explicit override true/false resolves before item
or enchantment availability; confirmed absence delegates to the selected item
predicate. Base predicates require the checked enchantment projection. Native
forcedGlint is never substituted for a Java component override.

Cached Items.<clinit> instructions 0x2247 through 0x228e register Elytra through
registerItem(ResourceKey,Properties). Bootstrap 5 of that overload targets base
Item.<init>. Thus the vanilla-elytra predicate names an inspected base branch;
it requires minecraft:elytra identity and still needs explicit override knowledge.
No default override absence is inferred from the item name or registration.

WorldFrame validates input states/reference and a copied nonempty item identity,
owns a fresh input copy, and recomputes nullable javaFoil. Supplied results that
disagree with the branches are rejected. Empty equipment has no foil draw and
does not accept selector inputs. Appearance snapshots/history retain inputs and
results through owned equipment JSON. ReplayAppearance checks exact input shape,
strict string/number/Boolean types and recomputation. Known recorded states and
results require equal known current states/results; unknown recorded inputs make
no assertion and are distinct from confirmed absence. Older archives may omit
both fields. NativeBridge.appearanceState exposes javaItemFoil counts/reasons.

The normal installed-game producer currently supplies no javaFoilInputs. This
entry point does not attest the source of an adapter's declarations; source
ownership remains its responsibility. Nonempty native slots therefore remain
unknown rather than using nativeGlint or the base component map as complete Java
hasFoil proof. Live glint uniforms/activation and deterministic export remain
unfinished. No verification gate was relaxed. These edits are unbuilt and
untested; no Bedrock, UI, account action, encoder or export was started.

### Replay editor/export FOV contract alignment (source only)

Source comparison found an integration mismatch: offline CameraPose/CameraPath
and camera.cjs accept FOV 1 through 179 degrees, but NativeCameraTarget and the
native camera command validator accepted only 5 through 170. A saved/imported
path with FOV 2 or 175 could pass editor/helper validation and then fail the scoped
native camera command. Both command validators now accept the same 1..=179
degree range as the existing path/protocol contract.

The native camera component stores radians at component+0x50. target_bytes still
converts command degrees to radians in the copied 44-byte camera record; it does
not change aspect ratio or near/far planes. The observer previously applied a
degree-shaped upper bound of 179 directly to that radian field. It now requires
a positive angle strictly below float32 PI, leaving matrix, ownership, source
freshness and scoped restoration checks separate. Unit annotations distinguish
the public degree command from the native radian observation.

This removes a source validation inconsistency, without claiming expanded-FOV
game behavior, camera alignment or deterministic-video fidelity. The exact change
and unverified scope are in inspection/camera-candidates.json.staticFovContractPort.
No build, test, game, UI, account action, encoder or export was run for this change.

### Accumulated Java camera yaw during curve evaluation (source only)

Further source comparison found that valid input keys can accumulate an
unwrapped Java yaw track beyond the input-key angle limit after repeated turns.
Rust CameraCurve and camera.cjs previously reused saved-pose validation on
Catmull-Rom interior samples, rejecting that accumulated yaw even though endpoint
sampling retained it. Java CameraPath keeps the unwrapped track through spline
evaluation. This could interrupt a valid imported path during preview/export.

CameraPose.validate retains the saved-pose yaw limit. Its separate
validate_evaluated operation checks all finite values, world coordinates, pitch
and FOV without applying that key-input limit to accumulated yaw. The Rust Java
segment sampler uses this validation. camera.cjs likewise retains its public
pose input bounds and uses a private checkedPose path for evaluated unwrapped
angles. Both wire conversions validate raw geometry before float quantization,
then normalize equivalent yaw turns before conversion, retaining the existing
positive-180 to negative-180 rule. Unwrapped angles are not normalized before the
spline, so its turn direction and control values remain intact.

The path format/fingerprint, key sorting, yaw unwrap tie rule, easing, rational
sample timestamps and position/pitch/FOV checks retain their existing contracts.
This is a source correction only. Neither the sampler nor the helper/game was
executed; no builds, tests, encoder or export ran. Rendered camera agreement and
deterministic-video fidelity remain unverified.

### Native simulation cadence in export intent (source only)

The export recipe previously bound output FPS without binding the native tick
rate used to advance the scene. ExportIntent schemas 2 and 3 include
nativeTicksPerSecond from the freeze acknowledgement's observed integral rate
(1 through 1000). The job constructs intent after reading that rate and uses
the same value for NativeExportClock's timeline. No simulation-rate default is
supplied. Schema 1 remains readable only without the new rate field; its omitted
optional field and legacy hash domain retain the old typed serialization rule.
Schema 2 requires the rate and uses the v2 intent hash domain. The outer receipt
format remains schema 1, with the independently versioned nested intent.

Before writing a new receipt, export_identity reads the locked frame manifest
with bounded newline-terminated records and cancellation checks. Every indexed
record must have the recipe's rate in requiredControls.ticksPerSecond and the
observed world.controls.clock.ticksPerSecond. With frame zero as its capture-local
tick origin, record i must have tick origin+floor(i*rate/FPS) and the exact float
partial used by Timeline.frame. Missing, extra, unordered, malformed or mismatched
frames prevent receipt writing. Absolute tick origins remain outside intent so
fresh replay-world captures can be compared by relative cadence.

Paired comparison applies the same rate/cadence check to every bound schema-2
frame and reports nativeTickRate and intentSchema differences explicitly. A
legacy recipe's missing rate remains unspecified; it is not promoted to 20 TPS
or treated as identical to a schema-2 recipe. Existing replay/camera/pixel/audit
checks remain in effect. This binds an additional observed export parameter; it
does not verify native reconstruction, terrain, shader fidelity or repeatable
video. Source changes are unbuilt and untested. No game, encoder, decoder,
comparison, account action or export was run for this change.

### Exact unchanged cosmetic samples during replay (source only)

Source review found that body/wing/root replay could decompose and reconstruct
an identical matrix at fractional sample times. A floating-point quaternion
round trip need not reproduce the captured matrix's residuals exactly, so this
could alter a stationary sampled transform. BodySample and WingSample now return
owned exact copies for identical endpoint samples. The shared WingSample.blend
also copies an equal validated matrix, preserving a constant part/basis when
other parts of a sample change. ModelRootSample reuses an equal immutable root.

Unchanged body/layer RGBA channels and double model-root position components
retain equal endpoint bits, including signed zero; changed components keep their
existing interpolation formulas. LayerColorSample copies a fully unchanged
sample, retaining its identities, presence, dithering and shader inputs exactly.
AttachmentTransform already had an equal-matrix copy path and is unchanged.

These paths operate only after the existing phase/sample validation and history
readers' contiguity checks. They do not bridge missing actors, missed ticks, long
pauses or chunk gaps, and do not infer motion between sparse observations. No
archive format or readiness/verification gate changed. The source correction
has not been built or tested; neither the renderer nor game was run. Stationary
pixel agreement, all-pose native animation fidelity and deterministic video remain
unverified. Source scope is recorded in inspection/cosmetics-replay-sampling-source.json.

### Affine model-root replay and explicit coordinates (source only)

ModelRootSample now preserves an unchanged basis exactly even when its separate
double world position changes. For changing bases, the existing positive-handed
orthogonal interpolation remains the first path. Previously unsupported affine
bases use the existing AttachmentTransform bounded polar interpolation, with
explicit row-major 3x3 to column-major 4x4 conversion and zero translation. This
allows same-parity reflected or sheared roots within that method's admitted
range. World positions retain their separate double interpolation and are made
camera-relative only when producing the draw matrix. The root archive format is
unchanged.

The affine path retains AttachmentTransform's rejection of parity changes,
singular or ill-conditioned factors, failed convergence and non-positive stretch.
It supplies a bounded interpolation policy for recorded endpoints; it does not
establish the native engine's intermediate animation law. Wing and body-part
interpolation, history continuity checks and export readiness gates are unchanged.

Native root capture and recorded-root playback now require an explicit
cameraRelative=true declaration before adding or subtracting the camera origin.
The built-in native frame producer already supplies that declaration. Missing or
incompatible coordinate declarations produce a recorded-root error instead of
being interpreted as relative coordinates. Older tracks without recorded roots
remain readable. These source edits have not been built or tested, and Bedrock
was not opened. Affine playback, native animation fidelity and deterministic video
remain unverified; source scope is recorded in
inspection/cosmetics-replay-sampling-source.json.

### Shared rational camera timestamps (source only)

The baked .abr camera exporter previously sampled origin+index*1000/FPS,
whereas CameraCurve.sample_at and native video queue steps sampled an integer
millisecond floor plus its rational remainder. Those expressions have different
floating-point operation order despite representing the same rational time.
camera.cjs now provides sampleTime, frameTime and sampleAt. Baked camera packets
use the floor/remainder representation derived from their integer frame index,
and helper video steps use the same sampleTime conversion. The baked export's
FPS range remains 1 through 120; native video retains its existing 1 through 60
range. Packet timestamps remain floored whole milliseconds and packet ordering
is unchanged.

The shared conversion requires a bounded integer millisecond origin, denominator
1 through 1000 and a remainder within that denominator, with no positive fraction
at the 24-hour limit. Frame construction rejects unsafe integer numerators.
Direct camera sampling now rejects non-finite or out-of-range times before
selecting a segment. Saved path formats, curve formulas, native tick scheduling
and capture verification gates are unchanged.

This is source alignment of camera-time arithmetic. No particular pixel or
floating-boundary disagreement was reproduced, and no helper, game, build, test,
encoder or export was run. Baked/native camera agreement and deterministic video
remain unverified. Scope is recorded in
inspection/camera-rational-sampling-source.json.

### Producer-wide flat-cache storage journal (source only)

native_emote_cache_producer_storage takes stable, fully guarded key/matrix
snapshots before and after the exact five-pointer native producer. One outermost
visible producer claims each actor owner on its callback thread; descendants
share that journal. Hidden parents leave ownership available to visible
descendants. The scope owns its event before capture can unwind, so failed
capture cannot orphan the thread-local producer event.

Complete native key ordering, equal stable vector counts, matrix bounds and
unchanged input/component scope are required at both boundaries. Unequal counts
inside insertion are never treated as a stable cache. The narrow matrix
reallocator defers to an active owner journal, which covers key-only movement,
in-place shifts and reallocations across recursive calls. Old allocations are
never reread after forwarding the original; no history mutex or journal borrow
spans that call.

The journal seals eligible old records and also captures successful witnessed
child ticket completions. This second set is necessary when a later sibling
shifts a newly inserted child's key or matrix. Final targets include new slots,
not just the pre-call keys. Each completion retains its own key, serial, frame,
native bits and opaque cache witness. Equal final bytes or a newer serial alone
cannot establish a completion. Replaced, conflicted and revoked records cannot
be revived. A child completion may supersede a still-open, strictly older
same-thread ancestor ticket for the same slot; history reads remain prohibited
until all producers and pending tickets close.

Only storage witnesses move. Recorded native/supplied poses and emote/replay
clocks are retained verbatim. Missing or changed unpaired slots lose eligibility.
Snapshots are limited to 512 KiB each and completed child seals to 1024 per
journal. Their maximum key payloads, completion snapshot and all 1024 possible
final targets are reserved under the shared 4 MiB per-thread retention budget
before the original runs. Failure or exhausted admission revokes owner history
and current-frame upload eligibility.

Clear/assignment and structural mutation outside recognized producer scopes,
current component readers, actual native cosmetics insertion, scene/GPU
ownership and deterministic replay/video remain unfinished. The installed
current-build candidate remains inactive and full parity remains unverified.
Only source was read and edited; no builds, tests, syntax checks, helper/project
execution, exports or Bedrock launch occurred. See
inspection/native-emote-cache-producer-storage-source.json.

### Transform observer and root-component migration (source only)

Cached native bytes narrow the data-driven draw candidate to 0x58b5250. The
previous string search found seven functions containing gliding_speed_value;
the current renderer candidate calls this one at 0x5986423. Its unique .rdata
pointer occupies slot 2 of table 0xf26a7f0, matching the older renderer's slot
and caller relationship. Both caller sequences pass model, indirect shared
renderer temporary, render data, translation and a fifth pointer in the
outgoing stack slot. Native stack reservation changes from 0x268 to 0x248.

The complete masked-byte comparisons still fail for all three old transform
entries; this candidate evidence does not replace those failures. The old
renderer calls global-transform helper 0x1d5cf90, while the current renderer
contains repeated type/rotation-helper work from that old helper. Inlining is
an inference; neither an equivalent standalone entry nor the complete inline
input/output boundaries have been established. Substituting another three-
pointer entry would not establish the required before/after transform pair.

Root-component reads in animation_pose, native_transform, attachment binding,
expression binding and skin selection now use the shared scene family's
ANIMATION_COMPONENT member rather than duplicated 0x270 literals. This retains
the approved old layout and leaves the independently observed current 0x290
member inactive. Complete component, render-data and camera-stack readers,
global-transform observation strategy, native insertion and runtime parity
remain unfinished. Only cached bytes and source were read/edited; no builds,
tests, syntax checks, project/game execution or Bedrock launch occurred. See
inspection/static-transform-component-layout-1.26.60.5.json.

### Native draw-entry observation strategy (source only)

The actual cosmetics root in live_frame::native_root comes from Draw's copied
model/view/projection matrices. Global's intermediate before/after matrices
were helper diagnostics; no feature placement uses them. The source now
supports an exact-family drawEntry strategy which records the actual model draw
without requiring the missing standalone helper. This avoids treating a guessed
three-pointer function as its replacement.

native_transform_profile joins the exact five-pointer draw entry, its real E8
caller, the renderer's slot 2 and the shared scene family. The older profile
retains helperThenDraw with the independently witnessed three-pointer helper.
Both complete 27-byte draw prologues are explicit, including their differing
stack reservations. Binding and every original are published before enablement;
all prefixes are preflighted first and failed group enablement disables already
enabled members. Observations remain gated until the whole group is enabled.
Each callback still forwards the original arguments exactly once.

drawEntry copies the native stack matrices and validates actor identity, frame,
world scope, component/model ownership and selected renderer before publication.
Identity/frame/scope are reread after copying. Duplicate observations remain
ambiguous. Global serializes observation=drawEntry and omits unavailable helper
before/after values; it never writes identity matrices into those fields.
Older JSON arrays deserialize as optional values and missing observation retains
the legacy default. Java consumes nativeGlobal as diagnostic metadata and the
root path continues to use the same Draw fields.

The current native draw can update component frame state during its original;
the scoped handoff described below retires earlier receipts and reads the new
native value. Downstream palette scopes still require exact frame and observation
version equality. Complete current component/frame layout and phase validation,
render-data/camera-stack layout, scene/GPU ownership, custom
native insertion and deterministic export remain unfinished. The current
candidate stays inactive under the old complete image/layout family; this is
not a readiness or parity claim. No builds, tests, syntax checks, project/helper
execution, exports, process inspection or Bedrock launch occurred. See
inspection/native-transform-draw-entry-source.json.

### Component frame handoff within the native draw (source only)

Cached 0x58b5250 draw bytes read component+0x368, choose the clock source using
component+0x35c and may force its value to the selected native counter minus
one at 0x58b56fe before the virtual update. Thus an observed component frame
cannot be assumed to increase monotonically, or to remain unchanged between
the typed draw entry and a later palette callback.

native_transform_draw_frame now holds a thread-confined guard around the
original five-pointer model draw. Its call identity records only that
invocation's data, model and root component. Actor/model/component ownership,
non-UI flags, world scope and root scale are rechecked before a frame handoff.
The guard owns cleanup before publishing its call identity, including unwind
paths, and closes after the original returns. Frame changes outside this scope
cannot refresh eligibility. Unchanged owned draw matrices remain the source of
model placement; the handoff neither resamples an arbitrary bone stack nor
constructs an intermediate transform.

Model/palette/draw/emote reads and skin/wing observations synchronize against
the actual component frame. A handoff creates a fresh owned observation for
the same actor/scope, retires the old skin reference and global receipts, clears
palette writes, uploads and attachment samples, and invalidates expression
selections without resetting their serials. Every ledger retains its prior
failure poison. Old owned global/reference state drops outside the TLS borrow.
No native palette or recorded emote/replay clock is edited.

Each actor scope and observed handoff receives a checked process-local serial.
Palette Stamp includes that observation version, so a previously captured
borrow stays invalid if native frame values repeat. This version is neither
archived nor added to the persistent cache-history context key: owned cache
poses must still be recoverable across real draws through their existing
native write/storage witnesses. Serial exhaustion rejects observation.

A failed handoff also advances an independent thread-local rejection epoch.
Existing scopes retain their old epoch, so a failed snapshot followed by a
return to the old native frame value cannot revive their earlier receipts.
The cell remains available when the main actor TLS borrow itself fails; a new
nested actor scope cannot clear its ancestor's rejection. Epoch exhaustion
remains terminal. Unrelated subsequent actor scopes start with the new epoch.

The boundary rereads the final native frame before closing and actor completion
revalidates it before publication. These source changes do not establish the
remaining current component/camera-stack layouts, actual scene/GPU ownership,
native custom-feature insertion or deterministic replay export. Current-build
activation and runtime parity remain unverified. No builds, tests, syntax
checks, project/helper execution, exports or Bedrock launch occurred. See
inspection/native-transform-frame-handoff-source.json.

### Native draw matrix-stack migration (source only)

Both cached renderer bodies follow render context+0x28 to the screen and
screen+0x18 to the camera's matrix stacks. Their native push reads the model
stack at camera+0x40 using map/capacity/offset/count at +8/+16/+24/+32 relative
to its 40-byte deque header. The selected matrix is one 64-byte block at
(offset+count-1)&(capacity-1). The current matrix writer independently reads
view at camera+0 and projection at camera+0x80 into the previously mapped
renderer output members. These are static semantic joins; no live or GPU
equivalence is inferred.

The shared scene family now names those context, camera and deque fields.
native_draw_matrices retains all three snapshots until it has recopied and
compared every header, selected map slot and matrix byte sequence. It rechecks
the context/screen/camera chain around the complete collection. Finite-value,
pointer-span, power-of-two capacity and depth limits apply before publication;
map/block overlap with deque metadata is rejected. Ring indexing retains the
native machine-width wrapping behavior. No native stack is modified.

Only the owned matrices escape the synchronous reader; no header, map, slot or
matrix address enters Global or replay state. Draw binding requires the
msvcDequeMatrix64Block1 layout label as well as the complete image and selected
scene family. The existing single-stack read cases still target the shared
reader, but no cases or other tests were run. This is a bounded recheck under
the native callback's ownership, not an atomic cross-thread snapshot or a
camera/scene/GPU lease.

Current component/render-data readers, actual scene/GPU ownership, custom native
feature insertion and deterministic replay export remain unfinished. The current
candidate stays inactive. Source and cached data only were read/edited; no
builds, tests, syntax checks, project/game execution, exports or Bedrock launch
occurred. See inspection/static-native-draw-matrix-layout-1.26.60.5.json.

### Independent skinned-material reader preflight (source only)

Skinned MaterialPtr observation previously required the entire terrain/layer/
define-tree validator to set its shared PROFILE. That left the mapped current
description and callback-thread info owner dependent on unrelated old-address
terrain checks. native_skinned_material_profile now validates the focused
skinned reader independently, with an embedded code-binding catalog selected
only after complete image and scene-family approval. The catalog does not
select or activate an executable-image family.

Both cached families declare their concrete material/control tables, info+0x10
in-place relationship, info+0x40 pointee, three path headers at +0xd0 and the
description's actual MaterialPtr member. The initial eight functions are hashed:
info loader, Destroy, DeleteThis, deleting destructor, shader path parser,
skinned description constructor, material consumer and concrete material copy.
The returned description/consumer bindings must join the same entries, parser
slot 4 and the three exact info-control methods. Reads are bounded to 64 KiB
per function and 128 KiB total; table pointers are compared after rebasing.

The preflight runs before either description or consumer hooks are created,
then caches its result so later dependency installation does not hash already
patched entries. Skinned shared-pair/path copies use this focused accepted
base. Terrain samples keep the existing full validator. The variation migration
below extends the focused preflight to its actual define-tree copy chain;
unreadable variation stays unknown rather than being inferred from paths. Existing info-owner
acquisition, no-resurrection/overflow guards and endpoint rechecks remain in
place. The native unique_ptr material and its descriptors remain borrowed;
the strong info owner is still confined to the native callback thread.

This removes a validation dependency from the source port, not the outstanding
scene/GPU ownership, descriptor contents, phase insertion or deterministic
export work. Current-build activation and runtime parity remain unverified.
Only source and cached data were read/edited. No builds, tests, syntax checks,
project/helper execution, exports, process inspection or Bedrock launch
occurred. See inspection/native-skinned-material-reader-source.json.

### Skinned material defines and state mask (source only)

The cached current material copy at 0x544ad10 takes its set from +0x98 and copies
the adjacent 16-bit state mask from +0xa8. Its real call at 0x544b0ff joins
set-copy 0x6b4570, which allocates a 64-byte sentinel, writes its self links and
color/nil bytes 1/1, and copies count at +8. The call at 0x6b45dc joins recursive
clone 0x118190; that routine preserves the parent, left/right links and color,
and calls node construction 0x118240 with the source string at node+0x20.

The node constructor confirms the 64-byte allocation, 32-byte string header,
length/capacity at node+0x30/+0x38 and inline capacity threshold 16. The older
family has the same field layout through its own separately mapped functions.
No generic shared-tree layout is inferred from a positive pointer or count.

The skinned reader preflight now checks eleven complete function digests and
five distinct direct edges covering material copy, set copy, node construction
and both recursive branches. Each edge must lie within its declared caller
and resolve to its real callee. The variation fields are part of the shared
scene layout and must agree with the embedded catalog before admission.
This permits the bounded define/state reader without terrain-layer validation.
Its digest format, ordering, sentinel/parent/link checks, UTF-8 and size bounds,
and endpoint rechecks stay intact. Invalid variation remains unknown.

Defines and state-mask bytes are declaration metadata, not proof of a compiled
shader, target Java material selection, descriptor contents or GPU ownership.
Scene/resource ownership, real native feature insertion and deterministic replay
export remain unfinished. The current-build candidate stays inactive, and no
runtime parity is claimed. Source/cached-data reads and edits only occurred;
no builds, tests, syntax checks, project/game execution, process inspection,
exports or Bedrock launch occurred. See
inspection/static-material-variation-layout-1.26.60.5.json.

### Selected skin controller/geometry reader (source only)

`native_skin_selection_profile.rs` replaces the selected-pair reader's old-only
instruction sites with two embedded source bindings in
`inspection/skin-selection-reader-bindings.json`. The accepted complete image
and selected scene family must agree first. The reader then joins its root and
controller functions to `model_body_binding` and compares ten bounded instruction
windows before admitting any skin reference. Admission is cached before the body
hook enables; the windows exclude the root entry patched by the transform hook.
The catalog's complete-function hashes are static evidence; this reader checks
the windows, while the body binding separately checks its controller function.

The cached current draw at `0x58b5250` forms params at component `+0x88`, reads
the frame at `+0x368`, traverses `+0x380/+0x388` in `0x28` byte steps, and copies
the controller from entry `+0/+8` and weak geometry from `+0x10/+0x18`. The native
strong-count CAS promotes geometry before `0x58b5eca` dispatches to `0x6b296a0`.
That controller reads geometry `+0xe8` and passes the same geometry as the fifth
body argument at `0x6b2a2a3`. Shared scene constants now drive these reader fields.
Frame/actor identity, unique selection, reference liveness and copy rechecks
remain required. The reader invokes no native method and acquires no scene/GPU
lease. Disassembly windows are retained in
`inspection/static-skin-selection-layout-1.26.60.5.json`.

This completes the selected-pair binding source migration, not every skin
dependency. Current animation/skin identity/name dependencies, equipment and
expression bindings, real submission and rendering ownership still need work.
The current image remains disabled; runtime parity is unverified. No game,
build, test or encoder was run. The separate data-only equipment comparison in
`inspection/static-equipment-functions-1.26.60.5.json` retains four failed full
normalized matches and the unique hierarchy-getter candidate `0x1e87940`.

### Equipment hierarchy-query reader (source only)

`native_hierarchy_query_profile.rs` independently admits the complete 74-byte
getter selected through the accepted image and scene family. Its source catalog
contains old `0x185cd0` and current `0x1e87940`, each with an exact byte hash. Both
form an output span from component begin/end at `+0x398/+0x3a0`, divide their
distance by four, and return the span through the second pointer argument. Only
the external fatal-handler relative address differs after normalization.

`native_attachment_binding::query_hierarchy` now requires the admitted getter in
the actual component's virtual slot `+0x40`. It copies begin/end and the first
bounded nonempty `u32` entry without calling the native method. The surrounding
draw's `Copies` ledger retains the header, table and element readbacks. It no
longer reads an unused capacity field that this getter does not establish.
Admission precedes the remaining attachment guards; those guards still apply.
`inspection/native-hierarchy-query-reader-source.json` retains the full cached
getter disassembly and explicitly records incomplete equipment migration and
unverified runtime parity. This change does not activate the current build.

### Equipment expression observer entries (source only)

The setup and evaluator source bindings now select an exact scene family from
`inspection/expression-observer-bindings.json`. `native_expression_profile.rs`
hashes the complete setup, wrapper and 17-byte evaluator leaf, then checks three
real E8 edges joining the accepted root draw to wrapper, setup and evaluator.
These checks run before detours and cache their result. Other equipment readers
retain their independent guards.

The old chain includes wrapper `0x1c386b0`, setup `0x1c38c50`, evaluator
`0x7c9e00`, and pinned return `0x1c3a2d9`. Its setup reserves `0x1b8` bytes.
The current chain includes wrapper `0x58b7520`, setup `0x58b7ac0`, evaluator
`0x86cae0`, and pinned return `0x58b8f75`; its setup reserves `0x1c8` bytes.
Both wrappers pass five pointers and two byte flags. The current parent hierarchy
node is held in R12 rather than old RBX; the destination bone stays in RSI.
The current leaf shim supplies R12/RSI only for that pinned return address.
Every other dispatcher caller jumps to the original trampoline without changing
its argument registers, vector arguments, stack arguments or return address.

Both trampolines are published before hook enablement. Observation starts only
after both hooks enable successfully; partial enablement forwards through native
originals. Disabled nested setups still quarantine outer pending observations.
No expression is evaluated twice, and the existing owned-result, copy readback
and completed native matrix-copy sealing requirements remain. Evidence and
unverified status are recorded in
`inspection/static-expression-observer-layout-1.26.60.5.json` and
`inspection/native-expression-observer-source.json`. The current scene remains
disabled; this does not complete native equipment rendering.

### Constant-expression stored hash reader (source only)

`native_molang_constant_profile.rs` selects the concrete sixteen-method class
table from an accepted image and scene family, compares its rebased entries,
hashes its complete clone, joins the clone's RIP-relative table assignment and
checks that its generic evaluator returns `object+8`. Its source catalog is
`inspection/constant-expression-reader-bindings.json`.

The current table `0xf1c7480` was the sole cached .rdata candidate satisfying the
old table's repeated method relationships and generic reader. Its slot-one clone
`0x3f65c60` independently installs that table, allocates `0x68` bytes and copies
the 32-bit tag at `+8` and a non-string POD value as eight bytes at `+0x10`.
The current generic evaluator is `0x79570`. The hash reader admits only tag 1 and
the exact concrete table; it copies the stored hash without invoking either
native method. Class admission now precedes any hash copy and uses shared scene
constants. The caller must still retain and recheck the native owner and bytes
within its synchronous draw.

Clone allocation fallback and string branches changed, so the failed full
normalized match is retained in
`inspection/static-constant-expression-clone-1.26.60.5.json`.
`inspection/native-constant-expression-reader-source.json` contains complete
cached clone/evaluator disassembly and the actual table join. No whole-function
equivalence, runtime parity or current-build activation is claimed. No game,
build, test or encoder was run for either migration.

### Equipment metadata and parent-bone reader family (source only)

`native_attachment_profile.rs` replaces the remaining old-only equipment byte
sites with `inspection/attachment-reader-bindings.json`. After exact image and
scene-family acceptance it uses the expression observer's cached setup preflight,
hashes the complete equipment draw wrapper, draw and child shared-pointer getter,
and checks four actual call/tail-jump edges. Root draw and body controller must
agree with their independent bindings. The preflight enables no hook and is
cached before later expression/body detours.

The current root draw calls wrapper `0x58b7880` at `0x58b602e`. That wrapper
tail-jumps at `0x58b7a20` to equipment draw `0x58bdad0`. The draw reads the root
child vector at `+0x300`, computes `slot*0x50`, and calls child getter `0x618b050`
at `0x58bdc9b`. That getter returns the shared-pointer slot at entry `+0x40`.
The equipment draw reads child selected pairs at `+0x380/+0x388`, forms params
at child `+0x88`, writes the equipment slot at child `+0x284` (params `+0x1fc`),
and calls controller `0x6b296a0` at `0x58be1a6`.

Equipment reader admission now uses two call return identities derived from
validated E8 instructions: current body return `0x6b2a2a8` and equipment return
`0x58be1ab`. The old pair `0x1cb8b24/0x1c3f5fa` remains derivable for its own
family; neither is hard-coded in the reader. This preserves caller identity
checks while removing another old-build assumption. The selected body's return
is exposed by `ModelBodyBinding` and in body status.

Shared scene constants now drive both equipment and expression child-slot/bone
readers. Cached current indexed-binding code uses method 2 at bone `+0xe4`,
parent hierarchy at `+4`, bone index at `+0`, and source stride `0xe8`; it copies
the parent matrix at `+0x38` and writes override flag `+0xe6`. Existing slot and
child uniqueness, reference liveness, actor/model/context identity, frame,
readback, source topology and completed dynamic-copy requirements remain.

Evidence is retained in `inspection/static-attachment-reader-layout-1.26.60.5.json`
and `inspection/native-attachment-reader-source.json`. This source migration does
not establish every animation/scene dependency, native custom-geometry insertion,
exclusive feature phase or actual resource/descriptor ownership. The current
scene stays disabled and native equipment rendering remains unverified. No game,
build, test or encoder was run.

### Encoder submission and frame-item reader family (source only)

`native_submit_profile.rs` binds the passive six-argument encoder observer and
its item/counter reader as one exact scene family using
`inspection/encoder-submit-bindings.json`. It hashes the complete encoder and
one native caller before the detour, bounds the caller's E8 instruction, and
checks that it targets the selected encoder. Shared scene constants now select
the frame item array, allocation counter, successful encoder submission counter,
128-byte item stride, count fields and capacity together with that entry.

Old encoder `0xb722470` is called by `0x5319ff0` at `0x531a0df`; the mapped
current encoder `0xbc5fb40` is called by `0x5699be0` at `0x5699ca9`.
Both reserve a bounded index through a compare/exchange loop, increment their
successful submission counter, and copy eight 16-byte blocks from encoder
`+0x40` to that frame's indexed RenderItem. Current allocation counter
`+0xdc57e8`, item array `+0xb4700`, and successful submission counter `+0x68c`
replace old `+0xdd17e8`, `+0xc0700`, and `+0x54c`. Current rejection counter
`+0x690` is separate and is not used to attest a successful submission. Native
count checks at encoder `+0x84/+0x8c` corroborate item `+0x44/+0x4c`.

The caller and callee establish encoder pointer, three `u16` handles, `u32`
depth and `u8` flags. Six saved integer registers and `0x68` locals put the two
stack arguments at callee RSP `+0xc0/+0xc8`, corresponding to entry
`+0x28/+0x30`. All six arguments continue through the original exactly once.
The original trampoline is published before enablement; callbacks during
partial enablement only forward, and installed status requires successful
enablement. Existing single-allocation/single-submission, exact frame, slot,
scope and age checks remain. The existing packet ownership fixture now uses
the selected layout constants and was not executed.

The initial normalized entry match and eleven normalized caller matches failed;
their failures are retained. Discovery used mnemonic similarity only to select
a candidate, then independently joined its thirteen decoded direct callers,
real argument registers/stack slots and concrete item/counter instructions.
Evidence is in `inspection/static-encoder-submit-layout-1.26.60.5.json` and
`inspection/native-encoder-submit-source.json`.

The current producer Context/frame reader and backend consumer remain separate
migrations. This observer does not insert custom geometry, establish an
exclusive native feature phase or acquire resource/descriptor ownership. The
current scene stays disabled and runtime parity remains unverified. No game,
build, test or encoder process was run.

### Producer Context and embedded-frame reader (source only)

`native_producer_frame_profile.rs` independently admits the producer metadata
reader through the accepted complete image and selected scene family. Its source
catalog `inspection/producer-frame-reader-bindings.json` contains five full
function hashes per family: Context frame step, actual swap, Context constructor,
Frame constructor and a Context-global accessor. Three real E8 edges join frame
step to swap and the Context constructor to both embedded Frame constructions.
Two bounded RIP-relative native loads must select the same Context global.

The current global is image RVA `0x12adf748`, primary encoder pointer is Context
`+0x13d18`, render/submit are `+0x1cfd0c0/+0x1cfd0c8`, and the two embedded
Frames start at `+0x13d40/+0xe88700`, establishing stride `0xe749c0`. The pair
ends exactly at the render pointer field, followed by submit eight bytes later.
The current frame number is `+0xe746d8`, cleared by Frame constructor
`0xbc76dd0` and incremented in swap `0xbc661e0`. Field `+0xe746d0` is timing
data and is not a frame identity. Context frame step `0xbc64c00` calls that swap
and rebinds encoder `+0` to the new actual submit pointer afterward.

For the old family, actual swap is `0xb728a90`; the former source comment naming
`0xb7278d0` as swap was inaccurate. This correction comes from the frame step's
actual direct call. Constructor and frame-step evidence also show why a uniform
offset adjustment would have been wrong: the encoder pointer advances by eight,
frame storage shrinks and the number moves differently from neighboring timing
data. Static windows and function hashes are retained in
`inspection/static-producer-frame-layout-1.26.60.5.json`.

`render_bridge::install_producer` validates these readers without a detour. Both
backend installation and native encoder observation require this preflight.
`producer_frame` now uses the independent admitted base, requires distinct
render/submit pointers from the actual embedded pair, requires encoder `+0` to
match submit, and rechecks Context/global/pointers/number before returning the
copied FrameKey. Backend frame metadata similarly rechecks global and number.
Frame-number reads in scope matching, diagnostics and existing fixtures use the
shared selected field. Fixtures were edited as source and were not executed.

These metadata relationships do not establish an exclusive feature phase or
native/GPU object lifetime. Current View layout, backend detours, real resource
ownership and native custom geometry insertion remain independent work. The
current scene stays disabled and runtime parity remains unverified, as recorded
in `inspection/native-producer-frame-reader-source.json`. No game, build, test
or encoder process was run.

### Native View record reader and observation strategy (source only)

`native_view_profile.rs` admits the selected View record family using full native
writer/frame-copy/optional-setter hashes in `inspection/view-reader-bindings.json`.
Its frame-copy function must equal the independently admitted producer swap.
Actual E8 edges join old writer to setter and both families to their native copy
targets. Reader field bounds, record count/stride and observation strategy must
agree with shared scene constants before any backend detour is installed.

The old swap copies `0x20000` bytes from Context `+0x1ec3080` into submit Frame
`+0x300`: 256 records of 512 bytes. Current swap `0xbc661e0` performs the same
transfer at `0xbc66781` from Context `+0x1eab080`, but with `0x14000` bytes:
256 records of 320 bytes. Current mapped writer `0x56ae6f0` writes packed viewport
coordinates at record `+0x10`, copies base view/projection matrices at
`+0x20/+0x60`, and copies additional native data at `+0xa0/+0xe0/+0x120`.
The reader interprets only viewport and base view/projection. Current frame
dimensions are `+0xdc5808/+0xdc580c`.

Old writer `0x53318e0` calls eight-argument setter `0xb737cb0`, whose four extra
pointer stack arguments remain preserved by the old-family typed observer.
Current mapped writers inline their copies. This evidence establishes no
current standalone setter entry or signature. The current source strategy
therefore observes actual native frame View snapshots; it installs no invented
setter hook and records snapshot reads separately from setter invocation counts.
The old setter observer remains enabled only for its exact family and reports
installed only after hook enablement succeeds.

`scope_for_view` now requires an admitted View reader, copies the bounded record
prefix containing viewport/matrices, reads it back, and rechecks dimensions,
frame number, snapshot presence and the world-frame scope/matrices/ambiguity
before matching a world scope. Diagnostic
array copies similarly use the selected stride and require complete readback
before publication. Matrix parsing reads owned copies and rejects non-finite
values; these readbacks do not claim atomic cross-thread capture or a native
lifetime lease. Existing forwarding/view fixtures were adapted as source and
were not executed.

Evidence and limitations are retained in
`inspection/static-view-reader-layout-1.26.60.5.json` and
`inspection/native-view-reader-source.json`. Current backend entry/packet readers,
actual feature-phase insertion and resource/descriptor ownership remain work.
View equality does not establish an exclusive native phase. The current scene
stays disabled and runtime parity remains unverified. No game, build, test or
encoder process was run.

### Backend submit and batch reset/fill reader family (source only)

`native_backend_profile.rs` admits the backend submit, its sole direct caller,
batch reset and vertex-fill functions from
`inspection/backend-batch-bindings.json`. It checks complete function hashes,
five real E8 call edges and the selected stack/packet layout before detours.
The binding is shared with `native_batch`, so later installation does not rehash
the already-patched backend. Every function remains within accepted code bounds;
the four-function evidence budget is 64 KiB.

Current submit `0xe820520..0xe82c8e3` is called by `0xe83b790` at `0xe83b830`.
Both families preserve six pointer-width arguments: the third argument is Frame,
saved at backend stack `+0x168`, and the fifth/sixth are read at entry stack
`+0x28/+0x30`. Current local allocation `0xe238` replaces old `0x1a238`; the
`0xc000` reduction agrees with the native View array reduction. The observer
forwards all six arguments exactly once and reports installed only after actual
enablement. The complete backend group reports ready only after its dependent
observers have installed; cached failures are not retried against partial hooks.

Old reset `0xdf86090` consumes this, Frame and a byte stereo flag. Current reset
`0xe7fc420` consumes this and Frame; the old third-argument branch is absent.
Separate typed callbacks preserve each ABI. Current submit supplies the reset
object at backend-stack `+0x10a0`, calls reset at `0xe820647`, and stores the
view index at stack `+0x1e8`. Old offsets were `+0x1090/+0x1a8`. Reset observation
requires the return address derived from the validated actual E8 call, checks
Frame identity/number, retires prior bindings/views and preserves overflow poison.
The derived stack pointer stays inside that enclosing backend call.

Current fill `0xe81a8d0` preserves four pointer arguments and its full `u32`
return. Its three actual backend callers distinguish temporary stack output from
non-indexed and indexed array packets. The actual non-indexed allocation still
uses batch count/array `+0x90/+0x98`, stride `0x70`, CBV/CBF `+0x50/+0x58`.
Indexed uses `+0x94/+0xa0`, stride `0x88`, CBV/CBF `+0x60/+0x68`. The caller
writes both addresses before fill. The reader requires the actual last allocated
array slot and rejects temporary output. Frame number, view index, draw handle,
count/array, CBV/CBF and command-list pointer are rechecked before recording a
match. Batch observation starts only after both trampolines are published and
both hooks enable; partial enablement forwards native calls without observation.

Source evidence and limitations are retained in
`inspection/static-backend-batch-layout-1.26.60.5.json` and
`inspection/native-backend-batch-source.json`. Existing slot/frame/forwarding
fixtures were adapted as source and were not run. Current framebuffer and other
graphics-state readers still need migration. These passive metadata joins do
not supply exclusive feature-phase insertion or native/GPU lifetime ownership.
The current scene stays disabled and runtime parity remains unverified. No game,
build, test or encoder process was run.

### Framebuffer setter and paged texture reader family (source only)

`native_framebuffer_profile.rs` replaces the old-only framebuffer sites with
`inspection/framebuffer-reader-bindings.json`. It hashes the complete setter,
transition helper, texture-page ensure routine, texture cleanup and external
resource override, then checks the actual backend call and ten internal E8
edges. The backend caller is joined to the independently admitted backend
binding and bounded by that function's actual extent. `render_bridge::install`
performs this preflight before backend detours; framebuffer installation later
reuses the cached admission rather than rehashing patched entries.

Current setter `0xe81fd10..0xe82051c` is called by the backend at `0xe821188`.
Both setter prologues consume two pointer registers and both R8D/R9D DWORD
inputs. The typed observer preserves all DWORD handle/flag bits, interpreting
only the low word/byte for matching. Descriptor-pointer fields `+0x10/+0x18`,
stored handle `+0x20`, flag `+0x2c`, command list `+0x30` and state cache
`+0x14458` retain their current layouts. Current OM binding calls use the actual
list, descriptor pointers, target count and single-range flag; those observed
arguments still have to match the captured native context before handoff.

The current framebuffer table moves to renderer `+0xc97698` from `+0xc97690`;
texture registry moves to `+0xbde178` from `+0xbde170`. Framebuffer records keep
stride `0x68`, count byte `+0x32` and depth handle `+0x10`. Texture records keep
stride `0xb8` and 4096 entries per inline/extension page. The page-root/link
offset is `0xb8000`, byte extent `0xb8008` and mode byte `0xb8018`. Source copies
retain their bounds, cycle checks, uniqueness and before/after rechecks. Resource
copies now require the admitted framebuffer reader family.

Current cleanup `0xe80e690` tests texture `+0x9b` bit `0x40` before bypassing its
ordinary retirement path. Current override `0xe815030` calls that cleanup, sets
the bit and stores its input at texture `+0`. The whole flag byte remains guarded
with the resource pointer. These private routines are read as evidence and are
never invoked as acquisition or lease APIs. Only actual public barrier resource
arguments are cloned as COM references; registry pointers are never invoked as
COM objects. Existing public-descriptor, resource-state, device, frame and epoch
checks remain.

The framebuffer callback forwards during partial enablement, publishes its
original before enabling, and reports installed only after actual enablement.
Cached failures remain failures. Evidence and explicit ownership limits are in
`inspection/static-framebuffer-reader-layout-1.26.60.5.json` and
`inspection/native-framebuffer-reader-source.json`. Remaining graphics/queue
readers, real backing/native ownership, exclusive feature-phase insertion and
replay rendering/export are still work. The current scene stays disabled and
runtime parity remains unverified. No game, build, test or encoder process ran.

### Seven-pointer body/cape/emote draw entry (source only)

The old Model::dataDrivenDraw entry at 0x1c44f70 has a current candidate at
0x6b801b0. The full normalized-byte matcher fails and remains recorded as such.
An independent cached-code join follows the real root model draw's direct call
to geometry/controller draw: old 0x1c36c8e -> 0x1cb7f00, current 0x58b5eca ->
0x6b296a0. Each controller has one decoded direct call to its corresponding
body entry, at 0x1cb8b1f / 0x6b2a2a3.

Both callers resolve the model from params+0x20 while retaining its control at
+0x28, pass metadata/controller in R8/R9, and put geometry/textures/MERS pointer
arguments in outgoing stack +0x20/+0x28/+0x30. Both callees consume the same seven
pointer slots, use the 0x58-byte local reservation, iterate geometry groups at
+0x88/+0x90 with 0x60-byte records and 0x10-byte mesh pairs, and bracket work with
the model's +0xb0/+0xb8 virtual methods. The current null-controller branch
inlines the older helper's mesh traversal before its fatal path; no equality of
those complete native bodies is asserted.

native_model_body_profile checks the exact body and controller function hashes,
both real E8 edges and their caller extents, shared ABI/layout label and all
26 complete prefix bytes. The result is cached before patching. native_body
now admits its own binding before dependency installation, preserves every
original argument once, and publishes installed=true only after actual hook
enablement. Geometry name/hierarchy and partial reads use shared scene members.
Existing skin/equipment/expression/submission guards remain in place; selecting
this body entry does not migrate those readers or enable the current image.

Live scene/GPU ownership, actual custom-feature insertion and deterministic
replay export remain unfinished. Runtime parity is unverified. Only source and
cached data were read/edited; no builds, tests, syntax checks, project/game
execution, exports, process inspection or Bedrock launch occurred. See
inspection/native-model-body-hook-source.json.

### Skinned description constructor migration (static source port)

Cached 1.26.60.5 caller 0x6b84260 builds current/previous bone spans and calls
0x668c750 at 0x6b85a16. Its quad, triangle and mesh material error paths join
the older caller independently of the failed whole-function matcher. Both
constructors retain all 21 Windows x64 argument slots with the same observed
byte, word, dword and pointer widths, and return the original this pointer.
Both callers construct each span as (end-begin)/64 followed by the original
begin pointer; both constructors copy those 16 bytes to output+0xc0/+0xd0.
That copy does not retain ownership of the underlying matrix allocation.

The material pair moves from +0x168/+0x170 to +0x140/+0x148; color moves from
+0x17c to +0x154, and dithering moves from +0x1fe to +0x1ce. These are not one
uniform member delta. The source now shares the selected family's mesh,
draw-count, bone-span, material, color, alpha and dithering members between
the constructor, material capture, emote capture, palette receipts and shader
consumer. The constructor binding checks its declared frame against the scene
family as well as its exact direct-call witness and full prologue. It rejects
an incompatible constructor before installing its dependent hooks. Installed
status follows successful hook enablement, not trampoline creation.

The current candidate remains inactive: its dependent cape, emote, material
consumer and scene ownership joins are not completely migrated. The approved
family remains 1.26.5203.0 with its complete text hash gate. No builds, tests,
syntax checks, project/game execution or Bedrock launch were performed.
See inspection/static-skinned-description-layout-1.26.60.5.json for cached
function hashes, argument loads, caller assembly and member-store bytes.

### Palette writer previous-input migration (static source port)

The current native palette writer is the leaf function 0x6b87e10..0x6b8806b.
Its unique 21-byte prefix, ordered current/previous matrix stores and actual
calls at 0x589e7b7/0x589ed37 join the older 0x12ee7f0 writer. The new leaf has
no .pdata entry; its return and following INT3 padding delimit the recorded
bytes. Both writers retain the seven argument widths and 64-byte matrices.
The owner vectors move from +0x510/+0x528 to +0x4e0/+0x4f8, while the native
draw-count store moves from +0x558 to +0x528.

Argument seven's pointee changed independently: the older writer reads an
inline matrix and a presence byte at +64; the current writer reads a presence
byte at +8, then follows the first pointer only when present. The observer now
supports both explicit forms. It guards the selected presence/header bytes,
the current input and any indirect previous matrix across the native call.
It copies the current pose for an absent previous input, matching the writer.
Its sealed receipt keeps owned matrix bytes and drops borrowed input addresses
after the synchronous call. Palette receipts and writer observations now share
the selected scene family's two explicit vector offsets instead of assuming
one contiguous block.

Installation resolves a profile-selected entry and exact direct-call witness,
checks all 27 prologue bytes and requires the previous-input form to agree
with the scene family. Installed status requires successful hook enablement.
The approved family remains old; the current candidate is not activated and
other part, cache and upload readers still require migration. No build, test,
syntax check, game/project execution or Bedrock launch occurred. See
inspection/static-palette-writer-layout-1.26.60.5.json.

### Skinned material consumer migration (static source port)

The current profiler function string and record join BgfxFrameBuilder::_insert
to 0x6645ae0. Its visitor 0x6645d80 still reads the variant tag at +8 and its
description pointer at +0; tag seven remains the skinned case. The actual
constructor store establishes table 0xf2bce70, whose insertion entry is now
+0x3b8 instead of the older table's +0x358. Assuming the old virtual slot would
select a different method.

The visitor copies its skinned input prefix at RBP+0x5a0 and passes it in R8
at call 0x664abbc to 0x66cfe00, retaining the helper's five pointer arguments.
The 184-byte prefix and vector field ordering remain the same; the helper's
stack allocation moves from 0xed8 to 0xe38. Its opening description reads
independently corroborate material+0x140 and dithering+0x1ce. These witnesses
do not pin the replaceable nested RenderMaterial or establish GPU lifetime.

The source now resolves the consumer entry, virtual slot and material helper
from the exact profile, couples the vtable/input layout to the selected scene
family and validates the helper's real direct caller. It checks both full
19-byte prologues before creating either hook, publishes both trampolines,
enables the material helper first and rolls it back if insertion enablement
fails. The current candidates remain inactive under the complete image/family
gate. No tests, builds, syntax checks, game/project execution or Bedrock launch
occurred. See inspection/static-skinned-material-consumer-layout-1.26.60.5.json.

### Owned matrix packer migration (static source port)

The skinned helper's direct call at 0x66d2577 joins the current packer
0x6697460..0x66976a0. It retains the two pointer arguments, count/pointer input
span and 64-byte matrices. Its stores still copy every supplied matrix and
pad to eight with exact zeroes and float-one diagonal entries. Register
allocation in the padding loop and the cookie/error path changed, so the
whole-function matcher failed; the recorded instruction comparison remains
independent evidence rather than a successful byte match.

The upload hook now resolves its exact-profile entry and real material call,
declares the matrix/header/output layout explicitly and checks a complete
25-byte prefix. Its RIP-relative security cookie reference uses the selected
profile's address, instead of embedding the older displacement. It preserves
the original native copy/padding call before guarded changes to owned output.
Installed status requires successful hook enablement. This source port does
not activate the current image or migrate the separate cache producer,
predicate, erase, prepare, lookup and lifetime readers. Scene/GPU ownership,
live feature insertion and deterministic export still require completion.
No builds, tests, syntax checks, project/game execution or Bedrock launch
occurred. See inspection/static-palette-pack-layout-1.26.60.5.json.

### Flat emote cache and previous-pose provenance (static source port)

Current prepare 0x589eb00 calls predicate 0x589f280 and writer 0x6b87e10.
It no longer performs the older tree's three lookup calls. It searches sorted
composite keys at Actor+0x120 with stride 0x38 and selects the corresponding
64-byte matrix from the parallel vector at +0x138. Keys compare unsigned hash,
string bytes, group byte at +0x30 and palette byte at +0x31. A cache miss uses
the current bone's matrix at boneSpan.pointer + part.boneIndex*0xe8 + 0x38.
The optional reference passed to the writer points directly to that selected
matrix. Current producer candidate 0x589f3f0 calls the same predicate and
recurses into itself; its storage mutation and lifetime hooks need migration.

native_emote_cache_flat ports a bounded reader for those parallel vectors.
It requires matching counts, nonoverlapping key/value storage, strictly sorted
unique composite keys and stable copied headers, keys, strings and selected
matrix bytes. A validated missing key remains distinct from a failed read.
The existing copy/guard budgets apply, with at most 1024 flat entries and a
1 MiB retained consumer budget per thread. The flat reader requires both its
exact product version and the selected flat scene family.

The consumer captures the lookup before the actual native predicate, retains
its guards through writer admission and requires the writer's actual source
pointer to match the selected cache matrix or verified current bone fallback.
Equal matrix bytes alone cannot establish historical provenance. History uses
a distinct flat-layout tag and opaque backing identities; no new native
address is archived or retained in persistent history. Borrowed lookup guards
leave with the synchronous consumer scope.

The approved scene family remains the older tree. Selecting flat storage
rejects cache hook installation until producer, clear, reallocation and
destruction coverage is migrated; pointer checks do not provide that lifetime
coverage. The old five-hook observer now preflights every entry before any
creation, publishes every original before enablement, rolls enabled entries
back on failure and admits observations only after complete enablement.
Installed status reflects that complete enablement, not trampoline allocation.
These source changes do not activate the installed build or prove live emote,
cosmetics, replay or video parity. No builds, tests, syntax checks, project/game
execution or Bedrock launch occurred. See
inspection/static-emote-cache-layout-1.26.60.5.json.

### Actor and cache destruction observers (static source port)

Four independent virtual methods match the current actor table at 0xf0a8d70.
Both current constructors store that table, and destructor 0xb93420 restores
it before releasing component/cache storage. Table slot eight selects deleting
destructor 0xcc2290, whose actual call at +11 targets 0xb93420. It preserves
the original this argument, conditionally frees the native allocation and
returns this. The destructor itself retains a one-pointer void ABI. The old
and current object allocations are 0x3b0 and 0x3d0 bytes respectively.

Current actor destruction frees the matrix vector at Actor+0x138 and then
destroys the key vector at +0x120. Standalone cache destructor 0xb92330,
also called during constructor cleanup, releases its values vector at +0x18
and tail-dispatches the key-vector cleanup. That generic key cleanup has many
unrelated callers; it is not used as an actor-specific hook.

native_emote_cache_lifetime now invalidates that owner's pose history and
current-thread frame-upload receipts before
forwarding either actual actor or cache destructor. It compares only opaque
history identities, without dereferencing partially destroyed objects or
requiring a current draw scope. Both typed originals are published before
enablement. Installation validates the complete destructor prefixes, the
actual direct callers, the actor table's deleting-destructor slot and its
wrapper prefix; both entries preflight before creation and roll back on
enable failure. Cache observation admission requires successful installation
of these destruction observers first.

Shared selected-family actor members now cover the cache offset, level and
animation component used by cache binding/history. The current constructor
stores its level at +0x200, replacing +0x1d8, while the animation component
moves from +0x270 to +0x290. These fields do not share one offset delta.

This ports the destruction boundaries, not complete cache mutation coverage
or an actor/GPU ownership lease. Current producer, clear/reallocation paths
and other actor/component readers still require migration. The approved family
remains old and the current candidates remain inactive. No builds, tests,
syntax checks, project/game execution or Bedrock launch occurred. See
inspection/static-actor-cache-lifetime-1.26.60.5.json and the separate virtual
method comparison in inspection/static-actor-table-functions-1.26.60.5.json.

A separate bounded paired-header search covers nearby actor-method entries
and the known prepare/producer. Its candidates are not identified mutation
hooks: one reviewed function uses the same displacements for an unrelated
string reached through a virtual-call result and nested pointer. The report
records that rejection and explicitly leaves global mutation coverage
incomplete. See inspection/static-cache-owner-access-candidates-1.26.60.5.json.

### Cache producer/preparer profile forwarding (static source port)

The cache observer now resolves producer, predicate and preparer entries from
the approved exact-image profile. Its five-pointer producer, three-pointer
byte-return predicate and five-pointer-plus-history-byte preparer retain the
original native arguments and returns. The real recursive producer/preparer
calls and producer-to-predicate call must reference the declared entries.
Each family's complete fixed prologue is checked before hook creation.

Caller provenance also belongs to the selected profile. The tree family
requires distinct fallback/cached predicate returns and three ascending real
lookup-call returns. Its erase and lookup hooks are selected only for that
family. The flat family instead requires one real predicate return and an
empty legacy lookup list. Both require the preparer's actual call to the
selected palette writer. The reader no longer embeds current caller RVAs;
the old callback RVAs and return filters are likewise removed from the
observer. Profile, storage and scene-family checks precede admission.

The exact profile migration retains grouped preflight, destruction-observer
installation, publication of every original before enablement, rollback on
partial enablement and observation admission only after complete enablement.
Status reports selected binding data rather than stale fixed addresses.

The current matrix reallocation helper is 0x5907980, called at 0x589fe96 with
the values-vector header at Actor+0x138 and the insertion-position pointer.
It inserts an identity matrix, copies existing matrices, frees old storage,
publishes the new header and returns the new insertion pointer. The producer
then writes the selected bone matrix. Key insertion precedes matrix insertion;
stable equal key/value counts cannot be assumed inside that helper. No
reallocation hook is installed by this source port. Mutation tracking and
other current component/ownership readers remain unfinished; current-build
activation remains disabled. No builds, tests, syntax checks, project/game
execution or Bedrock launch occurred. See
inspection/native-emote-cache-bindings-source.json.

### Flat-cache matrix relocation receipts (static source port)

native_emote_cache_relocation now supplies the source observer for the actual
0x5907980 values-vector insertion/reallocation entry. Its exact profile must
declare the two-pointer, pointer-return ABI, match all 13 prefix bytes and
resolve the real direct call returning at 0x589fe9b. The entry joins the same
grouped hook preflight, original publication and enable/rollback path as the
producer. Current-build activation remains disabled.

Admission requires the active native producer event, its observed nonzero raw
predicate byte and pending ticket, the matching logical inserted slot, current
actor scope/epoch and the values header at Actor+0x138. At this boundary the
key count must equal the old matrix count plus one; the old matrix vector must
be full and the insertion iterator aligned and within its extent. The shared
bounded key copier checks the complete ordered composite-key vector.

Before forwarding the original once, the observer copies old matrix bytes and
seals eligible committed history records by owner, epoch, logical slot, old
matrix identity, native bits, serial, frame and prior witness. It never reads
the freed old allocation after the original returns. The result must identify
the new insertion cell; both copied ranges and the inserted exact identity
matrix must match. Key metadata, the new matrix bytes, header and producer
context are rechecked before replacing any historical storage witness.

Successful relocation retains native/supplied pose values and every recorded
emote/replay clock, serial and frame. Pending/conflicted records are ineligible;
changed/missing observations revoke owner eligibility. A failed sealed ticket
invalidates its owner on drop. History retains only opaque storage identities,
not the borrowed matrix pointers. Producer/relocation guards stay on their
callback thread, and relocation refuses concurrent producers on other threads.
No history lock spans the native call. The input snapshot limit is 128 KiB;
retention across nested callbacks, including completion buffers, is limited to
4 MiB per thread, shared with the producer-wide storage journal. Repeated
guards perform additional bounded reads.

The native key has no render-context field. Cache write reservation now revokes
another context's eligibility for the same owner/slot, including equal native
matrix bytes, and conflicts pending foreign-context writes. Verified storage
movement may preserve unchanged slots from different contexts without merging
their keys or recalculating their clock data.

This covers the one identified values reallocator in source. The producer-wide
journal described below adds key-only movement and in-place insertion handling.
Clear/assignment coverage, current component readers, live scene/GPU insertion
and deterministic video remain unfinished. No builds,
tests, syntax checks, project/game execution or Bedrock launch occurred. See
inspection/native-emote-cache-relocation-source.json.

### Presentation epoch and recorded shader clocks (source only)

Cached PE disassembly locates the old 0x66d030 presentation helper and current
0x72c030 candidate. Both convert QPC into integer nanoseconds and subtract a
lazily initialized game epoch; the current shader caller then converts to
double and divides by 1,000,000,000. The full normalized-function matcher failed:
the reviewed mov/or operand ordering differs in a commutative high-word test.
This is a static semantic witness, not byte identity or current-build support.
See inspection/static-presentation-clock-layout-1.26.60.5.json and retain the
separate failed static-presentation-clock-function report.

native_presentation_clock reads the approved exact-image epoch only after its
static guard has completed initialization, and samples public Win32 QPC APIs.
It does not call the private helper or force its initialization. Binding checks
the helper's actual storage references and shader caller. Changed storage/domain,
uninitialized guards, invalid values and arithmetic overflow suppress sampling.
The current candidate stays inactive under the complete image/layout gate.

Completed recording markers retain this presentation sample. Shader observation
schema 2 requires its total input between two public samples in that domain,
and world publication rechecks the domain. Java retains the immutable domain
alongside the native CPU observation; legacy schema 1 remains readable.

The initial completed-marker/world-frame association now includes an optional
presentation mapping. Its three-second freshness/domain checks and explicit
alignmentVerified=false remain separate from the existing simulation-based
mappedStartTimeMs. The full receipt passes through the native/JNI path and the
existing AppearanceHistory manifest source field without filtering that data.

ShaderClockHistory adds bedrock_shader_clock_v1 to the archive through the
existing bounded writer and hash/index machinery. Samples retain native total,
delta, world phase/skip and override values, tick/partial/elapsed, observation
ID, world sequence and scope. Presentation timestamps remain distinct from
simulation time, including held ticks and repeated passes of one render frame.
Equal-time samples stay ordered in one bounded group; an excessive group fails
explicitly. The final nextUp descriptor bound is exclusive index coverage and
does not invent a sample. Old archives without a presentation mapping receive
no synthetic shader track.

Replay loading checks every block and its exact mapped timestamp bits, domain,
session and monotonic clocks, with bounded inflation and a two-block cache.
The reader exposes all observations at an exact presentation instant and never
interpolates or holds a phase through a missing frame. Its presentation endpoints
are reported separately from the simulation appearance duration. Recording
status reports missing observations and mapping availability.

This completes source recording/reading of these CPU observations, not native
phase restoration at arbitrary export times. Live cosmetics insertion, scene
ownership/material bindings, the complete installed-build layout family, GPU
equivalence and deterministic MP4 parity remain unfinished. All phase-applied,
alignment, runtime and video verification flags remain false. No builds, tests,
syntax checks, project/game execution or Bedrock launch were performed. See
inspection/native-shader-clock-history-source.json.

### Minecraft timer/session and renderer counter joins (static source port)

The short simulation-pause getter has one matching current suffix and moves
its timer load from Minecraft+0xe0 to +0xe8. Its adjacent scale method uses
the same pointer and writes Timer+0xc. Their concrete vtables join old
0xe7ca5b0 to current 0xf0a8920, with pause getter slot 5 and session predicate
slot 6. The latter independently moves session+0xc8 to +0xd0 while retaining
the ready flag, active owner/byte and level reads at +0x28/+0x30/+0x40.

The vtable constructor joins are 0x19d62f0 and 0xb80b30. Both explicitly copy
arguments+0x40 and arguments+0x48 into their two timer references, placing the
current references at +0xe8 and +0xf0. This establishes member candidates
without assuming every Minecraft member moves by eight bytes. It does not
establish the complete primary-client factory path or timer alias lifetime.

The GameRenderer counter increment at old 0x47af97c (member +0x220) joins
current 0x5316055 (member +0x238), inside candidate producer 0x5315cc0.
Its actual caller at 0x65a05ca obtains the renderer through the concrete
client table's slot 184, getter 0x65b13b0, then the game's +0x360 virtual slot
and getter 0x8ac4e0. The old chain uses client slot 186 and getter 0x5dc3710,
then game getter 0x80c4d0. This independently identifies the target renderer
object. The increment follows an indirect dispatch; equivalence of this integer
to a completed simulation/client/world tick remains unproven.

The source scene family now declares the approved old Minecraft table/slots
and session fields. live_client compares the method/table addresses without
calling them, and rechecks the primary client, Minecraft, session, timer
references, active owner/byte, ready flag and level before returning copied
values. A changed graph suppresses the sample; these checks do not acquire a
native ownership lease. Existing unbound data fixtures retain their old path,
and no fixtures were executed or new tests added.

Current candidates remain inactive. Complete current ECS/camera/resource
layouts, primary factory/ownership joins, native cosmetics insertion and
runtime/video evidence are still required. No game/process/project code,
builds, tests or syntax checks ran, and Bedrock stayed closed. Byte witnesses,
function hashes, table entries and the remaining limits are retained in
inspection/static-minecraft-timer-layout-1.26.60.5.json.

### ECS and native camera callback migration (source only)

The actor-type reader joins old 0x19f1e90 to current 0xb9cab0 and the weak
entity liveness routine joins 0x25c0380 to 0x2b96a10. Their complete normalized
instruction bytes retain the registry/storage offsets, entity generation masks
and paging shifts. Only external address operands are excluded from that byte
comparison. The source adapter continues to read the checked paths itself;
it does not invoke native throwing getters or the weak routine's refcount calls.
Local-player lookup reaches 0x6db0250, retaining the two component IDs and
pointer stride while moving its optional actor flag from +0x269 to +0x289.

Registry constructor candidates 0x25bfc50 and 0x2b96390 embed the registry at
owner+0x30 and store a global atomic construction ID at owner+0x160. Retained
camera Sources, weak-camera references and actor bindings now remember that ID
and compare it on reuse, in addition to sparse liveness/context/component checks.
A different registry constructed at the same allocation address is therefore
rejected when its ID differs. This remains a bounded identity observation, not
an acquired native reference or ownership lease. Existing fixture allocation
sizes/literal fields were adapted to include the ID; no cases were added/run.

The native camera query still requests hash 0x4f6047c7 and uses a 0x120 payload
stride. Current CameraSystem candidate 0x6f6ca00 delegates query iteration to
0x6f321c0, which dispatches through 0x6f6c450 and callback 0x6f31c50. That
callback accepts five arguments, resolves its mode selector, moves the actor
from r9 to r8 and tail-jumps at 0x6f31d18 to the actual three-pointer updater
0x6f313f0. Installing the wrapper with the updater ABI would discard arguments;
the profile binding identifies the actual updater instead.

native_camera installation now takes updater/iterator RVAs and ABIs from the
already-approved exact-image profile, checking the declared call or tail-jump
witness and both fixed prologues before creating either hook. Both trampolines
are published before activation. The iterator/context source enables first;
an updater-enable failure attempts to disable it and reports the rollback
status. Originals remain available to callbacks already in flight. Diagnostic
enabled flags are separate from trampoline allocation, and selected RVAs come
from the binding rather than hardcoded observer constants.

The current updater (2133 bytes) and iterator (271 bytes) are not normalized
byte matches for the old 2156/268-byte functions. Preserve the failed matcher
report alongside these call/query/storage witnesses. Complete camera matrix,
control/time consumer and other ECS component layouts still need migration.
Current-build candidates remain inactive; native cosmetics scene/material
ownership and deterministic replay/MP4 rendering are still unfinished. No
builds, tests, syntax checks, project/game execution or Bedrock launch occurred.
See inspection/static-ecs-camera-layout-1.26.60.5.json and the separate
inspection/static-ecs-function-map-1.26.60.5.json.

### Replay camera consumer argument and held-time port (source only)

The old Game play screen - Render caller explicitly places partial tick time
in xmm2 before calling 0x46cff30. The previous Run callback declared only its
two pointers and could not preserve that incoming float across its own work.
Run/run now take a third f32 and forward it unchanged on the single original
call. Only the existing fixture signature/call was adapted; no new case was
added and none ran. This is an argument-forwarding source fix, not a rendered
camera equivalence result.

The same unique string joins current caller 0x4e0b0b0. Its call at 0x4e0b2ed
selects the actual native partial value from screen+0x120+index*4 and reaches
consumer 0x53b3290, using player owner +0x480. The source does not reconstruct
that value from another screen field. The consumer reaches matrix helper
0xaa6820 at 0x53b3858. Consumer stack allocation changes 0x108 to 0xe8; the
matrix helper retains the old 0x158 stack prefix while its full body changes.
A superficial prefix/component-ID candidate at 0x6ef9490 was rejected: it
uses r8b as a mode selector and executes another component-query operation.

Constructor-paired base camera tables 0xe8ddfa0 and 0xf246770 retain the
preRenderUpdate slot at index 22. The derived slot calls the base at old
0x46aa525 and current 0x535e9e5, joining 0x4685680 to 0x53750c0. Its clock
pointer load moves from screen+0xb0 to +0xa8. Delta/last-time float fields
move from camera+0x400/+0x404 to +0x3f8/+0x3fc, while the zero/negative delta
branch still retains its previous native value. The native stack changes from
0xe8 to 0xf8. All camera-time reads, writes and later capture checks now use
the shared scene-family FRAME_TIME constant; the active old family stays +0x400.

Approved-profile bindings now declare the consumer, matrix and pre-render
targets/ABIs, recognized stack variants and checked direct-call witnesses.
Both consumer/matrix prologues pass before either hook is created, and both
originals are published before the matrix observer and then consumer enable.
A consumer-enable failure attempts to disable the matrix hook and reports the
rollback status, retaining originals for callbacks in flight. Diagnostic and
command-admission readiness use enabled state rather than allocated trampolines.
Pre-render installation also reports actual enabled state and the profile RVA.

The matrix helper's inspected operand offsets include component pose/lens and
cache storage, but distinct source/output roles and controlled consumption
through every world render path remain to be established. No alternate phase,
camera acknowledgement, native material/scene lease or export readiness is
invented from these joins. The installed-build candidates remain inactive.
Native cosmetics insertion, the complete dependent current layout family and
deterministic replay/MP4 parity are still unfinished. No builds, tests, syntax
checks, project/game execution or Bedrock launch occurred. Witnesses and limits
are in inspection/static-camera-consumer-layout-1.26.60.5.json.

### Synchronous material-info strong ownership (source only)

Named shader keys join the concrete RenderMaterial parser at old 0x4b0b640
to current 0x4df5f50 and its concrete table at 0xf2147e0. Both parsers pass
the three named destinations at +0xd0/+0xf0/+0x110. A second current parser
uses those same fields but belongs to a different table group; keys alone
therefore do not identify the concrete class. The copy-constructor table
stores provide an independent join.

MaterialPtr info loaders 0x5a5b080 and 0x5449be0 construct the shared object
in a 0x58-byte control allocation. Counts are at control+8/+12 and the info
object is control+0x10. The concrete control tables are 0xe954fe0 and
0xf24bb00. Their destroy routines (0x5a5e650/0x544d230) destroy the info's
unique_ptr and name/weak metadata. Delete routes through the third virtual
slot with flag 1; its deleting destructor frees exactly 0x58 bytes. Full
normalized function matching failed, so the independent class, allocation,
operand and release witnesses remain explicit.

native_material_owner now ports one actual native strong reference for the
existing synchronous skinned-material input scopes. Acquisition requires an
unsafe caller contract: the active native constructor/submission callback
owns a live MaterialPtr pair while the reference is acquired. It verifies
the concrete in-place control/object relation and all three rebased method
slots, then uses a bounded positive-count SeqCst compare-exchange loop.
Zero/dead and near-overflow counts are rejected rather than resurrected.
Drop releases that one count using the native last-strong destroy and
implicit-weak/delete protocol. An invalid release count latches further
acquisition/current checks off.

SkinnedMaterial acquires this info owner before copying shader paths and
variation metadata, and rechecks both the retained control and copied pair
before admitting replay shader inputs. The unsafe lifetime requirement now
propagates from the actual native description constructor callback through
observation, replay preparation and material copying. A positive count or a
readable arbitrary pointer is not accepted as that lifetime contract.

The info owner is !Send/!Sync and stays in the existing thread-local pending
and material-consumer scopes. It never enters an archive, asynchronous upload
or GPU retirement queue. Only owned scalar/path fingerprints leave the scope.
Copied-metadata read budgets are reported separately from owner atomics and
repeated endpoint checks. Diagnostic counters distinguish retentions, releases,
info destruction and control deletion without claiming runtime verification.
Pending resets, scope retirement, rejected preparation and consumer overflow
move retained entries out of RefCell borrows before dropping them, so a final
native reference release can reenter engine/observer code without that mutable
state borrow remaining active.

Retaining info does not pin its replaceable RenderMaterial unique_ptr,
descriptor heaps, scene targets or GPU submission. General borrowed material
observation still permits aliasing pairs; the concrete allocation relation
is required only by this scoped retained-info path. No material cohort is
fabricated for world geometry, and no generic empty owner is supplied to a
draw transaction. Complete native scene/resource ownership, Java-equivalent
material inputs and live phase insertion remain unfinished. Current-build
candidates stay inactive. No builds, tests, syntax checks, project/game code
or Bedrock launch ran. See inspection/native-material-info-owner-source.json
and the separate failed static-material-function-map-1.26.60.5.json report.

### Current-build core entry mapping and shared scene layout (source only)

Static joins against the cached 1.26.60.5 sections locate candidates for
renderWorld at 0x535f980, renderActor at 0x594be00 and levelTick at 0x6b8d550.
The first-person render workaround string identifies the world caller, which
calls the world candidate twice with the same three argument roles. The Level
constructor identifies vtable 0xf2ce790; tick remains at slot 0xcc0, with 21
neighboring slots recorded for comparison. The platform-global store signature
locates 0x128bef90 and its immediate platform load. Its downstream object
ownership and client layout are still unresolved.

The world-function accesses also demonstrate different member movements:
LevelRenderer's client pointer changes from +0x408 to +0x418, its player owner
from +0x468 to +0x480, and the two position vectors from +0x654/+0x660 to
+0x64c/+0x658. These are paired instruction observations, not complete object
layouts, a uniform offset delta or live camera-origin validation. Function
bytes, hashes, instruction contexts and virtual neighbors are retained in
inspection/static-scene-layout-1.26.60.5.json alongside the separate caller,
signature and core-entry difference reports. The two named task groups uniquely
join LevelRenderer's constructor at 0x53484d0. Its camera allocation and call
join player constructor 0x5386c00 and camera base 0x53841d0. Constructor table
stores locate render/app/player/owner vtables at 0xf2465c0, 0xf2466c0,
0xf246fd0 and 0xf247ae0. Six additional paired member accesses cover renderer
task owners/game-renderer/level and player level/dimension/renderer links.
The old constructor chain has an additional intermediate call; no blanket
function equivalence or ownership guarantee is inferred from these joins.

native_scene_layout now centralizes the existing pinned scene family. Primary
client lookup, world camera reads, owned-camera checks, frame/chunk/render work,
render clock, camera timing, fog, flipbook and shader clock joins use the shared
fields and common scene vtables. Installation explicitly rejects a profile
whose product version differs from that layout family before configuring
private readers or installing hooks.
The complete existing image hash remains required. Static 1.26.60.5 candidates
are not activated; changing three entry addresses cannot port the dependent
ECS, frame views, shader inputs, scene lifetime or native phase insertion.

Review consisted of source reads/edits and cached PE decoding as data. No game
module or helper was loaded or executed; no builds, tests, syntax checks,
exports or Bedrock launch occurred. Full native and deterministic video parity
remain unfinished and runtime-unverified.

The client join now follows the unique education disconnection-message path
from the old constructor to current constructor 0x65877f0 and final vtable
0xf2b9330. Five getter pairs establish different slot movements: local player
31 to 30, camera weak reference 43 to 42, game 79 to 77, renderer 187 to 185,
and Minecraft level 168 to 166. Client game/Minecraft/renderer fields move
from +0x1a8/+0x1b0/+0x1c0 to +0x1c8/+0x1d0/+0x1e0, and weak references
from +0x218/+0x248 to +0x238/+0x268. The level getter reads Minecraft's
session at +0xd0 instead of +0xc8. No uniform virtual-slot or object-field
delta is applied. Instruction evidence is retained in
inspection/static-client-layout-1.26.60.5.json; current timer fields, weak ECS
ownership, primary-client map and the full downstream reader family still
need joins.

The pinned client's getter slots, function fingerprints, weak-reference fields
and Minecraft session/timer offsets now also live in native_scene_layout.
The camera reader checks the exact pinned ClientInstance vtable as well as
its selected getter before reading weak references. These source changes
retain the old build family; the current candidates remain inactive. This
additional work also used source edits and file-data decoding only, with no
builds, tests, syntax checks, helper/game execution or Bedrock launch.

### Native timer third-argument forwarding (static source port)

The cached 1.26.60.5 Timer::advanceTime candidate is 0x5a3b180 (535 bytes),
compared with pinned 0x47323b0 (475 bytes). Its normal calculation path copies
the incoming r8 value to ebx and tests bl after accumulating ticks. A nonzero
flag limits positive output to one tick and retains excess whole ticks in the
existing +0x1c field. Six direct call sites in three current functions explicitly
clear r8d; this is not proof that every possible caller passes zero. The old
two-argument callback cannot preserve this argument merely by changing its RVA.

The approved profile now declares timer RVA and ABI. world_hooks::timer_binding
requires the previously accepted full code image, the shared layout family,
the candidate count and executable bounds. native_clock selects a typed legacy
or three-argument callback plus its corresponding entry fingerprint. Both feed
an owned, Copy invocation value into the existing control path. Every native
forward, including scoped playback-rate updates and both explicit stepping
calls, retains the same incoming low-byte flag; there is no forced-false policy
or TLS/global argument storage. Diagnostics identify the selected ABI and leave
native flag behavior unverified. The active profile still selects the legacy
ABI; current-build candidate addresses are not activated.

The game join also identifies current constructor 0x82d820 through its unique
AppSystemRegistry anchor and final vtable 0xf091b70. Virtual slot +0x360 selects
getter 0x8ac4e0, reading GameRenderer at +0x1628 instead of +0x1440. The paired
client lookup at 0x8bd280 reads the map at +0x9d0 instead of +0x970 and the
client object at node+0x38 instead of +0x30. Its key remains an unsigned byte
at +0x20; parent and sentinel fields retain their observed offsets. The pinned
map reader now takes all these field roles from native_scene_layout. None of
these joins establishes complete map ownership or downstream renderer lifetime.

Function bytes, hashes, getter/constructor contexts and all located timer direct
calls are retained in inspection/static-game-timer-layout-1.26.60.5.json.
Minecraft timer pointer fields/aliases, GameRenderer clock/matrices, native
feature insertion, scene/material ownership and deterministic replay/video
remain incomplete. Existing legacy fixture call sites were adapted to the
invocation type without adding cases. No tests, builds, syntax checks, helpers,
exports or game code were run; Bedrock remained closed. Review used source
reads/edits and decoding cached sections as data only.

### Renderer context and caller-bound clock bindings (static source port)

The retained frame-start/end labels join current functions 0x882f00 and
0x887880. Four retained renderer labels independently join renderCurrentFrame
at 0x53117b0. All three call 0x5314b40 with the renderer, output pointer and
float fraction. That candidate returns the output pointer and supplies
renderer+0x208 to ScreenContext constructor 0x56ffc0, which stores it at +0xa8.
The pinned counterparts use renderer+0x1f0 and screen+0xb0. Clock scale,
last-time, copied screen context and global-buffer accesses also move in the
current frame body. Their observations are retained separately from complete
ownership or layout claims.

The known world-render method's vtable reference joins the matrix writer at
relative entry +0x28: pinned 0x4c0da0 to current 0x4e0d4b0. Its corresponding
view/projection stores move from +0x388/+0x408 to +0x3c0/+0x440. This differs
from the clock's movement; no uniform GameRenderer field delta is applied.
The current completed-tick counter still needs an independent producer join.

Cached PE import descriptors identify the current fmod import and its unique
thunk at 0xeec6880 through IAT 0xf9a7e48. The frame calls it three times with
3600 as divisor: absolute wall time, frame delta and accumulated world time.
The subsequent source control covers the first two calls and observes the
conditional third. Runtime delta/world repeatability is not established by
these static joins; the follow-up shader-delta section below describes its
implemented scope.

Approved profile metadata now carries screen-context producer address, stack
fingerprint and exact world caller return address, plus the fmod thunk, IAT and
total-time caller return address. Binding resolution requires the existing full
image hash/layout family, executable bounds and known callback ABI. Each return
filter additionally requires the actual E8 instruction to call its declared
callee. Renderer-clock installation verifies the selected prologue and actual
RIP-relative thunk-to-IAT relationship. Its callback filters and diagnostics use
those bindings. Copied screen context, partial fraction and shader-global
pointer fields now also use the shared pinned scene definitions.

The approved profile still targets 1.26.5203.0. New candidates, hashes, constructor
and matrix-store contexts, frame labels, import data and caller instructions are
retained in inspection/static-render-context-layout-1.26.60.5.json. They do not
activate 1.26.60.5 or establish complete camera/shader/ECS/resource ownership.
Live native feature insertion, full world shader-time reconstruction and
deterministic replay/video remain unfinished. Work used source edits and cached
file decoding only: no builds, tests, syntax checks, helper/game execution,
exports or Bedrock launch occurred.

### Scoped unscaled shader delta (static source port)

The pinned frame's fmod return roles are total 0x47a92fe, delta 0x47a9321 and
world accumulator 0x47a94f2. Corresponding current-build candidates are
0x5311dca, 0x5311ded and 0x5311ef8. The actual native instructions retain
the second result in xmm6, narrow it to float, multiply by the game's own
world scale, widen to double, add the native world accumulator and wrap it.
The world update is conditional on the renderer's existing skip flag.

FmodBinding now validates the three distinct, ordered direct-call returns.
The exact world context publishes a thread-local renderer/generation/replay
timestamp/delta value and next expected role only after the existing renderer
timeline has accepted and applied its clock plan. Total time keeps its existing
epoch mapping. The delta role returns the unscaled replay timestamp difference,
including zero for held frames; the original native narrowing, multiplication,
skip policy and world accumulation remain in place. Each role rechecks the live
controlled renderer, clock generation, replay timestamp and renderer timeline.
Recognized invalid or out-of-order roles clear frame authority; unrelated math
calls retain their native output without consuming it. New world contexts and
context output/return mismatches clear or replace authority as before.

The conditional world role observes the original remainder return after the
controlled delta, then consumes the frame value. It does not override the
world phase or assert that the subsequent native store ran. Delta/world status
is explicitly diagnostic and leaves runtime verification false. The initial
world accumulator and absolute shader epoch still need recorded replay anchors;
different rebuilt scenes are not certified to share those starting phases.

Instruction bytes, call-site roles and native delta-to-world/skip witnesses
are retained in inspection/native-shader-delta-source.json. The current-build
profile remains inactive; the complete renderer/ECS/camera/resource family and
native cosmetics insertion remain unfinished. Full deterministic replay/video
parity is not claimed. No tests, new test cases, builds, syntax checks, helpers,
exports or game code were run; Bedrock stayed closed. This work used source
reads/edits and cached PE sections decoded as data only.

### Primary-world CPU shader-clock observations (source only)

native_shader_clock now observes the native total/delta/world remainder path
outside frozen playback as well as during controlled playback. An exact world
context starts observation only after the original constructor returns its
output. Read-only owner discovery joins the primary client, both renderer links,
level/session epoch, simulation and real timer identities, completed tick,
partial, rate and elapsed time. It brackets mutable owner/timing reads and does
not acquire a clock lease or call native virtual methods.

The import hook records both its original result and the effective return after
any Arctic control. Only the approved, ordered three caller roles consume this
observation. World-pass publication requires the selected camera clock, unchanged
primary identity, the constructed screen or its known renderer copy, matching
meaningful context header bytes, and the renderer clock's exact float bits. An
executed world remainder must match the native accumulator bits. If the native
skip flag prevented that call, the accumulator must instead retain its value
from before the frame; no absent world call is synthesized. Identity, phase and
skip state are rechecked before publication.

The optional observation is transported by the existing owned world packet at
nativeTiming.rendererState.shaderClock with observation/world sequence/scope
identities and scalar values only. Private addresses and copied context headers
remain in native code. WorldFrame parsing creates an immutable NativeShaderClock,
checking schema, source/session/sequence, timing/renderer-clock bit equality,
bounded native/effective remainder values and the optional skipped-world shape.
These CPU samples explicitly retain shader-upload and deterministic-video flags
as false. They do not establish native producer provenance from a fixture, GPU
upload, scene resource ownership or rendered pixels.

This exposes the inputs needed for recorded shader phase restoration; an archive
track and authoritative time association are still unfinished. Presentation time
can advance while simulation ticks are held. The recording format must preserve
that distinction instead of inventing strictly increasing simulation timestamps
or dropping paused-time observations. Current-build private bindings, native
cosmetic insertion and complete replay/video parity also remain unfinished.

See inspection/native-shader-clock-observation-source.json. No builds, tests,
new test cases, syntax checks, helper/game execution, exports or Bedrock launch
occurred; this was source inspection and editing only.

### Serialized native geometry stream (source only)

The DX12 frame owner now schedules the previously unused geometry producer.
`depth_tracker.geometry_world` selects exact submitted depth scope, native
surface dimensions and observed depth convention from preserved-copy metadata
or the current eligible depth state. Selection does not copy/map terrain or
wait for GPU completion. Preserved copies require the matching submitted
generation and queue; conflicting source/scope/convention candidates reject
the request. The software depth readback remains a separate operation.

After the regular Java frame imports textures and updates controllers, the
same serialized client owner calls `worldGeometryFromFrame`. The result still
has to match the original producer FrameKey, session, sequence, size and camera
bits. Explicit material inputs are required before uploading; the live source
currently lacks the Java-equivalent lightmap/fog cohort, so this case reports
that missing input rather than supplying white lighting.

`native_geometry_stream` manages one asynchronous upload ticket and one shared
completed upload. Nonblocking presentation polling checks exact frame currency
again. Disable/reset abandons pending tickets through the existing fenced
retirement pool. `Geometry.from_stream` accepts an exact scope/surface lookup,
explicit phase preparation/poses and matching pipeline depth convention, then
rechecks the same published upload after preflight. Recording owners retain
their own shared upload reference through native fence retirement. The stream
does not grant surface ownership, select native feature phases, insert a draw,
or authorize native body/cape/wing suppression.

This is an integrated source data path, not a working native draw integration.
Live material inputs, actual model/foreign feature phases and the game-facing
insertion/scene owner remain unfinished. No builds, tests, syntax checks,
project/game execution, GPU/JNI calls or exports were run; Bedrock stayed closed.
Full cosmetics and deterministic replay/export parity remain unverified. See
inspection/native-geometry-stream-source.json and
inspection/native-geometry-input-source.json.

### Recorded native wing palettes and material inputs (source only)

The selected native wing layer now participates in the final-palette recording
and replay path. Archive role 8 identifies an accepted `geometry.elytra` layer;
it is not a native child index. Existing hierarchy, group, owner, cache, source
span and animation-frame checks still determine which actual palette is copied.
Child layers retain their real pose space. Root wing transforms remain
independent of the emote torso when no recorded palette is selected.

Attachment history now writes `bedrock_attachment_samples_v3` with magic
`0x4152434154540003`. Readers retain explicit v2 cape and v1 attachment support.
The binary palette fields and existing allocation/count budgets are retained;
older format writes reject wing roles rather than dropping or relabelling them.

Java replay plans also carry recorded wing RGBA, dithering and shader inputs.
The existing owned-description and pinned synchronous material-copy consumers
now accept checked wing/frame identities as well as cape identities. Copies
still affect only the actual description and stack-local snapshot, with the
original selected native material/program and texture bindings. Current source
cohorts, material path/variation metadata, source spans and exact replay clock
must continue to agree. Separate wing receipts count every prepared/consumed
description and preserve failures; rerecording uses checked applied values.

This connects source behavior through existing game-facing consumers. It is
not runtime proof: no builds, tests, syntax checks, GPU/JNI calls, project/game
execution or exports were run, and Bedrock stayed closed. Wing GPU/material
fidelity, texture/program coverage, custom world-mesh insertion and deterministic
video/full parity remain unverified. See
inspection/native-wing-palette-replay-source.json.

### Replay export range selection (source only)

The Bedrock camera editor now offers start/end video selection and a camera-path
range shortcut. Frame counts follow Java VideoExport's inclusive endpoint
predicate, including its double frame-time calculation. Selected ranges use the
existing world rebuild and reject bootstrap clamping, out-of-recording ends or
an actual replay origin different from the selection. Camera timestamps retain
their absolute replay origin. Indexed consumers now admit the final sample at
exactly two hours; real-time budgets remain unchanged.

New export intent schema 4 binds the optional selection alongside the existing
cadence/encoding profile, and paired comparison reports selected-range changes.
Schemas 1..3 retain their original serialization, hash domains and frame limits.
This is source editing/manual review only. No builds, tests, syntax checks,
encoder/export, project/game execution or Bedrock launch occurred. Native world,
audio, appearance and deterministic-video parity remain incomplete/unverified.
See inspection/replay-export-range-source.json.

### Installed 1.26.60.5 migration discovery (static, 2026-10-30)

The installed executable now reports 1.26.60.5. Its code hash, PE timestamp,
image size and selected actor-dispatch member accesses differ from the pinned
1.26.5203.0 profile. The existing exact-image guard remains in force; older
RVAs and layouts are not promoted to this build. Its error now identifies the
expected build and the observed PE metadata instead of a generic mismatch.

Static section copies and bounded function/string comparisons are retained in
inspection/1.26.60.5-static.json and its referenced reports. Twelve of 90 sampled
referenced function entries have unique normalized-code candidates. A unique
actor-dispatch string candidate still changes instructions/member accesses.
Subsequent static joins locate world/tick entry candidates, recorded in
inspection/static-scene-layout-1.26.60.5.json. External-call semantics, complete
layouts, ABI and ownership must be joined before enabling a new profile.

The exact CodeView PDB path queried on Microsoft's symbol server returned 404.
No game module, process, hooks, build, tests or export were executed during this
discovery. Current-build compatibility and all native/replay parity claims
remain unproven. Section hashes/function similarity are migration evidence,
not runtime rendering evidence.

### Split-torso emote cape visibility (static source port)

Cached Java 26.2 PlayerCapeModel extends PlayerModel and calls its setupAnim,
so Arctic's PlayerModelEmoteMixin also runs on the cape model. Its expressive
split-body path hides BODY. The cape is a child of that body, and ModelPart's
visible=false return prevents child traversal. AvatarRenderer now applies this
same emote-derived gate before emitting its cape mesh through the shared
software/native geometry path.

Worn cosmetic models still use the body's transform directly, and the wing
model remains independent. The normal skin's copied visibility mask is not
treated as visibility for a separate cape model. Invisible-player/spectator
layer eligibility needs its own source inputs; native insertion and complete
cape/emote rendering parity remain unfinished. Evidence is static class-data
and source inspection only. No builds, tests, syntax checks, project/game code
or exports were run; Bedrock was not opened. See
inspection/emote-skin-submit-order-source.json.

### Java layer eligibility and body history v7 (static source port)

Java 26.2 AvatarRenderer.shouldRenderLayers skips all layers for a spectator.
Arctic CosmeticsLayer independently skips worn cosmetics and bent skin when
the actor is invisible. CapeLayer also requires showCape, while WingsLayer has
no corresponding invisible/showCape early return. Arctic's player-skin mixin
sets a loaded cape as an explicit elytra texture, which the Java wing selector
prefers before its showCape-controlled cape fallback.

JavaLayerVisibility now retains these three explicit decisions separately from
native culling and normal-skin ModelPart masks. The optional WorldFrame player
input requires numeric schema 1 and actual JSON booleans for known spectator,
invisible and showCape facts. Missing/null fields remain independently unknown;
coerced strings/numbers reject and all-unknown values canonicalize to null.
Shared AvatarRenderer emission applies the separate cape, wing and
custom/bent-skin gates. Its prior split-torso cape-parent rule still applies.
World replay emission and previews use recorded decisions rather than the
viewer's live flags.

Body history v7 appends a strict presence byte and, when present, known and value
mask bytes (1 spectator, 2 invisible, 4 showCape) after v6 skin visibility. Known
must be 1..7 and value bits must be a subset of known. Reader support
for versions 1..6 remains, including a distinct v6 kind/magic alias. Copying,
equality and interval sampling retain each independently unchanged known fact;
missing/changing facts become independently unknown. No discrete transition
time is invented. The three-byte maximum extension is charged to the existing
bounded block size. Complete replay appearance readiness and v7 body plans
require all three facts; a partial record does not prove complete eligibility.

ARCGEO06 reserves flags bit 16 for known layer eligibility on cosmetic base,
glow, cape, wing and bent-skin roles. Builder state resets per actor, rejects
mid-quad changes and rejects emission for explicitly hidden layers. Positive
eligibility requires known spectator state for wings, spectator and invisible
state for custom/bent skin, and all three facts for capes. Unknown
software preview behavior remains, but both native constant assembly and draw
preparation reject nonempty layer geometry without the supplied provenance.
Versions 1..5 still decode with unknown eligibility; v6 otherwise keeps the v5
payload layout. This does not establish native phase or scene ownership.

live_frame.player publishes native invisibility from the already copied
ActorDataFlagComponent words through Components.invisible (flag 5), plus optional
spectator state from guarded ActorGameTypeComponent copies described in
inspection/native-spectator-source.json. Default mode can resolve through the
optional native_game_mode reader, configured after the pinned executable hash
and exact client-level vtable/getter fingerprint match. It copies the shared
data pair and mode twice, checking level identity and reference metadata without
calling the virtual getter or acquiring native ownership. Failed or unsupported
observations and showCape remain unknown. Existing global native culling and suppression gates
are unchanged; complete invisible-player wing replacement remains unfinished.
The attachable observer now retains actual selected elytra wing poses even when
the skin is invisible; matching mesh-role color copies use the existing frame
and actor checks. Body, wing and layer-color history writers retain complete
native-model-draw observations for explicitly invisible players as well as
visible players, so the culling flag no longer erases these raw replay inputs.
Missing roots, incomplete parts and unknown hidden actors still leave gaps.
Body/wing writers retain their copied-matrix requirements, and wing samples
still require an observed equipped elytra. This changes no archive layout or
render/suppression/export eligibility; invisible-player world drawing remains
unfinished.
Cape-option inputs, native insertion and runtime evidence, including validation
of inherited/default spectator resolution, remain missing. Track-available
status is separate from playback verification, which remains false. No tests,
builds, syntax checks, project/game code execution or exports were run; Bedrock
was not opened. See inspection/java-layer-visibility-source.json.

### Java render factory references (source only)

JavaRenderType now supplies an owned reference-identity source for the four
currently emitted factory calls. Cached RenderTypes methods establish separate
entitySolid/armorCutoutNoCull caches and the Boolean.TRUE wrapper keys for
entityTranslucent/entityTranslucentEmissive. Cached Util memoizers use concurrent
computeIfAbsent maps, with argument pairs for BiFunction factories. The port
retains immutable objects without eviction or per-frame recycling, with bounded
admission of 65536 references. Base cosmetics, visible skin and bent skin share
the entityTranslucent cache for the same logical texture. Anonymous/general
inputs remain unknown; pixel identity never selects a factory.

ARCGEO07 carries optional factory/reference metadata after each batch's logical
texture key. Its one-byte presence plus optional factory byte and u64 reference
are charged to the existing frame budget. Native decoding checks role/key/ID
consistency in both directions, and the draw planner checks supplied object IDs
against known packet references. Versions 1..6 retain unknown references.
These IDs belong to the source-port renderer, not an installed Java JVM, and
cannot stand in for captured HashMap identity hashes or prepared-GPU equality.

Cached RenderType.prepare independently establishes that PreparedRenderType
equality includes pipeline, output target, dynamic transform buffer slice,
scissor state and prepared texture bindings. Those inputs, actual phase order
and native insertion ownership remain unfinished. The new reference producer
does not authorize a draw or complete live integration. See
inspection/java-render-type-reference-source.json. No builds, tests, syntax
checks, Java loading, GPU commands or game execution were performed.

### Native texture retirement-path guard (source only)

Committed target allocation ownership now has a public-API source path. Cached
texture creation at dfa4fe0 has a direct CreateCommittedResource route and a
separate allocator-helper route. The new observer hooks the actual public
device method, forwards it unchanged and retains only successful returned COM
resources with matching actual device identity and supported target shape.
Copied slot pointers or allocation-mode bytes never manufacture a receipt.

native_committed_resources keeps separate discovery pools: at most 16 targets
and 64 buffers, each charged at most 256 MiB of queried allocation sizes. Buffer
churn cannot evict target discovery. The resource and device are held through their typed
interfaces; cache identity checks use canonical numeric IUnknown identities.
No COM query or release occurs under that cache mutex. Cache eviction removes
only discovery ownership: a matching public-barrier resource's receipt is
retained by its attachment Witness, Bound and existing draw-asset transaction.
Bound.committed_allocations exposes both owners only for a valid complete pair.
Disabled-cache collection runs alongside the existing frame asset collection.
This accounting excludes driver overhead and the separate shadow/draw pools.

Restore snapshots now resolve their non-null graphics root CBV/SRV/UAV addresses
and nonempty vertex/index slot-zero spans against the buffer pool on the actual
single-node list device; linked-node recording provenance remains unknown. One
cache-lock sample must fully contain each span in exactly one
observed committed buffer; unknown/ambiguous/partial/overflowing coverage stays
unknown. Root descriptors provide no length, so their point query retains the
whole owning buffer without inventing shader read extents. Snapshot clones keep
the deduplicated Arc owners through the existing draw-asset retirement path.
Zero addresses and empty IA views retain their actual binding values without
requesting storage ownership. This optional ownership does not promote readiness
or remove the unsafe caller's native lifetime duties. See
inspection/native-restore-buffer-owners-source.json.

Readback heaps stay outside shader/IA buffer discovery. Entity and glint
constant-buffer construction now uses the existing nested private-command guard
so these Arctic-owned arenas do not populate native committed-buffer discovery.
No test/build, GPU allocation or runtime verification was performed.

These owners keep observed committed storage alive, but do not freeze contents,
prevent engine pooling/reuse, own placed allocations, preserve descriptor
contents or reserve an exclusive recording phase. They do not satisfy the whole
Cohort.native_owner contract or authorize insertion. Late/unobserved creation
and other creation API variants stay unknown. No builds, tests or runtime work
ran; details and remaining boundaries are in
inspection/native-committed-resources-source.json.

Cached texture teardown now has a more specific source trace. The pdata-bounded
routine at df98520 tests slot+9b bit 40 and skips its ordinary retirement and
slot-clear branch when set. The ordinary branch appends the resource pointer to
a context vector reached through the global at RVA 12006060, then +19f50, with
vector fields +2cbe0/+2cbe8/+2cbf0. A separate override candidate calls cleanup,
sets that bit and replaces slot+0 with its incoming pointer. The raw getter
returns slot+0 without acquiring a retained owner. These are inspected data
flows; the consumer's fence/release ordering is now traced below, while backing
allocation ownership remains unresolved.

Texture.copy now guards the whole mode byte along with its pointer. A mode
change invalidates the existing copied cohort even if the pointer still matches.
Stable summaries report ordinaryRetirementBypassed; changed/unavailable slots
report null. Neither bit value supplies Cohort.native_owner or permits insertion.
The source adds one byte/guard per texture within existing snapshot bounds,
without invoking a native getter, cleanup, allocator or registry pointer as COM.

Both cached text copies used for disassembly/xrefs were hashed and match the
pinned build. The committed-resource route now has the owner path above;
allocator-managed backing ownership remains unresolved. No builds, tests, syntax checks,
game loading/execution, process inspection, native calls, GPU work or exports
ran. The owning phase/material caller and full parity remain incomplete. See
inspection/native-texture-retirement-source.json.

The deferred vector's consumer is the already observed native completion
routine at dfa1540. It copies the context's deferred pointers to a temporary
vector, resets context.end to context.begin, releases the copied COM references,
then pushes the completed contexts into the free ring. The vector reset occurs
before releases; it cannot independently authorize retirement. The existing
original-success, completed-fence, queue/list identity and used-to-free checks
remain necessary.

depth_queue now samples the 24-byte header twice before and after that call,
only for contexts carrying actual retirement tickets. Its post-call guard also
requires unchanged begin/capacity and end==begin. Unknown, changing or nonempty
metadata leaves both snapshot and draw-asset owners pinned. Reads are constant
size within the 272-context ring bound; no vector element is dereferenced and no
native getter/COM method is called using copied addresses. Existing private GPU
fence and generation checks still govern eventual payload destruction. This
does not supply the native scene/allocation/descriptor owner or insertion caller.
The existing fixture buffer was extended to include the header without running
it. Static evidence and remaining integration limits are recorded in
inspection/native-fence-retirement-source.json; runtime parity is unverified.

### Independent wing model roots and layer transforms (source only)

Cached WingsLayer.submit selects its own ElytraModel, translates the entity
pose by (0,0,0.125f), then submits equipment layers. It does not parent that model
to the player's BODY. JavaWingModel now keeps that translation at the model-root
stage, before local wing offsets/rotations with local Z=0. The shared renderer
begins the Java-motion model with that same shifted root. Standalone emission
still accepts an entity root and applies the layer once; explicit glint uses
the same root/part helper, while observed glint keeps copying base clips.

The two stock wing root children also use retained default-map traversal,
following PartDefinition bake and ModelPart render. Base, explicit glint and
legacy wing emission share it; no left-first insertion order is assumed.
Target-JVM order has not been compared at runtime.

Completed native fallback poses are already bone-to-model matrices for an
independent geometry.elytra. They now compose under the entity root directly,
without posedBody * inverse(recordedWingBody). The archive's old torso reference
remains validated metadata, and missing samples remain gaps. This removes extra
humanoid emote parenting without inventing another controller or applying a Java
layer offset to already-completed native poses.

Independent wing emission no longer needs a humanoid body matrix when its own
wing inputs are complete. World replacement still requires the existing complete
native observation/item/scope/animation/texture/suppression checks. Shared
software/native staging changes preserve wire/archive versions and ABI. Legacy
geometry/controllers, matrix properties, full equipment/material variants,
native insertion/ownership and deterministic video remain unfinished or
unverified. No builds, tests, syntax checks, class loading, project/game execution,
process inspection, native commands or exports ran. See
inspection/wing-layer-root-source.json.

### Default generic affine pose arithmetic (source only)

JavaAffinePose now retains JOML 1.10.8's default generic affine float grouping
for point transforms, matrix products and translated copies. Linear products
use two nested non-fused multiply/add operations; translated rows use three,
with the left translation innermost. The old shared composition accumulated
from zero and added the translation before its other products. AvatarRenderer
now uses the generic recipe for its model/part/cape/wing composition products.

Fabric PoseXform and cached ModelPart.Cube.compile use Matrix4f.transformPosition.
Known Java AvatarPoseSink vertex passes now retain that generic point order in
software and staged native geometry. Position selection is separate from normal
selection: explicit armor-glint emission uses the same point recipe as base
wings despite not consuming entity normals. Deferred observed glint continues
copying the actual base clip coordinates. Untyped GENERAL vertex sinks retain
their previous point calculation; shared pose compositions also affect previews.

CosmeticsLayer.fit skips translation for Fit.NONE; the shared port now does too,
returning an owned unchanged clone. Moved/mirrored fits translate using the full
generic row, including zero X, before mirroring the first basis column. Parent
poses remain unchanged and no mesh is emitted again.

Copied native arrays do not include original JOML property flags. Identity and
translation shortcuts, PoseStack rotation/normal history, non-default FMA and
native/Java matrix equivalence are not inferred. This is the owned renderer's
default generic profile, not proof of every target branch or rendered pixel.
No wire/archive/ABI, native offset, shader or readiness gate changed. Cached
class bytes were decoded as data and the JOML archive hashed; no Java classes
loaded, builds, tests, syntax checks, project/game execution, process inspection,
native commands or exports ran. See inspection/affine-pose-arithmetic-source.json.
Live native insertion/ownership and full parity remain unfinished or unverified.

### Normal player-part traversal and emote hiding (source only)

Normal skin emission now follows the cached stock PlayerModel root definition,
instead of emitting head then SkinLimbs enum order. HumanoidModel adds six named
root parts to a default HashMap; PlayerModel replaces existing arm/leg keys and
adds outer layers beneath their owning parts. PartDefinition.bake retains that
map traversal in an Object2ObjectArrayMap, and ModelPart renders its own cubes
before children.values. JavaPlayerMesh now snapshots that same default-map
recipe once into an immutable six-part list. Each part still emits its cube
before its outer child, with the existing availability/visibility/material rules.

The normal/bent skin paths also now preserve the Java mixin's actual hide
branch. PlayerModelEmoteMixin hides bent original parts only for Rig.expressive
animations. CosmeticsLayer.drawLimbs independently emits custom split pieces
for any matching joint track. A legacy rightForearm-only emote therefore keeps
its original arm alongside custom split geometry; the port previously hid it.
Expressive split torso still hides the cape parent's geometry while retaining
an empty cape model submit. Worn attachments and independent wings keep their
existing paths, and the later custom bent loop keeps SkinLimbs enum order.

The shared emitter updates both Raster3D and NativeAvatarGeometry without
changing wire/archive/native ABI or supplying native ownership. Default-map
traversal has not been compared against a running target JVM, and extended
model definitions are outside this stock recipe. Cached class bytes were read
as data and dependency archives hashed; no builds, tests, syntax checks, Java
loading, project/game execution, process inspection, native commands or exports
ran. Actual native insertion/material/resource ownership, model/render fidelity
and deterministic video remain unfinished or unverified. See
inspection/player-part-traversal-source.json.

### Projection-specific software quad sorting (source only)

The validated ProjectionContext now privately retains its projection type,
alongside the already checked combined matrix. Raster3D selects an immutable
JavaQuadSorting helper only when its actual draw matrix still matches the
current world and captured projection and the frame declares camera-relative
positions. Raw JSON type changes cannot select another operator after that
validation. Missing or unchecked context retains the old comparator.

Cached ProjectionType/VertexSorting bytes select distance to origin for
perspective and negative centroid Z for orthographic. MeshData takes the float
midpoint of submitted vertices 0 and 2. The key function reads those positions;
it does not multiply a camera view matrix or inspect clipped/projected depth.
The software helper ports these operators on its emitted buffer positions,
including JOML's default non-fused distance grouping and signed-zero float
ordering. This does not prove native/Java vertex-space equivalence.

Known skin and simple base/bent/glow prepared draws use stable descending
Float.compare on the separate projection key. Both original primitives and
all clipped pieces share one original quad key. A draw with any unknown key
uses the older comparator for its complete geometry. Legacy queues, preview
constructors and the older skin fallback always retain that comparator, even
when a transferred triangle carries a new key. Existing validity/budget rules
remain, and solid cape/wing and glint primitives are not newly sorted.

No wire/archive/native ABI or runtime verification flag changed. No builds,
tests, syntax checks, Java loading, game/renderer/JNI/shader/GPU execution,
process inspection or exports ran. Vertex-space provenance, full geometry/GPU
equality, native insertion/ownership and deterministic replay/video remain
unfinished or unverified. See inspection/software-projection-quad-sort-source.json.

### Checked-view software skin model phase (source only)

ProjectionContext now retains a private transient copy of the affine view and
combined projection after its existing camera-pair validation succeeds. The
software raster requests that view only when its actual cloned draw matrix
still matches both the current world and checked projection. JSON cannot seed
these transient observations. Unknown or changed context supplies no view.

At beginModel, JavaModelView composes this supplied view with the actual model
root translation using the cached JOML 1.10.8 default non-fused affine grouping.
This is an owned software submit position, not an installed-Java pose/GPU
receipt. No actor-distance proxy, FOV inversion or camera-angle guess is used.

The skin stage now collects complete model submissions, retaining every part
and empty model. With all view positions known, JavaModelOrder sorts the whole
sequence once. Strict outputs consolidate only consecutive identical render
references with matching actual software bindings; an intervening different
reference, including an empty model, remains a boundary. Each resulting draw
uses the checked projection-specific quad sorter above. Missing/overflowed
view positions retain the older whole skin stage instead of sorting only the
known subset.

A frame-local lookup also detects a render reference changing its actual
image/pipeline/blend/emissive binding and makes known-phase output unavailable.
Normal skin, base cosmetics and bent skin share their implemented pipeline;
overlay and other vertex differences do not invent new prepared state. This
does not establish Java native GPU preparation/resource equality.

Native packet model coordinates and ARCGEO01..10 remain unchanged. Native
Java-equivalent pose/vertex-space provenance, insertion ownership,
non-default math profiles and full shader/raster fidelity
remain unfinished or unverified. Cached class bytes were decoded as data and
the JOML archive hashed; no Java classes loaded and no builds, tests, syntax
checks, project/game/renderer/JNI/shader/GPU execution, process inspection or
exports ran. See inspection/software-model-phase-source.json. Full parity is
not established.

### Owned software solid/custom phase consumption (source only)

WorldFrame.render now supplies a dedicated retained JavaSimplePhases collector
to Raster3D. Its source adapter is shared with NativeAvatarGeometry.Batch,
while software and native packet collections retain independent drain history.
Logical-ID sinks retain real owned JavaRenderType references and model tokens,
including empty model/part submissions. World gaps reset both histories;
software skips/surface mismatch/render suppression reset only software history.
Geometry-only request failures do not reset the software collector.

Known simple output now draws solids at (order 0, stage 0), the skin stage at
(0,7), custom base/bent at (0,8), selected wing glint at (1,7), then custom glow
at (1,8). The skin stage now has the checked-view model path above; unknown
views retain the older stage without inferred actor-distance view positions.

The software phase consumer traverses retained HashMap buckets, retaining
first prepared-binding encounter order and empty submissions. Compatible
draws reuse only actual image identity and this fixed frame's shader pipeline,
blend/emissive state. Normal skin, base and bent share their implemented pipeline;
all pose/light/tint/overlay/fog variation is already in vertex attributes.
This is complete equality for this software backend's bindings, not proof of
Java native GPU transform-slice/target/scissor/descriptor equality.

Cached class bytes reconfirm entitySolid/armorCutoutNoCull omit sortOnUpload,
while entityTranslucent/entityTranslucentEmissive enable it. Their builder
defaults false. Entity/emissive snippets use disconnected QUADS, for which
RenderType.canConsolidateConsecutiveGeometry returns true. Solid prepared
draws retain primitive order; checked custom base/bent/glow draws stably sort
projection-specific original quad keys within compatible draws, preserving
clipped triangles and equal-key order. Missing context retains the older key.
The previous global sort across incompatible materials is not used here.

Unknown names/general variants, changed model bindings, undeclared sorting
space or a collector larger than 2048 staged sinks preserve legacy rendering
and invalidate known phase history. Earlier clipped triangles move into that
path exactly once; meshes/animation are not emitted again and larger existing
draw collections are not removed by a new hard rejection. Known phase drawing
waits for finish to seal collection; legacy fallback retains immediate solids.
Migration failure also seals the frame. Failed partial finish cannot be retried,
and repeated successful finish returns the image without another collector drain.

The cached Java archive SHA-256 was confirmed as file data, and class bytes
were decoded with a data-only reader. No Java classes loaded and no builds,
tests, syntax checks, project/game/renderer/JNI/shader/GPU execution, process
inspection or exports ran. Full skin model sorting, unsupported variants,
native world insertion/resource ownership and deterministic replay/export
parity remain unfinished or unverified. Contract/reference details are in
inspection/software-simple-phase-source.json.

### Bounded direct export camera leases (source only)

Guarded direct-camera set requests now require leaseMs from 500 to 30000 ms.
The core automatic path requests 30000 ms for each explicit set/replacement.
Native acceptance uses Instant wall time, so a frozen clock, replay seek or
paused worker cannot renew that target. Explicit legacy manual requests may
still omit the lease. Engine offsets, C ABI and camera target units are unchanged.

Worker service, status, command admission, consumer selection and capture
observation expire the target under CONTROL. Expiry clears future target use
and copied camera summaries without writing engine memory. Each consumer now
holds a monotonic token joined to the accepted sequence/request identity until
its restoration guard finishes. Another callback cannot apply a new target or
reset the session underneath that claim. A frame guard is constructed before
fallible observations/patching and selection is rechecked before application.

An in-flight expired scope remains consumerActive until it restores. Only a
completed restored/unchanged-player observation may become lease_expired with
expiryObservation=true. Expiry before patching can establish inactivity but
cannot create a consumed-camera frame. A pending set expiring before its ack is
rejected. Worker expiry does not synthesize engine restoration or a release
acknowledgement, and does not erase component/actor/matrix errors.

Automatic acquisition now requires the lease capability and no active consumer
or deadline. New set acknowledgements must confirm the requested live lease.
Owned cleanup may return a fresh exact-identity inactive expiry status with
releaseSkipped=owned_camera_lease_expired instead of publishing a new command.
The export controller caches that actual observation separately from its clock
release. Lost acknowledgements still report uncertainty; native expiry bounds
future override use while an already-running scope must finish restoration.

No builds, tests, syntax checks, project/game execution, process inspection,
native commands or exports ran. Native scheduling/termination/restoration,
camera modes, live world integration and deterministic video remain unfinished
or unverified. See inspection/replay-camera-lease-source.json.

### Direct export camera ownership and cleanup (source only)

NativeExportClock's direct quaternion camera path now acquires only a freshly
observed idle native override. Later frames replace only its exact acknowledged
camera identity. The schema-1 camera request accepts optional expectedCamera
(session, sequence, requestId, active). Native queue admission and actual camera
consumer acceptance compare it under CONTROL before changing target state.
Mismatch records requestRejected and preserves the current owner. Accepted
receipts echo that expectation, which the core requires alongside correlation,
restored component, active-state and unchanged-player checks.

Idle acquisition reads a bounded recent camera report and requires the matching
fresh binding, both installed hooks, known idle identity and no queued/pending
work, error or restoration failure. A zero sequence has no previous receipt;
a later idle sequence must match an actual completed release. The automatic
path does not fall back to an unguarded command on old bundles or stale state.
Explicit manual commands retain their existing optional-guard behavior.

The export worker saves its camera acknowledgement before any later clock or
capture failure. It reserves one later camera release sequence before advancing
ticks, and refuses protocol capture while retaining a direct override. Cleanup
attempts guarded camera release, then releases the owned clock independently,
including when camera cleanup is rejected. Successful camera/clock releases
are cached separately, and the returned result includes the real camera receipt.
No cleanup action force-clears another controller's camera.

No native engine offset, C ABI, replay archive, frame readiness requirement or
runtime verification flag changed. Lost acknowledgements still leave uncertain
caller ownership; the bounded lease/consumer source path above now handles
future target expiry. Runtime recovery, native consumer/reentry ownership,
camera modes, live cosmetics and deterministic video remain unfinished or
unverified. No builds, tests, syntax
checks, project/game execution, process inspection, native commands or exports
ran. See inspection/replay-camera-ownership-source.json.

### Replay/export clock ownership transitions (source only)

Replay/export coordinators now put expectedClock on native clock operations.
The bounded object retains session, acknowledged sequence/request identity,
freeze generation, frozen mode and optional exact f32 playback rate. Initial
freeze/rate acquisition compares an observed idle identity. Step/sample/rate
updates and cleanup compare a successful active acknowledgement instead.
Native admission and the actual timer consumer both check it under the clock
state lock before accepting a transition. A stale report cannot alone justify
changing a clock subsequently acquired by a different controller.

Accepted native receipts echo that expectation, and the worker checks the
echo alongside its existing request/op/completion correlation. A deferred
rejection now goes to requestRejected; it preserves the current owner's
acknowledgement, lease and pending state instead of overwriting the last
successful receipt with a rejected request. Lease expiry and normal existing
timer maintenance remain independent of this incoming operation.

Export success/error cleanup, its initial-freeze fallback and playback release
now use guarded resume. A recent bounded report can instead confirm the same
identity is already idle, returning actual status with an explicit skip label
without publishing a command. NativeExportClock caches successful cleanup so
repeated calls do not send another resume. A lost acknowledgement can still
leave ownership uncertain; the coordinator reports failure and relies on the
native lease rather than forcing an unconditional release.

Manual clock commands retain their optional unguarded form. Guarded replay/
export paths require updated native readers and never downgrade to that form.
No new native offsets, game/account actions or resource ownership claims were
introduced. This coordinates control state; helper/native time alignment,
drift, real rendering and deterministic video remain unverified. Contract and
remaining boundaries are in inspection/replay-clock-ownership-source.json.
No build, test, syntax check, project/game/JNI/shader/GPU execution, process
inspection, native clock command or export ran; Bedrock was not opened.

### Indexed producer-frame camera timing receipts (source only)

The existing camera preRenderUpdate time hook now retains its guarded producer
FrameKey and fully acknowledged held-clock sample around the native call.
Successful exact output comparisons can enter an eight-frame bounded cache.
Missing or conflicting observations make a key ambiguous; later success cannot
overwrite that failure. Native clock capture resolves the actual primary
renderer/completed tick and reuses the existing strict lease/command/timer
admission after releasing its binding lock.

At world observation, camera timing must still match the same producer,
renderer, owned camera, session, freeze generation, clock identity and scaled
time. Actual output words, cache ambiguity, clock and producer are rechecked
before publishing an owned copy bound to that world sequence. Only the native
frame number is serialized; internal addresses supply no GPU lifetime lease.
The copy reaches CaptureFrame through the retained world's captureControls.

ControlFence.requireCameraTime is optional/default false and omitted when
false. Indexed NativeExportClock now requires it, alongside existing timing,
camera, world-work and frame-input gates, and checks bundle capability first.
Both core/native admission compare copied clock identities and exact finite
f32 output words. Receipt writing and paired comparison recheck requested
timing proof; stable timeline comparison includes whether it was required.
Older manifests remain readable without acquiring this additional proof.

FrameTag JSON now admits up to 64 KiB, replacing the old pose-only 4 KiB bound
for expanded work/material/camera summaries. Controller reports and commands
retain their separate 256 KiB/16 KiB bounds. Software glint/model rejection
guards also now use the existing NativeBridge IllegalArgumentException path.

This connects source observations and capture admission; it does not prove
hook order, native epoch/drift behavior, all animation/texture clocks, pixels
or deterministic export. No existing native hook RVAs/profile checks change.
See inspection/indexed-camera-time-source.json. No builds, tests, syntax
checks, project/game/JNI/shader/GPU execution, process inspection or exports
ran; Bedrock was not opened. Full feature parity remains unverified.

### Software wing glint from observed inputs (source only)

WorldFrame.render now supplies the verified local reference artwork to
Raster3D. Its modelGlint override consumes the same explicit live or recorded
sample used by native packet preparation. JavaArmorGlint can copy the complete
observed texture matrix, fog-position transform, modulator, fog ranges and
strength, without regenerating a wall clock or decomposing the matrix. Fog
distances use the emitted position under that affine transform before clipping.

For selected wing models the raster retains glint attributes alongside the
actual base vertices, copying their exact clip coordinates. It applies the
observed UV/fog transforms before the same clipping operations and queues
triangles in original primitive/part order. It does not rebuild or skin a
second wing mesh. The existing EQUAL-depth, no-depth-write, linear-repeat,
alpha-cutout and source-square RGB blend/retained-alpha path consumes them.

The deferred owned model phase runs after ordinary blended geometry and before
custom glow. JavaModelOrder ports the previously decoded Mth distance/float
comparison and fastutil unstable quicksort, using only the supplied view poses.
Empty selected models remain participants. Known inactive samples are distinct
from unknown; a mixed active/unknown owned wing scope is rejected because an
unknown participant can change unstable ties. An active sample also requires
verified artwork and the layered camera. All-unknown older inputs retain no
glint pass and establish no observed absence or parity.

Model scopes reject late metadata, resumed sinks, partial quads and unclosed
frames. The 2048-model and existing 200000-clipped-triangle bounds include the
added work. No archive/wire format or native suppression receipt changes.
This orders only this software-owned avatar phase, not native foreign world
participants. Complete ordinary phase fidelity, live input capture, native GPU
ownership/insertion, changing replay clocks and camera rebasing remain missing.
See inspection/software-wing-glint-observation-source.json. No build, test,
syntax check, project/game/JNI/shader/GPU execution, process inspection or
export ran; Bedrock was not opened. Rendering and full parity stay unverified.

### Packet glint handoff and per-model native profiles (source only)

WorldFrame now attaches explicit wing glint observations to replacement wing
avatars. Replay uses only recordedWingGlint with its existing exact/stable
context checks. AvatarRenderer passes that metadata before the first wing
sink, and NativeAvatarGeometry retains a copied immutable sample per model.
The builder rejects changes after any sink in the scope. The default draw
target ignores the metadata; Raster3D now has the separate owned phase above.

ARCGEO10 carries a state byte on every batch after model-submit metadata:
0 unknown, 1 explicit inactive, or 2 active followed by the 40 sample floats.
Known samples require a wing model and exact state/bit-pattern agreement
across its parts. Active samples need the layered camera. After all textures,
a strict presence byte distinguishes unknown simple phases from the existing
v9 epoch/phase trailer. Earlier ARCGEO01..09 layouts remain unchanged. Every
new byte is charged to the existing 96 MiB bound.

avatar_glint::packet_observations validates owned selection before optional
artwork/upload preparation. packet_schedule takes every active owned ID once
alongside the caller's complete original foreign participant sequence, using
packet view poses for owned models. It retains the existing full joint sort
and foreign delivery boundaries. Missing observations are errors, not absent
passes; known absence covers only the staged owned wing scope.

GlintGeometry::from_packet assembles independent per-model shader profiles.
Exact first-37-float bit equality can share constant contents; the last three
view-pose words remain ordering inputs. Constants::for_profiles packs at most
2048 profiles into one 256-byte-stride upload arena (512 KiB logical data).
Recording splits only adjacent equal-profile spans in schedule order and
pins the arena through the existing transaction/retirement owners. Constant
deduplication neither establishes PreparedRenderType equality nor permits
regrouping models across another profile or foreign participant.

The installed-game input producer, actual owning insertion callsite, complete
native foreign dispatcher/resource lifetime owner, complete rendering fidelity,
changing replay clock reconstruction and camera-editor rebasing remain missing.
No runtime readiness or parity flag changes. Contract and limits are in
inspection/native-packet-glint-source.json. Source reads/edits/manual review
only: no builds, tests, syntax checks, project/game/JNI/shader/GPU execution,
process inspection or exports; Bedrock remained closed.

### Recorded glint selection and uniform observations (source only)

WorldFrame now accepts optional explicit Player.javaWingGlint target inputs.
GlintSample distinguishes unknown from an explicitly absent pass. Active
selection requires the actual texture matrix, fog transform, ColorModulator,
fog ranges, GlintAlpha and submitted view translation. It validates/copies
40 floats and rejects coercions, nonfinite inputs and inconsistent fog/
strength. Active input needs the existing layered camera; known empty/no-foil
chest metadata conflicts with active selection. Foil=true alone never creates
the profile, and these fields supply no native suppression or game-phase ownership.

GlintHistory participates in the appearance recorder's begin/observe/finish
queue and stores optional bedrock_glint_samples_v1 blocks. Unknown observations
stay absent map entries; known no-pass samples are preserved separately from
active uniforms. Active frames capture dimension, declared position space,
camera and both world/equipment projections. Blocks retain bounded clocks,
128 actors, at most 64 frames and 2 MiB inflation. The reader checks the whole
index/blocks before mount, verifies source manifest on cached reads and keeps
two blocks under its preparation reservation. Existing archives without this
track retain unknown glint. Aggregate appearance preparation is now bounded
to 128 MiB with a 16 MiB glint-reader reservation; the VM heap is unchanged.

ReplayAppearance.wingGlint and BedrockPlatform.recordedWingGlint expose only
recorded exact-time or stable contiguous observations. Active data requires
exact camera/projection context; known absence has no transform to rebase.
Missing actors/blocks, long gaps, changing selection/uniforms/view poses or
incompatible cameras remain unknown. No live viewer clock/options, arbitrary
matrix interpolation across wraps, extrapolation or guessed camera rebase
reconstructs missing glint. Same-time changes fail recording rather than
overwriting observations. A recorded clock model is still required for full
animated resampling and wall-clock movement independent of native tick time.

Recorded samples now reach the optional packet/native preparation path above.
The installed-game observation producer, actual native phase callsite,
verified software/native glint rendering, proven camera-editor rebasing and full
deterministic replay/export parity remain unfinished. Track availability is
data status; playback verification
stays false and no readiness gate is relaxed. See
inspection/replay-glint-input-source.json. No builds, tests, syntax checks,
archive round-trips, project/game/JNI/shader/GPU execution or exports ran;
Bedrock was not opened.

### Joint model-phase glint scheduling and retention (source only)

WingSchedule now accepts the complete original order-1/model-phase-7 sequence:
selected owned wing ModelViewPoses and foreign model tokens/view translations.
It validates the owned source IDs and foreign token uniqueness, then calls the
shared Java model sorter once over every participant. It never separately
sorts the owned subset. This preserves foreign-influenced unstable ties. Adjacent
owned models form WingRun steps; every foreign token remains a boundary.
The complete sequence is bounded to 2048 models. Tokens are opaque adapter
identities, never native pointers or ownership receipts.

GlintGeometry::from_joint binds that schedule to its exact uploaded base,
pipeline and constants. Full-model flattening is used only for preflight;
the legacy whole-glint recorder rejects any schedule containing foreign steps.
GlintCursor exposes the next step, records one owned run at a time, and checks
the actual graphics-state ListToken and world scope on every delivery. Errors
poison the cursor against retrying possibly partial commands. Repeated/skipped
or out-of-order foreign acknowledgements fail. The drained result describes
cursor bookkeeping only, not rendering or GPU completion.

The actual native owner draws each foreign step outside Arctic's recording
entry, tracker lock and private-command guard, then acknowledges its exact
token through the tracked wrapper. Acknowledgement performs no foreign draw
and proves none. Every owned run still performs the native world/surface/
camera/device/projection/resource checks, reserves its owners before commands,
and restores the observed native graphics state before returning. Native
commands between runs can therefore update their normal graphics-state shadow.

native_draw_assets now coalesces chunks on one exact unsubmitted recording
lifetime into a shared fence reservation. It requires the same canonical/raw
list, typed ListToken and device, a replayable/nonfailed/nonpending owner, no
submission/queue use and no observed Release. A Reset creates a fresh graphics
identity and reservation. Up to 4096 snapshot/resource/native-owner chunks can
share one lifetime slot within the unchanged 16-slot pool. Returned asset
generations are shared by those chunks; status reports chunks separately.
All chunks remain subject to the existing replayability and actual GPU fence/
native-retirement/device-removal rules. Retired payloads still drop outside
the pool lock; failed or in-flight owners cannot be bypassed by appending.

This ports the generic ordering/interleaving protocol and its recording
retention path. The installed game still supplies no authoritative complete
participant sequence/poses, native foreign dispatcher, actual phase callsite
or resource/descriptor lease. Source labels/tokens/cursor acknowledgements do
not replace those producers. Recorded glint clocks/uniforms and full animation/
replay/export parity remain unfinished. See inspection/joint-glint-phase-source.json.
No builds, tests, syntax checks, project/game/JNI/shader/GPU execution, process
inspection or exports were performed; Bedrock was not opened.

### Phase-specific entity and glint recording (source only)

Cached WingsLayer calls the explicit equipment renderer with start order 0.
EquipmentLayerRenderer increments the order for each rendered base and glint,
so the supported first wing base is order 0 and its glint order 1. The shorter
general equipment overload defaults to order 1 and is not the wing callsite.
GLINT has a blend function; RenderType.hasBlending therefore routes its model
submit to translucentModels phase 7. This precedes Arctic glow's order 1
custom phase 8 and differs from simply drawing glint after the whole frame.

Phase::owned_avatar_ordered now retains the owned source collector's numeric
order and phase locations. Geometry maps those phases to feature-group indices;
Plan maps each group to its contiguous output-range slice. prepare_groups
keeps complete geometry/material preflight and selects only increasing unique
source groups. record_phase emits that current entity phase through the same
native state/surface/camera/device/ownership gates as the full recorder. The
legacy whole-frame recorder rejects known owned geometry spanning multiple
phases. Unkeyed inputs do not fabricate a phase mapping.

avatar_glint::order_wing_models validates explicit selected wing IDs in
original model order and uses the shared model_order implementation for actual
submitted view translations: float Mth distance and fastutil unstable sort.
GlintGeometry owns the upload-complete base Geometry, actual glint pipeline/
constant resources and selected order. Its target must match the base target.
record_glint only accepts the supported wing (order 1, phase 7) location.

Entity full/phase and glint entries share a recording transaction that captures
restorable native state, pins complete geometry/glint/surface/restore/native
owners through native_draw_assets before any commands, rechecks the exact
cohort/state and restores state on completion or unwind. Depth-tracker wrappers
hold the existing open-list state lock and reject private-command reentry.
Phase labels validate source membership; they do not establish a real native
callsite, target lease or that matching base depth was drawn. No runtime caller
was added and no glint/native drawing verification is claimed.

The exclusive model helper covers the explicit owned selection. Joint phases
now have a complete-input WingSchedule/GlintCursor source path described above;
the actual joint native participant/pose capture and dispatcher remain missing.
An unstable sort of an owned subset cannot replace that complete sequence.
Actual layer view poses, selection, replay-recorded
glint clock/texture transform/options/fog, complete native ownership and live
insertion remain unfinished. See inspection/native-avatar-phase-recording-source.json.
No builds, tests, syntax checks, shader/JNI/Java/game/project execution, GPU
commands or exports were run. Bedrock was not opened.

### Native armor glint on owned wing geometry (source only)

avatar_glint now supplies a separate explicit-input HLSL/pipeline source port
for Java's GLINT material. Cached RenderPipelines data establishes no culling,
EQUAL depth with no writes and BlendFunction.GLINT: RGB SRC_COLOR/ONE, alpha
ZERO/ONE. The shader uses position/UV, supplied texture transform, fog and
ColorModulator/GlintAlpha. It samples linear-repeat, discards modulated alpha
below .1, and fades RGB by total fog and strength. Vertex tint/actor opacity,
normals, entity lightmap/overlay and fog-color RGB mixing do not participate.

JavaReferenceAssets.armorGlintRgba and NativeBridge's guarded getter export a
bounded 65536-byte straight RGBA copy from the already verified configured
local Java archive. ClientVm.armor_glint_artwork checks the JNI array length
before copying, then GlintArtwork validates the pixel hash. This helper is
unscheduled and supplies no material selector. The PNG remains outside this
repository. Its pinned palette-only resource has no transparency or color
profile/gamma metadata; the statically decoded pixel hash is documented in
inspection/native-armor-glint-source.json. Java/JNI agreement is unverified.

avatar_gpu_upload::begin_with_glint accepts that optional verified artwork,
adding one dedicated texture/descriptor to the existing owned upload/fence
path. Packet texture handles cannot alias the extra slot. Straight RGBA rows
avoid the packet ARGB swizzle. Frame texture limits stay unchanged, with at
most 16384 extra art texels inside the same 128 MiB upload cap. Existing begin
delegates without art; all-empty geometry still produces no GPU ticket.

Each glint constant profile uses one 256-byte aligned slice, encoding projection,
texture matrix, fog transform, modulator, ranges and strength without an
implicit clock/options fallback. They bind to the actual uploaded vertex/
index resources and descriptor heap. The wing equipment projection uses the
same reverse-Z-aware helper as the base draw. Pipeline.prepare_wings takes
whole selected model IDs in their actual owning glint order, rejecting
duplicates, missing/non-wing/unknown-eligibility models, unavailable art and
changed upload/projection/device/node/buffer state. It reuses the exact base
geometry, UVs and indices rather than rebuilding an approximate second mesh.

Prepared borrows all inputs and records on the caller's matching recording
list under an explicit unsafe contract. The owner must supply actual glint
selection/phase placement, already-written matching base depth, native state
restoration, complete native target/resource ownership and draw-fence pinning.
No private list is submitted by this module, and no automatic post-frame or
cosmetic-overlay invocation was added. Original glint phase/bucket ordering,
live/replay texture transform/clock/options/fog capture, native insertion and
full armor/held-item rendering remain unfinished. Shader compilation, EQUAL
depth/raster equivalence and rendering/replay/export parity are unverified.

No builds, tests, syntax checks, shader/JNI/Java/project/game execution, GPU
commands or exports were run; Bedrock was not opened. The optional source
constructors contain future API operations but were not called in this work.

### Entity overlay vertex-to-fragment path (source only)

Cached Java entity.vsh emits the selected overlay texel as an ordinary smooth
vec4 overlayColor output, and entity.fsh reads its matching interpolated input.
The prior port applied the selected texel directly in the fragment stage.
avatar_entity.hlsl now emits Fragment.overlay from vertex_main and uses that
perspective-interpolated input in pixel_main. It is not a flat varying.

Raster3D likewise snapshots the common NO_OVERLAY texel or independent skin
texel into RGBA vertex attributes at internal offsets 19..22. Homogeneous
clipping handles those attributes with the existing varyings; the raster loop
perspective-interpolates all four before JavaEntityShader.fragment. The old
triangle-level overlay override is removed. Interpolated values are not
renormalized/clamped to the source normalized-input range. Wings omit this
mix; glow keeps it while bypassing the lightmap. Sample-alpha cutout and
texture/face/modulator multiplication still precede overlay, then lightmap
and fog follow without an overlay change to fragment alpha.

The explicit selected-texel input, draw-constant bytes, GPU vertex input,
geometry packet and replay archives retain their layouts. This ports the
missing shader-stage boundary rather than introducing an arbitrary UV1 or
overlay texture source. Bitwise interpolation and cross-backend pixel
equivalence remain unverified. Live material/lighting/view inputs, native
world/resource ownership and insertion, and full replay/export parity remain
unfinished. See inspection/entity-overlay-varying-source.json. No builds,
tests, syntax checks, shader compilation, GPU commands, game/project execution
or exports were run; Bedrock was not opened.

### Owned avatar simple phases (source only)

JavaSimplePhases is now retained by BedrockPlatform and participates in the
serialized WorldFrame.geometry staging path. It uses the actual owned
JavaRenderType objects as HashMap keys, retaining Java's bucket traversal
and per-bucket insertion order rather than sorting IDs/textures or recreating
maps every frame. Every model's part batches stay one submission; an emote
hiding the cape model BODY now keeps an empty cape submit. Worn/base/bent
submissions use order 0 custom phase 8, solid cape/wing models use phase 0,
and glow uses order 1 custom phase 8.

The collector ports FeatureSubmits.clear: remove already-empty bucket lists,
clear/retain used lists. Empty phases skip draining and retain their keys
while another supported phase keeps the numeric order alive; a fully empty
order is discarded. Session/dimension changes, known world gaps, unsupported
inputs and geometry failures invalidate history. The next valid collection
starts a new positive monotonic collector epoch. These are this renderer's
geometry-collection drains, not a claim about every native game frame or
an independent Java game's object hashes/cache history. Debug shuffle and
foreign feature participation are not included.

ARCGEO09 retains v8 batches and appends the known simple-phase output after
all textures: epoch u64, count u8, then order/stage u8, batch count u32 and
batch indices u32 for each nonempty phase. Valid ascending keys are (0,0),
(0,8), (1,8). Byte/count limits stay within the existing frame budget. The
decoder requires complete supported simple-role coverage, reference bucket
contiguity, per-bucket original submit order and known supported references.
Older packets keep unknown simple phases; unsupported input falls back to
the earlier packet versions without claiming a known traversal.

Phase::owned_avatars combines this metadata with explicit owning feature
references, preparations and Java-equivalent skin-model view poses. It joins
order 0 solid(0), unstable translucent models(7), custom(8), then order 1
glow(8), preserving distinct phase boundaries and their strictness. The
Geometry::from_owned_avatar_inputs entry follows through the existing exact
uploaded-frame, draw-plan/material/pipeline preflight. It records no commands.

This completes a source producer/transport/consumer path for the owned
supported avatar simple phases. It does not supply native world insertion,
foreign/joint world phase ordering, prepared GPU equality, Java view-pose
equivalence or native resource ownership. Those remain unfinished; live
integration and full rendering/replay/export parity are unverified. See
inspection/java-simple-phase-source.json. No builds, tests, syntax checks,
Java/project/game execution, GPU commands or exports were run. Bedrock was
not opened.

### Model submission boundaries and translucent phase sorting (source only)

AvatarDrawTarget now brackets normal skin parts, one cape model, and a wing
pair as separate model submissions. NativeAvatarGeometry retains their
frame-local identity and copies each root's translation in the existing
geometry coordinate space. An all-hidden skin emits an empty batch, retaining
the model's phase/consolidation boundary. Model scopes cannot nest, change
actor, contain custom geometry or resume after closing.

ARCGEO08 adds a strict per-batch presence byte after render-reference metadata,
then optional model ID u16 and root x/y/z f32. The decoder validates role,
finite translation, increasing/contiguous identities and unchanged actor,
pass and bitwise translation within a model. Versions 1..7 retain unknown
model boundaries. The existing frame and byte budgets include this metadata.

avatar_draw_order::Phase::translucent_models ports the model-phase algorithm
over these boundaries and explicit owning inputs. It requires original model
and part order, one render/prepared/consolidation/sort state for each model,
and an actual Java-equivalent submitted view-pose translation. It computes
Mth's float (x*x+y*y)+z*z and reproduces fastutil 8.5.18's unstable comparator
quicksort, including selection-sort and median thresholds and equal-key range
swaps. Whole models move together; the resulting outputs are strictly ordered
and enter the existing FeatureGroup::from_phases assembly. The Plan keeps all
parts on one vertex builder even when different models cannot consolidate,
and rejects split models, reordered parts or changed builder inputs.

This is separate from the existing stable per-draw quad sort. A copied
Bedrock root, including a camera-relative root, does not itself establish
Java view-pose equivalence. That pose producer, native/foreign phase history
and participation, prepared GPU equality and
native world insertion/resource ownership remain unfinished. Outline/ghost
variants and live rendering/export parity remain unverified. See
inspection/java-model-submit-source.json. No builds, tests, syntax checks,
game/project execution, GPU commands or exports were performed; Bedrock was
not opened.

### Replay playback speed and camera time (source only)

The native clock now has a source-only normal-playback rate operation, exposed
through NativeClockControl and the existing native-clock CLI JSON path. A rate
of 0 requests pause; finite rates through 16 multiply the bound timer's existing
game multiplier during its original advanceTime call. The cached routine loads
that field at +0xc for tick calculation and does not write it. Original clock
bookkeeping still executes, then the prior multiplier bytes are restored and
checked. This does not replace the native clock, overwrite its TPS or remove
the native ten-tick cap.

Commands carry the existing session/sequence/request identity and expiring
lease. Acknowledgement waits for an application/restoration tag from each
selected simulation/real-timer call, including alias handling. Rate and held
export stepping reject overlap. Resume, expiry, stale binding or application
failure release the rate; restoration failure blocks further control until a
new binding. Indexed-export admission also rejects an active rate or failed
restoration. One operation JSON shape is
`{"op":"rate","rate":2,"realTimer":true,"leaseMs":5000}`; this was not run.

The worker-owned PlaybackClock now has a CLI coordination path. An explicit
`native_clock_attach` replay control with a loaded bundle correlates one
initialized helper viewer with the native dispatcher through a unique marker,
then acquires the native rate with the original helper pause/speed. Subsequent
pause/speed commands validate both endpoints, confirm the requested helper
state and await native rate application. Once-per-second service reads fresh
correlated helper state and renews a five-second lease. End-of-replay service
pauses its one caught-up viewer. Acknowledged request identity and sequence are
rechecked before renewal/release; expiry or takeover never authorizes resuming
a different rate, export freeze or world.

The CLI releases the coordinator before seek/reset/exact steps, replay-file or
indexed-video export, and shutdown. Drop attempts owned release on the worker;
native lease expiry remains independent. Failed attach can leave the helper
paused; uncertain native operations are not treated as owned acknowledgements.
The launcher replay editor now exposes the shared coordinator through worker
jobs for attach, service, controls and release. Each job temporarily owns the
one replay connection and a passive clock handoff; the UI performs no native
clock IO. Successful results must match the pending job UUID and edition before
restoring resources. Background polling drains lifecycle state before scheduling
roughly one-second service. Failed/inactive leases stay available for recovery
without an automatic retry loop.

Indexed export takes the passive clock handoff and releases it on its worker
before dialogs or held-clock capture. Seek/reset/exact stepping and camera-file
export release before forwarding; file choices arriving during a clock job are
queued with their replay generation instead of being discarded. Stop/close
paths and unconsumed results schedule owned cleanup on workers, with native
lease expiry independent of app survival. See
inspection/replay-playback-ui-source.json. The UI handoffs and lifecycle remain
unbuilt and untested. Exact helper/native transition/epoch alignment and drift
still require validation. See
inspection/replay-playback-clock-coordinator-source.json.

Native application, restoration, tick-cap behavior and all
simulation/rendering/export fidelity remain unverified. See
inspection/native-playback-rate-source.json. No builds, tests, syntax checks,
control operations or game execution were performed.

Replay speed controls now use Java ReplayViewer's seven steps from 0.25x through
16x. The Rust playback admission and helper's initial/live speed validation
accept up to 16x; existing continuous speeds down to 0.1x remain accepted. The
connection form and replay editor share Slower/Faster controls, with endpoint
buttons disabled and non-preset inputs stepping to the next preset in the
requested direction. Normal helper playback now caps nonnegative real elapsed
time at 250ms before applying speed, matching Java's hitch cap. Normal camera
sampling and cameraTarget retain the fractional elapsed playhead instead of
the packet receipt's integer floor. Explicit indexed-export cameraTime still
takes precedence and retains its rational timeline. Native simulation/animation clock
scaling and pause fidelity are still separate, unverified work; this source
change does not establish 16x native-game parity. See
inspection/replay-playback-controls-source.json. No code was executed.

### Software quad primitives and ghost skin inputs (source only)

The software avatar path now preserves Java's second quad primitive as 2,3,0
before clipping, matching NativeGeometryPacket and the native draw planner.
The prior cyclic 0,2,3 rotation kept winding but changed the clipping and
interpolation anchor. This source correction is not a pixel-fidelity receipt.

Further cached Java renderer inspection establishes that ghost skins cannot
reuse the visible skin material: LivingEntityRenderer selects the ghost branch
using body visibility and isInvisibleToPlayer, multiplies 0x26ffffff by the model
tint as integer ARGB, and uses entityTranslucentCullItemTarget with the distinct
ITEM_ENTITY_TARGET. Outline selection follows the ghost and visible branches.
The separate target/composition path, selected color and viewer-specific
visibility still need producers before this material can be ported faithfully;
ActorFlags::Invisible and native alpha are insufficient. See
inspection/java-skin-material-source.json. No classes or shaders were executed.

### Retained emote millisecond phase (source only)

AppearanceSnapshot previously retained only float seconds for an advancing emote.
ReplayAppearance and native EmotePrograms then added new elapsed seconds to that
already rounded phase. Cosmetics.Playing now offers a snapshot sample that reads
its playback clock once and derives both float seconds and double elapsedMillis
before seconds rounding. The regular live seconds() path remains unchanged.
New unheld emote states retain that optional millisecond phase. Held poses retain
only their explicit fixed seconds. Appearance change keys exclude both advancing
phase fields while retaining the stable original start, so phase progression does
not itself create an appearance event.

ReplayAppearance adds replay elapsed milliseconds to a retained millisecond phase
before dividing by 1000 and narrowing once to float. Published native callbacks
and buffered render-program matching use the same calculation. Old archives
without elapsedMillis keep their existing float-seconds-offset calculation;
missing precision is not reconstructed. Saving a mounted new state advances both
phase fields so later loading uses the new snapshot origin consistently.

The optional phase must be numeric, finite, nonnegative, bounded to 24 hours and
absent for held poses. Its float-seconds projection must agree with the stored
seconds. Callback seeds validate the same invariant and include the retained
phase in their identity comparison. Existing actor/session leases, body-history
intervals, native pose receipts and capture readiness requirements remain in
effect. This is an optional logical-state field within the existing archive
format, not a new native equipment or animation verification claim.

No build, tests, helper, game, encoder or export was run. Live/replay phase
agreement, native emote fidelity and deterministic video remain unverified.
Source scope is recorded in inspection/emote-phase-source.json.

### Cape follower discontinuities and yaw sampling (source only)

Static decoding of the cached Java 26.2 ClientAvatarState.moveCloak method shows
independent axis handling: copy the old cloak endpoints, snap an axis's old and
current cloak values to its new position when its displacement is strictly above
10 or below -10 blocks, otherwise advance that axis by displacement*0.25. The
native cape simulation previously reset the entire Simulation and returned early
when any axis crossed that threshold. It now applies the per-axis rule, retaining
the other follower axes and continuing the existing walk, bob and flight updates.
The separately owned wing filter continues as before. Address/session changes
and missed-tick recovery keep their existing explicit reset behavior.

The actor's previous/current positions remain distinct when the cloak snaps.
Consequently the interpolated cloak-minus-actor displacement can exceed the old
64-block admission limit, even though all endpoint positions are valid. Native
motion and Java motion/history validation now accept finite displacements up to
the 64000000-block span of the bounded world endpoints. Cape pose angle clamps
and endpoint position bounds remain unchanged. Existing teleport assertions were
updated for retained Z following and interpolated X displacement; they were not
run and are not native parity evidence.

AvatarRenderer.extractCapeState calls Mth.rotLerp for body yaw. Static decoding
of rotLerp and wrapDegrees establishes signed float remainder by 360, subtraction
at >=180, addition at <-180, then previous+partial*turn. Native and recorded cape
sampling now retain that operation order, eliminating the extra +180/-180
rounding steps. Native flight scale sampling uses an explicit float square before
division, matching the existing Java snapshot sampler's source expression.

These source changes do not establish complete semantic agreement of native
movement, stance, flight or bob inputs with Java. Archive layouts and native
replacement/export readiness gates are unchanged. No build, tests, game launch
or export was performed. Teleport rendering, native cape fidelity and
deterministic video remain unverified. Source scope is recorded in
inspection/cape-physics-source.json.

### Cape walk and bob length arithmetic (source only)

Static class decoding establishes LocalPlayer.move's arithmetic: subtract double
X/Z positions across the movement call, narrow those two displacements to float,
call Mth.length(float,float), multiply the float result by 0.6f, and add it to
ClientAvatarState.walkDist. Mth.length widens its float inputs back to double,
sums their double squares, calls Math.sqrt and narrows the result once. The
native cape filter previously squared and summed in float. It now follows the
double-intermediate order after its existing displacement narrowing.

AbstractClientPlayer.updateBob uses Vec3.horizontalDistance: double x*x+z*z,
Math.sqrt, narrowing to float and Math.min with 0.1f. The native filter now uses
that square/sum/root order on its widened native float velocity components,
instead of hypot. Its existing ride/ground/death/swim conditions and 0.4f bob
filter remain in place; their complete native semantic agreement is unverified.

This aligns the source arithmetic for the inputs available to the filter. Java
adds walk distance per LocalPlayer.move call; the native port still derives one
displacement at a completed tick. Multiple movements within a tick, the timing
of cloak updates relative to movement, original Java double position/velocity
precision and unobserved prehistory remain fidelity gaps. No movement hook or
missing path is inferred from a tick endpoint. No build, tests, game launch or
export was run. Runtime movement/animation parity and deterministic video remain
unverified. Source scope is recorded in inspection/cape-physics-source.json.

### Native per-movement callback investigation (static only)

The Java LocalPlayer.move requirement needs each actual movement call's before
and after X/Z positions. The pinned public Actor header declares move(Vec3 const&)
as a nonvirtual method, so a guessed actor vtable slot cannot locate that method.
The declaration alone establishes neither its installed client address nor
complete coverage of the native local player's physics/collision paths.

Cached installed-image tracing found the moveRelative string at RVA ec545f1,
referenced at d37364f in registration function d370b10..d37e67f. Its associated
callback pointer loads at d373644 and targets d386cc0..d386d41. That callback calls
4a29c00 at d386cf5. The callee branches on owner byte +f38, normalizes its supplied
vector, and constructs command/variant state associated with owner +f10 before
calling 4a9e7a0. This suggests a command-facing input route; it does not establish
an actual synchronous displacement consumer or the movement coverage needed by
the cape's walk accumulator. This candidate is not admitted for a hook.

No native movement hook was installed or added, and no tick endpoint was
promoted to evidence of a complete movement path. A future binding must establish
same-actor before/after position ownership, completion, reentrancy and relevant
movement-path coverage. No live process inspection, build, tests, game launch or
export was performed. Evidence and the unresolved coverage boundary are recorded
in inspection/native-movement-candidates.json.

### Java cape equipment-layer branches (source only)

Static decoding of Java 26.2 CapeLayer.hasLayer establishes its component/asset
lookup: read EQUIPPABLE, require an asset ID, resolve that selected equipment
asset and test whether its requested layer list is nonempty. CapeLayer.submit
first hides the cape for a WINGS layer. Otherwise, a HUMANOID layer translates
the entity root by (0f,-0.05312500149011612f,0.06875000149011612f) before the cape
model's body pose. Item names, native armor occupancy or gliding do not establish
those selected Java asset layers.

JavaCapeLayer now selects HIDDEN, OFFSET, PLAIN or UNKNOWN from explicit copied
layer results. WorldFrame.ItemSlot can retain optional javaCapeLayers with a
versioned Java reference and nullable wings/humanoid booleans. These inputs need
a copied nonempty item identity, are validated and copied on frame/archive load,
and participate in known recorded equipment matching. Existing serializeNulls
capture retains explicit unknown flags. Replay validation checks their exact
fields/types; old archives without this optional field remain readable. Replay
avatars use the selected recorded equipment state rather than live viewer facts.

The software cape renderer applies a known HUMANOID translation to the entity
root before multiplying the posed body and cape-local matrices, preserving that
translation's stage under a turned/emote-posed torso. It uses the default
non-FMA JOML translateGeneric rightward float grouping decoded from JOML 1.10.8.
A known WINGS layer hides the ordinary cape. UNKNOWN retains the legacy plain
software pose and establishes no native replacement authority. Status exposes
selection counts and explicitly reports the missing native layer-input producer.

The installed native reader does not yet provide these Java component/asset
facts. Native empty-chest suppression/readiness gates are unchanged; this does
not enable or verify armored native cape replacement. Native pose-stack property
flags, complete model/material fidelity and deterministic video remain
unverified. No build, tests, game launch or export was performed. Source scope is
recorded in inspection/cape-equipment-layer-source.json.

### Referenced equipment payloads in appearance recordings (source only)

Equipment had been inlined through AppearanceSnapshot.Captured.withPlayerData's
generic 8 KiB actor limit. Six detailed slots can exceed that limit once raw
enchantments, projected components, foil inputs and cape layer inputs are kept.
NativeEquipmentCapture now retains each actor's full equipment JSON as a bounded
64 KiB asset and attaches bedrockEquipmentSource {schema:1,hash} to logical state.
The existing hash-addressed archive writer owns/copies that asset. References
participate in appearance change detection, so an equipment change remains an
event while unrelated look edits can reuse the same gear payload. Raw list order,
nullable fields and explicit unknown states are retained rather than truncated.

The capture helper keeps at most 256 SHA-256-keyed references scoped to its
AppearanceAssets owner, avoiding repeated cache writes for unchanged payloads.
It keeps no payload/engine pointer in that memo. Asset cache eviction remains
observable: history/snapshot writing still reads and verifies retained bytes and
fails if a required asset is missing, rather than omitting equipment.

ReplayAppearance accepts existing inline equipment and the new source descriptor,
rejecting conflicting non-null sources. Descriptors require exact schema/hash
fields. Equipment assets use a narrower AppearanceAssets.Archive.read overload:
the manifest size is checked against 64 KiB before allocating entry bytes, then
bounded read, exact size, content hash and independent SHA-256 checks apply.
Both inline and referenced inputs use the existing exact equipment field/type
validation, followed by item/enchantment/projection/layer semantic validation.
Validated referenced data stays private and read-only after preparation and is
shared by content hash across events. Its preparation charge of bytes*8+512 is
applied once against the existing resident budget. Equipment matching, cape layer
selection and re-saving mounted snapshots use the resolved data/references.

Generic actor, manifest, archive, history queue and resident limits are unchanged.
Current readers retain support for old inline archives; old-reader compatibility
with new source descriptors has not been established. No build, tests, archive
round trip, game launch or export was run. Capture performance, native rendering
parity and deterministic video remain unverified. Source scope is recorded in
inspection/equipment-recording-assets-source.json.

### Indexed encoding configuration in export intent (source only)

Frame timestamps already use integer 100-nanosecond boundaries derived from the
frame index. Source review found that encoder kind alone did not bind the
indexed worker's configuration: the built-in stream encoder could attempt
hardware then software, influenced by an environment setting. Indexed native
capture now invokes a versioned indexed_mf_hardware_disabled_v1 stream mode.
That mode requests MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS=0 directly, with no
hardware-enabled attempt or environment override. A launcher that does not know
the mode rejects it. Existing realtime and legacy stream modes keep their prior
selection policy. This can increase indexed encoding cost; performance is untested.

The profile mf_hardware_disabled_h264_v1 describes the attribute request, not a
verified selected transform or output guarantee. Microsoft's
[attribute documentation](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-readwrite-enable-hardware-transforms)
explains an exception for transforms registered within the process. The selected
encoder, process-local registrations, internal threading and OS implementation
are not established by this profile. Indexed FFmpeg retains libx264 preset fast,
CRF 18 and existing color/padding settings, but requests one codec thread and
output bitexact flags. Its profile is libx264_threads1_bitexact_v1; those flags
are not repeatability evidence or a pin of codec/binary versions.

New frame encoding metadata uses schema 2 and a profile field. ExportIntent
schema 3 includes encodingProfile, requires the expected profile for its encoder,
and retains the native tick-rate binding. Its hash uses the v3 domain. Schema-1
and schema-2 intents keep an absent profile, omitted optional serialization and
their prior domains; runtime hash compatibility is untested. The outer receipt
format remains independently versioned schema 1.

Before a new receipt is written, every schema-3 manifest frame must contain
encoding schema 2 and the intent's expected profile as well as the exact indexed
native cadence. Paired comparison applies the same check and reports an
encodingProfile intent difference. Generic video audit still accepts old schema-1
encoding metadata without a profile, while schema 2 requires the backend's named
profile. Native world, terrain, shader and video determinism flags remain false.
No build, tests, encoder/decoder, comparison, export or game launch was run.
Scope and limitations are in inspection/indexed-encoding-profile-source.json.

### Indexed encoding profile handshake (source only)

Native capture status now advertises its compiled indexedEncoding contract:
schema 1, frameMetadataSchema 2 and backend profile names. Active indexed
sessions expose their selected encodingProfile before the first frame/encoder
spawn, and terminal completion/error status retains that declaration after the
Session is released. Real-time capture does not declare an indexed profile.

The export worker checks the already correlated owned reservation reply for the
required capability before rebuilding the world or changing replay/camera/clock
state. It records capture ownership before that check, so a mismatch releases the
reservation through existing cleanup. Activation must confirm the selected
profile before native freeze or frame queuing. Completion must retain the same
profile before decoded audit and export identity binding. Missing/unknown profile
support is rejected rather than assumed from encoder kind or inferred after
capturing a complete clip.

This handshake establishes a request-contract declaration, not execution of the
external encoder, its codec identity/availability, correct decoded output, native
world fidelity or repeatability. Those boundaries remain unverified and the
existing capture/decoded-audit/manifest checks remain in effect. No build, tests,
encoder/decoder, export, comparison or Bedrock launch was performed. Scope is
recorded in inspection/indexed-encoding-profile-source.json.

### Guarded material-info backing pointer (static source only)

Cached data-section type information at 11a36f20 names a material-loader lambda
whose template includes shared_ptr<RenderMaterialInfo>, RenderMaterial and
ResourcePackManager. Its type getter at 5a5b070 appears in table entry e954fa8,
alongside loader callback 5a5a9f0. That callback's 5a5acb6 sequence reads a map's
info pointer at +40 and the info's backing pointer at +40. Independent helper
5a5b080..5a5b266 checks the same field, addresses the HashedString at info+10,
constructs a 0x48-byte info object inside a 0x58-byte shared-control allocation,
initializes its null backing pointer and stores the resulting shared pair.
These sites corroborate the pinned public RenderMaterialInfo declaration. Both
cached text copies used for this inspection have the profile's expected SHA-256.

native_frame_material now copies the 8-byte unique_ptr value at info+40 after
the existing guarded shared pair/control checks. A nonzero value must be a
canonical user address. The reader does not impose the observed in-place
allocation relation on arbitrary aliasing shared pointers. It neither reads the
RenderMaterial pointee nor copies names, takes leases, calls native methods or
changes refcounts. The pointer stays a private endpoint key guarded by existing
byte rechecks and positive-control checks; these observations do not establish
pointee lifetime or ownership.

Chunk receipts now distinguish null/non-null backing-pointer reference entries,
distinct non-null pointers and potentially drawable entries missing such a
pointer. Both native and host requireFrameInput matchers require the new
observation, consistent partitions and no missing potentially drawable backing
pointers. Old reference-only receipts cannot satisfy that prerequisite. No GPU,
shader, actual material ownership or terrain-ready claim is added.

At most one extra 8-byte read per admitted material reference adds 131072 bytes
and 16384 block reads. Combined caps are now 18253056 guarded bytes and 430080
block reads; the 212992 guarded-control cap is unchanged. Missing/unsupported
samples still fail rather than expanding limits. No build, tests, syntax check,
process access, game launch or export was performed. Runtime layout, full native
cosmetic fidelity and deterministic video remain unverified. Evidence and limits
are recorded in inspection/frame-material-pointer-source.json.

### Concrete material shader-path metadata (static source only)

The installed loader helper 5a58d70 allocates a 0x378-byte concrete material and
calls copy constructor 5a5c1c0. Its base table e8f7910 is replaced at 5a5cb58 by
concrete table e8f7550. The five rebased targets are pinned independently; its
parseShader override is 4b0b640. That parser selects vertexShader, fragmentShader
and geometryShader strings into PathBuffer<string> fields at d0/f0/110, agreeing
with cg::RenderMaterialBase and mce::RenderMaterial declarations. The base parser
5968860 uses the same offsets/names. The concrete append-path virtual at 76bc0
is return-only. No parser, virtual function or constructor is called by Arctic.

The observer now admits only that concrete vtable before reading the three
adjacent string headers. It copies each non-SSO string's bounded active bytes,
including its terminator, and registers all reads with the existing endpoint
guards. Paths are limited to 512 bytes, checked for UTF-8/control characters and
hashed with explicit stage/length boundaries. One inspection per distinct
backing-material pointer avoids repeated path copies for layer aliases. Other
classes, unreadable or unsupported strings remain unknown; valid empty stage
paths remain distinct from unknown metadata.

Receipts expose known/unknown object counts, nonempty vertex/fragment/geometry
path counts and a domain-separated fingerprint of copied per-object path digests
sorted with multiplicity. Neither native addresses, traversal order nor path
text is serialized. Completeness refers only to inspection of the selected
nonnull backing objects. A fingerprint of the known subset does not establish
the entire material's state, resolved shader asset identity, per-layer program
assignment, compiled programs, PSO/root bindings, uploads or GPU execution. The
compiledMaterialProgramsObserved/materialPipelineBindingVerified flags stay false.
Path metadata does not add a new requireFrameInput readiness condition.

This bounded metadata stage adds at most 26918912 guarded bytes and 81920 block
reads to the backing-pointer stage. Combined caps are now 45171968 guarded bytes
and 512000 block reads, with the existing 212992 control limit. They do not grow
in response to an unsupported scene. Runtime layout, performance, actual native
material ownership, full cosmetic parity and deterministic video remain
unverified. No build, tests, syntax check, live process inspection, game launch
or export was performed. Evidence is recorded in
inspection/frame-material-shader-paths-source.json.

### Skinned replay material-reference guards (static source only)

Cached skinned constructor instructions at 5e9e8b5 retain the source shared
control, then copy the object/control pair into description+168/+170 at
5e9e8d0/5e9e8da. The synchronous material helper obtains that description from
its third argument at 5ee8ea1, reads its info pointer at 5ee8ecd and follows
info+40 at 5ee8ed9. The fourth/fifth arguments are not treated as material
pointers. The caller at 5e4f967 consumes local output fields after the call,
without using a return value. These additional constructor/consumer sites are
pinned alongside the material profile before the replay consumer is installed.

Preparing a replay shader-input copy now requires a nonempty shared pair and
backing pointer, positive strong/weak controls, the pinned concrete vtable and
readable declared shader paths. It retains only a bounded borrowed byte cohort:
pair, backing pointer, vptr, adjacent string headers and active heap path bytes.
The existing prepared-description checks recheck that cohort at insertion and
around the synchronous local vector patch. A changed or unreadable material
invalidates the replay copy receipt. The original local vector restoration is
still attempted after the helper even when material stability fails. Legacy
RGBA-only preview does not gain this requirement; optional terrain metadata
readiness is unchanged.

One prepared description guards at most 1667 bytes in seven block reads, plus
positive-control checks before/after rechecks. The existing bounded pending and
consumer scopes retain these guards only for native submission. No native lease
is taken, no refcount/material field is changed and no address/path text is
serialized. The checks establish endpoint CPU metadata stability only. They do
not freeze lifetime, compare resource-pack/program identity against the archive,
cover full material state, prove shader asset resolution or establish GPU use.
Those distinctions remain explicit in status, and complete native material
replay, deterministic video and full parity remain unverified.

No build, tests, syntax check, game process access, launch or export was performed.
Evidence is recorded in inspection/skinned-material-reference-source.json.

### Recording and replaying declared material paths (static source only)

Actor shader inputs now optionally carry materialPaths: the guarded three-stage
path digest and boolean nonempty-stage flags. Capture reads the named constant
cohort around description material capture and rechecks the borrowed material
before attaching that fingerprint. Unsupported metadata stays absent. Native
replay requires an archived fingerprint, when present, to equal the live source;
preparation independently matches both source/requested metadata to the same
guarded draw description. The synchronous helper's existing guards still apply.
No addresses, path text or shader assets enter the appearance archive.

ActorShaderInputs preserves metadata through strict owned JSON validation,
copies, exact equality and unchanged interior samples. Changing paths across an
interval makes those shader inputs unknown. Full native-cape replay readiness
now requires a known recorded path fingerprint as well as material context and
the matching per-description copy receipts. Legacy recordings remain readable
and may preview; their unknown original path metadata cannot establish that
prerequisite. Re-recording successful legacy playback may retain the actual
unchanged paths observed during playback, without inventing original metadata.

The writer now emits bedrock_layer_colors_v5 with magic 415243434f4c0005. After
each optional material context, an optional path record stores a strict presence
byte, 32 digest bytes and a stage mask limited to bits 0..2. A fully populated
actor occupies 539 bytes including UUID. New blocks contain at most 14 frames
of 128 actors, totaling at most 966208 raw bytes including headers; the 1 MiB
block cap and 8 MiB reader reservation remain unchanged. Readers retain v1-v4
magic/layout handling and the v3/v4 16-frame bound. Native plan accounting also
includes the dynamic fingerprint string capacity. Status distinguishes track
format availability from actual sample metadata and keeps full identity and GPU
verification false.

This comparison covers declared stage-path metadata only. Unchanged paths can
resolve to different resource-pack assets or compiled shader binaries; defines,
pipeline state and full material state are not compared. GPU consumption, live
layout, archive round trips, native rendering parity and deterministic export
remain unverified. No builds, tests, syntax checks, game process access, launch,
encoder or export was performed. Evidence is recorded in
inspection/material-path-replay-source.json.

### Declared material variation replay (static source only)

Cached material copy instructions at 5a5c575 address the defines set at +98;
5a5c593 copies the adjacent two-byte state mask at +a8. Set copy helper 1185640
constructs a self-linked sentinel, stores color/nil bytes 1/1, copies the size
at +8 and traverses nodes through left/parent/right at +0/+8/+10. Node copy
431ae0 and native define consumer 8ca77a0 independently establish the string at
node+20, length +30 and capacity +38. The consumer starts at head.left and its
successor path checks nil at +19. These sites are now pinned with the profile.
They agree with the pinned public cg::RenderMaterialBase declaration.

native_material_variation copies at most 128 defines of 128 UTF-8 bytes each.
Its bounded iterative traversal checks canonical aligned pointers, sentinel
metadata, parent links, distinct nodes, strict byte ordering, copied size and
minimum/maximum links. It checks both inline and heap strings, including their
terminators, without reading the sentinel's unused string storage. Empty sets
are known; unreadable, oversized or unsupported sets remain unknown. All copied
bytes receive endpoint guards, with outer material/control/path checks before
and after the variation guards. Full red-black balancing is not interpreted.

The domain-separated variation digest includes the raw two-byte state mask,
define count and length-delimited in-order raw define strings. It includes no
addresses, allocation layout, padding or native lazy-cache values. The native
helper normally computes a flags cache at material+258; Arctic neither invokes
that helper nor reads/modifies the cache. Normal lazy initialization therefore
does not invalidate an otherwise stable declared-state cohort. Optional
variation metadata is guarded independently, so an unknown set does not erase
known stage paths. At most 24754 additional bytes and 258 block reads bring the
combined per-description material guard caps to 26421 bytes and 265 block reads.
Terrain path-reader limits and readiness remain unchanged.

Actor shader inputs now carry optional materialPaths.variationSha256. Native
replay requires known recorded variation to match the live source and the
prepared description; known cannot match unknown. Legacy path-only plans may
preview. Full native-cape replay readiness requires known recorded variation as
well as the path/context/copy prerequisites. Exact sample equality includes the
optional variation, so changed or missing metadata cannot be interpolated into
a claimed known value. Re-recording a successful legacy preview can retain its
actually observed unchanged live variation, without inventing the original
recording's missing state.

The writer now emits bedrock_layer_colors_v6, magic 415243434f4c0006. A present
path digest/stage-mask record gains a strict variation-presence byte and an
optional 32-byte digest. A fully populated actor occupies 605 bytes including
UUID. Thirteen frames of 128 actors plus headers occupy at most 1007018 raw
bytes, below the existing 1 MiB cap. The 8 MiB reader reservation remains in
place; v1-v5 readers retain their original layouts and 64/16/14-frame limits.
Native replay-plan accounting includes both digest string capacities. Status
reports format support separately from known samples and full verification.

These checks compare declared defines and a state mask. They do not compare
complete blend/depth/raster state, resource-pack assets, shader binaries or GPU
pipeline bindings. Actual native rendering fidelity, lifetime, performance,
archive round trips, deterministic export and full parity remain unverified.
No builds, tests, syntax checks, live process access, Bedrock launch, encoder or
export was performed. Evidence is recorded in
inspection/material-variation-replay-source.json.

### Finalized movement trace for cape locomotion (static investigation only)

The remaining walk-distance approximation needs actual movement calls, not a
single completed-tick displacement. Cached Java AbstractClientPlayer.tick also
confirms that ClientAvatarState snapshots old walk distance and moves its cloak
before Player.tick. No global client-tick phase is assumed equivalent to that
per-actor phase. RemotePlayer.tick/aiStep do not supply the local add-walk call.

Static native tracing now distinguishes three similarly named routes. The
LocalPlayerPostMoveSystem registration at 681eb46 creates a query with invoke
thunk 90005b0 and per-entity helper 9003670, whose input/flag path reaches helper
20f430. This does not establish displacement commit. ActorMoveSystem helper
9028620 is called by entity wrapper 9028ef0. Matching immediate view keys against
cached entt type-name FNV-1a32 values identifies MoveRequestComponent (152 bytes),
AABBShapeComponent (32) and SubBBsComponent (24). Its AABB/sub-box/request writes
cannot be treated as actual StateVector position changes.

FinalizeMoveSystem registration at 681b8bb instead creates object table e98af80.
Its registry invoke 63db550 calls query 63b4270, whose four typed views are AABB
shape, move request, offsets and StateVector. Their independently matched hashes
and strides are recorded in inspection/native-movement-finalization-source.json.
Four iteration branches call wrapper 63db090. The first branch establishes six
pointer arguments: opaque callback context, entity-dword address, AABB shape,
move request, offsets and StateVector. All four return sites immediately jump
back to iteration without consuming a return value. The wrapper brackets a
dynamic callback dispatched at 63db198 through a target loaded at 63db155.

This is a concrete route to the actual StateVector component, but the callback
assignment, committed position writes, same-actor lifetime and all-movement
coverage remain unresolved. Neither the opaque context nor an AABB address is
interpreted as an Actor. No hook or new walk integration is enabled by this
investigation. The current completed-tick approximation remains explicitly
unverified; these findings do not establish cape parity. All analysis used
cached bytes as data. No builds, tests, syntax checks, live process inspection,
Bedrock launch or export was performed.

### Ordered finalized movement for cape walking (static source only)

The finalizer callback is now statically resolved. Query instruction 63b428b
loads callback 63b3a70 and stores it at query RSP+58. The per-entity context at
63b43e7 stores that slot's address; wrapper 63db090 follows it at 63db155 and
dispatches at 63db198. Callback argument five is the wrapper's actual StateVector
argument six. Stores 63b3ace/63b3ad2/63b3ad7 commit XYZ using the AABB horizontal
center and its minimum Y plus the offsets component. The previous unresolved
target boundary is superseded by these instructions; full movement coverage is
still not established.

native_cape_movement now observes that six-pointer wrapper with the pinned
profile and an unchanged call to the native original. The primary client tick
opens a bounded thread-local scope before its original call. Only a recently
observed local actor from the same session, previous completed tick and UUID is
eligible. Each candidate call must carry that actor's actual ECS StateVector
and AABB pointers and entity/version. Registry identity, level, actor type and
component addresses are rechecked around the native call. The opaque query
context is not interpreted as Actor. Other entities in the query pass through.

The collector copies finite before/after position endpoints and preserves one
float increment per observed finalization. Reentrant matching calls, failed
reads, lock/borrow failures, ownership changes, discontinuous endpoints and
more than 64 calls invalidate the batch. Eight nested scopes are allowed;
unsupported nested levels mask their parent. At completion, a nonempty batch
must join the next completed tick and the refreshed local UUID/final position.
Only owned math leaves the scope; Arctic changes no native component/request,
reference count or engine allocation.

For a sequential cape simulation tick with matching start/end XZ, walk distance
now adds those increments individually in order, starting from old_walk. Each
increment uses Java's narrow-double-delta-to-float, double square/sum/sqrt,
narrow-length and float-times-0.6 order. It is not pre-summed and added once.
The original completed-tick endpoint calculation remains the fallback when
collection or matching is unavailable. Cloak, bob, wing and flight phase/order
remain unchanged. Existing cape snapshots already retain the resulting float
endpoints, so no archive layout change is needed.

Status exposes matched calls, completed/applied batches, rejected batches and
the limit; all-local-movement coverage, Java call equivalence and cape parity
remain false. A native finalizer may encompass/coalesce more than one internal
move. Worker callbacks do not inherit this thread-local scope, and matching
endpoints cannot exclude an unobserved excursion returning to the same position.
Java's pre-movement cloak/old-walk phase and full native route coverage remain
unresolved. No fixture or static analysis establishes live parity. No build,
tests, syntax check, live process inspection, Bedrock launch, encoder or export
was performed. Evidence is recorded in
inspection/native-movement-accumulator-source.json.

### Pre-movement cloak input for matched local history (static source only)

Cached Java AbstractClientPlayer.tick invokes ClientAvatarState.tick before
Player.tick. ClientAvatarState snapshots old walk distance and moves its cloak
using the supplied pre-movement position. The completed-tick native path had
instead supplied the resulting current actor position to that follower step.

The native movement scope now also captures actual feet before its original
client tick: double-widened StateVector X/Z and AABB minimum Y. Direct position
and bounds reads must match the copied actor and survive initial ownership and
endpoint rechecks. These three owned double coordinates join the existing
bounded finalized-movement batch; no extra engine pointer escapes.

For a sequential local cape tick with an admitted matching movement batch, the
filter may use those pre-movement feet only if all three coordinates exactly
match the previous simulation position. Actor rendering still retains its actual
previous/current completed-tick feet. The cloak performs one old/current
snapshot plus independent axis snap/follower step with the selected input.
Ordered walk increments remain separate float additions. Bob, wings, flight,
gaps, warmup and duplicate-tick behavior retain their prior rules. If feet Y or
history does not match, the completed-tick cloak input remains the fallback;
validated horizontal movement can still contribute walk increments separately.

This changes the input supplied to the filter, not the engine's scheduling:
Arctic still calculates the completed snapshot after the original tick returns.
A discontinuity committed inside a matched tick therefore does not prematurely
substitute the teleport destination for that tick's captured cloak input. Axis
snapping applies when that destination reaches a later admitted input. Existing
cape history already stores the resulting old/current cloak and actor endpoints,
so recorded replay retains the result without a format change or live follower
reconstruction.

Status counts preMovementCloakBatches but preMovementCloakPhaseVerified remains
false. Full actor-tick phase equivalence, remote-player input phase, worker
coverage, coalescing and unobserved position excursions remain unresolved.
Native float quantization is retained. No builds, tests, syntax checks, live
process inspection, Bedrock launch, encoder or export was performed. Runtime
cape/teleport rendering, archive round trips, deterministic video and full parity
remain unverified. Evidence is recorded in
inspection/cape-pre-movement-phase-source.json.

### Remote cape inputs from actual previous-position copies (static source only)

Cached ActorUpdatePreviousPositionSystem registration at 67fcf7a creates table
ea50510, whose registry invoke is 9035180. Its view uses StateVectorComponent
and ActorMovementTickNeededComponent, independently matched from immediate
FNV-1a32 keys to cached entt type names. SetPreviousPositionSystem registration
at 682e182 creates table ea34f70 and invoke 8b5ada0. Both load the same callback
8b30500 at 90351c6/8b5adfe. That complete 14-byte leaf copies current StateVector
XYZ into its previous-position fields without changing current position or AABB.

The shared three-pointer per-entity wrapper at 31cb800 forwards its R8 component
pointer into that leaf through an opaque callback slot at context+10. Its
cached direct callers continue iteration; no intended return value is identified.
native_cape_phase now observes the wrapper only when that slot resolves exactly
to the pinned copy leaf and the component belongs to a tracked remote player.
Other callbacks remain uninterpreted and every native original call passes
through with its three arguments unchanged. The wrapper/slot instructions,
entire copy leaf, both literal callback references and typed view keys are pinned.

Before each primary client tick, at most 128 recently observed nonlocal actors
with sequential cape history become candidates. A known local identity is
required so it is excluded. Lightweight UUID/player-directory joins validate
session, epoch, level, actor type, registry identity and entity version without
copying equipment or appearance assets. Actual StateVector/AABB component
addresses bind each copy. The observer retains finite position and feet from
the event, requiring the native original to leave current position/AABB unchanged
while previous XYZ become that copied position. Duplicate events invalidate
the actor even when identical. Failed reads, ownership/identity changes, callback
panics and lock failures leave its input unknown. Eight nested tick scopes are
bounded and unsupported scopes mask their parent.

At the next completed client tick, engine previous position must still equal the
observed copy. The owned per-UUID batch contains only directory epoch, tick and
copied/final position math. A remote cape uses those feet only when they match
all axes of prior filter history, and both current/previous actor positions match
the captured endpoints. Rendering retains actual completed-tick position
endpoints. Unknown samples keep the existing cloak-input fallback. No remote
walk distance is invented; local ordered movement, bob, wings and flight retain
their existing rules. Existing recorded cape endpoints preserve the resulting
filter output without an archive-format change.

Status exposes remotePhase captured/applied/rejected counts with verification
false. Complete system scheduling, Java actor-phase equivalence, worker scopes,
repeated resets/interpolation routes and hidden returning excursions remain
unverified. Native float quantization remains. No builds, tests, syntax checks,
live process inspection, Bedrock launch, encoder or export was performed.
Runtime rendering, performance, archive round trips, deterministic video and
full parity remain unverified. Evidence is recorded in
inspection/cape-previous-position-source.json.

### Owned avatar geometry staging (static source port)

AvatarRenderer.geometry now emits the shared skin, emote-posed limbs, cape,
wings and worn cosmetic models into NativeAvatarGeometry. AvatarDrawTarget
separates model emission from projection and rasterization. Raster3D remains
the software implementation; AvatarPoseSink supplies both implementations with
the same point and normal transform arithmetic. JavaWingModel's base geometry
uses that shared target too. The separate explicit-input glint path remains
software-only.

The result owns vertex attributes, triangle indices, colors, packed Java light,
material tint and texture snapshots. Batches retain emission order and separate
general, cosmetic base/glow, cape-solid and wing-cutout roles, including alpha
cutout, culling, per-face lighting and equipment-camera requirements. No shading
or projection is baked into positions. Java normal quantization is retained for
the corresponding material passes. Quad topology, including MeshDraw's padded
triangles, survives unchanged. This output needs renderer-side translucent/glow
ordering and actual native state selection; symbolic roles are not engine enums.

Each transaction allows at most 2048 draws, 400000 vertices, 32 textures and
16777216 total texture pixels with a 4096-side bound. Nonfinite vertices,
incomplete quads and exhausted budgets reject the transaction. Results expose
defensive array copies and immutable lists; no borrowed model arrays, mutable
BufferedImages or game pointers escape. The caller's avatar/emote time drives
animation without adding another wall clock. This alternate entry point is not
yet transported through JNI or consumed by the running adapter.

Cached instructions establish the native Mesh constructor at 4afcae0 and its
unique direct caller at 59cc439. The constructor accepts a shared buffer-service
pair, an rvalue MeshData, a temporary flag and an indirect string_view. It keeps
a weak service pair and moves MeshData at 4afcc87. The native copy constructor
instead calls MeshData's deep-copy constructor a002c0, which uses the engine
allocator and its large-allocation alignment bookkeeping. Java/Rust allocations
must never be handed to the engine's move constructor or destructor.

The preparation path at 4afced0 builds/copies the opaque 110-byte vertex layout
and calls the CPU packing routine at 4b001a0. The path at 4b02e70 locks the weak
service reference with a conditional strong-count increment, prepares data,
calls 4afd9f0 for nontemporary data and releases the lease. The public service
declarations are opaque. The helper at 4afee90 clears/releases vertex and
attribute resources; it is not an alternate upload path. The small constructor
at b7161b0 allocates internal native storage and is not established as a GPU
vertex-buffer creation API.

Native service acquisition, allocator/unwind integration, complete packing,
texture/material binding, submission ownership and GPU lifetime remain
unfinished. The existing runtime still uploads the software RGBA layer. No
builds, tests, syntax checks, project/game execution or Bedrock launch occurred.
GPU rendering, deterministic video and full parity remain unverified. Evidence
is recorded in inspection/native-avatar-geometry-source.json.

### World geometry JNI packet (static source port)

NativeBridge.worldGeometry(session,sequence) now exposes a copied CPU-geometry
packet for an exact current Java world snapshot. Gaps, unavailable input and
session/sequence mismatches return null. WorldFrame.emitAvatars centralizes the
existing appearance, acknowledged emote, cape-frame and replay selection for
both software rendering and geometry staging. A single builder across visible
actors enforces the existing geometry/texture budgets for the whole packet,
deduplicates texture snapshots by image identity and assigns canonical UUIDs.
Draws cannot resume after a later draw or actor, preserving emission order.

The ARCGEO01 little-endian contract carries session/sequence, world clock,
cosmetic math time, exact optional replay fraction, dimensions, coordinate-space
declaration, camera, projection matrices, actor IDs, material-role batches,
interleaved vertex attributes, canonical quad indices and ARGB texture snapshots.
Pass IDs are protocol values, never inferred Bedrock enum values. Missing camera
space remains unknown. Replay time retains its integer remainder/denominator;
converting it to f64 is reserved for pose math. Packed light and unbaked tint
remain separate. Texture pixels are little-endian ARGB u32, not RGBA bytes.

ClientVm.geometry contains the JNI request/copy path. It checks the returned
array length before copying. The avatar_geometry decoder rejects packets over
96 MiB, stale identities, invalid clocks/fractions, nonfinite values, unknown
roles, incompatible flags, duplicate actors, invalid texture references,
noncanonical topology, exhausted budgets and trailing bytes. Its retained byte
allocation is Rust-owned; ranges expose attributes/indices/pixels without
turning them into native C++ vector allocations.

The request is intentionally outside the presentation loop until native resource
and submission ownership are implemented. It neither requests geometry every
frame nor replaces the existing software layer, and it grants no suppression
authority. A session/sequence match is not proof of a native producer lease,
GPU readiness or submission. The future consumer must establish those joins
independently. JNI round trips, runtime performance, rendering, deterministic
video and feature parity remain unverified. No builds, tests, syntax checks or
Bedrock/process operations were performed. Source evidence and the complete
wire order are in inspection/native-avatar-geometry-packet-source.json.

### D3D12 avatar buffer upload (static source port)

avatar_gpu_upload.begin now contains the resource-upload implementation for a
decoded geometry frame and a live matching direct device/queue. It creates owned
default-heap vertex and R32_UINT index buffers, per-draw buffer views, RGBA8
textures and a shader-visible SRV heap. Descriptor zero holds an uploaded white
texel; packet texture N uses N+1. Little-endian ARGB packet pixels are explicitly
converted from B,G,R,A bytes to R,G,B,A, retaining alpha without premultiplication
or an invented color-space conversion.

One staging allocation holds the vertex/index bytes and texture footprints.
Device-reported footprints are checked for dimensions, format, rows, active row
bytes, pitch and extent before independently aligned placement. The allocation
is capped at 128 MiB and initialized before writes. Input lengths and whole-frame
vertex/index/pixel budgets are rechecked. The queue must belong to the supplied
device, use direct command lists and select a valid single node. Upload commands
transition only Arctic-created resources into vertex/index/shader-resource states;
no native Mesh, back buffer, world depth, material or player state is modified.

All owners enter a Ticket before queue execution. take_completed polls without
blocking and returns Uploaded only after its private fence reaches value 1.
UINT64_MAX is never treated as success. Preparation failures release ordinary
owners before execution. A failed Signal or abandoned pending ticket retains the
staging/default resources, allocator, command list, queue, device and fence in
one bounded retirement slot. The global single-upload permit prevents retries
from accumulating more unresolved work. A later begin call rechecks completion
or reported device removal before reclamation. No timeout or helper thread can
discard resources still in use. Uploaded keeps the draw resources and descriptors;
the future rendering owner must retain it through its separate draw fence.

This upload API is not yet invoked by presentation. Native producer/camera/depth
joins, shaders/material states, transparent/glow sorting, actual draw submission
and its retirement remain unfinished. It does not grant suppression authority or
replace the existing software layer. The DX11 path also remains to be ported.
No builds, tests, syntax checks, GPU calls, game/project execution or Bedrock
operations were performed. Actual uploads, world rendering, deterministic video
and full feature parity remain unverified. Source evidence is recorded in
inspection/native-avatar-gpu-upload-source.json.

### Explicit-input native entity materials (static source port)

avatar_material and shaders/avatar_entity.hlsl now contain the D3D12 entity
material implementation for cosmetic base/glow, solid capes and cutout wings.
Cached Java 26.2 entity/light/fog/lightmap shader resources and pipeline class
bytes supply the reference behavior. The HLSL accepts the staged 40-byte vertices,
unpacks ARGB and signed-short packed light, retains Java normal quantization,
computes front/back cardinal light and samples an explicit 16x16 lightmap before
interpolation. Artwork uses nearest-repeat sampling. Cutout compares sampled
alpha before tint/fade, overlay applies before lightmap, and environmental and
render-distance fog combine by their maximum while retaining fragment alpha.
Emissive layers bypass lightmap but keep their per-face cardinal lighting.

Inputs have no defaults or live clock. Color modulation, light directions,
fog-position transform/ranges/color, selected NO_OVERLAY texel and owned lightmap
must be supplied explicitly for the draw. Bedrock named constants are not
automatically reinterpreted as Java equivalents. The live producer is unfinished.
General skin roles are rejected pending exact material-variant mapping. Glint,
HDR and sRGB target fidelity also remain unported in this GPU path.

The material constructor contains shader-model-5 compilation, root-signature and
four graphics-pipeline creation paths. These functions were not invoked. Material
states retain translucent RGB/alpha factors, base depth writes, glow depth-test
without writes, cape culling and unblended wing cutout. Depth comparison follows
the caller's forward/reverse convention. Target winding is explicit. The owned
constant buffer uses 256 bytes per draw and 4096 bytes for a normalized float4
lightmap; at most 2048 draw slots are allocated. Serialization uses explicit
little-endian words. Root-signature references in pipeline descriptions have
cleanup on both success and failure.

Indexed draw recording binds owned buffers/descriptors and typed constants.
Before recording, the constants' material role, native tint and projection must
match the packet bit-for-bit. Wing draws require their layered camera. Reverse-Z
conversion changes the complete clip Z row to W-Z; floating-point equivalence to
the installed engine's original pipeline remains unverified. The recording API
requires an owning native world target/depth/state/camera scope and caller-held
resources through its draw fence. It does not establish that scope or submit a
queue. Transparent/glow sorting and draw-submission retirement remain unfinished.

Presentation still uses the existing software layer. No build, test, syntax
check, shader compilation, GPU recording/execution, game launch or process
operation occurred. No rendering, deterministic video or parity claim is made.
Evidence and pinned asset hashes are recorded in
inspection/native-avatar-material-source.json.

### Java feature grouping and quad ordering (static source port)

avatar_draw_order now ports cached Java MeshData/VertexSorting and
RenderTypeFeatureRenderer.Group behavior. Quad centers use float vertices 0/2
and pairwise sum/divide-by-two. Perspective keys use the default JOML grouping
dx*dx+(dy*dy+dz*dz); orthographic keys use literal negative centroid Z. Stable
descending sorting preserves emitted order on equal keys. Its comparator follows
Java float rules for signed zero and canonical NaNs. Alternate JOML fast-math/FMA
modes remain outside this port.

The planner takes explicit ordered feature groups, can-reorder flags, separate
RenderType object/prepared-equality IDs, consolidation flags and sorting policy.
It reuses a consecutive same-object draw only when consolidation is enabled;
broader first-prepared-equal reuse needs a reorderable group and consolidating
current submit. Draw creation order survives. Every batch must appear exactly
once, with coherent identity descriptors. No resource name/texture resemblance
is promoted into RenderType identity, and no global sort across unrelated groups
is invented. Native phase/group metadata production remains unfinished.

Empty batches now survive Java staging, the packet and native decoding because
even empty submits can split sorted draws. Mixed-frame uploads retain their
metadata but never bind zero-length views. An entirely empty upload returns
without GPU commands. Cached MeshData.SortState.writeIndices also establishes
the exact Java triangle sequence [0,1,2,2,3,0]; staging and decoding now use it
instead of rotating the second triangle. This is a correction to the unexecuted
ARCGEO01 source contract, not a completed archive migration or compatibility test.

Plans borrow their source Frame and contain coalesced original index ranges.
Uploaded exposes its frame/views immutably. The material recorder requires the
plan's exact uploaded frame, then preflights all material/camera/tint/descriptor,
constant-slot and index-range checks before recording ordered draws. At most
2048 groups/submits and 100000 quads are accepted. Sorting stays within the
supplied feature/render draws; adjacent source ranges merge without changing
the computed order. This can still require one GPU draw per quad; equivalent
optimized batching and actual performance remain unverified.

Perspective origin may come from the declared camera/world coordinate space.
Orthographic Z must already be in the actual sorter space; world Z is not assumed
equivalent. Whole-world ordering relative to Bedrock translucency, live phase
metadata, lighting/fog/overlay and scene packed-light production, native target
joins and draw-fence/presentation integration remain incomplete. The current
shared emitter still inherits its existing FULL_BRIGHT values where a faithful
scene packed-light producer has not been ported.

No builds, tests, syntax checks, project/game execution or GPU calls occurred.
Cross-language sorting, actual ordered drawing, deterministic video and full
parity remain unverified. Evidence is recorded in
inspection/native-avatar-draw-order-source.json.

### Owned native target submission and shared retirement (static source port)

avatar_draw_submit now contains recording/submission for an explicitly owned
native color/depth pair. Targets.from_owned is unsafe: its caller must supply
the actual current world owner, per-plane states and a retained thread-safe token
that prevents native writes/reuse until retirement. The constructor does not
discover or prove that ownership from scope numbers or COM references. The
installed-game producer for this lease is still unfinished.

Submission checks the geometry session/sequence and dimensions, nonzero world
scope, distinct targets, matching device/node/direct queue, resource/view format
compatibility, samples, RTV/DSV flags and explicit stencil state. Color and depth
are single-mip/single-array surfaces. The CPU order is built once. Private RTV/DSV
heaps and an owned direct allocator/list bind the packet viewport/scissor and
record the ordered geometry. Color plane zero transitions to render-target;
depth plane zero to depth-write. Stencil formats use a read-only-stencil view and
a separate plane-one depth-read transition. Changed states restore in reverse
order. Stencil contents are not modified. Preparation errors discard unsubmitted
commands without having changed target contents on the GPU.

gpu_flight now provides retirement for uploads and draws. One upload slot and
three draw slots bound pending work. Device/list/allocator/queue checks precede
execution; all referenced owners enter the ticket before ExecuteCommandLists.
A private fence is signaled once. Value one retires it; UINT64_MAX never does.
Signal failure or a dropped unfinished ticket retains every owner in its bounded
slot until actual fence completion or reported device removal. No sleep, helper
thread or timeout drops in-flight resources. Payload/native-owner destructors
run outside slot locks. DX12 before-frame cleanup collects retired abandoned
payloads even when no new upload/draw is requested.

The draw payload holds the native owner, targets, mesh/textures/descriptors,
constants, cached pipelines, target descriptor heaps, allocator/list and queue
through its separate draw fence. Its retired receipt identifies only the
caller-supplied scope and completed private submission; it does not independently
prove native ownership, shader behavior or pixels. Upload now reuses the same
retirement implementation rather than maintaining separate fence logic.

Arctic private recording/submission uses depth_tracker.private_commands so its
own descriptors/lists/barriers/draws cannot manufacture native-world evidence.
The TLS marker now restores the previous marker, preserving nested readback and
private-command scopes. Existing readback callsites use that guard as well.

Only retirement cleanup is connected to presentation. Native drawing is not
automatically requested. The installed-game lease/state producer, authoritative
feature groups, Java-equivalent shader/scene-light inputs, native readiness and
suppression handover, general skin/glint/HDR/sRGB/DX11 paths and live integration
remain unfinished. No builds, tests, syntax checks, project/game execution,
shader compilation, GPU calls or Bedrock/process operations occurred. Actual
submission, retirement, rendering, deterministic video and full parity remain
unverified. Evidence is recorded in
inspection/native-avatar-draw-submit-source.json.

### Native framebuffer context correlation (static source port)

Cached instructions identify a framebuffer-binding routine at
dfa9ef0..dfaa6de. Its three direct callers are dfa1cb4, dfab321 and dfb6922.
The main backend caller supplies a command context from stack+80 and the saved
backend renderer argument from stack+d8. The routine uses context+30 as the real
ID3D12GraphicsCommandList, matching the existing native queue-context layout.
Both observed binding paths dispatch vtable slot +170 (OMSetRenderTargets).
RTV/DSV argument pointers come from context+10/+18; the default path points to
inline handle values at +0/+8. Nondefault framebuffer entries use
renderer+c97690+handle*68, with the RTV count at entry+32. The epilogue stores the
requested handle at context+20 and flag byte at +2c. No SDK symbol name or
ownership semantics are assigned from those offsets.

native_framebuffer now observes that pinned routine and its actual public
binding callback. Native batch scopes retain the backend renderer argument so
the callback must join the same renderer/frame with an unchanged frame stamp.
Four observed register arguments forward unchanged, including their DWORD bits;
only handle low word/flag low byte are interpreted. Entry/callsite/epilogue byte
guards accompany the already required full executable fingerprint. Eight nested
requests and eight RTVs are bounded; missing/unsupported contexts mask parents.

The observer checks the actual native list and RTV/DSV argument pointers, native
framebuffer count and descriptor handles. Contiguous RTV binding reads only one
base handle and derives the range using that list device's actual descriptor
stride. It never treats the pointer as a count-sized array. After the original
routine returns, header/list/descriptor values, registry count, requested
handle/flag and frame stamp must remain consistent. Multiple or rejected binds
invalidate the request. Calls with no bind cannot borrow an earlier witness.
Native body/probe mode gates this observation, and Arctic private commands are
excluded. Status exposes scalar nativeFramebuffer samples without addresses.

The existing RTV-copy observer also now propagates known identities through
CopyDescriptors and CopyDescriptorsSimple. It uses bounded ranges and the actual
device stride, snapshots sources before updating destinations, and retains no
GPU resources. Known valid copies preserve native target mappings. Unknown,
malformed or over-budget ranges keep the conservative mapping/witness
invalidation. This removes unconditional loss of all known RTV identities on
every ordinary descriptor copy, and remains an issued-draw association rather
than proof of final scene contents or resource ownership.

This is an installed-path source hook and correlation port, not a live hook
result or drawing lease. Color/depth resource ownership and state resolution,
exclusive native synchronization or an exact state-restored in-list insertion
point, world/input/group joins and native drawing still remain unfinished.
No builds, tests, syntax checks, project/game execution, shader compilation,
GPU calls or process/Bedrock operations occurred. Runtime binding, native world
rendering, deterministic video and full parity remain unverified. Evidence is
recorded in inspection/native-framebuffer-context-source.json.

### Native framebuffer texture/resource joins (static source port)

Cached native code establishes the texture table at renderer+bde170, b8-byte
records and 4096 inline entries. Its allocated byte extent is at table+b8008.
Extension lookup follows table+b8000 to a root whose first word identifies page
one, then page+b8000 links. The transition helper dfb5890 loads record+0 into a
public ResourceBarrier pResource field and emits an all-subresource transition.
Framebuffer color words begin at entry+0, the depth word is at +10 and count at
+32. Command context+14458 points to the same registry. These instruction sites
now accompany the framebuffer hook's pinned guards.

native_framebuffer_resources copies those fields lazily during an actual
whole-resource, unsplit transition in the observed native routine. It follows
existing allocations only, checks extent divisibility/capacity, pointer bounds,
extension mode and cycles, and keeps at most nine logical attachment handles.
Duplicate handle roles merge. Each texture sample uses the existing bounded
32-guard/2048-byte copy mechanism; no native allocator or growth helper runs.

A copied slot pointer is compared numerically with the real public resource
argument. Only that actual callback object is queried/cloned; a guessed registry
pointer is never invoked as a COM object. References end with the original
framebuffer invocation unless a complete pair transfers to the subsequent bounded
native binding shadow. The sample rechecks copied framebuffer/registry/page/
resource-pointer fields and actual resource descriptions. Status reports handle
roles, descriptions and observed transition values without addresses. Logical
texture-slot joins can include aliases and repeated transition observations;
they are not counts of unique GPU objects or completed barriers.

The first-use helper dfc2e90 is a separate case: it appends a handle, sets validity
and stores the requested state twice in its cache. The inspected body emits no
immediate public barrier. Those cached words are therefore not treated as actual
queued resource state. Default swapchain targets and attachments without a
matching transition remain unresolved, and partial/split or post-bind transition
cohorts cannot supply an assumed whole-surface state.

This advances read-only resource correlation. Exact view/subresource mapping,
queued-state ownership, exclusive native synchronization or state-restored
in-list insertion, world/shader/group joins and native drawing remain unfinished.
Temporary callback COM references are not a rendering lease. No builds, tests,
syntax checks, project/game execution, shader compilation, GPU calls or process/
Bedrock operations occurred. Resource joins, rendering, deterministic video and
full parity remain unverified. Evidence is recorded in
inspection/native-framebuffer-resources-source.json.

### Native surface descriptor arguments (static source port)

native_surface_views now copies color/depth descriptor arguments from the actual
public CreateRenderTargetView/CreateDepthStencilView calls. It handles bounded
single-array Texture2D and Texture2DMS views, preserving explicit formats, mips,
RTV planes and DSV flags. Null descriptors admit a restricted typed default set;
unsupported or null resources invalidate the old handle rather than inheriting
its metadata. This records supplied arguments, not driver validation: these
creation APIs return void.

Actual public device/resource objects receive private scalar identity tokens.
The table retains no GPU objects or COM addresses. Device, descriptor kind and
handle select entries, and a real scoped transition resource must match the
resource token and immutable description. Recycled object addresses cannot
inherit an earlier object's token. The table is bounded to 4096 entries and each
copy to 256 descriptors.

Creation and descriptor copies hide destination metadata before the real API
call. Publication occurs afterward only if the destination revision and copied
source revisions survived. Source metadata is snapshotted before destination
invalidation; unknown sources, ambiguous overlaps, duplicate destinations,
capacity loss and counter exhaustion remain unknown. Concurrent rewrites cannot
publish an old known mapping through a superseded ticket.

The native framebuffer observer joins actual bound color/depth handles with its
transition-correlated texture slots and copies up to nine role snapshots.
Per-role output includes view format, mip, plane, samples, dimensions and DSV
flags. Aggregate argument matching requires all expected roles, the exact bound
handle count and a final descriptor cohort recheck under one metadata lock.
Malformed/aliasing legacy barriers and enhanced barriers now invalidate that
framebuffer resource cohort as well; private Arctic commands remain excluded.

This implements the descriptor-argument part of surface correlation. Unseen
creation, unsupported views, no-transition/deferred resources and the default
swapchain path remain unknown. Exact physical view validation, queued states,
native ownership or state-restored insertion, actual native world drawing and
full parity remain unfinished. No builds, tests, syntax checks, project/game
execution or Bedrock operations were performed. Evidence is recorded in
inspection/native-surface-views-source.json.

### Assets recorded into native lists (static source port)

native_draw_assets adds a separate bounded owner pool for future cosmetics
commands inserted into engine-owned lists. The existing gpu_flight pool owns
private Arctic lists; it cannot on its own prove when a closed native list stops
being executable. The new unsafe retain_recording entry point transfers assets
before recording, requires the installed public observers and returns the
assets on preparation failure. The subsequent native_avatar_record source
transaction invokes it; a game-facing cosmetics insertion caller is still absent.

The source retains up to 16 opaque payloads, their devices/fences and the first
submitting queue, without retaining the native list itself. Execute/Reset joins
use canonical IUnknown identity, while the actual graphics-list argument remains
a numeric alias for the native context join. Destruction candidates are captured
atomically before COM Release; a zero return records those exact generations
without reading a destroyed object. Unknown interface aliases can retain owners
conservatively rather than release them early.

The existing submission gate encloses pre-execute reservation, the real Execute
and private fence signals. Each owner has a monotonically advancing submission
ordinal. Reset removes replayability but does not imply GPU completion. Native
used-to-free retirement now carries separate asset tickets through the existing
stable context/list/fence/ring checks. The ticket must still match its generation,
canonical list, actual queue and submission ordinal. A completed private fence
alone leaves a closed native list's owners intact.

The before-frame callback collects only after both non-replayability and private
GPU completion, or actual device removal. It removes payloads under the pool
lock and destroys them afterward, outside submission/tracker/pool locks. Signal
failure, cross-queue execution, poisoned submission gates and uncertain identity
pin the affected owners; timeouts and slot pressure never evict them. Separate
asset retirement counters are included in nativeQueue diagnostics.

Cached code was reread for the submitted byte/fence checks at dfa1668/dfa167d,
used-ring removal at dfa16b3..dfa16e2, free-ring insertion at dfa1913..dfa1996
and the removed-fence sentinel at dfa19ad. This source change does not establish
live ownership behavior. Native insertion/state restoration, world/target/shader
inputs, actual cosmetics drawing, deterministic video and full parity remain
unfinished. No builds, tests, syntax checks, project/game execution or Bedrock
operations were performed. Evidence is recorded in
inspection/native-draw-assets-source.json.

### Cosmetics graphics-state restoration (static source port)

native_graphics_state and native_graphics_observer now capture the state changed
by the current avatar_material.record_ordered implementation: pipeline, graphics
root signature and all declared arguments, shader-visible heaps, topology,
vertex slot zero and index binding. Changing heaps also affects compute table
bindings, so the source captures/restores those tables without rebinding or
changing the compute signature and its other arguments. These rules follow
[Using a Root Signature](https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-a-root-signature)
and [Setting Descriptor Heaps](https://learn.microsoft.com/en-us/windows/win32/desktop/direct3d12/setting-descriptor-heaps).

The actual CreateRootSignature callback uses the public versioned deserializer
to attach a scalar parameter/range layout to the returned object. It bounds
serialized data to 64 KiB, root parameters/cost to 64 DWORDs, copied descriptor
ranges to 256 and private metadata to 3856 bytes. ARCRS002 retains range
type/count/offset and root flags; legacy ARCRS001 still supplies its original
argument layout without fabricated range/flag facts. Graphics CBV/SRV/UAV types
stay distinct. Individual/offset constant
writes preserve exact 32-bit values and initialization of every declared word.
Unknown signatures or missing arguments never acquire guessed defaults.

Restore snapshots also retain a bounded descriptor slot plan for both graphics
and compute tables. It resolves the declared SRV/UAV/CBV or sampler ranges
against actual bound shader-visible heaps on the list's actual device, using
their real handle starts, increments and sizes. Explicit offsets, APPEND and
unbounded ranges preserve their table origin; an unbounded range must still fit
the actual remaining heap and shared 256-slot budget. Legacy/unknown range or
flag facts, direct heap indexing, mixed heap classes, oversized coverage and
invalid/overflowing handles leave the whole optional plan unknown.

Each span holds its actual heap through an Arc and records the slot interval and
public CPU/GPU handles internally. Snapshot clones retain these objects through
the existing draw-asset path. No descriptor contents are read, frozen or owned by
this slot plan. Version-1.0 converted range metadata supplies neither shader-use
reflection nor a volatility/static-data contract. Resource/sampler descriptor
creation/copy tracking now supplies the referenced-object cohort described below;
complete allocation and frozen-content ownership remain separate. Restoration
continues to use its original bindings. No readiness promotion or new hooks were
introduced. See inspection/native-descriptor-spans-source.json; no builds, tests,
API/GPU execution or Bedrock operations ran.

The existing device observer now captures public descriptor heap creation and
CBV/SRV/UAV/sampler writes, including supported extended sampler and feedback
APIs. Resource/sampler descriptor copies feed the same cache. Every driver call
is forwarded once with its original arguments/result. Metadata queries and
bookkeeping use the private guard outside tracker/submission locks. Provenance
activates only after all observers enable, for the retained actual device.

Heap IDs and slot revisions are monotonic. Destinations stay unknown while
pending; overlapping writes cannot choose a winner from call/return order and
remain unknown until a later uncontested write. Unknown query results get a
guard through the original call. Copy sources must remain unchanged, cannot be
shader visible and cannot overlap destinations. Resets/overflow suspend knowledge
until active calls drain. Disabled external writes clear discovery immediately,
so toggling off/on between frames cannot revive old descriptor values.

Restore snapshots can now retain a complete known slot/value cohort from their
descriptor plan, including actual referenced COM objects and available committed
allocation owners. Pre-emission snapshot matching checks its known revisions.
Retained cohorts survive discovery eviction through the existing draw-asset
lifetime path. Normal restoration still restores its original bindings; metadata
changes after emission must not strand private Arctic state on the engine list.
This does not freeze slot contents, lease placed allocations or provide the
exclusive native phase contract. Missing/unknown cohorts preserve the existing
unsafe caller ownership requirements rather than authorizing insertion.

Discovery is bounded to 32 heaps/128 MiB of descriptor handle ranges, 4096 slots,
256 descriptors per operation/capture and 512 active calls before suspension.
Canonical referenced resource identities deduplicate a separate 128-object/
256-MiB queried allocation budget. These are discovery/query bounds, excluding
driver overhead and separately retained transaction/frame pools. Removed owning
payloads are released outside the descriptor mutex. No descriptor bytes are read
or rewritten, no tests were added/run, and no build, API/GPU call or Bedrock
operation ran. See inspection/native-descriptor-resources-source.json.

Observed creation/reset starts an epoch and attaches a fresh private list token.
Snapshots validate that token on the live list; a later object at the same COM
address cannot borrow an old shadow. Global stamps reject changed binding
cohorts without wrapping. Close clears the shadow. Snapshot copies own the PSO,
root objects and heaps they restore; a future insertion caller must also pin
the snapshot and native resources referenced by copied GPU addresses through
native retirement. Capture is inactive until an observed enabled creation/reset,
and skipped setters after disabling invalidate that recording epoch.

Extra hooks capture graphics tables/constants/SRV/UAV, compute signature/tables,
heaps, topology and vertex/index setters; existing pipeline/root/CBV hooks feed
the same state. Bundles, indirect calls (including zero maximum counts) and
optional ray-tracing state-object setters reject readiness until reset. The
indirect handling follows [Indirect Drawing](https://learn.microsoft.com/en-us/windows/win32/direct3d12/indirect-drawing).
Snapshot restoration itself runs inside the private-command guard and refuses
a mismatched list lifetime before writing any state.

This is the restore set for the existing cosmetics recorder, not a general
graphics state block. Targets, viewport/scissor, compute non-table arguments and
other dynamic state must remain unchanged. No native insertion caller invokes
capture/retention/restoration around live cosmetics yet. World/target/material
ownership and native resource lifetime still require joins, and runtime hook
coverage/performance and restoration fidelity remain unverified. Native drawing,
deterministic video and full parity remain unfinished. No builds, tests, syntax
checks, project/game execution or Bedrock operations were performed. Evidence is
recorded in inspection/native-graphics-state-source.json.

### Native avatar recording transaction (static source port)

native_avatar_record now connects the ordered material recorder to native-list
asset retention and graphics-state restoration. Its source API receives owned
uploaded geometry/constants/pipelines, explicit feature groups and an unsafe
native cohort owner. It neither discovers that native ownership nor submits a
private command list. A game-facing insertion caller and authoritative live
material/target/group producer are still missing.

Actual root CBV arguments 2/3 must resolve through the enclosing native batch's
packet map. The exact retained world's unique producer FrameKey must match the
current thread's guarded backend frame. Session/sequence and camera-origin/
projection bits must match the uploaded packet, together with camera-relative
space, dimensions and the complete pipeline target convention. The producer and
consumer now share packet_projection/native_session helpers without changing
their original float operation order/session format. Batch binding resolution
has a pure entry point so preparation does not inflate observed draw counters.

The direct list/pipeline and every uploaded vertex/index/texture/constant resource
must share the device. Heap properties must expose the drawing node; the uploaded
shader-visible SRV heap must match device/node/type. The unsafe native owner must
still establish bound compatible views/states, viewport/scissor, feature phase
and lifetime of native resources/descriptors referenced by restored bindings.

All fallible material/range preparation completes before recording. The new
prepare_ordered result borrows immutable resources and owns its ranges; the
existing public record_ordered delegates to that same path. The transaction
captures native state, registers geometry/snapshot/native-owner clones in the
bounded native_draw_assets pool, rechecks native world/state identity, then
records inside the private-command guard. A guard restores native state on
normal completion or Rust unwinding. Restore heaps/root constants and argument
variants are resolved during capture, avoiding restore-time buffer construction
and missing-value expectations. Post-registration early errors remain pinned
until native reset/retirement. Recorded receipts are not execution, presentation,
body suppression or pixel-fidelity evidence.

Static comparison also found and corrected a staging transport mismatch:
WorldFrame.parse requires x/y/z/yaw/pitch/FOV, but NativeGeometryPacket required
a three-value camera. It now accepts the real six-value snapshot and writes
only the first three position doubles into ARCGEO01's existing 24-byte camera
field, matching the unchanged Rust decoder. This correction was not executed.

The recording transaction is source infrastructure, not live native insertion.
Native world rendering, authoritative live/replay inputs, camera/video fidelity
and full feature parity remain unfinished. No builds, tests, syntax checks,
project/game execution or Bedrock operations were performed. Evidence is recorded
in inspection/native-avatar-record-source.json.

### Bound surface handoff to native draws (static source port)

native_bound_surfaces now carries a complete framebuffer attachment sample from
the pinned native routine into the recording list's current binding shadow.
Every observed OM bind has a fresh epoch. After forwarding the original native
routine and rechecking its header/frame/arguments, handoff requires that exact
epoch and the graphics observer's live list token. The actual public list argument
is cloned only through that native call; the shadow holds no owning list reference.

The current single-RTV material path requires one distinct color/depth pair,
complete descriptor/role snapshots, recorded whole-resource transitions ending
in render-target/depth-write states, matching Texture2D dimensions/samples,
single array/mip and writable depth view flags. Canonical resource identity is
copied only from actual public COM arguments. Registry/slot guards, immutable
resource descriptions and descriptor revisions are rechecked when consumed.

The shadow retains at most 16 binding pairs and charges at most 256 MiB of queried
resource allocation sizes, using actual resource device/heap creation masks.
Aliases across bindings are charged conservatively. The native draw-asset pool's
transaction pins have their separate slot bound; this shadow budget does not
claim to bound all physical engine or GPU memory.

Only after forwarding a nonempty actual native draw is the binding associated
with its exact root-CBV/batch/world scope. Private Arctic commands cannot arm that
association. Related or unknown clears disarm the scope; a later native draw can
associate it again. New binding, relevant/unknown barriers, aliasing/enhanced
barriers, render-pass changes, opaque bundle/indirect calls, reset/close/eviction
discard the affected binding. Descriptor mutations fail revision checks. An
unchanged binding can persist across batches on one open list while the current
native draw/batch join independently supplies its world scope.

Actual viewport/scissor setters now supply the full-surface raster checks. The
recording transaction verifies view formats, dimensions/samples and device/node
against that tracked pair, pins its Bound Arc before recording, then rechecks the
same binding after preparation. depth_tracker.record_avatar exposes the actual
graphics/surface shadows under their tracker lock for the future feature-phase
caller. The recorder continues to leave targets and raster bounds unchanged.

This is binding/resource correlation, not a native ownership lease or live draw
result. Native allocation/heap and descriptor-content ownership, exact feature
phase/order, authoritative material/group inputs and a game-facing insertion
caller remain unfinished. MRT, read-only/deferred/default-swapchain paths need
their own material/state contracts. Native world drawing, deterministic replay/
video fidelity and full parity remain unverified. No builds, tests, syntax checks,
project/game execution or Bedrock operations were performed. Evidence is recorded
in inspection/native-bound-surfaces-source.json.

### Native geometry input without depth readback (static source port)

world_hooks.geometry_input now copies one exact retained world observation into
owned geometry input without requiring a CPU depth image. It requires the
scope's current session/sequence and unique guarded producer FrameKey, carries
at most 1 MiB JSON, and preserves the requested native surface and depth
convention. live_frame.build still requires and downsamples its actual depth;
it now shares the same JSON builder for actor/appearance/timing/receipt fields.
No fabricated depth image is used to stage native geometry.

WorldFrame.parseGeometry shares the existing full snapshot validator, skipping
only terrain-image validation/allocation. An internal transient flag prevents
that object from entering WorldState as renderable, producing a software image
or serving a diagnostic depth snapshot. NativeBridge.worldGeometryFromFrame
checks expected session/sequence and emits through the existing avatar/packet
path, including acknowledged pose/appearance and rational replay time choices.

On the serialized Java client thread, a scoped geometry request temporarily
supplies its world to appearance/replay helpers and restores the previous world
and input-availability flag in finally. It does not publish WORLD/renderability,
body plans or capture receipts. Nested world/presentation mutation and body/
attachment-plan entry points reject reentrancy during that scope. This mutable
appearance path must not run directly on native draw workers.

ClientVm.geometry_input takes a mutable client borrow, checks exact native input
freshness before and after Java work, bounds the returned array before copying,
and decodes the unchanged ARCGEO01 packet. Session/sequence, dimensions and
camera-relative space must match the input, together with exact origin/projection
bits. The copied input keeps no native object reference or borrowed depth buffer;
its producer FrameKey is only a guarded numeric identity.

The helpers remain unscheduled until an authoritative GPU input/phase producer
can consume their results. Native upload scheduling/retirement joins, live
material/lightmap/fog/group inputs, general/glint/HDR shader variants and a
game-facing insertion caller remain unfinished. Native rendering, complete
cape/emote fidelity, deterministic replay/video comparisons and full parity are
unverified. No builds, tests, syntax checks, JNI/project/game execution or Bedrock
operations were performed. Evidence is recorded in
inspection/native-geometry-input-source.json.

### Packed scene light (static source port)

The shared avatar emitter now takes an optional packed Java block/sky light
sample for the actor/frame. Base cuboid and mesh cosmetics, skin pieces and
bent emote limbs, cape boxes and both wing paths pass it to their vertices.
Cuboid glow and the shared mesh glow/sheen paths retain Java's full-bright
semantics. Existing standalone previews retain their full-bright fallback.

Native staging preserves whether the sample exists, separately from its value.
ARCGEO01 batch flag 4 conveys that provenance without adding bytes; old readers
reject this previously unsupported flag. Updated readers accept older unflagged
staging, but native material preparation rejects nonempty lit batches without
scene provenance before recording GPU commands. Glow bypasses the lightmap.
Changing actors clears the source; a partial quad cannot change light context.
Known lit vertices must match the batch's copied source value.

Replay emission clears the viewer's live sample because it does not describe
the archived actor/time. Recorded lighting remains unfinished, as does the
authoritative live packed-light/lightmap producer. Bedrock TILE_LIGHT_COLOR is
not interpreted as Java packed light. General skin variants and live draw-phase
wiring also remain unfinished. No build, test, syntax check, project/game
execution, Bedrock launch or process inspection was performed. This change is
source work, not evidence of full parity; see
inspection/native-packed-scene-light-source.json.

### Replay body packed light v4 (static source port)

New body tracks use bedrock_body_samples_v4 and magic 0x415243424f440004.
Each actor retains the existing pose/root/color layout and appends a strict
presence byte plus an optional packed-light int32. The writer copies the
same-frame actor input, includes the five-byte maximum in its block budget,
and includes light availability/value in duplicate-clock consistency checks.
The mount keeps separate v1/v2/v3 kinds/magics and rejects conflicting tracks.
Older records retain unknown light, with no full-bright upgrade.

BodySample copies and exact endpoint samples preserve packed light. Interior
samples retain it only when both endpoints contain an equal value. Differing
or missing endpoints cannot establish a light transition, so they yield unknown
light rather than interpolating a packed integer. Existing discontinuity and
actor-absence checks remain in force. WorldFrame reads the selected archived
actor/time sample and never substitutes the viewer's live light.

Appearance capture now requires a light sample for actors needing recorded
body references. New body plans for a v4 track also reject missing light.
These checks do not prove lighting fidelity: the live packed-light producer,
recorded lightmap/material timelines, software lightmap sampling and native
draw-phase wiring remain unfinished. Track availability reports format support,
not that every frame contains light. No builds, tests, syntax checks, archive
round trips, project/game execution or Bedrock operations were performed.
See inspection/replay-packed-scene-light-source.json.

### Software entity material stages (static source port)

JavaEntityShader ports the cached Java 26.2 entity shader's explicit lightmap,
overlay, ColorModulator and fog stages into Raster3D's existing cosmetic base,
glow, solid cape and cutout wing paths. WorldFrame accepts an optional schema-1
javaEntityShaderInputs cohort and copies its validated arrays before emission.
The profile requires a declared position space; its fogPosition is a row-major
affine transform from emitted positions into Java fog coordinates. No dimension,
Bedrock named constant or packed-light value supplies implicit material inputs.

Lit vertices decode packed UV2 as two signed shorts, clamp the normalized
half-texel coordinates and bilinearly sample the explicit 16x16 RGBA lightmap.
Front/back cardinal lighting uses copied directions without normalization.
Clipping and perspective interpolation preserve lightmap and fog varyings.
The fragment path keeps texture-alpha cutout before tint/modulator, applies
overlay before lightmap, bypasses lightmap for glow, then applies environmental
and distance fog to RGB while preserving alpha. Quantization follows those
stages. Base depth writes survive a zero alpha produced after cutout.
Explicit lit shading requires corresponding packed scene light. Constructors
without a profile retain the existing preview behavior.

This profile is not captured from the installed game yet, is not transported to
the native GPU constants, and is deliberately not borrowed from the viewer
during replay appearance. Recorded lightmap/material playback and fog-space
rebasing remain unfinished. General skin variants, glint's separate path,
software blend/composition and raster precision also remain distinct fidelity
work. No build, test, syntax check, shader execution, project/game execution or
Bedrock operation was performed. Cached shader resources were read as ZIP data;
no Java class was loaded. See inspection/software-entity-shader-source.json.

### Recorded entity material/lightmap track (static source port)

EntityMaterialHistory now records bedrock_entity_material_v1 through the same
appearance-history start/sample/finish lifecycle and serialized compressed asset
writer. Each optional observation owns its immutable JavaEntityShader, dimension,
coordinate-space flag, six camera fields and projection matrix. Material values
are 1058 f32 fields: 1024 lightmap channels, modulator, directions, fog-position
transform, selected overlay texel, fog color and fog ranges. Missing material
remains a recorded gap. At most 64 frames occupy a bounded 512 KiB inflated
block; the reader has a two-block cache and validates the source manifest again
when loading a block. Mount reserves 8 MiB and supports at most 4096 chunks.

ReplayAppearance uses exact fractional replay time and returns only an available
recorded material. Exact observations preserve their values. Interior sampling
requires equal material values at contiguous observations, compatible context,
no missed tick and no long pause. Changed or missing samples yield no material;
they do not interpolate packed/cohort identities or borrow the live viewer.
WorldFrame.render now consumes the selected archived profile, and appearance
capture readiness requires one for actors needing recorded body references.
Older archives still mount without this optional track, with material unknown.

For camera-relative world positions and an exactly identity fog transform,
Java entity.vsh derives fog distances from Position before ModelViewMat.
Re-emitting the recorded actor relative to the selected replay camera therefore
supplies the edited fog position directly. Such profiles can follow camera edits.
Other affine fog profiles require their bit-identical recorded camera/projection;
their general camera rebasing is not implemented. Recorded dimensions and
coordinate spaces must match in either case.

This source path does not establish an installed-game material producer,
changing-material reconstruction, per-role/actor material cohorts, GPU constant
transport, general skin/glint variants or live draw-phase insertion. Archive
round trips, camera behavior, rendered pixels and deterministic video remain
unverified. No builds, tests, syntax checks, archive/project/game execution,
Bedrock launch, attachment or process inspection were performed. See
inspection/replay-entity-material-source.json.

### Material-bearing native geometry and constant assembly (static source port)

WorldFrame.geometry now selects the same immutable live or recorded entity
material profile as software rendering. When present, NativeGeometryPacket
encodes ARCGEO02: after the optional equipment matrix it writes a strict material
presence byte and 1058 little-endian floats, in the same scalar order as the
archive profile. The existing 96 MiB bound includes this 4233-byte extension.
Material-free staging keeps ARCGEO01 and its original byte layout. The Rust
decoder accepts both versions; old packets retain unknown material rather than
acquiring a white lightmap or inferred fog. Material input needs a declared
coordinate space, finite fields, normalized lightmap/overlay and ordered fog
ranges. All copied arrays remain owned by the decoded frame.

Inputs.for_frame and Constants.from_frame derive each nonempty batch's GPU
constant bytes from that copied cohort and its matching body/equipment camera.
Supported material, scene-light provenance, tint, projection and shader values
are checked before the new upload resource is allocated. Empty submits retain
their place in the source-to-uniform mapping without acquiring a CBV. For
material-bearing packets, later draw preparation compares all 256 draw bytes
and all 4096 lightmap bytes, in addition to the existing kind/projection/tint
identity checks. A constant buffer from a different shader/lightmap cohort is
rejected before recording any commands.

Geometry.from_uploaded assembles a completed owned upload, packet-derived
constants and caller-supplied authoritative feature groups, then preflights the
complete mapping. Assembly records/submits no commands and acquires no native
scene lease. The actual owning group/object/equality identities, scene resource
and descriptor lifetime owner, live feature phase and insertion caller remain
necessary. This does not activate rendering or establish a live material
producer. General skin/glint variants, per-role/actor cohorts and runtime
GPU/replay/video fidelity remain unfinished or unverified. No builds, tests,
syntax checks, JNI/project/game execution, shader compilation, GPU calls or
Bedrock operations were performed. See
inspection/native-geometry-material-source.json.

### Bent emote skin submission order (static source correction)

The Fabric layer submits worn cosmetics before drawLimbs in every supported
version branch. AvatarRenderer previously emitted expressive split skin in the
earlier normal-player loop. It now omits bending parts there, emits cape/wing
and worn models, restores the worn opacity scope, and finally emits the bent
replacements through cosmeticSink. Each part is still emitted once, using the
same posed transform, joint curve/time, texture, packed light and skin tint.
Both software rendering and native staging share this corrected source order.

Cached Java 26.2 class data also confirms that default custom submissions use
order(0), blended non-outline submissions enter translucentCustomGeometry, and
the Arctic glow path explicitly selects order(1). Those facts do not provide
the missing owning native group/object/equality identities or insertion phase.
Logical texture identifiers cannot be inferred from bitmap identity. General
skin variants, layer visibility, native attachment fidelity and actual rendered
order remain unfinished or unverified. No builds, tests, syntax checks, class
loading, project/game execution or Bedrock operations were performed. See
inspection/emote-skin-submit-order-source.json.

### Java player ModelPart skin cubes (static source port)

JavaPlayerMesh now supplies normal skin geometry from cached Java 26.2
HumanoidModel.createMesh and PlayerModel.createMesh definitions. It uses the
existing JavaModelBox ModelPart face order, winding, normals and max-Y UV
layout, with wide/slim arm definitions and independent modern left-side atlas
regions. Hat inflation is .5; jacket, sleeves and pants inflate .25. Outer
children share the owning part's zero local pose and emit after its base cube.
The existing copied caller transforms supply pivots and animation; this helper
does not manufacture native bone matrices or change part traversal order.

AvatarRenderer uses this mesh for head and non-bending skin parts in both
software rendering and native staging. Arctic's SkinLimbs still provides split
emote pieces after worn submissions. Tint, packed light and existing layer
selection are retained. The old PlayerSkin cosmetic-box helper remains for
preview/reference callers. Skin material, overlay and per-part visibility are
not inferred from the mesh: native GENERAL batches still fail material
preparation. Live insertion and complete model/group/traversal fidelity remain
unfinished. No builds, tests, syntax checks, class loading, project/game
execution or Bedrock operations were performed. See
inspection/java-player-mesh-source.json.

### Java 26.2 lightmap generator (static source port)

JavaLightmap ports the cached lightmap.fsh calculation from explicit final
lighting uniforms: sky/block factors, night vision, darkness, boss darkening,
brightness, and four RGB tint/ambient vectors. It preserves the brightness
rational, component-wise ambient/night-vision maximum, sky contribution,
parabolic block tint whitening, darkening and maximum-component gamma stages.
Its 16x16 output indexes block level on X and sky level on Y, with alpha one.
Cached Lightmap constructor data confirms an RGBA8_UNORM render target, so the
generator quantizes before the entity shader's bilinear sampling. The adapter
must explicitly choose a nearest-even or nearest-up conversion policy; neither
is inferred as the installed/source backend's behavior.

JavaEntityShader.Inputs now accepts exactly one of captured lightmap pixels or
lightmapState. Required generator factors use nullable fields so absent values
cannot silently become zero. Vectors are copied before generation. Only the
resolved immutable pixel snapshot is archived or transported in ARCGEO02, so
replay does not rerun world/flicker/random logic and no archive/wire layout or
scalar count changes. No Bedrock shader constant name supplies these inputs.

The literal source shader divides its gamma scale by the maximum RGB component.
A zero maximum produces a nonfinite source result whose backend treatment is
not established. Generation rejects that state and requires captured pixels;
it does not silently replace the source behavior. Source-backend UNORM rules,
GPU precision/fusion and generated pixel equivalence remain unverified. A live
producer of Java-equivalent final uniforms, flicker/effect timing, native phase
insertion and complete rendering/replay/video parity remain unfinished. No
builds, tests, syntax checks, class loading, shader/GPU execution, project/game
execution or Bedrock operations were performed. See
inspection/java-lightmap-generator-source.json.

### Explicit visible skin material and body overlay (static source port)

Cached Java 26.2 PlayerModel constructor data selects entityTranslucent for the
ordinary visible player model; LivingEntityRenderer keeps separate ghost,
outline and missing-body branches. JavaSkinMaterial now represents the explicit
ordinary selection with its own copied normalized body-overlay texel. The
adapter may supply WorldFrame.Player.javaSkinMaterial (schema 1, kind
entity-translucent, overlayColor RGBA). No material is guessed from image alpha,
and the worn cosmetic NO_OVERLAY texel cannot fill a missing body overlay.

Normal JavaPlayerMesh parts use the new skinSink. Software JAVA_SKIN retains
sample-alpha .1 cutout, per-face lighting, translucent blending and base depth
writes, including surviving zero-alpha fragments. Its immutable selected overlay
is carried through triangle clipping/staging and used before lightmap/fog.
Bent SkinLimbs remain in the Arctic custom pass with the cosmetic overlay and
their later submission order. Unknown selection retains legacy GENERAL staging
and still fails native material preparation.

Native SkinTranslucent shares the entity-translucent GPU state but has a distinct
semantic kind. Inputs.for_batch requires the copied body overlay; frame-only
skin preparation rejects it. Constant checks compare overlay bytes directly,
and all shader/lightmap bytes when the frame carries a material cohort. The
order planner allows skin/base roles to share an adapter-proven entityTranslucent
prepared identity, preserving each batch's separate overlay/tint constants;
it does not infer object, equality or group IDs from the pixels.

ARCGEO03 retains the v2 optional common material cohort and adds pass 6. That
pass appends four normalized finite f32 overlay components after its fixed
batch header, before vertices. Empty submits preserve the same metadata.
The decoder retains v1/v2; the producer selects v3 only for explicit skin draws,
and includes the extension in the existing 96 MiB budget. Copied scene-light,
camera, grouping, target ownership and actual native phase checks still apply.

Body archives now write bedrock_body_samples_v5 with magic 0x415243424f440005.
After v4 packed light, each actor appends kind byte 0 unknown or 1 normal
entityTranslucent, plus the four overlay floats for kind 1. The bounded block
estimate includes the 17-byte maximum extension. V1-v4 readers are retained
with unknown material. Exact samples preserve the immutable selection; interior
samples require equal selections/overlays. Playback never borrows live viewer
material, and missing recorded selection cannot attest body appearance readiness
or authorize a new body plan in a v5 track.

These are adapter-owned source inputs, not a live producer or suppression
receipt. Actual installed-game skin/overlay/lighting capture, ghost/outline/
dissolve variants, per-part visibility and native feature-phase insertion remain
unfinished. No builds, tests, syntax checks, class loading, JNI/project/game
execution, shader/GPU execution or Bedrock operations were performed. See
inspection/java-skin-material-source.json.

### Base/outer skin visibility and replay v6 (static source port)

JavaSkinVisibility carries separate copied base-part and outer-layer masks in
Arctic Attach order. WorldFrame accepts schema-1 javaSkinVisibility with required
baseParts and the six hat/jacket/sleeve/pants options. Missing values remain
unknown; no network/engine bit field is reinterpreted as this logical mask.
Cached PlayerModel.setupAnim data confirms those six outer flags independently
control their ModelParts. Normal JavaPlayerMesh emission now honors base and
outer visibility. Bent SkinLimbs honor the outer option even when the normal
parent was hidden for the emote, as in the Fabric layer. Attachment transforms
are not removed by hiding ordinary geometry, so worn models retain their poses.

Native staging distinguishes BentSkin from worn cosmetic base and snapshots
visibility knowledge per skin draw, clearing it when actors change. ARCGEO04
adds pass 7 and flag bit 8, valid only on normal/bent skin. The existing v3
layout otherwise remains unchanged, including the pass-6 overlay extension.
Older packets decode with visibility unknown. Both constant assembly and draw
preparation reject nonempty skin geometry without supplied visibility. Bent
skin uses the cosmetic base shader/overlay and can share an adapter-proven
entityTranslucent identity. Actual group/object/equality IDs are still required.
The flag records Java input availability, not native ownership or suppression.

Body tracks now write bedrock_body_samples_v6 / 0x415243424f440006. After the v5
material field, visibility writes a strict presence byte and, when known, two
bounded mask bytes. The maximum three-byte extension is included in block
budgeting. V1-v5 remain readable with unknown visibility. Sampling preserves
exact values or equal neighboring values, without interpolating masks or
borrowing live viewer settings. Body appearance readiness and v6 body-plan
availability require the recorded visibility.

Installed-game visibility/effect capture, cape/armor/held-item visibility, live
native feature-phase production/insertion and rendered/replay/video fidelity
remain unfinished or unverified. No builds, tests, syntax checks, class loading,
project/game execution or Bedrock operations were performed. See
inspection/skin-visibility-source.json.

### Logical cosmetic texture identity (static source port)

Fabric registration uses arctic:look/cosmetic/<id> and its _glow partner, plus
arctic:look/mesh/<id>_<image index> and shared mesh/white. Cuboid/mesh factory
overloads now retain those identifiers in Bedrock staging. Both live registration
and ReplayAppearance preparation pass the item ID, so shared content-addressed
bitmaps no longer erase distinct registration names. Anonymous preview factory
calls keep unknown item identity; the statically registered white key is known.

NativeAvatarGeometry stores the immutable logical identifier separately from
the copied texture index. A named texture has one frozen frame binding. Equal
snapshots from another image object may reuse it after exact dimensions/pixel
comparison; conflicts reject the staging transaction. Different logical names
can share copied pixels while retaining their independent rendering identity.
The native draw planner includes that name when checking caller-supplied
prepared identities, rather than allowing a shared pixel index to erase it.

ARCGEO05 adds a strict key-presence byte to every batch, after any pass-6 overlay
and before vertices, with a bounded length-prefixed ASCII namespace:path when
present. The source selects v5 only when at least one draw carries a key and
charges all extension bytes to the existing 96 MiB bound. The Rust decoder
retains v1-v4 with unknown identity and validates grammar, length and one copied
binding per named texture. No engine pointer or texture-manager lookup is
derived from this data, and archive format is unchanged: existing Model.id
reconstructs the registered cosmetic identifiers during preparation.

Cached SubmitNodeCollection data also distinguishes translucent model and
custom-geometry phases; SimpleFeatureRenderPhase.sortFeatureInto emits groups
with strictlyOrdered=false, which RenderTypeFeatureRenderer inverts into
canReorder=true. Exact owning factory/cache identities, complete phase
grouping, native-only actor skin/cape/wing identifiers, native insertion and scene lifetime
joins remain unfinished. This data does not create a RenderType reference or
prove GPU ownership. No builds, tests, syntax checks, class loading,
project/game execution or Bedrock operations were performed. See
inspection/logical-texture-identity-source.json.

### Arctic skin, cape and wing texture identities (static source port)

FabricPlatform.registerTexture registers a static image at arctic:look/<hash>;
an animated cape registers each arctic:look/<hash>/<frame> separately. Live
registration now retains those names alongside the corresponding loaded images.
The retained whole animation strip has no invented Java name. AvatarRenderer
carries the selected name through normal skin, bent emote limbs, cape geometry
and both completed-native-pose and Java-model wing paths into ARCGEO05 staging.
The image/name setters bind metadata to the exact image object; legacy direct
replacement of the public image fields cannot reuse a different image's name.

WorldFrame uses the acknowledged skin/cape/wing texture and selected animation
frame. Name lookup follows the same active replay, retained replay, then live
image-owner precedence as pixel lookup. Unknown archive bindings do not borrow
names from a lower-priority live registration. Replay preparation reconstructs
names from explicit Arctic look definitions, including Java's bare-hash name
for a static cape represented by replay's internal /0 alias. Metadata survives
explicit replay unmount alongside its retained images and clears on world reset.
Native-only captured images remain unknown; their content-addressed storage
keys are not Java registration evidence.

This extends the prior logical cosmetic texture port; it does not establish
RenderType object identity, feature-group ownership, native phase insertion or
deterministic video parity. No builds, tests, syntax checks, project execution
or Bedrock launch were performed. Source review only.

### Feature phase dispatcher assembly (static source port)

Cached Java 26.2 PhaseSubmitGrouper does not preserve each phase output
collection as a separate draw group. It merges adjacent deliveries whose
featureType reference and strictlyOrdered flag match. Empty collections leave
the previous group unchanged, while each new phase starts with no previous
group. PreparedGroup forwards the flag, then RenderTypeFeatureRenderer creates
its draw group with canReorder = !strictlyOrdered. In particular, the false
passed by SimpleFeatureRenderPhase permits prepared-type consolidation.

avatar_draw_order::FeatureGroup::from_phases now ports that assembly over owned,
already ordered PhaseOutput data. It preserves phase and feature boundaries,
merges matching adjacent deliveries, ignores empty collections and explicitly
inverts strictness. Geometry::from_phase_outputs joins this assembler to the
existing complete draw-plan/material preflight without issuing GPU commands.
Input limits cover phase count, empty/nonempty deliveries, staged submits and
resulting groups; Plan still requires every staged batch exactly once.

The producer must supply real feature-reference identities and sorted phase
output. CustomFeatureRenderer.Submit.batchKey is its RenderType reference;
SimpleFeatureRenderPhase stores batches in a HashMap and may shuffle them in
debug mode. TranslucentFeatureRenderPhase instead uses a separate unstable
model-distance sort and emits strictlyOrdered=true. Texture-name sorting or
raw per-actor emission would not reproduce those inputs, so no such fallback
is installed. Foreign feature boundaries cannot be omitted from an assembled
scope. The actual native feature-phase producer, scene-owner join and scheduled
insertion remain unfinished. This is source/data inspection and editing only;
no tests, builds, syntax checks, project execution or Bedrock launch occurred.

### Java replay camera argument conversion (static source port)

Java CameraPath.at returns double spline components, but ReplayViewer.PATH
casts yaw, pitch and FOV to float before calling backend.camera. Fabric's
ReplayPlayback.camera receives those floats and writes the yaw to the player
rotation fields. The previous Bedrock path normalized the evaluated double
yaw before converting to protocol float32, omitting Java's earlier rounding
of the unwrapped spline angle.

CameraCurve::sample_for_view_at and camera.cjs sampleForView/sampleForViewAt
now retain that argument conversion for Java-curve segments before wire yaw
normalization. Live helper camera instructions, indexed export's expected
camera calculation, baked replay frame samples and the final baked duration
sample use this path. Raw curve evaluation and saved keyframes remain intact
for the editor; fixed cameras and non-Java interpolation retain their existing
rules. Rational timestamps, interpolation formulas, path fingerprints and
archive format are unchanged.

This ports a source-level conversion order, not proof of native camera/GPU or
video equivalence. No helper, project/game code, tests, builds, syntax checks
or exports were run; Bedrock was not opened. Live native cosmetics insertion,
scene ownership, replay rendering and deterministic video parity still require
completion and runtime evidence when permitted. See
inspection/camera-rational-sampling-source.json.
