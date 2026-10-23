//! The phone notification web app (`/notify`) and the calls it makes. The app is the same for everyone; what a
//! phone may do is decided by the moderator key it signs in with (`X-Admin-Key`, a moderator's).

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, db_error, error, moderator, now};
use crate::push::{self, Subscription, TOPICS};

const PAGE: &str = include_str!("../notify.html");
const WORKER: &str = include_str!("../notify_sw.js");
const MANIFEST: &str = include_str!("../notify.webmanifest");
const ICON_180: &[u8] = include_bytes!("../../assets/notify/icon-180.png");
const ICON_192: &[u8] = include_bytes!("../../assets/notify/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../../assets/notify/icon-512.png");

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/notify", routing::get(page))
        .route("/notify/", routing::get(page))
        .route("/notify/sw.js", routing::get(worker))
        .route("/notify/manifest.webmanifest", routing::get(manifest))
        .route(
            "/notify/icon-180.png",
            routing::get(|| async { png(ICON_180) }),
        )
        .route(
            "/notify/icon-192.png",
            routing::get(|| async { png(ICON_192) }),
        )
        .route(
            "/notify/icon-512.png",
            routing::get(|| async { png(ICON_512) }),
        )
        .route("/v1/notify/config", routing::get(config))
        .route("/v1/notify/state", routing::post(state))
        .route("/v1/notify/subscribe", routing::post(subscribe))
        .route("/v1/notify/topics", routing::post(topics))
        .route("/v1/notify/unsubscribe", routing::post(unsubscribe))
        .route("/v1/notify/test", routing::post(test))
}

async fn page() -> Response {
    let mut response = Html(PAGE).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn worker() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        WORKER,
    )
        .into_response()
}

async fn manifest() -> Response {
    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        MANIFEST,
    )
        .into_response()
}

fn png(bytes: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}

/// The server's public push key and what a phone can listen for.
async fn config(State(state): State<Shared>) -> Response {
    match state.store.push_keys() {
        Ok((_, public)) => Json(json!({
            "key": public,
            "topics": TOPICS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(e) => db_error("push key", e),
    }
}

#[derive(Deserialize)]
struct Keys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize)]
struct Sub {
    endpoint: String,
    keys: Keys,
}

#[derive(Deserialize)]
struct SubscribeBody {
    subscription: Sub,
    #[serde(default)]
    topics: Vec<String>,
}

#[derive(Deserialize)]
struct EndpointBody {
    endpoint: String,
    #[serde(default)]
    topics: Vec<String>,
}

fn who(state: &Shared, headers: &HeaderMap) -> Result<String, Response> {
    moderator(state, headers).ok_or_else(|| error(StatusCode::UNAUTHORIZED, "wrong key"))
}

/// What this phone listens for (`null` when it isn't signed up), and whether the key is good.
async fn state(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EndpointBody>,
) -> Response {
    let owner = match who(&state, &headers) {
        Ok(owner) => owner,
        Err(denied) => return denied,
    };
    match state.store.push_topics(&owner, &body.endpoint) {
        Ok(topics) => Json(json!({ "name": owner, "topics": topics })).into_response(),
        Err(e) => db_error("push state", e),
    }
}

async fn subscribe(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<SubscribeBody>,
) -> Response {
    let owner = match who(&state, &headers) {
        Ok(owner) => owner,
        Err(denied) => return denied,
    };
    let sub = Subscription {
        endpoint: body.subscription.endpoint,
        p256dh: body.subscription.keys.p256dh,
        auth: body.subscription.keys.auth,
        topics: push::clean_topics(&body.topics),
    };
    if !push::endpoint_ok(&sub.endpoint) {
        return error(StatusCode::BAD_REQUEST, "that isn't a push address");
    }
    match state.store.push_subscribe(&owner, &sub, now()) {
        Ok(true) => Json(json!({ "topics": sub.topics })).into_response(),
        Ok(false) => error(StatusCode::CONFLICT, "too many phones on this key"),
        Err(e) => db_error("push subscribe", e),
    }
}

async fn topics(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EndpointBody>,
) -> Response {
    let owner = match who(&state, &headers) {
        Ok(owner) => owner,
        Err(denied) => return denied,
    };
    let chosen = push::clean_topics(&body.topics);
    match state.store.push_set_topics(&owner, &body.endpoint, &chosen) {
        Ok(true) => Json(json!({ "topics": chosen })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "this phone isn't signed up"),
        Err(e) => db_error("push topics", e),
    }
}

async fn unsubscribe(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EndpointBody>,
) -> Response {
    let owner = match who(&state, &headers) {
        Ok(owner) => owner,
        Err(denied) => return denied,
    };
    match state.store.push_unsubscribe(&owner, &body.endpoint) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => db_error("push unsubscribe", e),
    }
}

/// A test note to this phone.
async fn test(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EndpointBody>,
) -> Response {
    let owner = match who(&state, &headers) {
        Ok(owner) => owner,
        Err(denied) => return denied,
    };
    let phone = match state.store.push_get(&owner, &body.endpoint) {
        Ok(Some(phone)) => phone,
        Ok(None) => return error(StatusCode::NOT_FOUND, "this phone isn't signed up"),
        Err(e) => return db_error("push test", e),
    };
    let state = state.clone();
    let sent = tokio::task::spawn_blocking(move || {
        push::send(
            &state.store,
            &phone,
            "test",
            "Arctic",
            "Notifications work.",
            "/notify",
        )
    })
    .await
    .unwrap_or(false);
    Json(json!({ "sent": sent })).into_response()
}
