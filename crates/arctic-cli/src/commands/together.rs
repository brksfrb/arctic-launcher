//! `arctic together`: play together without a server, from the terminal.
//! Hosting shares this PC's "Open to LAN" world; joining makes a friend's
//! world show up in your game's LAN list. Runs until the session ends or
//! you press Ctrl+C.

use std::sync::mpsc;

use arctic_core::{Error, Result};
use arctic_share::{Share, ShareEvent};
use serde_json::json;

use super::Ctx;
use crate::cli::TogetherCommand;

pub fn run(ctx: &Ctx, cmd: &TogetherCommand) -> Result<i32> {
    let (tx, rx) = mpsc::channel();
    let share = Share::new(move |_, event| {
        let _ = tx.send(event);
    })
    .map_err(|e| Error::Other(format!("networking: {e}")))?;
    match cmd {
        TogetherCommand::Host { port } => {
            // Same key as the launcher, so the invite code stays the same.
            let key_path = ctx.dirs.profile_root().join("share.key");
            let key =
                arctic_share::load_or_create_key(&key_path).map_err(|e| Error::io(&key_path, e))?;
            share.host(key, *port);
        }
        TogetherCommand::Join { code } => {
            share.join(code).map_err(Error::Other)?;
        }
    }
    for event in rx {
        if let Some(code) = report(ctx, &event) {
            return Ok(code);
        }
    }
    Ok(0)
}

/// Print one event; returns the exit code once the session is over.
fn report(ctx: &Ctx, event: &ShareEvent) -> Option<i32> {
    match event {
        ShareEvent::HostReady { code } => ctx.out.emit(
            json!({"event": "host_ready", "code": code}),
            || format!("Hosting. Give your friends this code:\n\n    {code}\n\nOpen a world to LAN in Minecraft to share it. Ctrl+C stops."),
        ),
        ShareEvent::HostWorld(Some(world)) => ctx.out.emit(
            json!({"event": "host_world", "motd": world.motd, "port": world.port}),
            || format!("Sharing \"{}\" (LAN port {}).", world.motd, world.port),
        ),
        ShareEvent::HostWorld(None) => ctx.out.emit(json!({"event": "host_world", "motd": null}), || {
            "Waiting for a world opened to LAN...".into()
        }),
        ShareEvent::Guests(n) => ctx.out.emit(json!({"event": "guests", "count": n}), || {
            format!("{n} friend{} connected.", if *n == 1 { "" } else { "s" })
        }),
        ShareEvent::Joined { motd, port } => ctx.out.emit(
            json!({"event": "joined", "motd": motd, "port": port}),
            || format!("Joined \"{motd}\". It's in Multiplayer's LAN list (or connect to localhost:{port}). Ctrl+C leaves."),
        ),
        ShareEvent::Stopped { error } => {
            ctx.out.emit(json!({"event": "stopped", "error": error}), || match error {
                Some(e) => format!("Stopped: {e}"),
                None => "Stopped.".into(),
            });
            return Some(if error.is_some() { 1 } else { 0 });
        }
    }
    None
}
