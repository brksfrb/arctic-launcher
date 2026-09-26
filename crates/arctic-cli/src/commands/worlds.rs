//! `arctic worlds …`: an instance's singleplayer worlds.

use std::path::Path;

use arctic_core::worlds::{self, World};
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::WorldsCommand;

pub fn run(ctx: &Ctx, command: &WorldsCommand) -> Result<i32> {
    let instance = instance_or_default(ctx, command.instance())?;
    let game_dir = instance.game_dir(&ctx.dirs);
    let saves = game_dir.join("saves");
    match command {
        WorldsCommand::List { .. } => {
            for w in worlds::list(&saves) {
                ctx.out.emit(
                    json!({"folder": w.folder, "name": w.name, "size": w.size, "path": w.path}),
                    || format!("{:<32} {:>9}  {}", w.name, megabytes(w.size), w.folder),
                );
            }
        }
        WorldsCommand::Sources { .. } => {
            for source in worlds::sources(&ctx.dirs, &saves) {
                let count = worlds::list(&source.saves).len();
                ctx.out.emit(
                    json!({"label": source.label, "saves": source.saves, "worlds": count}),
                    || {
                        format!(
                            "{} ({count} worlds)\n    {}",
                            source.label,
                            source.saves.display()
                        )
                    },
                );
            }
        }
        WorldsCommand::Import { path, .. } => {
            let folder = import(path, &saves)?;
            ctx.out.emit(
                json!({"event": "imported", "folder": folder, "instance": instance.id}),
                || format!("Imported into {} as \"{folder}\".", instance.name),
            );
        }
        WorldsCommand::Backup { world, .. } => {
            let found = find(&saves, world)?;
            let zip = worlds::backup(&found, &game_dir.join(worlds::BACKUPS_DIR))?;
            ctx.out.emit(
                json!({"event": "backed_up", "folder": found.folder, "zip": zip}),
                || format!("Backed up \"{}\" to {}", found.name, zip.display()),
            );
        }
        WorldsCommand::Remove { world, .. } => {
            let found = find(&saves, world)?;
            worlds::trash(&found, &saves)?;
            ctx.out
                .emit(json!({"event": "removed", "folder": found.folder}), || {
                    format!("Moved \"{}\" to saves/.trash", found.name)
                });
        }
    }
    Ok(0)
}

/// A world folder or a .zip holding one.
fn import(path: &Path, saves: &Path) -> Result<String> {
    std::fs::create_dir_all(saves).map_err(|e| Error::io(saves, e))?;
    let is_zip = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if is_zip {
        worlds::import_zip(path, saves)
    } else {
        worlds::import_folder(path, saves)
    }
}

/// By folder name first, then by the name shown in game.
fn find(saves: &Path, query: &str) -> Result<World> {
    let all = worlds::list(saves);
    let by = |f: &dyn Fn(&World) -> bool| all.iter().find(|w| f(w)).cloned();
    by(&|w| w.folder == query)
        .or_else(|| by(&|w| w.folder.eq_ignore_ascii_case(query)))
        .or_else(|| by(&|w| w.name.eq_ignore_ascii_case(query)))
        .ok_or_else(|| {
            Error::Other(format!(
                "no world matches '{query}' (see `arctic worlds list`)"
            ))
        })
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
