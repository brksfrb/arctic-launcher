//! Arctic cosmetics server.
//!
//! Configuration (environment):
//! - `ARCTIC_COSMETICS_SECRET` (required, 32+ chars): signs session tokens.
//! - `ARCTIC_COSMETICS_ADDR` (default `0.0.0.0:8080`)
//! - `ARCTIC_COSMETICS_DB` (default `cosmetics.db`)
//! - `ARCTIC_COSMETICS_ASSETS` (default `assets`): `catalog.json` and `capes/`.
//! - `ARCTIC_COSMETICS_ADMIN_KEY` (16+ chars, optional): remove any gallery item
//!   with the `X-Admin-Key` header.
//! - `ARCTIC_COSMETICS_TRUST_PROXY=1`: rate-limit by `X-Forwarded-For` (only
//!   behind a reverse proxy that sets it).

mod auth;
mod catalog;
mod gallery;
mod images;
mod limit;
mod routes;
mod store;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use routes::AppState;

const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const MIN_SECRET_LEN: usize = 32;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run().await {
        log::error!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_owned());
    let secret = std::env::var("ARCTIC_COSMETICS_SECRET")
        .map_err(|_| "ARCTIC_COSMETICS_SECRET is not set".to_owned())?;
    if secret.len() < MIN_SECRET_LEN {
        return Err(format!(
            "ARCTIC_COSMETICS_SECRET must be at least {MIN_SECRET_LEN} characters"
        ));
    }
    let addr: SocketAddr = env("ARCTIC_COSMETICS_ADDR", "0.0.0.0:8080")
        .parse()
        .map_err(|e| format!("ARCTIC_COSMETICS_ADDR: {e}"))?;
    let db = PathBuf::from(env("ARCTIC_COSMETICS_DB", "cosmetics.db"));
    let assets = PathBuf::from(env("ARCTIC_COSMETICS_ASSETS", "assets"));
    let store = store::Store::open(&db).map_err(|e| format!("database {}: {e}", db.display()))?;
    let catalog = catalog::Catalog::load(&assets)?;
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    catalog
        .register(&store, started)
        .map_err(|e| format!("registering presets: {e}"))?;
    let state = AppState {
        store,
        catalog,
        challenges: auth::Challenges::default(),
        limiter: limit::Limiter::new(120, Duration::from_secs(60)),
        secret: secret.into_bytes(),
        session_url: env("ARCTIC_SESSION_URL", SESSION_URL),
        trust_proxy: env("ARCTIC_COSMETICS_TRUST_PROXY", "0") == "1",
        admin_key: std::env::var("ARCTIC_COSMETICS_ADMIN_KEY")
            .ok()
            .filter(|k| k.len() >= 16),
    };
    let app = routes::router(Arc::new(state));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {addr}: {e}"))?;
    log::info!("listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|e| e.to_string())
}
