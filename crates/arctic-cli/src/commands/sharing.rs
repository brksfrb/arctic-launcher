//! `arctic share …` and `arctic import …`: instances, HUD layouts,
//! crosshairs, client settings and whole profiles as codes, text or files.

use std::path::Path;

use arctic_core::settings::Settings;
use arctic_core::sharing::{self, Bundle, ClientPart, Input, InstancePack, Part, ProfilePack};
use arctic_core::{Error, Result, cosmetics};
use serde_json::json;

use super::{Ctx, fresh_account, instance_or_default};
use crate::cli::{ShareArgs, ShareAs, ShareWhat};

pub fn share(ctx: &Ctx, args: &ShareArgs) -> Result<i32> {
    let (bundle, left_out) = build(ctx, args)?;
    for file in &left_out {
        ctx.out.warn(&format!(
            "left out {file}: it isn't from Modrinth, so your friend needs it separately"
        ));
    }
    let summary = bundle.summary();
    match args.r#as {
        ShareAs::Code => {
            let account = fresh_account(ctx, args.account.as_deref())?;
            let base = cosmetics::base_url();
            let token = cosmetics::token_for(&ctx.dirs, &base, &account)?;
            let code = sharing::codes::create(&base, &token, &bundle)?;
            ctx.out.emit(
                json!({"kind": bundle.kind(), "code": code, "summary": summary}),
                || format!("{summary}\nCode: {code}\nThey can use it with `arctic import {code}` or in the launcher."),
            );
        }
        ShareAs::Text => {
            let text = bundle.to_text();
            ctx.out.emit(
                json!({"kind": bundle.kind(), "text": text, "summary": summary}),
                || text.clone(),
            );
        }
        ShareAs::File => {
            let path = args
                .out
                .clone()
                .unwrap_or_else(|| format!("arctic-{}.json", bundle.kind()).into());
            std::fs::write(&path, bundle.to_file_text()).map_err(|e| Error::io(&path, e))?;
            ctx.out.emit(
                json!({"kind": bundle.kind(), "file": path, "summary": summary}),
                || format!("{summary}\nSaved to {}", path.display()),
            );
        }
    }
    Ok(0)
}

fn build(ctx: &Ctx, args: &ShareArgs) -> Result<(Bundle, Vec<String>)> {
    let instance = instance_or_default(ctx, args.instance.as_deref())?;
    let game_dir = instance.game_dir(&ctx.dirs);
    let part = |p| Ok((Bundle::Client(ClientPart::read(&game_dir, p)?), Vec::new()));
    match args.what {
        ShareWhat::Instance => {
            let play = Settings::load(&ctx.dirs)?.last_version;
            let e = InstancePack::export(&ctx.dirs, &instance, play.as_deref(), false)?;
            if e.pack.game_version.is_none() {
                return Err(Error::Other(
                    "pick a version on the Play tab first (the Vanilla instance follows it)".into(),
                ));
            }
            Ok((Bundle::Instance(e.pack), e.left_out))
        }
        ShareWhat::Hud => part(Part::Hud),
        ShareWhat::Crosshair => part(Part::Crosshair),
        ShareWhat::Client => part(Part::All),
        ShareWhat::Profile => {
            let (pack, left_out) = ProfilePack::export(&ctx.dirs)?;
            Ok((Bundle::Profile(pack), left_out))
        }
    }
}

/// A code, share text, or a file path.
pub fn import(ctx: &Ctx, source: &str, instance: Option<&str>) -> Result<i32> {
    let bundle = resolve(source)?;
    ctx.out.info(&bundle.summary());
    let progress = |p: arctic_core::ProgressInfo| ctx.out.progress(p);
    let result = apply(ctx, bundle, instance, &progress);
    ctx.out.end_progress();
    result
}

fn apply(
    ctx: &Ctx,
    bundle: Bundle,
    instance: Option<&str>,
    progress: arctic_core::Progress,
) -> Result<i32> {
    match bundle {
        Bundle::Instance(pack) => {
            let (created, report) = pack.install(&ctx.dirs, progress)?;
            report_skipped(ctx, &report.skipped);
            ctx.out.emit(
                json!({"event": "imported", "kind": "instance", "id": created.id, "name": created.name,
                       "mods": report.installed.len(), "skipped": report.skipped.len()}),
                || format!("Created instance \"{}\" with {} mods. Launch it with `arctic launch --instance {}`.",
                    created.name, report.installed.len(), created.id),
            );
        }
        Bundle::Client(part) => {
            let target = instance_or_default(ctx, instance)?;
            part.apply(&target.game_dir(&ctx.dirs))?;
            ctx.out.emit(
                json!({"event": "imported", "kind": part_kind(part.part), "instance": target.id}),
                || format!("Applied to {}. Close its game first if it's running; it takes effect on the next start.", target.name),
            );
        }
        Bundle::Profile(pack) => {
            let report = pack.import(&ctx.dirs, progress)?;
            report_skipped(ctx, &report.skipped_mods);
            for name in &report.existing {
                ctx.out.warn(&format!(
                    "kept your own \"{name}\" (an instance with that name exists)"
                ));
            }
            ctx.out.emit(
                json!({"event": "imported", "kind": "profile",
                       "created": report.created.iter().map(|i| &i.id).collect::<Vec<_>>(),
                       "existing": report.existing}),
                || {
                    format!(
                        "Profile imported: settings applied, {} instance(s) created.",
                        report.created.len()
                    )
                },
            );
        }
    }
    Ok(0)
}

fn resolve(source: &str) -> Result<Bundle> {
    let path = Path::new(source);
    if source.ends_with(".json") || (source.len() < 1024 && path.is_file()) {
        return sharing::read_file(path);
    }
    match sharing::read(source)? {
        Input::Bundle(b) => Ok(b),
        Input::Code(code) => sharing::codes::fetch(&cosmetics::base_url(), &code),
    }
}

fn report_skipped(ctx: &Ctx, skipped: &[arctic_core::mods::Skipped]) {
    for s in skipped {
        ctx.out.warn(&format!("skipped {}: {}", s.title, s.reason));
    }
}

fn part_kind(part: Part) -> &'static str {
    match part {
        Part::Hud => "hud",
        Part::Crosshair => "crosshair",
        Part::All => "client",
    }
}
