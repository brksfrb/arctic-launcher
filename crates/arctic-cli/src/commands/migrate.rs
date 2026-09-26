//! `arctic migrate …`: bring instances and HUD setups over from other
//! launchers and clients.

use std::path::PathBuf;

use arctic_core::migrate::{self, Category, Found, FoundClient, Scan};
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::MigrateCommand;

const MB: f64 = 1024.0 * 1024.0;

pub fn run(ctx: &Ctx, command: &MigrateCommand) -> Result<i32> {
    match command {
        MigrateCommand::List { folder } => list(ctx, &scan(folder.as_ref())),
        MigrateCommand::Instance {
            which,
            folder,
            skip,
            name,
        } => {
            let scan = scan(folder.as_ref());
            let found = pick(&scan.instances, which, Found::key, |f| &f.name)?;
            import_one(ctx, found, skip, name.as_deref())?;
            Ok(0)
        }
        MigrateCommand::All { folder, skip } => {
            let scan = scan(folder.as_ref());
            let mut failed = 0;
            for found in &scan.instances {
                if let Err(e) = import_one(ctx, found, skip, None) {
                    failed += 1;
                    ctx.out.warn(&format!("{}: {e}", found.key()));
                }
            }
            Ok(i32::from(failed > 0))
        }
        MigrateCommand::Hud {
            which,
            instance,
            on,
        } => {
            let scan = migrate::scan();
            let client = pick(&scan.clients, which, FoundClient::key, |c| &c.profile)?;
            let on = on.clone().unwrap_or_else(|| client.guesses());
            let target = instance_or_default(ctx, instance.as_deref())?;
            client
                .with_choices(&on)
                .apply(&target.game_dir(&ctx.dirs))?;
            ctx.out.emit(
                json!({"event": "imported", "kind": "hud", "from": client.key(), "instance": target.id, "on": on}),
                || format!("Applied {}'s HUD to {}.", client.key(), target.name),
            );
            Ok(0)
        }
    }
}

fn scan(folder: Option<&PathBuf>) -> Scan {
    match folder {
        Some(f) => migrate::scan_folder(f),
        None => migrate::scan(),
    }
}

fn list(ctx: &Ctx, scan: &Scan) -> Result<i32> {
    let all_sizes = migrate::measure_all(&scan.instances);
    for (f, sizes) in scan.instances.iter().zip(all_sizes) {
        let loader = f.loader.as_ref().map_or("Vanilla".to_owned(), |l| {
            format!(
                "{} {}",
                l.kind.label(),
                l.version.as_deref().unwrap_or("(newest)")
            )
        });
        let parts: Vec<String> = Category::ALL
            .iter()
            .filter_map(|c| {
                let n = sizes.counts.get(c).copied().unwrap_or(0);
                (n > 0).then(|| {
                    let mb = sizes.bytes.get(c).copied().unwrap_or(0) as f64 / MB;
                    format!("{} {n} ({mb:.0} MB)", c.label().to_lowercase())
                })
            })
            .collect();
        ctx.out.emit(
            json!({"key": f.key(), "launcher": f.launcher.id(), "name": f.name, "game_version": f.game_version,
                   "loader": loader, "game_dir": f.game_dir, "into_vanilla": f.into_vanilla,
                   "memory_mb": f.memory_mb, "notes": f.notes,
                   "sizes": Category::ALL.iter().map(|c| (c.id(), sizes.bytes.get(c).copied().unwrap_or(0))).collect::<std::collections::HashMap<_, _>>(),
                   "other_loader_mods": sizes.other_loader.len()}),
            || {
                let target = if f.into_vanilla { " → your Vanilla instance".to_owned() } else { String::new() };
                let mut text = format!("{}\n    {} · {loader}{target}\n    {}", f.key(), f.game_version, parts.join(", "));
                if !sizes.other_loader.is_empty() {
                    text.push_str(&format!("\n    ({} mods in this shared folder are for another loader; left out)", sizes.other_loader.len()));
                }
                for n in &f.notes {
                    text.push_str(&format!("\n    note: {n}"));
                }
                text
            },
        );
    }
    for c in &scan.clients {
        let on = c.settings.widgets_on().unwrap_or(0);
        ctx.out.emit(
            json!({"key": c.key(), "kind": "hud", "widgets_on": on,
                   "unsure": c.unsure.iter().map(|u| json!({"widget": u.widget, "label": u.label, "guess": u.guess})).collect::<Vec<_>>(),
                   "notes": c.notes}),
            || {
                let mut text = format!("{} (HUD and keys)\n    {on} widgets on", c.key());
                if !c.unsure.is_empty() {
                    let list: Vec<String> = c.unsure.iter().map(|u| format!("{}{}", u.widget, if u.guess { "?" } else { "" })).collect();
                    text.push_str(&format!("\n    not saved as on or off: {} (? = guessed on; choose with --on)", list.join(", ")));
                }
                for n in &c.notes {
                    text.push_str(&format!("\n    note: {n}"));
                }
                text
            },
        );
    }
    for (launcher, why) in &scan.seen {
        ctx.out
            .emit(json!({"launcher": launcher.id(), "note": why}), || {
                format!("{}: {why}", launcher.name())
            });
    }
    Ok(0)
}

fn import_one(ctx: &Ctx, found: &Found, skip: &[String], name: Option<&str>) -> Result<()> {
    let categories: Vec<Category> = Category::ALL
        .into_iter()
        .filter(|c| !skip.iter().any(|s| s.eq_ignore_ascii_case(c.id())))
        .collect();
    let progress = |p: arctic_core::ProgressInfo| ctx.out.progress(p);
    let result = migrate::import(&ctx.dirs, found, &categories, name, &progress);
    ctx.out.end_progress();
    let done = result?;
    for n in &done.notes {
        ctx.out.warn(n);
    }
    let rec = done.recognized.clone().unwrap_or_default();
    for m in &done.added {
        ctx.out.info(&format!("added missing {m}"));
    }
    for m in &done.turned_off {
        ctx.out.warn(&format!(
            "{m} doesn't run on Minecraft {}: it came switched off",
            found.game_version
        ));
    }
    let off = |m: &String| done.turned_off.iter().any(|t| m.starts_with(t.as_str()));
    for m in rec.mismatched.iter().filter(|m| !off(m)) {
        ctx.out.warn(&format!(
            "{m}: Modrinth lists it for other versions (kept on; its own files allow {})",
            found.game_version
        ));
    }
    ctx.out.emit(
        json!({"event": "imported", "from": found.key(), "instance": done.instance.id, "created": done.created,
               "files": done.files, "kept": done.kept, "mods": done.mods, "recognized": rec.tracked,
               "unknown_mods": rec.unknown, "other_loader": done.other_loader.len()}),
        || {
            let into = if done.created { "new instance" } else { "instance" };
            let mut text = format!(
                "{} → {into} \"{}\": {} files copied",
                found.key(),
                done.instance.name,
                done.files
            );
            if done.kept > 0 {
                text.push_str(&format!(", {} already there (kept yours)", done.kept));
            }
            if done.mods > 0 {
                text.push_str(&format!(", {} mods ({} recognized on Modrinth)", done.mods, rec.tracked));
            }
            text
        },
    );
    Ok(())
}

/// By exact key, then by name (case-insensitive), then a unique partial match.
fn pick<'a, T>(
    items: &'a [T],
    which: &str,
    key: impl Fn(&T) -> String,
    name: impl Fn(&T) -> &String,
) -> Result<&'a T> {
    let w = which.to_lowercase();
    if let Some(t) = items.iter().find(|t| key(t).to_lowercase() == w) {
        return Ok(t);
    }
    if let Some(t) = items.iter().find(|t| name(t).to_lowercase() == w) {
        return Ok(t);
    }
    let partial: Vec<&T> = items
        .iter()
        .filter(|t| key(t).to_lowercase().contains(&w))
        .collect();
    match partial.as_slice() {
        [one] => Ok(one),
        [] => Err(Error::Other(format!(
            "nothing matches '{which}' (see `arctic migrate list`)"
        ))),
        _ => Err(Error::Other(format!(
            "'{which}' matches {} things; use the full key from `arctic migrate list`",
            partial.len()
        ))),
    }
}
