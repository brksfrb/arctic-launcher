//! Mods: browsing and installing from Modrinth, and managing an instance's
//! `mods/` folder.
//!
//! Installed Modrinth mods are tracked in `<instance>/mods.json` so the UI
//! can show titles, icons and versions; jars dropped in by hand are listed
//! too (untracked). Disabled mods are renamed to `*.jar.disabled`, which is
//! what loaders ignore.

mod files;
mod index;
pub mod modpack;
mod modrinth;
pub mod performance;
mod resolve;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::loaders::LoaderKind;
use crate::net::{DownloadJob, download_all};
use crate::{Error, Progress, ProgressInfo, Result};

use index::ModIndex;
use modrinth::{Project, Version};
use resolve::{Planned, Target};

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
    /// Search modpacks instead of mods.
    pub modpacks: bool,
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
///
/// `limit` is clamped to 1..=100 (0 means the default of 20). An empty
/// `game_version` searches all versions; `loader` filters by loader (Quilt
/// also matches Fabric mods).
pub fn search(query: &SearchQuery) -> Result<SearchPage> {
    modrinth::search(query)
}

/// Install the newest version of `project_id` compatible with the instance
/// (and its required dependencies) into `mods_dir`. Returns what was
/// installed. `index` is the instance's `mods.json`.
///
/// `project_id` may also be a slug. Dependencies whose project is already
/// installed are left alone; the requested project itself is always
/// (re)installed, replacing any other version of it. Everything is resolved
/// before anything is downloaded, so a missing dependency changes nothing.
pub fn install(
    project_id: &str,
    game_version: &str,
    loader: LoaderKind,
    mods_dir: &Path,
    index: &Path,
    progress: Progress,
) -> Result<Vec<InstalledMod>> {
    progress(ProgressInfo::stage("Resolving dependencies"));
    let loaders = modrinth::compatible_loaders(loader);
    let target = Target {
        loaders: &loaders,
        game_version,
        loader,
    };
    let current = ModIndex::load(index)?;
    let installed = installed_projects(&current, mods_dir);
    let source = resolve::Modrinth {
        loaders: &loaders,
        game_version,
    };
    let plan = resolve::resolve(project_id, target, &installed, &source)?;
    let (mods, jobs) = prepare(&plan, mods_dir)?;
    let mods = with_project_info(mods);

    download_all("Downloading mods", jobs, progress)?;
    let updated = record(&current, &mods, mods_dir)?;
    updated.save(index)?;
    Ok(mods)
}

/// Jars in `mods_dir`, joined with the tracking info from `index`.
///
/// Includes disabled (`*.jar.disabled`) files; a missing folder is empty.
/// Sorted by title (file name for untracked jars), case-insensitively.
pub fn list(mods_dir: &Path, index: &Path) -> Result<Vec<ModFile>> {
    let tracked = ModIndex::load(index).unwrap_or_else(|e| {
        log::warn!("ignoring unreadable {}: {e}", index.display());
        ModIndex::default()
    });
    files::list(mods_dir, &tracked)
}

/// Enable or disable a mod (renames to/from `.jar.disabled`).
///
/// `file_name` may be given with or without the `.disabled` suffix; asking
/// for the state a mod is already in succeeds without doing anything.
pub fn set_enabled(mods_dir: &Path, file_name: &str, enabled: bool) -> Result<()> {
    files::set_enabled(mods_dir, file_name, enabled)
}

/// Delete a mod file and forget it in `index`.
///
/// Deletes both the enabled and the disabled form; a file that is already
/// gone is not an error.
pub fn remove(mods_dir: &Path, index: &Path, file_name: &str) -> Result<()> {
    files::delete(mods_dir, file_name)?;
    let base = files::base_name(file_name);
    let current = ModIndex::load(index)?;
    if current.by_file(base).is_some() {
        current.without_file(base).save(index)?;
    }
    Ok(())
}

/// Icon bytes for a project (cached under `cache_dir`).
///
/// Cached by the SHA-1 of the URL, so repeated calls never hit the network.
/// Icons over 2 MB are rejected.
pub fn icon(url: &str, cache_dir: &Path) -> Result<Vec<u8>> {
    files::icon(url, cache_dir)
}

/// Path of an instance's mod index.
pub fn index_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join("mods.json")
}

// ---------------------------------------------------------------- helpers

/// Projects in the index whose file is still in the mods folder.
fn installed_projects(index: &ModIndex, mods_dir: &Path) -> HashSet<String> {
    index
        .mods
        .iter()
        .filter(|m| files::exists(mods_dir, &m.file_name))
        .map(|m| m.project_id.clone())
        .collect()
}

/// Index entries (titles still unknown) and download jobs for a plan.
fn prepare(plan: &[Planned], mods_dir: &Path) -> Result<(Vec<InstalledMod>, Vec<DownloadJob>)> {
    plan.iter()
        .map(|p| prepare_one(&p.version, p.dependency, mods_dir))
        .collect::<Result<Vec<_>>>()
        .map(|pairs| pairs.into_iter().unzip())
}

fn prepare_one(
    version: &Version,
    dependency: bool,
    mods_dir: &Path,
) -> Result<(InstalledMod, DownloadJob)> {
    let file = version
        .primary_file()
        .ok_or_else(|| Error::Other(format!("version {} has no files", version.id)))?;
    files::check_file_name(&file.filename)?;
    let entry = InstalledMod {
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
        title: version.project_id.clone(),
        version_number: version.version_number.clone(),
        file_name: file.filename.clone(),
        icon_url: None,
        dependency,
    };
    let job = DownloadJob {
        url: file.url.clone(),
        dest: mods_dir.join(&file.filename),
        sha1: file.hashes.sha1.clone(),
        size: file.size,
        lzma: None,
    };
    Ok((entry, job))
}

/// Fill in titles and icons with one batch request. Cosmetic: on failure
/// the project id stays as the title.
fn with_project_info(mods: Vec<InstalledMod>) -> Vec<InstalledMod> {
    let ids: Vec<&str> = mods.iter().map(|m| m.project_id.as_str()).collect();
    match modrinth::projects(&ids) {
        Ok(projects) => apply_project_info(mods, &projects),
        Err(e) => {
            log::warn!("could not fetch mod titles: {e}");
            mods
        }
    }
}

fn apply_project_info(mods: Vec<InstalledMod>, projects: &[Project]) -> Vec<InstalledMod> {
    let by_id: HashMap<&str, &Project> = projects.iter().map(|p| (p.id.as_str(), p)).collect();
    mods.into_iter()
        .map(|m| match by_id.get(m.project_id.as_str()) {
            Some(p) if !p.title.trim().is_empty() => InstalledMod {
                title: p.title.clone(),
                icon_url: modrinth::non_empty(p.icon_url.clone()),
                ..m
            },
            _ => m,
        })
        .collect()
}

/// The index after installing `mods`. Older files of the same projects are
/// deleted, as is a stale disabled copy of the very same file.
fn record(current: &ModIndex, mods: &[InstalledMod], mods_dir: &Path) -> Result<ModIndex> {
    let mut updated = current.clone();
    for m in mods {
        if let Some(old) = current.by_project(&m.project_id) {
            if old.file_name == m.file_name {
                files::delete_disabled_copy(mods_dir, &m.file_name)?;
            } else {
                files::delete(mods_dir, &old.file_name)?;
            }
        }
        updated = updated.with(m.clone());
    }
    Ok(updated)
}
