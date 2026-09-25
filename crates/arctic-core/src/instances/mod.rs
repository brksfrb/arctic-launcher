//! Instances: isolated game directories.
//!
//! MVP ships exactly one, the built-in "Vanilla" instance, whose version is
//! picked on the Play tab. User-created instances (with mod loaders) will
//! live next to it under `instances/<id>/` and reuse the shared libraries,
//! assets and runtimes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};

pub const VANILLA_ID: &str = "vanilla";
const INSTANCE_FILE: &str = "instance.json";
/// Game directory inside an instance (what vanilla calls `.minecraft`).
const GAME_DIR: &str = "minecraft";

/// Which mod loader, if any, an instance runs. Only `Vanilla` exists today;
/// loaders plug in here and resolve an extra version profile on top of the
/// vanilla one (see `VersionJson::inherits_from`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Loader {
    Vanilla,
    // TODO(loaders): Fabric { loader_version: String }, Quilt { .. }, Forge { .. }, NeoForge { .. }
}

impl Loader {
    /// Default snowflake for instances of this loader, so different kinds
    /// are recognisable at a glance (users can override it).
    pub fn default_icon(&self) -> InstanceIcon {
        match self {
            Loader::Vanilla => InstanceIcon {
                style: FlakeStyle::Classic,
                color: None,
            },
            // TODO(loaders): Fabric → Stellar, Quilt → Plate, Forge → Crystal, …
        }
    }
}

/// Snowflake shapes used as instance icons.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlakeStyle {
    #[default]
    Classic,
    Stellar,
    Dendrite,
    Plate,
    Star,
    Crystal,
}

impl FlakeStyle {
    pub const ALL: [FlakeStyle; 6] = [
        FlakeStyle::Classic,
        FlakeStyle::Stellar,
        FlakeStyle::Dendrite,
        FlakeStyle::Plate,
        FlakeStyle::Star,
        FlakeStyle::Crystal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FlakeStyle::Classic => "Classic",
            FlakeStyle::Stellar => "Stellar",
            FlakeStyle::Dendrite => "Dendrite",
            FlakeStyle::Plate => "Plate",
            FlakeStyle::Star => "Star",
            FlakeStyle::Crystal => "Crystal",
        }
    }
}

/// How an instance is drawn: flake style plus an optional custom color
/// (`None` = the theme's accent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceIcon {
    pub style: FlakeStyle,
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub loader: Loader,
    /// Pinned Minecraft version. `None` for the default instance, which
    /// follows the Play tab selection.
    pub version: Option<String>,
    /// Per-instance memory override in MiB (falls back to global settings).
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    /// Icon shown for this instance; defaults to the loader's style.
    #[serde(default)]
    pub icon: InstanceIcon,
}

impl Instance {
    pub fn vanilla_default() -> Self {
        Self {
            id: VANILLA_ID.into(),
            name: "Vanilla".into(),
            loader: Loader::Vanilla,
            version: None,
            max_memory_mb: None,
            icon: Loader::Vanilla.default_icon(),
        }
    }

    pub fn is_default(&self) -> bool {
        self.id == VANILLA_ID
    }

    pub fn game_dir(&self, dirs: &DataDirs) -> PathBuf {
        dirs.instance_dir(&self.id).join(GAME_DIR)
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.instance_dir(&self.id).join(INSTANCE_FILE), self)
    }
}

/// Load the default Vanilla instance, creating it on first run.
pub fn load_default(dirs: &DataDirs) -> Result<Instance> {
    let path = dirs.instance_dir(VANILLA_ID).join(INSTANCE_FILE);
    match load_json::<Instance>(&path)? {
        Some(instance) => Ok(instance),
        None => {
            let instance = Instance::vanilla_default();
            instance.save(dirs)?;
            Ok(instance)
        }
    }
}

/// User-created instances (everything except the default). Empty in MVP.
pub fn list_custom(dirs: &DataDirs) -> Result<Vec<Instance>> {
    let Ok(entries) = std::fs::read_dir(dirs.instances()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join(INSTANCE_FILE);
        match load_json::<Instance>(&path) {
            Ok(Some(i)) if !i.is_default() => out.push(i),
            Ok(_) => {}
            Err(e) => log::warn!("skipping broken instance {}: {e}", path.display()),
        }
    }
    out.sort_by_key(|i| i.name.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_defaults_for_old_instance_files() {
        let json = r#"{"id":"x","name":"X","loader":{"type":"vanilla"},"version":null}"#;
        let instance: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(instance.icon, InstanceIcon::default());
        let custom: InstanceIcon =
            serde_json::from_str(r#"{"style":"dendrite","color":[255,0,128]}"#).unwrap();
        assert_eq!(custom.style, FlakeStyle::Dendrite);
        assert_eq!(custom.color, Some([255, 0, 128]));
    }

    #[test]
    fn default_instance_is_created_once() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let first = load_default(&dirs).unwrap();
        assert!(first.is_default());
        assert_eq!(load_default(&dirs).unwrap(), first);
        assert!(
            first
                .game_dir(&dirs)
                .ends_with("instances/vanilla/minecraft")
        );
    }

    #[test]
    fn custom_list_excludes_default() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        load_default(&dirs).unwrap();
        assert!(list_custom(&dirs).unwrap().is_empty());
        let custom = Instance {
            id: "abc".into(),
            name: "Test".into(),
            ..Instance::vanilla_default()
        };
        custom.save(&dirs).unwrap();
        assert_eq!(list_custom(&dirs).unwrap(), vec![custom]);
    }
}
