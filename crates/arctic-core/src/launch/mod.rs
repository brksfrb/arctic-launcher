//! Launch pipeline: resolve files → download → build command → spawn.
//!
//! `prepare` does all the slow work (downloads) and returns a `LaunchPlan`;
//! `process::spawn` starts Minecraft as a child process and watches it from
//! background threads.

pub mod args;
pub mod files;
mod game_window;
pub mod logparse;
pub mod process;
pub mod startup;
mod startup_cache;
mod truststore;

pub use process::{GameEvent, GameHandle, spawn, spawn_detached};

use crate::proxy::ProxySettings;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::auth::{Account, LaunchIdentity};
use crate::error::IoContext;
use crate::instances::Instance;
use crate::net::download_all;
use crate::settings::Settings;
use crate::storage::DataDirs;
use crate::versions::{RuleEnv, VersionEntry, VersionJson, load_version, mark_installed};
use crate::{
    APP_VERSION, Error, LAUNCHER_BRAND, Progress, ProgressInfo, Result, arctic_mod, java, loaders,
};

use self::args::{JvmOptions, Placeholders, substitute};

/// Everything needed to launch one version for one account.
pub struct LaunchRequest<'a> {
    pub dirs: &'a DataDirs,
    pub version: &'a VersionEntry,
    pub instance: &'a Instance,
    /// Must already be refreshed if it is a Microsoft account.
    pub account: &'a Account,
    pub settings: &'a Settings,
    /// Where the game can switch accounts (a running launcher), if anywhere.
    pub bridge: Option<&'a crate::bridge::BridgeInfo>,
    /// Which copy of this instance this is (0 when it's the only one
    /// running); later copies write their own log file.
    pub copy: u32,
    /// Where the game goes straight after starting, if anywhere.
    pub quick_play: Option<QuickPlay>,
}

/// Skip the title screen: join a server or open a world right away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPlay {
    /// A server address as typed in the game.
    Server(String),
    /// A world's folder name in `saves`.
    World(String),
    /// Start a duel from the title screen (Arctic Client): a flat arena
    /// with this kit, shared through Play together, with this friend
    /// (Arctic id, name) invited.
    Duel {
        kit: String,
        friend: Option<(String, String)>,
    },
    /// Watch a replay (an `.mcpr` file) from the title screen (Arctic Client).
    Replay(PathBuf),
}

impl QuickPlay {
    /// What the Arctic Client reads from its session file for a replay.
    pub fn replay_json(&self) -> Option<serde_json::Value> {
        let QuickPlay::Replay(file) = self else {
            return None;
        };
        Some(serde_json::json!({ "file": file, "at": unix_now() }))
    }

    /// What the Arctic Client reads from its session file for a duel.
    pub fn duel_json(&self) -> Option<serde_json::Value> {
        let QuickPlay::Duel { kit, friend } = self else {
            return None;
        };
        let now = unix_now();
        Some(serde_json::json!({
            "kit": kit,
            "friend": friend.as_ref().map(|f| &f.0),
            "friend_name": friend.as_ref().map(|f| &f.1),
            "at": now,
        }))
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A fully resolved command line, ready to spawn.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub log_file: PathBuf,
    /// Run on the high-performance graphics card of laptops with two.
    pub high_performance_gpu: bool,
    /// Maximize the game window once it opens.
    pub maximize_window: bool,
    /// Access token and proxy password, kept only to redact them from logs.
    secrets: Vec<String>,
}

impl LaunchPlan {
    /// The secrets worth hiding, longest first (empty and placeholder ones
    /// left out).
    fn log_secrets(&self) -> Vec<String> {
        let mut secrets: Vec<String> = self
            .secrets
            .iter()
            .filter(|s| !s.is_empty() && s.as_str() != "0")
            .cloned()
            .collect();
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        secrets
    }

    /// Command line safe to show or log (access token and proxy password
    /// replaced).
    pub fn redacted_command(&self) -> String {
        let secrets = self.log_secrets();
        let redact = |a: &String| {
            secrets
                .iter()
                .fold(a.clone(), |arg, s| arg.replace(s.as_str(), "<redacted>"))
        };
        std::iter::once(self.java.display().to_string())
            .chain(self.args.iter().map(redact))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A version whose files are all downloaded and laid out; command lines
/// for any account can be built from it without further I/O.
pub struct Installation {
    pub version: VersionJson,
    pub java: PathBuf,
    pub game_dir: PathBuf,
    /// Libraries + client jar, in classpath order.
    classpath: Vec<PathBuf>,
    natives_dir: PathBuf,
    game_assets: PathBuf,
    asset_index: String,
    /// (log config path, JVM argument template)
    logging: Option<(PathBuf, String)>,
}

/// Download (or verify) everything `entry` needs into the shared folders.
///
/// All missing files (Java runtime, libraries, client jar, assets, log
/// config) go through a single parallel download queue; the small metadata
/// needed to list them is fetched concurrently first. Safe to call from a
/// worker thread.
pub fn install(
    dirs: &DataDirs,
    version: VersionJson,
    game_dir: &Path,
    java_override: Option<PathBuf>,
    progress: Progress,
) -> Result<Installation> {
    let clock = std::time::Instant::now();
    fs::create_dir_all(game_dir).at(game_dir)?;
    let libs = files::resolve_libraries(dirs, &version, &RuleEnv::for_version(&version));
    let client = files::client_job(dirs, &version)?;
    let client_jar = client.dest.clone();
    let logging = files::logging_job(dirs, &version);

    progress(ProgressInfo::stage("Checking files"));
    let java_override = java_override.filter(|p| p.is_file());
    let (runtime, assets) = std::thread::scope(|scope| {
        let runtime = scope.spawn(|| match &java_override {
            Some(_) => Ok(None),
            None => java::plan_runtime_for(
                dirs,
                java::component_for(&version),
                crate::versions::rules::needs_rosetta(&version),
            )
            .map(Some),
        });
        let assets = files::plan_assets(dirs, &version, game_dir);
        let runtime = runtime
            .join()
            .unwrap_or_else(|_| Err(Error::Other("Java runtime check crashed".into())));
        (runtime, assets)
    });
    let (runtime, assets) = (runtime?, assets?);
    log::info!(
        "install: runtime and asset plan done at {} ms",
        clock.elapsed().as_millis()
    );

    let mut jobs = libs.jobs;
    jobs.push(client);
    jobs.extend(logging.iter().map(|(job, _)| job.clone()));
    jobs.extend(runtime.iter().flat_map(|r| r.jobs.iter().cloned()));
    let assets_fresh = files::assets_verified_recently(dirs, &assets.index_name);
    if !assets_fresh {
        jobs.extend(assets.jobs.iter().cloned());
    }
    download_all("Downloading game files", jobs, progress)?;
    if !assets_fresh {
        files::mark_assets_verified(dirs, &assets.index_name);
    }
    log::info!(
        "install: files checked at {} ms",
        clock.elapsed().as_millis()
    );

    progress(ProgressInfo::stage("Preparing"));
    let java = match (runtime, java_override) {
        (Some(plan), _) => plan.finish()?,
        (None, Some(path)) => path,
        (None, None) => unreachable!("runtime is planned whenever there is no override"),
    };
    let natives_dir = dirs.version_dir(&version.id).join("natives");
    files::extract_natives(&libs.natives, &natives_dir)?;
    assets.finish()?;
    mark_installed(dirs, &version.id)?;
    log::info!("install: done at {} ms", clock.elapsed().as_millis());

    let mut classpath = libs.classpath;
    classpath.push(client_jar);
    Ok(Installation {
        java,
        game_dir: game_dir.to_path_buf(),
        classpath,
        natives_dir,
        game_assets: assets.game_assets.clone(),
        asset_index: assets.index_name.clone(),
        logging: logging.map(|(job, template)| (job.dest, template)),
        version,
    })
}

/// Download everything and build the command. Safe to call from a worker thread.
///
/// For modded instances the vanilla version is installed first (the loader
/// installer needs its client jar and Java), then the loader profile is
/// layered on top and its extra libraries are downloaded.
pub fn prepare(req: &LaunchRequest, progress: Progress) -> Result<LaunchPlan> {
    // A proxy that is on but unusable must stop the launch: starting
    // without it would quietly connect directly.
    let proxy = ProxySettings::load(req.dirs);
    if proxy.enabled
        && let Err(why) = proxy.validate()
    {
        return Err(crate::Error::Other(format!(
            "The proxy is on but its settings are incomplete: {why}"
        )));
    }
    let dirs = req.dirs;
    let game_dir = req.instance.game_dir(dirs);
    let mut timer = StepTimer::new();
    // The Arctic session (a network round trip) is fetched while the game files are checked.
    let mut share_session = false;
    // First start of this game folder: the profile's default game settings.
    match crate::game_defaults::Defaults::load(dirs)
        .and_then(|d| d.apply_if_new(&game_dir, Some(&req.version.id)))
    {
        Ok(true) => log::info!("applied default game settings to {}", game_dir.display()),
        Ok(false) => {}
        Err(e) => log::warn!("default game settings: {e}"),
    }
    // What the profile's instances share: the newest copies, into this one.
    if let Err(e) = crate::shared::sync(
        dirs,
        req.settings.shared,
        Some((req.instance, &req.version.id)),
    ) {
        log::warn!("sharing between instances: {e}");
    }
    let java_override = req
        .instance
        .java_path
        .clone()
        .or_else(|| req.settings.java_override.clone());
    progress(ProgressInfo::stage("Reading version"));
    let vanilla = load_version(dirs, req.version, &|_| {})?;
    timer.step("read version");
    let loader = effective_loader(dirs, req.instance, &vanilla.id);
    timer.step("pick loader");
    let version = match loader
        .as_ref()
        .map(|(k, v)| (Some(*k), Some(v.as_str())))
        .unwrap_or((None, None))
    {
        (Some(kind), Some(loader_version)) => {
            let base = install(
                dirs,
                vanilla.clone(),
                &game_dir,
                java_override.clone(),
                progress,
            )?;
            timer.step("vanilla files");
            let stage = format!("Installing {} {loader_version}", kind.label());
            progress(ProgressInfo::stage(&stage));
            let profile = loaders::install_profile(
                dirs,
                kind,
                loader_version,
                &vanilla,
                &base.java,
                progress,
            )?;
            timer.step("loader profile");
            if matches!(
                kind,
                loaders::LoaderKind::Fabric | loaders::LoaderKind::Quilt
            ) {
                // An older Fabric Loader than the mod needs would stop the
                // game at startup: play without the Arctic Client then.
                let fits = kind != loaders::LoaderKind::Fabric
                    || arctic_mod::loader_fits(&vanilla.id, loader_version);
                if req.instance.arctic_mod && !fits {
                    log::warn!(
                        "Fabric Loader {loader_version} is too old for the Arctic Client on {}; starting without it",
                        vanilla.id
                    );
                }
                arctic_mod::sync(
                    &game_dir.join("mods"),
                    &vanilla.id,
                    req.instance.arctic_mod && fits,
                )?;
                timer.step("arctic client");
                // Vanilla instances (on Fabric underneath) and Fabric/Quilt
                // instances alike; in those, mods the player already has stay theirs.
                progress(ProgressInfo::stage("Checking performance mods"));
                crate::mods::performance::sync(
                    &game_dir,
                    &vanilla.id,
                    req.instance.performance,
                    req.instance.shaders,
                    req.instance.loader.kind().is_some(),
                    progress,
                )?;
                crate::mods::polarium::sync(
                    &game_dir.join("mods"),
                    &vanilla.id,
                    req.instance.performance,
                )?;
                // Vanilla instances (nothing in the folder but the launcher's own mods): load only
                // the Fabric API modules those use, which starts the game noticeably faster.
                if req.instance.performance
                    && req.instance.loader.kind().is_none()
                    && let Err(e) = crate::mods::slim_api::apply(&game_dir.join("mods"))
                {
                    log::warn!("couldn't cut Fabric API down: {e}");
                }
                timer.step("performance mods");
                share_session = req.instance.arctic_mod && arctic_mod::supports(&vanilla.id);
            }
            profile
        }
        _ => vanilla,
    };
    let installation = std::thread::scope(|scope| {
        let session =
            share_session.then(|| scope.spawn(|| share_cosmetics_session(req, &game_dir)));
        let installed = install(dirs, version, &game_dir, java_override, progress);
        if let Some(handle) = session {
            let _ = handle.join();
        }
        installed
    })?;
    timer.step("game files + arctic session");
    timer.done();
    if proxy.active().is_some() {
        let hosts = game_dir.join(crate::proxy::HOSTS_FILE);
        std::fs::write(&hosts, proxy.hosts_file()).map_err(|e| crate::Error::io(&hosts, e))?;
    }
    Ok(plan(req, &installation))
}

/// How long each part of getting ready took, logged when done (to find slow starts).
struct StepTimer {
    start: std::time::Instant,
    last: std::time::Instant,
    steps: Vec<String>,
}

impl StepTimer {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            start: now,
            last: now,
            steps: Vec::new(),
        }
    }

    fn step(&mut self, name: &str) {
        let now = std::time::Instant::now();
        self.steps
            .push(format!("{name} {}ms", (now - self.last).as_millis()));
        self.last = now;
    }

    fn done(&self) {
        log::info!(
            "ready to start in {}ms: {}",
            self.start.elapsed().as_millis(),
            self.steps.join(", ")
        );
    }
}

/// The loader to launch with. Vanilla instances run on Fabric, invisible
/// to the player, for the Arctic Client (versions it supports) and the
/// Performance mods (any version Fabric supports), unless both are off
/// ("pure vanilla").
fn effective_loader(
    dirs: &DataDirs,
    instance: &Instance,
    game: &str,
) -> Option<(loaders::LoaderKind, String)> {
    if let (Some(kind), Some(version)) = (instance.loader.kind(), instance.loader.version()) {
        return Some((kind, version.to_owned()));
    }
    let client = instance.arctic_mod && arctic_mod::supports(game);
    let fabric = (client || instance.performance || instance.shaders)
        .then(|| arctic_mod::client_loader(dirs, game))
        .flatten();
    if fabric.is_none() {
        // Pure vanilla: make sure earlier Fabric runs left nothing behind.
        let game_dir = instance.game_dir(dirs);
        let _ = crate::mods::slim_api::undo(&game_dir.join("mods"));
        let _ = arctic_mod::sync(&game_dir.join("mods"), game, false);
        let _ = crate::mods::performance::sync(&game_dir, game, false, false, false, &|_| {});
        let _ = crate::mods::polarium::sync(&game_dir.join("mods"), game, false);
    }
    fabric.map(|v| (loaders::LoaderKind::Fabric, v))
}

/// JVM flags that make the game start faster, for Vanilla instances (where the launcher
/// itself chose every mod): the JVM skips re-checking the game's and the mods' bytecode
/// as it loads classes, which saves over a second of every start. Not for instances with
/// the player's own mods, where a broken mod is better reported than run.
pub(crate) fn fast_start_flags(instance: &Instance) -> Vec<String> {
    if instance.loader.kind().is_some()
        || instance.jvm_args.contains("BytecodeVerification")
        || instance.jvm_args.contains("Xverify")
    {
        return Vec::new();
    }
    vec![
        "-XX:+UnlockDiagnosticVMOptions".to_owned(),
        "-XX:-BytecodeVerificationRemote".to_owned(),
    ]
}

/// The garbage collector Mojang's own launcher sets up: G1 with short pauses and room for the
/// game's bursts of short-lived objects. Without it Java 8 (Minecraft 1.16.5 and older) uses
/// the parallel collector, which stops the game for every collection: the regular FPS drops
/// (measured on 1.8.9: frame-dropping pauses down to about a third).
const G1_FLAGS: [&str; 6] = [
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
];

/// Generational ZGC collects while the game runs: on 1.21.11 it stopped the game over 16 ms
/// about a tenth as often as G1, at no cost in FPS. It needs Java 21 and spare cores for its
/// own threads, and some memory headroom.
const ZGC_MIN_JAVA: u32 = 21;
/// From Java 23 ZGC is always generational (the flag is deprecated, then gone).
const ZGC_ALWAYS_GENERATIONAL: u32 = 23;
const ZGC_MIN_THREADS: usize = 8;
const ZGC_MIN_MEMORY_MB: u32 = 3072;

/// The collector for a game: none of ours when the player chose one in their own flags; ZGC on a
/// Java the launcher picked (`java_major`, None for the player's own Java) that is new enough, on
/// a computer with `threads` and a heap of `memory_mb` that suit it; G1 otherwise.
fn gc_flags<'a>(
    user_flags: impl IntoIterator<Item = &'a str>,
    java_major: Option<u32>,
    threads: usize,
    memory_mb: u32,
) -> Vec<String> {
    let chose = user_flags
        .into_iter()
        .any(|f| f.starts_with("-XX:+Use") && f.ends_with("GC"));
    if chose {
        return Vec::new();
    }
    let zgc = java_major.is_some_and(|j| j >= ZGC_MIN_JAVA)
        && threads >= ZGC_MIN_THREADS
        && memory_mb >= ZGC_MIN_MEMORY_MB;
    if zgc {
        let mut flags = vec!["-XX:+UseZGC".to_owned()];
        if java_major.is_some_and(|j| j < ZGC_ALWAYS_GENERATIONAL) {
            flags.push("-XX:+ZGenerational".to_owned());
        }
        return flags;
    }
    G1_FLAGS.iter().map(|f| (*f).to_owned()).collect()
}

/// Tell the Arctic mod its menu style and let it publish looks as this
/// player. Best effort: without the cosmetics server the game still starts.
fn share_cosmetics_session(req: &LaunchRequest, game_dir: &Path) {
    let base = crate::cosmetics::base_url();
    let token = crate::cosmetics::token_for(req.dirs, &base, req.account)
        .inspect_err(|e| log::info!("Arctic cosmetics unavailable: {e}"))
        .ok();
    let settings = req.settings;
    let proxy = ProxySettings::load(req.dirs);
    let session = crate::cosmetics::ModSession {
        base: &base,
        token: token.as_deref(),
        style: settings.client_style,
        fancy: settings.client_fancy,
        style_set: settings.client_style_set,
        share_server: settings.share_server_with_friends,
        proxy: &proxy,
        bridge: req.bridge,
        duel: req.quick_play.as_ref().and_then(QuickPlay::duel_json),
        replay: req.quick_play.as_ref().and_then(QuickPlay::replay_json),
    };
    if let Err(e) = crate::cosmetics::write_mod_session(game_dir, &session) {
        log::warn!("arctic session file: {e}");
    }
}

/// Build the command line for `req` from a finished installation.
pub fn plan(req: &LaunchRequest, inst: &Installation) -> LaunchPlan {
    let settings = req.settings;
    let proxy = ProxySettings::load(req.dirs);
    let mut env = if settings.fullscreen {
        RuleEnv::for_version(&inst.version)
    } else {
        RuleEnv::for_version(&inst.version).with_feature("has_custom_resolution")
    };
    let identity = req.account.identity();
    let mut vars = placeholders(req, inst, &identity);
    let mut game_extra = proxy
        .active()
        .map(ProxySettings::game_args)
        .unwrap_or_default();
    if let Some(quick) = &req.quick_play {
        quick_play(quick, &inst.version, &mut env, &mut vars, &mut game_extra);
    }
    let logging_arg = inst.logging.as_ref().map(|(path, template)| {
        let vars = HashMap::from([("path", path.display().to_string())]);
        substitute(template, &vars)
    });
    // The Arctic Client loads the classes a start needs ahead of the game on other cores
    // (see startup_cache); it must be there to do it.
    let preload_flags = match inst.version.java_version.as_ref() {
        Some(java) if req.instance.arctic_mod && arctic_mod::supports(&req.version.id) => {
            startup_cache::flags(
                &inst.game_dir,
                &req.version.id,
                java.major_version,
                req.instance.loader.kind().is_none(),
            )
        }
        _ => Vec::new(),
    };
    let opts = JvmOptions {
        min_memory_mb: settings.min_memory_mb,
        max_memory_mb: req.instance.max_memory_mb.unwrap_or(settings.max_memory_mb),
        extra: settings
            .extra_jvm_args
            .split_whitespace()
            .chain(req.instance.jvm_args.split_whitespace())
            .map(str::to_owned)
            .chain(arctic_mod::jvm_flag())
            .chain(arctic_mod::brand_flag(&req.instance.loader))
            .chain(gc_flags(
                settings
                    .extra_jvm_args
                    .split_whitespace()
                    .chain(req.instance.jvm_args.split_whitespace()),
                // The player's own Java may be any version: only the launcher's runtimes get ZGC.
                (req.instance.java_path.is_none() && settings.java_override.is_none())
                    .then(|| inst.version.java_version.as_ref().map(|j| j.major_version))
                    .flatten(),
                std::thread::available_parallelism().map_or(1, |n| n.get()),
                req.instance.max_memory_mb.unwrap_or(settings.max_memory_mb),
            ))
            .chain(fast_start_flags(req.instance))
            .chain(
                (settings.game_maximized && !settings.fullscreen)
                    .then(|| game_window::CLIENT_FLAG.to_owned()),
            )
            .chain(preload_flags)
            .chain(truststore::flags(
                req.dirs,
                inst.version
                    .java_version
                    .as_ref()
                    .map_or(8, |j| j.major_version),
            ))
            .chain(proxy.active().map(|_| {
                format!(
                    "-Djdk.net.hosts.file={}",
                    inst.game_dir.join(crate::proxy::HOSTS_FILE).display()
                )
            }))
            .collect(),
        logging_arg,
        fullscreen: settings.fullscreen,
        resolution: (settings.window_width, settings.window_height),
        game_extra,
    };
    LaunchPlan {
        java: inst.java.clone(),
        args: args::build(&inst.version, &env, &vars, &opts),
        game_dir: inst.game_dir.clone(),
        log_file: req.dirs.logs().join(match req.copy {
            0 => format!("game-{}.log", req.instance.id),
            n => format!("game-{}-{}.log", req.instance.id, n + 1),
        }),
        high_performance_gpu: settings.high_performance_gpu,
        maximize_window: settings.game_maximized && !settings.fullscreen,
        secrets: vec![identity.access_token, proxy.password.clone()],
    }
}

/// Versions from 1.20 take `--quickPlay*`; older ones only know
/// `--server`/`--port` (and can't open a world directly).
fn quick_play(
    quick: &QuickPlay,
    version: &VersionJson,
    env: &mut RuleEnv,
    vars: &mut Placeholders,
    game_extra: &mut Vec<String>,
) {
    let (feature, key, value) = match quick {
        QuickPlay::Server(address) => {
            ("is_quick_play_multiplayer", "quickPlayMultiplayer", address)
        }
        QuickPlay::World(folder) => (
            "is_quick_play_singleplayer",
            "quickPlaySingleplayer",
            folder,
        ),
        // The game starts at its title screen; the Arctic Client does the rest.
        QuickPlay::Duel { .. } | QuickPlay::Replay(_) => return,
    };
    if takes_placeholder(version, key) {
        env.features.insert(feature);
        vars.insert(key, value.clone());
        return;
    }
    match quick {
        QuickPlay::Server(address) => {
            if let Some(a) = crate::servers::Address::parse(address) {
                game_extra.extend([
                    "--server".into(),
                    a.host,
                    "--port".into(),
                    a.port.to_string(),
                ]);
            }
        }
        QuickPlay::World(_) => log::info!("this version can't open a world directly"),
        QuickPlay::Duel { .. } | QuickPlay::Replay(_) => {}
    }
}

fn takes_placeholder(version: &VersionJson, key: &str) -> bool {
    use crate::versions::model::Argument;
    let needle = format!("${{{key}}}");
    version.arguments.as_ref().is_some_and(|a| {
        a.game.iter().any(|arg| match arg {
            Argument::Plain(s) => s.contains(&needle),
            Argument::Conditional { value, .. } => {
                value.values().into_iter().any(|s| s.contains(&needle))
            }
        })
    })
}

fn placeholders(
    req: &LaunchRequest,
    inst: &Installation,
    identity: &LaunchIdentity,
) -> Placeholders {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let path_str = |p: &Path| p.display().to_string();
    let classpath = inst
        .classpath
        .iter()
        .map(|p| path_str(p))
        .collect::<Vec<_>>()
        .join(sep);
    HashMap::from([
        ("auth_player_name", identity.username.clone()),
        ("auth_uuid", identity.uuid.clone()),
        ("auth_access_token", identity.access_token.clone()),
        ("auth_session", identity.access_token.clone()),
        ("auth_xuid", identity.xuid.clone()),
        ("clientid", "0".to_string()),
        ("user_type", identity.user_type.to_string()),
        ("user_properties", "{}".to_string()),
        ("version_name", inst.version.id.clone()),
        ("version_type", inst.version.kind.clone()),
        ("game_directory", path_str(&inst.game_dir)),
        ("assets_root", path_str(&req.dirs.assets())),
        ("game_assets", path_str(&inst.game_assets)),
        ("assets_index_name", inst.asset_index.clone()),
        ("natives_directory", path_str(&inst.natives_dir)),
        ("library_directory", path_str(&req.dirs.libraries())),
        ("classpath", classpath),
        ("classpath_separator", sep.to_string()),
        ("launcher_name", LAUNCHER_BRAND.to_string()),
        ("launcher_version", APP_VERSION.to_string()),
        ("resolution_width", req.settings.window_width.to_string()),
        ("resolution_height", req.settings.window_height.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_play_uses_the_versions_own_flags() {
        let modern: VersionJson = serde_json::from_str(
            r#"{"id":"1.21","mainClass":"M","arguments":{"jvm":[],"game":[
                {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],
                 "value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]}}"#,
        )
        .unwrap();
        let old: VersionJson =
            serde_json::from_str(r#"{"id":"1.8.9","mainClass":"M","minecraftArguments":"--x"}"#)
                .unwrap();
        let join = QuickPlay::Server("play.example.net:25570".into());
        let (mut env, mut vars, mut extra) = (RuleEnv::current(), Placeholders::new(), Vec::new());
        quick_play(&join, &modern, &mut env, &mut vars, &mut extra);
        assert!(env.features.contains("is_quick_play_multiplayer"));
        assert_eq!(vars["quickPlayMultiplayer"], "play.example.net:25570");
        assert!(extra.is_empty());

        let (mut env, mut vars) = (RuleEnv::current(), Placeholders::new());
        quick_play(&join, &old, &mut env, &mut vars, &mut extra);
        assert_eq!(extra, ["--server", "play.example.net", "--port", "25570"]);
        extra.clear();
        quick_play(
            &QuickPlay::World("w".into()),
            &old,
            &mut env,
            &mut vars,
            &mut extra,
        );
        assert!(extra.is_empty() && vars.is_empty());
    }

    #[test]
    fn the_player_choosing_a_collector_wins() {
        assert!(gc_flags(["-XX:+UseZGC", "-XX:+ZGenerational"], Some(21), 16, 8192).is_empty());
        assert!(gc_flags(["-XX:+UseShenandoahGC"], Some(8), 16, 8192).is_empty());
    }

    #[test]
    fn java_8_games_get_g1() {
        assert!(gc_flags(["-Xss2M"], Some(8), 16, 8192).contains(&"-XX:+UseG1GC".to_owned()));
    }

    #[test]
    fn new_java_on_a_roomy_computer_gets_zgc() {
        assert_eq!(
            gc_flags([], Some(21), 16, 4096),
            ["-XX:+UseZGC", "-XX:+ZGenerational"]
        );
        assert_eq!(gc_flags([], Some(25), 16, 4096), ["-XX:+UseZGC"]);
    }

    #[test]
    fn small_computers_and_the_players_own_java_keep_g1() {
        let g1 = |flags: Vec<String>| flags.contains(&"-XX:+UseG1GC".to_owned());
        assert!(g1(gc_flags([], Some(21), 4, 8192)));
        assert!(g1(gc_flags([], Some(21), 16, 2048)));
        assert!(g1(gc_flags([], None, 16, 8192)));
    }

    #[test]
    fn redacts_access_token() {
        let plan = LaunchPlan {
            java: PathBuf::from("java"),
            args: vec!["--accessToken".into(), "SECRET123".into()],
            game_dir: PathBuf::new(),
            log_file: PathBuf::new(),
            high_performance_gpu: false,
            maximize_window: false,
            secrets: vec!["SECRET123".into(), String::new()],
        };
        let cmd = plan.redacted_command();
        assert!(!cmd.contains("SECRET123"));
        assert!(cmd.ends_with("--accessToken <redacted>"));
    }

    #[test]
    fn redacts_proxy_password() {
        let plan = LaunchPlan {
            java: PathBuf::from("java"),
            args: vec!["--proxyPass".into(), "hunter2".into()],
            game_dir: PathBuf::new(),
            log_file: PathBuf::new(),
            high_performance_gpu: false,
            maximize_window: false,
            secrets: vec!["tok".into(), "hunter2".into()],
        };
        assert!(plan.redacted_command().ends_with("--proxyPass <redacted>"));
    }
}
