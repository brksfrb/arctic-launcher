//! Measure the real pre-launch pipeline (Play to Java process) in an isolated data directory,
//! with an offline account. Never point it at a real launcher data directory.
//!
//! `cargo run -p arctic-core --features offline-accounts --example startup_bench -- <root> <version>
//!  [--performance] [--without-client] [--profile vanilla|fabric|arctic] [--spawn]
//!  [--window <seconds>] [--command-out <file>]`
//!
//! `--profile` picks what the player would have: `vanilla` (no client, no performance mods),
//! `fabric` (a Fabric instance with nothing in it) or `arctic` (the default instance: Arctic
//! Client and performance mods on Fabric underneath). Without it the instance is Fabric 0.20.1.
//! `--window` (with `--spawn`) waits for the game window and needs a display (Xvfb).
//! `tools/launchbench/run.py` runs first installs, repeat launches and version switches with it.
//!
//! `--command-out` writes the planned game command (`[java, args...]`, JSON) so the game start
//! can be timed separately with the same flags. Set ARCTIC_BENCH_JAVA to an existing Java
//! executable to avoid measuring a runtime download.
use std::path::PathBuf;
use std::time::Instant;

use arctic_core::auth::offline;
use arctic_core::instances::{self, Loader};
use arctic_core::launch::{self, LaunchRequest};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;
use arctic_core::versions::VersionManifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let root = args.first().ok_or("expected an isolated data root")?;
    let version = args.get(1).ok_or("expected a game version")?;
    let dirs = DataDirs::new(root);
    dirs.ensure()?;
    let manifest = VersionManifest::fetch(&dirs)?;
    let entry = manifest.find(version).ok_or("version not in manifest")?;
    let mut instance = instances::load_default(&dirs)?;
    let value = |name: &str| {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    match value("--profile") {
        None => {
            instance.loader = Loader::Fabric {
                version: "0.20.1".into(),
            };
            instance.performance = flag("--performance");
            instance.arctic_mod = !flag("--without-client");
        }
        Some("vanilla") => {
            instance.loader = Loader::Vanilla;
            instance.performance = false;
            instance.arctic_mod = false;
        }
        Some("fabric") => {
            let version = arctic_core::arctic_mod::client_loader(&dirs, &entry.id)
                .ok_or("no Fabric loader for this version")?;
            instance.loader = Loader::Fabric { version };
            instance.performance = false;
            instance.arctic_mod = false;
        }
        Some("arctic") => {
            instance.loader = Loader::Vanilla;
            instance.performance = true;
            instance.arctic_mod = true;
        }
        Some(other) => return Err(format!("unknown profile {other}").into()),
    }
    instance.max_memory_mb = Some(2048);
    instance.java_path = std::env::var_os("ARCTIC_BENCH_JAVA").map(PathBuf::from);
    let account = offline::create("StartupOffline")?;
    let settings = Settings::default();
    let started = Instant::now();
    let plan = launch::prepare(
        &LaunchRequest {
            dirs: &dirs,
            version: entry,
            instance: &instance,
            account: &account,
            settings: &settings,
            bridge: None,
            copy: 0,
            quick_play: None,
        },
        &|_| {},
    )?;
    println!("BENCH prepare_ms={}", started.elapsed().as_millis());
    if let Some(i) = args.iter().position(|a| a == "--command-out") {
        let out = args.get(i + 1).ok_or("--command-out needs a file")?;
        let mut command = vec![plan.java.display().to_string()];
        command.extend(plan.args.iter().cloned());
        std::fs::write(out, serde_json::to_string(&command)?)?;
        println!("BENCH game_dir={}", plan.game_dir.display());
    }
    if flag("--spawn") {
        let spawn = Instant::now();
        let (seen, window) = std::sync::mpsc::channel();
        let game = launch::spawn(&plan, move |event| {
            if matches!(
                event,
                launch::GameEvent::WindowReady | launch::GameEvent::Exited { .. }
            ) {
                let _ = seen.send(event);
            }
        })?;
        println!(
            "BENCH spawn_ms={} total_ms={}",
            spawn.elapsed().as_millis(),
            started.elapsed().as_millis()
        );
        if let Some(seconds) = value("--window") {
            let wait = std::time::Duration::from_secs(seconds.parse()?);
            match window.recv_timeout(wait) {
                Ok(launch::GameEvent::WindowReady) => println!(
                    "BENCH window_ms={} play_to_window_ms={}",
                    spawn.elapsed().as_millis(),
                    started.elapsed().as_millis()
                ),
                Ok(event) => println!("BENCH window=none ({event:?})"),
                Err(_) => println!("BENCH window=none (timeout)"),
            }
        }
        // The game-to-title time is measured separately; this covers only Play-to-process work.
        game.terminate();
    }
    // A launcher keeps running while music and other languages download and the assets are
    // noted as checked; the next launch of a bench that left before that would check them all again.
    let vanilla = arctic_core::versions::load_version(&dirs, entry, &|_| {})?;
    if let Some(index) = &vanilla.asset_index {
        let waited = Instant::now();
        while !launch::files::assets_verified_recently(&dirs, &index.id)
            && waited.elapsed().as_secs() < 300
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    Ok(())
}
