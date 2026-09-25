//! Mod loaders: Fabric, Quilt, NeoForge and Forge.
//!
//! Each loader contributes a version profile that inherits from a vanilla
//! version (`inheritsFrom`). `install_profile` makes sure the loader's
//! profile (and, for Forge/NeoForge, the installer's processed files) are
//! present, and returns the profile merged onto the vanilla version, ready
//! for `launch::install`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::storage::DataDirs;
use crate::versions::VersionJson;
use crate::{Error, Progress, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
}

impl LoaderKind {
    pub const ALL: [LoaderKind; 4] = [
        LoaderKind::Fabric,
        LoaderKind::Quilt,
        LoaderKind::NeoForge,
        LoaderKind::Forge,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::NeoForge => "NeoForge",
            LoaderKind::Forge => "Forge",
        }
    }

    /// Modrinth's loader facet name.
    pub fn modrinth_id(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Forge => "forge",
        }
    }
}

/// One installable loader version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderVersion {
    pub id: String,
    /// Marked stable/recommended by the loader's metadata.
    pub stable: bool,
}

/// Minecraft versions this loader supports.
pub fn game_versions(kind: LoaderKind) -> Result<Vec<String>> {
    Err(Error::Other(format!(
        "{} support is not available yet",
        kind.label()
    )))
}

/// Loader versions available for `game_version`, newest first.
pub fn loader_versions(kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let _ = game_version;
    Err(Error::Other(format!(
        "{} support is not available yet",
        kind.label()
    )))
}

/// Ensure the loader profile for (`vanilla`, `loader_version`) is installed
/// and return it merged onto `vanilla`. `java` is the runtime used to run
/// Forge/NeoForge installer processors (the vanilla client jar is already
/// downloaded when this is called).
pub fn install_profile(
    dirs: &DataDirs,
    kind: LoaderKind,
    loader_version: &str,
    vanilla: &VersionJson,
    java: &Path,
    progress: Progress,
) -> Result<VersionJson> {
    let _ = (dirs, loader_version, vanilla, java, progress);
    Err(Error::Other(format!(
        "{} support is not available yet",
        kind.label()
    )))
}
