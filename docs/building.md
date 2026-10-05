# Building from source

## Requirements

- [Rust](https://rustup.rs), stable toolchain
- **Windows:** the MSVC toolchain (Visual Studio Build Tools). The Windows SDK's resource
  compiler embeds the app icon; without it the build still succeeds with a default icon.
- **Linux:** a C toolchain (`build-essential`) and ALSA headers (`libasound2-dev`, for voice
  chat). The window uses X11 or Wayland through OpenGL, loaded at runtime.
- **macOS:** Xcode Command Line Tools and CMake (`brew install cmake`).

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

**macOS app:** build both kinds of Mac, then make the universal, ad-hoc signed
`Arctic Launcher.app` (zipped in `dist/`):

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p arctic-app -p arctic-cli --target aarch64-apple-darwin
cargo build --release -p arctic-app -p arctic-cli --target x86_64-apple-darwin
packaging/macos/make-app.sh 0.2.0
```

On Apple Silicon, Minecraft versions before 1.19 (whose graphics libraries have no arm64 build)
run through Rosetta with Intel Java; newer ones run natively.

### The Arctic Client

The launcher embeds `mod/dist/arctic-client.pack` (all the client's jars in one ~1 MB file),
which is committed, so the steps above don't need Java. The client is a shared core (`mod/core`, Java 8, no Minecraft code) plus one
adapter source tree for every Fabric version (`mod/versions/fabric`), where version
differences sit in `//#if MC >= 26.2` … `//#else` … `//#endif` blocks that the build
resolves. `mod/targets.json` lists what gets built and which Minecraft versions each jar
covers; the launcher reads the same file. To change the client, you need JDK 25:

```sh
cd mod && ./gradlew -p core build   # the core's tests, and that it still compiles for Java 8
python mod/check.py                 # every target compiles and its mixins fit (or: python mod/check.py 1.16.5)
python mod/build.py                 # every target, then the pack (or: python mod/build.py 26.2)
```

`check.py` is what CI runs for every target on each push: it compiles the adapter for that
version and checks each mixin hook against that version's game jar, so a shared change
that breaks an old version fails there instead of at someone's game start. Versions 1.8.9
to 1.12.2 build from `mod/versions/legacy` (Legacy Fabric) instead.

Old versions differ in small ways that the adapter hides behind helpers in `Compat`
(entity rotation fields before 1.17, button labels that were plain strings before 1.16,
and so on), `compat/GuiGraphics` (drawing before 1.20) and `compat/KeyCodes` (key codes),
plus a few renames in `versions/fabric/build.gradle`. Use those rather than the game's API
directly in shared code, and the older targets keep compiling. Features a version can't
support are switched off per version in `build.gradle`'s mixin config properties.

Launch an instance with `ARCTIC_COSMETICS_URL=http://127.0.0.1:8080` set to point the
client and the launcher at a local cosmetics server. Adding
`-Darctic.selftest=true` to the instance's JVM arguments makes the client screenshot each
of its screens and quit, which is handy after changing an adapter.

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
(rate limits use `X-Forwarded-For`).

Moderation: set `ARCTIC_COSMETICS_ADMIN_KEYS=name:key,name:key` (keys at least 16 characters,
names letters, digits, `-` or `_`). Each moderator signs in at `/admin` with their own key; to
add or remove a person, edit the list and restart. Shared skins wait there for approval before
they show in the gallery; reported ones come back to the same queue, and every decision is
recorded under the moderator's name. The same key works as an `X-Admin-Key` header for the API
(`arctic servers review`, `DELETE /v1/gallery/{id}`). Without any keys, `/admin` is off. Put
the page behind Cloudflare Access (admin.arcticlauncher.com) for a second lock. The older
single `ARCTIC_COSMETICS_ADMIN_KEY` still works as the moderator `admin`.

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
