<p align="center">
  <img src="docs/assets/icon.png" width="112" alt="Arctic Launcher">
</p>

<h1 align="center">Arctic Launcher</h1>

<p align="center">
  A fast, lightweight Minecraft launcher.<br>
  Every release, mod loaders, mods, skins and playing with friends, in one small app.
</p>

<p align="center">
  <a href="https://github.com/brksfrb/arctic-launcher/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/brksfrb/arctic-launcher?style=flat-square&color=38bdf8"></a>
  <a href="https://github.com/brksfrb/arctic-launcher/actions/workflows/ci.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/brksfrb/arctic-launcher/ci.yml?branch=main&style=flat-square"></a>
  <a href="LICENSE"><img alt="License: GPL-3.0" src="https://img.shields.io/badge/license-GPL--3.0-7dd3fc?style=flat-square"></a>
  <img alt="Windows and Linux" src="https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-0ea5e9?style=flat-square">
</p>

<p align="center">
  <a href="https://github.com/brksfrb/arctic-launcher/releases/latest"><b>Download</b></a> ·
  <a href="https://arcticlauncher.com">Website</a> ·
  <a href="docs/cli.md">Command line</a>
</p>

<p align="center">
  <img src="docs/assets/screenshot.webp" alt="Arctic Launcher" width="900">
</p>

## Why Arctic

- **Plays every Minecraft release.** Pick a version and press Play. Arctic downloads the
  game and the exact Java it needs, so you don't install Java yourself.
- **Fast.** Downloads run in parallel over dozens of connections: a complete fresh install
  takes about 20 seconds on a fast connection, and relaunching an installed version is
  instant.
- **Light.** One small native app, no Electron and no browser engine. It uses almost no
  CPU while you play.
- **Mods in a click.** Create Fabric, Quilt, NeoForge or Forge instances, then search
  Modrinth and install mods with their dependencies, or install a whole modpack in one
  click. Each instance keeps its own version, mods, worlds and Java settings.
- **Bring your worlds.** Import worlds from the Minecraft Launcher, Prism Launcher, other
  instances or a .zip, and back them up with one click.
- **Play together.** Invite friends into your world with a code. No server, no port
  forwarding: open your world to LAN and Arctic connects you directly (or through an
  encrypted relay when a direct link isn't possible).
- **Wear any skin, any cape, free.** Pick a skin from your library (or copy a player's by
  name), choose a cape or use your own image, and every Arctic player sees it, on any
  account. Preview everything on a 3D model. Microsoft accounts can also change their real
  Minecraft skin from here.
- **Skin gallery.** Browse skins other players shared, search by name or creator, and wear
  one in a click. Share your own from your library.
- **Arctic Client.** Vanilla gets a fresh look: restyled menus (pick Arctic, Aurora or
  Classic), HUD widgets like FPS, CPS, ping and keystrokes that you drag into place, and
  everyone's Arctic looks in game. Press Right Shift in game for the Arctic menu. It's on
  by default for Minecraft 26.3, comes to Fabric and Quilt instances too, and you can turn
  it off per instance.
- **Your accounts, side by side.** Sign in with Microsoft in the browser or with a short
  code. Keep several accounts and switch in one click.
- **Profiles.** Keep completely separate setups on one PC, each with its own accounts,
  settings, instances and worlds. Switch from the top of the sidebar.
- **Live logs.** Minecraft's output appears inside the launcher, readable, searchable and
  filterable by warnings or errors.
- **Made to look at.** An animated arctic night with aurora, snowfall and shooting stars.
  Aurora, Dark and Light themes.
- **Discord status.** Friends see what you're playing. Turn it off in Settings.
- **Command line included.** Launch versions and instances, install mods and manage
  profiles from a terminal, with scriptable JSON output.
- **Stays up to date.** Updates install themselves in seconds, verified before they're
  applied.
- **Open source** under the GPL-3.0.

## Install

### Windows

1. Download **`arctic-launcher-windows-x64.exe`** from the
   [latest release](https://github.com/brksfrb/arctic-launcher/releases/latest).
2. Run it. There is no installer; put it wherever you like.

Windows may show a SmartScreen prompt for new apps. Choose **More info → Run anyway**.

### Linux

1. Download **`arctic-launcher-linux-x64`** from the
   [latest release](https://github.com/brksfrb/arctic-launcher/releases/latest).
2. Make it executable and run it:

   ```sh
   chmod +x arctic-launcher-linux-x64
   ./arctic-launcher-linux-x64
   ```

Works on X11 and Wayland desktops with OpenGL.

### Command line tool (optional)

Grab `arctic-windows-x64.exe` or `arctic-linux-x64` from the same release and put it on
your `PATH` as `arctic`. See the [command line guide](docs/cli.md).

## Getting started

The first time you open Arctic, a short setup picks your theme, account, memory and
first instance. After that:

1. On **Play**, pick a version or an instance. The picker shows which versions are
   already installed.
2. Press **Play**.

For mods, open **Instances**, create a Fabric instance and use **Browse mods**. To play
with friends, open **Play together**. Memory, window size, theme and more are under
**Settings**. To keep a separate setup,
for example for testing or family members, create a **profile** from the switcher under
the logo.

## Command line

```sh
arctic launch                          # latest release, active account
arctic launch 1.20.1 --account Alex --memory 6G
arctic launch latest --wait
arctic install 1.21.4                  # download only
arctic instances create "Fabric" --version 26.3 --loader fabric
arctic mods install sodium --instance fabric
arctic launch --instance fabric
arctic profiles create "Speedruns" --switch
arctic open --launch 1.21.4            # open the launcher and start playing
```

Everything also works with `--json` for scripts. Full reference:
[docs/cli.md](docs/cli.md).

## FAQ

**Where are my files?**
Windows: `%LOCALAPPDATA%\ArcticLauncher`. Linux: `~/.local/share/ArcticLauncher`.
Each profile lives in `profiles/<name>/`. Worlds are inside
`instances/vanilla/minecraft/saves`. `arctic paths` prints them all.

**Where are my sign-in details stored?**
Only on your computer, in your profile folder. They are sent only to Microsoft, Xbox and
Mojang to sign you in. Arctic's own service for looks never receives your password or
tokens: it checks who you are the same way a Minecraft server does.

**What does Arctic connect to?**
Mojang and Microsoft for the game and sign-in, Modrinth and the Fabric, Quilt, NeoForge
and Forge servers for mods and loaders, GitHub for updates, and the Arctic looks service,
which shares the skin and cape you picked with other Arctic players. Play together connects you directly to your friend, using a public
relay only to set that up or when a direct link fails.

**Does it need Java?**
No. Arctic downloads the official Java runtime each Minecraft version needs and keeps it
up to date.

**Mods?**
Yes. Create a Fabric, Quilt, NeoForge or Forge instance under **Instances** and browse
Modrinth from there. You can also drop `.jar` files into the instance's `mods` folder.

**Do my friends need Arctic to play together?**
Yes, both of you need Arctic Launcher, the same Minecraft version and the same mods.

**Is this an official Minecraft product?**
No. Arctic Launcher is an independent project, not approved by or associated with Mojang
or Microsoft.

## Building from source

See [docs/building.md](docs/building.md). The architecture overview is in
[docs/architecture.md](docs/architecture.md).

## License

[GPL-3.0-or-later](LICENSE). "Minecraft" is a trademark of Mojang Synergies AB.
