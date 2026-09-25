//! Mods: browsing and installing from Modrinth, and managing an instance's
//! `mods/` folder.
//!
//! Installed Modrinth mods are tracked in `<instance>/mods.json` so the UI
//! can show titles, icons and versions; jars dropped in by hand are listed
//! too (untracked). Disabled mods are renamed to `*.jar.disabled`, which is
//! what loaders ignore.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::loaders::LoaderKind;
use crate::{Error, Progress, Result};

/// How search results are ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: String,
    pub game_version: String,
    pub loader: Option<LoaderKind>,
    pub sort: SortBy,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchPage {
    pub hits: Vec<ProjectHit>,
    pub total: usize,
}

/// A Modrinth mod installed into an instance (from `mods.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledMod {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub file_name: String,
    pub icon_url: Option<String>,
    /// Installed only because another mod requires it.
    #[serde(default)]
    pub dependency: bool,
}

/// A jar in the mods folder, tracked (installed via Arctic) or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModFile {
    /// File name without the `.disabled` suffix.
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub tracked: Option<InstalledMod>,
}

/// Search Modrinth for mods.
pub fn search(query: &SearchQuery) -> Result<SearchPage> {
    let _ = query;
    Err(Error::Other("mod search is not available yet".into()))
}

/// Install the newest version of `project_id` compatible with the instance
/// (and its required dependencies) into `mods_dir`. Returns what was
/// installed. `index` is the instance's `mods.json`.
pub fn install(
    project_id: &str,
    game_version: &str,
    loader: LoaderKind,
    mods_dir: &Path,
    index: &Path,
    progress: Progress,
) -> Result<Vec<InstalledMod>> {
    let _ = (project_id, game_version, loader, mods_dir, index, progress);
    Err(Error::Other("mod install is not available yet".into()))
}

/// Jars in `mods_dir`, joined with the tracking info from `index`.
pub fn list(mods_dir: &Path, index: &Path) -> Result<Vec<ModFile>> {
    let _ = (mods_dir, index);
    Ok(Vec::new())
}

/// Enable or disable a mod (renames to/from `.jar.disabled`).
pub fn set_enabled(mods_dir: &Path, file_name: &str, enabled: bool) -> Result<()> {
    let _ = (mods_dir, file_name, enabled);
    Err(Error::Other("not available yet".into()))
}

/// Delete a mod file and forget it in `index`.
pub fn remove(mods_dir: &Path, index: &Path, file_name: &str) -> Result<()> {
    let _ = (mods_dir, index, file_name);
    Err(Error::Other("not available yet".into()))
}

/// Icon bytes for a project (cached under `cache_dir`).
pub fn icon(url: &str, cache_dir: &Path) -> Result<Vec<u8>> {
    let _ = (url, cache_dir);
    Err(Error::Other("not available yet".into()))
}

/// Path of an instance's mod index.
pub fn index_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join("mods.json")
}
