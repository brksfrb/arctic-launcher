//! `arctic versions` and `arctic install`.

use arctic_core::ProgressInfo;
use arctic_core::settings::Settings;
use arctic_core::versions::{self, VersionKind, VersionManifest};
use arctic_core::{Result, instances, launch};
use serde_json::json;

use super::{Ctx, resolve_version};
use crate::cli::{InstallArgs, VersionsArgs};

pub fn list(ctx: &Ctx, args: &VersionsArgs) -> Result<i32> {
    let manifest = VersionManifest::fetch(&ctx.dirs)?;
    let installed = versions::installed_versions(&ctx.dirs);
    let rows = manifest
        .versions
        .iter()
        .filter(|v| args.all || v.kind == VersionKind::Release)
        .filter(|v| !args.installed || installed.contains(&v.id))
        .take(args.limit.unwrap_or(usize::MAX));
    for v in rows {
        let is_installed = installed.contains(&v.id);
        let latest = v.id == manifest.latest.release;
        ctx.out.emit(
            json!({
                "id": v.id,
                "type": v.kind,
                "released": v.release_time,
                "installed": is_installed,
                "latest": latest,
            }),
            || {
                let date: String = v.release_time.chars().take(10).collect();
                let mut line = format!("{:<22} {date}", v.id);
                if is_installed {
                    line.push_str("  installed");
                }
                if latest {
                    line.push_str("  latest");
                }
                line
            },
        );
    }
    Ok(0)
}

/// Download everything a version needs (same queue the launch uses).
pub fn install(ctx: &Ctx, args: &InstallArgs) -> Result<i32> {
    let manifest = VersionManifest::fetch(&ctx.dirs)?;
    let entry = resolve_version(&manifest, &args.version)?;
    let instance = instances::load_default(&ctx.dirs)?;
    let settings = Settings::load(&ctx.dirs)?;
    let progress = |p: ProgressInfo| ctx.out.progress(p);
    let version = versions::load_version(&ctx.dirs, &entry, &progress)?;
    let installed = launch::install(
        &ctx.dirs,
        version,
        &instance.game_dir(&ctx.dirs),
        settings.java_override,
        &progress,
    )?;
    ctx.out.end_progress();
    ctx.out.emit(
        json!({"event": "installed", "version": installed.version.id, "java": installed.java}),
        || {
            format!(
                "Installed {} (Java: {})",
                installed.version.id,
                installed.java.display()
            )
        },
    );
    Ok(0)
}
