# Releasing and auto-update

The launcher updates itself from GitHub Releases of `brksfrb/arctic-launcher`.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. `.github/workflows/release.yml` builds on Windows and Linux and publishes the release
   with four assets: `arctic-launcher-windows-x64.exe`, `arctic-launcher-linux-x64`,
   `arctic-windows-x64.exe` and `arctic-linux-x64`. GitHub stores a `sha256:` digest for
   every asset, and the updater refuses assets without one.

Tags containing a `-` (e.g. `v0.3.0-beta.1`) are published as **pre-releases**, which only the
**Beta** update channel sees.

## Optional vs required updates

Updates are **optional** by default: users see a dismissible banner. Put one of these HTML
comments in the release notes to change that. They stay invisible on GitHub.

| Marker | Effect |
|---|---|
| `<!-- arctic:required -->` | Everyone below this version must update before playing |
| `<!-- arctic:min-supported=0.2.0 -->` | Required only for users older than `0.2.0`; optional for everyone else |

Use required updates for things like broken auth or security fixes.

## Update flow in the app

1. On start (if enabled) and from *About → Check for updates*, the app calls
   `GET /repos/brksfrb/arctic-launcher/releases`.
2. It picks the newest non-draft release above the running version that matches the channel.
3. On **Update**, it streams the exe to `cache/updates/` and verifies SHA-256. It then
   swaps the running executable using `self_replace`, which works on Windows while the
   exe is running.
4. **Restart now** relaunches the new version.

The whole app is one small executable, so an update is a single download plus a swap.

## Secrets used by CI

| Secret | Purpose |
|---|---|
| `ARCTIC_MSA_CLIENT_ID` (optional) | Baked into release builds as the default Azure client ID |

TODO: Authenticode code signing (keep the certificate in CI secrets; never commit `.pfx`).
