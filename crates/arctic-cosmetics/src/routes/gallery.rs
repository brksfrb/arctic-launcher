//! `/v1/gallery`: browse, share, use, report and remove community skins.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, db_error, error, known_texture, now, player, store_upload};
use crate::gallery::{self, PublishError, Sort};
use crate::images::Kind;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/gallery", routing::get(list).post(publish))
        .route("/v1/gallery/{id}", routing::delete(remove))
        .route("/v1/gallery/{id}/use", routing::post(use_item))
        .route("/v1/gallery/{id}/report", routing::post(report))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    sort: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    limit: Option<usize>,
}

async fn list(State(state): State<Shared>, Query(q): Query<ListQuery>) -> Response {
    let sort = if q.sort == "new" {
        Sort::New
    } else {
        Sort::Popular
    };
    let limit = q.limit.unwrap_or(24).clamp(1, gallery::MAX_PAGE);
    match state.store.gallery_page(sort, &q.q, q.offset, limit) {
        Ok(page) => Json(page).into_response(),
        Err(e) => db_error("gallery list", e),
    }
}

#[derive(Deserialize)]
struct PublishBody {
    png: Option<String>,
    hash: Option<String>,
    #[serde(default)]
    model: String,
    name: String,
}

async fn publish(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<PublishBody>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let Some(name) = gallery::clean_name(&body.name) else {
        return error(StatusCode::BAD_REQUEST, "give the skin a name");
    };
    let texture = match (&body.png, &body.hash) {
        (Some(png), _) => store_upload(&state, png, Kind::Skin, now()),
        (None, Some(hash)) => known_texture(&state, hash),
        (None, None) => return error(StatusCode::BAD_REQUEST, "skin needs png or hash"),
    };
    let texture = match texture {
        Ok(t) => t,
        Err(r) => return *r,
    };
    let author = match state.store.author_name(&uuid) {
        Ok(Some(n)) => n,
        Ok(None) => return error(StatusCode::UNAUTHORIZED, "sign in first"),
        Err(e) => return db_error("author", e),
    };
    let model = if body.model == "slim" {
        "slim"
    } else {
        "classic"
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    match state
        .store
        .gallery_publish(&id, &texture, model, &name, &uuid, &author, now())
    {
        Ok(()) => Json(
            json!({ "id": id, "name": name, "texture": texture, "model": model, "author": author }),
        )
        .into_response(),
        Err(PublishError::Duplicate) => error(StatusCode::CONFLICT, "this skin is already in the gallery"),
        Err(PublishError::TooMany) => error(
            StatusCode::FORBIDDEN,
            "you've shared the maximum number of skins",
        ),
        Err(PublishError::Db(e)) => {
            log::error!("gallery publish: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

/// Count a use and return the texture to download.
async fn use_item(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    match state.store.gallery_take(&id) {
        Ok(Some(texture)) => Json(json!({ "texture": texture })).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("gallery use", e),
    }
}

async fn report(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    match state.store.gallery_report(&id, &uuid) {
        Ok(true) => Json(json!({ "reported": true })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("gallery report", e),
    }
}

/// Authors remove their own items; `X-Admin-Key` removes anything.
async fn remove(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let admin = state.admin_key.as_deref().is_some_and(|key| {
        headers
            .get("x-admin-key")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|given| given == key)
    });
    let author = if admin {
        None
    } else {
        player(&state, &headers)
    };
    if !admin && author.is_none() {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    }
    match state.store.gallery_delete(&id, author.as_deref()) {
        Ok(true) => Json(json!({ "deleted": true })).into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("gallery delete", e),
    }
}
