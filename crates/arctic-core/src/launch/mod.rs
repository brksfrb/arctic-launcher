//! Launch pipeline: resolve files → download → build command → spawn.
//!
//! `prepare` does all the slow work (downloads) and returns a `LaunchPlan`;
//! `process::spawn` starts Minecraft as a child process and watches it from
//! background threads.

pub mod args;
pub mod files;
pub mod logparse;
pub mod process;

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
}

/// A fully resolved command line, ready to spawn.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub log_file: PathBuf,
    /// Access token and proxy password, kept only to redact them from logs.
    secrets: Vec<String>,
}

impl LaunchPlan {
    /// Command line safe to show or log (access token and proxy password
    /// replaced).
    pub fn redacted_command(&self) -> String {
        let redact = |a: &String| {
            self.secrets
                .iter()
                .filter(|s| !s.is_empty() && s.as_str() != "0")
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
    fs::create_dir_all(game_dir).at(game_dir)?;
    let libs = files::resolve_libraries(dirs, &version, &RuleEnv::current());
    let client = files::client_job(dirs, &version)?;
    let client_jar = client.dest.clone();
    let logging = files::logging_job(dirs, &version);

    progress(ProgressInfo::stage("Checking files"));
    let java_override = java_override.filter(|p| p.is_file());
    let (runtime, assets) = std::thread::scope(|scope| {
        let runtime = scope.spawn(|| match &java_override {
            Some(_) => Ok(None),
            None => java::plan_runtime(dirs, java::component_for(&version)).map(Some),
        });
        let assets = files::plan_assets(dirs, &version, game_dir);
        let runtime = runtime
            .join()
            .unwrap_or_else(|_| Err(Error::Other("Java runtime check crashed".into())));
        (runtime, assets)
    });
    let (runtime, assets) = (runtime?, assets?);

    let mut jobs = libs.jobs;
    jobs.push(client);
    jobs.extend(logging.iter().map(|(job, _)| job.clone()));
    jobs.extend(runtime.iter().flat_map(|r| r.jobs.iter().cloned()));
    jobs.extend(assets.jobs.iter().cloned());
    download_all("Downloading game files", jobs, progress)?;

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
    let java_override = req
        .instance
        .java_path
        .clone()
        .or_else(|| req.settings.java_override.clone());
    progress(ProgressInfo::stage("Reading version"));
    let vanilla = load_version(dirs, req.version, &|_| {})?;
    let loader = effective_loader(dirs, req.instance, &vanilla.id);
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
                if req.instance.loader.kind().is_none() {
                    progress(ProgressInfo::stage("Checking performance mods"));
                    crate::mods::performance::sync(
                        &game_dir,
                        &vanilla.id,
                        req.instance.performance,
                        req.instance.shaders,
                        progress,
                    )?;
                }
                if req.instance.arctic_mod && arctic_mod::supports(&vanilla.id) {
                    share_cosmetics_session(req, &game_dir);
                }
            }
            profile
        }
        _ => vanilla,
    };
    let installation = install(dirs, version, &game_dir, java_override, progress)?;
    if proxy.active().is_some() {
        let hosts = game_dir.join(crate::proxy::HOSTS_FILE);
        std::fs::write(&hosts, proxy.hosts_file()).map_err(|e| crate::Error::io(&hosts, e))?;
    }
    Ok(plan(req, &installation))
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
        let _ = arctic_mod::sync(&game_dir.join("mods"), game, false);
        let _ = crate::mods::performance::sync(&game_dir, game, false, false, &|_| {});
    }
    fabric.map(|v| (loaders::LoaderKind::Fabric, v))
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
        proxy: &proxy,
        bridge: req.bridge,
    };
    if let Err(e) = crate::cosmetics::write_mod_session(game_dir, &session) {
        log::warn!("arctic session file: {e}");
    }
}

/// Build the command line for `req` from a finished installation.
pub fn plan(req: &LaunchRequest, inst: &Installation) -> LaunchPlan {
    let settings = req.settings;
    let proxy = ProxySettings::load(req.dirs);
    let env = if settings.fullscreen {
        RuleEnv::current()
    } else {
        RuleEnv::current().with_feature("has_custom_resolution")
    };
    let identity = req.account.identity();
    let vars = placeholders(req, inst, &identity);
    let logging_arg = inst.logging.as_ref().map(|(path, template)| {
        let vars = HashMap::from([("path", path.display().to_string())]);
        substitute(template, &vars)
    });
    let opts = JvmOptions {
        min_memory_mb: settings.min_memory_mb,
        max_memory_mb: req.instance.max_memory_mb.unwrap_or(settings.max_memory_mb),
        extra: settings
            .extra_jvm_args
            .split_whitespace()
            .chain(req.instance.jvm_args.split_whitespace())
            .map(str::to_owned)
            .chain(arctic_mod::jvm_flag())
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
        game_extra: proxy
            .active()
            .map(ProxySettings::game_args)
            .unwrap_or_default(),
    };
    LaunchPlan {
        java: inst.java.clone(),
        args: args::build(&inst.version, &env, &vars, &opts),
        game_dir: inst.game_dir.clone(),
        log_file: req.dirs.logs().join(match req.copy {
            0 => format!("game-{}.log", req.instance.id),
            n => format!("game-{}-{}.log", req.instance.id, n + 1),
        }),
        secrets: vec![identity.access_token, proxy.password.clone()],
    }
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
    fn redacts_access_token() {
        let plan = LaunchPlan {
            java: PathBuf::from("java"),
            args: vec!["--accessToken".into(), "SECRET123".into()],
            game_dir: PathBuf::new(),
            log_file: PathBuf::new(),
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
            secrets: vec!["tok".into(), "hunter2".into()],
        };
        assert!(plan.redacted_command().ends_with("--proxyPass <redacted>"));
    }
}
