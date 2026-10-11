# Releasing and auto-update

The launcher updates itself from GitHub Releases of `brksfrb/arctic-launcher`.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. `.github/workflows/release.yml` builds on Windows, Linux and macOS and publishes the
   release: `arctic-launcher-windows-x64.exe`, `arctic-launcher-linux-x64`,
   `arctic-launcher-macos.zip` (the app), `arctic-launcher-macos` (the program an update
   replaces), the command line tools `arctic-windows-x64.exe`, `arctic-linux-x64` and
   `arctic-macos`, and `SHA256SUMS.txt`. GitHub stores a `sha256:` digest for every asset,
   and the updater refuses assets without one.

Running the workflow by hand (Actions → Release → Run workflow) builds everything without
publishing; the downloads are attached to the run (test builds, like a Mac app for a tester).

Tags containing a `-` (e.g. `v0.3.0-beta.1`) are published as **pre-releases**, which only the
**Beta** update channel sees.

## Optional vs required updates

Updates are **optional** by default: users see a dismissible banner (or, with automatic
installs on, they download by themselves). A **required** update covers the window with an
"Update required" overlay: it downloads right away and restarts into the new version after a
short countdown, with **Update & Restart** and **Exit** buttons. It waits for a running game to
close before restarting. Put one of these HTML
comments in the release notes to change that. They stay invisible on GitHub.

| Marker | Effect |
|---|---|
| `<!-- arctic:required -->` | Everyone below this version must update before playing |
| `<!-- arctic:min-supported=0.2.0 -->` | Required only for users older than `0.2.0`; optional for everyone else |

Use required updates for things like broken auth or security fixes.

## Update flow in the app

1. A few seconds after start (if enabled, so opening isn't slowed down) and from
   *About → Check for updates*, the app calls
   `GET /repos/brksfrb/arctic-launcher/releases`.
2. It picks the newest non-draft release above the running version that matches the channel.
3. On **Update**, it streams the exe to `cache/updates/` and verifies SHA-256. It then
   swaps the running executable using `self_replace`, which works on Windows while the
   exe is running.
4. **Restart now** relaunches the new version.

The whole app is one small executable, so an update is a single download plus a swap.

## "What's new" after an update

The first start of a new version shows a popup built from the release notes of every
release since the version that ran before (Settings → Updates → "Show what's new after
updating" turns it off; the popup has "Don't show this again"). Write the notes for it:

- Sections titled `Fixed` (a `### Fixed` heading or a line that is only `**Fixed**`) are listed
  under **What's fixed**; every other section under **What's new**, with its title.
- `### Download`, `### SHA-256`, HTML comments and the "Full Changelog" line are left out.
- Inline `**bold**`, `` `code` `` and `[links](…)` (shown as text) are understood.

## Secrets used by CI

| Secret | Purpose |
|---|---|
| `ARCTIC_MSA_CLIENT_ID` | Arctic's own Azure client ID, baked into release builds (sign-in works once Mojang approves it) |

TODO: Authenticode code signing (keep the certificate in CI secrets; never commit `.pfx`).
