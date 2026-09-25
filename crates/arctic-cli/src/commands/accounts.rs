//! `arctic accounts …`.

use std::sync::atomic::AtomicBool;

use arctic_core::auth::AccountStore;
use arctic_core::auth::microsoft::{self, MsaConfig};
#[cfg(feature = "offline-accounts")]
use arctic_core::auth::offline;
use arctic_core::{Error, Result};
use serde_json::json;

use super::Ctx;
use crate::cli::AccountsCommand;

pub fn run(ctx: &Ctx, command: &AccountsCommand) -> Result<i32> {
    let mut store = AccountStore::load(&ctx.dirs)?;
    match command {
        AccountsCommand::List => list(ctx, &store),
        #[cfg(feature = "offline-accounts")]
        AccountsCommand::AddOffline { username } => {
            let account = offline::create(username)?;
            store.upsert(account.clone());
            store.save(&ctx.dirs)?;
            ctx.out.emit(
                json!({"event": "added", "username": account.username, "uuid": account.uuid, "type": "offline"}),
                || format!("Added offline account {} (now active)", account.username),
            );
        }
        AccountsCommand::Login { browser } => {
            let cfg = MsaConfig::load(&ctx.dirs)?;
            let cancel = AtomicBool::new(false);
            let account = if *browser {
                eprintln!("Opening your browser to sign in…");
                microsoft::login_with_browser(&cfg, &cancel)?
            } else {
                let code = microsoft::start_device_code(&cfg)?;
                ctx.out.emit(
                    json!({"event": "device_code", "code": code.user_code, "url": code.verification_uri}),
                    || format!("Go to {} and enter the code: {}", code.verification_uri, code.user_code),
                );
                microsoft::finish_device_code(&cfg, &code, &cancel)?
            };
            store.upsert(account.clone());
            store.save(&ctx.dirs)?;
            ctx.out.emit(
                json!({"event": "added", "username": account.username, "uuid": account.uuid, "type": "microsoft"}),
                || format!("Signed in as {} (now active)", account.username),
            );
        }
        AccountsCommand::Use { account } => {
            let id = find_id(&store, account)?;
            store.set_active(&id);
            store.save(&ctx.dirs)?;
            let name = store
                .active()
                .map(|a| a.username.clone())
                .unwrap_or_default();
            ctx.out
                .emit(json!({"event": "active", "username": name}), || {
                    format!("Now playing as {name}")
                });
        }
        AccountsCommand::Remove { account } => {
            let id = find_id(&store, account)?;
            store.remove(&id);
            store.save(&ctx.dirs)?;
            ctx.out
                .emit(json!({"event": "removed", "account": account}), || {
                    format!("Removed {account}")
                });
        }
    }
    Ok(0)
}

fn list(ctx: &Ctx, store: &AccountStore) {
    if store.accounts.is_empty() && !ctx.out.json {
        println!("No accounts. Add one with `arctic accounts login`.");
    }
    for account in &store.accounts {
        let active = store.active.as_deref() == Some(account.id.as_str());
        ctx.out.emit(
            json!({
                "id": account.id,
                "username": account.username,
                "uuid": account.uuid,
                "type": account.kind_label().to_lowercase(),
                "active": active,
            }),
            || {
                let marker = if active { "*" } else { " " };
                format!(
                    "{marker} {:<18} {:<10} {}",
                    account.username,
                    account.kind_label(),
                    account.uuid
                )
            },
        );
    }
}

fn find_id(store: &AccountStore, query: &str) -> Result<String> {
    store
        .find(query)
        .map(|a| a.id.clone())
        .ok_or_else(|| Error::Other(format!("no account matches '{query}'")))
}
