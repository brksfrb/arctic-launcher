//! On-disk layout and JSON persistence.
//!
//! ```text
//! <root>                          %LOCALAPPDATA%\ArcticLauncher (default)
//! ├── settings.json               user settings (no secrets)
//! ├── accounts.json               account store (tokens; local only, never synced)
//! ├── msa.json                    optional local Microsoft client config
//! ├── meta/                       cached Mojang manifests
//! ├── shared/                     game files shared by all instances
//! │   ├── versions/<id>/<id>.{json,jar}
//! │   ├── libraries/…             maven layout
//! │   └── assets/{indexes,objects,virtual,log_configs}
//! ├── runtimes/<component>/       managed Java runtimes
//! ├── instances/<id>/             instance.json + minecraft/ (game dir)
//! ├── logs/                       launcher + game logs
//! └── cache/                      downloaded updates, temp files
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::IoContext;
use crate::{Error, Result};

/// Env var to relocate all launcher data (handy for development).
pub const DATA_DIR_ENV: &str = "ARCTIC_DATA_DIR";
/// Marker file next to the executable that enables portable mode.
pub const PORTABLE_MARKER: &str = "portable.txt";
const DIR_NAME: &str = "ArcticLauncher";

/// Resolved directory layout. Cheap to clone and pass around.
#[derive(Debug, Clone)]
pub struct DataDirs {
    root: PathBuf,
}

impl DataDirs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve the data root: `ARCTIC_DATA_DIR`, then portable mode, then
    /// the OS local data dir (not roaming: game files are large).
    pub fn resolve() -> Result<Self> {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
            return Ok(Self::new(dir));
        }
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(exe_dir) = exe_dir
            && exe_dir.join(PORTABLE_MARKER).exists()
        {
            return Ok(Self::new(exe_dir.join("data")));
        }
        dirs::data_local_dir()
            .map(|d| Self::new(d.join(DIR_NAME)))
            .ok_or_else(|| Error::Other("could not determine local app data directory".into()))
    }

    /// Create the top-level directories.
    pub fn ensure(&self) -> Result<()> {
        for dir in [
            self.meta(),
            self.versions(),
            self.libraries(),
            self.assets(),
            self.runtimes(),
            self.instances(),
            self.logs(),
            self.cache(),
        ] {
            fs::create_dir_all(&dir).at(&dir)?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn msa_config_file(&self) -> PathBuf {
        self.root.join("msa.json")
    }
    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn shared(&self) -> PathBuf {
        self.root.join("shared")
    }
    pub fn versions(&self) -> PathBuf {
        self.shared().join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn libraries(&self) -> PathBuf {
        self.shared().join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.shared().join("assets")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
}

/// Load JSON from `path`, returning `None` if the file does not exist.
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Write JSON atomically (temp file + rename) so a crash never leaves a
/// truncated settings/accounts file behind.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp, bytes).at(&tmp)?;
    fs::rename(&tmp, path).at(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/x.json");
        assert_eq!(load_json::<Vec<u32>>(&path).unwrap(), None);
        save_json(&path, &vec![1u32, 2, 3]).unwrap();
        assert_eq!(load_json::<Vec<u32>>(&path).unwrap(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn layout_is_rooted() {
        let d = DataDirs::new("root");
        assert_eq!(
            d.version_dir("1.21"),
            Path::new("root/shared/versions/1.21")
        );
        assert_eq!(
            d.instance_dir("vanilla"),
            Path::new("root/instances/vanilla")
        );
    }

    #[test]
    fn ensure_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let d = DataDirs::new(dir.path());
        d.ensure().unwrap();
        assert!(d.libraries().is_dir() && d.runtimes().is_dir());
    }
}
