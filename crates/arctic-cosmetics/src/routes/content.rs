//! 3D cosmetics and emotes: the catalog, its files, and emotes being played.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{MAX_LOOKUP, Shared, db_error, error, player};
use crate::auth;
use crate::presence;

/// A looping emote stops on its own after this long (clients stop it
/// sooner, when the player moves).
const MAX_LOOP_MS: u64 = 5 * 60 * 1000;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/cosmetics", routing::get(catalog))
        .route("/v1/assets/{hash}", routing::get(asset))
        .route("/v1/emote", routing::post(play_emote))
        .route("/v1/emotes", routing::get(emotes))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

async fn catalog(State(state): State<Shared>) -> Response {
    Json(json!({
        "cosmetics": state.content.cosmetics,
        "emotes": state.content.emotes,
    }))
    .into_response()
}

/// A geometry or animation file by content hash.
async fn asset(State(state): State<Shared>, Path(hash): Path<String>) -> Response {
    let hash = hash.strip_suffix(".json").unwrap_or(&hash);
    match state.content.file(hash) {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes.to_vec(),
        )
            .into_response(),
        None => error(StatusCode::NOT_FOUND, "not found"),
    }
}

#[derive(Deserialize)]
struct EmoteBody {
    /// Emote id, or null to stop.
    id: Option<String>,
}

async fn play_emote(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EmoteBody>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    if let Some(id) = &body.id
        && state.content.emote(id).is_none()
    {
        return error(StatusCode::BAD_REQUEST, "unknown emote");
    }
    match state.store.set_emote(&uuid, body.id.as_deref(), now_ms()) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_error("emote", e),
    }
}

#[derive(Deserialize)]
struct EmotesQuery {
    uuids: String,
}

/// Emotes playing right now among these (in-world) player UUIDs.
async fn emotes(State(state): State<Shared>, Query(q): Query<EmotesQuery>) -> Response {
    let uuids: Vec<String> = q
        .uuids
        .split(',')
        .map(|u| u.trim().replace('-', "").to_ascii_lowercase())
        .filter(|u| auth::is_uuid(u))
        .take(MAX_LOOKUP)
        .collect();
    let length = |id: &str| {
        state.content.emote(id).map(|e| {
            if e.looping {
                MAX_LOOP_MS
            } else {
                (e.length * 1000.0) as u64
            }
        })
    };
    match state.store.emotes(&uuids, now_ms(), length) {
        Ok(found) => Json(
            found
                .into_iter()
                .collect::<HashMap<String, presence::Playing>>(),
        )
        .into_response(),
        Err(e) => db_error("emotes", e),
    }
}
