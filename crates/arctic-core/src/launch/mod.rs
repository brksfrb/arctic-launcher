//! Launch pipeline: resolve files → download → build command → spawn.
//!
//! `prepare` does all the slow work (downloads) and returns a `LaunchPlan`;
//! `process::spawn` starts Minecraft as a child process and watches it from
//! background threads.

pub mod args;
pub mod files;
mod game_window;
mod language;
pub mod logparse;
#[cfg(any(windows, test))]
mod named_runtime;
mod priority;
pub mod process;
pub mod progress;
#[cfg(test)]
mod repair_tests;
mod runtime_repair;
pub(crate) mod session;
pub mod startup;
pub mod startup_cache;
mod truststore;

pub use process::{GameEvent, GameHandle, spawn, spawn_detached};

use crate::proxy::ProxySettings;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::auth::{Account, LaunchIdentity};
use crate::error::IoContext;
use crate::instances::Instance;
use crate::net::{DownloadJob, download_all};
use crate::settings::Settings;
use crate::storage::DataDirs;
use crate::versions::{RuleEnv, VersionEntry, VersionJson, load_version, mark_installed};
use crate::{
    APP_VERSION, Error, LAUNCHER_BRAND, Progress, ProgressInfo, Result, arctic_mod, java, loaders,
};

use self::args::{JvmOptions, Placeholders, split_jvm_args, substitute};

/// Everything needed to launch one version for one account.
#[derive(Clone)]
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
    managed_runtime: Option<PathBuf>,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub log_file: PathBuf,
    /// Run on the high-performance graphics card of laptops with two.
    pub high_performance_gpu: bool,
    /// Maximize the game window once it opens.
    pub maximize_window: bool,
    custom_max_heap: bool,
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

    /// How Java is started, for the game's log (and so for crash reports): the Java, its flags and
    /// the classpath's file names, without the game's own arguments (the account is in those).
    pub fn java_summary(&self) -> String {
        let java = self
            .java
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map_or_else(
                || self.java.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
        let mut flags = Vec::new();
        let mut classpath = Vec::new();
        // A stray word before `-cp` (a JVM argument split on a space) is not the main class.
        // Stopping there used to hide the classpath in crash reports.
        let cp_at = self
            .args
            .iter()
            .position(|a| a == "-cp" || a == "-classpath");
        let mut args = self.args.iter().enumerate();
        while let Some((i, arg)) = args.next() {
            if arg == "-cp" || arg == "-classpath" {
                classpath = args
                    .next()
                    .map(|(_, cp)| {
                        std::env::split_paths(cp)
                            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                            .collect()
                    })
                    .unwrap_or_default();
                continue;
            }
            if !arg.starts_with('-') {
                if cp_at.is_some_and(|at| i < at) {
                    continue;
                }
                // The main class: what follows is the game's.
                break;
            }
            flags.push(java_flag_without_path(arg));
        }
        let heap_source = if self.custom_max_heap {
            args::CUSTOM_MAX_HEAP_SOURCE
        } else {
            ""
        };
        format!(
            "Java {java}{heap_source}, flags: {} | classpath: {}",
            flags.join(" "),
            classpath.join(", ")
        )
    }
}

/// `-javaagent:C:/…/mods/arctic-mod.jar` is `-javaagent:arctic-mod.jar`: the flag and the file,
/// not where the player keeps it.
fn java_flag_without_path(arg: &str) -> String {
    let Some((name, value)) = arg.split_once(['=', ':']) else {
        return arg.to_owned();
    };
    let looks_like_path = value.contains(['/', '\\']);
    if !looks_like_path {
        return arg.to_owned();
    }
    let file = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let sep = &arg[name.len()..=name.len()];
    format!("{name}{sep}{file}")
}

/// A version whose files are all downloaded and laid out for launch.
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
    let repair = files::LibraryRepair::new(dirs)?;
    let installed = install_with_repair(dirs, version, game_dir, java_override, progress, &repair)?;
    repair.finish();
    Ok(installed)
}

fn install_with_repair(
    dirs: &DataDirs,
    version: VersionJson,
    game_dir: &Path,
    java_override: Option<PathBuf>,
    progress: Progress,
    repair: &files::LibraryRepair,
) -> Result<Installation> {
    let clock = std::time::Instant::now();
    fs::create_dir_all(game_dir).at(game_dir)?;
    // 1.8.9 lists this folder when a server sends a resource pack and crashes if it isn't there
    // ("Parameter 'directory' is not a directory"); later versions make it themselves.
    let server_packs = game_dir.join("server-resource-packs");
    if !server_packs.exists() {
        let _ = fs::create_dir_all(&server_packs);
    }
    let libs = files::resolve_libraries(dirs, &version, &RuleEnv::for_version(&version));
    let client = files::client_job(dirs, &version)?;
    let client_jar = client.dest.clone();
    let logging = files::logging_job(dirs, &version);

    progress(ProgressInfo::stage("Checking files"));
    let java_override = java_override.filter(|p| p.is_file());
    let (runtime, assets) = std::thread::scope(|scope| {
        let runtime = scope.spawn(|| match &java_override {
            Some(path) => java::managed_runtime(dirs, path)
                .map(|runtime| {
                    let name = runtime.file_name().unwrap().to_string_lossy();
                    let intel = cfg!(target_os = "macos")
                        .then(|| name.strip_suffix("-x64"))
                        .flatten();
                    java::plan_runtime_for(dirs, intel.unwrap_or(&name), intel.is_some())
                })
                .transpose(),
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
    let (mut runtime, assets) = (runtime?, assets?);
    log::info!(
        "install: runtime and asset plan done at {} ms",
        clock.elapsed().as_millis()
    );

    let mut jobs = libs.jobs;
    jobs.push(client);
    // After a crash that pointed at a damaged file: each library and the client checked in full.
    repair.check(&jobs, &libs.bootstrap, libs.main_class.as_ref())?;
    jobs.extend(logging.iter().map(|(job, _)| job.clone()));
    let replacements = runtime
        .as_mut()
        .map_or_else(Vec::new, |r| std::mem::take(&mut r.jobs));
    let assets_fresh = files::assets_verified_recently(dirs, &assets.index_name);
    if !assets_fresh {
        jobs.extend(assets.jobs.iter().cloned());
    }
    crate::net::download_all_with_replacements(
        "Downloading game files",
        jobs,
        replacements,
        progress,
    )?;
    files::verify_main_class(libs.main_class.as_ref())?;
    if !assets_fresh {
        finish_assets_later(dirs, assets.index_name.clone(), assets.later.clone());
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
    language::prepare(&client_jar, game_dir)?;
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

/// The assets the game can start without, fetched on a thread of their own while it runs; all of
/// the version's assets count as checked only once they're in (a launcher closed meanwhile
/// leaves the rest for the next start).
fn finish_assets_later(dirs: &DataDirs, index_name: String, later: Vec<DownloadJob>) {
    if later.is_empty() {
        files::mark_assets_verified(dirs, &index_name);
        return;
    }
    let dirs = dirs.clone();
    std::thread::spawn(
        move || match download_all("Music and languages", later, &|_| {}) {
            Ok(()) => files::mark_assets_verified(&dirs, &index_name),
            Err(e) => log::warn!("music and languages for {index_name}: {e}"),
        },
    );
}

/// Download everything and build the command. Safe to call from a worker thread.
///
/// For modded instances the vanilla version is installed first (the loader
/// installer needs its client jar and Java), then the loader profile is
/// layered on top and its extra libraries are downloaded.
pub fn prepare(req: &LaunchRequest, progress: Progress) -> Result<LaunchPlan> {
    prepare_in(req, &req.instance.game_dir(req.dirs), progress)
}

/// Prepare a game whose lifetime will be watched by the launcher.
pub fn prepare_watched(req: &LaunchRequest, progress: Progress) -> Result<WatchedLaunch> {
    let (session, copy) =
        session::GameSession::reserve(req.dirs, req.instance, req.settings.shared, req.copy)?;
    let req = LaunchRequest {
        copy,
        ..req.clone()
    };
    let plan = prepare_in(&req, session.game_dir(), progress)?;
    Ok(WatchedLaunch { plan, session })
}

pub struct WatchedLaunch {
    pub plan: LaunchPlan,
    session: session::GameSession,
}

impl WatchedLaunch {
    pub fn spawn(self, on_event: impl Fn(GameEvent) + Send + Sync + 'static) -> Result<GameHandle> {
        process::spawn_with_session(&self.plan, Some(self.session), on_event)
    }
}

fn prepare_in(req: &LaunchRequest, game_dir: &Path, progress: Progress) -> Result<LaunchPlan> {
    // "Play with Arctic Client" is off in Settings: the game starts as if the instance had no client
    // (the mod is taken out of its mods folder, no client flags or login sharing).
    if !req.settings.play_with_client && req.instance.arctic_mod {
        let mut instance = req.instance.clone();
        instance.arctic_mod = false;
        let without_client = LaunchRequest {
            dirs: req.dirs,
            version: req.version,
            instance: &instance,
            account: req.account,
            settings: req.settings,
            bridge: req.bridge,
            copy: req.copy,
            quick_play: req.quick_play.clone(),
        };
        return prepare_in(&without_client, game_dir, progress);
    }
    // "Gentle launch": few downloads at a time, and the game starts behind other programs.
    crate::net::set_gentle(req.settings.gentle_launch);
    priority::set_gentle(req.settings.gentle_launch);
    // A proxy that is on but unusable must stop the launch: starting
    // without it would quietly connect directly.
    let proxy = ProxySettings::try_load(req.dirs).map_err(|e| {
        crate::Error::Other(format!(
            "The proxy settings can't be read ({e}), so the game was not started without it. Set the proxy again in Settings > Network."
        ))
    })?;
    if proxy.enabled
        && let Err(why) = proxy.validate()
    {
        return Err(crate::Error::Other(format!(
            "The proxy is on but its settings are incomplete: {why}"
        )));
    }
    let dirs = req.dirs;
    let repair = files::LibraryRepair::new(dirs)?;
    let mut timer = StepTimer::new();
    // The Arctic session (a network round trip) is fetched while the game files are checked.
    let mut share_session = false;
    // First start of this game folder: the profile's default game settings.
    match crate::game_defaults::Defaults::load(dirs)
        .and_then(|d| d.apply_if_new(game_dir, Some(&req.version.id)))
    {
        Ok(true) => log::info!("applied default game settings to {}", game_dir.display()),
        Ok(false) => {}
        Err(e) => log::warn!("default game settings: {e}"),
    }
    timer.step("default settings");
    // What the profile's instances share: the newest copies, into this one.
    if let Err(e) = crate::shared::sync_at(
        dirs,
        req.settings.shared,
        Some((req.instance, &req.version.id, game_dir.to_path_buf())),
    ) {
        log::warn!("sharing between instances: {e}");
    }
    if let Err(e) = crate::game_defaults::skip_multiplayer_warning(game_dir) {
        log::warn!("multiplayer warning: {e}");
    }
    timer.step("shared files sync");
    let java_override = req
        .instance
        .java_path
        .clone()
        .or_else(|| req.settings.java_override.clone());
    progress(ProgressInfo::stage("Reading version"));
    let vanilla = load_version(dirs, req.version, &|_| {})?;
    let runtime = match java_override.as_ref().filter(|p| p.is_file()) {
        Some(java) => java::managed_runtime(dirs, java),
        None => Some(java::runtime_dir(
            dirs,
            java::component_for(&vanilla),
            crate::versions::rules::needs_rosetta(&vanilla),
        )?),
    };
    if let Some(runtime) = runtime {
        runtime_repair::recover_detached(&runtime, &game_log_file(req), game_dir);
    }
    // Going up to a newer game version upgrades worlds for good: back the recent ones up first.
    let backed_up = crate::worlds::backup_before_version_change(game_dir, &vanilla.id);
    if backed_up > 0 {
        log::info!(
            "backing up {backed_up} recent world(s) before {}",
            vanilla.id
        );
    }
    timer.step("read version");
    let launcher_owned_vanilla =
        req.instance.loader.kind().is_none() && req.instance.game_folder.is_none();
    if launcher_owned_vanilla {
        crate::mods::performance::recover_vanilla_api(game_dir)?;
    }
    let loader = effective_loader(dirs, req.instance, &vanilla.id, game_dir);
    timer.step("pick loader");
    let version = match loader
        .as_ref()
        .map(|(k, v)| (Some(*k), Some(v.as_str())))
        .unwrap_or((None, None))
    {
        (Some(kind), Some(loader_version)) => {
            let base = install_with_repair(
                dirs,
                vanilla.clone(),
                game_dir,
                java_override.clone(),
                progress,
                &repair,
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
                // Quilt provides its own Fabric compatibility version, not its loader version.
                let fits =
                    arctic_mod_fits(dirs, &vanilla.id, kind, loader_version, &profile, progress)?;
                if req.instance.arctic_mod && !fits {
                    log::warn!(
                        "{} Loader {loader_version} cannot provide the Fabric Loader version needed by the Arctic Client on {}; starting without it",
                        kind.label(),
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
                // Best effort, like the Forge branch: no Sodium is better than no game (an
                // unreachable Modrinth CDN, a locked or truncated index).
                if let Err(e) = crate::mods::performance::sync_for_launch(
                    game_dir,
                    &vanilla.id,
                    req.instance,
                    progress,
                ) {
                    log::warn!("performance mods: {e}");
                }
                if let Err(e) = crate::mods::polarium::sync(
                    &game_dir.join("mods"),
                    &vanilla.id,
                    req.instance.performance,
                ) {
                    log::warn!("polarium: {e}");
                }
                // Vanilla instances (nothing in the folder but the launcher's own mods): load only
                // the Fabric API modules those use, which starts the game noticeably faster.
                if req.instance.loader.kind().is_none() {
                    crate::mods::performance::finish_vanilla(
                        game_dir,
                        &vanilla.id,
                        launcher_owned_vanilla,
                        req.instance.performance,
                    )?;
                }
                timer.step("performance mods");
                share_session =
                    req.instance.arctic_mod && fits && arctic_mod::supports(&vanilla.id);
            } else if kind == loaders::LoaderKind::Forge {
                // Forge instances: the Arctic Client's Forge build, where there is one (1.8.9).
                arctic_mod::sync_forge(
                    &game_dir.join("mods"),
                    &vanilla.id,
                    req.instance.arctic_mod,
                )?;
                share_session = req.instance.arctic_mod && arctic_mod::supports_forge(&vanilla.id);
                timer.step("arctic client");
                // 1.8.9: Patcher and Entity Culling for FPS (the Performance switch); never stops a start.
                progress(ProgressInfo::stage("Checking performance mods"));
                if let Err(e) = crate::mods::performance::sync_forge(
                    game_dir,
                    &vanilla.id,
                    req.instance.performance,
                    progress,
                ) {
                    log::warn!("performance mods for Forge: {e}");
                }
                timer.step("performance mods");
            }
            profile
        }
        _ => {
            if launcher_owned_vanilla
                && let Err(e) =
                    crate::mods::performance::finish_vanilla(game_dir, &vanilla.id, true, false)
            {
                log::warn!("vanilla performance cleanup: {e}");
            }
            vanilla
        }
    };
    // The Arctic sign-in (a network round trip) runs while the game files are checked.
    let session = share_session.then(|| start_cosmetics_session(req, game_dir));
    let installation =
        install_with_repair(dirs, version, game_dir, java_override, progress, &repair)?;
    if let Some(finished) = session {
        let _ = finished.recv_timeout(SESSION_WAIT);
    }
    timer.step("game files + arctic session");
    if proxy.active().is_some() {
        let hosts = game_dir.join(crate::proxy::HOSTS_FILE);
        std::fs::write(&hosts, proxy.hosts_file()).map_err(|e| crate::Error::io(&hosts, e))?;
    }
    let plan = plan(req, &installation);
    repair.finish();
    timer.step("JVM arguments + startup cache");
    timer.done();
    Ok(plan)
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

/// Imports and shares keep the chosen Quilt loader; check its actual compatibility at launch.
fn arctic_mod_fits(
    dirs: &DataDirs,
    game: &str,
    kind: loaders::LoaderKind,
    loader_version: &str,
    profile: &VersionJson,
    progress: Progress,
) -> Result<bool> {
    if kind != loaders::LoaderKind::Quilt {
        return Ok(arctic_mod::loader_fits(game, loader_version));
    }
    if arctic_mod::min_loader(game).is_none() {
        return Ok(true);
    }
    let libs = files::resolve_libraries(dirs, profile, &RuleEnv::for_version(profile));
    let Some((jar, _)) = libs.main_class else {
        return Ok(false);
    };
    let jobs = libs
        .jobs
        .into_iter()
        .filter(|job| job.dest == jar)
        .collect();
    download_all("Checking Quilt compatibility", jobs, progress)?;
    Ok(arctic_mod::quilt_loader_fits(game, &jar))
}

/// The loader to launch with. Vanilla instances run on Fabric, invisible
/// to the player, for the Arctic Client (versions it supports) and the
/// Performance mods (any version Fabric supports), unless both are off
/// ("pure vanilla").
fn effective_loader(
    dirs: &DataDirs,
    instance: &Instance,
    game: &str,
    game_dir: &Path,
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
        let _ = arctic_mod::sync(&game_dir.join("mods"), game, false);
        if let Err(e) =
            crate::mods::performance::sync(game_dir, game, false, false, &[], None, &|_| {})
        {
            log::warn!("vanilla performance cleanup: {e}");
        }
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

/// LWJGL's per-thread scratch stack in KiB (its default is 64). Vulkan (VulkanMod) lists the
/// graphics card's extensions into it at start, and with many layers or drivers they don't fit:
/// "OutOfMemoryError: Out of stack space" before the window opens. LWJGL 2 ignores it.
const LWJGL_STACK_KIB: u32 = 1024;

fn lwjgl_stack_flag(launcher_args: &str, instance_args: &str) -> Option<String> {
    let own = split_jvm_args(launcher_args)
        .into_iter()
        .chain(split_jvm_args(instance_args))
        .any(|a| a.starts_with("-Dorg.lwjgl.system.stackSize="));
    (!own).then(|| format!("-Dorg.lwjgl.system.stackSize={LWJGL_STACK_KIB}"))
}

/// A player JVM argument Java can be given. Anything else is a word Java takes as the main class
/// (`Could not find or load main class Forge`), so it is dropped. `@file` is Java's argument file.
fn user_jvm_arg_allowed(arg: &str) -> bool {
    if arg.starts_with('-') || arg.starts_with('@') {
        return true;
    }
    log::warn!("ignoring JVM argument {arg:?}: Java would treat it as the main class");
    false
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

/// How long Play waits for the Arctic sign-in after the game files are ready. A slow or unreachable Arctic
/// server must not hold the game back; the sign-in goes on in the background and the game picks the token
/// up from the session file when it lands.
const SESSION_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// Start telling the Arctic mod its menu style and letting it publish looks as this player, on a thread of
/// its own. The receiver gets a message when the session file is final.
fn start_cosmetics_session(req: &LaunchRequest, game_dir: &Path) -> std::sync::mpsc::Receiver<()> {
    let (done, finished) = std::sync::mpsc::channel();
    let dirs = req.dirs.clone();
    let account = req.account.clone();
    let settings = req.settings.clone();
    let bridge = req.bridge.cloned();
    let quick_play = req.quick_play.clone();
    let game_dir = game_dir.to_path_buf();
    std::thread::spawn(move || {
        share_cosmetics_session(
            &dirs,
            &account,
            &settings,
            bridge.as_ref(),
            quick_play.as_ref(),
            &game_dir,
        );
        let _ = done.send(());
    });
    finished
}

/// Tell the Arctic mod its menu style and let it publish looks as this
/// player. Best effort: without the cosmetics server the game still starts.
/// The session file is written first without a token, so the game always finds a current one, and again
/// when the sign-in has given a token.
fn share_cosmetics_session(
    dirs: &DataDirs,
    account: &Account,
    settings: &Settings,
    bridge: Option<&crate::bridge::BridgeInfo>,
    quick_play: Option<&QuickPlay>,
    game_dir: &Path,
) {
    let base = crate::cosmetics::base_url();
    let proxy = ProxySettings::load(dirs);
    let write = |token: Option<&str>| {
        let session = crate::cosmetics::ModSession {
            base: &base,
            token,
            style: settings.client_style,
            fancy: settings.client_fancy,
            style_set: settings.client_style_set,
            share_server: settings.share_server_with_friends,
            proxy: &proxy,
            bridge,
            duel: quick_play.and_then(QuickPlay::duel_json),
            replay: quick_play.and_then(QuickPlay::replay_json),
            language: if settings.language == "system" {
                ""
            } else {
                &settings.language
            },
        };
        if let Err(e) = crate::cosmetics::write_mod_session(game_dir, &session) {
            log::warn!("arctic session file: {e}");
        }
    };
    write(None);
    let token = crate::cosmetics::token_for(dirs, &base, account)
        .inspect_err(|e| log::info!("Arctic cosmetics unavailable: {e}"))
        .ok();
    if token.is_some() {
        write(token.as_deref());
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
    // Quoted values stay one argument (`-Dx="1.8.9 Forge"`). A leftover word is not passed on:
    // Java would take it as the main class.
    let user_args = split_jvm_args(&settings.extra_jvm_args)
        .into_iter()
        .chain(split_jvm_args(&req.instance.jvm_args))
        .filter(|arg| user_jvm_arg_allowed(arg))
        .collect::<Vec<_>>();
    let custom_max_heap = user_args
        .iter()
        .rev()
        .find_map(|arg| arg.strip_prefix("-Xmx"));
    let custom_memory_mb = custom_max_heap.and_then(args::heap_size_mb);
    let mods = if req.instance.max_memory_mb.is_none() || custom_max_heap.is_some() {
        mod_count(&inst.game_dir.join("mods"))
    } else {
        0
    };
    let asked = match req.instance.max_memory_mb {
        Some(own) => own,
        None => {
            let asked = crate::system::memory_for_mods(settings.max_memory_mb, mods);
            if asked > settings.max_memory_mb && custom_max_heap.is_none() {
                log::info!(
                    "{mods} mods: giving the game {asked} MB (more than the {} MB setting)",
                    settings.max_memory_mb
                );
            }
            asked
        }
    };
    let max_memory_mb = crate::system::playable_memory_mb(asked);
    if let Some(size) = custom_max_heap {
        log::warn!(
            "JVM arguments set -Xmx{size}, overriding the launcher memory setting ({max_memory_mb} MB)"
        );
        // Use the pack's recommendation independently of the player's memory settings.
        let recommended = crate::system::memory_for_mods(0, mods);
        if let Some(custom_mb) = custom_memory_mb.filter(|&mb| mb < recommended) {
            log::warn!(
                "JVM arguments limit the heap to {custom_mb} MB, below the recommended {recommended} MB for {mods} mods"
            );
        }
    } else if max_memory_mb < asked {
        log::warn!(
            "memory setting {asked} MB is more than this computer can spare: using {max_memory_mb} MB"
        );
    }
    let native_x86_64 =
        java::native_x86_64() && !crate::versions::rules::needs_rosetta(&inst.version);
    let java_major = if !native_x86_64 {
        None
    } else if req.instance.java_path.is_some() || settings.java_override.is_some() {
        process::runtime_major_version(&inst.java)
    } else {
        inst.version.java_version.as_ref().map(|j| j.major_version)
    };
    let opts = JvmOptions {
        min_memory_mb: settings.min_memory_mb.min(max_memory_mb),
        max_memory_mb,
        extra: args::java4shell_flags(java_major, native_x86_64, &user_args)
            .iter()
            .map(|&arg| arg.to_owned())
            .chain(user_args.iter().cloned())
            .chain(arctic_mod::jvm_flag())
            .chain(arctic_mod::brand_flag(&req.instance.loader))
            .chain(Some(arctic_mod::version_flag()))
            // Capture anew for every launch, including after a DST/OS zone change.
            .chain(Some(format!(
                "-Darctic.localUtcOffsetSeconds={}",
                chrono::Local::now().offset().local_minus_utc()
            )))
            .chain(gc_flags(
                user_args.iter().map(String::as_str),
                // The player's own Java may be any version: only the launcher's runtimes get ZGC.
                (req.instance.java_path.is_none() && settings.java_override.is_none())
                    .then(|| inst.version.java_version.as_ref().map(|j| j.major_version))
                    .flatten(),
                std::thread::available_parallelism().map_or(1, |n| n.get()),
                custom_memory_mb.unwrap_or(max_memory_mb),
            ))
            .chain(fast_start_flags(req.instance))
            .chain(lwjgl_stack_flag(
                &settings.extra_jvm_args,
                &req.instance.jvm_args,
            ))
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
            .chain(
                proxy
                    .active()
                    .into_iter()
                    .flat_map(ProxySettings::jvm_flags),
            )
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
        managed_runtime: java::managed_runtime(req.dirs, &inst.java),
        log_file: game_log_file(req),
        high_performance_gpu: settings.high_performance_gpu,
        maximize_window: settings.game_maximized && !settings.fullscreen,
        custom_max_heap: custom_max_heap.is_some(),
        secrets: vec![identity.access_token, proxy.password.clone()],
    }
}

fn game_log_file(req: &LaunchRequest) -> PathBuf {
    let logs = req.dirs.logs();
    if req.copy == 0 {
        logs.join(format!("game-{}.log", req.instance.id))
    } else {
        logs.join("copies")
            .join(&req.instance.id)
            .join(format!("game-{}.log", req.copy))
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

/// Mods that will load (`.jar` files in the mods folder; switched-off ones end in `.disabled`).
/// Jars in a mods folder.
pub fn mod_count(mods: &Path) -> usize {
    std::fs::read_dir(mods).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("jar"))
            })
            .count()
    })
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
    fn quilt_client_fit_reads_the_selected_jar_and_removes_an_incompatible_client() {
        use loaders::LoaderKind;
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let game = "1.21.11";
        assert_eq!(arctic_mod::min_loader(game).as_deref(), Some("0.19.5"));
        // https://github.com/QuiltMC/quilt-loader/blob/0.30.0-beta.3/src/main/resources/quilt.mod.json
        let metadata = include_str!("../../tests/fixtures/quilt-loader-0.30.0-beta.3.mod.json");
        let profile: VersionJson = serde_json::from_value(serde_json::json!({
            "id": "quilt-loader-0.30.0-beta.3-1.21.11",
            "mainClass": "org.quiltmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{"name": "org.quiltmc:quilt-loader:0.30.0-beta.3", "url": "https://example.invalid/"}]
        })).unwrap();
        let jar = dirs
            .libraries()
            .join(crate::versions::maven_path("org.quiltmc:quilt-loader:0.30.0-beta.3").unwrap());
        fs::create_dir_all(jar.parent().unwrap()).unwrap();
        let mods = dir.path().join("mods");
        for (metadata, expected) in [
            (metadata.to_owned(), false),
            // Synthetic future compatibility declaration; Quilt's version stays unchanged.
            (metadata.replace("0.18.4", "0.19.5"), true),
            (metadata.replace("fabricloader", "other"), false),
            ("{}".to_owned(), false),
            ("broken metadata".to_owned(), false),
        ] {
            let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
            zip.start_file("quilt.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(metadata.as_bytes()).unwrap();
            zip.finish().unwrap();
            let fits = arctic_mod_fits(
                &dirs,
                game,
                LoaderKind::Quilt,
                "0.30.0-beta.3",
                &profile,
                &|_| {},
            )
            .unwrap();
            assert_eq!(fits, expected);
            arctic_mod::sync(&mods, game, true).unwrap();
            assert!(mods.join(arctic_mod::FILE_NAME).exists());
            arctic_mod::sync(&mods, game, fits).unwrap();
            assert_eq!(mods.join(arctic_mod::FILE_NAME).exists(), expected);
        }
        assert!(
            !arctic_mod_fits(&dirs, game, LoaderKind::Fabric, "0.18.4", &profile, &|_| {}).unwrap()
        );
        assert!(
            arctic_mod_fits(&dirs, game, LoaderKind::Fabric, "0.19.5", &profile, &|_| {}).unwrap()
        );
        assert!(arctic_mod::quilt_loader_fits(
            "unsupported-game",
            Path::new("missing.jar")
        ));
        assert!(!arctic_mod::quilt_loader_fits(
            game,
            Path::new("missing.jar")
        ));
    }

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
    fn launch_uses_each_last_custom_heap_flag_once() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let account = Account {
            id: "test".into(),
            username: "Test".into(),
            uuid: "0".into(),
            kind: crate::auth::AccountKind::Offline,
        };
        let version = VersionEntry {
            id: "1.20.1".into(),
            kind: crate::versions::VersionKind::Release,
            url: String::new(),
            release_time: String::new(),
            sha1: String::new(),
        };
        let settings = Settings {
            max_memory_mb: 6144,
            extra_jvm_args: "-Xms768M -Xmx5G -XX:+UseG1GC".into(),
            ..Settings::default()
        };
        let instance = Instance {
            arctic_mod: false,
            jvm_args: "-Xmx4G -Xms1G -Xmx3584M".into(),
            ..Instance::vanilla_default()
        };
        let inst = Installation {
            version: serde_json::from_str(
                r#"{"id":"1.20.1","mainClass":"M","javaVersion":{"component":"java-runtime-gamma","majorVersion":17},"arguments":{"jvm":["-cp","${classpath}","-Xms512M","-Xmx6G"]}}"#,
            )
            .unwrap(),
            java: "java".into(),
            game_dir: temp.path().into(),
            classpath: Vec::new(),
            natives_dir: temp.path().into(),
            game_assets: temp.path().into(),
            asset_index: String::new(),
            logging: None,
        };
        let plan = plan(
            &LaunchRequest {
                dirs: &dirs,
                version: &version,
                instance: &instance,
                account: &account,
                settings: &settings,
                bridge: None,
                copy: 0,
                quick_play: None,
            },
            &inst,
        );
        let heap_flags: Vec<_> = plan
            .args
            .iter()
            .filter(|arg| arg.starts_with("-Xm"))
            .collect();
        assert_eq!(heap_flags, ["-Xms1G", "-Xmx3584M"]);
        assert!(plan.args.iter().any(|arg| arg == "-XX:+UseG1GC"));
        if java::native_x86_64() {
            assert_eq!(
                &plan.args[..3],
                [
                    "-XX:+UnlockDiagnosticVMOptions",
                    "-XX:-UseBASE64Intrinsics",
                    "-XX:+UseG1GC",
                ]
            );
        } else {
            assert!(
                !plan
                    .args
                    .iter()
                    .any(|arg| arg.contains("UseBASE64Intrinsics"))
            );
        }
        let summary = plan.java_summary();
        assert!(summary.contains(args::CUSTOM_MAX_HEAP_SOURCE));
        assert_eq!(crate::crash::max_memory_mb(&summary), Some(3584));
    }

    #[test]
    fn java_summary_names_flags_and_jars_but_not_folders_or_the_account() {
        let plan = LaunchPlan {
            java: PathBuf::from("C:/Users/me/runtimes/java-runtime-epsilon/bin/javaw.exe"),
            args: vec![
                "-Xmx8G".into(),
                "-javaagent:C:/Users/me/mods/arctic-mod.jar".into(),
                r"-Djava.library.path=C:\Users\me\natives".into(),
                "-cp".into(),
                // A drive-letter colon is a classpath separator on Unix.
                std::env::join_paths([
                    "/private/libs/sponge-mixin-0.17.4.jar",
                    "/private/libs/asm-9.10.jar",
                ])
                .unwrap()
                .to_string_lossy()
                .into_owned(),
                "net.fabricmc.loader.impl.launch.knot.KnotClient".into(),
                "--accessToken".into(),
                "SECRET123".into(),
            ],
            game_dir: PathBuf::new(),
            log_file: PathBuf::new(),
            high_performance_gpu: false,
            maximize_window: false,
            managed_runtime: None,
            custom_max_heap: false,
            secrets: vec!["SECRET123".into()],
        };
        let s = plan.java_summary();
        assert!(s.starts_with("Java java-runtime-epsilon"), "{s}");
        assert!(
            s.contains("-Xmx8G -javaagent:arctic-mod.jar -Djava.library.path=natives"),
            "{s}"
        );
        assert!(s.contains("sponge-mixin-0.17.4.jar, asm-9.10.jar"), "{s}");
        assert!(
            !s.contains("Users")
                && !s.contains("private")
                && !s.contains("SECRET123")
                && !s.contains("KnotClient"),
            "{s}"
        );
    }

    #[test]
    fn java_summary_finds_the_classpath_after_a_stray_token() {
        let plan = LaunchPlan {
            java: PathBuf::from("runtimes/jre-legacy/bin/javaw.exe"),
            args: vec![
                "-Xms512M".into(),
                "-Xmx3955M".into(),
                // What crash #212 left in front of every real flag.
                "Forge".into(),
                "-XX:+UseG1GC".into(),
                "-cp".into(),
                std::env::join_paths(["/libraries/launchwrapper-1.12.jar", "/versions/1.8.9.jar"])
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "net.minecraft.launchwrapper.Launch".into(),
                "--username".into(),
                "Steve".into(),
            ],
            game_dir: PathBuf::new(),
            log_file: PathBuf::new(),
            high_performance_gpu: false,
            maximize_window: false,
            managed_runtime: None,
            custom_max_heap: false,
            secrets: Vec::new(),
        };
        let s = plan.java_summary();
        assert!(s.contains("-XX:+UseG1GC"), "{s}");
        assert!(s.contains("launchwrapper-1.12.jar, 1.8.9.jar"), "{s}");
        assert!(!s.contains("Launch") && !s.contains("Steve"), "{s}");
    }

    #[test]
    fn a_bare_word_in_jvm_arguments_cannot_become_the_main_class() {
        let kept: Vec<_> = split_jvm_args(r#"-Dx="1.8.9 Forge" Forge 1.8.9 -Xmx4G @C:\args.txt"#)
            .into_iter()
            .filter(|arg| user_jvm_arg_allowed(arg))
            .collect();
        assert_eq!(kept, ["-Dx=1.8.9 Forge", "-Xmx4G", r"@C:\args.txt"]);
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
            managed_runtime: None,
            custom_max_heap: false,
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
            managed_runtime: None,
            custom_max_heap: false,
            secrets: vec!["tok".into(), "hunter2".into()],
        };
        assert!(plan.redacted_command().ends_with("--proxyPass <redacted>"));
    }

    #[test]
    fn lwjgl_gets_room_for_vulkans_lists_unless_the_player_chose() {
        assert_eq!(
            lwjgl_stack_flag("", "").as_deref(),
            Some("-Dorg.lwjgl.system.stackSize=1024")
        );
        assert_eq!(
            lwjgl_stack_flag("-Dorg.lwjgl.system.stackSize=256", ""),
            None
        );
        assert_eq!(
            lwjgl_stack_flag("", "-Xmx4G -Dorg.lwjgl.system.stackSize=512"),
            None
        );
    }
}
