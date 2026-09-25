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

fn search(ctx: &Ctx, inst: &Instance, text: &str, limit: usize) -> Result<()> {
    let page = mods::search(&SearchQuery {
        text: text.to_owned(),
        game_version: inst.version.clone().unwrap_or_default(),
        loader: inst.loader.kind(),
        sort: SortBy::Relevance,
        offset: 0,
        limit,
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
        if f.file_name == arctic_core::arctic_mod::FILE_NAME {
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
