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
    let content = crate::content::Content::load(&assets).unwrap();
    content.register(&store, 0).unwrap();
    router(Arc::new(AppState {
        store,
        catalog,
        content,
        challenges: Challenges::default(),
        limiter: Limiter::new(1000, Duration::from_secs(60)),
        read_limiter: Limiter::new(1000, Duration::from_secs(60)),
        crash_limiter: Limiter::new(3, Duration::from_secs(3600)),
        suggest_limiter: Limiter::new(3, Duration::from_secs(3600)),
        secret: b"test-secret-test-secret-test-secret".to_vec(),
        session_url: fake_session().await,
        trust_proxy: false,
        admins: crate::admin::Moderators::parse(
            Some("alice:alice-key-alice-key,bob:bob-key-bob-key-bob"),
            Some("admin-key-admin-key"),
        ),
    }))
}

async fn admin_call(
    app: &Router,
    key: &str,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-admin-key", key)
        .header("content-type", "application/json");
    let req = match body {
        Some(b) => req.body(Body::from(b.to_string())),
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
async fn crash_reports_are_kept_and_shown_to_moderators() {
    let app = app().await;
    let report = json!({
        "launcher": "0.2.1", "game": "1.21.4", "loader": "fabric", "os": "windows",
        "title": "Out of memory", "log": "java.lang.OutOfMemoryError: Java heap space
    at a.b"
    });
    for _ in 0..2 {
        let (s, _) = call(&app, "POST", "/v1/crash", None, Some(report.clone())).await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, _) = call(
        &app,
        "POST",
        "/v1/crash",
        None,
        Some(json!({"game": "1.21.4", "log": "  "})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = admin_call(
        &app,
        "nope-nope-nope-nope",
        "GET",
        "/v1/admin/crashes",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, groups) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        "/v1/admin/crashes",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(groups["groups"].as_array().unwrap().len(), 1);
    assert_eq!(groups["groups"][0]["count"], 2);
    let id = groups["groups"][0]["id"].as_i64().unwrap();
    let (_, detail) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        &format!("/v1/admin/crashes/{id}"),
        None,
    )
    .await;
    assert!(detail["log"].as_str().unwrap().contains("OutOfMemoryError"));
    let (s, _) = admin_call(
        &app,
        "alice-key-alice-key",
        "DELETE",
        &format!("/v1/admin/crashes/{id}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, groups) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        "/v1/admin/crashes",
        None,
    )
    .await;
    assert!(groups["groups"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn suggestions_reach_the_dashboard() {
    let app = app().await;
    let (s, _) = call(
        &app,
        "POST",
        "/v1/suggest",
        None,
        Some(json!({"text": " "})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/suggest",
        None,
        Some(json!({"kind": "bug", "text": "maps lag", "os": "linux", "log": "boom"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = admin_call(
        &app,
        "nope-nope-nope-nope",
        "GET",
        "/v1/admin/suggestions",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, list) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        "/v1/admin/suggestions",
        None,
    )
    .await;
    assert_eq!(list["entries"][0]["kind"], "bug");
    assert_eq!(list["entries"][0]["has_log"], true);
    let id = list["entries"][0]["id"].as_i64().unwrap();
    let (_, detail) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        &format!("/v1/admin/suggestions/{id}"),
        None,
    )
    .await;
    assert_eq!(detail["log"], "boom");
    let (s, _) = admin_call(
        &app,
        "alice-key-alice-key",
        "DELETE",
        &format!("/v1/admin/suggestions/{id}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn dashboard_is_served_only_with_moderators_configured() {
    let app = app().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let csp = resp.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(csp.contains("default-src 'none'") && csp.contains("frame-ancestors 'none'"));
    let (s, _) = admin_call(&app, "", "GET", "/v1/admin/me", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, me) = admin_call(&app, "admin-key-admin-key", "GET", "/v1/admin/me", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["name"], "admin");
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
    // A known texture of the wrong kind: a cape can't be worn as a skin.
    let (s, _) = call(
        &app,
        "PUT",
        "/v1/look",
        Some(&token),
        Some(json!({"skin": {"hash": cape_hash, "model": "classic"}})),
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
    // Not 2:1 frames (64×64 would be a two-frame animated cape).
    let wrong = images::test_png(64, 48);
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

#[tokio::test(flavor = "multi_thread")]
async fn gallery_share_browse_use_report() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let skin = images::test_png(64, 64);
    let (s, item) = call(
        &app,
        "POST",
        "/v1/gallery",
        Some(&token),
        Some(json!({"png": b64(&skin), "model": "slim", "name": "Ice Knight"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{item}");
    assert_eq!(item["author"], "Notch");
    assert_eq!(item["pending"], true);
    let id = item["id"].as_str().unwrap().to_owned();
    // Not public until a moderator lets it in.
    let (_, page) = call(&app, "GET", "/v1/gallery", None, None).await;
    assert_eq!(page["total"], 0);
    let (s, _) = call(&app, "POST", &format!("/v1/gallery/{id}/use"), None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = admin_call(
        &app,
        "bad-key-bad-key-bad",
        "POST",
        &format!("/v1/admin/gallery/{id}"),
        Some(json!({"action": "approve"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, queue) = admin_call(
        &app,
        "alice-key-alice-key",
        "GET",
        "/v1/admin/gallery?queue=pending",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(queue["items"][0]["id"], id.as_str());
    let (s, _) = admin_call(
        &app,
        "bob-key-bob-key-bob",
        "POST",
        &format!("/v1/admin/gallery/{id}"),
        Some(json!({"action": "approve"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, log) = admin_call(&app, "alice-key-alice-key", "GET", "/v1/admin/log", None).await;
    assert_eq!(log["entries"][0]["moderator"], "bob");
    let (s, _) = call(
        &app,
        "POST",
        "/v1/gallery",
        Some(&token),
        Some(json!({"png": b64(&skin), "name": "Again"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/gallery",
        None,
        Some(json!({"png": b64(&skin), "name": "x"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (_, page) = call(&app, "GET", "/v1/gallery?sort=new&q=knight", None, None).await;
    assert_eq!(page["total"], 1);
    let (s, used) = call(&app, "POST", &format!("/v1/gallery/{id}/use"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/textures/{}.png", used["texture"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, page) = call(&app, "GET", "/v1/gallery", None, None).await;
    assert_eq!(page["items"][0]["downloads"], 1);

    let (s, _) = call(
        &app,
        "POST",
        &format!("/v1/gallery/{id}/report"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Admin removal.
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/gallery/{id}"))
        .header("x-admin-key", "admin-key-admin-key")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );
    let (_, page) = call(&app, "GET", "/v1/gallery", None, None).await;
    assert_eq!(page["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn cosmetics_are_worn_and_emotes_relayed() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let (s, catalog) = call(&app, "GET", "/v1/cosmetics", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let halo = catalog["cosmetics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "halo")
        .unwrap()
        .clone();
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/assets/{}", halo["model"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "geometry is served by hash");
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/textures/{}.png", halo["texture"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "texture is served by hash");

    let put = |body| call(&app, "PUT", "/v1/look", Some(&token), Some(body));
    let (s, v) = put(json!({"cosmetics": ["halo", "frost_wings"]})).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, _) = put(json!({"cosmetics": ["nope"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // An older client changing the cape keeps the cosmetics on.
    let (_, v) = put(json!({"cape": {"preset": "aurora"}})).await;
    assert_eq!(v["cosmetics"], json!(["halo", "frost_wings"]));
    let (_, found) = call(
        &app,
        "GET",
        &format!("/v1/players?uuids={UUID}"),
        None,
        None,
    )
    .await;
    assert_eq!(found[UUID]["cosmetics"], json!(["halo", "frost_wings"]));

    // Emotes show for players on Arctic right now.
    let (s, _) = call(
        &app,
        "POST",
        "/v1/emote",
        Some(&token),
        Some(json!({"id": "wave"})),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/emote",
        Some(&token),
        Some(json!({"id": "nope"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let emotes = format!("/v1/emotes?uuids={UUID}");
    let (_, v) = call(&app, "GET", &emotes, None, None).await;
    assert!(v.as_object().unwrap().is_empty(), "not checked in yet");
    let (s, _) = call(
        &app,
        "POST",
        "/v1/online",
        Some(&token),
        Some(json!({"as": UUID})),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, v) = call(&app, "GET", &emotes, None, None).await;
    assert_eq!(v[UUID]["id"], "wave");
    let (_, _) = call(
        &app,
        "POST",
        "/v1/emote",
        Some(&token),
        Some(json!({"id": null})),
    )
    .await;
    let (_, v) = call(&app, "GET", &emotes, None, None).await;
    assert!(v.as_object().unwrap().is_empty(), "stopped");
}

#[tokio::test(flavor = "multi_thread")]
async fn share_codes_round_trip() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let bundle = json!({"arctic_share": 1, "kind": "hud", "data": {"hud": {}}});
    let (s, _) = call(&app, "POST", "/v1/shares", None, Some(bundle.clone())).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, v) = call(
        &app,
        "POST",
        "/v1/shares",
        Some(&token),
        Some(bundle.clone()),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let code = v["code"].as_str().unwrap().to_owned();
    let (_, again) = call(
        &app,
        "POST",
        "/v1/shares",
        Some(&token),
        Some(bundle.clone()),
    )
    .await;
    assert_eq!(again["code"], code.as_str());
    let pretty = format!("{}-{}", &code[..4], &code[4..]).to_uppercase();
    let (s, back) = call(&app, "GET", &format!("/v1/shares/{pretty}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(back, bundle);
    let (s, _) = call(&app, "GET", "/v1/shares/nothere2", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    for bad in [
        json!({"arctic_share": 1, "kind": "virus"}),
        json!({"arctic_share": 2, "kind": "hud"}),
        json!(["not", "an", "object"]),
    ] {
        let (s, _) = call(&app, "POST", "/v1/shares", Some(&token), Some(bad)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    let huge = json!({"arctic_share": 1, "kind": "profile", "data": "x".repeat(300_000)});
    let (s, _) = call(&app, "POST", "/v1/shares", Some(&token), Some(huge)).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
}

/// A tiny Minecraft server: answers status with `motd`, and logins with an
/// encryption request (premium).
fn fake_minecraft(motd: std::sync::Arc<std::sync::Mutex<String>>) -> u16 {
    use std::io::{Read, Write};
    fn var_int(r: &mut impl Read) -> usize {
        let (mut v, mut shift) = (0usize, 0);
        loop {
            let mut b = [0u8];
            r.read_exact(&mut b).unwrap();
            v |= usize::from(b[0] & 0x7f) << shift;
            if b[0] & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }
    fn packet(r: &mut impl Read) -> Vec<u8> {
        let len = var_int(r);
        let mut buf = vec![0; len];
        r.read_exact(&mut buf).unwrap();
        buf
    }
    fn send(w: &mut impl Write, body: &[u8]) {
        let mut out = Vec::new();
        let mut n = body.len();
        loop {
            if n < 0x80 {
                out.push(n as u8);
                break;
            }
            out.push((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
        out.extend_from_slice(body);
        w.write_all(&out).unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let mut s = stream.unwrap();
            let handshake = packet(&mut s);
            if *handshake.last().unwrap() == 1 {
                packet(&mut s);
                let text = motd.lock().unwrap().clone();
                let json = json!({"description": text, "players": {"online": 2, "max": 20},
                    "version": {"name": "1.21.8", "protocol": 772}})
                .to_string();
                assert!(json.len() < 0x80);
                let mut body = vec![0, json.len() as u8];
                body.extend_from_slice(json.as_bytes());
                send(&mut s, &body);
                let ping = packet(&mut s);
                send(&mut s, &ping);
            } else {
                packet(&mut s);
                send(&mut s, &[1, 0]);
            }
        }
    });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn servers_submit_verify_and_review() {
    let app = app().await;
    let token = microsoft_token(&app).await;
    let (s, list) = call(&app, "GET", "/v1/servers", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(list["servers"].as_array().unwrap().is_empty());

    // The code isn't known until submitting: the MOTD is filled in after.
    let motd = std::sync::Arc::new(std::sync::Mutex::new(String::from("Welcome!")));
    let port = fake_minecraft(motd.clone());
    let submission = json!({"address": format!("127.0.0.1:{port}"), "name": "Test SMP", "tags": ["SMP", "bad tag"]});
    let (s, _) = call(&app, "POST", "/v1/servers", None, Some(submission.clone())).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, v) = call(&app, "POST", "/v1/servers", Some(&token), Some(submission)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let id = v["id"].as_str().unwrap().to_owned();
    *motd.lock().unwrap() = format!("Welcome! {}", v["code"].as_str().unwrap());
    let (s, v) = call(
        &app,
        "POST",
        &format!("/v1/servers/{id}/verify"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["state"], "pending");

    // Waiting for review: not public yet; admins see it, premium detected.
    let (_, list) = call(&app, "GET", "/v1/servers", None, None).await;
    assert!(list["servers"].as_array().unwrap().is_empty());
    let (s, _) = call(&app, "GET", "/v1/admin/servers", None, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let admin = |method: &'static str, uri: String, body: Option<serde_json::Value>| {
        let app = app.clone();
        async move {
            let mut req = Request::builder()
                .method(method)
                .uri(uri)
                .header("x-admin-key", "admin-key-admin-key");
            let req = match body {
                Some(b) => {
                    req = req.header("content-type", "application/json");
                    req.body(Body::from(b.to_string()))
                }
                None => req.body(Body::empty()),
            }
            .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            let status = resp.status();
            let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
                .await
                .unwrap();
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default(),
            )
        }
    };
    let (s, pending) = admin("GET", "/v1/admin/servers".into(), None).await;
    assert_eq!(s, StatusCode::OK);
    let entry = &pending["servers"][0];
    assert_eq!(entry["id"], id.as_str());
    assert_eq!(entry["cracked"], false);
    assert_eq!(entry["players"], 2);
    let (s, _) = admin(
        "POST",
        format!("/v1/admin/servers/{id}"),
        Some(json!({"action": "approve"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, list) = call(&app, "GET", "/v1/servers", None, None).await;
    assert_eq!(list["servers"][0]["name"], "Test SMP");
    assert!(list["servers"][0].get("code").is_none());
}
