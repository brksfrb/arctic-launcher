//! Content discovery: Modrinth's filter lists (categories, game versions, loaders), a project's
//! version list, where each kind of project installs, and what shaders need to run.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::modrinth::{self, Version};
use super::packs::{OCULUS, PackKind};
use super::{ProjectType, packs};
use crate::Result;
use crate::loaders::LoaderKind;

// ------------------------------------------------------------------ tags

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Category {
    pub name: String,
    #[serde(default)]
    pub project_type: String,
    /// "categories", "features", "resolutions" or "performance impact".
    #[serde(default)]
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GameVersionTag {
    pub version: String,
    /// "release", "snapshot", "alpha" or "beta".
    #[serde(default)]
    pub version_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoaderTag {
    pub name: String,
    #[serde(default)]
    pub supported_project_types: Vec<String>,
}

/// What the filters can offer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub categories: Vec<Category>,
    pub game_versions: Vec<GameVersionTag>,
    pub loaders: Vec<LoaderTag>,
}

/// The three tag lists, newest game versions first (as Modrinth answers).
pub fn tags() -> Result<Tags> {
    let api = modrinth::API;
    Ok(Tags {
        categories: modrinth::get_api(&format!("{api}/tag/category"), "categories")?,
        game_versions: modrinth::get_api(&format!("{api}/tag/game_version"), "game versions")?,
        loaders: modrinth::get_api(&format!("{api}/tag/loader"), "loaders")?,
    })
}

impl Tags {
    /// Category names that apply to `kind`, in Modrinth's order, without repeats (the
    /// "categories" header only: features and resolutions are other kinds of filter).
    pub fn categories_for(&self, kind: ProjectType) -> Vec<&str> {
        let mut seen = HashSet::new();
        self.categories
            .iter()
            .filter(|c| c.project_type == kind.id() && c.header == "categories")
            .map(|c| c.name.as_str())
            .filter(|n| seen.insert(*n))
            .collect()
    }

    /// Loaders that apply to `kind`.
    pub fn loaders_for(&self, kind: ProjectType) -> Vec<&str> {
        self.loaders
            .iter()
            .filter(|l| l.supported_project_types.iter().any(|t| t == kind.id()))
            .map(|l| l.name.as_str())
            .collect()
    }

    /// Release game versions, newest first.
    pub fn releases(&self) -> Vec<&str> {
        self.game_versions
            .iter()
            .filter(|v| v.version_type == "release")
            .map(|v| v.version.as_str())
            .collect()
    }
}

/// "performance" → "Performance", "world-generation" → "World generation".
pub fn pretty_name(id: &str) -> String {
    let spaced = id.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// -------------------------------------------------------------- versions

/// One version of a project, as the project page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub id: String,
    pub number: String,
    pub name: String,
    /// "release", "beta" or "alpha".
    pub version_type: String,
    pub date: String,
    pub downloads: u64,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

impl From<&Version> for VersionInfo {
    fn from(v: &Version) -> Self {
        Self {
            id: v.id.clone(),
            number: v.version_number.clone(),
            name: v.name.clone(),
            version_type: v.version_type.clone(),
            date: v.date_published.clone(),
            downloads: v.downloads,
            game_versions: v.game_versions.clone(),
            loaders: v.loaders.clone(),
        }
    }
}

impl VersionInfo {
    /// Would this version install into an instance on `game` with `loader` (`None`: vanilla)?
    /// Packs and shaders are matched on their own loader names instead.
    pub fn fits(&self, kind: ProjectType, loader: Option<LoaderKind>, game: &str) -> bool {
        let game_ok = game.trim().is_empty() || self.game_versions.iter().any(|g| g == game);
        let loader_ok = match kind {
            ProjectType::Mod | ProjectType::Modpack => match loader {
                Some(l) => {
                    let wanted = modrinth::compatible_loaders(l, game);
                    self.loaders.iter().any(|x| wanted.contains(&x.as_str()))
                }
                None => kind == ProjectType::Modpack,
            },
            ProjectType::ResourcePack => self.loaders.iter().any(|l| l == "minecraft"),
            ProjectType::Shader => self
                .loaders
                .iter()
                .any(|l| PackKind::Shader.loaders().contains(&l.as_str())),
        };
        game_ok && loader_ok
    }

    /// What a shader version asks of the player, from the loaders Modrinth tags it with.
    pub fn shader_requirement(&self) -> Option<ShaderRequirement> {
        let has = |name: &str| self.loaders.iter().any(|l| l == name);
        match (has("iris"), has("optifine")) {
            (true, _) => Some(ShaderRequirement::Iris),
            (false, true) => Some(ShaderRequirement::OptiFineFormat),
            _ => None,
        }
    }
}

/// How a shader pack is meant to be run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderRequirement {
    /// Tagged for Iris (and so Oculus on Forge).
    Iris,
    /// Tagged for OptiFine only; Iris reads that format too.
    OptiFineFormat,
}

/// Every version of a project, newest first, away from the UI thread.
pub fn project_version_list(project_id: &str) -> Result<Vec<VersionInfo>> {
    let url = format!(
        "{}/project/{}/version",
        modrinth::API,
        modrinth::encode(project_id)
    );
    let versions: Vec<Version> = modrinth::get_api(&url, &format!("project {project_id}"))?;
    Ok(versions.iter().map(VersionInfo::from).collect())
}

// ---------------------------------------------------------------- routing

/// Where an installed project of `kind` goes in an instance's game folder; `None` for
/// modpacks, which become instances of their own.
pub fn install_dir(kind: ProjectType, game_dir: &Path) -> Option<PathBuf> {
    kind.folder().map(|f| game_dir.join(f))
}

// ----------------------------------------------------------- shader loader

/// What an instance needs before shaders do anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderLoader {
    /// The Vanilla instance has its own shader switch (Iris comes with it).
    Switch,
    /// Iris/Oculus is installed.
    Present,
    /// The mod is missing: installing a shader pack adds it.
    Missing {
        project_id: &'static str,
        name: &'static str,
    },
}

/// `loader` is the instance's (`None`: Vanilla); `installed` its tracked mod project ids.
pub fn shader_loader(loader: Option<LoaderKind>, installed: &HashSet<String>) -> ShaderLoader {
    let Some(loader) = loader else {
        return ShaderLoader::Switch;
    };
    let id = packs::shader_mod(loader);
    if installed.contains(id) {
        ShaderLoader::Present
    } else {
        ShaderLoader::Missing {
            project_id: id,
            name: if id == OCULUS { "Oculus" } else { "Iris" },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::packs::IRIS;

    fn info(game: &[&str], loaders: &[&str]) -> VersionInfo {
        VersionInfo {
            id: "v".into(),
            number: "1".into(),
            name: String::new(),
            version_type: "release".into(),
            date: String::new(),
            downloads: 0,
            game_versions: game.iter().map(|s| s.to_string()).collect(),
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn install_dir_routes_by_project_type() {
        let game = Path::new("/g");
        assert_eq!(install_dir(ProjectType::Mod, game), Some(game.join("mods")));
        assert_eq!(
            install_dir(ProjectType::ResourcePack, game),
            Some(game.join("resourcepacks"))
        );
        assert_eq!(
            install_dir(ProjectType::Shader, game),
            Some(game.join("shaderpacks"))
        );
        assert_eq!(install_dir(ProjectType::Modpack, game), None);
        // The pack installer uses the very same folders.
        assert_eq!(PackKind::Resource.folder(), "resourcepacks");
        assert_eq!(PackKind::Shader.folder(), "shaderpacks");
    }

    #[test]
    fn mods_fit_on_game_and_loader_with_quilt_taking_fabric() {
        let v = info(&["1.21.1"], &["fabric"]);
        assert!(v.fits(ProjectType::Mod, Some(LoaderKind::Fabric), "1.21.1"));
        assert!(v.fits(ProjectType::Mod, Some(LoaderKind::Quilt), "1.21.1"));
        assert!(!v.fits(ProjectType::Mod, Some(LoaderKind::Forge), "1.21.1"));
        assert!(!v.fits(ProjectType::Mod, Some(LoaderKind::Fabric), "1.20.1"));
        assert!(!v.fits(ProjectType::Mod, None, "1.21.1"));
    }

    #[test]
    fn packs_and_shaders_fit_on_their_own_loaders() {
        assert!(info(&["1.21.1"], &["minecraft"]).fits(
            ProjectType::ResourcePack,
            Some(LoaderKind::Fabric),
            "1.21.1"
        ));
        let optifine = info(&["1.21.1"], &["optifine"]);
        assert!(optifine.fits(ProjectType::Shader, None, "1.21.1"));
        assert!(!optifine.fits(ProjectType::ResourcePack, None, "1.21.1"));
        assert!(!info(&["1.21.1"], &["canvas"]).fits(ProjectType::Shader, None, "1.21.1"));
        assert!(!optifine.fits(ProjectType::Shader, None, "1.8.9"));
    }

    #[test]
    fn shader_requirement_follows_the_loader_tags() {
        assert_eq!(
            info(&[], &["iris", "optifine"]).shader_requirement(),
            Some(ShaderRequirement::Iris)
        );
        assert_eq!(
            info(&[], &["optifine"]).shader_requirement(),
            Some(ShaderRequirement::OptiFineFormat)
        );
        assert_eq!(info(&[], &["canvas"]).shader_requirement(), None);
    }

    #[test]
    fn shader_loader_warns_when_the_mod_is_missing() {
        let none = HashSet::new();
        assert_eq!(shader_loader(None, &none), ShaderLoader::Switch);
        assert_eq!(
            shader_loader(Some(LoaderKind::Fabric), &none),
            ShaderLoader::Missing {
                project_id: IRIS,
                name: "Iris"
            }
        );
        assert!(matches!(
            shader_loader(Some(LoaderKind::Forge), &none),
            ShaderLoader::Missing { name: "Oculus", .. }
        ));
        let have: HashSet<String> = [IRIS.to_owned()].into();
        assert_eq!(
            shader_loader(Some(LoaderKind::NeoForge), &have),
            ShaderLoader::Present
        );
    }

    #[test]
    fn tags_filter_by_project_type_and_header() {
        let tags = Tags {
            categories: vec![
                Category {
                    name: "optimization".into(),
                    project_type: "mod".into(),
                    header: "categories".into(),
                },
                Category {
                    name: "client".into(),
                    project_type: "mod".into(),
                    header: "environments".into(),
                },
                Category {
                    name: "realistic".into(),
                    project_type: "shader".into(),
                    header: "categories".into(),
                },
            ],
            game_versions: vec![
                GameVersionTag {
                    version: "26w01a".into(),
                    version_type: "snapshot".into(),
                },
                GameVersionTag {
                    version: "1.21.1".into(),
                    version_type: "release".into(),
                },
            ],
            loaders: vec![
                LoaderTag {
                    name: "fabric".into(),
                    supported_project_types: vec!["mod".into(), "modpack".into()],
                },
                LoaderTag {
                    name: "iris".into(),
                    supported_project_types: vec!["shader".into()],
                },
            ],
        };
        assert_eq!(tags.categories_for(ProjectType::Mod), ["optimization"]);
        assert_eq!(tags.categories_for(ProjectType::Shader), ["realistic"]);
        assert_eq!(tags.loaders_for(ProjectType::Shader), ["iris"]);
        assert_eq!(tags.loaders_for(ProjectType::Modpack), ["fabric"]);
        assert_eq!(tags.releases(), ["1.21.1"]);
        assert_eq!(pretty_name("world-generation"), "World generation");
    }
}
