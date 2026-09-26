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

### The Arctic mod

The launcher embeds `mod/fabric/dist/arctic-mod-<version>.jar`, which is committed, so the
steps above don't need Java. To change the mod, you need JDK 25:

```sh
cd mod/fabric
./gradlew build
cp build/libs/arctic-mod-26.3-*.jar dist/arctic-mod-26.3.jar
```

Launch a Fabric instance with `ARCTIC_COSMETICS_URL=http://127.0.0.1:8080` set to point the
mod and the launcher at a local cosmetics server.

### The cosmetics server

```sh
ARCTIC_COSMETICS_SECRET=$(openssl rand -hex 32) \
ARCTIC_COSMETICS_ASSETS=crates/arctic-cosmetics/assets \
cargo run -p arctic-cosmetics
```

It listens on `0.0.0.0:8080` and stores data in `cosmetics.db`. For deployment there is a
Dockerfile: `docker build -f crates/arctic-cosmetics/Dockerfile -t arctic-cosmetics .` (mount
a volume at `/data`).

Optional settings: `ARCTIC_COSMETICS_TRUST_PROXY=1` when it runs behind a reverse proxy
(rate limits use `X-Forwarded-For`), and `ARCTIC_COSMETICS_ADMIN_KEY` to remove any gallery
item with `DELETE /v1/gallery/{id}` and an `X-Admin-Key` header.

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
