# ❄ Arctic Launcher

A lightweight, open-source Minecraft: Java Edition launcher written in Rust with a native
[egui](https://github.com/emilk/egui) UI. There are no webviews and no Electron.

**Website:** [arcticlauncher.com](https://arcticlauncher.com) · **Docs (soon):** docs.arcticlauncher.com ·
**License:** GPL-3.0-or-later

> Status: early MVP for personal use. Windows only for now; the code is structured so Linux
> can follow.

## Features (MVP)

| Area | What works |
|---|---|
| Accounts | Microsoft login via **browser** (auth code + PKCE) or **device code**; **offline** accounts; multi-account cards with player-head avatars and a sidebar switcher |
| Versions | Every official **release** from Mojang's version manifest (vanilla only), searchable, with **installed** badges |
| Java | Downloads the **right Java runtime per version** from Mojang, so no JDK install is needed |
| Launch | Builds the official command line (modern + legacy argument formats, natives, assets, log4j config) and runs Minecraft as a child process; parallel downloads with live speed/ETA; optional minimize-while-playing |
| Instances | Built-in **Vanilla** instance with a customizable snowflake icon (6 styles, any color); ready for future mod-loader instances |
| Logs | Live **Minecraft output** (log4j XML collapsed into readable lines) and the **launcher log**, with level filters, search, copy and auto-scroll |
| Settings | Max/min RAM, window resolution (+ presets), fullscreen, extra JVM args, custom Java path, update channel, theme (Aurora / Dark / Light), animations |
| Updates | Checks GitHub Releases; **stable/beta** channels; **optional vs required** updates; SHA-256 verified in-place self-update |

Not in the MVP: skins/cosmetics, proxies, Linux builds, mod loaders (Fabric/Quilt/Forge).

## Build and run (Windows)

Prerequisites: [Rust](https://rustup.rs) (stable, MSVC toolchain) and Git.

```powershell
git clone https://github.com/brksfrb/arctic-launcher
cd arctic-launcher
cargo run --release -p arctic-app
```

The release binary is `target\release\arctic-launcher.exe`, a single self-contained
executable.

Useful commands:

```powershell
cargo test --workspace              # unit tests
cargo clippy --all-targets          # lints
# End-to-end smoke test: downloads + launches a version with an offline account
$env:ARCTIC_DATA_DIR = "$PWD\data"; cargo run -p arctic-core --example smoke_launch -- latest --spawn
```

### Command line

`arctic.exe` (crate `arctic-cli`) drives the same data headlessly. It can list, install
and launch versions, manage accounts, and open the GUI:

```powershell
cargo run --release -p arctic-cli -- launch 1.21.4 --offline Steve
```

Full reference: [docs/cli.md](docs/cli.md).

### Microsoft login

Offline accounts work out of the box. Microsoft login needs an Azure app registration
(a client ID). See [docs/microsoft-auth.md](docs/microsoft-auth.md). The client ID is read from
`ARCTIC_MSA_CLIENT_ID` or `%LOCALAPPDATA%\ArcticLauncher\msa.json` and never from the repo.

## Where data lives

Everything goes under `%LOCALAPPDATA%\ArcticLauncher` (override with `ARCTIC_DATA_DIR`, or put a
`portable.txt` next to the exe to use `.\data`):

```
settings.json      user settings (no secrets)
accounts.json      account tokens: local only, never committed or synced
shared\            versions, libraries, assets (shared by all instances)
runtimes\          managed Java runtimes
instances\vanilla\ default instance (game dir: minecraft\)
logs\              game output (game-<instance>.log)
```

Tip: if the game can't reach Minecraft services because antivirus or a corporate proxy
inspects HTTPS, add `-Djavax.net.ssl.trustStoreType=Windows-ROOT` under
*Settings → Extra JVM arguments* so Java trusts the Windows certificate store.

## Architecture

```
crates/
├── arctic-core/   library, no UI; all logic is unit-tested here
│   ├── auth/        accounts store, offline, microsoft/{oauth, browser, xbox}
│   ├── versions/    Mojang manifest + version JSON model + rule engine
│   ├── java/        Mojang Java runtime index → managed runtimes
│   ├── launch/      file resolution/downloads (files.rs), argument building (args.rs)
│   ├── instances/   instance model (Vanilla today, loaders later)
│   ├── update/      GitHub Releases self-updater
│   ├── storage/     data-dir layout + atomic JSON persistence
│   ├── net/         shared HTTP agent + parallel, checksum-verified downloader
│   └── settings.rs
├── arctic-cli/    `arctic` headless CLI (clap)
└── arctic-app/    egui/eframe binary
    ├── app.rs       state machine + window shell
    ├── tasks.rs     background threads → UI events (mpsc)
    ├── theme.rs     ice-blue look
    └── ui/          one file per tab: play, accounts, instances, settings, about
```

More detail, including how mod loaders and Linux will plug in:
[docs/architecture.md](docs/architecture.md). Release process:
[docs/releasing.md](docs/releasing.md).

## Contributing

Issues and PRs are welcome. Please run `cargo fmt`, `cargo clippy --all-targets` and
`cargo test --workspace` before opening a PR. Never commit client IDs, tokens or keystores.

## License

Arctic Launcher is free software under the [GNU General Public License v3.0 or later](LICENSE).
It is not affiliated with Mojang Studios or Microsoft. "Minecraft" is a trademark of Mojang
Synergies AB.
