//! Moderation: the dashboard page and what it calls. Every call needs a
//! moderator's key in `X-Admin-Key`; with no keys configured it's all off.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, db_error, error, moderator, now};
use crate::gallery::Action;

const PAGE: &str = include_str!("../admin.html");
/// Most history entries one request returns.
const LOG_LIMIT: usize = 200;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/admin", routing::get(page))
        .route("/v1/admin/me", routing::get(me))
        .route("/v1/admin/gallery", routing::get(queue))
        .route("/v1/admin/gallery/{id}", routing::post(decide))
        .route("/v1/admin/log", routing::get(log))
}

/// The dashboard: one self-contained page that talks only to this server.
async fn page(State(state): State<Shared>) -> Response {
    if state.admins.is_empty() {
        return error(StatusCode::NOT_FOUND, "not found");
    }
    (
        [
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(
                    "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                     img-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; \
                     frame-ancestors 'none'",
                ),
            ),
            (header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        Html(PAGE),
    )
        .into_response()
}

fn denied() -> Response {
    error(StatusCode::FORBIDDEN, "moderators only")
}

async fn me(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(name) = moderator(&state, &headers) else {
        return denied();
    };
    match state.store.gallery_pending_count() {
        Ok(pending) => Json(json!({ "name": name, "pending": pending })).into_response(),
        Err(e) => db_error("pending count", e),
    }
}

#[derive(Deserialize)]
struct QueueQuery {
    #[serde(default)]
    queue: String,
}

async fn queue(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<QueueQuery>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.gallery_admin_list(&q.queue) {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(e) => db_error("moderation queue", e),
    }
}

#[derive(Deserialize)]
struct Decision {
    action: String,
}

async fn decide(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(d): Json<Decision>,
) -> Response {
    let Some(name) = moderator(&state, &headers) else {
        return denied();
    };
    let Some(action) = Action::parse(&d.action) else {
        return error(
            StatusCode::BAD_REQUEST,
            "action is approve, reject, restore or remove",
        );
    };
    match state.store.gallery_moderate(&id, action, &name, now()) {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("moderation decision", e),
    }
}

async fn log(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.moderation_log(LOG_LIMIT) {
        Ok(entries) => Json(json!({ "entries": entries })).into_response(),
        Err(e) => db_error("moderation log", e),
    }
}
