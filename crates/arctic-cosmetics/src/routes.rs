//! HTTP API (JSON). Everything under `/v1`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::{self, Challenges};
use crate::catalog::Catalog;
use crate::limit::Limiter;
use crate::store::Store;

/// Players per lookup request.
const MAX_LOOKUP: usize = 100;

pub struct AppState {
    pub store: Store,
    pub catalog: Catalog,
    pub challenges: Challenges,
    pub limiter: Limiter,
    pub secret: Vec<u8>,
    pub session_url: String,
    /// Behind a reverse proxy: take the client address from
    /// `X-Forwarded-For` instead of the connection.
    pub trust_proxy: bool,
}

type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/catalog", get(catalog))
        .route("/v1/textures/{file}", get(texture))
        .route("/v1/players", get(players))
        .route("/v1/auth/challenge", post(challenge))
        .route("/v1/auth/verify", post(verify))
        .route("/v1/me", get(me))
        .route("/v1/me/equipped", put(equip))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .with_state(state)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

async fn rate_limit(State(state): State<Shared>, request: Request, next: Next) -> Response {
    let forwarded = state
        .trust_proxy
        .then(|| forwarded_ip(request.headers()))
        .flatten();
    let ip = forwarded.or_else(|| {
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip())
    });
    if let Some(ip) = ip
        && !state.limiter.allow(ip)
    {
        return error(StatusCode::TOO_MANY_REQUESTS, "slow down");
    }
    next.run(request).await
}

/// The original client address set by a trusted proxy (the last hop it
/// appended, so clients can't spoof it by sending their own header).
fn forwarded_ip(headers: &HeaderMap) -> Option<std::net::IpAddr> {
    headers
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .rsplit(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

async fn catalog(State(state): State<Shared>) -> Response {
    Json(state.catalog.public()).into_response()
}

async fn texture(State(state): State<Shared>, Path(file): Path<String>) -> Response {
    let Some(id) = file.strip_suffix(".png") else {
        return error(StatusCode::NOT_FOUND, "not found");
    };
    match state.catalog.texture(id) {
        Some(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            png,
        )
            .into_response(),
        None => error(StatusCode::NOT_FOUND, "not found"),
    }
}

#[derive(Deserialize)]
struct PlayersQuery {
    uuids: String,
}

/// `?uuids=a,b,c` → `{ "a": { "cape": "aurora" } }` for players with cosmetics.
async fn players(State(state): State<Shared>, Query(q): Query<PlayersQuery>) -> Response {
    let uuids: Vec<String> = q
        .uuids
        .split(',')
        .map(|u| u.trim().replace('-', "").to_ascii_lowercase())
        .filter(|u| auth::is_uuid(u))
        .take(MAX_LOOKUP)
        .collect();
    match state.store.equipped_many(&uuids) {
        Ok(found) => Json(found.into_iter().collect::<HashMap<_, _>>()).into_response(),
        Err(e) => {
            log::error!("lookup: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn challenge(State(state): State<Shared>) -> Response {
    Json(json!({ "server_id": state.challenges.issue() })).into_response()
}

#[derive(Deserialize)]
struct VerifyBody {
    name: String,
    server_id: String,
}

async fn verify(State(state): State<Shared>, Json(body): Json<VerifyBody>) -> Response {
    if !state.challenges.take(&body.server_id) {
        return error(StatusCode::BAD_REQUEST, "unknown or expired challenge");
    }
    let url = state.session_url.clone();
    let lookup =
        tokio::task::spawn_blocking(move || auth::has_joined(&url, &body.name, &body.server_id))
            .await;
    let (uuid, name) = match lookup {
        Ok(Ok(Some(found))) => found,
        Ok(Ok(None)) => return error(StatusCode::UNAUTHORIZED, "session not verified"),
        Ok(Err(e)) => {
            log::warn!("hasJoined: {e}");
            return error(StatusCode::BAD_GATEWAY, "could not reach Mojang");
        }
        Err(e) => {
            log::error!("hasJoined task: {e}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };
    if let Err(e) = state.store.touch(&uuid, &name, now()) {
        log::error!("touch: {e}");
        return error(StatusCode::INTERNAL_SERVER_ERROR, "database error");
    }
    let expires = now() + auth::TOKEN_TTL_SECS;
    Json(json!({
        "token": auth::sign(&state.secret, &uuid, expires),
        "uuid": uuid,
        "name": name,
        "expires": expires,
    }))
    .into_response()
}

/// The signed-in player's uuid from `Authorization: Bearer …`.
fn player(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    auth::verify(&state.secret, token, now())
}

async fn me(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    match state.store.equipped(&uuid) {
        Ok(equipped) => Json(json!({
            "uuid": uuid,
            "owned": state.catalog.owned_by(&uuid),
            "equipped": equipped,
        }))
        .into_response(),
        Err(e) => {
            log::error!("me: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

#[derive(Deserialize)]
struct EquipBody {
    cape: Option<String>,
}

async fn equip(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<EquipBody>,
) -> Response {
    let Some(uuid) = player(&state, &headers) else {
        return error(StatusCode::UNAUTHORIZED, "sign in first");
    };
    if let Some(cape) = &body.cape
        && !state.catalog.owned_by(&uuid).iter().any(|id| id == cape)
    {
        return error(StatusCode::FORBIDDEN, "you don't own that cosmetic");
    }
    match state.store.set_cape(&uuid, body.cape.as_deref(), now()) {
        Ok(()) => Json(json!({ "equipped": { "cape": body.cape } })).into_response(),
        Err(e) => {
            log::error!("equip: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    const UUID: &str = "069a79f444e94726a5befca90e38aaf5";

    /// Stand-in for Mojang's `hasJoined`: accepts any server id for "Notch".
    async fn fake_session() -> String {
        async fn has_joined(Query(q): Query<HashMap<String, String>>) -> Response {
            if q.get("username").map(String::as_str) == Some("Notch") {
                Json(json!({ "id": UUID, "name": "Notch" })).into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/hasJoined", get(has_joined)))
                .await
                .unwrap();
        });
        format!("http://{addr}/hasJoined")
    }

    async fn app() -> Router {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        router(Arc::new(AppState {
            store: Store::memory().unwrap(),
            catalog: Catalog::load(&assets).unwrap(),
            challenges: Challenges::default(),
            limiter: Limiter::new(1000, Duration::from_secs(60)),
            secret: b"test-secret-test-secret-test-secret".to_vec(),
            session_url: fake_session().await,
            trust_proxy: false,
        }))
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sign_in_equip_and_lookup() {
        let app = app().await;
        let (s, catalog) = call(&app, "GET", "/v1/catalog", None, None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(catalog.as_array().unwrap().len(), 4);

        let (_, ch) = call(&app, "POST", "/v1/auth/challenge", None, None).await;
        let server_id = ch["server_id"].as_str().unwrap().to_owned();
        let (s, v) = call(
            &app,
            "POST",
            "/v1/auth/verify",
            None,
            Some(json!({"name": "Notch", "server_id": server_id})),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let token = v["token"].as_str().unwrap().to_owned();

        // Challenges are single use.
        let (s, _) = call(
            &app,
            "POST",
            "/v1/auth/verify",
            None,
            Some(json!({"name": "Notch", "server_id": server_id})),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        let (s, me) = call(&app, "GET", "/v1/me", Some(&token), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(
            me["owned"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o == "aurora")
        );

        let (s, _) = call(
            &app,
            "PUT",
            "/v1/me/equipped",
            Some(&token),
            Some(json!({"cape": "nope"})),
        )
        .await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, _) = call(
            &app,
            "PUT",
            "/v1/me/equipped",
            Some(&token),
            Some(json!({"cape": "aurora"})),
        )
        .await;
        assert_eq!(s, StatusCode::OK);

        let dashed = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        let (_, found) = call(
            &app,
            "GET",
            &format!("/v1/players?uuids={dashed},00000000000000000000000000000000"),
            None,
            None,
        )
        .await;
        assert_eq!(found[UUID]["cape"], "aurora");
        assert_eq!(found.as_object().unwrap().len(), 1);
    }

    #[test]
    fn forwarded_ip_uses_the_proxy_hop() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "6.6.6.6, 10.0.0.7".parse().unwrap());
        assert_eq!(forwarded_ip(&h), Some("10.0.0.7".parse().unwrap()));
        assert_eq!(forwarded_ip(&HeaderMap::new()), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_unverified_and_unsigned() {
        let app = app().await;
        let (_, ch) = call(&app, "POST", "/v1/auth/challenge", None, None).await;
        let server_id = ch["server_id"].as_str().unwrap();
        let (s, _) = call(
            &app,
            "POST",
            "/v1/auth/verify",
            None,
            Some(json!({"name": "Herobrine", "server_id": server_id})),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _) = call(&app, "GET", "/v1/me", Some("forged.1.00"), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _) = call(&app, "GET", "/v1/textures/aurora.png", None, None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = call(&app, "GET", "/v1/textures/../catalog.json", None, None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }
}
