//! `arctic skin …` (the Minecraft skin and capes Mojang shows everyone) and
//! `arctic look …` (the Arctic look: skin, cape and cosmetics other Arctic
//! players see).

use std::path::Path;

use arctic_core::auth::{Account, AccountKind};
use arctic_core::cosmetics::{self, CapeChoice, NewLook, Texture};
use arctic_core::skins::{self, Variant, api};
use arctic_core::{Error, Result, cosmetic_models};
use serde_json::json;

use super::{Ctx, fresh_account};
use crate::cli::{LookCommand, SkinCommand, SkinModel};

pub fn skin(ctx: &Ctx, command: &SkinCommand) -> Result<i32> {
    let account = fresh_account(ctx, command.account())?;
    let AccountKind::Microsoft(session) = &account.kind else {
        return Err(Error::Other(
            "Minecraft skins can only be changed on Microsoft accounts (try `arctic look`)".into(),
        ));
    };
    let token = session.access_token.reveal();
    let token: &str = &token;
    let profile = match command {
        SkinCommand::Show { .. } => api::profile(token)?,
        SkinCommand::Set { file, model, .. } => {
            let png = read_png(file)?;
            let variant = variant(&png, *model)?;
            api::upload_skin(token, variant, &png)?
        }
        SkinCommand::Reset { .. } => api::reset_skin(token)?,
        SkinCommand::Cape { cape, .. } => {
            let profile = api::profile(token)?;
            let id = match cape.as_str() {
                "none" => None,
                wanted => Some(
                    profile
                        .capes
                        .iter()
                        .find(|c| c.id == wanted || c.alias.eq_ignore_ascii_case(wanted))
                        .map(|c| c.id.clone())
                        .ok_or_else(|| {
                            Error::Other(format!("you don't own a cape called '{wanted}'"))
                        })?,
                ),
            };
            api::set_cape(token, id.as_deref())?
        }
    };
    let skin = profile.active_skin();
    let cape = profile.active_cape();
    ctx.out.emit(
        json!({
            "name": profile.name,
            "skin": skin.map(|s| json!({"url": s.url, "model": s.variant().api_name()})),
            "cape": cape.map(|c| &c.alias),
            "capes": profile.capes.iter().map(|c| json!({"id": c.id, "name": c.alias})).collect::<Vec<_>>(),
        }),
        || {
            let mut text = format!(
                "{}: {} skin, cape: {}",
                profile.name,
                skin.map_or("default", |s| s.variant().label()),
                cape.map_or("none", |c| c.alias.as_str())
            );
            for c in &profile.capes {
                text.push_str(&format!("\n    cape {} ({})", c.alias, c.id));
            }
            text
        },
    );
    Ok(0)
}

pub fn look(ctx: &Ctx, command: &LookCommand) -> Result<i32> {
    let account = fresh_account(ctx, command.account())?;
    let base = cosmetics::base_url();
    let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
    let presets = cosmetics::catalog(&base)?;
    // An older server has no 3D cosmetics; that's just an empty list.
    let items = cosmetic_models::catalog(&base)
        .map(|c| c.cosmetics)
        .unwrap_or_default();
    let current = cosmetics::my_look(&base, &token)?;
    let mut new = NewLook::from_look(&current, &presets);
    match command {
        LookCommand::Show { .. } => {
            show_look(ctx, &account, &current, &presets, &items, true);
            return Ok(0);
        }
        LookCommand::Skin { file, model, .. } => {
            new.skin = match file.to_str() {
                Some("none") => None,
                _ => {
                    let png = read_png(file)?;
                    let variant = variant(&png, *model)?;
                    Some((Texture::Png(png), variant))
                }
            };
        }
        LookCommand::Cape { cape, .. } => new.cape = cape_choice(cape, &presets)?,
        LookCommand::Wear { ids, .. } => {
            for id in ids {
                if !items.iter().any(|i| &i.id == id) {
                    return Err(Error::Other(format!(
                        "no cosmetic called '{id}' (see `arctic look show`)"
                    )));
                }
            }
            new.cosmetics = Some(ids.clone());
        }
    }
    let look = cosmetics::set_look(&base, &token, &new)?;
    show_look(ctx, &account, &look, &presets, &items, false);
    Ok(0)
}

/// A preset id or name, a PNG file, or `none`.
fn cape_choice(cape: &str, presets: &[cosmetics::Preset]) -> Result<Option<CapeChoice>> {
    if cape == "none" {
        return Ok(None);
    }
    if let Some(p) = presets
        .iter()
        .find(|p| p.id == cape || p.name.eq_ignore_ascii_case(cape))
    {
        return Ok(Some(CapeChoice::Preset(p.id.clone())));
    }
    let path = Path::new(cape);
    if !path.is_file() {
        return Err(Error::Other(format!(
            "'{cape}' is not a cape preset or a PNG file (see `arctic look show`)"
        )));
    }
    let png = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    if !cosmetics::is_cape_png(&png) {
        return Err(Error::Other(
            "that PNG isn't a cape (64x32, or a wider animated cape strip)".into(),
        ));
    }
    Ok(Some(CapeChoice::Custom(Texture::Png(png))))
}

fn show_look(
    ctx: &Ctx,
    account: &Account,
    look: &cosmetics::Look,
    presets: &[cosmetics::Preset],
    items: &[cosmetic_models::Item],
    with_choices: bool,
) {
    let cape_name = look.cape.as_deref().map(|hash| {
        presets
            .iter()
            .find(|p| p.texture == hash)
            .map_or("custom", |p| p.name.as_str())
    });
    ctx.out.emit(
        json!({
            "account": account.username,
            "skin": look.skin,
            "model": look.model,
            "cape": look.cape,
            "cosmetics": look.cosmetics,
            "presets": presets.iter().map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>(),
            "items": items.iter().map(|i| json!({"id": i.id, "name": i.name, "slot": i.slot})).collect::<Vec<_>>(),
        }),
        || {
            let mut text = format!(
                "{}: {} skin, cape: {}, wearing: {}",
                account.username,
                if look.skin.is_some() { look.variant().label() } else { "Minecraft" },
                cape_name.unwrap_or("none"),
                if look.cosmetics.is_empty() { "nothing".into() } else { look.cosmetics.join(", ") }
            );
            if !with_choices {
                return text;
            }
            text.push_str("\nCapes:");
            for p in presets {
                text.push_str(&format!("\n    {:<16} {}", p.id, p.name));
            }
            text.push_str("\nCosmetics (one per slot):");
            for i in items {
                text.push_str(&format!("\n    {:<16} {} ({})", i.id, i.name, i.slot));
            }
            text
        },
    );
}

fn read_png(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::io(path, e))
}

/// `--model`, or a guess from the image; also checks it's a valid skin.
fn variant(png: &[u8], model: Option<SkinModel>) -> Result<Variant> {
    let image = skins::decode(png)?;
    Ok(match model {
        Some(SkinModel::Slim) => Variant::Slim,
        Some(SkinModel::Classic) => Variant::Classic,
        None => image.guess_variant(),
    })
}
