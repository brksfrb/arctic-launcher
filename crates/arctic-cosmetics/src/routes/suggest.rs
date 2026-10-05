//! `POST /v1/suggest`: ideas and bug reports typed into the launcher.

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{ClientAddr, Shared, db_error, error, now};
use crate::suggestions::{Suggestion, clean_text};

pub fn routes() -> Router<Shared> {
    Router::new().route("/v1/suggest", routing::post(submit))
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    kind: String,
    text: String,
    #[serde(default)]
    contact: String,
    #[serde(default)]
    launcher: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    log: String,
}

async fn submit(
    State(state): State<Shared>,
    Extension(addr): Extension<ClientAddr>,
    Json(body): Json<Body>,
) -> Response {
    if let Some(ip) = addr.0
        && !state.suggest_limiter.allow(ip)
    {
        return error(StatusCode::TOO_MANY_REQUESTS, "enough for now, thanks");
    }
    if clean_text(&body.text).len() < 3 {
        return error(StatusCode::BAD_REQUEST, "write something first");
    }
    let suggestion = Suggestion {
        kind: &body.kind,
        text: &body.text,
        contact: &body.contact,
        launcher: &body.launcher,
        os: &body.os,
        log: &body.log,
    };
    match state.store.suggestion_add(&suggestion, now()) {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(e) => db_error("suggestion", e),
    }
}
