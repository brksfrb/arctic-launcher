# Architecture

## Principles

- **Core vs UI.** `arctic-core` holds all logic and is UI-agnostic. `arctic-app` only renders
  state and starts background jobs. A future CLI or another frontend can reuse the core.
- **Never block the UI thread.** Network and disk work runs on `std::thread`s (see
  `arctic-app/src/tasks.rs`), which report back through an `mpsc` channel of `Event`s and call
  `request_repaint()`.
- **Small and fast.** The app uses blocking `ureq` with no async runtime, eframe's `glow`
  backend instead of `wgpu`, and one static executable. Launch checks use file sizes rather
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

`AccountStore` holds `Vec<Account>` plus the active id. Each `Account` is either:

- `Offline`: UUID = Java's `nameUUIDFromBytes("OfflinePlayer:<name>")`.
- `Microsoft(MicrosoftSession)`: MSA refresh token, Minecraft access token, expiry, xuid.

The Microsoft chain is MSA OAuth → Xbox Live → XSTS → `login_with_xbox` → profile, and it
has two entry points:

- **Browser**: authorization code + PKCE, using a loopback redirect on an ephemeral port
  (`http://localhost:<port>`, listening on both 127.0.0.1 and ::1).
- **Device code**: shows a code, polls the token endpoint and honours `slow_down`.

Tokens are refreshed before launch when they are within 5 minutes of expiry.
TODO: encrypt `accounts.json` at rest with Windows DPAPI.

## Instances and future mod loaders

`instances/<id>/instance.json` + `instances/<id>/minecraft/` (game dir). Libraries, assets,
versions and runtimes live in `shared/` and `runtimes/`, so instances stay small.

To add a loader later:

1. Add a variant to `instances::Loader` (e.g. `Fabric { loader_version }`).
2. Fetch the loader's profile JSON into `shared/versions/<loader-id>/`. It has
   `inheritsFrom: <vanilla id>`, which `VersionJson::inherits_from` already models.
3. Merge the profile with its parent (libraries appended, `mainClass` and arguments
   overridden). `versions::load_version` currently rejects inherited profiles with a clear
   error; that check is where the merge goes.
4. Show instances in the Instances tab (`arctic-app/src/ui/instances.rs`).

## Linux later

Platform-specific spots are marked `TODO(linux)`:

- Rule evaluation already maps the host to Mojang's `windows`/`linux`/`osx` names
  (`versions::rules`).
- `java::platform_key` / `java_executable` are the only runtime path differences.
  Runtime `link` entries (symlinks) still need creating.
- The classpath separator is already `cfg`-dependent.
- The updater's asset name would gain a Linux variant (`update::WINDOWS_ASSET`).

## Updates (`arctic-core::update`)

See [releasing.md](releasing.md). Briefly: GitHub Releases API → pick the newest release
for the channel → verify the asset's GitHub `sha256:` digest → `self_replace` → restart.
