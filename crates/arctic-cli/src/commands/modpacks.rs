//! `arctic modpacks …`.

use std::path::Path;

use arctic_core::mods::{self, SearchQuery, SortBy, modpack};
use arctic_core::{ProgressInfo, Result};
use serde_json::json;

use super::Ctx;
use crate::cli::ModpacksCommand;

pub fn run(ctx: &Ctx, command: &ModpacksCommand) -> Result<i32> {
    match command {
        ModpacksCommand::Search { query, limit } => search(ctx, query, *limit)?,
        ModpacksCommand::Install { pack } => install(ctx, pack)?,
    }
    Ok(0)
}

fn search(ctx: &Ctx, text: &str, limit: usize) -> Result<()> {
    let page = mods::search(&SearchQuery {
        text: text.trim().to_owned(),
        sort: if text.trim().is_empty() {
            SortBy::Downloads
        } else {
            SortBy::Relevance
        },
        limit,
        project_type: mods::ProjectType::Modpack,
        ..SearchQuery::default()
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

/// A `.mrpack` file, or a Modrinth slug or project id.
fn install(ctx: &Ctx, pack: &str) -> Result<()> {
    let progress = |p: ProgressInfo| ctx.out.progress(p);
    let file = Path::new(pack);
    let instance = if file.is_file() {
        modpack::install_file(&ctx.dirs, file, &progress)?
    } else {
        modpack::install_from_modrinth(&ctx.dirs, pack, &progress)?
    };
    ctx.out.end_progress();
    ctx.out.emit(
        json!({
            "event": "installed",
            "id": instance.id,
            "name": instance.name,
            "version": instance.version,
            "loader": instance.loader.label(),
        }),
        || {
            format!(
                "Installed {} ({} {}). Start it with: arctic launch --instance {}",
                instance.name,
                instance.loader.label(),
                instance.version.as_deref().unwrap_or(""),
                instance.id
            )
        },
    );
    Ok(())
}
