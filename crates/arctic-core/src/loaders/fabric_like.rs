//! Fabric (meta.fabricmc.net v2) and Quilt (meta.quiltmc.org v3).
//!
//! Both publish ready-made launcher profiles, so installing is just
//! fetching (and caching) one JSON document; its Maven libraries are
//! downloaded later by `launch::install`.

use serde::Deserialize;
use serde_json::Value;

use super::util::{
    compare_versions, encode_segment, load_saved_profile, parse_profile, save_profile,
};
use super::{LoaderKind, LoaderVersion};
use crate::net::get_json;
use crate::storage::DataDirs;
use crate::versions::{VersionJson, merge};
use crate::{Error, Result};

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const QUILT_META: &str = "https://meta.quiltmc.org/v3";

#[derive(Debug, Deserialize)]
struct GameEntry {
    version: String,
}

#[derive(Debug, Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Debug, Deserialize)]
struct LoaderInfo {
    version: String,
    /// Fabric only; Quilt marks pre-releases in the version string.
    stable: Option<bool>,
}

/// (meta base URL, profile id prefix)
fn meta(kind: LoaderKind) -> (&'static str, &'static str) {
    match kind {
        LoaderKind::Quilt => (QUILT_META, "quilt-loader"),
        _ => (FABRIC_META, "fabric-loader"),
    }
}

pub fn game_versions(kind: LoaderKind) -> Result<Vec<String>> {
    let (base, _) = meta(kind);
    let entries: Vec<GameEntry> =
        get_json(&format!("{base}/versions/game")).map_err(|e| unreachable_meta(kind, &e))?;
    Ok(entries.into_iter().map(|e| e.version).collect())
}

pub fn loader_versions(kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let (base, _) = meta(kind);
    let url = format!("{base}/versions/loader/{}", encode_segment(game_version));
    let entries: Vec<LoaderEntry> = get_json(&url).map_err(|e| unreachable_meta(kind, &e))?;
    Ok(to_loader_versions(entries))
}

fn to_loader_versions(entries: Vec<LoaderEntry>) -> Vec<LoaderVersion> {
    let mut out: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| LoaderVersion {
            stable: e
                .loader
                .stable
                .unwrap_or_else(|| !e.loader.version.contains('-')),
            id: e.loader.version,
        })
        .collect();
    // Quilt's per-game list is unordered; Fabric's is already newest first.
    out.sort_by(|a, b| compare_versions(&b.id, &a.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// Fetch (or reuse) the profile and merge it onto `vanilla`.
pub fn install(
    dirs: &DataDirs,
    kind: LoaderKind,
    loader_version: &str,
    vanilla: &VersionJson,
) -> Result<VersionJson> {
    let (base, prefix) = meta(kind);
    let game = vanilla.id.as_str();
    let expected_id = format!("{prefix}-{loader_version}-{game}");
    if let Some(profile) = load_saved_profile(dirs, &expected_id) {
        return Ok(merge(vanilla.clone(), profile));
    }
    let url = format!(
        "{base}/versions/loader/{}/{}/profile/json",
        encode_segment(game),
        encode_segment(loader_version)
    );
    let raw: Value = get_json(&url).map_err(|e| {
        Error::Other(format!(
            "{} {loader_version} could not be installed for Minecraft {game} \
             (is this combination available?): {e}",
            kind.label()
        ))
    })?;
    let profile = parse_profile(&raw)?;
    let id = save_profile(dirs, &raw)?;
    if id != expected_id {
        log::debug!(
            "{} profile id {id} differs from {expected_id}",
            kind.label()
        );
    }
    Ok(merge(vanilla.clone(), profile))
}

fn unreachable_meta(kind: LoaderKind, e: &Error) -> Error {
    Error::Other(format!(
        "could not load the {} version list: {e}",
        kind.label()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_loader_list_keeps_stability_flag() {
        let json = r#"[
            {"loader":{"separator":".","build":5,"maven":"net.fabricmc:fabric-loader:0.19.5","version":"0.19.5","stable":true},
             "intermediary":{"maven":"net.fabricmc:intermediary:1.21.4","version":"1.21.4","stable":true}},
            {"loader":{"separator":".","build":4,"maven":"net.fabricmc:fabric-loader:0.19.4-beta.1","version":"0.19.4-beta.1","stable":false}},
            {"loader":{"version":"0.16.10","stable":false}}
        ]"#;
        let list = to_loader_versions(serde_json::from_str(json).unwrap());
        let ids: Vec<(&str, bool)> = list.iter().map(|v| (v.id.as_str(), v.stable)).collect();
        assert_eq!(
            ids,
            [
                ("0.19.5", true),
                ("0.19.4-beta.1", false),
                ("0.16.10", false)
            ]
        );
    }

    #[test]
    fn quilt_loader_list_is_sorted_and_prereleases_unstable() {
        let json = r#"[
            {"loader":{"maven":"org.quiltmc:quilt-loader:0.20.0-beta.9","version":"0.20.0-beta.9","build":9}},
            {"loader":{"version":"0.24.0"}},
            {"loader":{"version":"0.29.2"}},
            {"loader":{"version":"0.20.0-beta.10"}}
        ]"#;
        let list = to_loader_versions(serde_json::from_str(json).unwrap());
        let ids: Vec<(&str, bool)> = list.iter().map(|v| (v.id.as_str(), v.stable)).collect();
        assert_eq!(
            ids,
            [
                ("0.29.2", true),
                ("0.24.0", true),
                ("0.20.0-beta.10", false),
                ("0.20.0-beta.9", false)
            ]
        );
    }

    #[test]
    fn parses_game_list() {
        let list: Vec<GameEntry> = serde_json::from_str(
            r#"[{"version":"26.4-snapshot-1","stable":false},{"version":"26.3","stable":true}]"#,
        )
        .unwrap();
        assert_eq!(list[1].version, "26.3");
    }

    #[test]
    fn reuses_saved_profile_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let raw = serde_json::json!({
            "id": "fabric-loader-0.16.10-1.21.4", "inheritsFrom": "1.21.4",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{"name": "net.fabricmc:fabric-loader:0.16.10", "url": "https://maven.fabricmc.net/"}]
        });
        save_profile(&dirs, &raw).unwrap();
        let vanilla: VersionJson = serde_json::from_str(
            r#"{"id":"1.21.4","type":"release","mainClass":"net.minecraft.client.main.Main"}"#,
        )
        .unwrap();
        let merged = install(&dirs, LoaderKind::Fabric, "0.16.10", &vanilla).unwrap();
        assert_eq!(merged.id, "fabric-loader-0.16.10-1.21.4");
        assert!(merged.main_class.ends_with("KnotClient"));
        assert_eq!(merged.kind, "release");
    }
}
