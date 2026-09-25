//! `arctic launch`.

use std::sync::mpsc;

use arctic_core::ProgressInfo;
use arctic_core::auth::{self, Account, AccountStore, offline};
use arctic_core::launch::{self, GameEvent, LaunchRequest};
use arctic_core::settings::Settings;
use arctic_core::versions::VersionManifest;
use arctic_core::{Error, Result, instances};
use serde_json::json;

use super::{Ctx, resolve_version};
use crate::cli::LaunchArgs;
use crate::output::parse_memory;

pub fn run(ctx: &Ctx, args: &LaunchArgs) -> Result<i32> {
    let settings = apply_overrides(Settings::load(&ctx.dirs)?, args)?;
    let manifest = VersionManifest::fetch(&ctx.dirs)?;
    let entry = resolve_version(&manifest, &args.version)?;
    let account = pick_account(ctx, args)?;
    let instance = instances::load_default(&ctx.dirs)?;

    let progress = |p: ProgressInfo| ctx.out.progress(p);
    let req = LaunchRequest {
        dirs: &ctx.dirs,
        version: &entry,
        instance: &instance,
        account: &account,
        settings: &settings,
    };
    let plan = launch::prepare(&req, &progress)?;
    ctx.out.end_progress();

    if args.dry_run {
        ctx.out.emit(
            json!({"event": "dry_run", "version": entry.id, "command": plan.redacted_command()}),
            || plan.redacted_command(),
        );
        return Ok(0);
    }
    if !args.wait {
        let pid = launch::spawn_detached(&plan)?;
        ctx.out.emit(
            json!({"event": "launched", "version": entry.id, "account": account.username, "pid": pid, "log": plan.log_file}),
            || format!("Launched Minecraft {} as {} (pid {pid}). Log: {}", entry.id, account.username, plan.log_file.display()),
        );
        return Ok(0);
    }
    wait_for_game(ctx, &plan, &entry.id, &account.username)
}

/// Stay attached: stream the parsed game log, return the game's exit code.
fn wait_for_game(
    ctx: &Ctx,
    plan: &launch::LaunchPlan,
    version: &str,
    username: &str,
) -> Result<i32> {
    let (tx, rx) = mpsc::channel();
    let game = launch::spawn(plan, move |event| {
        let _ = tx.send(event);
    })?;
    ctx.out.emit(
        json!({"event": "launched", "version": version, "account": username, "pid": game.pid()}),
        || {
            format!(
                "Launched Minecraft {version} as {username} (pid {})",
                game.pid()
            )
        },
    );
    for event in rx {
        match event {
            GameEvent::Output(lines) => {
                for line in lines {
                    ctx.out.emit(
                        json!({"event": "log", "level": format!("{:?}", line.level).to_lowercase(), "text": line.text}),
                        || line.text.clone(),
                    );
                }
            }
            GameEvent::WindowReady => ctx.out.emit(json!({"event": "window_ready"}), || {
                "── Game window is up ──".into()
            }),
            GameEvent::Exited { code } => {
                ctx.out.emit(json!({"event": "exited", "code": code}), || {
                    format!(
                        "Minecraft exited ({})",
                        code.map_or("killed".into(), |c| c.to_string())
                    )
                });
                return Ok(code.unwrap_or(1));
            }
        }
    }
    Ok(1)
}

fn apply_overrides(settings: Settings, args: &LaunchArgs) -> Result<Settings> {
    let max_memory_mb = match &args.memory {
        Some(text) => parse_memory(text)
            .ok_or_else(|| Error::Other(format!("invalid --memory '{text}' (try 4G or 4096M)")))?,
        None => settings.max_memory_mb,
    };
    Ok(Settings {
        max_memory_mb,
        window_width: args.width.unwrap_or(settings.window_width),
        window_height: args.height.unwrap_or(settings.window_height),
        fullscreen: args.fullscreen || settings.fullscreen,
        java_override: args.java.clone().or(settings.java_override.clone()),
        ..settings
    }
    .sanitized())
}

/// `--offline NAME`, `--account QUERY`, or the active account. Microsoft
/// sessions are refreshed (and saved) when needed.
fn pick_account(ctx: &Ctx, args: &LaunchArgs) -> Result<Account> {
    let mut store = AccountStore::load(&ctx.dirs)?;
    if let Some(name) = &args.offline {
        let account = offline::create(name)?;
        if args.save {
            store.upsert(account.clone());
            store.save(&ctx.dirs)?;
        }
        return Ok(account);
    }
    let account = match &args.account {
        Some(query) => store
            .find(query)
            .cloned()
            .ok_or_else(|| Error::Other(format!("no account matches '{query}' (see `arctic accounts list`)")))?,
        None => store.active().cloned().ok_or_else(|| {
            Error::Other("no account selected: use --offline NAME, --account NAME, or `arctic accounts add-offline`".into())
        })?,
    };
    let (fresh, changed) = auth::ensure_fresh(&ctx.dirs, &account)?;
    if changed {
        store.upsert(fresh.clone());
        store.save(&ctx.dirs)?;
    }
    Ok(fresh)
}
