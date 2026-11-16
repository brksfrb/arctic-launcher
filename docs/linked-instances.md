# Linked instances

An instance can be **linked** with friends: the owner shares it by a short code, a friend enters
the code in the launcher and gets a linked copy, and later changes the owner publishes reach the
copy as a pending update. There are no accounts or e-mail sign-ins: the code is what you share, and
the owner key the server hands out when the link is made is what lets only the owner's PC publish.

Code: `crates/arctic-core/src/linked/` (manifest, allow-list, diff, apply, server client),
`crates/arctic-app/src/link_tasks.rs` and `crates/arctic-app/src/ui/linked.rs` (launcher), and
`mod/core/src/main/java/com/arcticlauncher/client/servers/MissingMods*.java` (the in-game missing-mods
screen).

## What is synced, and what never is

Only two things travel:

- **The mod list.** Exact Modrinth versions, each with the SHA-1 and SHA-512 of its file. Jars that
  are not from Modrinth (added by hand, from another site, or changed so their hash isn't Modrinth's
  file any more) are left out, and the owner sees which ones on the Share / Link dialog. The
  launcher's own jars (the Arctic Client, its performance mods) are never in a manifest: each PC
  installs its own.
- **Config files** under `config/`, if the owner keeps "Include config files" on. Plain-text formats
  only (`json`, `json5`, `jsonc`, `toml`, `properties`, `cfg`, `conf`, `txt`, `yml`, `yaml`, `ini`,
  `snbt`), at most 256 KiB each and 1.5 MiB together.

Never: worlds and saves, screenshots, `options.txt` and other game settings, key bindings,
`servers.dat`, accounts, Java paths, JVM flags, logs, caches. Files under `config/` that look
personal are refused too: Arctic's own settings (`config/arctic*`, which hold the proxy and the
launcher bridge secret), and any file or folder whose name contains `account`, `auth`, `token`,
`secret`, `session`, `password`, `credential`, `cookie`, `private`, `api_key`, `keybind`, `keymap`,
`hotkey` or `servers`, or a folder named `cache`, `logs`, `backups`, `waypoints`, `saves`, `worlds`,
`screenshots`, `replays`… (full list: `linked/paths.rs`).

### The path allow-list

Every config path in a manifest is checked on **both** sides (`paths::check_config_path`) before
anything is read or written. A path must be relative, use `/`, start with `config/`, be at most 8
folders deep and 240 bytes long, and consist of letters, digits and `. _ - + ( ) [ ]` and spaces.
Refused: `..` and `.` components, empty components (`config//a`), absolute paths, backslashes, drive
letters and `:`, hidden names (a leading `.`), names ending in `.` or a space, Windows device names
(`con`, `nul`, `com1`…), control characters, other extensions and the personal names above. When
writing, a path that goes through a symlink or junction is refused as well. Mod file names must be
plain `*.jar` names (no folders), so a manifest can only ever touch `mods/<name>.jar` and the
allowed `config/…` files.

## Approval: the diff screen

Nothing is applied without the player's yes. Taking a link (the first copy, every update, and the
missing mods a server asks for) opens a diff screen in the launcher listing:

- the **Minecraft version or loader** change, if the owner moved,
- mods **added**, **updated** (old → new version) and **removed**,
- config files **added**, **changed** and **removed** (removed: only files that came from the link
  earlier),

each with its own tick box. Defaults: everything the owner changed is ticked; a mod the friend
added themselves is listed as a removal but **not** ticked; a config file the link brought earlier
and the friend edited since is not ticked for removal. While a version change is unticked, mod
additions and updates can't be applied (they were picked for the other version).

An update the owner publishes shows up as a **pending update** (a badge on the instance and a bar on
its page) until the friend reviews it; "Not now" hides the reminder until the next update.

### Applying, all or nothing

`linked::apply`:

1. Each added or updated mod is looked up on Modrinth by version id; the version must belong to the
   manifest's project and have a file with the manifest's SHA-512, served from
   `https://cdn.modrinth.com/`. The files are downloaded into a staging folder in the game folder
   (`.arctic-link/`), then their size, SHA-1 and SHA-512 are checked. Config files are decoded and
   their SHA-256 checked. A failed download or a hash mismatch stops here: nothing changed.
2. Replaced and removed files are moved to a backup folder, the staged ones moved into place,
   `mods.json` written, then the instance (`instance.json`, for a version change) and `link.json`
   saved. Each step is journaled; any failure undoes the journal in reverse.
3. The work folders are deleted.

Removing, replacing or changing config files needs the game closed (the game holds its jars);
adding mods (the missing-mods flow) works while it runs.

## Unlinking

"Unlink" makes the instance a normal local one: `link.json` is deleted, every file stays as it is.
An owner can unlink (the code keeps working for friends, nothing new is published) or **Stop
sharing**, which also deletes the link on the server so the code stops working; the friends' copies
are then told that the owner stopped sharing, and keep their files.

## Local state: `instances/<id>/link.json`

```json
{
  "code": "abcdefgh",
  "role": "owner",
  "key": "local1:…",          // owner only: the owner key, encrypted like account tokens
  "include_configs": true,     // owner only
  "revision": 3,               // owner: last published; friend: last taken
  "manifest": { … },           // that revision's manifest
  "latest": 4,                 // friend: newest revision the server has
  "dismissed": 0,              // friend: revision put off with "Not now"
  "gone": false                // the owner stopped sharing
}
```

A duplicated instance doesn't take `link.json` along.

## Manifest format (version 1)

JSON, UTF-8. Readers refuse a newer `arctic_link` than they know ("update the launcher"), and check
every field (ids, hashes, names, paths, sizes, duplicates) before using any of it.

```json
{
  "arctic_link": 1,
  "name": "Our SMP",
  "game_version": "1.21.11",
  "loader": { "type": "fabric", "version": "0.17.2" },
  "mods": [
    {
      "project": "AANobbMI",
      "version": "OihdIimA",
      "title": "Sodium",
      "file": "sodium-fabric-0.6.13+mc1.21.11.jar",
      "size": 1048576,
      "sha1": "40 hex",
      "sha512": "128 hex",
      "enabled": true,
      "dependency": false
    }
  ],
  "configs": [
    {
      "path": "config/sodium-options.json",
      "size": 512,
      "sha256": "64 hex",
      "data": "base64 of the file"
    }
  ]
}
```

| Field | Rules |
|---|---|
| `arctic_link` | Format version, currently `1`. |
| `name` | 1–48 characters, no control characters. |
| `game_version` | Minecraft version id: letters, digits, `._+-` and spaces, ≤ 64. |
| `loader` | `{"type": "vanilla"}` or `{"type": "fabric" \| "quilt" \| "neo_forge" \| "forge", "version": …}` (the launcher's `Loader`). |
| `mods[]` | ≤ 1000. `project`, `version`: Modrinth ids (alphanumeric, ≤ 32). `file`: a plain `*.jar` name. Hashes lowercase hex. One entry per project and per file name. `enabled: false` installs it switched off. |
| `configs[]` | ≤ 1000, each ≤ 256 KiB, together ≤ 1.5 MiB. `path` passes the allow-list; `data` must decode to `size` bytes with `sha256`. |

The whole manifest is at most 3 MiB.

## Server API (to implement on arctic-server)

Base URL: the same Arctic server as looks, friends and share codes (`cosmetics::base_url()`, i.e.
`ARCTIC_COSMETICS_URL` or the default). All bodies are JSON; errors are `{"error": "text"}` with the
status codes below (the launcher shows `error` as is).

Codes use the share-code alphabet (`abcdefghjkmnpqrstuvwxyz23456789`, 8 characters, shown as
`abcd-efgh`); they can share the generator and the namespace check with `/v1/shares`, but links are
a separate table: a link changes over time, a share doesn't.

### `POST /v1/links` — make a link

Headers: `Authorization: Bearer <Arctic session>` **optional** (sent when the launcher has one; use it
only for rate limits and abuse reports, never to own the link).

Body: `{"manifest": <manifest>}`

`201 Created`:

```json
{ "code": "abcdefgh", "key": "<owner key>", "revision": 1 }
```

- `key`: random, at least 32 and at most 128 printable ASCII characters (e.g. 32 bytes base64url).
  Store only a hash of it (SHA-256); it is never sent again.
- `revision` starts at 1.
- Validate like the launcher does (at least: JSON object, `arctic_link` a number, body ≤ 3 MiB);
  `413` when too big, `400` when not a manifest, `429` when rate-limited (suggested: 20 links per IP
  or session per day).

### `PUT /v1/links/{code}` — publish a new revision

Headers: `X-Arctic-Link-Key: <owner key>`

Body: `{"manifest": <manifest>, "base_revision": 3}`

`200 OK`: `{"revision": 4}`

- `403` wrong or missing key, `404` no such code (or deleted), `409` when `base_revision` isn't the
  current revision (published from another PC in between), `413` too big, `429` rate-limited
  (suggested: 60 publishes per link per hour).
- The new revision is the current one + 1; keep only the current manifest (older ones may be kept
  for a while, but nothing reads them).

### `GET /v1/links/{code}` — read a link

No authentication.

`200 OK`:

```json
{ "code": "abcdefgh", "revision": 4, "updated": 1763300000, "manifest": { … } }
```

`updated`: Unix seconds of the last publish. `404` (or `410`) when unknown or deleted. Add
`Cache-Control: no-store`.

### `GET /v1/links/{code}/head` — newest revision only

No authentication. `200 OK`: `{"revision": 4, "updated": 1763300000}`; `404`/`410` as above. The
launcher asks this for each linked copy when it starts and every 15 minutes while it runs, so keep
it cheap (no manifest read).

### `DELETE /v1/links/{code}` — stop sharing

Headers: `X-Arctic-Link-Key: <owner key>`. `204 No Content`; `403` wrong key; `404` already gone
(the launcher treats `404`/`410` as done).

### Limits and retention

- Body ≤ 3 MiB for `POST`/`PUT`.
- A link nobody fetched (`GET` or `head`) for 180 days may be deleted.
- Never log manifests' `configs[].data` or the key.

## Missing mods on joining a server

When a server refuses the game because mods are missing or don't match, the Arctic Client shows its
own **Missing mods** screen in place of Minecraft's disconnect screen (1.21.11, 26.3 and every other
Fabric build; 1.8.9 Legacy Fabric and Forge). It lists what is known:

- **Fabric** servers with Fabric API refuse with the registry entries the client lacks
  (`create:andesite_alloy`…); their namespaces are the mod ids.
- **Forge 1.8.9** (FML) refuses with `Mod rejections` and the mod ids (with versions where FML gives
  them).
- **NeoForge / Forge** (1.13+) servers name their missing mods or channels in the disconnect text
  where they do; a vanilla or Fabric client often only gets "this server requires NeoForge/Forge":
  then the screen says which loader the server needs and that Arctic's mod install can't change a
  loader.
- **Vanilla** clients and plain refusals ("incompatible", "mismatched mod list") get a generic
  message: the screen shows it, plus how a server owner can share their instance as a linked
  instance (Share / Link → code), which is the reliable way.

Mod ids already loaded in the game are marked as installed (with their version when the server gave
one that differs: "mismatched").

**Install with Arctic Launcher** sends the list over the launcher bridge; the launcher looks each id
up on Modrinth (as a project slug, for the instance's version and loader), shows the same diff
screen (adds only) and, once approved, downloads and hash-checks the files. The game then offers
**Restart and rejoin**: it quits, and the launcher starts the instance again straight into the
server (Quick Play).

### Bridge endpoints

Both need the bridge secret (`Authorization: Bearer <secret>`) like every bridge call; `launch` is
the per-launch token from the game's session file, which tells the launcher which instance it is.

`POST /v1/missing-mods`

```json
{
  "launch": "<launch token>",
  "server": "play.example.net",
  "mods": [ { "id": "create", "version": "0.5.1" }, { "id": "jei" } ],
  "loader": "neoforge",      // optional: what the server said it needs
  "reason": "…"              // the disconnect text, for the launcher's log
}
```

`200 OK` with the state (below); `400` for a bad body; `404` for an unknown launch token.

`GET /v1/missing-mods/{launch}` → state:

```json
{ "state": "idle" | "looking" | "review" | "installing" | "installed" | "declined" | "failed",
  "installed": 2, "not_found": ["somemod"], "message": "…" }
```

`POST /v1/missing-mods/{launch}/restart` → `{}`: the launcher relaunches this instance into
`server` once the game has quit; the game quits right after.
