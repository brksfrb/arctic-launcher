//! `arctic proxy` and `arctic crash`.

use std::path::PathBuf;

use arctic_core::proxy::ProxySettings;
use arctic_core::{Error, Result, crash};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::ProxyCommand;

pub fn proxy(ctx: &Ctx, cmd: &ProxyCommand) -> Result<i32> {
    let mut proxy = ProxySettings::load(&ctx.dirs);
    match cmd {
        ProxyCommand::Show => {}
        ProxyCommand::Set {
            host,
            port,
            username,
            password,
        } => {
            proxy.enabled = true;
            proxy.host = host.trim().to_owned();
            proxy.port = *port;
            proxy.username = username.clone().unwrap_or_default();
            proxy.password = password.clone().unwrap_or_default();
            proxy.validate().map_err(Error::Other)?;
            proxy.set = arctic_core::auth::now_secs();
            proxy.save(&ctx.dirs)?;
        }
        ProxyCommand::Off => {
            proxy.enabled = false;
            proxy.set = arctic_core::auth::now_secs();
            proxy.save(&ctx.dirs)?;
        }
        ProxyCommand::Test => {
            arctic_core::net::test_proxy(&proxy)?;
            ctx.out
                .emit(json!({"event": "proxy_test", "ok": true}), || {
                    "The proxy works.".into()
                });
            return Ok(0);
        }
    }
    let on = proxy.active().is_some();
    ctx.out.emit(
        json!({
            "enabled": proxy.enabled,
            "active": on,
            "host": proxy.bare_host(),
            "port": proxy.port,
            "username": proxy.username,
        }),
        || match (on, proxy.enabled) {
            (true, _) => format!(
                "Proxy on: socks5 {}:{}{}. Some servers block proxies.",
                proxy.bare_host(),
                proxy.port,
                if proxy.username.is_empty() {
                    ""
                } else {
                    " (with login)"
                }
            ),
            (false, true) => {
                "Proxy on but incomplete: launches are blocked until it's fixed.".into()
            }
            (false, false) => "Proxy off.".into(),
        },
    );
    Ok(0)
}

/// Explain a crash from a log or crash report (default: an instance's last game log).
pub fn crash(ctx: &Ctx, file: Option<&PathBuf>, instance: Option<&str>) -> Result<i32> {
    let path = match (file, instance) {
        (Some(f), _) => f.clone(),
        (None, query) => {
            let instance = instance_or_default(ctx, query)?;
            ctx.dirs.logs().join(format!("game-{}.log", instance.id))
        }
    };
    let text = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
    let text = String::from_utf8_lossy(&text);
    match crash::diagnose(&text) {
        Some(d) => {
            ctx.out.emit(
                json!({"cause": d.title, "detail": d.detail, "mods": d.mods}),
                || format!("{}\n{}", d.title, d.detail),
            );
            Ok(0)
        }
        None => {
            ctx.out.emit(json!({"cause": null}), || {
                format!("No known cause found in {}.", path.display())
            });
            Ok(1)
        }
    }
}
