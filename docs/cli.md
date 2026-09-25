# Command line (`arctic`)

`arctic.exe` is a headless companion to the launcher. It uses the same data folder,
accounts, settings and downloads as the GUI, so you can script installs and launches,
make desktop shortcuts, or run Minecraft on a machine you only reach over SSH.

```powershell
arctic launch                       # latest release as the active account
arctic launch 1.21.4 --offline Steve
arctic launch latest --account Alex --memory 6G --wait
arctic open --launch 1.20.1         # open the launcher window and start 1.20.1
```

Build it with `cargo build --release -p arctic-cli`. The binary is
`target\release\arctic.exe`. Releases ship it as `arctic-windows-x64.exe`.

## Global options

| Option | Meaning |
|---|---|
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
| `-a, --account <NAME>` | Saved account by username, UUID or id (default: the active account) |
| `--offline <USERNAME>` | Play offline with this name. Not saved unless you add `--save` |
| `-m, --memory <SIZE>` | Max heap, e.g. `4G`, `6144M`, `6144` |
| `--width <PX>` / `--height <PX>` | Window size |
| `--fullscreen` | Start fullscreen |
| `--java <PATH>` | Use this `java(w).exe` instead of the managed runtime |
| `-w, --wait` | Stay attached: print the game log, exit with the game's code |
| `--dry-run` | Prepare everything and print the command line (token redacted) |

Without `--wait`, the game is started **detached**: it keeps running after `arctic` exits,
and its output goes to `logs\game-vanilla.log`. Microsoft sessions are refreshed
automatically when needed.

### `arctic accounts`

```text
arctic accounts list
arctic accounts add-offline <USERNAME>
arctic accounts login [--browser]     # device code by default (works over SSH)
arctic accounts use <ACCOUNT>
arctic accounts remove <ACCOUNT>
```

`<ACCOUNT>` is a username (case-insensitive), a Minecraft UUID (with or without dashes),
or the internal id. Microsoft login needs a client ID (see [microsoft-auth.md](microsoft-auth.md)).

### `arctic java <VERSION>`

Prints the Java executable that version uses, installing the runtime if needed. This is
handy for running tools against the same JVM.

### `arctic paths`

Shows the data folder layout: settings, accounts, versions, libraries, assets, runtimes,
instances and logs.

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

The launcher exe is looked up next to `arctic.exe` (as `arctic-launcher.exe` or
`Arctic Launcher.exe`) or via `ARCTIC_LAUNCHER_EXE`.

## GUI flags

The same flags work on the launcher itself, which is useful for desktop shortcuts:

```text
"Arctic Launcher.exe" --launch 1.21.4 --account Steve --no-intro
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
