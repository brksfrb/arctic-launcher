//! Mod loaders: Fabric, Quilt, NeoForge and Forge.
//!
//! Each loader contributes a version profile that inherits from a vanilla
//! version (`inheritsFrom`). `install_profile` makes sure the loader's
//! profile (and, for Forge/NeoForge, the installer's processed files) are
//! present, and returns the profile merged onto the vanilla version, ready
//! for `launch::install`.

use std::path::Path;

use serde::{Deserialize, Serialize};

mod fabric_like;
mod forge;
mod forge_meta;
mod installer;
mod legacy_forge;
mod processors;
mod util;

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

/// Minecraft versions this loader supports, newest first.
pub fn game_versions(kind: LoaderKind) -> Result<Vec<String>> {
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric_like::game_versions(kind),
        LoaderKind::NeoForge => forge_meta::neoforge_game_versions(),
        LoaderKind::Forge => forge_meta::forge_game_versions(),
    }
}

/// Loader versions available for `game_version`, newest first.
pub fn loader_versions(kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let game_version = game_version.trim();
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric_like::loader_versions(kind, game_version),
        LoaderKind::NeoForge => forge_meta::neoforge_loader_versions(game_version),
        LoaderKind::Forge => forge_meta::forge_loader_versions(game_version),
    }
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
    let loader_version = loader_version.trim();
    if loader_version.is_empty() || loader_version.contains(['/', '\\']) {
        return Err(Error::Other(format!(
            "invalid {} version \"{loader_version}\"",
            kind.label()
        )));
    }
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            fabric_like::install(dirs, kind, loader_version, vanilla)
        }
        LoaderKind::NeoForge => {
            check_neoforge_target(loader_version, &vanilla.id)?;
            let sources = [forge_meta::neoforge_installer(loader_version)];
            forge::install(dirs, &sources, vanilla, java, progress)
        }
        LoaderKind::Forge => {
            if !forge_meta::is_supported_forge_game(&vanilla.id) {
                return Err(Error::Other(format!(
                    "Forge for Minecraft {} is not supported (Minecraft {} or newer is required)",
                    vanilla.id,
                    forge_meta::OLDEST_FORGE_GAME
                )));
            }
            let sources = forge_meta::forge_installers(&vanilla.id, loader_version);
            forge::install(dirs, &sources, vanilla, java, progress)
        }
    }
}

/// Catch a NeoForge build picked for the wrong Minecraft version early,
/// with a clear message instead of a failing installer.
fn check_neoforge_target(loader_version: &str, game: &str) -> Result<()> {
    match forge_meta::neoforge_minecraft_version(loader_version) {
        Some(target) if target != game => Err(Error::Other(format!(
            "NeoForge {loader_version} is for Minecraft {target}, not {game}"
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_neoforge_for_other_minecraft_versions() {
        assert!(check_neoforge_target("21.1.251", "1.21.1").is_ok());
        let err = check_neoforge_target("21.1.251", "1.20.4").unwrap_err();
        assert!(err.to_string().contains("for Minecraft 1.21.1"));
    }

    #[test]
    fn rejects_bad_versions_before_any_network_access() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let vanilla: VersionJson =
            serde_json::from_str(r#"{"id":"1.6.4","mainClass":"net.minecraft.client.main.Main"}"#)
                .unwrap();
        let java = Path::new("java");
        let old = install_profile(
            &dirs,
            LoaderKind::Forge,
            "9.11.1.1345",
            &vanilla,
            java,
            &|_| {},
        );
        assert!(old.unwrap_err().to_string().contains("1.7.10 or newer"));
        let bad = install_profile(&dirs, LoaderKind::Fabric, "../x", &vanilla, java, &|_| {});
        assert!(bad.is_err());
        let empty = install_profile(&dirs, LoaderKind::Quilt, " ", &vanilla, java, &|_| {});
        assert!(empty.is_err());
    }
}
