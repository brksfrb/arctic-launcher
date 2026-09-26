//! On-disk layout and JSON persistence.
//!
//! ```text
//! <root>                          %LOCALAPPDATA%\ArcticLauncher (default)
//! ├── profiles.json               profile list + active profile
//! ├── profiles/<id>/              everything that belongs to one profile:
//! │   ├── settings.json             settings (no secrets)
//! │   ├── accounts.json             accounts (tokens; local only)
//! │   ├── proxy.json                SOCKS5 proxy (may hold a password; local only)
//! │   ├── instances/<id>/           instance.json + minecraft/ (worlds, mods…)
//! │   └── logs/                     game logs
//! ├── msa.json                    optional local Microsoft client config
//! ├── meta/                       cached Mojang manifests
//! ├── shared/                     content-addressed game files (all profiles)
//! │   ├── versions/<id>/<id>.{json,jar}
//! │   ├── libraries/…             maven layout
//! │   └── assets/{indexes,objects,virtual,log_configs}
//! ├── runtimes/<component>/       managed Java runtimes
//! ├── logs/launcher.log           launcher log
//! └── cache/                      avatars, downloaded updates
//! ```
//!
//! A `DataDirs` is either unscoped (profile paths fall back to the root,
//! used only before profiles are resolved and for migration) or scoped to a
//! profile via [`DataDirs::with_profile`].

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
    /// Where profile-scoped data lives (`root` until a profile is chosen).
    profile_root: PathBuf,
}

impl DataDirs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            profile_root: root.clone(),
            root,
        }
    }

    /// The same layout scoped to profile `id`.
    pub fn with_profile(&self, id: &str) -> Self {
        Self {
            root: self.root.clone(),
            profile_root: self.profiles_dir().join(id),
        }
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

    /// Create the launcher-wide directories, plus the profile's own ones
    /// when this layout is scoped to a profile.
    pub fn ensure(&self) -> Result<()> {
        let mut dirs = vec![
            self.meta(),
            self.versions(),
            self.libraries(),
            self.assets(),
            self.runtimes(),
            self.launcher_logs(),
            self.cache(),
        ];
        if self.profile_root != self.root {
            dirs.extend([self.instances(), self.logs()]);
        }
        for dir in dirs {
            fs::create_dir_all(&dir).at(&dir)?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Folder of the current profile (the root when unscoped).
    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }
    pub fn profiles_file(&self) -> PathBuf {
        self.root.join("profiles.json")
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.profile_root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.profile_root.join("accounts.json")
    }
    pub fn proxy_file(&self) -> PathBuf {
        self.profile_root.join("proxy.json")
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
        self.profile_root.join("instances")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    /// Game logs of the current profile.
    pub fn logs(&self) -> PathBuf {
        self.profile_root.join("logs")
    }
    /// Launcher-wide logs (`launcher.log`).
    pub fn launcher_logs(&self) -> PathBuf {
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
    fn profile_scoping_splits_shared_and_private_paths() {
        let d = DataDirs::new("root").with_profile("work");
        assert_eq!(
            d.settings_file(),
            Path::new("root/profiles/work/settings.json")
        );
        assert_eq!(
            d.instance_dir("vanilla"),
            Path::new("root/profiles/work/instances/vanilla")
        );
        assert_eq!(d.logs(), Path::new("root/profiles/work/logs"));
        assert_eq!(d.libraries(), Path::new("root/shared/libraries"));
        assert_eq!(d.launcher_logs(), Path::new("root/logs"));
        assert_eq!(d.profiles_file(), Path::new("root/profiles.json"));
    }

    #[test]
    fn ensure_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let d = DataDirs::new(dir.path());
        d.ensure().unwrap();
        assert!(d.libraries().is_dir() && d.runtimes().is_dir());
        assert!(
            !d.instances().exists(),
            "unscoped layout must not create profile dirs"
        );
        let scoped = d.with_profile("p");
        scoped.ensure().unwrap();
        assert!(scoped.instances().is_dir() && scoped.logs().is_dir());
    }
}
