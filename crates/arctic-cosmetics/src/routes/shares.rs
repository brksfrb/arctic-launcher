//! `/v1/shares`: create a short code for a bundle, and read one back.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde_json::{Value, json};

use super::{Shared, db_error, error, now, player};
use crate::shares::{self, ShareError};

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/shares", routing::post(create))
        .route("/v1/shares/{code}", routing::get(read))
}

/// Signed-in players only, so a flood of codes can be traced and capped.
async fn create(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(bundle): Json<Value>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let kind = bundle
        .get("kind")
        .and_then(Value::as_str)
        .filter(|k| shares::KINDS.contains(k));
    let (Some(kind), Some(1)) = (kind, bundle.get("arctic_share").and_then(Value::as_u64)) else {
        return error(StatusCode::BAD_REQUEST, "not an Arctic share bundle");
    };
    // Keys come out sorted, so the same bundle always hashes the same.
    let body = bundle.to_string();
    if body.len() > shares::MAX_BYTES {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "that is too big to share");
    }
    match state.store.share_create(kind, &body, &uuid, now()) {
        Ok(code) => Json(json!({ "code": code })).into_response(),
        Err(ShareError::TooMany) => error(
            StatusCode::TOO_MANY_REQUESTS,
            "you've made a lot of codes today; try again tomorrow",
        ),
        Err(ShareError::Db(e)) => {
            log::error!("share create: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn read(State(state): State<Shared>, Path(code): Path<String>) -> Response {
    let Some(code) = shares::clean_code(&code) else {
        return error(StatusCode::NOT_FOUND, "no such code");
    };
    match state.store.share_get(&code) {
        Ok(Some(body)) => ([(header::CONTENT_TYPE, "application/json")], body).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such code"),
        Err(e) => db_error("share read", e),
    }
}
