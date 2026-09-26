//! `arctic packs …`: resource packs and shaders from Modrinth.

use arctic_core::instances::Instance;
use arctic_core::mods::packs::{self, PackKind};
use arctic_core::mods::{self, SearchQuery, SortBy};
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::{PackKindArg, PacksCommand};

pub fn run(ctx: &Ctx, command: &PacksCommand) -> Result<i32> {
    match command {
        PacksCommand::Search {
            query,
            kind,
            instance,
            limit,
        } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            let page = mods::search(&SearchQuery {
                text: query.clone(),
                game_version: game(ctx, &inst),
                project_type: kind_of(*kind).project_type(),
                sort: SortBy::Relevance,
                limit: *limit,
                ..SearchQuery::default()
            })?;
            for hit in page.hits {
                ctx.out.emit(
                    json!({"id": hit.project_id, "slug": hit.slug, "title": hit.title, "downloads": hit.downloads}),
                    || format!("{:<28} {:>10}  {}", hit.slug, hit.downloads, hit.title),
                );
            }
        }
        PacksCommand::Install {
            project,
            kind,
            instance,
            keep_off,
        } => {
            let kind = kind_of(*kind);
            let inst = instance_or_default(ctx, instance.as_deref())?;
            let game_dir = inst.game_dir(&ctx.dirs);
            if kind == PackKind::Shader {
                shader_support(ctx, &inst)?;
            }
            let progress = |p: arctic_core::ProgressInfo| ctx.out.progress(p);
            let file = packs::install(kind, project, &game(ctx, &inst), &game_dir, &progress);
            ctx.out.end_progress();
            let file = file?;
            if !keep_off {
                packs::set_active(kind, &game_dir, &file, true)?;
            }
            ctx.out.emit(
                json!({"event": "installed", "file": file, "active": !keep_off}),
                || {
                    format!(
                        "Installed {file}{}",
                        if *keep_off { "" } else { " and switched it on" }
                    )
                },
            );
        }
        PacksCommand::List { kind, instance } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            for p in packs::list(kind_of(*kind), &inst.game_dir(&ctx.dirs)) {
                ctx.out.emit(
                    json!({"file": p.file_name, "active": p.active, "size": p.size}),
                    || format!("{} {}", if p.active { "on " } else { "off" }, p.file_name),
                );
            }
        }
        PacksCommand::Use {
            file,
            kind,
            instance,
            state,
        } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            let on = state == "on";
            packs::set_active(kind_of(*kind), &inst.game_dir(&ctx.dirs), file, on)?;
            ctx.out.emit(
                json!({"event": "toggled", "file": file, "active": on}),
                || format!("{file} is now {}", if on { "on" } else { "off" }),
            );
        }
        PacksCommand::Remove {
            file,
            kind,
            instance,
        } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            packs::remove(kind_of(*kind), &inst.game_dir(&ctx.dirs), file)?;
            ctx.out.emit(json!({"event": "removed", "file": file}), || {
                format!("Removed {file}")
            });
        }
    }
    Ok(0)
}

fn kind_of(arg: PackKindArg) -> PackKind {
    match arg {
        PackKindArg::Resource => PackKind::Resource,
        PackKindArg::Shader => PackKind::Shader,
    }
}

/// The instance's Minecraft version (Vanilla follows the Play tab choice).
fn game(ctx: &Ctx, inst: &Instance) -> String {
    inst.version.clone().unwrap_or_else(|| {
        arctic_core::settings::Settings::load(&ctx.dirs)
            .ok()
            .and_then(|s| s.last_version)
            .unwrap_or_default()
    })
}

/// Shaders need Iris (Oculus on Forge): switch it on for Vanilla, install
/// it into modded instances.
fn shader_support(ctx: &Ctx, inst: &Instance) -> Result<()> {
    let Some(kind) = inst.loader.kind() else {
        if !inst.shaders {
            let updated = Instance {
                shaders: true,
                ..inst.clone()
            };
            updated.save(&ctx.dirs)?;
            ctx.out
                .info("Shaders switched on for this instance (Iris comes with the next start).");
        }
        return Ok(());
    };
    let game_dir = inst.game_dir(&ctx.dirs);
    let index = mods::index_path(&ctx.dirs.instance_dir(&inst.id));
    let project = packs::shader_mod(kind);
    let have = mods::list(&game_dir.join("mods"), &index)?
        .iter()
        .any(|m| m.tracked.as_ref().is_some_and(|t| t.project_id == project));
    if have {
        return Ok(());
    }
    let version = inst
        .version
        .clone()
        .ok_or_else(|| Error::Other("the instance has no version".into()))?;
    let progress = |p: arctic_core::ProgressInfo| ctx.out.progress(p);
    let result = mods::install(
        project,
        &version,
        kind,
        &game_dir.join("mods"),
        &index,
        &progress,
    );
    ctx.out.end_progress();
    result.map_err(|e| {
        Error::Other(format!(
            "shaders need Iris, which couldn't be installed: {e}"
        ))
    })?;
    ctx.out.info("Installed Iris so shaders work.");
    Ok(())
}
