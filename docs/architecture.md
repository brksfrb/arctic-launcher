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
| `arctic-cosmetics` | The Arctic cosmetics server (capes, sign-in via Mojang's session server) |
| `mod/fabric` | The Arctic Fabric mod (Java, Gradle), bundled into the launcher as a jar |

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
publishes it to the looks server (`arctic-cosmetics`), which stores textures by SHA-1 and
relays each player's look to every Arctic client. Nothing is owned or unlocked: presets are
just textures everyone may use, and custom images are allowed (size-checked: 64×64 skins,
2:1 capes up to 512×256).

The server only checks **who** publishes, so nobody can change someone else's look:

- **Microsoft accounts:** the client asks for a challenge, calls Mojang's
  `session/minecraft/join` with it, and the server confirms with `hasJoined`, like a
  Minecraft server does. No password or token reaches Arctic.
- **Offline accounts:** the first launcher to use a name claims it with a random key, kept
  in the profile's `cosmetics.json`; later changes need the same key. The server derives
  the UUID from the name exactly like the game, so a key can only claim an offline UUID.

Both give an HMAC-signed token that expires after a week. Before launching a Fabric/Quilt
instance with the mod, the launcher writes `config/arctic-session.json` so cape changes
made in game publish as that player.

The Fabric mod (`mod/fabric`) swaps in the player's Arctic skin (with the right arm model)
and cape, looks players up in batches of 100, caches for 10 minutes, and adds an Arctic menu
(title and pause screens) to pick a preset cape, hide all looks, or hide one player's look.
The launcher embeds the built jar (`mod/fabric/dist/`) and copies it into Fabric and Quilt
instances of supported Minecraft versions before launch (`arctic_mod::sync`).

## Platforms

Windows and Linux (x86-64) are supported from the same code. The differences are small:
- `javaw.exe` vs `bin/java` in managed runtimes;
- runtime symlinks, which exist only on Linux;
- the classpath separator;
- the updater's per-platform release asset;
- the Windows-only title bar tint and exe icon.

Rule evaluation maps the host to Mojang's `windows`/`linux`/`osx` names.

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
