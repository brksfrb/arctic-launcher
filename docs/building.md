# Building from source

## Requirements

- [Rust](https://rustup.rs), stable toolchain
- **Windows:** the MSVC toolchain (Visual Studio Build Tools). The Windows SDK's resource
  compiler embeds the app icon; without it the build still succeeds with a default icon.
- **Linux:** a C toolchain (`build-essential`). The window uses X11 or Wayland through
  OpenGL, loaded at runtime.

## Build

```sh
git clone https://github.com/brksfrb/arctic-launcher
cd arctic-launcher
cargo build --release -p arctic-app -p arctic-cli
```

This produces:

| Binary | Windows | Linux |
|---|---|---|
| Launcher | `target/release/arctic-launcher.exe` | `target/release/arctic-launcher` |
| Command line | `target/release/arctic.exe` | `target/release/arctic` |

Run the launcher directly with `cargo run --release -p arctic-app`.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs these on every push.

## Development tips

- `ARCTIC_DATA_DIR=<dir>` keeps a dev build's data away from your real installation. A
  `portable.txt` next to the executable does the same, using `./data`.
- `cargo run -p arctic-core --example smoke_launch -- latest --spawn` downloads and starts a
  version and prints timings.
- `arctic -v …` shows the launcher's log on stderr.

## Microsoft sign-in in your build

Microsoft sign-in needs an Azure application (client) ID that Mojang has approved for
Minecraft. Official releases include one. For your own builds, register an app and pass
its ID at build time or at runtime, as described in
[microsoft-auth.md](microsoft-auth.md). Without it, the Microsoft options show as
unavailable.

## Releasing

See [releasing.md](releasing.md).
