//! HTTP API (JSON). Everything under `/v1`.
//!
//! Players choose their own look (skin, model, cape) and the server simply
//! relays it to everyone else; nothing is owned or unlocked. The only check
//! is identity, so nobody can change someone else's look.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::auth::{self, Challenges};
use crate::catalog::Catalog;
use crate::images::{self, Kind};
use crate::limit::Limiter;
use crate::store::{Look, Store};

/// Players per lookup request.
const MAX_LOOKUP: usize = 100;
/// Request body limit: two base64 textures and some JSON.
const MAX_BODY: usize = 2 * images::MAX_PNG_BYTES * 4 / 3 + 4096;

pub struct AppState {
    pub store: Store,
    pub catalog: Catalog,
    pub challenges: Challenges,
    pub limiter: Limiter,
    pub secret: Vec<u8>,
    pub session_url: String,
    /// Behind a reverse proxy: take the client address from
    /// `X-Forwarded-For` instead of the connection.
    pub trust_proxy: bool,
}

type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/catalog", get(catalog))
        .route("/v1/textures/{file}", get(texture))
        .route("/v1/players", get(players))
        .route("/v1/auth/challenge", post(challenge))
        .route("/v1/auth/verify", post(verify))
        .route("/v1/auth/offline", post(offline))
        .route("/v1/look", get(my_look).put(set_look))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .with_state(state)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn db_error(what: &str, e: rusqlite::Error) -> Response {
    log::error!("{what}: {e}");
    error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
}

async fn rate_limit(State(state): State<Shared>, request: Request, next: Next) -> Response {
    let forwarded = state
        .trust_proxy
        .then(|| forwarded_ip(request.headers()))
        .flatten();
    let ip = forwarded.or_else(|| {
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip())
    });
    if let Some(ip) = ip
        && !state.limiter.allow(ip)
    {
        return error(StatusCode::TOO_MANY_REQUESTS, "slow down");
    }
    next.run(request).await
}

/// The original client address set by a trusted proxy (the last hop it
/// appended, so clients can't spoof it by sending their own header).
fn forwarded_ip(headers: &HeaderMap) -> Option<std::net::IpAddr> {
    headers
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .rsplit(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

async fn catalog(State(state): State<Shared>) -> Response {
    Json(state.catalog.presets()).into_response()
}

async fn texture(State(state): State<Shared>, Path(file): Path<String>) -> Response {
    let Some(hash) = file
        .strip_suffix(".png")
        .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return error(StatusCode::NOT_FOUND, "not found");
    };
    match state.store.texture(hash) {
        Ok(Some(png)) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                // Content-addressed: it never changes.
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            png,
        )
            .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("texture", e),
    }
}

#[derive(Deserialize)]
struct PlayersQuery {
    uuids: String,
}

/// `?uuids=a,b,c` → `{ "a": { "skin", "model", "cape" } }` for players with a look.
async fn players(State(state): State<Shared>, Query(q): Query<PlayersQuery>) -> Response {
    let uuids: Vec<String> = q
        .uuids
        .split(',')
        .map(|u| u.trim().replace('-', "").to_ascii_lowercase())
        .filter(|u| auth::is_uuid(u))
        .take(MAX_LOOKUP)
        .collect();
    match state.store.looks(&uuids) {
        Ok(found) => Json(found.into_iter().collect::<HashMap<_, _>>()).into_response(),
        Err(e) => db_error("lookup", e),
    }
}

async fn challenge(State(state): State<Shared>) -> Response {
    Json(json!({ "server_id": state.challenges.issue() })).into_response()
}

fn session(state: &AppState, uuid: &str, name: &str) -> Response {
    if let Err(e) = state.store.touch(uuid, name, now()) {
        return db_error("touch", e);
    }
    let expires = now() + auth::TOKEN_TTL_SECS;
    Json(json!({
        "token": auth::sign(&state.secret, uuid, expires),
        "uuid": uuid,
        "name": name,
        "expires": expires,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct VerifyBody {
    name: String,
    server_id: String,
}

/// Microsoft accounts: confirm the Mojang session join, like a server does.
async fn verify(State(state): State<Shared>, Json(body): Json<VerifyBody>) -> Response {
    if !state.challenges.take(&body.server_id) {
        return error(StatusCode::BAD_REQUEST, "unknown or expired challenge");
    }
    let url = state.session_url.clone();
    let lookup =
        tokio::task::spawn_blocking(move || auth::has_joined(&url, &body.name, &body.server_id))
            .await;
    match lookup {
        Ok(Ok(Some((uuid, name)))) => session(&state, &uuid, &name),
        Ok(Ok(None)) => error(StatusCode::UNAUTHORIZED, "session not verified"),
        Ok(Err(e)) => {
            log::warn!("hasJoined: {e}");
            error(StatusCode::BAD_GATEWAY, "could not reach Mojang")
        }
        Err(e) => {
            log::error!("hasJoined task: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    }
}

#[derive(Deserialize)]
struct OfflineBody {
    name: String,
    /// Secret the launcher generated for this name (hex, 32–128 chars).
    key: String,
}

const NAME_TAKEN: &str = "this name is used by another Arctic player";

/// Offline accounts: the first launcher to use a name claims it with a key;
/// later sign-ins need the same key.
async fn offline(State(state): State<Shared>, Json(body): Json<OfflineBody>) -> Response {
    let name_ok = !body.name.is_empty()
        && body.name.len() <= 16
        && body
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let key_ok =
        (32..=128).contains(&body.key.len()) && body.key.bytes().all(|b| b.is_ascii_hexdigit());
    if !name_ok || !key_ok {
        return error(StatusCode::BAD_REQUEST, "invalid name or key");
    }
    let uuid = images::offline_uuid(&body.name);
    let hash = hex::encode(Sha256::digest(body.key.as_bytes()));
    if let Err(e) = state.store.touch(&uuid, &body.name, now()) {
        return db_error("touch", e);
    }
    // Claims only apply to unclaimed names, so re-reading settles races.
    if let Err(e) = state.store.set_key_hash(&uuid, &hash) {
        return db_error("claim", e);
    }
    match state.store.key_hash(&uuid) {
        Ok(Some(existing)) if existing == hash => session(&state, &uuid, &body.name),
        Ok(_) => error(StatusCode::FORBIDDEN, NAME_TAKEN),
        Err(e) => db_error("key", e),
    }
}

/// The signed-in player's uuid from `Authorization: Bearer …`.
fn player(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    auth::verify(&state.secret, token, now())
}

async fn my_look(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    match state.store.look(&uuid) {
        Ok(look) => Json(look).into_response(),
        Err(e) => db_error("look", e),
    }
}

/// A skin: a new PNG (base64) or a texture the server already has.
#[derive(Deserialize)]
struct SkinPart {
    png: Option<String>,
    hash: Option<String>,
    #[serde(default)]
    model: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CapePart {
    Preset { preset: String },
    Custom { png: String },
    Known { hash: String },
}

#[derive(Deserialize)]
struct LookBody {
    skin: Option<SkinPart>,
    cape: Option<CapePart>,
}

/// Replace the player's whole look. `null` parts are cleared.
async fn set_look(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<LookBody>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let t = now();
    let mut look = Look {
        model: "classic".into(),
        ..Look::default()
    };
    if let Some(skin) = body.skin {
        let stored = match (&skin.png, &skin.hash) {
            (Some(png), _) => store_upload(&state, png, Kind::Skin, t),
            (None, Some(hash)) => known_texture(&state, hash),
            (None, None) => Err(Box::new(error(
                StatusCode::BAD_REQUEST,
                "skin needs png or hash",
            ))),
        };
        match stored {
            Ok(hash) => look.skin = Some(hash),
            Err(r) => return *r,
        }
        look.model = if skin.model == "slim" {
            "slim"
        } else {
            "classic"
        }
        .into();
    }
    look.cape = match body.cape {
        None => None,
        Some(CapePart::Preset { preset }) => match state.catalog.preset(&preset) {
            Some(hash) => Some(hash.to_owned()),
            None => return error(StatusCode::BAD_REQUEST, "unknown preset"),
        },
        Some(CapePart::Custom { png }) => match store_upload(&state, &png, Kind::Cape, t) {
            Ok(hash) => Some(hash),
            Err(r) => return *r,
        },
        Some(CapePart::Known { hash }) => match known_texture(&state, &hash) {
            Ok(hash) => Some(hash),
            Err(r) => return *r,
        },
    };
    match state.store.set_look(&uuid, &look, t) {
        Ok(()) => Json(look).into_response(),
        Err(e) => db_error("set look", e),
    }
}

/// A texture that was uploaded before (so looks can be re-published by hash).
fn known_texture(state: &AppState, hash: &str) -> Result<String, Box<Response>> {
    let hash = hash.to_ascii_lowercase();
    match state.store.texture(&hash) {
        Ok(Some(_)) => Ok(hash),
        Ok(None) => Err(Box::new(error(StatusCode::BAD_REQUEST, "unknown texture"))),
        Err(e) => Err(Box::new(db_error("texture", e))),
    }
}

fn store_upload(state: &AppState, b64: &str, kind: Kind, t: u64) -> Result<String, Box<Response>> {
    let png = STANDARD
        .decode(b64.trim())
        .map_err(|_| Box::new(error(StatusCode::BAD_REQUEST, "image is not valid base64")))?;
    let hash =
        images::check(&png, kind).map_err(|e| Box::new(error(StatusCode::BAD_REQUEST, &e)))?;
    state
        .store
        .put_texture(&hash, &png, t)
        .map_err(|e| Box::new(db_error("texture", e)))?;
    Ok(hash)
}

#[cfg(test)]
mod tests;
