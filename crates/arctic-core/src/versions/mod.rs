//! Mojang version metadata: the global manifest and per-version JSON.

pub mod model;
pub mod rules;

pub use model::{AssetIndex, Library, VersionJson, maven_path};
pub use rules::{RuleEnv, rules_allow};

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::net::{DownloadJob, download_all, get_json};
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Progress, Result};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MANIFEST_CACHE: &str = "version_manifest_v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub release_time: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Other,
}

impl VersionManifest {
    /// Fetch the manifest, caching it; fall back to the cache when offline.
    pub fn fetch(dirs: &DataDirs) -> Result<Self> {
        let cache = dirs.meta().join(MANIFEST_CACHE);
        match get_json::<Self>(MANIFEST_URL) {
            Ok(manifest) => {
                if let Err(e) = save_json(&cache, &manifest) {
                    log::warn!("could not cache version manifest: {e}");
                }
                Ok(manifest)
            }
            Err(net_err) => {
                log::warn!("version manifest fetch failed, using cache: {net_err}");
                load_json(&cache)?.ok_or(net_err)
            }
        }
    }

    /// All official releases, newest first (manifest order).
    pub fn releases(&self) -> impl Iterator<Item = &VersionEntry> {
        self.versions
            .iter()
            .filter(|v| v.kind == VersionKind::Release)
    }

    pub fn find(&self, id: &str) -> Option<&VersionEntry> {
        self.versions.iter().find(|v| v.id == id)
    }
}

/// Written into `versions/<id>/` once every file of a version was
/// downloaded and verified by a successful launch preparation.
const INSTALLED_MARKER: &str = ".installed";

pub fn mark_installed(dirs: &DataDirs, id: &str) -> Result<()> {
    let marker = dirs.version_dir(id).join(INSTALLED_MARKER);
    std::fs::write(&marker, crate::APP_VERSION).map_err(|e| Error::io(marker, e))
}

/// Ids of versions that are fully downloaded (can launch offline).
pub fn installed_versions(dirs: &DataDirs) -> HashSet<String> {
    let Ok(entries) = std::fs::read_dir(dirs.versions()) else {
        return HashSet::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().join(INSTALLED_MARKER).is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

/// Ensure `<shared>/versions/<id>/<id>.json` is present and verified, then parse it.
pub fn load_version(
    dirs: &DataDirs,
    entry: &VersionEntry,
    progress: Progress,
) -> Result<VersionJson> {
    let path = dirs
        .version_dir(&entry.id)
        .join(format!("{}.json", entry.id));
    let cached_ok = path.is_file()
        && crate::net::sha1_file(&path).is_ok_and(|h| h.eq_ignore_ascii_case(&entry.sha1));
    if !cached_ok {
        let _ = std::fs::remove_file(&path);
        let job = DownloadJob {
            url: entry.url.clone(),
            dest: path.clone(),
            sha1: Some(entry.sha1.clone()),
            size: None,
            lzma: None,
        };
        download_all("Version metadata", vec![job], progress)?;
    }
    let parsed: VersionJson = load_json(&path)?
        .ok_or_else(|| Error::Other(format!("version file missing: {}", path.display())))?;
    if parsed.inherits_from.is_some() {
        // TODO(loaders): merge with the parent profile once mod loaders land.
        return Err(Error::Other(
            "inherited (mod loader) profiles are not supported yet".into(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_markers() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        assert!(installed_versions(&dirs).is_empty());
        std::fs::create_dir_all(dirs.version_dir("1.21")).unwrap();
        std::fs::create_dir_all(dirs.version_dir("1.20")).unwrap();
        mark_installed(&dirs, "1.21").unwrap();
        assert_eq!(
            installed_versions(&dirs),
            HashSet::from(["1.21".to_string()])
        );
    }

    #[test]
    fn filters_releases_and_tolerates_unknown_kinds() {
        let json = r#"{
            "latest": {"release": "1.21", "snapshot": "24w01a"},
            "versions": [
                {"id": "24w01a", "type": "snapshot", "url": "u", "time": "t", "releaseTime": "t", "sha1": "s", "complianceLevel": 1},
                {"id": "1.21", "type": "release", "url": "u", "time": "t", "releaseTime": "t", "sha1": "s"},
                {"id": "x", "type": "experiment", "url": "u", "time": "t", "releaseTime": "t", "sha1": "s"},
                {"id": "b1.7.3", "type": "old_beta", "url": "u", "time": "t", "releaseTime": "t", "sha1": "s"}
            ]
        }"#;
        let m: VersionManifest = serde_json::from_str(json).unwrap();
        let ids: Vec<_> = m.releases().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["1.21"]);
        assert_eq!(m.find("x").unwrap().kind, VersionKind::Other);
        assert_eq!(m.find("b1.7.3").unwrap().kind, VersionKind::OldBeta);
    }
}
