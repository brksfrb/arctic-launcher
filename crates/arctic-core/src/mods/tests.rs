//! Network-free tests for the Modrinth wire format, version choice and the
//! public folder operations.

use std::fs;

use serde_json::json;

use super::modrinth::{self, SearchResponse, Version, encode, pick_version};
use super::*;

const SEARCH_SAMPLE: &str = r#"{
  "hits": [
    {"project_id": "AANobbMI", "project_type": "mod", "slug": "sodium",
     "author": "jellysquid3", "title": "Sodium",
     "description": "The fastest rendering optimization mod",
     "categories": ["fabric", "neoforge", "optimization"],
     "downloads": 70000000, "follows": 30000,
     "icon_url": "https://cdn.modrinth.com/data/AANobbMI/icon.png",
     "date_created": "2021-01-03T00:53:34Z", "latest_version": "mc1.21.4-0.6.5",
     "license": "LicenseRef-Polyform-Shield-1.0.0", "gallery": []},
    {"project_id": "xyz", "slug": "plain", "title": "Plain", "description": "",
     "author": "someone", "downloads": 5, "icon_url": "", "categories": []},
    {"project_id": "nulls", "icon_url": null}
  ],
  "offset": 0, "limit": 20, "total_hits": 123
}"#;

#[test]
fn search_response_maps_to_page() {
    let resp: SearchResponse = serde_json::from_str(SEARCH_SAMPLE).unwrap();
    let page = resp.into_page();
    assert_eq!(page.total, 123);
    assert_eq!(page.hits.len(), 3);
    let sodium = &page.hits[0];
    assert_eq!(
        (sodium.project_id.as_str(), sodium.slug.as_str()),
        ("AANobbMI", "sodium")
    );
    assert_eq!(sodium.author, "jellysquid3");
    assert_eq!(sodium.downloads, 70_000_000);
    assert_eq!(sodium.categories, ["fabric", "neoforge", "optimization"]);
    assert!(sodium.icon_url.as_deref().unwrap().ends_with("icon.png"));
    assert_eq!(page.hits[1].icon_url, None, "empty icon URL is no icon");
    assert_eq!(page.hits[2].title, "");
}

#[test]
fn search_url_encodes_query_facets_and_clamps_limit() {
    let q = SearchQuery {
        text: " fast & light ".into(),
        game_version: "1.21.4".into(),
        loader: Some(LoaderKind::Fabric),
        sort: SortBy::Downloads,
        offset: 40,
        limit: 500,
        project_type: super::ProjectType::Mod,
    };
    let url = modrinth::search_url(&q);
    let facets = encode(r#"[["project_type:mod"],["versions:1.21.4"],["categories:fabric"]]"#);
    assert_eq!(
        url,
        format!(
            "https://api.modrinth.com/v2/search?query=fast%20%26%20light&facets={facets}\
             &index=downloads&offset=40&limit=100"
        )
    );
    let any = modrinth::search_url(&SearchQuery::default());
    assert!(any.ends_with("&index=relevance&offset=0&limit=20"), "{any}");
    assert!(any.contains(&encode(r#"[["project_type:mod"]]"#)));
    let quilt = modrinth::search_url(&SearchQuery {
        loader: Some(LoaderKind::Quilt),
        ..SearchQuery::default()
    });
    assert!(quilt.contains(&encode(r#"["categories:quilt","categories:fabric"]"#)));
}

#[test]
fn encoder_keeps_unreserved_and_escapes_utf8() {
    assert_eq!(encode("aZ09-_.~"), "aZ09-_.~");
    assert_eq!(encode("a b/\"ç"), "a%20b%2F%22%C3%A7");
    assert_eq!(
        modrinth::project_versions_url("P1", &["fabric"], "1.21.4"),
        "https://api.modrinth.com/v2/project/P1/version?loaders=%5B%22fabric%22%5D\
         &game_versions=%5B%221.21.4%22%5D"
    );
}

fn version(id: &str, kind: &str, date: &str, game: &str, loader: &str) -> serde_json::Value {
    json!({
        "id": id, "project_id": "P", "name": id, "version_number": id,
        "version_type": kind, "date_published": date, "downloads": 1,
        "game_versions": [game], "loaders": [loader], "featured": false,
        "files": [
            {"url": format!("https://cdn/{id}-sources.jar"), "filename": format!("{id}-sources.jar"),
             "primary": false, "size": 2, "hashes": {"sha1": "aa", "sha512": "bb"}},
            {"url": format!("https://cdn/{id}.jar"), "filename": format!("{id}.jar"),
             "primary": true, "size": 3, "hashes": {"sha1": "cc", "sha512": "dd"}}
        ],
        "dependencies": []
    })
}

fn versions(values: Vec<serde_json::Value>) -> Vec<Version> {
    serde_json::from_value(json!(values)).unwrap()
}

#[test]
fn newest_release_is_preferred_over_newer_betas() {
    let all = versions(vec![
        version(
            "beta-new",
            "beta",
            "2024-12-10T00:00:00Z",
            "1.21.4",
            "fabric",
        ),
        version(
            "rel-old",
            "release",
            "2024-11-01T00:00:00Z",
            "1.21.4",
            "fabric",
        ),
        version(
            "rel-new",
            "release",
            "2024-12-01T00:00:00Z",
            "1.21.4",
            "fabric",
        ),
    ]);
    let chosen = pick_version(&all, &["fabric"], "1.21.4").unwrap();
    assert_eq!(chosen.id, "rel-new");
    assert_eq!(chosen.primary_file().unwrap().filename, "rel-new.jar");
}

#[test]
fn falls_back_to_beta_then_alpha_and_filters_incompatible() {
    let all = versions(vec![
        version(
            "rel-other-game",
            "release",
            "2024-12-10T00:00:00Z",
            "1.20.1",
            "fabric",
        ),
        version(
            "rel-forge",
            "release",
            "2024-12-10T00:00:00Z",
            "1.21.4",
            "forge",
        ),
        version("alpha", "alpha", "2024-12-09T00:00:00Z", "1.21.4", "fabric"),
        version("beta", "beta", "2024-12-01T00:00:00Z", "1.21.4", "fabric"),
    ]);
    assert_eq!(
        pick_version(&all, &["fabric"], "1.21.4").unwrap().id,
        "beta"
    );
    assert_eq!(
        pick_version(&all[..3], &["fabric"], "1.21.4").unwrap().id,
        "alpha"
    );
    assert!(pick_version(&all, &["quilt"], "1.21.4").is_none());
    assert_eq!(
        pick_version(&all, &["quilt", "fabric"], "1.21.4")
            .unwrap()
            .id,
        "beta"
    );
}

#[test]
fn primary_file_falls_back_to_first_and_versions_without_files_are_skipped() {
    let mut raw = version("v", "release", "2024-01-01T00:00:00Z", "1.21.4", "fabric");
    raw["files"][1]["primary"] = json!(false);
    let v: Version = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(v.primary_file().unwrap().filename, "v-sources.jar");
    raw["files"] = json!([]);
    assert!(pick_version(&versions(vec![raw]), &["fabric"], "1.21.4").is_none());
}

#[test]
fn http_errors_are_readable() {
    assert!(modrinth::status_error(200, "x").is_none());
    let not_found = modrinth::status_error(404, "project foo").unwrap();
    assert_eq!(
        not_found.to_string(),
        "project foo was not found on Modrinth"
    );
    let limited = modrinth::status_error(429, "search").unwrap().to_string();
    assert!(limited.contains("rate limit"), "{limited}");
    assert!(modrinth::user_agent().starts_with("brksfrb/arctic-launcher/"));
}

fn tracked(project: &str, file: &str) -> InstalledMod {
    InstalledMod {
        project_id: project.into(),
        version_id: format!("{project}-v"),
        title: project.into(),
        version_number: "1".into(),
        file_name: file.into(),
        icon_url: None,
        dependency: false,
    }
}

#[test]
fn project_info_fills_titles_and_icons() {
    let projects: Vec<Project> = serde_json::from_value(json!([
        {"id": "a", "title": "Alpha", "icon_url": "https://i/a.png", "slug": "alpha"},
        {"id": "b", "title": "", "icon_url": null}
    ]))
    .unwrap();
    let mods = apply_project_info(
        vec![tracked("a", "a.jar"), tracked("b", "b.jar")],
        &projects,
    );
    assert_eq!(mods[0].title, "Alpha");
    assert_eq!(mods[0].icon_url.as_deref(), Some("https://i/a.png"));
    assert_eq!(mods[1].title, "b");
}

#[test]
fn record_replaces_old_file_of_same_project() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    fs::write(d.join("a-1.jar.disabled"), b"old").unwrap();
    fs::write(d.join("a-2.jar"), b"new").unwrap();
    let current = ModIndex::default().with(tracked("a", "a-1.jar"));
    assert_eq!(installed_projects(&current, d).len(), 1);
    let updated = record(&current, &[tracked("a", "a-2.jar")], d).unwrap();
    assert_eq!(updated.mods, [tracked("a", "a-2.jar")]);
    assert!(!d.join("a-1.jar.disabled").exists());
    assert!(d.join("a-2.jar").is_file());
}

#[test]
fn public_list_enable_remove_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mods_dir = dir.path().join("mods");
    let index = index_path(dir.path());
    fs::create_dir(&mods_dir).unwrap();
    fs::write(mods_dir.join("a.jar"), b"a").unwrap();
    fs::write(mods_dir.join("Hand.jar"), b"h").unwrap();
    ModIndex::default()
        .with(tracked("zz", "a.jar"))
        .save(&index)
        .unwrap();

    let names = |l: Vec<ModFile>| -> Vec<(String, bool)> {
        l.into_iter().map(|m| (m.file_name, m.enabled)).collect()
    };
    let listed = list(&mods_dir, &index).unwrap();
    assert_eq!(
        listed[0].file_name, "Hand.jar",
        "untracked sorted by file name"
    );
    assert_eq!(listed[1].tracked.as_ref().unwrap().project_id, "zz");

    set_enabled(&mods_dir, "a.jar", false).unwrap();
    assert!(names(list(&mods_dir, &index).unwrap()).contains(&("a.jar".into(), false)));
    remove(&mods_dir, &index, "a.jar").unwrap();
    assert_eq!(
        names(list(&mods_dir, &index).unwrap()),
        [("Hand.jar".into(), true)]
    );
    assert!(ModIndex::load(&index).unwrap().mods.is_empty());
    remove(&mods_dir, &index, "Hand.jar").unwrap();
    assert!(list(&mods_dir, &index).unwrap().is_empty());
}
