# Bedrock adapter

This directory contains two separate implementations:

- `offline/`: a protocol connector for servers that accept unsigned Bedrock players.
- `native/` and `src/`: an experimental Windows native renderer/input adapter for the
  existing Java Arctic Client core. This is not a complete game-engine port.

Equipment snapshots now preserve optional enchantment state, the forced-glint
flag and the selected native base predicate's result. Recordings retain these
values and require known values to match during replay. The source readers check
the pinned tag layout, actual item virtual getter and repeated copies; custom
overrides or unreadable state stay unknown. Native glint layout/fidelity remain
unverified, while Java options/clock, texture and glint rendering remain
incomplete. No build, test or Bedrock launch was performed.

Software wings now have a source port of Java's tick-smoothed animation,
one-pixel box deformation and independent left/right wing part offsets. Native
completed ticks publish the angle endpoints; rendering interpolates them without
advancing the filter. Cape tick track v2 records those endpoints with a v1 reader
for older archives. Missing v2 wing ticks stay gaps. The initial observed filter
state, native crouch semantics, cross-runtime math and rendered fidelity remain
unverified; this does not establish animation parity from entity creation.

Wings use Java's unblended armor cutout material with separate front/back
lighting and depth writes. Older providers/archives retain their sampled native
wing path with explicit right-wing mirroring. Equipment projection layering now
uses a raw native camera pair that must reproduce the published world matrix
before Java derives the separate wing projection. Equipment glint, full native
lighting and native pixel fidelity remain incomplete or unverified.
These are static source changes only; no build, test or Bedrock launch was performed.

Java cosmetic passes now derive their normal matrix once per owned draw pose,
using the cached JOML reference's float operation order. Singular results pass
through Java's signed-byte packing instead of receiving an invented direction.
Software Arctic capes now use a separate Java solid pass and vanilla cape mesh,
including outward face winding, the reversed bottom-face V, back-face culling,
front-normal lighting and unblended RGB/depth writes even at zero texture alpha.
The local cape pose also follows Java's quaternion/matrix/Euler round trip and
JOML's default sine-derived cosine calculation. Native capes retained by the
engine keep their native rendering. This remains an unbuilt source port:
original pose-stack history, native front-face agreement, world lightmaps, fog
and complete rendering parity are unresolved or unverified. No build, test or
Bedrock launch was performed.

The pinned skinned draw consumer and its material helper now have source hooks
for replaying a recorded raw cape's 32 shader values and optional 12-value
material context (multiplicative tint, fog, render distance and HUD opacity).
The version 4 layer track preserves that context, with version 1–3 readers.
The hooks modify checked local
draw fields during the synchronous material copy and restore them afterward.
Readiness requires a matching copy receipt for every prepared description; source
and replay cohorts can differ. This has not been built or tested. Full material,
fog, resource/GPU coverage and deterministic video remain unverified or incomplete.

The shader-input observer source now reads named vector layouts from the pinned
installed Actor material when D3D reflection has been stripped. It matches exact
shader bytecode hashes and observed CB0 bindings before copying bounded vector
ranges at queue time. This can include FogControl, FogColor and TileLightColor
beyond the existing raw prefix, within a separate 512-byte vector budget per
stage. Matrices remain uninterpreted, and these labels do not establish material
ownership or apply native fog/lighting to cosmetics. This addition is unbuilt
and untested; full rendering parity remains unfinished.

Expressive emotes' split skin limbs now use the same Java translucent base pass
as worn cosmetics, matching the Fabric layer's material selection. Their sampled
native tint/fade remains available through the pass. Java cosmetic passes also
pass their normal inputs through Java's signed-byte vertex packing before
lighting. These are source additions; native pose-normal transforms, world
lightmaps, fog and native material application are still incomplete or unverified.

The native snapshot source now copies agreed local dimension name/ID metadata
from the inspected 1.26.5203.0 layout. Known dimensions select Java 26.2's Default
or Nether directional-light pair for worn cosmetics, with separate front/back
colours on both base and emissive passes. Unknown/inconsistent copies keep the
existing lighting fallback. This is an unbuilt source addition: world lightmaps,
fog, native shader/material replay and full rendering parity remain unfinished.

Worn cosmetic rendering now has source ports of Java 26.2's texture-alpha
cutoff, separate base/glow depth-write rules, repeating nearest texture sampling,
and quad-center distance sorting for declared camera-relative native frames.
The cutoff happens before tint/fade, so fading an accepted texel does not remove
its geometry. Cuboid idle animation also follows Java's current motion-toggle
behavior, while retaining an explicit clock for replay. These changes are
unbuilt and untested; native lighting, shaders and full rendering parity remain
unfinished.

The launcher replay editor source now includes a selectable camera timeline,
previous/next keyframe controls, sampling at the edit time, and an X/Z path
overview with the selected camera's heading. These views use the new Rust
camera sampler ported from Java CameraPath and the offline helper. Selecting a
key preserves its saved values; sampling between keys evaluates its curve.
Indexed export uses that sampler to check the helper's requested float32 camera
against the accepted path at each exact fractional timestamp. This checks camera
intent and helper sampling; it does not prove native-world or video fidelity.
These additions have not been built, tested or opened in the application.

The native bridge source now draws world cosmetics on every presentation at
the presentation resolution, fitted uniformly to at most 3840x2160. HUD/menu
layout and input retain a separate two-pixel GUI scale. Terrain depth uses a
conservative footprint resample, including native depth smaller than the output.
DX11/DX12 overlay textures can resize in their existing slot; DX12 retires its
buffered draws before changing that descriptor. The Java heap ceiling is now
512 MiB, and RGBA conversion uses a row buffer instead of another full image.
This removes the source path's fixed 33 ms update limit and half-resolution
world layer. Performance, image fidelity and full native rendering parity have
not been validated. The Java/native bundle must be rebuilt together to use
these changes. No build, test or Bedrock launch was performed.

The replay appearance source now records the six observed native body-part
matrices alongside cape tick history and sampled wings. Playback uses those
recorded references for replacement bodies and attached cosmetics; replay emote
programs pose the recorded baseline instead of the viewer's current animation.
Missing actors, sample gaps and missed ticks cannot authorize a fresh replacement
plan or pass pose-dependent indexed capture readiness. Older archives retain
their preview path, but cannot attest the newly required recorded body poses.
This is sampled interpolation, not a complete native animation recording:
dynamic attachment bindings, full equipment posing, native materials and
deterministic MP4 export remain incomplete or unverified. These additions were
made through source edits only; no build, test or Bedrock launch was performed.

Indexed export also carries the frame's fractional millisecond through native
dispatch receipts into cape, wing, body-pose and emote sampling. The rendered
layer reports the fraction it used, and capture requires the same rational
timestamp as the queued camera sample. This removes a whole-millisecond
quantization step in the source path; native animation fidelity and deterministic
video are still unverified. The Java/native bundle must be rebuilt together to
use this change; the installed bundle has not been rebuilt or launched.

Native appearance snapshots and histories now also retain loaded Bedrock skin
pixels and the observed slim/classic arm model. Replay uses the recorded Arctic
skin when present, otherwise the archived native skin. New recordings mark an
unavailable native image explicitly; an emote cannot replace it with the viewer's
current skin, and indexed capture waits for complete recorded appearance data.
Legacy archives retain their existing preview behavior. This is a source port
for prepared standard skins; persona geometry, animated native skin features and
full native rendering parity remain incomplete or unverified. No build, test or
game launch was performed.

The source also archives completed native attachment palettes in
`bedrock_attachment_samples_v2` (with a version 1 reader). It retains indexed bone
values in the player's root space, slot/layout keys and resolved rig anchors,
with no native pointers.
The loader samples contiguous matching layouts on the fractional replay clock.
The source now publishes a leased plan that maps matching recorded palettes
back into the current checked native draw space. Collection and CPU copying do
not prove complete equipment or GPU coverage. The existing emote export gate
requiring known-empty native attachments remains. No builds, tests or Bedrock
launch ran for this source addition.

Version 2 also carries palettes from an actual selected root cape draw, keyed by
the native cape texture identity and static indexed layout. Playback of matching
raw native cape art keeps the native mesh/material and supplies its sampled
palette through the existing owned output-copy path. Explicit Arctic capes keep
their Java physics and animated art. Native palette samples require contiguous
matching layouts; a missing sample cannot substitute movement-based cape motion.
Indexed readiness requires fresh native copy receipts for the archive and exact
fractional replay sample, plus an owned copied-description receipt for the
archived cape RGBA. The source applies that tint to the copied native description
and restores original bytes on failure. Version 1 archives retain their preview
path. This source implementation adds no new native
offset or hook and remains unbuilt and unverified; GPU coverage, native material
history and full parity remain incomplete. No builds, tests or game launch ran.

Attachment replay now separates rotation from affine stretch instead of blending
matrix entries. This preserves interpolated rotations alongside nonuniform scale,
shear and a consistent reflection. Exact endpoints and unchanged collapsed poses
remain intact; changing singular poses or handedness remain gaps. Camera capture
also compares native positions at their float32 precision, avoiding decimal JSON
rounding errors far from the origin and premature matches nearby. These are source
changes only; interpolation fidelity, GPU results and deterministic video remain
unverified. No builds, tests or Bedrock launch ran.

New body recordings use `bedrock_body_samples_v3`, retaining the actual model
basis, world-space origin and observed body RGBA as well as the six body parts. Replay rebases that
root to the current camera for bodies, capes and cosmetics; matching native
attachment plans use the same recorded placement. Version 1 and 2 body tracks
still load for preview, but lack the color data now required by capture readiness
(version 1 also lacks the model root).
This remains sampled visual replay: native simulation/culling, unsupported root
interpolation, GPU fidelity and deterministic video are incomplete or unverified.
No builds, tests or game launch ran.

The source now carries native body color from the existing checked skin draw,
before suppression changes its alpha. Replacement skins use that RGBA; worn
cosmetics and glow inherit its opacity. Replay uses the archived
body color rather than the viewer's current tint. Missing or conflicting colors
remain unknown, and new tracks with unknown color cannot authorize fresh
replacement plans or pass appearance capture readiness. This adds draw tint and
opacity, not native lighting/material/shader parity. No builds, tests or game
launch ran.

Classic capes and wings now also retain their own checked original draw RGBA.
The separate `bedrock_layer_colors_v4` track (with version 1–3 readers) preserves
observed, absent and unknown states on the fractional replay clock, with layout
changes left as color gaps. Version 4 also carries the named material context;
missing context and changing interior shader samples remain unknown.
Replacement cape/wing art uses its own RGBA once, including alpha, with body fade
as the fallback for a layer without native art. New replay plans and appearance
capture wait for required layer color data. The track reserves 8 MiB and the
combined appearance preparation limit is now 128 MiB, including optional glint
history preparation; texture retention stays at
64 MiB. This is source-only work: material/shader identity, full native lighting,
interpolation fidelity, GPU results and deterministic video remain unverified.
No builds, tests or game launch ran.

Version 2 and newer layer tracks also record the observed cape/wing dithering flag, retaining
unknown separately from disabled. Raw native cape replay copies the recorded
flag and RGBA into the checked owned draw description after construction, with
readback and rollback on rejection. Color receipts retain both original and
applied flag values. Indexed readiness requires the known
recorded flag at the requested sample. Legacy tracks and flag transitions do
not invent it. This remains a source implementation; shader/GPU behavior is
unverified. No tests, builds or game launch ran.

Offline disassembly of the previously cached 1.26.5203.0 executable also maps
named native actor lighting, overlay, tint, glint and UV constants into the
existing read-only shader snapshot. These values are diagnostic CPU copies;
per-material lighting replay and GPU execution remain unverified. No game
process, build or test was started for this source change.

New layer recordings also retain the selected cape/wing draw's 32 named actor
shader inputs, including lighting, overlay, tint, glint and UV values. Native
reads recheck the complete set and draw scope; Java validates it against the
same native observation. Unknown or conflicting inputs stay unknown. Version 3
uses bounded 16-frame blocks; old tracks keep their original binary layouts.
Fractional samples preserve these inputs only across unchanged endpoints.

Raw native cape replay plans carry known recorded inputs as expectations, and
acknowledgement rechecks them before and after the owned tint/dithering copy.
Indexed cape capture also requires matching known inputs. This detects a current
draw whose CPU lighting differs from the archive; it does not restore different
lighting or establish shader/GPU fidelity. Legacy tracks can still preview but
cannot satisfy this new input check. No tests, builds or game launch ran.

Native appearance source now also archives loaded standard Bedrock cape pixels.
It records known absence separately from unavailable pixels, and prepares raw
cape art under a separate texture key so Arctic's animated cape/elytra preparation
cannot modify it. Replay selects explicit Arctic capes first, then the recorded
native cape. An explicit recorded absence can request scoped native cape hiding;
capture waits for that draw receipt or an observed absence. Unknown state cannot
authorize hiding or pass the new cape readiness gate. The native cape image does
not automatically supply elytra art. This work is source-only; native cape motion,
materials, GPU behavior and full parity remain unverified. No builds, tests or
game launch ran.

Replay MP4 export source now includes a separate decoded-frame audit after the
owned camera, appearance and clock controls have been released. The built-in
Windows path decodes to NV12 through Media Foundation; the FFmpeg path emits
NV12 frame hashes. Both feed the same bounded frame-count/timestamp checks and
write a new `.decoded.jsonl` beside the MP4 and `.frames.jsonl`. Its final record
contains separate input RGBA, decoded NV12 and encoded-file SHA-256 digests.
Requested encoder color settings are checked in the input manifest; this does
not verify the decoded color metadata or equate lossy video with the RGBA input.
Decoder/backend changes can change decoded hashes. This is source code only:
no decoder, encoder, build, test or game was run. Repeatable native rendering,
deterministic video and full Java parity remain incomplete or unverified.

## Offline connections

In the launcher, open **Bedrock → Offline play**, enter a name and an offline server,
and start the connector. Join the local address shown in Bedrock. No Microsoft/Xbox
account or tokens are used by the connector. The remote server must explicitly accept
offline players. Names use 3–16 letters, digits or underscores.

The first start needs Node.js 24+ with npm and downloads the pinned protocol helper.
The launcher owns the helper process and stops it when the connector is stopped or
the launcher exits. It listens only on `127.0.0.1`; it is not exposed to other machines.

From the command line:

```text
arctic bedrock offline play.example.net:19132 --name ArcticGuest
arctic bedrock offline play.example.net:19132 --name ArcticGuest --skin skin.png --cape cape.png --slim
```

**Choose skin** and **Choose cape** in Offline play select the same optional images.
Skins accept 64×64 and legacy 64×32 PNGs; legacy limbs and base-layer opacity use
the launcher's Java skin preparation. `--slim` chooses slim arms and requires a skin.
Static capes accept power-of-two widths 64–1024 in a 2:1 layout and are reduced to
Bedrock's 64×32 texture. The helper transmits these RGBA pixels in the self-signed login;
server policy determines what other players can see. This does not alter the local
game's player renderer. Reconnect to change appearance. Animated capes, meshes and
emotes still need native rendering support. With no images selected, the incoming
game appearance is forwarded. Offline UUIDs use Java's `OfflinePlayer:<name>` rule.

The target's ping selects supported protocol data. `--version 1.26.51` can select a
known protocol explicitly; this does not translate between incompatible game versions.
`--play` also starts the installed game. Keep the command open while playing.

This implements unsigned network sessions, not a change to the stock game's sign-in
UI or Store entitlement checks. A game client must permit joining the local endpoint.
Full stock-client gameplay without sign-in has not been verified. Authenticated
servers and Realms cannot be joined through this mode.

Protocol integration checks:

```text
cd mod/versions/bedrock/offline
npm ci --ignore-scripts --omit=optional --no-audit --no-fund
npm test
```

The headless network tests make real encrypted connections through the relay to local
modern and legacy Bedrock protocol servers and check the chosen name, Java-compatible
offline UUID, unauthenticated identity, custom skin/cape pixels and self-signed login.
The private helper explicitly disables online authentication
entry points; it cannot fall back to saved Xbox credentials.

To check the built Rust CLI's first-use installation and session lifecycle:

```text
node mod/versions/bedrock/offline/launcher-smoke.cjs target/debug/arctic.exe target/bedrock-offline-smoke
node mod/versions/bedrock/offline/launcher-replay-smoke.cjs target/debug/arctic.exe target/bedrock-replay-smoke
node mod/versions/bedrock/offline/launcher-replay-smoke.cjs target/debug/arctic.exe target/bedrock-replay-smoke --kill-owner
```

### Offline Bedrock packet replays

**Record connection…** selects a new `.abr` file before starting Offline play.
**Choose recording…** and **Start local playback** serve it at `127.0.0.1:19143`.
Stop the local session to finish the recording. Existing files are never overwritten.
The launcher stops/finalizes the helper on a background worker.

```text
arctic bedrock offline play.example.net:19132 --name ArcticGuest --record-to session.abr
arctic bedrock replay session.abr --speed 1 --start-ms 0
arctic bedrock replay session.abr --nethernet
```

Type `stop` followed by Enter in either CLI command to finalize and exit cleanly.

The **Show as a local LAN world (preview)** option and CLI `--nethernet` select
NetherNet instead of direct RakNet playback. The helper binds `127.0.0.2:7551`
by default, announces **Offline playback** to the local native discovery listener,
and verifies self-signed login keys and the advertised nonce without Xbox/OIDC
authentication. ICE gathering, signalling and media candidates stay on loopback;
no STUN/TURN services are configured. Reset retires only the viewer's peer, leaving
the local listener available for a fresh connection. These additions are static
code ports, not runtime evidence: no build, tests or Bedrock launch was performed
for this change. Native replay playback and deterministic MP4 remain unverified.

Playback uses a fresh offline encrypted connection. It restores recorded world
packets and translates the recording player's movement into server position updates.
`--start-ms` rebuilds from the beginning through that timestamp; speed is 0.1–8.
This is a Bedrock packet recording, not a Java `.mcpr`, a video, or a fabricated scene.
The launcher camera editor adds position/rotation/FOV keyframes, Java Catmull–Rom curves and linear/smooth/hold
interpolation, JSON path save/open, live preview, pause/resume, speed changes and
forward seek. CLI `reset` rebuilds from the recording on a fresh connection at the
requested timestamp; it pauses playback and retires the old viewer. The source
also accepts `{"op":"reset","timeMs":1000,"reconnect":true}` for an initialized
viewer on the local RakNet protocol 1.26.51 listener. It serializes a transfer back
to that same loopback endpoint, flushes old-world packets in protocol order and
briefly keeps the retiring peer alive to deliver the transfer. The replacement
must use a new connection/viewer ID, the same offline profile, acknowledge local
player initialization and catch up packets to the paused target before reset
status becomes ready. A 60-second deadline fails the transition explicitly.
This uses Mojang's documented
[TransferPacket](https://mojang.github.io/bedrock-protocol-docs/1.26.50/packets/transfer-packet/)
fields; installed-client reconnect behavior and terrain-ready binding remain
unverified. NetherNet and other protocols retain manual rejoin; omitted/false
`reconnect` retains the prior reset behavior.

The launcher **Seek** control resets for backward moves and **Restart world**
returns to the recorded initialization checkpoint. The editor displays transfer,
initialization, catch-up and failure status and disables playback/camera/export
changes during automatic reset. Old-generation disconnect events cannot clear
a newer viewer's readiness. A timeout does not claim a successful reset or
restart the helper/game; a later manual rejoin can recover packet playback.
These changes were inspected and edited statically only, without builds, tests,
helper execution or opening Bedrock. Reset readiness is a connection and packet
queue acknowledgement, not proof of native world rendering or deterministic video.

The indexed export source can now request a fresh world with **Reload world
before export** and a selected start time (`startMs` in CLI export options).
It reserves the native capture slot without opening an encoder, confirms the
existing native local UUID matches the replay viewer and refuses another active
appearance owner. After transfer, it checks the fresh viewer and catch-up target,
then waits for a new native world epoch with idle bindings in the same process.
The reservation renews while waiting; activating indexed capture requires that
same owner and an unexpired lease. Error/cancellation releases it and addresses
only the owned replay-reset generation. Omitted start time retains export from
the current world/playhead. The returned `worldRebuild` receipt records the old
and new viewer/native identities; broad native/video verification remains false.
NetherNet rebuild export, complete terrain/appearance/audio fidelity and repeated
decoded-video equality remain unfinished and unverified. No builds, tests,
helper operations, client launch or runtime capture were performed for this port.

The static native equipment port now copies armor and hand container groups into
owned world snapshots, with explicit empty/unknown slot state, canonical item IDs
and observed stack counts. It requires known getter RVAs, bounded vectors, stable
slot headers/names, and unchanged component/container pointers. Unreadable groups
remain unknown; an independently readable chest still supports the earlier cape
and elytra checks. The hand group retains native order and is not a whole-player
inventory count. Item icon rendering, durability and native armor/held-item emote
poses remain unfinished; the general game-widget capability stays disabled.

New Bedrock appearance histories record these observations per actor as optional
`bedrockEquipment` state. Equipment changes participate in the same bounded event
history and asset budget; no raw engine pointers or item objects are archived.
Playback validates the groups and requires observed recorded fields to match the
actual native viewer before acknowledging appearance readiness. Old snapshots
without equipment observations keep their prior appearance behavior. Emote export
additionally requires known empty armor/hands until native equipment consumes the
replacement pose. This is a completion check, not an implementation of those
missing poses. No builds, tests, helpers or Bedrock launch were performed here.

The static item-icon port now reads loose or `.brarchive` resources from an
explicit ordered directory-pack stack, without extracting or modifying the game.
It bounds archive tables/entries, checks source stability and containment, accepts
binary payloads, and refuses conflicting loose/archive versions. The sprite
loader resolves observed item-atlas keys in supplied pack priority order and
decodes only unambiguous static PNG mappings. The streaming catalog reader
accepts JSON comments and an omitted `texture_name`, rejects duplicate keys,
and masks unsupported overrides instead of falling through to vanilla. Static
texture arrays use an exact observed frame index; the loader does not assume
the first entry or clamp an invalid index. Its worker queue, decoded cache and
pending upload pixels are bounded. Dynamic selectors, MCB atlases, tint/glint,
3D block icons and durability
remain unsupported by this path.

`NativeBridge.bindItemResources(session, json)` takes `roots` (absolute pack
directories, highest priority first) and optional `icons` (canonical item ID to
actual registry atlas key). Native equipment snapshots now carry optional
raw `iconKey` metadata from the stable copied Item prefix. `iconFrame` is emitted
only for the pinned base getter when the component getter and humanoid-armor
predicate both return constant false/null, and no alternate atlas is selected.
That constrained branch returns the stored key/frame without stack or animation
arguments. A raw key alone cannot authorize native sprite selection. An explicit
adapter binding must agree when a resolved native selection is present. These
metadata layouts remain unverified in the
running installed client. It requires a current matching world. The engine
adapter can supply explicit roots through this compatibility entry point. The
native world snapshot now also carries an observed stack automatically, using
the exact-image directory-pack observer described below. Native observations
supersede explicit bindings when present. No default vanilla stack or icon-key
mapping is guessed. `itemResourceState()` and appearance state report
preparation/coverage errors with `activeEngineStackVerified: false`.

The optional frame is preserved in equipment histories and compared against
native replay equipment when known. Older archives without it remain readable;
missing frame information cannot select a multi-texture array. Cache identity
includes the selected frame, so distinct variants cannot reuse another image.
The selection/branch evidence is in `native/inspection/item-icons-1.26.5203.0.json`.
Status exposes `staticBaseLayerOnly: true`, `nativeItemRenderingVerified: false`
and `iconFrameLayoutVerified: false`. Component, humanoid/trim and other item
overrides still need their own resolved engine output. This does not establish
complete armor icons or native item-rendering parity.

The emote port now has an actor/session-scoped target-pose bridge. The client
publishes immutable animation programs with bounded leases; an owned native skin
draw supplies its actual six completed part matrices and checked tick timing.
The callback runs the shared Java `Rig.poseMatrices` implementation. Replay
programs require the same qualified held-clock/dispatch marker used by the
cosmetic clock, and keep their original event phase, held state and local UUID
remapping. Live programs use the existing native animation-clock origin.

Target matrices return to the world snapshot only when native clock, actor,
animation frame and original part matrices agree. The software renderer may use
that target when the current program and phase still match. World gaps, explicit
appearance mount/clear, emote changes, expired publishers and conflicting native
targets invalidate it. The callback reads no UI, Looks catalog, files or engine
objects; it receives copied timing and matrices. Program count is bounded to
128 actors and retained animation data to 16 MiB.

This is target math, not a native armor/held-item application receipt. Equipment
palette mapping, deferred buffer lifetime and native mesh posing remain
unfinished. Equipped-emote export still requires known empty attachment
containers, and `nativeEquipmentPoseApplied`/`nativeEmotePoseVerified` remain
false. No probe configuration was enabled. This source is unbuilt and untested;
no game or test helper was run and Bedrock was not opened.

The GUI backend now draws loaded static item sprites and actual stack counts;
`itemIcon` omits the count. Armor has its own widget capability while other game
features remain disabled. Live armor rows require a complete known group and all
its icons; unbound, unknown or pending resources produce no live panel. A world
change clears the sprite binding and queued stale reads; late uploads cannot
cross its session. Status includes pending jobs and decoded/upload byte usage.
This source code has not been built or tested. No resource loader, helper,
game launch, icon upload or live render was executed for this port.

The native resource observer now has source wiring from the primary client to
the manager's full stack, selected subpacks, optional overlays and base-directory
strategies. Its field paths and forward lookup order were derived from offline
disassembly of the cached 1.26.5203.0 image, cross-checked with public declarations;
see `native/inspection/resource-packs-1.26.5203.0.json`. It decodes only recognized
plain field getters without calling them. Absolute directory paths come from
the engine strategy's resolved path field. ZIP/encrypted/custom strategies,
relative paths, unstable copies and exceeded bounds leave the binding unavailable.
It never substitutes a lower pack after an unsupported active entry.

Pack instances retain their own ordered subpack/overlay/base roots. Catalog lookup
chooses the first existing atlas file within an instance, then merges keys across
instances; it cannot reveal a parent's atlas key hidden by a selected subpack's
catalog. Texture files use the same grouped resource lookup. A pass-through full
stack composition hook invalidates resource revisions before and after restack.
Buffered world draws recheck the current primary stack and cannot republish a
superseded revision or selected-subpack identity. World-frame gaps
discard native bindings so preparation can restart on a fresh snapshot.
Native copies are bounded to 64 instances/64 roots, 64 KiB of path text and
256 KiB of guarded bytes. Owned snapshot memory is counted in world history.
This observer and bridge are unbuilt and untested; active stack selection,
reload behavior and live sprite rendering remain unverified. Only source work
and offline inspection ran; no project code or game/test helper was executed.

Appearance histories also have an independent `bedrock_wing_samples_v1` track
for actual completed native render poses. Each sample retains both wing matrices
and its rigid torso reference, UUID, native tick/rate and precise mapped time.
Blocks contain at most 64 samples/128 actors and 2 MiB uncompressed, sharing the
appearance writer's existing queue, archive and index budgets. Empty blocks are
gaps. Mount verifies every chunk; playback retains at most two decoded blocks
within a reserved 16 MiB preparation budget and remaps the recorded local UUID.

Wing playback interpolates positive axis scales and translations linearly and
uses shortest-path quaternion interpolation for rotation. It preserves exact
sample endpoints, rejects missing actors/skipped ticks/long unobserved intervals,
and does not extrapolate or substitute the viewer's live wing animation. Wings
retain an independent model root; the saved torso reference is metadata rather
than an emote parent. Native wing
suppression still requires its existing owned draw receipt and isolated probe
gate; the track does not enable that gate. Sampled interpolation is not proof of
native animation fidelity, lighting or glint. Deterministic wing export remains
gated, and full Java parity is still incomplete. These changes are unbuilt and
untested; no Bedrock process or helper was opened.

Camera editing currently targets protocol data 1.26.51 (2193, the installed 1.26.52
family). It uses Bedrock's public camera presets/instructions, independently of the
native overlay. The pinned protocol's FOV easing field is corrected to a string per
[Mojang's schema](https://mojang.github.io/bedrock-protocol-docs/1.26.51/types/camera-instruction/).
The CLI also supports exact paused packet steps with a generation and increasing
sequence number. Completion acknowledges that eligible recorded world updates through the
target timestamp has been queued, including multi-batch updates at that timestamp.
It does not acknowledge client processing or terrain readiness. Reset, disconnect,
stop and timeout cancel pending steps, and stale generations/sequences are rejected.
Headless encrypted-wire tests cover fresh connections, old-peer retirement, held
camera messages, complete batches and cancellation; they do not launch Minecraft.
Pausing stops packet delivery and the camera timeline; it does not freeze the client's
own simulation/particles. Camera paths can only see chunks present in the recording;
camera roll and the Java client's native replay backend are not provided by this route.

New editor keyframes use `catmull_rom`, ported from Java's `CameraPath`: duplicated
endpoint controls, sequential shortest-turn yaw unwrapping, clamped pitch and
linear FOV. Existing paths retain their saved modes; a missing mode remains
linear. **Open path…** also accepts an extracted Java replay `arctic.json` with
extras version 1, sorts its camera keys and retains the last edit at a duplicate
timestamp. Imported edits are saved in the Bedrock camera format and use the same
sampler during live packet playback and baked `.abr` export. Curves that leave
the supported world-coordinate range are rejected before packet serialization.
These additions are source-only and have not been built or tested.

**Export edited replay…** writes a new `.abr` with a baked camera track (1–120
samples/s, default 25) and the
original world packets/timing. Existing files are refused. Real-time MP4 recording
is available through the loaded native bundle, described below. Direct local singleplayer
packet capture is unfinished. Playback's source now indexes resource-pack archives
already embedded as recorded download descriptors/chunks before the world bootstrap.
It checks the advertised size and SHA-256, preserves the original pack stack and
serves a fresh viewer's chunk requests from those bytes. Offers use the loopback
server with CDN URLs cleared. Downloads are separate from replay timing, so a cached
viewer and a new viewer can negotiate independently of the recorded client's requests.
The launcher shows the included pack count and size.

Each archive is bounded to 512 MiB; the combined budget is 1 GiB and 65,536 indexed
chunks, within the existing 2 GiB recording limit. Archive payloads stay on disk;
only descriptors, offsets and hashes are retained. Queued requests and retries are
bounded, and chunk packets are checked again before serving. Missing/incomplete
archives or hash mismatches reject playback before the server reports readiness.
Recordings where packs were obtained from a CDN or were already cached may lack
their download bytes; this port cannot reconstruct those missing archives.
These are source edits only: no build, tests, helper or Bedrock process was run.
Gameplay and resource-pack negotiation in the installed client remain unverified.

The recording player's visual packets now have a source playback adapter. It
maps that player's recorded UUID/unique entity identity onto each fresh viewer,
preserving the skin/cape/animated-image/geometry data carried by player-list and
skin packets. Both legacy grouped and current per-entry player lists are handled.
Remote entries retain their identities. Owned held-item changes, arm swings and
native Bedrock emote lists/events are projected onto the recorded local runtime
entity. Initial skin, held-item and emote-list state is deferred until after the
world bootstrap; arbitrary client requests remain filtered. Fresh recordings
normalize outgoing self skin updates to the selected offline UUID, and record
the normalized packet that the relay forwards. Login envelopes remain excluded.
If no initial skin was present in the recorded protocol stream, it cannot be
recovered from that recording. Arctic catalog look/emote history is still separate
and is not yet embedded by this adapter.

Indexed native export now requires cape-frame and cosmetic-idle time to match
the recorded frame timestamp. The Java layer accepts this clock only from an
acknowledged dispatch-only marker and the same held tick/fraction with both
native timers controlled. The same time selects world/preview cape frames and
cosmetic idle motion. Capture tags include it only after the world layer was
rendered, and the host/native indexed-capture requirements compare it before
accepting pixels. This does not replace the physics clock or recorded emote
history; live Arctic emote start times still use their existing native clock.
These additions are source-only, without builds, tests, helpers or game launches.
They do not establish native visual-event handling or deterministic-video parity.

The Bedrock client's source now retains raw appearance downloads in its own
`appearance-assets` cache. Texture PNGs, cuboid geometry, idle/emote animation
JSON and GLB models are checked against their content-addressed SHA-1 before
storage and reuse. Skin/cape preparation happens afterward. Managed entries are
bounded to 4 MiB each, 4,096 files and 256 MiB; cache I/O failure does not prevent
an otherwise valid live download from being used. Launcher-provided raw textures
are copied to the cache on the existing background worker.

`NativeBridge.saveAppearance(output)` adds a client-thread source API for a new
absolute `.arctic-looks.zip` snapshot. It captures cached player looks, fits,
classic/slim selection, cape rate, current loaded model definitions and emote
phase/hold state, with explicit absent looks/emotes and local-player identity.
The archive includes the required raw files and per-file SHA-256 alongside their
original content hashes. Sorted entries, fixed ZIP times and stored payloads
avoid wall-clock/path metadata. Missing assets or changing model definitions
fail capture; existing files are never overwritten. Capturing the state does
not schedule lookups or asset downloads.

The source also reads these archives without extracting them, checking indexed
sizes, hashes, images, models and actor identities before committing a mount.
Mounted looks restore skins, classic/slim selection, cape frames/rates, fitted
cosmetics and recorded emote phases. The recorded local identity maps to the
current viewer. Export dispatch markers drive recorded animation time; without
one, the explicit snapshot time stays held. Live catalog downloads cannot replace
the mounted assets. Native body/cape suppression still requires its existing gates.

`native-appearance` provides request-correlated save/load/time/clear/state commands
for an already-loaded adapter. The replay editor can select a snapshot for indexed
MP4 export. Export refuses to replace an existing mount, uses a private owner token,
requires the selected manifest, replay time and draw readiness in frame metadata, and releases its
mount during cleanup. Selected snapshots must precede the export origin. Texture
history has a separate 64 MiB admission budget, including eventual unmount, so a
buffered native draw can still resolve previously acknowledged pixels. History
ends on disconnect/session change. Models and new look selection leave on unmount.

Draw readiness checks visible actors against the native body/cape plans actually
acknowledged in that frame. A previous mount's body/cape, unavailable attachment
poses, missing native replacement or custom cape during elytra flight cannot
satisfy the selected-snapshot capture fence. This gate can reject an export while
the remaining native pose/replacement ports are incomplete; it does not infer
that pixels, terrain or an entire replay are correct.

The source now also has an explicitly armed appearance-history recorder. It
observes loaded logical changes on rendered client callbacks, including actor
presence, fitted models, cape rates, emote starts/stops/held phases and settings.
Advancing animation time alone does not create another event. A bounded writer
copies raw assets into its private staging cache and seals a format-2 timeline in
the same `.arctic-looks.zip` extension. Full states are checked asset entries; the
manifest contains only their ordered timestamps and content hashes. Missing assets or queue/index/byte overflow
fail the take. World/local identity changes and backward clocks fail it as well.
`history_finish` queues finalization; `history.stage=complete` confirms the archive
writer finished. An interrupted or cancelled take is not reported as complete.

The reader shares prepared images/models/animations across timeline events within
one bounded 128 MiB preparation budget and selects the latest state at each replay time.
Each state stores a separate cosmetic clock, preserving cape/idle animation phase
when the recording's timeline starts at zero. Older format-1 snapshots retain
their original time behavior. Export checks both origin and last frame against
the selected history's range and retains its draw-readiness requirements.

The CLI source now offers paired recording through `native_record_begin` and
`native_record_finish` while an existing unsigned relay take is initialized. It
records a fresh local marker's actual queue time, waits for the matching native
dispatcher/post-tick receipt, and waits for the matched world frame to reach it.
The native consumer validates that receipt and derives the first appearance time
from the completed-tick clock. Manual history origins remain available separately.

The launcher source exposes the same paired capture in the Offline connection
card. Choose **Record connection…** before starting the relay, join that
recording world with the native bundle already loaded, then choose **Capture
Arctic appearance…**. **Save replay with appearance** finalizes the pair while
leaving the connection open; **Stop local session** saves the pair first.
**Discard appearance take** releases only that owned appearance capture while
the packet stream continues. Capture starts at the matched marker, so the
earlier packet prefix has no paired appearance coverage.

Worker results return the connection and any retained lease after failure, so
saving/cleanup can be retried. A begin whose acknowledgement and cleanup both
fail retains recovery ownership instead of forgetting a possibly active writer.
The CLI reports that lease and supports `native_record_cancel`. The launcher
defers a normal close through paired saving, keeps the window open on a save
failure, and prevents tray handover/background restart while its Bedrock work
owns a session. Relay events are polled across launcher tabs. Unconsumed worker
results attempt owned cleanup in the background; forced termination and crash
recovery remain unfinished. These UI/lifecycle changes are source-only and have
not been built, run, tested or used to open Bedrock.

Paired finish stops both capture streams before waiting for writer completion,
then publishes `take.abr.appearance.json` beside `take.abr` and
`take.abr.arctic-looks.zip`. The descriptor binds the recording UUID, both file
hashes and the appearance manifest. The helper hashes its actual source descriptor;
replacement paths, escaping companions, changed bytes and foreign IDs reject the
pair. New live takes get UUIDs; older ID-less recordings keep their prior format
and camera-export behavior. Source changes after playback indexing are rejected.
Export without an explicit appearance selection uses a valid adjacent companion,
checks the open replay's hash, and compares the manifest/ID again after native
mount. Corrupt or disappearing links do not silently select current appearance.

This is an initial marker/post-tick clock association. It does not prove continuous
clock alignment across lag, pauses or speed changes, earlier prefix coverage, or
packet/terrain application. Alignment/native/video verification flags stay false.
New histories additionally record the endpoints of Arctic's native, tick-driven
cape filter: previous/current player and cloak positions, body-yaw pair, walk/bob
phase and flight ticks. The checked local camera context keeps this filter alive
in first person even when no local model draw occurs. Owned tick data accompanies
the world/depth snapshot, so delayed rendering does not read a newer filter state.

Cape data uses separate zlib-compressed binary blocks of at most 64 ticks / 128
actors and a 2 MiB inflated limit, sharing the archive's asset/queue budgets.
Every block is validated before mount; playback holds one decoded block and
rechecks the source manifest and content hashes when loading another. Recorded
endpoints reproduce displacement interpolation, body yaw, walk/bob and quadratic
flight blend at the requested frame time. The recording checks that endpoints
reproduce the supplied native motion sample and fails if same-tick data changes
or disappears. Missing ticks/actors stay gaps. A physics-bearing history never
falls back to the new viewer's filter; missing motion hides that cape layer and
cannot satisfy selected-appearance export readiness.

This records Arctic's filter driven by observed game ticks; it does not inspect
the engine's internal cloth solver. Armor/held-item poses and native wings remain
unported, and observed state is not a verified suppression or GPU result.
These edits have not been built, tested or executed. Full parity remains unfinished.

The format keeps packet direction, original bytes and monotonic timestamps; login
envelopes, session keys and transport handshakes are excluded. Writes have bounded
queues, a 2 GiB file limit and a finalization marker. Incomplete files are preserved
and refused rather than presented as playable. A headless test records a real
encrypted offline session (bootstrap, block updates and player movement), then
opens a fresh offline connection and verifies the replayed packets and identity.
Additional tests check camera interpolation, wire FOV bytes, live camera/clear and
pause commands, and exported camera tracks. The built-CLI replay smoke checks all
the way through stdin controls, helper export and fresh encrypted playback of the
exported world and camera with an offline identity; it does not start Minecraft.

The helper uses MIT-licensed [bedrock-protocol](https://github.com/PrismarineJS/bedrock-protocol)
and its transitive dependencies, pinned by `offline/package-lock.json`.

## Native client preview

The scoped camera controller now has a normal command route in the source,
independent of the body probe. `arctic bedrock native-camera` sets/releases a
native position/quaternion/FOV target in an already-running game. It requires a
fresh owned camera, the matching world session, and an increasing sequence.
The launcher API correlates each reply with a unique request ID so an older
receipt cannot satisfy a new command. Requests expire after two seconds in the
consumer queue. A restoration failure disables further control for that session.
Requests and reports use `arctic-bedrock-PID.camera-request.json` and
`arctic-bedrock-PID.camera.json` beside the loaded bundle; the same request schema
is exposed by `arctic_bedrock_camera_v1` for in-process callers. Legacy probe
filenames remain restricted to the isolated probe mode. No camera override is
enabled until a caller explicitly supplies a target.

This API was added through static source edits only. It is not yet connected to
the Java replay backend or a native terrain/presentation fence. Full replay
lifecycle and deterministic MP4 export remain unfinished.

The static emote port now registers visible native actors with the shared remote
emote watcher. Body readiness can select the actor's already-loaded native skin
when an emote plays without an Arctic custom skin. Native skin keys must have the
owned image's SHA-256 form and still match that actor's current appearance and
arm model at its scoped draw. The snapshot retains `nativeSkin`/`nativeSlim`
separately from the texture/arm model acknowledged by a replacement draw. The
renderer uses that acknowledged pair until a later native draw accepts a new
plan; active native replacement textures stay pinned within the bounded image
cache. A full pinned cache declines new uploads rather than evicting an active
replacement's skin.

Bedrock emotes now use the shared rig's matrix-preserving posing entry point.
Six-part emotes keep untouched native matrices, position-only tracks preserve
their native rotations, and expressive root/torso/chest/pelvis transforms compose
directly with the baseline rotation matrices. No native pose is modified or
reconstructed through Euler angles by this path. These changes have not been
built or tested. Body suppression still uses the experimental isolated-probe
gate and the existing supported standing rig; all-posture/armor/native-cape
replacement and deterministic replay export remain unfinished.

The next static port adds a separate native-cape replacement path. Readiness
includes the loaded Arctic cape hash/frame and the actor's original native cape
key. The native model observer classifies the geometry's actual hierarchy: a
cape-only `cape`/optional `root` hierarchy is distinct from the six-part skin
hierarchy. It also requires the existing standing rig, empty chest slot, matching
animation generation, current actor appearance and a completed-tick physics
sample. The skinned-draw constructor retains its root/color/mesh checks before
substituting the owned cape's borrowed bone palette. Body and cape replacements
have different receipt types and counters.

The world layer enables replacement only for that matching cape receipt. It
retains the accepted cape hash across look changes, samples its loaded animated
frames on the world clock, and uses physics copied with the native draw receipt.
It does not infer suppression from a missing cape image. This new path remains
behind the existing experimental isolated gate. Its cape-mesh classifier and
actual native invocation have not been validated in the installed game. All-pose,
armor/elytra coverage and deterministic rendered export remain unfinished; no
tests, builds or game launches were performed for this source port.

The read-only chest-slot source port now also copies an optional canonical item
ID. It follows the public SDK's weak item reference, checks namespace/local/full
name agreement and rechecks the item, slot, vector and container after copying.
Unreadable or inconsistent names remain absent; an unreadable chest slot never
becomes an empty slot. World snapshots distinguish `elytraEquipped` from the
existing `elytra` gliding flag and reject contradictory equipment fields.
The menu preview therefore hides cape geometry for equipped folded wings as well
as active flight. The world path retains its existing empty-chest requirement;
this does not authorize a new native suppression path. Selected-snapshot export
continues to reject custom cape/elytra frames until wing rendering is ported.
The optional name layout is SDK-derived and has not been validated in the installed
game. No tests, builds, helper scripts or Bedrock launch were run for this addition.

The static clock port now has a regular script/API route using
`arctic_bedrock_clock_v1`, `bedrock::client::clock_control`, and
`arctic bedrock native-clock`. The existing control worker handles bounded
PID-scoped request/report files; commands carry live sessions, increasing
sequences, unique request IDs and expiring leases. Freeze/step/sample/renew/resume
operate on the owned native client timer and optionally its matching real timer.
Legacy probe files remain restricted to probe mode. No clock changes until a
caller requests them. The camera-delta, fog and scoped renderer clock bindings
are installed for the exact-image control path; shader diagnostics retain their
isolated-probe gate.

A step receipt now waits for the actual primary level tick after its native
callback and cape simulation finish, as well as the requested real-timer step.
Calculated timer counts alone cannot satisfy this completion. Unexpected extra
ticks, lost ownership, expired leases or incomplete operations release the
clock. Session transitions clear counters and pending state. The host API checks
the actual timer fractions and request-correlated receipt. This is source-only,
without builds, tests or a Bedrock launch; native replay packet/terrain fences,
integrated-server stepping and deterministic rendered-video export remain
unfinished.

The static replay lifecycle port distinguishes the encrypted connection from
the client's local-player initialization acknowledgement. Native LAN replay
feeds bounded packet batches through the recording's initialization checkpoint,
then holds the timeline until the new client acknowledges the same runtime
entity. This also works while paused; waiting time does not advance playback.
Missing/mismatched checkpoints are rejected, and initialization has a bounded
timeout. Reset, seek and exact stepping honor that minimum bootstrap timestamp.
The helper's `waitForClientInit` option defaults on for NetherNet and off for
legacy RakNet playback.

Replay state carries viewer identity, generation, initialization and packet
queue readiness separately. The replay editor waits for initialization and
packet catch-up before enabling real-time MP4 capture. These are protocol
receipts, not proof that native terrain or rendering has completed. No builds,
tests or game launches were performed for this source change; native
packet-application/terrain fences and deterministic rendered export remain
unfinished.

The static pose port now copies the six completed matrices from the hierarchy
selected by the actual skin draw. It checks the standard rig's rest transforms,
pivots, affine shape, rigid basis and handedness, rather than identifying the
pose from bone names or a standing AABB. The copied pose is bound to the actor's
appearance, animation frame and owned native world draw. The world renderer can
use that source for posture and attack translations/rotations without deriving
a model scale from the actor's changing height. The restricted standing-only
observation remains a fallback when no selected skin draw was captured.

Body/cape suppression receipts now require the same captured skin pose. Existing
experimental suppression controls, texture readiness, original-cape ownership
and empty-chest checks remain in place. The snapshot also forwards the observed
fall-flying flag to the cape renderer. This expands source support for standard
rig postures; it does not establish all-pose, armor, persona or elytra parity.
No builds, tests or game launches were performed for this change.

Wing emission now keeps Java's two-pixel layer translation at its model root,
before local wing poses, and follows baked root-child traversal for base/glint.
Legacy native wing poses stay under their independent entity root instead of a
humanoid emote torso. Native replacement gates remain, and full rendering,
controller and export parity remain unverified.

Shared pose composition and known Java vertex passes now use JOML's default
generic affine float grouping, including explicit glint point transforms.
Fit.NONE preserves an unchanged pose copy; moved fits translate before mirroring.
Original matrix-property/normal history and native pose equivalence remain
unknown. No builds, tests or Bedrock launches were performed for these edits.

Normal skin parts now follow the stock Java model's baked child-map traversal,
retaining each part's cube-before-outer-child sequence. Bent originals are hidden
only in the expressive rig branch; legacy joint-only emotes retain Java's normal
part plus separate custom split geometry. Shared emission updates software and
staged native geometry, without establishing native world insertion or pixel
parity. These edits were reviewed as source only.

The source renderer uses continuous Java-layout skin boxes for unbent limbs and
the torso; expressive emotes still use the shared split limb geometry for joints
they bend. Head/hat geometry uses the same cached skin model. Cosmetic glow and
sheen are submitted after ordinary base transparency, matching Java's ordered
cosmetic pass while retaining terrain/opaque depth tests. The native cape filter
ignores repeated or older completed-tick callbacks and restarts from observed
feet after missing ticks, rather than integrating one sample as missing history.

The static input port maps Minecraft keyboard binding names to Windows input and
the shared UI's GLFW codes separately. This includes F1–F24 (including the default
F9/F10 replay bindings), punctuation, navigation, keypad digits/operators, locks
and left/right modifiers. Middle and side mouse button bindings now have native
press/release events and held-state lookup. This does not add native replay
backend registration or game-control injection. Keypad Enter remains shared
with ordinary Enter in the current native key-state packet. These edits were
reviewed as source only, without builds, tests, helpers or Bedrock launches.

Direct native-camera export now acquires an idle override and changes/releases
only its acknowledged camera identity. Native admission and camera consumption
both check ownership. Cleanup retains successful camera/clock releases separately
and attempts clock release even if camera cleanup fails. The protocol camera
route and frame readiness checks remain. Automatic direct targets now carry a
30-second wall-time lease, and exact consumer claims retain restoration ownership
through expiry. Cleanup can recognize the same owner already expired without
publishing another command. Lost acknowledgement reconciliation and full runtime
camera/export parity remain unverified. These are source-only edits,
without builds, tests, native commands or Bedrock launches.

Software world rendering now also consumes its own retained solid/custom phase
collector. Known named materials draw in phase/bucket order and sort compatible
custom buffers separately; unknown/oversized input retains legacy rendering.
Native packet collector history stays independent. A checked raw view/projection
pair also enables whole-skin model ordering with consecutive compatible draws;
unknown views keep the older skin stage. Checked prepared skin/base/bent/glow
draws now select perspective origin-distance or orthographic negative-Z keys
from original quad midpoints, preserving clipped pieces and stable ties.
Unchecked contexts and legacy queues keep the older sorter. Native GPU
insertion, Java-equivalent vertex space, complete quad/model fidelity and full
parity remain unfinished or unverified. This remains source-only, without
builds, tests or Bedrock launches.

The source renderer also has an explicit-input Java armor-glint pass: moving UVs,
blurred repeating texture sampling, fog fade, EQUAL depth and source-square RGB
blending with retained destination alpha. Its adult wing entry point reuses the
base geometry. The world-render source path now consumes explicit live or
recorded-only observations, retaining base clip coordinates and deferring
selected whole wing models until after ordinary translucency and before glow.
It uses observed texture/fog transforms and unstable model ordering, and rejects
mixed active/unknown wing selection rather than sorting an incomplete subset.
Active samples require verified artwork and a layered camera. An authoritative
live producer and changing-clock reconstruction are still missing; full parity
remains incomplete. The optional `java_reference_jar` bundle option reads hash-pinned
Java 26.2 artwork from a local archive as data only; no game asset is bundled and
the archive is never added to the class path. Explicit live or recorded samples
now reach staged wing models and the optional ARCGEO10 native preparation path,
which retains separate per-model uniforms and joint draw order. Its actual
native input producer/insertion callsite and changing replay clock/camera
reconstruction remain unfinished; complete software/native fidelity is unverified.
This does not establish live or deterministic-export parity. Absent or rejected artwork remains
unavailable. These changes have not been built or tested; Bedrock was not opened.

The static indexed-video port adds `bedrock::capture::control` and
`arctic bedrock native-capture --bundle BUNDLE --capture-id ID OPERATION.json`.
Replay/export clock acquisition, follow-ups and cleanup now carry an expected
clock identity which the native consumer rechecks before applying a command.
Cleanup releases that acknowledged state or observes that the same identity is
already idle; changed ownership rejects the request. These guarded paths need
an updated native bundle and remain unbuilt and untested.
The indexed-export source now also requires the camera pre-render timing output
from the same guarded native producer frame and acknowledged held-clock sample.
That optional fence is checked during capture, receipt writing and paired
comparison; older captures without it retain their previous interpretation.
Metadata has a 64 KiB admission bound for the expanded receipts. This path has
not been built or exercised, and establishes no deterministic-video parity.
Use a fresh capture ID for each clip and the same ID for its later operations.
The JSON `op` values are `start`, `observe`, `sample`, `stop` and `finish`.
`start` takes the live native `session`, FFmpeg/output paths, FPS and frame count;
`clean_view` defaults to true and `settle_frames` to three. `observe` returns a
recent tagged presentation without GPU readback or encoding. `sample` takes the
next frame `index` and the actual world's `min_sequence`, `ticks`, `partial`,
`clock_ms` and six-value `camera`; optional `frames` requests 1–120 fresh
presentations of that same native sample. `finish` takes the expected `frames`
count and waits for encoder completion. `stop` aborts an incomplete indexed clip.

Capture commands and frame manifests carry capture/request identities. The
adapter refuses a later sample/stop from another capture, including unscoped
legacy commands while a scoped clip is active. A sample receipt means matching
presented pixels entered the encoder queue; successful `finish` also requires
the encoder's frame count and the output/manifest files. The existing tick,
camera, world-sequence, clean-view, pixel-settling and DX12 buffer-coverage
checks remain in place. This API neither drives a replay timeline nor asserts
terrain readiness or repeatable moving-video output. The launcher MP4 button
still uses real-time capture. This is source-only and has not been built or run.

The encoder source now validates pixel sizes, ordered single-frame indexed
batches and the requested total before accepting completion. Frame manifest rows
are written after their RGBA bytes reach the encoder pipe, with rational frame
time, 100 ns timestamps and the requested output color profile. This records pipe
input, with `decodedFrameVerified: false`; it does not attest decoded MP4 pixels.
Completion also checks manifest-row count, encoder exit and a nonempty video,
syncing the manifest and the originally created output handle. The host waits up
to 90 seconds for finalization, allowing the native 60-second idle watchdog to
report its actual result. No encoder, helper, build, test or Bedrock launch ran.

Indexed samples can now include a `controls` requirement with `clock` and
`camera` objects, each containing the operation's `sequence` and `requestId`.
`realTimer` defaults to true. The native world pass must retain the same completed
held-clock receipt across rendering, and its actual producer frame must have a
matching restored camera consumer. Queued commands, expired clock leases,
incomplete ticks and ambiguous camera operations cannot supply those receipts.
A bounded history retains camera receipts for deferred native submissions.

`ControlFence::from_receipts` forms the expected identities from acknowledged
clock/camera operations. `IndexedSample::from_observation` requires them in a
recent presented-world tag and requests a later world sequence at that same
tick/fraction/camera. The requirement is checked before GPU readback and again
before queueing pixels, and the actual receipts remain in the frame manifest.
These source changes have not been built or tested. They do not prove replay
packet application, terrain readiness, complete cosmetics or deterministic video.

The static input port now supplies `localMoving()` from the game's processed
local movement component. The previous default was always false, so Arctic's
normal walk/jump/sneak emote cancellation never ran on Bedrock. The new source
copies directional input, analog movement and jump/sneak state with entity and
component revalidation. It does not infer input from velocity or hard-coded
physical keys. Retained world state during a render/depth gap cannot supply
stale input to the cancellation check.

The component hash, 100-byte stride, movement axes and jump/sneak accesses were
corroborated in previously copied sections of the exact installed executable,
alongside public SDK declarations. No new game process was needed for that
static inspection. Keyboard remapping/controller behavior and the complete
emote cancellation path have not been validated in-game; no tests or builds
were performed, and full feature parity remains unfinished.

The static packet port adds an after-return observer for the exact client's
`network_stack_latency` dispatcher. Existing copied executable sections identify
its factory case, packet vtable/ID, dispatcher singleton and callback thunk. The
hook checks those bindings, copies only the timestamp/response flag while the
native shared pointer is still borrowed, and forwards all original arguments
once. It records returns on the thread established by completed primary-client
ticks, with matching native world sessions. It does not create packets or alter
network callbacks, packet contents or transport behavior.

The engine report's `nativePacket` section keeps bounded diagnostic receipts
and separate call/return/rejection counts. A dispatcher return is not proof that
earlier asynchronous chunk work or terrain finished. Replay steps still report
packet queue progress only; packet application and deterministic video flags
remain false. This hook is source-only, without builds, tests or game launches.

Replay steps can now carry an optional unique `marker` timestamp. After packet
and camera catch-up, the helper queues a fresh connection-local latency probe
and reports `markerQueued`/`markerTimestamp`. Native-marker steps require one
initialized viewer and remain bound to that original connection until queue
completion; plain replay steps retain their existing event format.

`PacketArm::new`, `bedrock::packet::arm` and `bedrock::packet::wait` provide the
matching native request path. `PacketArm::replay_command` constructs the exact
helper step after arming succeeds. The `bedrock native-packet` CLI reads arm
JSON containing `session`, `sequence`, `generation`, `replaySequence` and
`timeMs`; it fills missing `requestId` and `marker` and returns the populated
request plus helper command. `--wait` requires that same populated request.
The caller must permit an actual client tick after the marker dispatch.

Native requests expire, reject reused markers and match the world session,
request/replay identities and native observation counter. Completion requires
the marker's primary-thread dispatcher return and a later completed primary
client tick. `ControlFence::with_packet` can require that receipt in an indexed
frame in the receipt's native session at or after its completion tick. The
native report refreshes every 500 ms, and host waits reject reports older than
two seconds so a stalled worker cannot reuse an old live-context receipt.
Callback/tick correlation remains
separate from asynchronous chunk application and terrain readiness. No builds,
tests or game launches were performed; full replay parity remains unfinished.

The source export controller in `bedrock::export_clock` now derives native
ticks and interpolation fractions from integer frame indices. At 60 FPS and
20 TPS, adjacent frames can share a completed tick; at lower output rates it
requests every intermediate tick rather than skipping simulation. Each request
must retain the original native session, freeze identity, rates and exact tick.
Packet feeders receive ordered callbacks before ticks and fractional frames.
Those callbacks still need real packet-application/terrain barriers; queue
completion alone cannot provide them.

`NativeExportClock::capture_frame` connects these clock receipts to the scoped
native camera and the existing indexed encoder. It renews the held clock,
requires matching camera/clock identities and tick rate in a later presented
world layer, and requests the next indexed frame. Failed feeds or captures
disable the controller to prevent replaying a partially completed frame.
The caller owns the already-started capture, native camera targets and explicit
camera/clock cleanup; `resume` remains available after failure. This is a core
worker API, not yet the launcher's integrated replay-export action. No builds,
tests, helpers or game launches were performed for this source port, and it
does not establish deterministic rendered video or full feature parity.

Replay helper commands now accept an optional `requestId`. Such commands emit
a separate `replay_control` response containing the completed operation's
result and a fresh state snapshot; ordinary commands keep their existing
events. Correlated failures carry the same identity. The Rust
`OfflineConnector::request_control` worker API waits for that reply without
using coalesced periodic status as confirmation, preserves unrelated lifecycle
events, and bounds both waiting and deferred-event storage.

`bedrock::export_feed::ReplayQueue` pauses the existing playback connection and
binds to its one initialized viewer/player/generation. It feeds exact rational
timeline points from `export_clock`, resumes the existing replay step sequence,
and requires matching queue receipts and fresh viewer identity after each step.
It also configures the editor's existing camera path through Bedrock's camera
packets, so the engine supplies its own orientation. A failure disables that
feeder; resets/reconnects require reacquiring it. This is transport integration,
not a native application or terrain fence, and the launcher's integrated export
action is still unfinished. These changes are source-only, with no builds,
tests, helper execution or Bedrock launches.

Native marker waiting is split into `packet::wait_dispatch` (also CLI
`native-packet --wait-dispatch`) and `packet::wait` (`--wait`). The first waits
for the matching primary-thread dispatcher return without advancing time;
the second requires its later completed client tick. `NativeExportClock`
emits an `AfterTick` callback only after that planned tick is acknowledged.
`ReplayQueue::confirm` checks the live viewer and queue without feeding packets
again; `queue` rejects `AfterTick` points.

`export_feed::NativeMarkerFeed` connects these phases. Before a tick it arms,
queues and waits for the exact marker at the previous held tick. Afterwards it
requires the same dispatcher observation to complete on exactly the planned
next tick, then rechecks viewer ownership. Fractional frames retain queue-only
feeding and never add a tick for a marker. Their application barrier, plus
asynchronous terrain completion, remain unfinished. Whether the installed
client can deliver the required primary-thread dispatch while frozen is also
unverified; the source path times out instead of weakening that requirement.
No builds, tests, helpers, packet injections or game launches were performed.

Export steps can now provide `cameraTime: {remainder, denominator}` relative to
their integer `timeMs`. The helper feeds recorded packets only through that
integer timestamp, samples the editor camera at the full rational time, and
requires that camera instruction to be queued before returning the step.
Only these steps gain `cameraTime`/`cameraQueued` receipt fields; recorded-camera
mode reports `cameraQueued: false`. The core feeder checks the exact rational
receipt as well as queue/viewer ownership. Baked `.abr` camera poses also use
the exact frame-index time, while their packet records keep integer timestamps.

Correlated state replies include each viewer's actual packet/camera target.
When attaching between milliseconds, `ReplayQueue` aligns forward to the next
integer origin instead of requesting a backward packet step. Callers use the
resulting `position_ms()` as their export timeline origin. These are source
ports only; no tests, builds, generated exports, helpers or game launches were
performed, and native application/terrain/full-parity verification remains open.

Editor step receipts now also expose `cameraPose`, using the same normalized
yaw and float32 values sent in the camera packet. The core feeder retains it
as a requested view, never as proof that the engine applied the instruction.
`ControlFence::from_presented` can instead require that view in a real presented
world layer, alongside the acknowledged held clock. Its camera JSON is
`{session, pose: [x,y,z,yaw,pitch,fov]}`; existing `{sequence,requestId}` camera
requirements retain their scoped-consumer/restoration checks. The two forms
are distinct and require a valid camera target or command identity.

After `advance_frame` and its feeder succeed, `capture_protocol_frame` accepts
that latest frame receipt and the queued pose, renews the native clock, waits
for the actual presented camera/tick/fraction/rate to match, and queues the
indexed GPU frame. It neither patches a camera component nor advances another
tick. Duplicate/stale frame receipts are rejected. Frame manifests retain
`requiredControls` beside the actual world camera and GPU tag. This core path
is source-only and is not yet the launcher's complete replay-export action;
no builds, tests, exports, helpers or Bedrock launches were performed.

`bedrock::export_job::run` now composes the source APIs into an owned indexed
video job over the existing replay connection and already-loaded client. It
validates new output paths/range, reserves capture before changing playback,
reads fresh native sequence counters, aligns the replay origin, applies the
editor path, freezes both timers and feeds/captures each frame. Each frame uses
a unique dispatch-only native marker at its held tick, including frame zero
and fractional frames; actual simulation ticks retain their stronger post-tick
markers. Dispatch-only completion never asserts a later tick or chunk readiness.
Capture controls record which marker stage they require.

The running `bedrock replay` CLI session accepts a launcher-side JSON command
with `op: "native_export"` and an `options` object containing `bundle`, `ffmpeg`,
`output`, `fps`, `frames`, `cameraPath`, and optional `settleFrames` (default 3).
`cameraPath` is the saved editor path object. The source job reports progress,
waits for exact encoder frame/output/manifest completion, then clears its owned
camera and clock. `cancel export` or `stop` sets cooperative cancellation even
while the CLI worker is occupied; bounded native waits still apply. Failures
and unwinds attempt capture/camera/clock cleanup, retain any created output,
and keep the existing helper available for inspection/reset. The native adapter
reports command counters and process-wide marker usage so oversized jobs can
fail before consuming the remaining budget. No game/helper launch or account
action is part of this job.

This is source-only integration. The replay world lifecycle,
complete packet application/terrain barriers, audio export and full
cosmetic fidelity remain unfinished. No tests, builds, jobs, exports, helpers or
Bedrock launches were run, and native-world/deterministic-video flags remain false.

The replay editor now wires **Export MP4 (preview)…** to that same source job.
It takes a snapshot of the current path, FPS, frame count and optional recorded
appearance file and transfers the
existing connection to a worker while choosing files/exporting. The worker
returns the connection after completion, cancellation or failure. Progress uses
queued frame counts and encoder stages; it is distinct from real-time Record
MP4. Only one initialized, caught-up viewer can enable the action.

The UI retains a UUID and one coalesced progress slot per export, ignores stale
return events and preserves any newer connection. Cancel export requests bounded cooperative
cleanup; Stop local session waits for cleanup and then stops that helper on a
worker. New session/load/record actions stay unavailable while that connection
is owned by export, and launcher exit signals cancellation. No GUI, dialogs,
tests, builds, helper, export or Bedrock launch was executed for this wiring.
Full native-world/cosmetic/replay parity remains unfinished and unverified.

On Windows x64 with Rust/MSVC and a JDK:

```text
python mod/versions/bedrock/build.py --java-home "C:\Program Files\Java\jdk-26.0.1" --game-version 1.26.52 --smoke
```

This builds `dist/arctic_bedrock.dll`, `dist/arctic-bedrock-loader.exe`, the shared-core
`arctic-bedrock.jar`, and a machine-local configuration. The smoke check injects into
a disposable hidden DirectX 11 host. It submits a fixture skin/model through the
exported engine API, verifies visible model pixels and terrain occlusion on the GPU
render target, clears the world, and verifies the shared menu. This checks the
rendering bridge; the fixture does not run inside Minecraft.
`--renderer dx11` selects DX11; otherwise the bundle selects DX12. The pinned DX12
installed game has completed isolated world tests with live world/actor/tick
hooks and matching GPU depth delivered to the shared renderer. Full cosmetics
parity remains unfinished; other game builds require their own bindings.
`--offline-name ArcticGuest` chooses the local Arctic profile (default `ArcticGuest`).
It uses a Java-compatible offline UUID and never requests a Mojang session proof.
This profile does not fabricate an in-world player or change the game's own account.
All smoke checks use an isolated `ArcticTest` offline profile and headless rendering;
the native host also redirects cosmetics requests to loopback and uses a temporary
config directory without saved launcher sessions. No Minecraft gameplay is started.

Use **Bedrock → Arctic Client → Load a native client bundle**, choosing `dist`, or:

```text
arctic bedrock play --native-client mod/versions/bedrock/dist
```

Right Shift opens the reused shared client menu. The native loader verifies a selected
process and the launcher waits for a first-render marker instead of treating a loaded
DLL as successful initialization. Duplicate loads are rejected immediately. The embedded JVM lives until process exit; restart
the game before rebuilding or reloading its native bundle.

The source adds an opt-in **Mods → Bedrock Cosmetics** setting for experimental
body, cape, wing and emote rendering in a normal client session. It defaults off
(`bedrockNativeCosmetics` in `config/arctic.json`). Its settings also expose cape
animation and the shared emote-wheel key/hold controls. Enabling it requests the
existing checked native rendering path; an unavailable model, pose, texture or
draw binding still cannot supply replacement readiness. Automatic third-person
emote camera control remains unavailable on this adapter.

The render-thread bridge sends the actual native enable state back to Java.
Turning it off clears replacement readiness and new emote programs while keeping
textures needed by an already acknowledged suppressed frame until it retires.
Source rendering now also keeps a bounded, read-only animation cache for buffered
emote receipts. Bodies, capes and worn cosmetics use the six poses carried by the
native frame; wings keep their independent model/controller pose instead of
being reparented through that body's emote transform.
New menu emotes wait for a matching native target. Retained animation data can
render an acknowledged older frame but cannot authorize new native callbacks.
Export still waits for the currently requested emote, including its stop.
This synchronization work has not been built or tested; GPU fidelity and full
emote/equipment parity remain unverified.
Isolated probe runs continue to use their explicit body/elytra probe settings.
The normal setting does not enable probe clock, camera or terrain-animation
controls. This source wiring requires the matching Java/native bundle; no bundle
was rebuilt, tests run or game opened for it. Full rendered parity and deterministic
video remain unverified.

| Feature | Current state |
| --- | --- |
| Shared menu and HUD rasterization, input dispatch | Implemented; software and D3D11 host tests pass |
| FPS, click counts, default physical key states | Native render/input data available |
| Settings and HUD placement | Shared Java core reused |
| Skin/cape textures and player previews | Software triangle backend; classic/slim skins and cape frames tested headlessly |
| Cuboid/glTF cosmetics, glow, fits, expressive/split-limb emotes | Shared model backend implemented; diagnostic cuboid head attachment observed in the installed world; other assets/emotes unverified |
| Camera/depth/actor snapshot input | Exact-build producer, native identity/timer and per-view DX12 bindings; 102 new frame deliveries over ten seconds and 590 native pool handoffs verified in an installed-game run without exhaustion |
| Local Arctic profile without Xbox/Mojang credentials | Implemented; Java-compatible offline identity |
| Bedrock entity state and identity mapping | Checked player-list/component readers verified with the installed game's isolated offline player |
| Skins, capes, cosmetics and emotes on world players | Standing diagnostic head attachment verified against actual GPU pixels; body/cape replacement, full pose coverage and other cosmetics/emotes remain unfinished |
| Cape physics | Tick-driven Java filter implemented; two animated cape phases observed in the installed world with an empty chest and absent native cape; moving physics, full alignment and native cape replacement unverified |
| Equipment/elytra rendering | Cape suppression implemented; equipment models and attachment pending |
| Native camera controls, minimap and game HUD hooks | Pending |
| Native timer freeze, single ticks and fractional samples | Exact-build isolated probe verified in the installed client; integrated-server/replay stepping pending |
| Offline relay recording and packet playback | Implemented in launcher/CLI; encrypted offline network tests; stock-client gameplay unverified |
| Replay camera/keyframe editor and `.abr` camera export | Launcher + public packet route implemented; headless wire tests; stock-client gameplay unverified |
| Real-time MP4/H.264 recording of presented pixels | DX11/DX12 implemented; both hidden GPU hosts encode/decode clips; installed-client use unverified |
| Installed image inspection and native observers | Package-context inspection and exact-hash world/actor/tick hooks verified in automated installed-game runs |
| Native Java replay backend, deterministic replay export and singleplayer recording | Pending |
| Shared Arctic account integration | Pending |

The renderer input contract and remaining engine requirements are documented in
[`native/ENGINE.md`](native/ENGINE.md). Deterministic cape/mesh clocks are available
for animation sampling. Native replay bindings, deterministic video export and direct singleplayer
recording remain unfinished; offline relay recording/playback is described above.

### MP4 recording

With the updated native bundle already loaded, **Arctic Client → Record clip…**
selects that bundle and a new MP4 filename. The source prefers Arctic's built-in
Windows encoder when available, with a local FFmpeg executable as fallback. The replay
editor also has **Record MP4…**. Recording uses the currently presented game and
Arctic overlay in real time, so start replay playback before recording the desired
segment. This is not the Java replay exporter's deterministic simulation/camera clock.

```text
arctic bedrock capture --bundle mod/versions/bedrock/dist --ffmpeg "C:\tools\ffmpeg.exe" clip.mp4 --seconds 10 --fps 30
python mod/versions/bedrock/build.py --java-home "C:\Program Files\Java\jdk-26.0.1" --game-version 1.26.52 --capture-smoke
```

The native adapter copies actual DX11/DX12 back-buffer pixels after overlay
submission and encodes H.264 in an MP4 container, without audio. It supports SDR
RGBA/BGRA buffers up to 3840×2160, 1–60 FPS and clips up to two hours. Existing
files are refused. Resize/HDR formats, prolonged rendering stalls or encoder
overload stop the capture with an error; partial files are retained. Short missing
real-time samples repeat the previous captured frame. Encoding uses a bounded
64 MiB queue outside the render thread and a ten-second encoder watchdog.

Both encoder source paths now explicitly request limited-range BT.709 with
centered progressive 4:2:0 chroma and black padding on odd source dimensions.
FFmpeg's conversion/metadata and Media Foundation's chroma-siting attributes are
set explicitly; their implementations can still produce different compressed
or decoded frames. These source changes have not been built or exercised, and
do not establish native replay or decoded-video determinism.

`--capture-smoke` uses isolated `ArcticTest` configs with public cosmetics requests
redirected to loopback. The hidden DX11 host encodes/decodes eight full-size frames
and verifies skin/cape pixels in the video. The hidden DX12 host verifies eight
BGRA frames with padded GPU rows and color checks after decoding. No Minecraft
gameplay or saved account is started. FFmpeg must be on PATH or supplied with
`--ffmpeg`; optional `--ffprobe` adds codec/dimension/frame-count metadata checks.

### Inspecting the installed build without starting it

```powershell
mod/versions/bedrock/native/inspect-installed.ps1
```

The windowless probe runs under Windows'
[package-context diagnostic API](https://learn.microsoft.com/en-us/powershell/module/appx/invoke-commandindesktoppackage).
This can read the installed protected executable where an ordinary file read fails.
It reads executable/RTTI sections into ignored local `target/bedrock-reference/`
and reports hashes, PE metadata and unique signature candidates. It does not execute
Minecraft code, read account data, or patch the installation. This resolved access
for installed 1.26.5203.0; live native identities and classic skin pixels have
since been observed. Matching terrain depth now reaches the shared renderer;
full pose coverage and complete cosmetics still require validation. A unique
signature does not establish Java parity.

The DLL installs pass-through world/actor/tick observers for that exact image
hash. Its bounded `arctic-bedrock-PID.engine.json` diagnostics include native
UUIDs, component samples, camera/timer data, and two post-render animation bone
samples. The standing producer can map six native attachment matrices when its
rig and head-look checks pass, and requires matching scoped DX12 depth before
publishing a render frame. Other body states, native renderer suppression and
visible installed-world alignment remain unfinished. Native detour tests run in
a headless fixture.
The windowless `inspect_debug_chain` example reaches the actual installed
process under package identity and validates its private startup desktop. Its
default mode stops at process creation. `native/inspect-initialization.ps1`
additionally advances the system loader to an entry-point trap, holds that
thread, and verifies Arctic's JVM and all three engine detour installations.
It then terminates only its own debuggee; it does not execute the EXE entry
point or claim an offline world test. Full live-world behavior remains unverified.
`-IsolateProfile` checks the optional fresh-profile/network/GDK-user guards while
holding entry. `-IsolatedBoot -BootSeconds 180` then resumes on the unshown
desktop for a bounded observation, using fresh `ArcticTest` game options.
It records GPU startup frames and cleans up only the owned process tree.
The installed game has reached its signed-out ArcticTest menus and local-world
loading screen with actual GDK user requests denied. A four-frame H.264 MP4 was
encoded and decoded successfully from its real swap-chain pixels. A later isolated
loopback/empty-user run reached real world and actor rendering, the primary-client
clock, and the ArcticTest player directory after correcting an NSI enumeration
denial. The weak entity reader now resolves the native local-player UUID and
third-person actor position/rotation/bounds. The classic skin reader now copies
the installed client's real 64x64 RGBA pixels, including its zero-depth 2D image
layout. Skeletal observations preserve unresolved native parent links explicitly;
the standing adapter uses the completed cached matrices independently of those
links and has mapped all six parts in an installed-game offline run. Preserved
GPU depth has reached the shared renderer in automatic tests; full cosmetics
binding and deterministic video remain unfinished.
`-AutoWorld -Loopback -EmptyUserProvider -SeedProbe COMPLETED_PROBE` automates
menu navigation and saves live callback/frame evidence; it requires `-IsolatedBoot`
and an existing isolated test world. These modes do not alter licensing.
`native/test-world.ps1` selects a completed isolated test seed and performs the
build, hooks, navigation and capture automatically, using a release build by
default and keeping the game muted on its private desktop.
For the complete current automated movement/cape check, run:

```powershell
mod/versions/bedrock/native/test-world.ps1 -MovementProbe
```

For all current native camera, clock, body, emote, cape and movement diagnostics:

```powershell
mod/versions/bedrock/native/test-world.ps1 -CameraProbe
```

This command also checks the camera component's registry membership in three
perspectives, restores the starting view, freezes/steps both native timers and
compares captured frames. It saves `camera-evidence.json`, `clock-evidence.json`
and marked GPU images inside the printed probe folder. Camera control and
complete replay/export parity remain unfinished.

`-CameraControlProbe` additionally tests native position, quaternion and FOV
changes against the rendered camera and marked GPU pixels. It checks player
state and camera restoration, with evidence in `camera-control-evidence.json`.
This remains experimental and reports failure when component writes do not
reach the native world view.

`-IndexedExportProbe -HeadlessPresent -Ffmpeg "C:\tools\ffmpeg.exe"` additionally encodes two
eight-frame native camera paths with the world frozen. Every requested frame
waits for matching native timing and camera metadata; manifests retain raw GPU
hashes, and the test decodes both MP4s and compares the frames exactly. It restores
the camera, timers and native HUD on failure. This is an export diagnostic;
native replay playback/reset and full deterministic replay parity remain pending.
Installed-game tests now encode and decode these indexed MP4s successfully.
Exact repeatability still fails on native water pixels even when camera matrices,
ticks and interpolation match. The test retains both clips and their frame
manifests and reports this failure explicitly.
Indexed capture now waits for identical fresh images covering every DX12 buffer.
The installed game completed both paths with this check, but five of eight paired
frames still differed; this wait has not fixed repeatability. Native registry
generation now preserves the world session across stale metadata refreshes while
retaining the sample freshness limits.
`-HeadlessPresent` opts into the isolated presentation adapter to avoid throttling
on an unshown desktop. Actual presentation status and device failures remain in
the native diagnostics; frame capture still reads real GPU pixels.

The controller selects the isolated test seed, enters the world, switches camera,
holds and releases movement keys, and checks actual native movement and matching
GPU frames. No manual keypresses or desktop navigation are required. It prints
state changes with elapsed time and input counts; `auto-world.json` retains
per-state timings. Menu evidence uses the same frame that selected the next
scripted action. Unknown menus stop input and fail with a captured frame.
Pending menu captures use the existing navigation-stage deadline, so a slow
transition does not trigger another key or abort after the shorter snapshot wait.
Add `-CosmeticsProbe` to run a fast headless Java HTTP preflight and then test
a diagnostic hat through the normal asset service in the installed world. The
script checks the world layer against a real GPU screenshot and saves evidence;
this attachment check does not establish complete cosmetic parity.
`-CapeProbe` additionally verifies two animated cape phases against actual GPU
pixels. It currently requires a standing classic player with no native cape and
an empty chest slot; full pose/armor coverage and native cape replacement remain
unfinished.
`-TransformProbe` also requires an owned matrix from the native model draw,
after resource scripts and model scale have been evaluated. The exact-image
observers are installed automatically; no late hook file or manual navigation
is needed. The fresh test-world clone is made peaceful and creative before
world entry. Observation alone does not establish alignment or full parity.
`-MovementProbe` includes the cape and transform checks, then sends bounded
forward/sideways input to the owned private window. It requires observed native
player displacement and nonzero cape input; a queued key alone cannot pass.
Moving cape rendering and complete physics parity are separate checks.
GPU checks now require a marker carried with the exact rendered overlay pixels
through native texture upload and post-render capture. The movement probe also
requires matching moving cape pixels and native clock advancement during input.
Camera `.abr` exports preserve source metadata and derive each
sample timestamp from its integer frame index, producing byte-identical files for
the same recording, path and sample rate. This reproducibility does not extend to
rendered MP4 frames or the game's simulation clock.

`-BodyProbe` remains experimental. Skipping the root skin method on the CPU
failed because those frames did not deliver the scoped depth needed by the
replacement. The current probe forwards that method and substitutes collapsed
bone matrices in its owned skinned draw description. Only exact native root/color matches and
a loaded replacement skin can enable this change; ordinary launches pass through.
Its report must not be interpreted as all-pose body rendering or full parity.
An installed-game run has now displayed the diagnostic replacement skin in its
exact tagged GPU frame with movement/cape checks passing. Texture readiness is
retained during a live-world depth gap; geometry still requires a fresh matched
frame. Original-body pixel/depth absence remains a separate check.
`-BodyAbsenceProbe` additionally switches the private JVM between native-only,
replacement and suppression-only modes and compares matching GPU/depth captures.
The alpha-only experiment left the original body visible. The current experiment
retains native bounds and draw parameters while collapsing the borrowed skin
matrices. The installed-game comparison passed for the tested standing pose:
45,204 of 45,818 masked depth samples moved farther, none moved nearer, and
171,175 of 182,932 masked GPU pixels changed. Both the empty native-body capture
and the replacement-skin capture were inspected. Broader pose coverage remains pending.
These controls accept only fixed modes and suppression methods, with alpha values from 0 through 0.01;
ordinary launches do not enable them. The comparison is a bounded diagnostic,
and retains the all-pose/parity verification flags as false.
`-EmoteProbe` includes that comparison, triggers the catalogued local diagnostic
emote through Arctic's normal playback method, and requires two different arm
poses to reach their matching GPU frames as the native clock advances. It stops
the emote before running the movement check. The same script handles world
selection, input, captures and process cleanup on the muted private desktop.
An installed-game EmoteProbe run passed the body comparison, two visibly
different arm poses in exact GPU captures, emote stop, animated cape and movement
checks. This demonstrates the diagnostic rig; all-pose fidelity and native
deterministic replay video remain unfinished.
`-ClockProbe` includes those checks and exercises the installed client's native
timer stepping: freeze, individual ticks, fractional interpolation and resume.
It compares completed native ticks and world-clock samples after each command;
the command acknowledgement alone cannot pass the check. Integrated-server
stepping and deterministic replay video remain separate unfinished work.
The installed-game clock test now also confirms byte-identical cosmetic layers,
including cape state, across a paused native tick. Full native GPU frames still
change while the client timer is frozen, so deterministic video remains unverified.
`-RenderClockProbe` additionally binds the renderer's separate wall clock to the
native sample and requires matching cropped native GPU frames, excluding HUD
edges. Its world snapshots record the actual renderer-clock values. This is a
strict diagnostic; replay reset/camera bindings and full video export remain pending.
An installed-game RenderClockProbe run now passes with coordinated client,
real-timer and renderer-clock control: frozen native world crops are byte-identical.
The only differences across the complete screenshots were FPS-counter digits.
This verifies the sampled scene; replay reset repeatability and deterministic
MP4 export are still unfinished.

The native DLL uses a vendored MIT-licensed [hudhook](https://github.com/veeenu/hudhook)
0.9.3 with documented post-render callbacks for frame capture, and the
JNI crate. The Java bundle contains the existing Arctic core, Gson and Netty. All
dependency license metadata is retained in the assembled jar and Cargo dependencies.
