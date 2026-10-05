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
/// Most crash groups one request returns.
const CRASH_LIMIT: usize = 100;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/admin", routing::get(page))
        .route("/v1/admin/me", routing::get(me))
        .route("/v1/admin/gallery", routing::get(queue))
        .route("/v1/admin/gallery/{id}", routing::post(decide))
        .route("/v1/admin/log", routing::get(log))
        .route("/v1/admin/crashes", routing::get(crash_groups))
        .route(
            "/v1/admin/crashes/{id}",
            routing::get(crash_detail).delete(crash_delete),
        )
        .route("/v1/admin/suggestions", routing::get(suggestion_list))
        .route(
            "/v1/admin/suggestions/{id}",
            routing::get(suggestion_detail)
                .post(suggestion_done)
                .delete(suggestion_delete),
        )
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

async fn crash_groups(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.crash_groups(CRASH_LIMIT) {
        Ok(groups) => Json(json!({ "groups": groups })).into_response(),
        Err(e) => db_error("crash groups", e),
    }
}

async fn crash_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.crash_detail(id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("crash detail", e),
    }
}

async fn crash_delete(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    let Some(name) = moderator(&state, &headers) else {
        return denied();
    };
    match state.store.crash_delete_group(id) {
        Ok(true) => {
            let _ =
                state
                    .store
                    .moderation_note(&name, "remove", &id.to_string(), "crash group", now());
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("crash delete", e),
    }
}

async fn suggestion_list(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.suggestions(CRASH_LIMIT) {
        Ok(entries) => Json(json!({ "entries": entries })).into_response(),
        Err(e) => db_error("suggestions", e),
    }
}

async fn suggestion_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.suggestion_detail(id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("suggestion detail", e),
    }
}

#[derive(serde::Deserialize)]
struct DoneBody {
    done: bool,
}

async fn suggestion_done(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<DoneBody>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.suggestion_set_done(id, body.done) {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("suggestion done", e),
    }
}

async fn suggestion_delete(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return denied();
    }
    match state.store.suggestion_delete(id) {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("suggestion delete", e),
    }
}
