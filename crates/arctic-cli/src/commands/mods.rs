//! `arctic mods …`.

use std::path::PathBuf;

use arctic_core::instances::{self, Instance};
use arctic_core::mods::{self, SearchQuery, SortBy};
use arctic_core::{Error, ProgressInfo, Result};
use serde_json::json;

use super::Ctx;
use crate::cli::ModsCommand;

pub fn run(ctx: &Ctx, command: &ModsCommand) -> Result<i32> {
    match command {
        ModsCommand::Search {
            query,
            instance,
            limit,
        } => search(ctx, &modded(ctx, instance)?, query, *limit)?,
        ModsCommand::Install { project, instance } => {
            install(ctx, &modded(ctx, instance)?, project)?
        }
        ModsCommand::List { instance } => list(ctx, &instances::find(&ctx.dirs, instance)?)?,
        ModsCommand::Remove { file, instance } => {
            let (dir, index) = folders(ctx, &instances::find(&ctx.dirs, instance)?);
            mods::remove(&dir, &index, file)?;
            ctx.out.emit(json!({"event": "removed", "file": file}), || {
                format!("Removed {file}")
            });
        }
        ModsCommand::Recognize { instance } => {
            let inst = modded(ctx, instance)?;
            let (dir, index) = folders(ctx, &inst);
            let game = inst.version.clone().unwrap_or_default();
            let kind = inst
                .loader
                .kind()
                .ok_or_else(|| Error::Other("no mod loader".into()))?;
            let r = mods::recognize(&dir, &index, &game, kind)?;
            ctx.out.emit(
                json!({"event": "recognized", "tracked": r.tracked, "unknown": r.unknown, "mismatched": r.mismatched}),
                || format!("{} mods recognized on Modrinth, {} not found there", r.tracked, r.unknown),
            );
        }
        ModsCommand::Retarget {
            version,
            instance,
            apply,
        } => retarget(ctx, modded(ctx, instance)?, version, *apply)?,
        ModsCommand::Toggle {
            file,
            instance,
            state,
        } => {
            let (dir, _) = folders(ctx, &instances::find(&ctx.dirs, instance)?);
            let on = state == "on";
            mods::set_enabled(&dir, file, on)?;
            ctx.out.emit(
                json!({"event": "toggled", "file": file, "enabled": on}),
                || format!("{file} is now {}", if on { "on" } else { "off" }),
            );
        }
    }
    Ok(0)
}

/// The instance, which must have a mod loader.
fn modded(ctx: &Ctx, query: &str) -> Result<Instance> {
    let inst = instances::find(&ctx.dirs, query)?;
    if inst.loader.kind().is_none() || inst.version.is_none() {
        return Err(Error::Other(format!("{} has no mod loader", inst.name)));
    }
    Ok(inst)
}

/// (mods folder, mods.json) of an instance.
fn folders(ctx: &Ctx, inst: &Instance) -> (PathBuf, PathBuf) {
    (
        inst.game_dir(&ctx.dirs).join("mods"),
        mods::index_path(&ctx.dirs.instance_dir(&inst.id)),
    )
}

/// `arctic mods retarget`: what the mods become on another version, and the switch.
fn retarget(ctx: &Ctx, mut inst: Instance, version: &str, apply: bool) -> Result<()> {
    let manifest = arctic_core::versions::VersionManifest::fetch(&ctx.dirs)?;
    let game = super::resolve_version(&manifest, version)?.id;
    let kind = inst
        .loader
        .kind()
        .ok_or_else(|| Error::Other("no mod loader".into()))?;
    let (dir, index) = folders(ctx, &inst);
    let progress = |_: ProgressInfo| {};
    let checks = mods::retarget::check(&dir, &index, &game, kind, &progress)?;
    for c in &checks {
        ctx.out.emit(
            json!({"event": "check", "file": c.file_name, "title": c.title, "project": c.project_id,
                   "current": c.current, "available": c.available, "dependency": c.dependency}),
            || match (&c.project_id, &c.available) {
                (None, _) => format!("  ?  {} (not on Modrinth, kept as is)", c.title),
                (Some(_), Some(v)) => format!(
                    "  ok {} {} -> {v}",
                    c.title,
                    c.current.as_deref().unwrap_or("?")
                ),
                (Some(_), None) => format!("  -- {} (no version for {game} yet)", c.title),
            },
        );
    }
    if !apply {
        return Ok(());
    }
    let applied = mods::retarget::apply(&dir, &index, &checks, &game, kind, &progress)?;
    let build = arctic_core::loaders::default_loader_version(kind, &game)?;
    inst.version = Some(game.clone());
    inst.loader = arctic_core::instances::Loader::new(Some(kind), build);
    inst.save(&ctx.dirs)?;
    ctx.out.emit(
        json!({"event": "retargeted", "version": game, "installed": applied.installed.len(),
               "disabled": applied.disabled, "failed": applied.failed.iter().map(|(p, e)| json!({"project": p, "error": e.to_string()})).collect::<Vec<_>>()}),
        || {
            format!(
                "{} is now on {game}: {} mods updated, {} turned off{}",
                inst.name,
                applied.installed.len(),
                applied.disabled.len(),
                if applied.failed.is_empty() {
                    String::new()
                } else {
                    format!(", {} failed", applied.failed.len())
                }
            )
        },
    );
    Ok(())
}

fn search(ctx: &Ctx, inst: &Instance, text: &str, limit: usize) -> Result<()> {
    let page = mods::search(&SearchQuery {
        text: text.to_owned(),
        game_version: inst.version.clone().unwrap_or_default(),
        loader: inst.loader.kind(),
        sort: SortBy::Relevance,
        offset: 0,
        limit,
        project_type: mods::ProjectType::Mod,
    })?;
    for hit in page.hits {
        ctx.out.emit(
            json!({
                "id": hit.project_id,
                "slug": hit.slug,
                "title": hit.title,
                "author": hit.author,
                "downloads": hit.downloads,
                "description": hit.description,
            }),
            || format!("{:<28} {:>10}  {}", hit.slug, hit.downloads, hit.title),
        );
    }
    Ok(())
}

fn install(ctx: &Ctx, inst: &Instance, project: &str) -> Result<()> {
    let (Some(kind), Some(game)) = (inst.loader.kind(), inst.version.as_deref()) else {
        return Err(Error::Other("instance has no mod loader".into()));
    };
    let (dir, index) = folders(ctx, inst);
    let progress = |p: ProgressInfo| ctx.out.progress(p);
    let installed = mods::install(project, game, kind, &dir, &index, &progress)?;
    ctx.out.end_progress();
    for m in installed {
        let note = if m.dependency { " (dependency)" } else { "" };
        ctx.out.emit(
            json!({
                "event": "installed",
                "project": m.project_id,
                "title": m.title,
                "version": m.version_number,
                "file": m.file_name,
                "dependency": m.dependency,
            }),
            || format!("Installed {} {}{note}", m.title, m.version_number),
        );
    }
    Ok(())
}

fn list(ctx: &Ctx, inst: &Instance) -> Result<()> {
    let (dir, index) = folders(ctx, inst);
    for f in mods::list(&dir, &index)? {
        if f.file_name == arctic_core::arctic_mod::FILE_NAME
            || f.file_name == mods::polarium::FILE_NAME
        {
            continue;
        }
        let title = f
            .tracked
            .as_ref()
            .map_or_else(|| f.file_name.clone(), |m| m.title.clone());
        let state = if f.enabled { "on " } else { "off" };
        ctx.out.emit(
            json!({"file": f.file_name, "enabled": f.enabled, "title": title, "size": f.size}),
            || format!("{state} {title:<32} {}", f.file_name),
        );
    }
    Ok(())
}
