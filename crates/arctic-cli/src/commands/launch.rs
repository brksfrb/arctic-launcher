//! `arctic launch`.

use std::sync::{Mutex, mpsc};

use arctic_core::ProgressInfo;
#[cfg(feature = "offline-accounts")]
#[cfg(feature = "offline-accounts")]
use arctic_core::auth::offline;
use arctic_core::auth::{self, Account, AccountStore};
use arctic_core::launch::{self, GameEvent, LaunchRequest, QuickPlay};
use arctic_core::mods::malware::{self, Reason};
use arctic_core::settings::Settings;
use arctic_core::versions::VersionManifest;
use arctic_core::{Error, Result, instances};
use serde_json::json;

use super::{Ctx, fresh_account, resolve_version};
use crate::cli::LaunchArgs;
use crate::output::parse_memory;

pub fn run(ctx: &Ctx, args: &LaunchArgs) -> Result<i32> {
    let settings = apply_overrides(Settings::load(&ctx.dirs)?, args)?;
    let manifest = VersionManifest::fetch_for_launch(&ctx.dirs)?;
    let instance = match &args.instance {
        Some(query) => instances::find(&ctx.dirs, query)?,
        None => instances::load_default(&ctx.dirs)?,
    };
    // Custom instances pin their Minecraft version.
    let version = instance.version.as_deref().unwrap_or(&args.version);
    let entry = resolve_version(&manifest, version)?;
    let account = pick_account(ctx, args)?;

    let progress = |p: ProgressInfo| ctx.out.progress(p);
    // Staying attached keeps this process alive: it can switch accounts
    // for the game too.
    let bridge = args
        .wait
        .then(|| {
            arctic_core::bridge::start(CliAccounts {
                dirs: ctx.dirs.clone(),
                session_updates: Mutex::new(()),
            })
            .ok()
        })
        .flatten();
    let req = LaunchRequest {
        dirs: &ctx.dirs,
        version: &entry,
        instance: &instance,
        account: &account,
        settings: &settings,
        bridge: bridge.as_ref(),
        copy: 0,
        quick_play: quick_play(args)?,
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
    if settings.check_mods_for_malware {
        let findings = malware::scan(&ctx.dirs, &plan.game_dir.join("mods"));
        if !findings.is_empty() {
            for finding in findings {
                let path = finding.path.display().to_string();
                let severity = if finding.reason == Reason::KnownMalware {
                    "Known malware"
                } else {
                    "Suspicious"
                };
                let reason = match &finding.reason {
                    Reason::KnownMalware => "file hash matches the blocklist",
                    Reason::Filename(reason) => reason,
                    Reason::Fractureiser => "Fractureiser indicators",
                };
                ctx.out.emit(
                    json!({"event": "mod_malware_warning", "file": path, "severity": severity, "reason": reason}),
                    || format!(
                        "{}: {severity} ({})",
                        path.replace(char::is_control, "�"),
                        reason.replace(char::is_control, "�"),
                    ),
                );
            }
            if !args.allow_flagged_mods {
                return Err(Error::Other("Launch cancelled. The check looks for known and obvious malware and cannot guarantee safety. Use --allow-flagged-mods to launch anyway.".into()));
            }
        }
    }
    if !args.wait {
        let pid = launch::spawn_detached(&plan)?;
        ctx.out.emit(
            json!({"event": "launched", "version": entry.id, "account": account.username, "pid": pid, "log": plan.log_file}),
            || format!("Launched Minecraft {} as {} (pid {pid}). Log: {}", entry.id, account.username, plan.log_file.display()),
        );
        return Ok(0);
    }
    let code = wait_for_game(ctx, &plan, &entry.id, &account.username);
    // What the game changed goes to the profile's other instances.
    if let Err(e) = arctic_core::shared::sync(&ctx.dirs, settings.shared, None) {
        log::warn!("sharing between instances: {e}");
    }
    code
}

fn quick_play(args: &LaunchArgs) -> Result<Option<QuickPlay>> {
    if let Some(address) = &args.server {
        if arctic_core::servers::Address::parse(address).is_none() {
            return Err(Error::Other(format!(
                "\"{address}\" isn't a server address"
            )));
        }
        return Ok(Some(QuickPlay::Server(address.trim().to_owned())));
    }
    if let Some(file) = &args.replay {
        let file = std::path::absolute(file).map_err(|e| Error::Other(e.to_string()))?;
        if !file.is_file() {
            return Err(Error::Other(format!("no replay at {}", file.display())));
        }
        return Ok(Some(QuickPlay::Replay(file)));
    }
    Ok(args.world.clone().map(QuickPlay::World))
}

/// The CLI's accounts for the in-game switcher: read from disk each time,
/// refreshed and saved like `arctic launch` does.
struct CliAccounts {
    dirs: arctic_core::storage::DataDirs,
    session_updates: Mutex<()>,
}

impl arctic_core::bridge::Accounts for CliAccounts {
    fn list(&self) -> Vec<arctic_core::bridge::AccountEntry> {
        let store = AccountStore::load(&self.dirs).unwrap_or_default();
        store
            .accounts
            .iter()
            .map(|a| arctic_core::bridge::AccountEntry {
                id: a.id.clone(),
                name: a.username.clone(),
                uuid: a.uuid.clone(),
                microsoft: a.is_microsoft(),
                active: store.active.as_deref() == Some(a.id.as_str()),
            })
            .collect()
    }

    fn session(&self, id: &str, force: bool) -> Result<arctic_core::bridge::SessionGrant> {
        let updating = self
            .session_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut store = AccountStore::load(&self.dirs)?;
        let account = store
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| Error::Other("no such account".into()))?;
        let (fresh, changed) = auth::refresh_session(&self.dirs, &account, force)?;
        if changed && store.replace_refreshed(&account, fresh.clone()) {
            store.save(&self.dirs)?;
        }
        drop(updating);
        Ok(arctic_core::bridge::SessionGrant {
            identity: fresh.identity(),
            minecraft_refresh_at: fresh.minecraft_refresh_at(),
            arctic_token: if force {
                None
            } else {
                let base = arctic_core::cosmetics::base_url();
                arctic_core::cosmetics::token_for(&self.dirs, &base, &fresh).ok()
            },
        })
    }
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
            GameEvent::Exited { code, .. } => {
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
        game_maximized: args.maximized || settings.game_maximized,
        java_override: args.java.clone().or(settings.java_override.clone()),
        ..settings
    }
    .sanitized())
}

/// `--offline NAME`, `--account QUERY`, or the active account. Microsoft
/// sessions are refreshed (and saved) when needed.
fn pick_account(ctx: &Ctx, args: &LaunchArgs) -> Result<Account> {
    #[cfg(feature = "offline-accounts")]
    if let Some(name) = &args.offline {
        let account = offline::create(name)?;
        if args.save {
            let mut store = AccountStore::load(&ctx.dirs)?;
            store.upsert(account.clone());
            store.save(&ctx.dirs)?;
        }
        return Ok(account);
    }
    fresh_account(ctx, args.account.as_deref())
}
