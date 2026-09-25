//! `arctic profiles …`.

use arctic_core::profiles::ProfileStore;
use arctic_core::{Error, Result};
use serde_json::json;

use super::Ctx;
use crate::cli::ProfilesCommand;

pub fn run(ctx: &Ctx, command: &ProfilesCommand) -> Result<i32> {
    let mut store = ProfileStore::load_or_init(&ctx.root)?;
    match command {
        ProfilesCommand::List => {
            for p in &store.profiles {
                let active = p.id == store.active;
                ctx.out.emit(
                    json!({"id": p.id, "name": p.name, "active": active, "path": ctx.root.with_profile(&p.id).profile_root()}),
                    || format!("{} {:<24} {}", if active { "*" } else { " " }, p.name, p.id),
                );
            }
        }
        ProfilesCommand::Create { name, switch } => {
            let id = store.create(&ctx.root, name)?;
            if *switch {
                store.set_active(&id);
            }
            store.save(&ctx.root)?;
            ctx.out.emit(
                json!({"event": "created", "id": id, "active": switch}),
                || {
                    format!(
                        "Created profile {name}{}",
                        if *switch { " (now active)" } else { "" }
                    )
                },
            );
        }
        ProfilesCommand::Use { profile } => {
            let id = find(&store, profile)?;
            store.set_active(&id);
            store.save(&ctx.root)?;
            ctx.out.emit(json!({"event": "active", "id": id}), || {
                format!("Now using profile {profile}")
            });
        }
        ProfilesCommand::Rename { profile, new_name } => {
            let id = find(&store, profile)?;
            store.rename(&id, new_name)?;
            store.save(&ctx.root)?;
            ctx.out.emit(
                json!({"event": "renamed", "id": id, "name": new_name}),
                || format!("Renamed {profile} to {new_name}"),
            );
        }
        ProfilesCommand::Remove { profile } => {
            let id = find(&store, profile)?;
            store.remove(&ctx.root, &id)?;
            store.save(&ctx.root)?;
            ctx.out.emit(json!({"event": "removed", "id": id}), || {
                format!("Removed {profile} (moved to profiles/.trash)")
            });
        }
    }
    Ok(0)
}

fn find(store: &ProfileStore, query: &str) -> Result<String> {
    store
        .find(query)
        .map(|p| p.id.clone())
        .ok_or_else(|| Error::Other(format!("no profile matches '{query}'")))
}
