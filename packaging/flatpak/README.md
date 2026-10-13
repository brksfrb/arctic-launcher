# Arctic Flatpak distribution

The existing `brksfrb/arctic-launcher` repository owns both the source and the
signed Flatpak feed, served by its GitHub Pages site. No separate source repository
or Flathub app submission is needed. Flathub supplies the Freedesktop runtime only.

Install: https://brksfrb.github.io/arctic-launcher/arctic.flatpakref

Opening this file in a Flatpak-capable software manager installs Arctic and registers
its update feed. Future stable releases appear in that software manager. Automatic
installation follows the user's software-manager settings; command-line users can
run `flatpak update`. The standalone `.flatpak` release asset also records the feed.

## Release process

`.github/workflows/flatpak.yml` runs when a stable GitHub release is published:

1. Compile the launcher and CLI on the existing Ryzen 9 runner.
2. Package on a disposable GitHub Linux worker using Freedesktop 25.08. The Ryzen
   service's sandbox remains unchanged.
3. Import `FLATPAK_GPG_PRIVATE_KEY` into a temporary keyring and sign the app and
   repository. Only the public key belongs in Git.
4. Install with signature verification, exercise a signed update on a separate copy,
   verify preserved user data, and test the launcher and Fabric game under Xvfb.
5. Attach the bundle, install file and checksums to the release, then publish Pages.

Manual workflow runs build and test without publishing. Test logs and screenshots
are retained as workflow artifacts. The public feed is replaced only after all checks
pass. Each deployment contains a complete repository, so existing installations can
update without earlier repository objects being retained.

Keep the same signing key across releases. Back it up securely; never replace it as
a routine release step. Its public fingerprint is:

`EFC0A0E57389D69329175FF63946443ACBE7F0AF`

GitHub Pages must use **GitHub Actions** as its deployment source. The workflow needs
`pages: write`, `id-token: write` and the `github-pages` environment. Repository files
are deployment artifacts rather than committed generated binaries.

## Sandbox behavior

App ID: `com.arcticlauncher.ArcticLauncher`, branch: `stable`, architecture: `x86_64`.
The package downloads Java and game files under its own XDG data directory:

`~/.var/app/com.arcticlauncher.ArcticLauncher/data/ArcticLauncher`

It has network access for downloads, sign-in, multiplayer and local game bridges;
X11/Wayland, graphics and audio access for the launcher and Java games; and desktop
tray/Discord IPC access. File picking uses desktop portals. It does not grant general
access to the user's home directory; automatic discovery of other launchers' data is
therefore limited by the sandbox.

The launcher's built-in executable replacement is disabled inside Flatpak. The CLI is
available with `flatpak run --command=arctic com.arcticlauncher.ArcticLauncher`.
