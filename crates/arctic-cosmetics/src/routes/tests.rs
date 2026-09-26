use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use axum::routing::get;
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
    let store = Store::memory().unwrap();
    let catalog = Catalog::load(&assets).unwrap();
    catalog.register(&store, 0).unwrap();
    router(Arc::new(AppState {
        store,
        catalog,
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

async fn microsoft_token(app: &Router) -> String {
    let (_, ch) = call(app, "POST", "/v1/auth/challenge", None, None).await;
    let server_id = ch["server_id"].as_str().unwrap().to_owned();
    let (s, v) = call(
        app,
        "POST",
        "/v1/auth/verify",
        None,
        Some(json!({"name": "Notch", "server_id": server_id})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_owned()
}

fn b64(png: &[u8]) -> String {
    STANDARD.encode(png)
}

#[tokio::test(flavor = "multi_thread")]
async fn custom_skin_and_preset_cape_are_relayed() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let skin = images::test_png(64, 64);
    let (s, look) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"skin": {"png": b64(&skin), "model": "slim"}, "cape": {"preset": "aurora"}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{look}");
    assert_eq!(look["model"], "slim");

    let dashed = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
    let (_, found) = call(
        &app,
        "GET",
        &format!("/v1/players?uuids={dashed}"),
        None,
        None,
    )
    .await;
    let skin_hash = found[UUID]["skin"].as_str().unwrap().to_owned();
    let cape_hash = found[UUID]["cape"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/textures/{skin_hash}.png"),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/textures/{cape_hash}.png"),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Re-publishing by hash keeps the skin without uploading it again.
    let (s, again) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"skin": {"hash": skin_hash, "model": "slim"}, "cape": {"hash": cape_hash}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{again}");
    assert_eq!(again["skin"], skin_hash);
    let bogus = "0".repeat(40);
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"cape": {"hash": bogus}})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Clearing everything removes the player from lookups.
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"skin": null, "cape": null})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, found) = call(
        &app,
        "GET",
        &format!("/v1/players?uuids={UUID}"),
        None,
        None,
    )
    .await;
    assert_eq!(found.as_object().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn custom_capes_are_checked_but_not_restricted() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let cape = images::test_png(128, 64);
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"cape": {"png": b64(&cape)}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let wrong = images::test_png(64, 64);
    let (s, v) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"cape": {"png": b64(&wrong)}})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"cape": {"preset": "nope"}})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_names_are_claimed_by_key() {
    let app = app().await;
    let key = "ab".repeat(32);
    let (s, v) = call(
        &app,
        "POST",
        "/v1/auth/offline",
        None,
        Some(json!({"name": "Steve", "key": key})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["uuid"], images::offline_uuid("Steve"));
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/offline",
        None,
        Some(json!({"name": "Steve", "key": key})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let other = "cd".repeat(32);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/offline",
        None,
        Some(json!({"name": "Steve", "key": other})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/offline",
        None,
        Some(json!({"name": "Steve", "key": "short"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unverified_and_unsigned() {
    let app = app().await;
    let (_, ch) = call(&app, "POST", "/v1/auth/challenge", None, None).await;
    let server_id = ch["server_id"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/verify",
        None,
        Some(json!({"name": "Herobrine", "server_id": server_id})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
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
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some("forged.1.00"),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&app, "GET", "/v1/textures/../catalog.json", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn catalog_lists_presets_with_textures() {
    let app = app().await;
    let (s, presets) = call(&app, "GET", "/v1/catalog", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let first = &presets.as_array().unwrap()[0];
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/textures/{}.png", first["texture"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[test]
fn forwarded_ip_uses_the_proxy_hop() {
    let mut h = HeaderMap::new();
    h.insert("x-forwarded-for", "6.6.6.6, 10.0.0.7".parse().unwrap());
    assert_eq!(forwarded_ip(&h), Some("10.0.0.7".parse().unwrap()));
    assert_eq!(forwarded_ip(&HeaderMap::new()), None);
}
