//! `POST /v1/crash`: crash reports players chose to send.

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{ClientAddr, Shared, db_error, error, now};
use crate::crashes::{self, Report};

pub fn routes() -> Router<Shared> {
    Router::new().route("/v1/crash", routing::post(submit))
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    launcher: String,
    #[serde(default)]
    game: String,
    #[serde(default)]
    loader: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    title: String,
    log: String,
}

async fn submit(
    State(state): State<Shared>,
    Extension(addr): Extension<ClientAddr>,
    Json(body): Json<Body>,
) -> Response {
    if let Some(ip) = addr.0
        && !state.crash_limiter.allow(ip)
    {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "enough reports for now, thanks",
        );
    }
    if body.log.trim().is_empty() || body.game.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "nothing to report");
    }
    let (launcher, game, loader, os, title) = (
        crashes::clean_field(&body.launcher),
        crashes::clean_field(&body.game),
        crashes::clean_field(&body.loader),
        crashes::clean_field(&body.os),
        crashes::clean_field(&body.title),
    );
    let report = Report {
        launcher: &launcher,
        game: &game,
        loader: &loader,
        os: &os,
        title: &title,
        log: &body.log,
    };
    match state.store.crash_add(&report, now()) {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(e) => db_error("crash report", e),
    }
}
