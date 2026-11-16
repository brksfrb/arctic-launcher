//! Cache contract used by PolyPatcher 1.10.4's legacy OneConfig wrapper/loader:
//! Verified against PolyPatcher-1.8.9-forge-1.10.4.jar (stage0) and
//! oneconfig-loader-launchwrapper-1.0.0-beta17.jar (stage1). The embedded
//! wrapper uses the launchwrapper subdirectory, unlike older loader sources.
//! Both stages fall back to these files when the API is unreachable.
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::IoContext;
use crate::{Error, Result};

const API: &str = "https://api.polyfrost.org/oneconfig/1.8.9-forge";
const LOADER: &str = "launchwrapper/OneConfig-Loader.jar";
const RUNTIME: &str = "OneConfig (1.8.9-forge).jar";

#[derive(Deserialize, Serialize)]
struct Artifact {
    url: String,
    sha256: String,
}

#[derive(Deserialize, Serialize)]
struct Metadata {
    loader: Artifact,
    release: Artifact,
    snapshot: Artifact,
}

impl Metadata {
    fn files(&self, snapshot: bool) -> [(&'static str, &Artifact); 2] {
        [
            (LOADER, &self.loader),
            (
                RUNTIME,
                if snapshot {
                    &self.snapshot
                } else {
                    &self.release
                },
            ),
        ]
    }
}

pub(super) fn prepare(game_dir: &Path) -> Result<()> {
    prepare_with(game_dir, |url| {
        Ok(crate::net::agent()
            .get(url)
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(45)))
            .build()
            .call()?
            .body_mut()
            .with_config()
            .limit(128 * 1024 * 1024)
            .read_to_vec()?)
    })
}

fn valid(bytes: &[u8], artifact: &Artifact) -> bool {
    !bytes.is_empty() && hex::encode(Sha256::digest(bytes)).eq_ignore_ascii_case(&artifact.sha256)
}

fn retry<T>(mut action: impl FnMut() -> Result<T>) -> Result<T> {
    for attempt in 1..=3 {
        match action() {
            Ok(value) => return Ok(value),
            Err(e) if attempt == 3 => return Err(e),
            Err(e) => log::info!("OneConfig prefetch attempt {attempt} failed: {e}"),
        }
    }
    unreachable!()
}

fn prepare_with(game_dir: &Path, mut fetch: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<()> {
    let dir = game_dir.join("OneConfig");
    // Match the loader's release/snapshot preference without changing player settings.
    let config: Option<serde_json::Value> = crate::storage::load_json(&dir.join("OneConfig.json"))?;
    let snapshot = config
        .as_ref()
        .and_then(|v| v["updateChannel"].as_u64())
        .is_some_and(|v| v != 0);
    let manifest = dir.join("arctic-cache.json");
    let cached: Option<Metadata> = crate::storage::load_json(&manifest).unwrap_or(None);
    // Refresh metadata before launch so OneConfig's own updater sees the same hashes.
    // If the API is offline, only use a cache whose bytes we previously verified.
    let meta: Metadata = match retry(|| Ok(serde_json::from_slice(&fetch(API)?)?)) {
        Ok(meta) => meta,
        Err(e) => {
            if let Some(meta) = &cached
                && meta.files(snapshot).iter().all(|(name, artifact)| {
                    std::fs::read(dir.join(name)).is_ok_and(|b| valid(&b, artifact))
                })
            {
                log::info!("OneConfig API unavailable ({e}); using verified cached jars");
                return Ok(());
            }
            return Err(e);
        }
    };
    std::fs::create_dir_all(&dir).at(&dir)?;
    for (name, artifact) in meta.files(snapshot) {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|b| valid(&b, artifact)) {
            continue;
        }
        let bytes = retry(|| {
            if !artifact.url.starts_with("https://") || artifact.sha256.len() != 64 {
                return Err(Error::Other("invalid OneConfig artifact metadata".into()));
            }
            let bytes = fetch(&artifact.url)?;
            if !valid(&bytes, artifact) {
                return Err(Error::Checksum(name.into()));
            }
            Ok(bytes)
        })?;
        std::fs::create_dir_all(path.parent().unwrap()).at(&path)?;
        let temp = path.with_extension("arctic-part");
        std::fs::write(&temp, bytes).at(&temp)?;
        std::fs::rename(&temp, &path).at(&path)?;
    }
    crate::storage::save_json(&manifest, &meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preference_and_failed_download_never_publish_partial_jars() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("OneConfig");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("OneConfig.json"),
            br#"{"autoUpdate":true,"updateChannel":1}"#,
        )
        .unwrap();
        let artifact = |name: &str| Artifact {
            url: format!("https://example.invalid/{name}"),
            sha256: hex::encode(Sha256::digest(name.as_bytes())),
        };
        let meta = serde_json::to_vec(&Metadata {
            loader: artifact("loader"),
            release: artifact("release"),
            snapshot: artifact("snapshot"),
        })
        .unwrap();
        let mut failed = 0;
        assert!(
            prepare_with(dir.path(), |url| {
                if url == API {
                    return Ok(meta.clone());
                }
                failed += 1;
                Ok(b"bad checksum".to_vec())
            })
            .is_err()
        );
        assert_eq!(failed, 3);
        assert!(!cache.join(LOADER).exists());
        assert!(!cache.join("arctic-cache.json").exists());
        prepare_with(dir.path(), |url| {
            if url == API {
                return Ok(meta.clone());
            }
            assert!(!url.ends_with("/release"));
            Ok(url.rsplit('/').next().unwrap().as_bytes().to_vec())
        })
        .unwrap();
        assert_eq!(std::fs::read(cache.join(LOADER)).unwrap(), b"loader");
        assert_eq!(std::fs::read(cache.join(RUNTIME)).unwrap(), b"snapshot");
    }

    #[test]
    fn retries_checksums_and_reuses_verified_cache_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"mock jar";
        let artifact = || Artifact {
            url: "https://example.invalid/jar".into(),
            sha256: hex::encode(Sha256::digest(bytes)),
        };
        let meta = serde_json::to_vec(&Metadata {
            loader: artifact(),
            release: artifact(),
            snapshot: artifact(),
        })
        .unwrap();
        let mut calls = 0;
        prepare_with(dir.path(), |url| {
            calls += 1;
            if url == API {
                return Ok(meta.clone());
            }
            if calls == 2 {
                return Ok(b"truncated".to_vec());
            }
            Ok(bytes.to_vec())
        })
        .unwrap();
        assert_eq!(calls, 4);
        for name in [LOADER, RUNTIME] {
            assert_eq!(
                std::fs::read(dir.path().join("OneConfig").join(name)).unwrap(),
                bytes
            );
        }
        prepare_with(dir.path(), |_| Err(Error::Http("mock offline".into()))).unwrap();
        std::fs::write(dir.path().join("OneConfig").join(RUNTIME), b"broken").unwrap();
        let mut attempts = 0;
        assert!(
            prepare_with(dir.path(), |_| {
                attempts += 1;
                Err(Error::Http("offline".into()))
            })
            .is_err()
        );
        assert_eq!(attempts, 3);
    }
}
