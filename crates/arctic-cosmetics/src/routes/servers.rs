//! `/v1/servers`: the public server list, owner submissions and the admin
//! review (`X-Admin-Key`).

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, db_error, error, moderator, now, player};
use crate::listing::{self, Listing, ListingEdit, Live, State as ListingState, SubmitError};

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

/// Signed-in owners submit. The server goes straight to the review queue.
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
        Ok(l) => {
            crate::push::notify(
                &state,
                "server",
                "Server waiting for review".into(),
                name.to_owned(),
                "/admin#servers/pending".into(),
            );
            Json(
                json!({ "id": l.id, "address": l.address, "code": l.code, "state": l.state.as_str() }),
            )
            .into_response()
        }
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

/// Older launchers still call this after submitting. Listings already wait
/// for review; this reports the state, and moves a leftover unverified one
/// into the queue (no MOTD code).
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
    if listing.state == ListingState::Unverified {
        match state.store.listing_set_state(&id, ListingState::Pending) {
            Ok(_) => return Json(json!({ "state": "pending" })).into_response(),
            Err(e) => return db_error("server verify", e),
        }
    }
    Json(json!({ "state": listing.state.as_str() })).into_response()
}

#[derive(Deserialize)]
struct ReviewQuery {
    state: Option<String>,
}

async fn review(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(q): Query<ReviewQuery>,
) -> Response {
    // Moderators and read-only keys read the lists; only moderators decide (see `decide`).
    if super::admin::viewer(&state, &headers, &uri).is_none() {
        return error(StatusCode::FORBIDDEN, "admins only");
    }
    let wanted = match q.state.as_deref().unwrap_or("pending") {
        "unverified" => ListingState::Unverified,
        "listed" => ListingState::Listed,
        "curated" => ListingState::Curated,
        "rejected" => ListingState::Rejected,
        _ => ListingState::Pending,
    };
    let list = match state.store.listing_in_state(wanted) {
        Ok(list) => list,
        Err(e) => return db_error("server review", e),
    };
    // Asked now, not at the last round of the pinger: what the page shows is how it is.
    let problems = ping_and_store(&state, &list).await;
    let list = match state.store.listing_in_state(wanted) {
        Ok(list) => list,
        Err(e) => return db_error("server review", e),
    };
    let views = list
        .iter()
        .map(|l| {
            let problem = list_problem(&problems, &l.id);
            admin_view(l, problem)
        })
        .collect::<Vec<_>>();
    Json(json!({ "servers": views })).into_response()
}

/// Why a server didn't answer this time, if it didn't.
fn list_problem<'a>(problems: &'a [(String, Option<String>)], id: &str) -> Option<&'a str> {
    problems
        .iter()
        .find(|(i, _)| i == id)
        .and_then(|(_, p)| p.as_deref())
}

fn admin_view(l: &Listing, problem: Option<&str>) -> serde_json::Value {
    let mut v = serde_json::to_value(l).unwrap_or_default();
    v["problem"] = json!(problem);
    v["state"] = json!(l.state.as_str());
    v["owner"] = json!(l.owner);
    v["code"] = json!(l.code);
    v["partner"] = json!(l.partner);
    v["partner_tier"] = json!(l.partner_tier);
    v
}

#[derive(Deserialize)]
struct Decision {
    action: String,
    /// With `partner`: the partner's name (empty to make it an ordinary server again).
    #[serde(default)]
    group: String,
    /// With `partner`: `exclusive` (our own servers) or `hosted` (default).
    #[serde(default)]
    tier: String,
    /// With `edit`. Absent leaves the name.
    #[serde(default)]
    name: Option<String>,
    /// With `edit`. Absent leaves the description.
    #[serde(default)]
    description: Option<String>,
    /// With `edit`. Absent leaves the tags.
    #[serde(default)]
    tags: Option<Vec<String>>,
    /// With `edit`: `true`, `false`, or `clear` (NULL, so the pinger looks again).
    #[serde(default)]
    cracked: Option<String>,
    /// With `edit`: a player's id, or empty to clear. Absent leaves the owner.
    #[serde(default)]
    owner: Option<String>,
}

/// The edit fields on a decide call, or why they can't be applied.
fn listing_edit_from(d: &Decision) -> Result<ListingEdit, &'static str> {
    let mut edit = ListingEdit::default();
    let mut any = false;
    if let Some(name) = d.name.as_deref() {
        let name = name.trim();
        let n = name.chars().count();
        if !(1..=listing::MAX_NAME).contains(&n) {
            return Err("the name needs 1 to 40 characters");
        }
        edit.name = Some(name.to_owned());
        any = true;
    }
    if let Some(description) = d.description.as_deref() {
        let description = description.trim();
        if description.chars().count() > listing::MAX_DESCRIPTION {
            return Err("the description is too long (200 characters at most)");
        }
        edit.description = Some(description.to_owned());
        any = true;
    }
    if let Some(tags) = d.tags.as_deref() {
        edit.tags = Some(tags.to_vec());
        any = true;
    }
    if let Some(cracked) = d.cracked.as_deref() {
        edit.cracked = Some(match cracked.trim().to_ascii_lowercase().as_str() {
            "true" | "cracked" => Some(true),
            "false" | "premium" => Some(false),
            "clear" | "unknown" | "" => None,
            _ => return Err("cracked is true, false or clear"),
        });
        any = true;
    }
    if let Some(owner) = d.owner.as_deref() {
        let owner = owner.trim().replace('-', "").to_ascii_lowercase();
        edit.owner = Some(if owner.is_empty() {
            None
        } else if crate::auth::is_uuid(&owner) {
            Some(owner)
        } else {
            return Err("the owner should be a player's id, or empty to clear");
        });
        any = true;
    }
    if !any {
        return Err("nothing to change");
    }
    Ok(edit)
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
    let edit = if d.action == "edit" {
        match listing_edit_from(&d) {
            Ok(edit) => Some(edit),
            Err(msg) => return error(StatusCode::BAD_REQUEST, msg),
        }
    } else {
        None
    };
    let done = match d.action.as_str() {
        "approve" => state.store.listing_set_state(&id, ListingState::Listed),
        "reject" => state.store.listing_set_state(&id, ListingState::Rejected),
        "remove" => state.store.listing_remove(&id),
        "partner" => state.store.listing_set_partner(&id, &d.group, &d.tier),
        "edit" => {
            let Some(edit) = edit else {
                return error(StatusCode::BAD_REQUEST, "nothing to change");
            };
            state.store.listing_edit(&id, &edit)
        }
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "action is approve, reject, remove, partner or edit",
            );
        }
    };
    match done {
        Ok(true) => {
            let _ = state
                .store
                .moderation_note(&name, &d.action, &id, "server", now());
            if d.action == "approve"
                && let Ok(Some(l)) = state.store.listing_get(&id)
            {
                // Public as soon as it answers, not at the pinger's next round.
                let state = state.clone();
                tokio::spawn(async move {
                    ping_and_store(&state, &[l]).await;
                });
            }
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
        match state.store.listing_to_ping() {
            Ok(list) => {
                ping_and_store(&state, &list).await;
            }
            Err(e) => log::error!("server pinger: {e}"),
        }
    }
}

/// Ping `list` now and keep what each says (players, version; premium or
/// cracked the first time). Returns each server's id with why it didn't
/// answer, if it didn't.
async fn ping_and_store(state: &Shared, list: &[Listing]) -> Vec<(String, Option<String>)> {
    let addresses: Vec<String> = list.iter().map(|l| l.address.clone()).collect();
    let Ok(results) =
        tokio::task::spawn_blocking(move || arctic_ping::ping_all(&addresses, None)).await
    else {
        return Vec::new();
    };
    let now = now();
    let mut problems = Vec::with_capacity(list.len());
    let mut probes = Vec::new();
    for (l, result) in list.iter().zip(results) {
        problems.push((l.id.clone(), result.as_ref().err().map(|e| e.to_string())));
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
            probes.push((l.id.clone(), l.address.clone(), s.protocol));
        }
    }
    if !probes.is_empty() {
        let st = state.clone();
        // Wait for the probes: the review page should show premium or cracked
        // for what it just asked, and a cleared value gets filled in again.
        let _ = tokio::task::spawn_blocking(move || {
            std::thread::scope(|scope| {
                for (id, address, protocol) in probes {
                    let st = &st;
                    scope.spawn(move || {
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
            });
        })
        .await;
    }
    problems
}
