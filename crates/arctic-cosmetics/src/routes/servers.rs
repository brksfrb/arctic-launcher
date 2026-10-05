//! `/v1/servers`: the public server list, owner submissions and the admin
//! review (`X-Admin-Key`).

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, db_error, error, moderator, now, player};
use crate::listing::{self, Listing, Live, State as ListingState, SubmitError};

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/servers", routing::get(public).post(submit))
        .route("/v1/servers/{id}/verify", routing::post(verify))
        .route("/v1/admin/servers", routing::get(review))
        .route("/v1/admin/servers/{id}", routing::post(decide))
}

/// Curated and approved servers seen lately, in a new random order.
async fn public(State(state): State<Shared>) -> Response {
    match state.store.listing_public(now()) {
        Ok(mut list) => {
            shuffle(&mut list);
            Json(json!({ "servers": list })).into_response()
        }
        Err(e) => db_error("servers", e),
    }
}

fn shuffle<T>(list: &mut [T]) {
    for i in (1..list.len()).rev() {
        let r = u64::from_le_bytes(
            uuid::Uuid::new_v4().as_bytes()[..8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        list.swap(i, (r % (i as u64 + 1)) as usize);
    }
}

#[derive(Deserialize)]
struct Submission {
    address: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Signed-in owners submit; the answer holds the code for their MOTD.
async fn submit(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(s): Json<Submission>,
) -> Response {
    let Some(owner) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let Some(address) = listing::clean_address(&s.address) else {
        return error(StatusCode::BAD_REQUEST, "that isn't a server address");
    };
    let name = s.name.trim();
    if name.is_empty() || name.chars().count() > listing::MAX_NAME {
        return error(StatusCode::BAD_REQUEST, "the name needs 1 to 40 characters");
    }
    let description = s.description.trim();
    if description.chars().count() > listing::MAX_DESCRIPTION {
        return error(
            StatusCode::BAD_REQUEST,
            "the description is too long (200 characters at most)",
        );
    }
    let tags = listing::clean_tags(&s.tags);
    match state
        .store
        .listing_submit(&owner, &address, name, description, &tags, now())
    {
        Ok(l) => Json(
            json!({ "id": l.id, "address": l.address, "code": l.code, "state": l.state.as_str() }),
        )
        .into_response(),
        Err(SubmitError::Taken) => error(
            StatusCode::CONFLICT,
            "that server is already on the list or waiting",
        ),
        Err(SubmitError::TooMany) => error(
            StatusCode::TOO_MANY_REQUESTS,
            "that's enough servers for today",
        ),
        Err(SubmitError::Db(e)) => db_error("server submit", e),
    }
}

/// The owner asks us to look for the code; then it waits for an admin.
async fn verify(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(owner) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let listing = match state.store.listing_get(&id) {
        Ok(Some(l)) if l.owner.as_deref() == Some(owner.as_str()) => l,
        Ok(_) => return error(StatusCode::NOT_FOUND, "no such submission"),
        Err(e) => return db_error("server verify", e),
    };
    if listing.state != ListingState::Unverified {
        return Json(json!({ "state": listing.state.as_str() })).into_response();
    }
    let address = listing.address.clone();
    let checked = tokio::task::spawn_blocking(move || check(&address)).await;
    let (status, cracked) = match checked {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => {
            return error(
                StatusCode::BAD_GATEWAY,
                &format!("couldn't reach the server: {e}"),
            );
        }
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "the check failed"),
    };
    if !listing::motd_has_code(&status.motd, &listing.code) {
        return error(
            StatusCode::PRECONDITION_FAILED,
            &format!("\"{}\" isn't in the server's MOTD yet", listing.code),
        );
    }
    let live = Live {
        players: status.online,
        max_players: status.max,
        version: status.version,
    };
    let saved = state
        .store
        .listing_set_state(&id, ListingState::Pending)
        .and_then(|_| state.store.listing_pinged(&id, Some(&live), now()))
        .and_then(|()| match cracked {
            Some(c) => state.store.listing_set_cracked(&id, c),
            None => Ok(()),
        });
    match saved {
        Ok(()) => Json(json!({ "state": "pending" })).into_response(),
        Err(e) => db_error("server verify", e),
    }
}

/// A status ping and, when the server lets it, the premium/cracked probe.
pub(crate) fn check(address: &str) -> arctic_ping::Result<(arctic_ping::Status, Option<bool>)> {
    let status = arctic_ping::ping(address, None)?;
    let cracked = arctic_ping::Address::parse(address)
        .and_then(|a| arctic_ping::login_check(&a, status.protocol).ok())
        .and_then(|login| match login {
            arctic_ping::Login::Premium => Some(false),
            arctic_ping::Login::Cracked => Some(true),
            arctic_ping::Login::Refused(_) => None,
        });
    Ok((status, cracked))
}

#[derive(Deserialize)]
struct ReviewQuery {
    state: Option<String>,
}

async fn review(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<ReviewQuery>,
) -> Response {
    if moderator(&state, &headers).is_none() {
        return error(StatusCode::FORBIDDEN, "admins only");
    }
    let wanted = match q.state.as_deref().unwrap_or("pending") {
        "unverified" => ListingState::Unverified,
        "listed" => ListingState::Listed,
        "curated" => ListingState::Curated,
        "rejected" => ListingState::Rejected,
        _ => ListingState::Pending,
    };
    match state.store.listing_in_state(wanted) {
        Ok(list) => Json(json!({ "servers": list.iter().map(admin_view).collect::<Vec<_>>() }))
            .into_response(),
        Err(e) => db_error("server review", e),
    }
}

fn admin_view(l: &Listing) -> serde_json::Value {
    let mut v = serde_json::to_value(l).unwrap_or_default();
    v["state"] = json!(l.state.as_str());
    v["owner"] = json!(l.owner);
    v["code"] = json!(l.code);
    v["partner"] = json!(l.partner);
    v
}

#[derive(Deserialize)]
struct Decision {
    action: String,
    /// With `partner`: the partner's name (empty to make it an ordinary server again).
    #[serde(default)]
    group: String,
}

async fn decide(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(d): Json<Decision>,
) -> Response {
    let Some(name) = moderator(&state, &headers) else {
        return error(StatusCode::FORBIDDEN, "admins only");
    };
    let done = match d.action.as_str() {
        "approve" => state.store.listing_set_state(&id, ListingState::Listed),
        "reject" => state.store.listing_set_state(&id, ListingState::Rejected),
        "remove" => state.store.listing_remove(&id),
        "partner" => state.store.listing_set_partner(&id, &d.group),
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "action is approve, reject, remove or partner",
            );
        }
    };
    match done {
        Ok(true) => {
            let _ = state
                .store
                .moderation_note(&name, &d.action, &id, "server", now());
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "no such server"),
        Err(e) => db_error("server decide", e),
    }
}

/// Keep player counts fresh and learn premium/cracked once per server.
pub async fn pinger(state: Shared, every: std::time::Duration) {
    let mut tick = tokio::time::interval(every);
    loop {
        tick.tick().await;
        let list = match state.store.listing_to_ping() {
            Ok(l) => l,
            Err(e) => {
                log::error!("server pinger: {e}");
                continue;
            }
        };
        let addresses: Vec<String> = list.iter().map(|l| l.address.clone()).collect();
        let Ok(results) =
            tokio::task::spawn_blocking(move || arctic_ping::ping_all(&addresses, None)).await
        else {
            continue;
        };
        let now = now();
        for (l, result) in list.iter().zip(results) {
            let live = result.as_ref().ok().map(|s| Live {
                players: s.online,
                max_players: s.max,
                version: s.version.clone(),
            });
            if let Err(e) = state.store.listing_pinged(&l.id, live.as_ref(), now) {
                log::error!("server pinger: {e}");
            }
            if l.cracked.is_none()
                && let Ok(s) = result
            {
                let (id, address, st) = (l.id.clone(), l.address.clone(), state.clone());
                let protocol = s.protocol;
                tokio::task::spawn_blocking(move || {
                    let found = arctic_ping::Address::parse(&address)
                        .and_then(|a| arctic_ping::login_check(&a, protocol).ok());
                    let cracked = match found {
                        Some(arctic_ping::Login::Premium) => Some(false),
                        Some(arctic_ping::Login::Cracked) => Some(true),
                        _ => None,
                    };
                    if let Some(c) = cracked {
                        let _ = st.store.listing_set_cracked(&id, c);
                    }
                });
            }
        }
    }
}
