//! `/v1/profile`, `/v1/friends`, `/v1/invites` and `/v1/presence`: Arctic
//! profiles (accounts linked into one person), friends, invites and the
//! launcher's "I'm online" check-in.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, error, now, player};
use crate::auth;
use crate::friends::{self, FriendError};

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/profile", routing::get(profile).put(update))
        .route("/v1/profile/link", routing::post(link))
        .route("/v1/profile/unlink", routing::post(unlink))
        .route("/v1/profile/recovery", routing::post(new_recovery))
        .route("/v1/profile/recover", routing::post(recover))
        .route("/v1/friends", routing::get(list).post(request))
        .route("/v1/friends/{id}/accept", routing::post(accept))
        .route("/v1/friends/{id}", routing::delete(remove))
        .route("/v1/invites", routing::post(invite))
        .route("/v1/invites/{id}", routing::delete(dismiss))
        .route("/v1/presence", routing::post(presence))
        .route("/v1/messages", routing::get(history).post(send))
        .route("/v1/messages/new", routing::get(new_messages))
        .route("/v1/messages/read", routing::post(read))
        .route(
            "/v1/attachments",
            routing::post(upload).layer(axum::extract::DefaultBodyLimit::max(
                crate::chat::MAX_IMAGE_BYTES,
            )),
        )
        .route("/v1/attachments/{id}", routing::get(attachment))
        .route("/v1/voice/join", routing::post(voice_join))
        .route("/v1/voice/leave", routing::post(voice_leave))
}

fn fail(e: FriendError) -> Response {
    let (status, message) = match e {
        FriendError::NotFound => (
            StatusCode::NOT_FOUND,
            "no one found: use their friend code (names only work for Microsoft accounts)",
        ),
        FriendError::Yourself => (StatusCode::BAD_REQUEST, "that's you"),
        FriendError::Already => (StatusCode::CONFLICT, "you're already friends"),
        FriendError::TooMany => (StatusCode::TOO_MANY_REQUESTS, "that's the limit for now"),
        FriendError::NotFriends => (StatusCode::FORBIDDEN, "you can only invite friends"),
        FriendError::NoInvites => (StatusCode::FORBIDDEN, "they don't take invites right now"),
        FriendError::LastAccount => (
            StatusCode::BAD_REQUEST,
            "that's the only account on this profile",
        ),
        FriendError::Db(e) => {
            log::error!("friends: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    };
    error(status, message)
}

/// The signed-in account and its profile (made on first use).
fn me(state: &Shared, headers: &HeaderMap) -> Result<(String, String), Box<Response>> {
    let uuid = player(state, headers)
        .ok_or_else(|| Box::new(error(StatusCode::UNAUTHORIZED, "sign in first")))?;
    let id = state
        .store
        .profile_id(&uuid, now())
        .map_err(|e| Box::new(fail(e)))?;
    Ok((uuid, id))
}

async fn profile(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.profile(&id) {
        Ok(p) => Json(p).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct Update {
    name: Option<String>,
    share_online: Option<bool>,
    share_server: Option<bool>,
    allow_invites: Option<bool>,
    show_accounts: Option<bool>,
}

async fn update(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(u): Json<Update>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let name = u.name.as_deref().map(str::trim);
    if let Some(n) = name
        && (n.is_empty()
            || n.chars().count() > friends::MAX_NAME
            || n.chars().any(char::is_control))
    {
        return error(StatusCode::BAD_REQUEST, "the name needs 1 to 24 characters");
    }
    let settings: Vec<(&str, bool)> = [
        ("share_online", u.share_online),
        ("share_server", u.share_server),
        ("allow_invites", u.allow_invites),
        ("show_accounts", u.show_accounts),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|v| (k, v)))
    .collect();
    match state
        .store
        .profile_update(&id, name, &settings)
        .and_then(|()| state.store.profile(&id))
    {
        Ok(p) => Json(p).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct LinkBody {
    /// The other account's Arctic sign-in token: proof you hold it.
    token: String,
}

async fn link(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<LinkBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Some(other) = auth::verify(&state.secret, &b.token, now()) else {
        return error(
            StatusCode::UNAUTHORIZED,
            "that account's sign-in didn't check out",
        );
    };
    match state
        .store
        .profile_link(&id, &other, now())
        .and_then(|()| state.store.profile(&id))
    {
        Ok(p) => Json(p).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct UnlinkBody {
    uuid: String,
}

async fn unlink(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<UnlinkBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let uuid = b.uuid.replace('-', "").to_ascii_lowercase();
    match state
        .store
        .profile_unlink(&id, &uuid)
        .and_then(|()| state.store.profile(&id))
    {
        Ok(p) => Json(p).into_response(),
        Err(e) => fail(e),
    }
}

/// Friends (as each allows), requests both ways and invites waiting.
async fn list(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let now = now();
    match state
        .store
        .friends(&id, now)
        .and_then(|f| Ok((f, state.store.invites(&id, now)?)))
    {
        Ok(((friends, incoming, outgoing), invites)) => Json(json!({
            "friends": friends, "incoming": incoming, "outgoing": outgoing, "invites": invites,
        }))
        .into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct RequestBody {
    /// Their friend code, or the name of one of their Microsoft accounts.
    #[serde(alias = "code")]
    name: String,
}

async fn request(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<RequestBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let name = b.name.trim();
    let usable = !name.is_empty()
        && name.len() <= 16
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !usable {
        return error(
            StatusCode::BAD_REQUEST,
            "that isn't a friend code or a Minecraft name",
        );
    }
    match state.store.friend_request_to(&id, name, now()) {
        Ok((friends, name)) => Json(json!({ "friends": friends, "name": name })).into_response(),
        Err(e) => fail(e),
    }
}

async fn accept(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(from): Path<String>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.friend_accept(&id, &from, now()) {
        Ok(()) => Json(json!({ "friends": true })).into_response(),
        Err(e) => fail(e),
    }
}

async fn remove(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(other): Path<String>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.friend_remove(&id, &other) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct InviteBody {
    to: String,
    /// `server` or `together`.
    kind: String,
    target: String,
}

async fn invite(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<InviteBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let target = b.target.trim();
    let valid = match b.kind.as_str() {
        "server" => crate::listing::clean_address(target).is_some(),
        "together" => {
            !target.is_empty()
                && target.len() <= friends::MAX_TARGET
                && target
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        }
        _ => false,
    };
    if !valid {
        return error(
            StatusCode::BAD_REQUEST,
            "invite to a server address or a play-together code",
        );
    }
    match state.store.invite(&id, &b.to, &b.kind, target, now()) {
        Ok(invite) => Json(json!({ "id": invite })).into_response(),
        Err(e) => fail(e),
    }
}

async fn dismiss(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(invite): Path<String>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.invite_dismiss(&id, &invite) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(e),
    }
}

/// A new recovery code (shown once; it replaces any earlier one).
async fn new_recovery(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.profile_new_recovery(&id) {
        Ok(code) => Json(json!({ "code": code })).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct RecoverBody {
    code: String,
    /// The offline account's name.
    name: String,
    /// This PC's new key for it.
    key: String,
}

/// An offline account on a new PC: the recovery code moves its name to the
/// new key and signs it in.
async fn recover(State(state): State<Shared>, Json(b): Json<RecoverBody>) -> Response {
    let name_ok = !b.name.is_empty()
        && b.name.len() <= 16
        && b.name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let key_ok = (32..=128).contains(&b.key.len()) && b.key.bytes().all(|c| c.is_ascii_hexdigit());
    if !name_ok || !key_ok {
        return error(StatusCode::BAD_REQUEST, "invalid name or key");
    }
    let uuid = crate::images::offline_uuid(&b.name);
    match state.store.profile_recovers(&b.code, &uuid) {
        Ok(true) => {}
        Ok(false) => {
            return error(
                StatusCode::FORBIDDEN,
                "that recovery code isn't for this name",
            );
        }
        Err(e) => return fail(e),
    }
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(b.key.as_bytes()));
    if let Err(e) = state.store.replace_key_hash(&uuid, &hash) {
        return super::db_error("recover", e);
    }
    super::session(&state, &uuid, &b.name)
}

/// The launcher is open (it calls this every minute or so).
async fn presence(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    match state.store.launcher_online(&uuid, now()) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::db_error("presence", e),
    }
}

#[derive(Deserialize)]
struct SendBody {
    to: String,
    #[serde(default)]
    text: String,
    /// An uploaded screenshot (`POST /v1/attachments`).
    #[serde(default)]
    image: Option<String>,
}

async fn send(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<SendBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Some(text) = crate::chat::clean_text_for(&b.text, b.image.is_some()) else {
        return error(StatusCode::BAD_REQUEST, "messages need 1 to 500 characters");
    };
    match state
        .store
        .chat_send(&id, &b.to, &text, b.image.as_deref(), now())
    {
        Ok(m) => Json(m).into_response(),
        Err(FriendError::NotFriends) => {
            error(StatusCode::FORBIDDEN, "you can only message friends")
        }
        Err(FriendError::TooMany) => error(StatusCode::TOO_MANY_REQUESTS, "slow down a little"),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct HistoryQuery {
    with: String,
    before: Option<i64>,
}

/// A page of the conversation with a friend, newest last.
async fn history(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HistoryQuery>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.chat_history(&id, &q.with, q.before) {
        Ok(list) => Json(json!({ "messages": list })).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct NewQuery {
    #[serde(default)]
    after: i64,
}

/// Messages to me newer than `after` (clients poll this).
async fn new_messages(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<NewQuery>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.chat_new(&id, q.after) {
        Ok(list) => Json(json!({ "messages": list })).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct ReadBody {
    with: String,
}

async fn read(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<ReadBody>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match state.store.chat_read(&id, &b.with) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(e),
    }
}

/// Upload a screenshot (the PNG itself as the body); send it with a message.
async fn upload(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    if crate::chat::png_size(&body).is_none() {
        return error(
            StatusCode::BAD_REQUEST,
            "send a PNG up to 2 MB and 4096 pixels a side",
        );
    }
    match state.store.attachment_put(&id, &body, now()) {
        Ok(key) => Json(json!({ "id": key })).into_response(),
        Err(FriendError::TooMany) => error(
            StatusCode::TOO_MANY_REQUESTS,
            "that's a lot of screenshots; try later",
        ),
        Err(e) => fail(e),
    }
}

/// A screenshot from one of your chats.
async fn attachment(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Response {
    let (_, id) = match me(&state, &headers) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return error(StatusCode::NOT_FOUND, "no such screenshot");
    }
    match state.store.attachment_get(&id, &key) {
        Ok(Some(png)) => (
            [
                (axum::http::header::CONTENT_TYPE, "image/png"),
                (axum::http::header::CACHE_CONTROL, "private, max-age=86400"),
            ],
            png,
        )
            .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such screenshot"),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
struct VoiceJoin {
    /// Hash of the Minecraft server address.
    room: String,
    /// This launcher's peer address.
    node: String,
    /// The UUID played as there (offline servers derive it from the name).
    #[serde(rename = "as")]
    playing_as: String,
}

/// Check in to a voice room; the answer lists the others in it.
async fn voice_join(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<VoiceJoin>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    let playing_as = b.playing_as.replace('-', "").to_ascii_lowercase();
    if !crate::voice::valid_room(&b.room)
        || !crate::voice::valid_node(&b.node)
        || !auth::is_uuid(&playing_as)
    {
        return error(StatusCode::BAD_REQUEST, "bad room, node or uuid");
    }
    match state.store.voice_join(
        &uuid,
        &b.room.to_ascii_lowercase(),
        &b.node,
        &playing_as,
        now(),
    ) {
        Ok(members) => Json(json!({ "members": members })).into_response(),
        Err(e) => super::db_error("voice join", e),
    }
}

async fn voice_leave(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    match state.store.voice_leave(&uuid) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::db_error("voice leave", e),
    }
}
