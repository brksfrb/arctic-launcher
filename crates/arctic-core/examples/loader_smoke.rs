//! Live smoke test of mod-loader installation: runs the full `prepare`
//! pipeline for each case with an offline account and prints the command.
//! It never starts the game.
//!
//! ```text
//! cargo run -p arctic-core --example loader_smoke -- [kind:game[:loader] ...]
//! ```
//! `kind` is fabric, quilt, neoforge or forge; without `loader` the newest
//! stable (recommended) build is used. Without arguments a default set of
//! cases runs. Set `ARCTIC_DATA_DIR` to keep downloads out of your real
//! data folder.

use std::time::Instant;

use arctic_core::auth::offline;
use arctic_core::instances::{Instance, Loader};
use arctic_core::launch::{self, LaunchRequest};
use arctic_core::loaders::{self, LoaderKind};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;
use arctic_core::versions::VersionManifest;

const DEFAULT_CASES: [&str; 5] = [
    "fabric:1.21.4",
    "quilt:1.21.4",
    "neoforge:1.21.1",
    "forge:1.20.1",
    "forge:1.12.2:14.23.5.2860",
];

/// Main classes a loader profile may launch with.
const LOADER_MAIN_CLASSES: [&str; 5] = [
    "net.fabricmc.loader.impl.launch.knot.KnotClient",
    "org.quiltmc.loader.impl.launch.knot.KnotClient",
    "cpw.mods.bootstraplauncher.BootstrapLauncher",
    "net.neoforged.fml.startup.Client",
    "net.minecraft.launchwrapper.Launch",
];

type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> BoxResult<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cases: Vec<String> = if args.is_empty() {
        DEFAULT_CASES.iter().map(|s| s.to_string()).collect()
    } else {
        args
    };
    let dirs = DataDirs::resolve()?;
    dirs.ensure()?;
    println!("data dir: {}", dirs.root().display());
    let manifest = VersionManifest::fetch(&dirs)?;
    list_game_versions();

    let mut failures = 0;
    for case in &cases {
        println!("\n=== {case}");
        if let Err(e) = run_case(&dirs, &manifest, case) {
            failures += 1;
            println!("FAILED: {e}");
        }
    }
    println!(
        "\n{} of {} cases passed",
        cases.len() - failures,
        cases.len()
    );
    if failures > 0 {
        return Err(format!("{failures} case(s) failed").into());
    }
    Ok(())
}

fn parse_kind(s: &str) -> BoxResult<LoaderKind> {
    LoaderKind::ALL
        .into_iter()
        .find(|k| k.modrinth_id() == s)
        .ok_or_else(|| format!("unknown loader {s}").into())
}

fn run_case(dirs: &DataDirs, manifest: &VersionManifest, case: &str) -> BoxResult<()> {
    let mut parts = case.split(':');
    let kind = parse_kind(parts.next().unwrap_or_default())?;
    let game = parts.next().ok_or("missing game version")?.to_owned();
    let loader_version = match parts.next() {
        Some(v) => v.to_owned(),
        None => pick_loader_version(kind, &game)?,
    };
    println!("{} {loader_version} on Minecraft {game}", kind.label());

    let entry = manifest
        .find(&game)
        .ok_or(format!("unknown Minecraft version {game}"))?;
    let instance = smoke_instance(kind, &game, loader_version);
    let account = offline::create("ArcticTester")?;
    let settings = Settings::default();
    let started = Instant::now();
    let last_stage = std::sync::Mutex::new(String::new());
    let progress = |p: arctic_core::ProgressInfo| {
        if let Ok(mut last) = last_stage.lock()
            && *last != p.stage
        {
            println!("  {:>6.1}s {}", started.elapsed().as_secs_f64(), p.stage);
            *last = p.stage.to_owned();
        }
    };
    let plan = launch::prepare(
        &LaunchRequest {
            dirs,
            version: entry,
            instance: &instance,
            account: &account,
            settings: &settings,
        },
        &progress,
    )?;
    println!("prepared in {:.1?}", started.elapsed());
    let main = plan
        .args
        .iter()
        .find(|a| LOADER_MAIN_CLASSES.contains(&a.as_str()))
        .ok_or("the command does not launch a loader main class")?;
    println!("main class: {main}");
    println!("{}", plan.redacted_command());
    Ok(())
}

fn smoke_instance(kind: LoaderKind, game: &str, loader_version: String) -> Instance {
    let loader = Loader::new(Some(kind), loader_version);
    Instance {
        id: format!("smoke-{}-{game}", kind.modrinth_id()),
        name: format!("{} {game}", kind.label()),
        icon: loader.default_icon(),
        loader,
        version: Some(game.to_owned()),
        max_memory_mb: None,
        arctic_mod: false,
        java_path: None,
        jvm_args: String::new(),
    }
}

/// Print how long each loader's Minecraft version list takes to load.
fn list_game_versions() {
    for kind in LoaderKind::ALL {
        let started = Instant::now();
        match loaders::game_versions(kind) {
            Ok(list) => println!(
                "{:<8} {} Minecraft versions in {:.1?} (newest: {})",
                kind.label(),
                list.len(),
                started.elapsed(),
                list.first().map_or("-", String::as_str)
            ),
            Err(e) => println!("{:<8} version list failed: {e}", kind.label()),
        }
    }
}

/// Newest stable (recommended) build, else the newest one.
fn pick_loader_version(kind: LoaderKind, game: &str) -> BoxResult<String> {
    let started = Instant::now();
    let versions = loaders::loader_versions(kind, game)?;
    println!(
        "{} versions for {game}: {} listed in {:.1?}",
        kind.label(),
        versions.len(),
        started.elapsed()
    );
    versions
        .iter()
        .find(|v| v.stable)
        .or(versions.first())
        .map(|v| v.id.clone())
        .ok_or_else(|| format!("no {} builds for {game}", kind.label()).into())
}
