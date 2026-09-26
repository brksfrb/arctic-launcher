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
}

/// A fully resolved command line, ready to spawn.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub log_file: PathBuf,
    /// Access token, kept only to redact it from logs.
    secret: String,
}

impl LaunchPlan {
    /// Command line safe to show or log (access token replaced).
    pub fn redacted_command(&self) -> String {
        let redact = |a: &String| {
            if !self.secret.is_empty() && self.secret != "0" && a.contains(&self.secret) {
                a.replace(&self.secret, "<redacted>")
            } else {
                a.clone()
            }
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
    let dirs = req.dirs;
    let game_dir = req.instance.game_dir(dirs);
    let java_override = req
        .instance
        .java_path
        .clone()
        .or_else(|| req.settings.java_override.clone());
    progress(ProgressInfo::stage("Reading version"));
    let vanilla = load_version(dirs, req.version, &|_| {})?;
    let version = match (req.instance.loader.kind(), req.instance.loader.version()) {
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
                arctic_mod::sync(&game_dir.join("mods"), &vanilla.id, req.instance.arctic_mod)?;
                if req.instance.arctic_mod && arctic_mod::supports(&vanilla.id) {
                    share_cosmetics_session(dirs, &game_dir, req.account);
                }
            }
            profile
        }
        _ => vanilla,
    };
    let installation = install(dirs, version, &game_dir, java_override, progress)?;
    Ok(plan(req, &installation))
}

/// Let the Arctic mod publish looks as this player. Best effort: without
/// the cosmetics server the game still starts, and the mod can show looks.
fn share_cosmetics_session(dirs: &DataDirs, game_dir: &Path, account: &Account) {
    let base = crate::cosmetics::base_url();
    match crate::cosmetics::token_for(dirs, &base, account) {
        Ok(token) => {
            if let Err(e) = crate::cosmetics::write_mod_session(game_dir, &base, &token) {
                log::warn!("arctic session file: {e}");
            }
        }
        Err(e) => log::info!("Arctic cosmetics unavailable: {e}"),
    }
}

/// Build the command line for `req` from a finished installation.
pub fn plan(req: &LaunchRequest, inst: &Installation) -> LaunchPlan {
    let settings = req.settings;
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
            .collect(),
        logging_arg,
        fullscreen: settings.fullscreen,
        resolution: (settings.window_width, settings.window_height),
    };
    LaunchPlan {
        java: inst.java.clone(),
        args: args::build(&inst.version, &env, &vars, &opts),
        game_dir: inst.game_dir.clone(),
        log_file: req
            .dirs
            .logs()
            .join(format!("game-{}.log", req.instance.id)),
        secret: identity.access_token,
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
            secret: "SECRET123".into(),
        };
        let cmd = plan.redacted_command();
        assert!(!cmd.contains("SECRET123"));
        assert!(cmd.ends_with("--accessToken <redacted>"));
    }
}
