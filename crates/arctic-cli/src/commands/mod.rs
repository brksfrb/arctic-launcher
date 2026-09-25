//! Command implementations. Each returns the process exit code.

pub mod accounts;
pub mod launch;
pub mod misc;
pub mod versions;

use arctic_core::storage::DataDirs;
use arctic_core::versions::{VersionEntry, VersionManifest};
use arctic_core::{Error, Result};

use crate::output::Out;

/// Shared state for a command run.
pub struct Ctx {
    pub dirs: DataDirs,
    pub out: Out,
}

/// `latest`, `latest-snapshot`, or an exact version id.
pub fn resolve_version(manifest: &VersionManifest, query: &str) -> Result<VersionEntry> {
    let id = match query {
        "latest" | "latest-release" => manifest.latest.release.as_str(),
        "latest-snapshot" => manifest.latest.snapshot.as_str(),
        other => other,
    };
    manifest.find(id).cloned().ok_or_else(|| {
        Error::Other(format!(
            "unknown version '{query}' (see `arctic versions --all`)"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aliases() {
        let manifest: VersionManifest = serde_json::from_str(
            r#"{"latest":{"release":"1.21","snapshot":"24w01a"},"versions":[
                {"id":"24w01a","type":"snapshot","url":"u","releaseTime":"t","sha1":"s"},
                {"id":"1.21","type":"release","url":"u","releaseTime":"t","sha1":"s"}]}"#,
        )
        .unwrap();
        assert_eq!(resolve_version(&manifest, "latest").unwrap().id, "1.21");
        assert_eq!(
            resolve_version(&manifest, "latest-snapshot").unwrap().id,
            "24w01a"
        );
        assert_eq!(resolve_version(&manifest, "1.21").unwrap().id, "1.21");
        assert!(resolve_version(&manifest, "9.9").is_err());
    }
}
