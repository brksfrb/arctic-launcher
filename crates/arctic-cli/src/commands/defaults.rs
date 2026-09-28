//! `arctic defaults …`: default game settings for new instances.

use arctic_core::Result;
use arctic_core::game_defaults::{COMMON, Defaults};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::DefaultsCommand;

pub fn run(ctx: &Ctx, command: &DefaultsCommand) -> Result<i32> {
    let mut d = Defaults::load(&ctx.dirs)?;
    match command {
        DefaultsCommand::Show => {
            if d.is_empty() {
                ctx.out
                    .info("No defaults: new instances start with Minecraft's own settings.");
            }
            for (k, v) in &d.values {
                let label = COMMON.iter().find(|c| c.key == k.as_str());
                ctx.out.emit(json!({"key": k, "value": v}), || match label {
                    Some(c) => format!("{:<28} {:<10} ({})", k, v, c.kind.show(v)),
                    None => format!("{k:<28} {v}"),
                });
            }
        }
        DefaultsCommand::Set { key, value } => {
            d.set(key, value)?;
            d.save(&ctx.dirs)?;
            ctx.out
                .emit(json!({"event": "set", "key": key, "value": value}), || {
                    format!("{key} = {value}")
                });
        }
        DefaultsCommand::Unset { key } => {
            d.values.remove(key);
            d.save(&ctx.dirs)?;
            ctx.out.emit(json!({"event": "unset", "key": key}), || {
                format!("{key} left at the game's default")
            });
        }
        DefaultsCommand::Capture { instance } => {
            let instance = instance_or_default(ctx, instance.as_deref())?;
            let captured = Defaults::capture(&instance.game_dir(&ctx.dirs))?;
            captured.save(&ctx.dirs)?;
            ctx.out.emit(json!({"event": "captured", "count": captured.values.len(), "instance": instance.id}), || {
                format!("Copied {} settings from {} as the defaults.", captured.values.len(), instance.name)
            });
        }
        DefaultsCommand::Apply { instance } => {
            let instance = instance_or_default(ctx, instance.as_deref())?;
            let n = d.apply_to(&instance.game_dir(&ctx.dirs), instance.version.as_deref())?;
            ctx.out.emit(
                json!({"event": "applied", "count": n, "instance": instance.id}),
                || format!("Applied {n} settings to {}.", instance.name),
            );
        }
        DefaultsCommand::Clear => {
            Defaults::default().save(&ctx.dirs)?;
            ctx.out.emit(json!({"event": "cleared"}), || {
                "Defaults cleared.".to_owned()
            });
        }
    }
    Ok(0)
}
