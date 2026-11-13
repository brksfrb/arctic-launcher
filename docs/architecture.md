# Architecture

## Principles

- **Core vs UI.** `arctic-core` holds all logic and is UI-agnostic. `arctic-app` only renders
  state and starts background jobs. A future CLI or another frontend can reuse the core.
- **Never block the UI thread.** Network and disk work runs on `std::thread`s (see
  `arctic-app/src/tasks.rs`), which report back through an `mpsc` channel of `Event`s and call
  `request_repaint()`.
- **Small and fast.** The app uses blocking `ureq`, eframe's `glow` backend instead of
  `wgpu`, and one static executable. The only async runtime is inside `arctic-share`, and it
  starts the first time you use Play together. Launch checks use file sizes rather
  than re-hashing thousands of assets, and downloads are SHA-1 verified as they stream.
- **No secrets in git.** Tokens live only in `accounts.json` in the local data dir. The Azure
  client ID comes from env or local config.

## Launch pipeline (`arctic-core::launch`)

1. `versions::load_version` downloads `<id>.json` and verifies it against the manifest's SHA-1.
2. `java::ensure_runtime` picks `javaVersion.component` (or `jre-legacy` for old versions),
   installs it from Mojang's runtime index into `runtimes/<component>/`, and records the
   manifest SHA-1 in a marker file so later launches do nothing.
3. `launch::files` resolves libraries by evaluating the rules (`versions::rules`), plus the
   client jar, the log4j config, legacy natives (extracted with zip-slip protection) and assets.
   Pre-1.7 `virtual` and pre-1.6 `map_to_resources` asset layouts are handled too.
4. `launch::args` builds the command line. It supports the modern `arguments` object with
   rules/features and the legacy `minecraftArguments` string, substituting `${placeholders}`.
5. `launch::spawn` starts `javaw.exe` with the instance's game dir as the working directory.
   Output goes to `logs/game-<instance>.log`, and the access token is redacted from logs.

### Downloader (`net::download_all`)

- All missing files of a launch (Java runtime, libraries, client jar, assets, log config) go
  into **one queue**, deduplicated by destination.
- **64 keep-alive connections.** ureq's default pool keeps only 3 idle connections per host,
  which forced a TLS handshake on most requests, so the pool is sized to the worker count.
- **Two-ended queue:** 8 workers take the largest files and the rest take the smallest, so
  bandwidth-bound and latency-bound work overlap. Total time is roughly
  max(bytes / bandwidth, files / request rate).
- Java runtime files use Mojang's **LZMA** streams, decompressed while downloading. Both the
  compressed and the final SHA-1 are verified.
- Timeouts are per phase (connect, response, then a body limit scaled to file size), so large
  files on slow connections are not cut off. Each file is **retried 3×** before the launch
  fails.
- Progress (files and bytes) is reported at most every 50 ms. The UI turns it into speed and
  ETA.

Measured on a ~50 MB/s line for a fresh 26.3 install (663 MB, 5.6k files): 39 s → 18 s.
Relaunching an installed version prepares in about 0.2 s.

### Game process (`launch::process`)

The game's stdout/stderr are piped through two threads that write the log file and watch for
the "window is up" line (`Backend library:` / `LWJGL Version:`). A third thread waits for the
exit and handles Force close. The UI only receives `GameEvent`s and never blocks on the
process. After a successful prepare, `versions/<id>/.installed` marks the version as
installed for the version picker.

### Logs

`launch::logparse` turns game output into `LogLine`s. Modern versions print log4j XML
events because of the official logging config; these are collapsed into
`[thread/LEVEL] message` lines, with stack traces kept. Parsing runs on the tee threads,
which stream `GameEvent::Output` to the UI (capped at 20k lines). The launcher's own
`log` output goes to stderr, `logs/launcher.log` and an in-memory ring buffer
(`arctic-app/src/logbook.rs`). Both are shown on the Logs tab.

## UI notes

- **Title bar:** on Windows 11 the native caption, border and text colors are set via DWM
  (`titlebar.rs`) to match the theme. The native bar keeps snapping, resizing and shadows.
- **Icons:** `icon_raster.rs` is a dependency-free rasterizer shared by the app (window
  icon) and `build.rs`, which generates the multi-size `.ico` and embeds it with
  `winresource`. No binary assets are checked in.
- **Instance icons:** `InstanceIcon { style: FlakeStyle, color }` is stored per instance.
  Loaders get a default style (`Loader::default_icon`), and users can override both.

## Accounts (`arctic-core::auth`)

`AccountStore` holds `Vec<Account>` plus the active id. Each `Account` holds a
`MicrosoftSession`: MSA refresh token, Minecraft access token, expiry, xuid.

The Microsoft chain is MSA OAuth → Xbox Live → XSTS → `login_with_xbox` → profile, and it
has two entry points:

- **Browser**: authorization code + PKCE, using a loopback redirect on an ephemeral port
  (`http://localhost:<port>`, listening on both 127.0.0.1 and ::1).
- **Device code**: shows a code, polls the token endpoint and honours `slow_down`.

Tokens are refreshed before launch when they are within 5 minutes of expiry.
TODO: encrypt `accounts.json` at rest with Windows DPAPI.

## Crates

| Crate | What it is |
|---|---|
| `arctic-core` | Everything that isn't UI: versions, Java, launching, loaders, mods, skins, accounts |
| `arctic-app` | The egui launcher |
| `arctic-cli` | The `arctic` command |
| `arctic-share` | Play together: peer-to-peer LAN tunnels (iroh) |
| `mod/` | The Arctic Client (Java, Gradle): a shared core plus per-version adapters, bundled into the launcher as jars |

## Instances and mod loaders (`instances`, `loaders`)

`instances/<id>/instance.json` + `instances/<id>/minecraft/` (game dir). Libraries, assets,
versions and runtimes live in `shared/` and `runtimes/`, so instances stay small.

For a loader instance, `launch::prepare` installs vanilla first, then asks
`loaders::install_profile` for the loader's version JSON and merges it onto vanilla
(`versions::merge`: the loader's libraries win over vanilla ones with the same
`group:artifact`, arguments are appended).

- **Fabric / Quilt:** one request to their meta servers for the profile.
- **NeoForge / Forge (1.13+):** the official installer jar is downloaded (and cached), the
  libraries bundled in it are extracted, and its client processors run with the instance's
  Java. A marker in `meta/loaders/` records the result, so later launches need no network.
- **Forge 1.7.10–1.12.2:** the legacy installer format; the universal jar is extracted and
  referenced directly.

## Mods (`mods`)

Modrinth search and install. Installing resolves the whole dependency tree before
downloading anything, so a missing dependency changes nothing on disk. Installed mods are
tracked in `instances/<id>/mods.json` (project, version, title, icon); jars added by hand are
listed too. Disabling a mod renames it to `*.jar.disabled`.

## Play together (`arctic-share`)

The host's invite code is its iroh endpoint id. The host listens for Minecraft's "Open to
LAN" multicast announcements (224.0.2.60:4445) to find the local world's port. A guest
connects over QUIC (hole punching, relay fallback), opens a local TCP port, and announces it
to games on the same PC with multicast TTL 0, so the world appears in the Multiplayer LAN
list. Each Minecraft connection is one bidirectional QUIC stream. Only connections from the
guest's own machine are accepted.

## Skins (`skins`)

A per-profile library in `profiles/<id>/skins/` (`library.json` + PNGs). Skin changes use
the Minecraft services API with the account's access token. Legacy 64×32 skins are expanded
and cleaned the same way the game does. The 3D preview (`arctic-app/src/ui/skins/model.rs`)
builds a textured mesh per body part, back-face culls and depth-sorts the faces.

## Arctic looks and the mod

A **look** is a skin (plus arm model) and a cape that the player picks locally. The launcher
publishes it to the looks server, which stores textures by SHA-1 and
relays each player's look to every Arctic client. Nothing is owned or unlocked: presets are
just textures everyone may use, and custom images are allowed (size-checked: 64×64 skins,
2:1 capes up to 512×256).

The server only checks **who** publishes, so nobody can change someone else's look:

The client asks for a challenge, calls Mojang's `session/minecraft/join` with it, and the
server confirms with `hasJoined`, like a Minecraft server does. No password or token reaches
Arctic. That gives an HMAC-signed token that expires after a week. Before launching with the
client, the launcher writes `config/arctic-session.json` (server, token, and the menu style
picked in the launcher) so cape changes made in game publish as that player.

## The Arctic Client (`mod/`)

Vanilla instances run the Arctic Client on supported versions: Fabric plus the Arctic mod,
installed quietly (`launch::effective_loader`); Fabric and Quilt instances get the mod too.
The launcher embeds `mod/dist/arctic-client.pack`: every target's jar, with each file
stored once and LZMA-compressed together (about 1 MB for all versions, against about 24 MB
of jars; most files are the same in every version). Before launch it rebuilds the jar the
game needs from it and copies it in (`arctic_mod::sync`).

The client is split so it can cover many Minecraft versions:

- `mod/core` holds everything that doesn't touch Minecraft, compiled for Java 8 so the same
  code runs down to 1.8.9: a small UI toolkit drawn from rectangles and text (`ui`, `gfx`),
  the menu styles and the widget skins (`style`), the animated Arctic backdrop, the title
  menu, the Arctic menu (Right Shift) and the HUD editor (`menu`), the HUD widgets (`hud`:
  unmoved widgets stack in a column, moved ones pin to the nearest screen edge), Zoom,
  Freelook and Fullbright (`feature`, with keys stored by Minecraft key name),
  looks lookups and cape changes over plain HTTP (`looks`), and `config/arctic.json`.
- `mod/versions/fabric` is a thin adapter, one source tree built for each target in
  `mod/targets.json` (version differences are `//#if MC …` blocks, mostly in `Compat`): a `Platform` (game state, screens, textures,
  sign-in), a `Gfx` over that version's GUI renderer, a screen that hosts core pages, and
  mixins that replace the title screen, draw the HUD, count clicks, open the menu, restyle
  vanilla buttons, sliders, fields and checkboxes, and apply Arctic skins and capes.

The Performance switch (`mods::performance`) adds well-known optimization mods to Vanilla
instances from Modrinth, tracked in `arctic-performance.json` beside the game folder so they
never mix with user mods. It rechecks for new builds weekly, keeps what's installed when
offline, and runs Fabric even on versions the client doesn't support yet.

Vanilla screens keep their own logic; only their widgets and backdrop are restyled, so
adapters stay small. The Vanilla style turns restyling off and keeps the HUD.

## Platforms

Windows and Linux (x86-64) are supported from the same code. The differences are small:
- `javaw.exe` vs `bin/java` in managed runtimes;
- runtime symlinks, which exist only on Linux;
- the classpath separator;
- the updater's per-platform release asset;
- the Windows-only title bar tint and exe icon.

Rule evaluation maps the host to Mojang's `windows`/`linux`/`osx` names.

## Bedrock Edition (`arctic-core::bedrock`)

Windows only. Since 1.21.120 the game is a GDK package (`Microsoft.MinecraftUWP` for the
release, `Microsoft.MinecraftWindowsBeta` for Preview) installed in `C:\XboxGames`; its data
is in `%APPDATA%\Minecraft Bedrock\Users\`: `Shared\games\com.mojang` for packs and one
folder per Xbox account for worlds, settings and the server list. An old UWP install's
`LocalState\games\com.mojang` is read too.

- **Finding and starting it** goes through Windows' `PackageManager` (WinRT): the package's
  version and folder, and its app list entry to launch (the `minecraft:` link is the
  fallback). "Running" is a `Minecraft.Windows.exe` from the package's folder.
- **Versions** come from community archives of Microsoft's package links
  (LiteLDev's `minecraft-windows-gdk-version-db`, then MinecraftBedrockArchiver's
  `GdkLinks`), kept in `cache/bedrock-versions.json` for offline use; only `*.xboxlive.com`
  / `.cn` `.msixvc` links are accepted. Installing hands the link to
  `PackageManager.AddPackageByUriAsync` with `ForceUpdateFromAnyVersion`: Windows downloads
  the encrypted package and unlocks it with the Store license, so nothing from the archives
  but a link to Microsoft is trusted. Package version `1.26.5203.0` is game version 1.26.52.3.
  Builds before 1.21.120 are UWP `.appx` packages known only by a Windows Update id (the
  raythnetwork list, merged into the same catalog; Beta is skipped, it needs an account).
  `bedrock::fe3` asks Microsoft's update service for the link, the package is downloaded
  into `cache/bedrock-packages` (the newest three are kept) and installed from the file; switching between the two kinds removes
  the installed package first with its data kept. A GDK install is told apart by its
  `MicrosoftGame.Config`.
- **Worlds** (`minecraftWorlds\<id>`, name in `levelname.txt`) import from and export to
  `.mcworld` (the folder's contents zipped). **Packs** are read from their `manifest.json`
  (comments allowed, names looked up in `texts/en_US.lang`); `.mcpack` / `.mcaddon` files
  are sorted into resource, behavior or skin packs by their modules, replacing a pack with the
  same UUID. Removed worlds and packs move to `com.mojang\arctic-trash`.
- **Servers** are `minecraftpe\external_servers.txt` (`id:name:host:port:time` lines) and
  are pinged with RakNet's unconnected ping over UDP (`arctic_ping::ping_bedrock`).

- **Clients** (`bedrock::client`): Bedrock has no mod loader, so a client (Latite or Flarial, chosen in
  the Bedrock tab or `settings.json`'s `bedrock_client`) is a DLL loaded into the running game, as other
  launchers do it. The DLL comes from the client's own GitHub releases (SHA-256 checked against the digest
  GitHub lists, kept in `cache/bedrock-clients`), and after Play the launcher waits for
  `Minecraft.Windows.exe`, a few seconds for its graphics, then runs `LoadLibraryW` on a thread inside it.
  GDK games only (1.21.120+). A client that can't be fetched or loaded never stops the game from starting.
  Arctic doesn't write or ship these clients; the tab warns that servers may ban modified clients.

- **Offline connector** (`bedrock::offline`, `mod/versions/bedrock/offline`) is a
  loopback-only Bedrock protocol relay for servers that accept unsigned identities.
  Both relay legs use offline mode; online/Realm authentication entry points are
  disabled in the private helper process. The chosen username is used on the upstream
  session, with Java-compatible offline UUIDs. `bedrock::appearance` decodes bounded
  PNGs into RGBA login data: skin preparation shares Java's legacy limb/opacity rules,
  while static HD capes are reduced to 64×32. The helper checks dimensions and buffers
  again before signing them and prevents downstream skin changes from replacing the
  selected appearance. Node.js 24+ and pinned npm dependencies are prepared on first use, without
  install scripts or optional native modules. A process guard stops the connector when
  its launcher session ends. This does not patch the game's sign-in UI or Store license.
- **Bedrock packet replays** record decrypted world packets and client input at the
  offline relay boundary, with directions and monotonic arrival times. Authentication
  and transport handshakes are excluded. Files use a bounded, exclusive `.abr` stream
  with a finalization marker; incomplete recordings remain on disk and are refused.
  A loopback offline server performs a fresh encrypted login, restores bootstrap and
  world updates, and translates recorded self movement into server position packets.
  Launcher/CLI controls select recording, live pause/resume, playback speed and forward
  seeking. Camera keyframes use public CameraPresets/CameraInstruction packets with
  deterministic position, shortest-yaw, pitch and FOV interpolation. Camera-path JSON
  can be saved/opened; export bakes a 1–120 Hz track (default 25) into a new `.abr`
  while preserving world packets/timing and source metadata. Integer-indexed
  timestamps and canonical headers make repeated identical exports byte-identical.
  Controls use the helper's owned bounded stdin JSON protocol;
  periodic timeline events are coalesced rather than queued while another tab is open.
  Backward seeking needs a fresh client connection and bootstrap restoration. The
  helper finishes pending writes before shutdown, including when its parent exits.
  Stock-client gameplay remains unverified. Native replay integration, deterministic
  video frame stepping and singleplayer capture remain unfinished.
- **Native client preview** (`mod/versions/bedrock`) embeds the shared Java client
  core in a JVM inside a Windows DLL. A headless Java2D `Gfx` adapter renders shared
  menus/HUD into RGBA, uploaded through hudhook's DirectX 11/12 swap-chain renderer.
  Window events feed the shared pages; each JNI callback uses a bounded local frame.
  The loader selects a PID, checks its executable, rejects duplicate loads and waits
  for a first-render marker. Native configuration is checked before launching a game.
  The local Arctic profile is explicitly offline; headless smoke tests use isolated
  config directories and loopback cosmetics endpoints without reading saved accounts.
  A software triangle backend draws the shared skin, cuboid, glTF, expressive
  emote and cape models with depth testing, alpha blending, glow and fits. Cape
  and mesh sampling also accept explicit animation clocks. A versioned C input
  API copies bounded actor/camera/depth snapshots and textures into the render
  thread; stale worlds expire after 500 ms. A limited exact-build producer now joins
  player-list/EnTT identity and rotation copies, the native timer/camera, and scoped
  legacy DX12 depth for standing head/body attachments. Unavailable limbs are masked
  out and the original body/cape is retained. Installed-client execution is unverified.
  Java 26.2 cape chasing motion, gait/flight response and model rotation are ported
  for supplied tick data, including its sine lookup behavior and elytra suppression.
  Complete actor/camera/depth binding, equipment rendering and the shared replay editor
  remain unfinished. The preview makes no Java parity claim. A disposable hidden
  D3D11 host verifies injection/JVM/menu rendering, fixture skin geometry, terrain
  occlusion, moving cape geometry and disconnect clearing through actual GPU output.
  A vendored hudhook 0.9.3 adds post-overlay DX11/DX12 callbacks. Native readback
  copies the presented back buffer, restores DX12 PRESENT state and waits for its
  direct-queue fence before mapping. A bounded encoder queue feeds external FFmpeg
  on a worker, with exclusive MP4 creation, repeated frames for short real-time gaps,
  and a watchdog. Atomic local request files and per-PID JSON reports connect GUI/CLI
  controls to the loaded adapter; rejecting another request does not abort a capture.
  Hidden DX11 and DX12 hosts verify full MP4 encode/decode round trips with offline
  configs. This records real-time presented pixels, independently of Java replay APIs.
  A separate windowless package-context probe can read the installed executable and
  confirm signature uniqueness without starting Minecraft. Live actor/camera/depth
  producer validation remains unfinished; signatures alone are not engine integration.
  Exact-hash observers now detour world/actor/tick callbacks and safely copy candidate
  camera matrices into per-PID diagnostics. ABI forwarding is tested with actual
  headless fixture detours; UUID/limb mapping and matching depth remain unbound.

### Bedrock native preview review boundary

The launcher does **not** build or ship `arctic_bedrock.dll` or
`arctic-bedrock-loader.exe`. Build the separate workspace manually using
`mod/versions/bedrock/build.py`, then explicitly choose that bundle in the tab or
use `bedrock play --native-client`. The root workspace excludes the native
workspace. Only source, vendored source and inspection data are ported; no
prebuilt DLL/EXE/SO or credentials/private keys are included.

No new injection, native hooks, game-memory access, relay, npm install or video
subprocess runs on ordinary launcher startup. After explicit native-bundle
loading, DLL initialization installs rendering/game hooks and starts observation;
individual camera/clock/capture/appearance commands then operate within that
loaded process. Inspection/probe tools below are separate manually invoked tools.
Native engine execution and visual correctness on a real installed game remain
unverified by Linux checks or Windows cross-compilation.

- **Remote injection:** `native/src/loader.rs` validates the selected PID/module,
  allocates/writes its DLL path (`WriteProcessMemory`) and invokes `LoadLibraryW`
  using `CreateRemoteThread`. The vendored `hudhook/src/inject.rs` also contains
  an injection implementation. `runtime.rs` loads a local JVM; `shader_dxc.rs`
  loads a local compiler library. These local library loads are not remote injection.
  Main's separate `crates/arctic-core/src/bedrock/client.rs` is retained: explicit
  Latite/Flarial actions download a release DLL and use its existing remote injector.
- **Game-memory writes:** `native_camera_control.rs`, `native_camera_time.rs`,
  `native_clock.rs`, `native_render_clock.rs`, `native_emote_upload.rs` and
  `native_skinned_desc.rs` call `WriteProcessMemory` on the current game process
  for camera/timer/render/animation state. `world_hooks.rs::read` centralizes
  `ReadProcessMemory` on that process. Its readers and hook installation modules
  are inventoried below. MinHook additionally patches executable code/trampolines.
  GPU buffer mapping/readback and JNI copies occur in the rendering/capture code.
- **Launcher subprocesses:** `bedrock/native.rs::launch` starts the chosen loader;
  `bedrock/offline.rs` runs `node` for version discovery and the relay/replay helper,
  and `npm ci --ignore-scripts --omit=optional --no-audit --no-fund` on first use
  or a changed lockfile. `bedrock/video_audit.rs` starts the selected FFmpeg decoder
  or the launcher's `--decode-video` process. The native `src/capture.rs` starts
  FFmpeg or the launcher executable with `--encode-video-stream`; both are explicit
  recording/export actions. Starting Minecraft delegates to main's existing
  Windows package launch. No native build tool is invoked by the launcher.
- **Manual subprocess tools:** `build.py` runs Gradle, Cargo, disposable render hosts,
  FFmpeg and ffprobe; native `examples/render_host.rs` and `render_host_dx12.rs`
  spawn the loader, and `examples/isolation_host.rs` spawns its own child process.
  `native/auto-replay.py`, `auto-world.py`, `control-probe.py`,
  `inspect-initialization.ps1` and `inspect-installed.ps1` run the
  build/probe/recording tools explicitly; they are not launcher startup tasks.
- **Network/downloads:** `offline.rs`'s npm invocation downloads pinned packages from
  `registry.npmjs.org` and the pinned `prismarine-xbox-services` tarball from
  `codeload.github.com`. `offline/connector.cjs` pings and connects to the chosen
  server over UDP/RakNet and serves the local client on loopback.
  `offline/replay.cjs`, `replay-nethernet.cjs`, `loopback-rtc.cjs` and
  `lan-transport.cjs` provide local replay/NetherNet/discovery/RTC transports;
  `record-installed-world.cjs` is a separate manual local-world recording tool.
  Connector `loadProtocol` replaces Xbox/Realm/online authentication entry points
  with errors; the transitive Xbox libraries remain in the lockfile, but this
  offline path does not request Xbox tokens. Native `probe_network.rs`,
  `probe_isolation.rs`, `probe_join.rs` and `probe_errors.rs` hook networking and
  Xbox/error/join routines for manually selected probes; their presence is not
  a guarantee that the stock game makes no Xbox connections. Minecraft can still
  contact its normal Xbox/Store services. Main's existing version catalog,
  Microsoft update/UWP download and third-party GitHub release endpoints are retained.
- **Java assets and test networking:** `AppearanceAssets.fetch`, `Looks` and
  `Cosmetics` use the existing Arctic texture/asset service when the explicitly
  loaded native Java client needs assets; the new cache retains their raw bytes.
  `native/cosmetics-probe.py`, `examples/java_lookup_host.rs`,
  `examples/isolation_host.rs`, `examples/support/loopback_connect.rs`,
  `offline/*test.cjs`, `launcher-smoke.cjs`, `launcher-replay-smoke.cjs` and
  Java `Smoke.java` use disposable local services/sockets. Isolation tests also
  attempt wildcard/documentation-address sockets to exercise their guards.

The following source inventory uses paths relative to
`mod/versions/bedrock/native/`. It includes hook/memory code behind optional
probe configurations and test-only hooks, not just the default DLL path.

**MinHook installation and wrapper modules**

- [`src/depth_queue.rs`](../mod/versions/bedrock/native/src/depth_queue.rs)
- [`src/depth_tracker.rs`](../mod/versions/bedrock/native/src/depth_tracker.rs)
- [`src/native_batch.rs`](../mod/versions/bedrock/native/src/native_batch.rs)
- [`src/native_body.rs`](../mod/versions/bedrock/native/src/native_body.rs)
- [`src/native_camera.rs`](../mod/versions/bedrock/native/src/native_camera.rs)
- [`src/native_camera_control.rs`](../mod/versions/bedrock/native/src/native_camera_control.rs)
- [`src/native_camera_time.rs`](../mod/versions/bedrock/native/src/native_camera_time.rs)
- [`src/native_cape_movement.rs`](../mod/versions/bedrock/native/src/native_cape_movement.rs)
- [`src/native_cape_phase.rs`](../mod/versions/bedrock/native/src/native_cape_phase.rs)
- [`src/native_clock.rs`](../mod/versions/bedrock/native/src/native_clock.rs)
- [`src/native_emote_cache.rs`](../mod/versions/bedrock/native/src/native_emote_cache.rs)
- [`src/native_emote_cache_lifetime.rs`](../mod/versions/bedrock/native/src/native_emote_cache_lifetime.rs)
- [`src/native_emote_upload.rs`](../mod/versions/bedrock/native/src/native_emote_upload.rs)
- [`src/native_expression_binding.rs`](../mod/versions/bedrock/native/src/native_expression_binding.rs)
- [`src/native_flipbook.rs`](../mod/versions/bedrock/native/src/native_flipbook.rs)
- [`src/native_fog.rs`](../mod/versions/bedrock/native/src/native_fog.rs)
- [`src/native_framebuffer.rs`](../mod/versions/bedrock/native/src/native_framebuffer.rs)
- [`src/native_gpu_constants.rs`](../mod/versions/bedrock/native/src/native_gpu_constants.rs)
- [`src/native_graphics_observer.rs`](../mod/versions/bedrock/native/src/native_graphics_observer.rs)
- [`src/native_packet.rs`](../mod/versions/bedrock/native/src/native_packet.rs)
- [`src/native_palette_writer.rs`](../mod/versions/bedrock/native/src/native_palette_writer.rs)
- [`src/native_render_clock.rs`](../mod/versions/bedrock/native/src/native_render_clock.rs)
- [`src/native_resource_packs.rs`](../mod/versions/bedrock/native/src/native_resource_packs.rs)
- [`src/native_shader_consumer.rs`](../mod/versions/bedrock/native/src/native_shader_consumer.rs)
- [`src/native_skinned_desc.rs`](../mod/versions/bedrock/native/src/native_skinned_desc.rs)
- [`src/native_submit.rs`](../mod/versions/bedrock/native/src/native_submit.rs)
- [`src/native_task_activity.rs`](../mod/versions/bedrock/native/src/native_task_activity.rs)
- [`src/native_transform.rs`](../mod/versions/bedrock/native/src/native_transform.rs)
- [`src/probe_errors.rs`](../mod/versions/bedrock/native/src/probe_errors.rs)
- [`src/probe_isolation.rs`](../mod/versions/bedrock/native/src/probe_isolation.rs)
- [`src/probe_join.rs`](../mod/versions/bedrock/native/src/probe_join.rs)
- [`src/probe_network.rs`](../mod/versions/bedrock/native/src/probe_network.rs)
- [`src/render_bridge.rs`](../mod/versions/bedrock/native/src/render_bridge.rs)
- [`src/world_hooks.rs`](../mod/versions/bedrock/native/src/world_hooks.rs)
- [`vendor/hudhook/src/hooks/dx11.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/hooks/dx11.rs)
- [`vendor/hudhook/src/hooks/dx12.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/hooks/dx12.rs)
- [`vendor/hudhook/src/hooks/dx9.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/hooks/dx9.rs)
- [`vendor/hudhook/src/hooks/mod.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/hooks/mod.rs)
- [`vendor/hudhook/src/hooks/opengl3.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/hooks/opengl3.rs)
- [`vendor/hudhook/src/mh.rs`](../mod/versions/bedrock/native/vendor/hudhook/src/mh.rs)
- [`vendor/hudhook/vendor/minhook/src/buffer.c`](../mod/versions/bedrock/native/vendor/hudhook/vendor/minhook/src/buffer.c)
- [`vendor/hudhook/vendor/minhook/src/hook.c`](../mod/versions/bedrock/native/vendor/hudhook/vendor/minhook/src/hook.c)
- [`vendor/hudhook/vendor/minhook/src/trampoline.c`](../mod/versions/bedrock/native/vendor/hudhook/vendor/minhook/src/trampoline.c)

**Game-memory read boundary and its consumers**

- [`src/animation_pose.rs`](../mod/versions/bedrock/native/src/animation_pose.rs)
- [`src/depth_queue.rs`](../mod/versions/bedrock/native/src/depth_queue.rs)
- [`src/ecs.rs`](../mod/versions/bedrock/native/src/ecs.rs)
- [`src/live_client.rs`](../mod/versions/bedrock/native/src/live_client.rs)
- [`src/native_appearance.rs`](../mod/versions/bedrock/native/src/native_appearance.rs)
- [`src/native_attachment_binding.rs`](../mod/versions/bedrock/native/src/native_attachment_binding.rs)
- [`src/native_batch.rs`](../mod/versions/bedrock/native/src/native_batch.rs)
- [`src/native_body.rs`](../mod/versions/bedrock/native/src/native_body.rs)
- [`src/native_camera.rs`](../mod/versions/bedrock/native/src/native_camera.rs)
- [`src/native_camera_control.rs`](../mod/versions/bedrock/native/src/native_camera_control.rs)
- [`src/native_camera_time.rs`](../mod/versions/bedrock/native/src/native_camera_time.rs)
- [`src/native_cape_movement.rs`](../mod/versions/bedrock/native/src/native_cape_movement.rs)
- [`src/native_cape_phase.rs`](../mod/versions/bedrock/native/src/native_cape_phase.rs)
- [`src/native_chunk_work.rs`](../mod/versions/bedrock/native/src/native_chunk_work.rs)
- [`src/native_clock.rs`](../mod/versions/bedrock/native/src/native_clock.rs)
- [`src/native_dimension.rs`](../mod/versions/bedrock/native/src/native_dimension.rs)
- [`src/native_draw_matrices.rs`](../mod/versions/bedrock/native/src/native_draw_matrices.rs)
- [`src/native_emote_cache.rs`](../mod/versions/bedrock/native/src/native_emote_cache.rs)
- [`src/native_emote_cache_lifetime.rs`](../mod/versions/bedrock/native/src/native_emote_cache_lifetime.rs)
- [`src/native_emote_cache_relocation.rs`](../mod/versions/bedrock/native/src/native_emote_cache_relocation.rs)
- [`src/native_emote_palette.rs`](../mod/versions/bedrock/native/src/native_emote_palette.rs)
- [`src/native_emote_upload.rs`](../mod/versions/bedrock/native/src/native_emote_upload.rs)
- [`src/native_engine_entry_profile.rs`](../mod/versions/bedrock/native/src/native_engine_entry_profile.rs)
- [`src/native_equipment.rs`](../mod/versions/bedrock/native/src/native_equipment.rs)
- [`src/native_expression_binding.rs`](../mod/versions/bedrock/native/src/native_expression_binding.rs)
- [`src/native_flipbook.rs`](../mod/versions/bedrock/native/src/native_flipbook.rs)
- [`src/native_fog.rs`](../mod/versions/bedrock/native/src/native_fog.rs)
- [`src/native_frame_chunks.rs`](../mod/versions/bedrock/native/src/native_frame_chunks.rs)
- [`src/native_frame_input.rs`](../mod/versions/bedrock/native/src/native_frame_input.rs)
- [`src/native_frame_material.rs`](../mod/versions/bedrock/native/src/native_frame_material.rs)
- [`src/native_frame_ordinary.rs`](../mod/versions/bedrock/native/src/native_frame_ordinary.rs)
- [`src/native_framebuffer.rs`](../mod/versions/bedrock/native/src/native_framebuffer.rs)
- [`src/native_game_mode.rs`](../mod/versions/bedrock/native/src/native_game_mode.rs)
- [`src/native_gpu_constants.rs`](../mod/versions/bedrock/native/src/native_gpu_constants.rs)
- [`src/native_input.rs`](../mod/versions/bedrock/native/src/native_input.rs)
- [`src/native_item_enchantments.rs`](../mod/versions/bedrock/native/src/native_item_enchantments.rs)
- [`src/native_live_client_profile.rs`](../mod/versions/bedrock/native/src/native_live_client_profile.rs)
- [`src/native_material_owner.rs`](../mod/versions/bedrock/native/src/native_material_owner.rs)
- [`src/native_molang_constant_profile.rs`](../mod/versions/bedrock/native/src/native_molang_constant_profile.rs)
- [`src/native_network_work.rs`](../mod/versions/bedrock/native/src/native_network_work.rs)
- [`src/native_packet.rs`](../mod/versions/bedrock/native/src/native_packet.rs)
- [`src/native_palette_binding.rs`](../mod/versions/bedrock/native/src/native_palette_binding.rs)
- [`src/native_palette_writer.rs`](../mod/versions/bedrock/native/src/native_palette_writer.rs)
- [`src/native_presentation_clock.rs`](../mod/versions/bedrock/native/src/native_presentation_clock.rs)
- [`src/native_render_clock.rs`](../mod/versions/bedrock/native/src/native_render_clock.rs)
- [`src/native_render_work.rs`](../mod/versions/bedrock/native/src/native_render_work.rs)
- [`src/native_resource_packs.rs`](../mod/versions/bedrock/native/src/native_resource_packs.rs)
- [`src/native_shader_clock.rs`](../mod/versions/bedrock/native/src/native_shader_clock.rs)
- [`src/native_shader_consumer.rs`](../mod/versions/bedrock/native/src/native_shader_consumer.rs)
- [`src/native_shader_inputs.rs`](../mod/versions/bedrock/native/src/native_shader_inputs.rs)
- [`src/native_shader_layout.rs`](../mod/versions/bedrock/native/src/native_shader_layout.rs)
- [`src/native_shared_chunk_work.rs`](../mod/versions/bedrock/native/src/native_shared_chunk_work.rs)
- [`src/native_skin_selection.rs`](../mod/versions/bedrock/native/src/native_skin_selection.rs)
- [`src/native_skinned_desc.rs`](../mod/versions/bedrock/native/src/native_skinned_desc.rs)
- [`src/native_skinned_material_profile.rs`](../mod/versions/bedrock/native/src/native_skinned_material_profile.rs)
- [`src/native_skins.rs`](../mod/versions/bedrock/native/src/native_skins.rs)
- [`src/native_submit.rs`](../mod/versions/bedrock/native/src/native_submit.rs)
- [`src/native_task_activity.rs`](../mod/versions/bedrock/native/src/native_task_activity.rs)
- [`src/native_transform.rs`](../mod/versions/bedrock/native/src/native_transform.rs)
- [`src/native_world_work.rs`](../mod/versions/bedrock/native/src/native_world_work.rs)
- [`src/players.rs`](../mod/versions/bedrock/native/src/players.rs)
- [`src/probe_isolation.rs`](../mod/versions/bedrock/native/src/probe_isolation.rs)
- [`src/probe_join.rs`](../mod/versions/bedrock/native/src/probe_join.rs)
- [`src/probe_network.rs`](../mod/versions/bedrock/native/src/probe_network.rs)
- [`src/render_bridge.rs`](../mod/versions/bedrock/native/src/render_bridge.rs)
- [`src/world_hooks.rs`](../mod/versions/bedrock/native/src/world_hooks.rs)

The standalone examples also deliberately load DLLs, allocate synthetic engine
memory and install fixture hooks. They are not launcher entry points:
`examples/render_host.rs`, `render_host_dx12.rs`, `isolation_host.rs`,
`async_host.rs`, `java_lookup_host.rs`, `inspect_debug_chain.rs` and
`examples/support/loopback_connect.rs`. The vendored renderer's DX9/DX11/DX12/
OpenGL backends map/copy GPU buffers; MinHook's HDE sources decode instructions
for trampolines. These are source dependencies, not prebuilt code.

## Profiles (`arctic-core::profiles`)

`profiles.json` lists profiles and the active one. `DataDirs::with_profile(id)` scopes the
per-profile paths (settings, accounts, instances, game logs) to `profiles/<id>/`. Shared
downloads (`shared/`, `runtimes/`, `meta/`, `cache/`) stay launcher-wide. On first start,
data from the pre-profiles layout is moved into `profiles/default/`. Removing a profile
moves its folder to `profiles/.trash/`. The app switches profiles by reloading all
profile-scoped state (`session.rs`), and the CLI takes `--profile`.

## Updates (`arctic-core::update`)

See [releasing.md](releasing.md). Briefly: GitHub Releases API → pick the newest release
for the channel → verify the asset's GitHub `sha256:` digest → `self_replace` → restart.
