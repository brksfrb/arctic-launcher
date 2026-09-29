//! `arctic skins …`: the skin library (the launcher's Cosmetics tab) and
//! the community gallery.

use std::path::Path;

use arctic_core::cosmetics::{self, GallerySort, NewLook, Texture};
use arctic_core::skins::{Library, SkinEntry, Variant};
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, fresh_account};
use crate::cli::{SkinModel, SkinsCommand};

/// Gallery page size (the server's).
const GALLERY_PAGE: usize = 24;

pub fn run(ctx: &Ctx, command: &SkinsCommand) -> Result<i32> {
    let dir = Library::dir(ctx.dirs.profile_root());
    let mut library = Library::load(&dir)?;
    match command {
        SkinsCommand::List => list(ctx, &library),
        SkinsCommand::Add { file, name, model } => {
            let png = std::fs::read(file).map_err(|e| Error::io(file, e))?;
            let name = name.clone().unwrap_or_else(|| file_stem(file));
            let entry = library.add(&dir, &name, &png, model.map(variant))?;
            ctx.out.emit(
                json!({"event": "added", "id": entry.id, "name": entry.name}),
                || format!("Added {} ({})", entry.name, entry.id),
            );
        }
        SkinsCommand::Rename { skin, name } => {
            let entry = find(&library, skin)?;
            let name = name.trim().to_owned();
            if name.is_empty() {
                return Err(Error::Other("the name can't be empty".into()));
            }
            library.update(&dir, &entry.id, |e| e.name = name.clone())?;
            ctx.out.emit(
                json!({"event": "renamed", "id": entry.id, "name": name}),
                || format!("Renamed {} to {name}", entry.name),
            );
        }
        SkinsCommand::Model { skin, model } => {
            let entry = find(&library, skin)?;
            let v = variant(*model);
            library.update(&dir, &entry.id, |e| e.variant = v)?;
            ctx.out.emit(
                json!({"event": "model", "id": entry.id, "model": v.api_name()}),
                || format!("{} now uses {} arms", entry.name, v.label()),
            );
        }
        SkinsCommand::Remove { skin } => {
            let entry = find(&library, skin)?;
            library.remove(&dir, &entry.id)?;
            ctx.out
                .emit(json!({"event": "removed", "id": entry.id}), || {
                    format!("Removed {}", entry.name)
                });
        }
        SkinsCommand::Export { skin, file } => {
            let entry = find(&library, skin)?;
            let png = Library::read_png(&dir, &entry.id)?;
            std::fs::write(file, png).map_err(|e| Error::io(file, e))?;
            ctx.out
                .emit(json!({"event": "exported", "file": file}), || {
                    format!("Saved {} to {}", entry.name, file.display())
                });
        }
        SkinsCommand::Wear { skin, account } => {
            let chosen = match skin.as_str() {
                "none" => None,
                query => {
                    let entry = find(&library, query)?;
                    Some((
                        Library::read_png(&dir, &entry.id)?,
                        entry.variant,
                        entry.name,
                    ))
                }
            };
            wear(ctx, account.as_deref(), chosen)?;
        }
        SkinsCommand::Gallery { query, new, page } => gallery(ctx, query, *new, *page)?,
        SkinsCommand::Take {
            id,
            wear: put_on,
            account,
        } => {
            let base = cosmetics::base_url();
            let item = find_gallery(&base, id)?;
            let png = cosmetics::texture(&base, &cosmetics::gallery_use(&base, &item.id)?)?;
            let entry = library.add(&dir, &item.name, &png, Some(item.variant()))?;
            ctx.out.emit(
                json!({"event": "added", "id": entry.id, "name": entry.name}),
                || format!("Added {} ({}) to your library", entry.name, entry.id),
            );
            if *put_on {
                wear(
                    ctx,
                    account.as_deref(),
                    Some((png, entry.variant, entry.name)),
                )?;
            }
        }
        SkinsCommand::Share {
            skin,
            name,
            account,
        } => {
            let entry = find(&library, skin)?;
            let png = Library::read_png(&dir, &entry.id)?;
            let name = name.clone().unwrap_or_else(|| entry.name.clone());
            let account = fresh_account(ctx, account.as_deref())?;
            let base = cosmetics::base_url();
            let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
            cosmetics::gallery_share(&base, &token, &png, entry.variant, &name)?;
            ctx.out.emit(json!({"event": "shared", "name": name}), || {
                format!("Shared {name} to the gallery")
            });
        }
        SkinsCommand::Report { id, account } => {
            let account = fresh_account(ctx, account.as_deref())?;
            let base = cosmetics::base_url();
            let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
            cosmetics::gallery_report(&base, &token, id)?;
            ctx.out.emit(json!({"event": "reported", "id": id}), || {
                "Reported. Thanks for keeping the gallery clean.".to_owned()
            });
        }
    }
    Ok(0)
}

fn list(ctx: &Ctx, library: &Library) {
    if library.skins.is_empty() {
        ctx.out
            .info("Your skin library is empty: add one with `arctic skins add FILE`");
    }
    for s in &library.skins {
        ctx.out.emit(
            json!({"id": s.id, "name": s.name, "model": s.variant.api_name(), "added": s.added}),
            || format!("{:<34} {:<8} {}", s.id, s.variant.label(), s.name),
        );
    }
}

/// A library skin by id, id prefix or name.
fn find(library: &Library, query: &str) -> Result<SkinEntry> {
    let q = query.trim();
    library
        .skins
        .iter()
        .find(|s| s.id == q)
        .or_else(|| {
            library
                .skins
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(q))
        })
        .or_else(|| {
            let mut prefixed = library.skins.iter().filter(|s| s.id.starts_with(q));
            match (prefixed.next(), prefixed.next()) {
                (Some(one), None) if q.len() >= 4 => Some(one),
                _ => None,
            }
        })
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "no skin '{q}' in your library (see `arctic skins list`)"
            ))
        })
}

/// Put a skin on as the Arctic look (`None`: back to the Minecraft skin),
/// keeping the cape and cosmetics.
fn wear(ctx: &Ctx, account: Option<&str>, skin: Option<(Vec<u8>, Variant, String)>) -> Result<()> {
    let account = fresh_account(ctx, account)?;
    let base = cosmetics::base_url();
    let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
    let presets = cosmetics::catalog(&base)?;
    let current = cosmetics::my_look(&base, &token)?;
    let mut look = NewLook::from_look(&current, &presets);
    let name = skin.as_ref().map(|(_, _, name)| name.clone());
    look.skin = skin.map(|(png, variant, _)| (Texture::Png(png), variant));
    cosmetics::set_look(&base, &token, &look)?;
    ctx.out
        .emit(json!({"event": "wearing", "skin": name}), || match &name {
            Some(name) => format!("{} now wears {name} for Arctic players", account.username),
            None => format!("{} is back to their Minecraft skin", account.username),
        });
    Ok(())
}

fn gallery(ctx: &Ctx, query: &str, newest: bool, page: usize) -> Result<()> {
    let sort = if newest {
        GallerySort::New
    } else {
        GallerySort::Popular
    };
    let offset = page.saturating_sub(1) * GALLERY_PAGE;
    let found = cosmetics::gallery(&cosmetics::base_url(), sort, query, offset)?;
    for item in &found.items {
        ctx.out.emit(
            json!({
                "id": item.id,
                "name": item.name,
                "author": item.author,
                "model": item.model,
                "uses": item.downloads,
            }),
            || {
                format!(
                    "{:<14} {:>6}  {} by {}",
                    item.id, item.downloads, item.name, item.author
                )
            },
        );
    }
    let pages = (found.total.max(0) as usize).div_ceil(GALLERY_PAGE);
    if pages > page {
        ctx.out.info(&format!(
            "Page {page} of {pages}: more with --page {}",
            page + 1
        ));
    }
    Ok(())
}

/// A gallery item by id (searched on the newest pages if not popular).
fn find_gallery(base: &str, id: &str) -> Result<cosmetics::GalleryItem> {
    for sort in [GallerySort::Popular, GallerySort::New] {
        let mut offset = 0;
        loop {
            let page = cosmetics::gallery(base, sort, "", offset)?;
            if let Some(item) = page.items.iter().find(|i| i.id == id) {
                return Ok(item.clone());
            }
            offset += GALLERY_PAGE;
            if page.items.is_empty() || offset as i64 >= page.total || offset >= GALLERY_PAGE * 20 {
                break;
            }
        }
    }
    Err(Error::Other(format!(
        "no gallery skin '{id}' (see `arctic skins gallery`)"
    )))
}

fn variant(model: SkinModel) -> Variant {
    match model {
        SkinModel::Classic => Variant::Classic,
        SkinModel::Slim => Variant::Slim,
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Skin".into())
}
