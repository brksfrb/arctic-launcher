//! Moderation: the dashboard page and what it calls. Every call needs a
//! moderator's key in `X-Admin-Key`; with no keys configured it's all off.
//! Read-only keys read everything the dashboard shows, in full, and change nothing:
//!
//! - `GET /v1/admin/gallery?queue=pending|listed|rejected|removed`: the skin gallery's queues
//! - `GET /v1/admin/log` (`?reads=1`: read-only keys' reads): History
//! - `GET /v1/admin/servers?state=pending|unverified|listed|curated|rejected` (see servers.rs)
//! - `GET /v1/admin/crashes?status=open|resolved&os=&launcher=&game=&since=&limit=`:
//!   groups of the same crash, each with its diagnosis, first error and root cause
//! - `GET /v1/admin/crashes/{id}`: one report's full log and its group's reports
//! - `GET /v1/admin/suggestions?status=open|done&kind=bug|idea&since=&limit=`
//! - `GET /v1/admin/suggestions/{id}`: one suggestion with its contact and details

use axum::extract::{Path, Query, State};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use serde::Deserialize;
use serde_json::json;

use super::{Shared, caller, db_error, error, moderator, now};
use crate::admin::Role;
use crate::crashes::{self, Filter, Resolution};
use crate::filters;
use crate::gallery::Action;
use crate::suggestions::Filter as SuggestionFilter;

const PAGE: &str = include_str!("../admin.html");
/// Most history entries one request returns.
const LOG_LIMIT: usize = 200;
/// Crash groups or suggestions one request returns unless it asks for another number.
const LIST_LIMIT: usize = 100;
/// The most one request may ask for.
const MAX_LIST: usize = 500;
/// A read-only key's reads are noted in History at most once per this many seconds.
const READ_NOTE_EVERY: u64 = 60;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/admin", routing::get(page))
        .route("/v1/admin/me", routing::get(me))
        .route("/v1/admin/gallery", routing::get(queue))
        .route("/v1/admin/gallery/{id}", routing::post(decide))
        .route("/v1/admin/log", routing::get(log))
        .route("/v1/admin/stats", routing::get(stats))
        .route("/v1/admin/crashes", routing::get(crash_groups))
        .route(
            "/v1/admin/crashes/{id}",
            routing::get(crash_detail)
                .post(crash_status)
                .delete(crash_delete),
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
    let Some((name, role)) = caller(&state, &headers) else {
        return denied();
    };
    let role = match role {
        Role::Moderator => "moderator",
        Role::Reader => "reader",
    };
    match waiting(&state) {
        Ok((pending, counts)) => Json(json!({
            "name": name,
            "role": role,
            "pending": pending,
            "counts": counts,
        }))
        .into_response(),
        Err(e) => db_error("pending count", e),
    }
}

/// Release downloads asked of GitHub at most this often.
const DOWNLOADS_EVERY: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Downloads of the launcher's and Polarium's release files. GitHub counts every download, not
/// every person, so these are not unique people. `None` when GitHub can't be reached.
fn github_downloads() -> Option<serde_json::Value> {
    static CACHE: OnceLock<Mutex<Option<(std::time::Instant, serde_json::Value)>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(held) = cache.lock()
        && let Some((at, value)) = held.as_ref()
        && at.elapsed() < DOWNLOADS_EVERY
    {
        return Some(value.clone());
    }
    let mut out = serde_json::Map::new();
    for (key, repo) in [
        ("launcher", "brksfrb/arctic-launcher"),
        ("polarium", "brksfrb/polarium"),
    ] {
        let url = format!("https://api.github.com/repos/{repo}/releases?per_page=100");
        let mut resp = ureq::get(&url)
            .header("User-Agent", "arctic-cosmetics")
            .header("Accept", "application/vnd.github+json")
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build()
            .call()
            .ok()?;
        let releases: Vec<serde_json::Value> = resp.body_mut().read_json().ok()?;
        let mut total = 0_u64;
        let mut per_release = Vec::new();
        for release in releases.iter().filter(|r| r["draft"] != true) {
            let downloads: u64 = release["assets"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["download_count"].as_u64()).sum())
                .unwrap_or(0);
            total += downloads;
            per_release.push(json!({"tag": release["tag_name"], "downloads": downloads}));
        }
        per_release.truncate(8);
        out.insert(
            key.to_owned(),
            json!({"total": total, "releases": per_release}),
        );
    }
    let value = serde_json::Value::Object(out);
    if let Ok(mut held) = cache.lock() {
        *held = Some((std::time::Instant::now(), value.clone()));
    }
    Some(value)
}

/// The overview tab: how many accounts the server knows, and the release downloads.
async fn stats(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if caller(&state, &headers).is_none() {
        return denied();
    }
    let counts = match state.store.overview(now()) {
        Ok(counts) => counts,
        Err(e) => return db_error("overview", e),
    };
    let downloads = tokio::task::spawn_blocking(github_downloads)
        .await
        .ok()
        .flatten();
    Json(json!({ "counts": counts, "downloads": downloads })).into_response()
}

/// What waits on each dashboard tab: gallery items and servers to review,
/// open crash groups and open suggestions (each counted up to `MAX_LIST`).
fn waiting(state: &Shared) -> rusqlite::Result<(i64, serde_json::Value)> {
    let gallery = state.store.gallery_pending_count()?;
    let servers = state
        .store
        .listing_in_state(crate::listing::State::Pending)?
        .len();
    let open_crashes = Filter {
        status: Some("open".to_owned()),
        ..Filter::default()
    };
    let crashes = state.store.crash_groups(&open_crashes, MAX_LIST)?.len();
    let open_suggestions = SuggestionFilter {
        done: Some(false),
        ..SuggestionFilter::default()
    };
    let suggestions = state.store.suggestions(&open_suggestions, MAX_LIST)?.len();
    Ok((
        gallery,
        json!({
            "gallery": gallery,
            "servers": servers,
            "crashes": crashes,
            "suggestions": suggestions,
        }),
    ))
}

#[derive(Deserialize)]
struct QueueQuery {
    #[serde(default)]
    queue: String,
}

async fn queue(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Query(q): Query<QueueQuery>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
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

#[derive(Deserialize)]
struct LogQuery {
    /// `1`: read-only keys' reads instead of moderators' decisions.
    #[serde(default)]
    reads: String,
}

async fn log(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Query(q): Query<LogQuery>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
        return denied();
    }
    match state.store.moderation_history(q.reads == "1", LOG_LIMIT) {
        Ok(entries) => Json(json!({ "entries": entries })).into_response(),
        Err(e) => db_error("moderation log", e),
    }
}

/// Someone allowed to read what the dashboard shows: a moderator or a
/// read-only key. A read-only key's reads go into History under its name, at
/// most one entry a minute per key (the next one counts those left out), so a
/// script reading many reports can't bury the moderators' decisions.
pub(super) fn viewer(state: &Shared, headers: &HeaderMap, uri: &Uri) -> Option<(String, Role)> {
    let (name, role) = caller(state, headers)?;
    if role == Role::Reader {
        let what = uri
            .path_and_query()
            .map_or_else(|| uri.path().to_owned(), |p| p.as_str().to_owned());
        if let Some(note) = read_note(&name, &what, now())
            && let Err(e) = state.store.read_note(&name, &note, now())
        {
            log::warn!("noting a read: {e}");
        }
    }
    Some((name, role))
}

/// What to note for this read by `name`, if anything: the first read in each
/// `READ_NOTE_EVERY` window, with how many reads the last window left out.
fn read_note(name: &str, what: &str, now: u64) -> Option<String> {
    static WINDOWS: OnceLock<Mutex<HashMap<String, (u64, u64)>>> = OnceLock::new();
    let mut windows = WINDOWS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (start, left_out) = windows.entry(name.to_owned()).or_insert((0, 0));
    if *start > 0 && now < *start + READ_NOTE_EVERY {
        *left_out += 1;
        return None;
    }
    let note = match *left_out {
        0 => what.to_owned(),
        n => format!("{what} (and {n} more reads in the minute before)"),
    };
    (*start, *left_out) = (now, 0);
    Some(note)
}

/// `?limit=` within 1..=`MAX_LIST`, `LIST_LIMIT` when not given.
fn list_limit(limit: Option<usize>) -> Result<usize, &'static str> {
    match limit {
        None => Ok(LIST_LIMIT),
        Some(n) if (1..=MAX_LIST).contains(&n) => Ok(n),
        Some(_) => Err("limit is 1 to 500"),
    }
}

fn since(text: Option<&str>) -> Result<Option<i64>, &'static str> {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(t) => filters::parse_since(t)
            .map(Some)
            .ok_or("since is a day (2026-10-10) or Unix seconds"),
    }
}

/// A filter value: empty or missing means any.
fn given(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

#[derive(Deserialize)]
struct CrashQuery {
    status: Option<String>,
    os: Option<String>,
    launcher: Option<String>,
    game: Option<String>,
    since: Option<String>,
    limit: Option<usize>,
}

impl CrashQuery {
    fn filter(self) -> Result<(Filter, usize), &'static str> {
        let status = match given(self.status).as_deref() {
            None | Some("all") => None,
            Some(s @ ("open" | "resolved")) => Some(s.to_owned()),
            Some(_) => {
                return Err("status is open, resolved or all");
            }
        };
        let filter = Filter {
            status,
            os: given(self.os),
            launcher: given(self.launcher),
            game: given(self.game),
            since: since(self.since.as_deref())?,
        };
        Ok((filter, list_limit(self.limit)?))
    }
}

async fn crash_groups(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Query(q): Query<CrashQuery>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
        return denied();
    }
    let (filter, limit) = match q.filter() {
        Ok(f) => f,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    match state.store.crash_groups(&filter, limit) {
        Ok(groups) => Json(json!({ "groups": groups })).into_response(),
        Err(e) => db_error("crash groups", e),
    }
}

async fn crash_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Path(id): Path<i64>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
        return denied();
    }
    match state.store.crash_detail(id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("crash detail", e),
    }
}

#[derive(Deserialize)]
struct CrashStatus {
    status: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    fixed_in: String,
}

async fn crash_status(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<CrashStatus>,
) -> Response {
    let Some(name) = moderator(&state, &headers) else {
        return denied();
    };
    if !body.fixed_in.trim().is_empty() && !filters::is_version(&body.fixed_in) {
        return error(
            StatusCode::BAD_REQUEST,
            "fixed_in is a launcher version like 0.3.5, or empty",
        );
    }
    let resolution = Resolution {
        by: &name,
        note: &body.note,
        fixed_in: &body.fixed_in,
    };
    let (resolved, action) = match body.status.as_str() {
        "resolved" => (Some(&resolution), "resolve"),
        "open" => (None, "reopen"),
        _ => return error(StatusCode::BAD_REQUEST, "status is open or resolved"),
    };
    match state.store.crash_set_status(id, resolved, now()) {
        Ok(true) => {
            let detail = format!("crash group {}", crashes::clean_field(&body.note));
            let _ =
                state
                    .store
                    .moderation_note(&name, action, &id.to_string(), detail.trim(), now());
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not found"),
        Err(e) => db_error("crash status", e),
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

#[derive(Deserialize)]
struct SuggestionQuery {
    status: Option<String>,
    kind: Option<String>,
    since: Option<String>,
    limit: Option<usize>,
}

impl SuggestionQuery {
    fn filter(self) -> Result<(SuggestionFilter, usize), &'static str> {
        let done = match given(self.status).as_deref() {
            None | Some("all") => None,
            Some("open") => Some(false),
            Some("done") => Some(true),
            Some(_) => {
                return Err("status is open, done or all");
            }
        };
        let kind = match given(self.kind).as_deref() {
            None | Some("all") => None,
            Some("bug") => Some("bug"),
            Some("idea") => Some("idea"),
            Some(_) => return Err("kind is bug, idea or all"),
        };
        let filter = SuggestionFilter {
            done,
            kind,
            since: since(self.since.as_deref())?,
        };
        Ok((filter, list_limit(self.limit)?))
    }
}

async fn suggestion_list(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Query(q): Query<SuggestionQuery>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
        return denied();
    }
    let (filter, limit) = match q.filter() {
        Ok(f) => f,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    match state.store.suggestions(&filter, limit) {
        Ok(entries) => Json(json!({ "entries": entries })).into_response(),
        Err(e) => db_error("suggestions", e),
    }
}

async fn suggestion_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    uri: Uri,
    Path(id): Path<i64>,
) -> Response {
    if viewer(&state, &headers, &uri).is_none() {
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
