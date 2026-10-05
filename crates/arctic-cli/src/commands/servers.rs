//! `arctic servers …`: an instance's multiplayer list with live status.

use arctic_core::proxy::ProxySettings;
use arctic_core::servers::public::{self, Access};
use arctic_core::servers::{self, Status};
use arctic_core::{Error, Result, cosmetics};
use serde_json::{Value, json};

use super::{Ctx, fresh_account, instance_or_default};
use crate::cli::ServersCommand;

pub fn run(ctx: &Ctx, command: &ServersCommand) -> Result<i32> {
    let proxy = ProxySettings::load(&ctx.dirs);
    match command {
        ServersCommand::List { instance, no_ping } => {
            let instance = instance_or_default(ctx, instance.as_deref())?;
            let list = servers::list(&instance.game_dir(&ctx.dirs))?;
            if list.is_empty() {
                ctx.out.info(&format!(
                    "{} has no saved servers (add them in Minecraft's multiplayer list).",
                    instance.name
                ));
                return Ok(0);
            }
            let addresses: Vec<String> = list.iter().map(|s| s.address.clone()).collect();
            let results = if *no_ping {
                Vec::new()
            } else {
                servers::ping_all(&addresses, proxy.active())
            };
            for (i, server) in list.iter().enumerate() {
                let status = results.get(i);
                let mut row = json!({"name": server.name, "address": server.address});
                if let Some(r) = status {
                    row["status"] = status_json(r);
                }
                ctx.out.emit(row, || {
                    let state = status.map_or_else(String::new, |r| format!("  {}", summary(r)));
                    format!("{:<24} {:<28}{state}", server.name, server.address)
                });
            }
        }
        ServersCommand::Ping { address } => {
            let result = servers::ping(address, proxy.active());
            ctx.out.emit(
                json!({"address": address, "status": status_json(&result)}),
                || {
                    let mut text = format!("{address}  {}", summary(&result));
                    if let Ok(s) = &result {
                        if !s.motd.is_empty() {
                            text.push_str(&format!("\n{}", s.motd));
                        }
                        if !s.players.is_empty() {
                            text.push_str(&format!("\nOnline: {}", s.players.join(", ")));
                        }
                    }
                    text
                },
            );
            return Ok(i32::from(result.is_err()));
        }
        ServersCommand::Browse {
            premium,
            cracked,
            random,
        } => browse(ctx, access(*premium, *cracked), *random)?,
        ServersCommand::Add {
            address,
            name,
            instance,
        } => {
            let instance = instance_or_default(ctx, instance.as_deref())?;
            let name = name.as_deref().unwrap_or("");
            let added = servers::add(&instance.game_dir(&ctx.dirs), name, address)?;
            ctx.out.emit(
                json!({"event": if added { "added" } else { "already_listed" }, "address": address, "instance": instance.id}),
                || match added {
                    true => format!("Added {address} to {}'s server list.", instance.name),
                    false => format!("{address} is already on {}'s server list.", instance.name),
                },
            );
        }
        ServersCommand::Move {
            address,
            before,
            instance,
        } => {
            let instance = instance_or_default(ctx, instance.as_deref())?;
            let moved =
                servers::move_before(&instance.game_dir(&ctx.dirs), address, before.as_deref())?;
            if !moved {
                return Err(Error::Other(
                    "that server (or the one to put it before) isn't on the list".into(),
                ));
            }
            ctx.out.emit(
                json!({"event": "moved", "address": address, "instance": instance.id}),
                || format!("Moved {address} in {}'s server list.", instance.name),
            );
        }
        ServersCommand::Submit {
            address,
            name,
            description,
            tags,
            account,
        } => {
            let base = cosmetics::base_url();
            let account = fresh_account(ctx, account.as_deref())?;
            let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
            let s = public::submit(&base, &token, address, name, description, tags)?;
            ctx.out.emit(
                json!({"event": "submitted", "id": s.id, "address": s.address, "code": s.code}),
                || {
                    format!(
                        "Put \"{}\" anywhere in {}'s MOTD, then run:\n  arctic servers verify {}\nAfter that it waits for approval; the code can go once it's listed.",
                        s.code, s.address, s.id
                    )
                },
            );
        }
        ServersCommand::Verify { id, account } => {
            let base = cosmetics::base_url();
            let account = fresh_account(ctx, account.as_deref())?;
            let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
            let state = public::verify(&base, &token, id)?;
            ctx.out.emit(
                json!({"event": "verified", "id": id, "state": state}),
                || match state.as_str() {
                    "pending" => "Found the code. Your server now waits for approval.".to_owned(),
                    other => format!("Its state is: {other}"),
                },
            );
        }
        ServersCommand::Review {
            state,
            approve,
            reject,
            remove,
        } => review(ctx, state, approve, reject, remove)?,
    }
    Ok(0)
}

fn access(premium: bool, cracked: bool) -> Access {
    match (premium, cracked) {
        (true, _) => Access::Premium,
        (_, true) => Access::Cracked,
        _ => Access::Any,
    }
}

fn browse(ctx: &Ctx, access: Access, random: bool) -> Result<()> {
    let list = public::browse(&cosmetics::base_url())?;
    let shown: Vec<&public::PublicServer> = if random {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos().into());
        public::pick(&list, access, seed).into_iter().collect()
    } else {
        list.iter().filter(|s| access.allows(s)).collect()
    };
    if shown.is_empty() {
        ctx.out.info("No servers match.");
    }
    for s in shown {
        let kind = match s.cracked {
            Some(true) => "cracked",
            Some(false) => "premium",
            None => "unknown",
        };
        ctx.out.emit(
            json!({"id": s.id, "name": s.name, "address": s.address, "tags": s.tags, "access": kind,
                "players": s.players, "max_players": s.max_players, "version": s.version}),
            || {
                format!(
                    "{:<22} {:<28} {:>6}/{:<6} {:<8} {}",
                    s.name,
                    s.address,
                    s.players,
                    s.max_players,
                    kind,
                    s.tags.join(", ")
                )
            },
        );
    }
    Ok(())
}

fn review(
    ctx: &Ctx,
    state: &str,
    approve: &Option<String>,
    reject: &Option<String>,
    remove: &Option<String>,
) -> Result<()> {
    let key = std::env::var("ARCTIC_ADMIN_KEY")
        .map_err(|_| Error::Other("set ARCTIC_ADMIN_KEY to the server's admin key".into()))?;
    let base = cosmetics::base_url();
    let action = [("approve", approve), ("reject", reject), ("remove", remove)]
        .into_iter()
        .find_map(|(a, id)| id.as_ref().map(|id| (a, id)));
    if let Some((action, id)) = action {
        public::decide(&base, &key, id, action)?;
        ctx.out.emit(json!({"event": action, "id": id}), || {
            format!("Done: {action} {id}")
        });
        return Ok(());
    }
    for s in public::review(&base, &key, state)? {
        ctx.out.emit(s.clone(), || {
            format!(
                "{}  {:<22} {:<28} {}/{}  cracked={}  owner={}  code={}",
                s["id"].as_str().unwrap_or(""),
                s["name"].as_str().unwrap_or(""),
                s["address"].as_str().unwrap_or(""),
                s["players"],
                s["max_players"],
                s["cracked"],
                s["owner"].as_str().unwrap_or("-"),
                s["code"].as_str().unwrap_or("-"),
            )
        });
    }
    Ok(())
}

fn status_json(result: &Result<Status>) -> Value {
    match result {
        Ok(s) => json!({
            "online": true, "players_online": s.online, "players_max": s.max,
            "players": s.players, "version": s.version, "protocol": s.protocol,
            "motd": s.motd, "ping_ms": s.ping_ms,
        }),
        Err(e) => json!({"online": false, "error": e.to_string()}),
    }
}

fn summary(result: &Result<Status>) -> String {
    match result {
        Ok(s) if s.ping_ms > 0 => format!("{}/{} online, {} ms", s.online, s.max, s.ping_ms),
        Ok(s) => format!("{}/{} online", s.online, s.max),
        Err(e) => format!("offline ({e})"),
    }
}
