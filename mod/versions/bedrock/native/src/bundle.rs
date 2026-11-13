use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = "arctic-bedrock.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub java_home: PathBuf,
    pub config_dir: PathBuf,
    /// Optional local archive used only for pinned Java reference artwork.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_reference_jar: Option<PathBuf>,
    pub game_version: String,
    /// Local Arctic profile; never an Xbox or Mojang credential.
    #[serde(default = "offline_name")]
    pub offline_name: String,
    #[serde(default)]
    pub renderer: Renderer,
}

fn offline_name() -> String {
    "ArcticGuest".into()
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    Dx11,
    #[default]
    Dx12,
}

impl Bundle {
    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(CONFIG_FILE);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let bundle: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        bundle.validate(dir)?;
        Ok(bundle)
    }

    pub fn validate(&self, dir: &Path) -> Result<(), String> {
        if !self.java_home.is_absolute() || !self.config_dir.is_absolute() {
            return Err("Java home and client config directory must be absolute paths".into());
        }
        if self.game_version.trim().is_empty() {
            return Err("a game version is required".into());
        }
        if let Some(path) = &self.java_reference_jar
            && (!path.is_absolute() || !path.is_file())
        {
            return Err("java_reference_jar must be an absolute path to a local file".into());
        }
        if !(3..=16).contains(&self.offline_name.len())
            || !self
                .offline_name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("offline_name needs 3–16 letters, digits or underscores".into());
        }
        for path in [self.jvm(), dir.join("arctic-bedrock.jar")] {
            if !path.is_file() {
                return Err(format!(
                    "missing {} (build the Bedrock client bundle)",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    pub fn jvm(&self) -> PathBuf {
        self.java_home.join("bin/server/jvm.dll")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_rejects_missing_runtime_and_relative_config() {
        let dir = tempfile::tempdir().unwrap();
        let mut bundle = Bundle {
            java_home: dir.path().join("java"),
            config_dir: dir.path().join("config"),
            java_reference_jar: None,
            game_version: "1.26.52".into(),
            offline_name: "ArcticGuest".into(),
            renderer: Renderer::Dx12,
        };
        assert!(bundle.validate(dir.path()).unwrap_err().contains("jvm.dll"));
        std::fs::create_dir_all(bundle.jvm().parent().unwrap()).unwrap();
        std::fs::write(bundle.jvm(), []).unwrap();
        std::fs::write(dir.path().join("arctic-bedrock.jar"), []).unwrap();
        assert!(bundle.validate(dir.path()).is_ok());
        bundle.offline_name = "../Account".into();
        assert!(
            bundle
                .validate(dir.path())
                .unwrap_err()
                .contains("offline_name")
        );
        bundle.offline_name = "ArcticGuest".into();
        bundle.config_dir = "relative".into();
        assert!(
            bundle
                .validate(dir.path())
                .unwrap_err()
                .contains("absolute")
        );
    }
}
