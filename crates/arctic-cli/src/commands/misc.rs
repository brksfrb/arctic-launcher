//! `arctic java`, `paths`, `update`, `open`.

use std::path::PathBuf;
use std::process::Command;

use arctic_core::ProgressInfo;
use arctic_core::update::{self, UpdateChannel};
use arctic_core::versions::{self, VersionManifest};
use arctic_core::{APP_VERSION, Error, Result, java};
use serde_json::json;

use super::{Ctx, resolve_version};
use crate::cli::OpenArgs;

/// Env var pointing at the launcher executable, for `arctic open`.
const LAUNCHER_EXE_ENV: &str = "ARCTIC_LAUNCHER_EXE";
/// Names the GUI is shipped/installed under, next to `arctic.exe`.
const LAUNCHER_EXE_NAMES: [&str; 3] = [
    "arctic-launcher.exe",
    "Arctic Launcher.exe",
    "arctic-launcher",
];

pub fn java_for(ctx: &Ctx, version: &str) -> Result<i32> {
    let manifest = VersionManifest::fetch(&ctx.dirs)?;
    let entry = resolve_version(&manifest, version)?;
    let progress = |p: ProgressInfo| ctx.out.progress(p);
    let json_version = versions::load_version(&ctx.dirs, &entry, &progress)?;
    let component = java::component_for(&json_version).to_owned();
    let exe = java::ensure_runtime(&ctx.dirs, &component, &progress)?;
    ctx.out.end_progress();
    ctx.out.emit(
        json!({"version": entry.id, "component": component, "java": exe}),
        || exe.display().to_string(),
    );
    Ok(0)
}

pub fn paths(ctx: &Ctx) -> Result<i32> {
    let d = &ctx.dirs;
    let rows = [
        ("root", d.root().to_path_buf()),
        ("settings", d.settings_file()),
        ("accounts", d.accounts_file()),
        ("versions", d.versions()),
        ("libraries", d.libraries()),
        ("assets", d.assets()),
        ("runtimes", d.runtimes()),
        ("instances", d.instances()),
        ("logs", d.logs()),
    ];
    for (name, path) in rows {
        ctx.out.emit(json!({"name": name, "path": path}), || {
            format!("{name:<10} {}", path.display())
        });
    }
    Ok(0)
}

pub fn update_check(ctx: &Ctx, beta: bool) -> Result<i32> {
    let channel = if beta {
        UpdateChannel::Beta
    } else {
        UpdateChannel::Stable
    };
    match update::check(channel)? {
        Some(info) => ctx.out.emit(
            json!({"current": APP_VERSION, "latest": info.version.to_string(), "required": info.required, "url": info.page_url}),
            || {
                let kind = if info.required { "required update" } else { "update" };
                format!("{kind} available: {APP_VERSION} → {} ({})", info.version, info.page_url)
            },
        ),
        None => ctx.out.emit(json!({"current": APP_VERSION, "latest": null}), || {
            format!("Arctic Launcher {APP_VERSION} is up to date")
        }),
    }
    Ok(0)
}

/// Start the GUI launcher (detached), forwarding the options as flags.
pub fn open(ctx: &Ctx, args: &OpenArgs) -> Result<i32> {
    let exe = launcher_exe()?;
    let mut cmd = Command::new(&exe);
    if let Some(v) = &args.launch {
        cmd.args(["--launch", v]);
    }
    if let Some(a) = &args.account {
        cmd.args(["--account", a]);
    }
    if let Some(t) = &args.tab {
        cmd.args(["--tab", t]);
    }
    if args.no_intro {
        cmd.arg("--no-intro");
    }
    cmd.env("ARCTIC_DATA_DIR", ctx.dirs.root());
    let child = cmd.spawn().map_err(|e| Error::io(&exe, e))?;
    ctx.out
        .emit(json!({"event": "opened", "pid": child.id()}), || {
            "Opened Arctic Launcher".into()
        });
    Ok(0)
}

fn launcher_exe() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(LAUNCHER_EXE_ENV) {
        return Ok(PathBuf::from(path));
    }
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_default();
    LAUNCHER_EXE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| {
            Error::NotConfigured(format!(
                "launcher executable not found next to arctic.exe; set {LAUNCHER_EXE_ENV}"
            ))
        })
}
