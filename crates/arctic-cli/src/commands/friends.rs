//! `arctic friends …`: your Arctic profile, friends and invites.

use arctic_core::friends::{self, InviteTo, Overview};
use arctic_core::{Error, Result, cosmetics};
use serde_json::json;

use super::{Ctx, fresh_account};
use crate::cli::FriendsCommand;

pub fn run(ctx: &Ctx, command: &FriendsCommand) -> Result<i32> {
    let base = cosmetics::base_url();
    let account = fresh_account(ctx, command.account())?;
    // Before any sign-in: the name still belongs to the old PC's key.
    if let FriendsCommand::Restore { code, .. } = command {
        cosmetics::recover_offline(&ctx.dirs, &base, &account, code)?;
        ctx.out.emit(
            json!({"event": "restored", "account": account.username}),
            || {
                format!(
                    "{}'s profile and friends are on this PC now.",
                    account.username
                )
            },
        );
        return Ok(0);
    }
    let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
    match command {
        FriendsCommand::Restore { .. } => {}
        FriendsCommand::Chat { name, send, .. } => {
            let o = friends::overview(&base, &token)?;
            let who = find(
                &o.friends
                    .iter()
                    .map(|f| (&f.id, &f.name))
                    .collect::<Vec<_>>(),
                name,
            )?;
            match send {
                Some(text) => {
                    let m = friends::send(&base, &token, &who, text)?;
                    ctx.out.emit(json!({"event": "sent", "id": m.id}), || {
                        format!("Sent to {name}.")
                    });
                }
                None => {
                    for m in friends::history(&base, &token, &who, None)? {
                        let from = if m.from == who { name.as_str() } else { "you" };
                        ctx.out.emit(
                            json!({"id": m.id, "from": from, "text": m.text, "sent": m.sent}),
                            || format!("{from}: {}", m.text),
                        );
                    }
                    friends::mark_read(&base, &token, &who)?;
                }
            }
        }
        FriendsCommand::Recovery { .. } => {
            let code = friends::new_recovery(&base, &token)?;
            ctx.out.emit(json!({"code": code}), || {
                format!(
                    "Recovery code: {code}\nSave it: it's shown only once. On a new PC:\n  arctic friends restore {code} --account <name>"
                )
            });
        }
        FriendsCommand::List { .. } => {
            let o = friends::overview(&base, &token)?;
            list(ctx, &o);
        }
        FriendsCommand::Add { name, .. } => {
            let r = friends::request(&base, &token, name)?;
            ctx.out.emit(
                json!({"event": if r.friends { "friends" } else { "requested" }, "name": r.name}),
                || {
                    if r.friends {
                        format!("You and {} are friends now.", r.name)
                    } else {
                        format!("Asked {} to be friends.", r.name)
                    }
                },
            );
        }
        FriendsCommand::Accept { name, .. } => {
            let o = friends::overview(&base, &token)?;
            let who = find(
                &o.incoming
                    .iter()
                    .map(|p| (&p.id, &p.name))
                    .collect::<Vec<_>>(),
                name,
            )?;
            friends::accept(&base, &token, &who)?;
            ctx.out
                .emit(json!({"event": "accepted", "name": name}), || {
                    format!("You and {name} are friends now.")
                });
        }
        FriendsCommand::Remove { name, .. } => {
            let o = friends::overview(&base, &token)?;
            let everyone: Vec<(&String, &String)> = o
                .friends
                .iter()
                .map(|f| (&f.id, &f.name))
                .chain(o.incoming.iter().map(|p| (&p.id, &p.name)))
                .chain(o.outgoing.iter().map(|p| (&p.id, &p.name)))
                .collect();
            let who = find(&everyone, name)?;
            friends::remove(&base, &token, &who)?;
            ctx.out.emit(json!({"event": "removed", "name": name}), || {
                format!("Removed {name}.")
            });
        }
        FriendsCommand::Invite {
            name,
            server,
            together,
            ..
        } => {
            let to = match (server, together) {
                (Some(s), _) => InviteTo::Server(s.clone()),
                (_, Some(c)) => InviteTo::Together(c.clone()),
                _ => {
                    return Err(Error::Other(
                        "say --server <ADDRESS> or --together <CODE>".into(),
                    ));
                }
            };
            let o = friends::overview(&base, &token)?;
            let who = find(
                &o.friends
                    .iter()
                    .map(|f| (&f.id, &f.name))
                    .collect::<Vec<_>>(),
                name,
            )?;
            friends::invite(&base, &token, &who, &to)?;
            ctx.out.emit(json!({"event": "invited", "name": name}), || {
                format!("Invited {name}.")
            });
        }
        FriendsCommand::Profile {
            name,
            share_online,
            share_server,
            invites,
            show_accounts,
            link,
            unlink,
            ..
        } => {
            let mut profile = friends::profile(&base, &token)?;
            let mut settings = profile.settings.clone();
            let mut changed = name.is_some();
            for (flag, value) in [
                (share_online, &mut settings.share_online),
                (share_server, &mut settings.share_server),
                (invites, &mut settings.allow_invites),
                (show_accounts, &mut settings.show_accounts),
            ] {
                if let Some(v) = flag {
                    *value = *v;
                    changed = true;
                }
            }
            if changed {
                profile = friends::update(&base, &token, name.as_deref(), &settings)?;
            }
            if let Some(other) = link {
                let other = fresh_account(ctx, Some(other))?;
                let other_token = cosmetics::token_for(&ctx.dirs, &base, &other)?;
                profile = friends::link(&base, &token, &other_token)?;
            }
            if let Some(who) = unlink {
                let uuid = profile
                    .accounts
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(who) || a.uuid == *who)
                    .map(|a| a.uuid.clone())
                    .ok_or_else(|| Error::Other(format!("{who} isn't on this profile")))?;
                profile = friends::unlink(&base, &token, &uuid)?;
            }
            let s = &profile.settings;
            let accounts: Vec<&str> = profile.accounts.iter().map(|a| a.name.as_str()).collect();
            ctx.out.emit(
                json!({"id": profile.id, "name": profile.name, "accounts": accounts, "settings": s}),
                || {
                    format!(
                        "{} [friend code {}] (accounts: {})\n  online shown: {}  server shown: {}  invites: {}  accounts shown: {}",
                        profile.name,
                        profile.code,
                        accounts.join(", "),
                        s.share_online,
                        s.share_server,
                        s.allow_invites,
                        s.show_accounts
                    )
                },
            );
        }
    }
    Ok(0)
}

fn list(ctx: &Ctx, o: &Overview) {
    for i in &o.invites {
        ctx.out.emit(
            json!({"invite": i.id, "from": i.from.name, "kind": i.kind, "target": i.target}),
            || format!("Invite from {} to {}", i.from.name, i.target),
        );
    }
    for p in &o.incoming {
        ctx.out.emit(json!({"request_from": p.name}), || {
            format!(
                "{} wants to be friends (arctic friends accept {})",
                p.name, p.name
            )
        });
    }
    for p in &o.outgoing {
        ctx.out.emit(json!({"requested": p.name}), || {
            format!("Asked {} (waiting)", p.name)
        });
    }
    if o.friends.is_empty() {
        ctx.out
            .info("No friends yet: arctic friends add <friend code>");
    }
    for f in &o.friends {
        let status = match (&f.server, f.in_game, f.online) {
            (Some(s), _, _) => format!("playing on {s}"),
            (None, true, _) => "in game".into(),
            (None, false, true) => "online".into(),
            _ => "offline".into(),
        };
        ctx.out.emit(
            json!({"id": f.id, "name": f.name, "online": f.online, "in_game": f.in_game, "server": f.server}),
            || format!("{:<20} {status}", f.name),
        );
    }
}

/// The profile id of the one person called `name` in `people`.
fn find(people: &[(&String, &String)], name: &str) -> Result<String> {
    people
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(name))
        .map(|(id, _)| (*id).clone())
        .ok_or_else(|| Error::Other(format!("nobody called {name} there")))
}
