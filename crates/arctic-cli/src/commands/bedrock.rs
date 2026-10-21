//! `arctic bedrock …`: Minecraft: Bedrock Edition on Windows.

use std::sync::atomic::AtomicBool;

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus};
use arctic_core::bedrock::versions::{self, Catalog};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Folders, Player};
use arctic_core::{Error, Result};
use serde_json::{Value, json};

use super::Ctx;
use crate::cli::{
    BedrockArgs, BedrockCommand, BedrockPacksCommand, BedrockServersCommand, BedrockWorldsCommand,
};

/// Versions listed without `--all`.
const NEWEST: usize = 10;

pub fn run(ctx: &Ctx, args: &BedrockArgs) -> Result<i32> {
    let edition = if args.preview {
        Edition::Preview
    } else {
        Edition::Release
    };
    let account = args.account.as_deref();
    match &args.command {
        BedrockCommand::Status => status(ctx, edition),
        BedrockCommand::Play => {
            let settings = arctic_core::settings::Settings::load(&ctx.dirs).unwrap_or_default();
            let client = settings
                .bedrock_client
                .as_deref()
                .and_then(bedrock::client::Client::parse);
            let loading = bedrock::launch_with_client(&ctx.dirs, edition, client, |message| {
                eprintln!("{message}")
            })?;
            if let Some(loading) = loading {
                // The client is loaded from a thread: stay until it has had its turn.
                let _ = loading.join();
            }
            ctx.out.emit(
                json!({"event": "launched", "edition": edition.id()}),
                || format!("Starting Minecraft {}.", edition.name()),
            );
            Ok(0)
        }
        BedrockCommand::Versions { all } => {
            let catalog = versions::catalog(&ctx.root.cache())?;
            list_versions(ctx, &catalog, edition, *all);
            Ok(0)
        }
        BedrockCommand::Install { version } => install(ctx, edition, version),
        BedrockCommand::Worlds(cmd) => run_worlds(ctx, edition, account, cmd),
        BedrockCommand::Packs(cmd) => run_packs(ctx, edition, cmd),
        BedrockCommand::Servers(cmd) => run_servers(ctx, edition, account, cmd),
    }
}

fn status(ctx: &Ctx, edition: Edition) -> Result<i32> {
    let install = bedrock::find(edition);
    let folders = bedrock::folders(edition);
    let running = install.is_some() && bedrock::running(edition);
    let accounts: Vec<Value> = folders
        .players
        .iter()
        .map(|p| json!({"id": p.id, "path": p.com_mojang}))
        .collect();
    ctx.out.emit(
        json!({
            "edition": edition.id(),
            "supported": bedrock::supported(),
            "installed": install.is_some(),
            "version": install.as_ref().map(|i| &i.version),
            "path": install.as_ref().map(|i| &i.path),
            "gdk": install.as_ref().map(|i| i.gdk),
            "running": running,
            "data": folders.root,
            "packs": folders.packs.as_ref().map(|p| &p.com_mojang),
            "accounts": accounts,
        }),
        || {
            if !bedrock::supported() {
                return "Bedrock Edition is only played on Windows.".into();
            }
            let mut text = match &install {
                Some(i) => format!(
                    "Minecraft {} {}{}{}\n  {}",
                    edition.name(),
                    versions::display(&i.version, edition),
                    if i.gdk { "" } else { " (old UWP build)" },
                    if running { ", running" } else { "" },
                    i.path.display()
                ),
                None => format!(
                    "Minecraft {} isn't installed (get it from the Microsoft Store).",
                    edition.name()
                ),
            };
            if let Some(root) = &folders.root {
                text.push_str(&format!("\nData: {}", root.display()));
            }
            for p in &folders.players {
                text.push_str(&format!(
                    "\n  {:<22} {}  (--account {})",
                    p.label(),
                    p.com_mojang.display(),
                    p.id
                ));
            }
            text
        },
    );
    Ok(0)
}

fn list_versions(ctx: &Ctx, catalog: &Catalog, edition: Edition, all: bool) {
    let installed = bedrock::find(edition).map(|i| i.version);
    let list = catalog.of(edition);
    let shown = if all {
        list.len()
    } else {
        NEWEST.min(list.len())
    };
    for v in &list[..shown] {
        let current = installed.as_deref() == Some(v.id.as_str());
        ctx.out.emit(
            json!({"id": v.id, "name": v.display(), "edition": edition.id(), "installed": current}),
            || {
                format!(
                    "{:<12} {:<14}{}",
                    v.display(),
                    v.id,
                    if current { "  installed" } else { "" }
                )
            },
        );
    }
    if shown < list.len() {
        ctx.out
            .info(&format!("…and {} older (--all).", list.len() - shown));
    }
}

fn install(ctx: &Ctx, edition: Edition, wanted: &str) -> Result<i32> {
    let catalog = versions::catalog(&ctx.root.cache())?;
    let version = if wanted.eq_ignore_ascii_case("latest") {
        catalog.latest(edition)
    } else {
        catalog.find(edition, wanted)
    }
    .cloned()
    .ok_or_else(|| {
        Error::Other(format!(
            "no {} version '{wanted}' (see `arctic bedrock versions{}`)",
            edition.name(),
            if edition == Edition::Preview {
                " --preview"
            } else {
                ""
            }
        ))
    })?;
    if bedrock::find(edition).is_some_and(|i| i.version == version.id) {
        ctx.out.emit(
            json!({"event": "already_installed", "version": version.id}),
            || {
                format!(
                    "{} {} is installed already.",
                    edition.name(),
                    version.display()
                )
            },
        );
        return Ok(0);
    }
    let cancel = AtomicBool::new(false);
    let result = versions::install(&version, &ctx.root.cache(), &cancel, &|p| {
        // Windows only tells how far along it is, not bytes.
        let stage = format!("{}  {:>3}%", p.stage, p.done);
        ctx.out
            .progress(arctic_core::ProgressInfo { stage: &stage, ..p });
    });
    ctx.out.end_progress();
    result?;
    ctx.out.emit(
        json!({"event": "installed", "edition": edition.id(), "version": version.id}),
        || format!("Installed {} {}.", edition.name(), version.display()),
    );
    Ok(0)
}

/// The account folder asked for, or the one played last.
fn player<'a>(folders: &'a Folders, account: Option<&str>, edition: Edition) -> Result<&'a Player> {
    if let Some(id) = account
        && !folders.players.iter().any(|p| p.id == id)
    {
        return Err(Error::Other(format!(
            "no account folder '{id}' (see `arctic bedrock status`)"
        )));
    }
    folders.player(account).ok_or_else(|| no_data(edition))
}

fn no_data(edition: Edition) -> Error {
    Error::Other(format!(
        "Minecraft {} has no data folders yet: start it once first",
        edition.name()
    ))
}

fn run_worlds(
    ctx: &Ctx,
    edition: Edition,
    account: Option<&str>,
    command: &BedrockWorldsCommand,
) -> Result<i32> {
    let folders = bedrock::folders(edition);
    let player = player(&folders, account, edition)?;
    match command {
        BedrockWorldsCommand::List => {
            for w in worlds::list(player) {
                ctx.out.emit(
                    json!({"id": w.id, "name": w.name, "size": w.size, "path": w.path}),
                    || format!("{:<32} {:>9}  {}", w.name, megabytes(w.size), w.id),
                );
            }
        }
        BedrockWorldsCommand::Import { path } => {
            let name = worlds::import(path, player)?;
            ctx.out
                .emit(json!({"event": "imported", "name": name}), || {
                    format!("Imported \"{name}\".")
                });
        }
        BedrockWorldsCommand::Export { world, to } => {
            let found = find_world(player, world)?;
            let to = to.clone().unwrap_or_else(|| {
                std::path::PathBuf::from(format!(
                    "{}.mcworld",
                    found.name.replace(['/', '\\', ':'], "_")
                ))
            });
            worlds::export(&found, &to)?;
            ctx.out.emit(
                json!({"event": "exported", "id": found.id, "file": to}),
                || format!("Exported \"{}\" to {}", found.name, to.display()),
            );
        }
        BedrockWorldsCommand::Backup { world } => {
            let found = find_world(player, world)?;
            let file = worlds::backup(&found, &ctx.root.root().join("bedrock").join("backups"))?;
            ctx.out.emit(
                json!({"event": "backed_up", "id": found.id, "file": file}),
                || format!("Backed up \"{}\" to {}", found.name, file.display()),
            );
        }
        BedrockWorldsCommand::Remove { world } => {
            let found = find_world(player, world)?;
            worlds::trash(&found, player)?;
            ctx.out
                .emit(json!({"event": "removed", "id": found.id}), || {
                    format!("Moved \"{}\" to com.mojang\\arctic-trash", found.name)
                });
        }
    }
    Ok(0)
}

fn find_world(player: &Player, query: &str) -> Result<World> {
    let list = worlds::list(player);
    list.iter()
        .find(|w| w.id == query)
        .or_else(|| list.iter().find(|w| w.name.eq_ignore_ascii_case(query)))
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "no world '{query}' (see `arctic bedrock worlds list`)"
            ))
        })
}

fn run_packs(ctx: &Ctx, edition: Edition, command: &BedrockPacksCommand) -> Result<i32> {
    let folders = bedrock::folders(edition);
    let shared = folders.packs.as_ref().ok_or_else(|| no_data(edition))?;
    match command {
        BedrockPacksCommand::List => {
            for p in packs::list(shared) {
                ctx.out.emit(pack_json(&p), || {
                    format!(
                        "{:<14} {:<32} {:<9} {}",
                        p.kind.name(),
                        p.name,
                        p.version,
                        if p.development { "development" } else { "" }
                    )
                });
            }
        }
        BedrockPacksCommand::Import { path } => {
            let names = packs::import(path, shared)?;
            ctx.out
                .emit(json!({"event": "imported", "packs": names}), || {
                    format!("Imported {}.", names.join(", "))
                });
        }
        BedrockPacksCommand::Remove { pack } => {
            let list = packs::list(shared);
            let query = pack.to_ascii_lowercase();
            let found = list
                .iter()
                .find(|p| p.uuid == query)
                .or_else(|| {
                    list.iter().find(|p| {
                        p.path
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(pack))
                    })
                })
                .or_else(|| list.iter().find(|p| p.name.eq_ignore_ascii_case(pack)))
                .ok_or_else(|| {
                    Error::Other(format!(
                        "no pack '{pack}' (see `arctic bedrock packs list`)"
                    ))
                })?;
            packs::trash(found, shared)?;
            ctx.out
                .emit(json!({"event": "removed", "uuid": found.uuid}), || {
                    format!("Moved \"{}\" to com.mojang\\arctic-trash", found.name)
                });
        }
    }
    Ok(0)
}

fn pack_json(p: &Pack) -> Value {
    json!({
        "kind": format!("{:?}", p.kind).to_lowercase(),
        "name": p.name,
        "description": p.description,
        "version": p.version,
        "uuid": p.uuid,
        "development": p.development,
        "path": p.path,
    })
}

fn run_servers(
    ctx: &Ctx,
    edition: Edition,
    account: Option<&str>,
    command: &BedrockServersCommand,
) -> Result<i32> {
    if let BedrockServersCommand::Ping { address } = command {
        let result = servers::ping(address);
        ctx.out.emit(
            json!({"address": address, "status": status_json(&result)}),
            || {
                let mut text = format!("{address}  {}", summary(&result));
                if let Ok(s) = &result {
                    if !s.status.motd.is_empty() {
                        text.push_str(&format!("\n{}", s.status.motd));
                    }
                    if !s.level.is_empty() {
                        text.push_str(&format!("\n{}", s.level));
                    }
                }
                text
            },
        );
        return Ok(i32::from(result.is_err()));
    }
    let folders = bedrock::folders(edition);
    let player = player(&folders, account, edition)?;
    match command {
        BedrockServersCommand::List { no_ping } => {
            let list = servers::list(player);
            if list.is_empty() {
                ctx.out
                    .info("No servers yet (add one with `arctic bedrock servers add`).");
                return Ok(0);
            }
            let addresses: Vec<String> = list.iter().map(|s| s.address()).collect();
            let results: std::sync::Mutex<Vec<Option<Result<BedrockStatus>>>> =
                std::sync::Mutex::new(addresses.iter().map(|_| None).collect());
            if !no_ping {
                servers::ping_each(&addresses, |i, r| {
                    if let Ok(mut all) = results.lock() {
                        all[i] = Some(r);
                    }
                });
            }
            let results = results.into_inner().unwrap_or_default();
            for (i, server) in list.iter().enumerate() {
                let status = results.get(i).and_then(Option::as_ref);
                let mut row = json!({"name": server.name, "address": addresses[i]});
                if let Some(r) = status {
                    row["status"] = status_json(r);
                }
                ctx.out.emit(row, || {
                    let state = status.map_or_else(String::new, |r| format!("  {}", summary(r)));
                    format!("{:<24} {:<28}{state}", server.name, addresses[i])
                });
            }
        }
        BedrockServersCommand::Add { address, name } => {
            let added = servers::add(player, name.as_deref().unwrap_or(""), address)?;
            ctx.out.emit(
                json!({"event": if added { "added" } else { "already_listed" }, "address": address}),
                || match added {
                    true => format!("Added {address} to the game's server list."),
                    false => format!("{address} is already on the list."),
                },
            );
        }
        BedrockServersCommand::Remove { address } => {
            if !servers::remove(player, address)? {
                return Err(Error::Other(format!("{address} isn't on the list")));
            }
            ctx.out
                .emit(json!({"event": "removed", "address": address}), || {
                    format!("Removed {address}.")
                });
        }
        BedrockServersCommand::Ping { .. } => unreachable!(),
    }
    Ok(0)
}

fn status_json(result: &Result<BedrockStatus>) -> Value {
    match result {
        Ok(s) => json!({
            "online": s.status.online,
            "max": s.status.max,
            "version": s.status.version,
            "protocol": s.status.protocol,
            "motd": s.status.motd,
            "level": s.level,
            "game_mode": s.game_mode,
            "ping_ms": s.status.ping_ms,
        }),
        Err(e) => json!({"error": e.to_string()}),
    }
}

fn summary(result: &Result<BedrockStatus>) -> String {
    match result {
        Ok(s) => format!(
            "{}/{} online  {} ms  {}",
            s.status.online, s.status.max, s.status.ping_ms, s.status.version
        ),
        Err(e) => format!("offline ({e})"),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
