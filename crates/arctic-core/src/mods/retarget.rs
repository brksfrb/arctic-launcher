//! Moving an instance's mods to another Minecraft version (or loader): which of them exist there,
//! and then switching them over.
//!
//! [`check`] asks Modrinth about every mod (files added by hand are first recognized by their
//! hash) and nothing changes on disk except the index learning those files. [`apply`] installs
//! each mod's newest version for the new target (with any new required dependencies), turns off
//! (never deletes) the mods that have none yet, so the game still starts, and leaves files
//! Modrinth doesn't know as they are.

use std::collections::HashSet;
use std::path::Path;

use super::index::ModIndex;
use super::{Failed, InstalledMod, files, identify, modrinth};
use crate::loaders::LoaderKind;
use crate::{Progress, ProgressInfo, Result};

/// One mod and what it becomes on the new version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModCheck {
    /// The file in the mods folder (without `.disabled`).
    pub file_name: String,
    pub title: String,
    /// Modrinth project; None for a file Modrinth doesn't know.
    pub project_id: Option<String>,
    /// The version installed now.
    pub current: Option<String>,
    /// The version it gets on the new target; None when there is none (yet).
    pub available: Option<String>,
    /// Pulled in for another mod rather than chosen.
    pub dependency: bool,
    pub enabled: bool,
}

impl ModCheck {
    /// Modrinth knows this mod but it has no version for the new target.
    pub fn missing(&self) -> bool {
        self.project_id.is_some() && self.available.is_none()
    }
}

/// What every mod in `mods_dir` becomes on `game_version` with `loader`.
pub fn check(
    mods_dir: &Path,
    index: &Path,
    game_version: &str,
    loader: LoaderKind,
    progress: Progress,
) -> Result<Vec<ModCheck>> {
    progress(ProgressInfo::stage("Recognizing mods"));
    // Files from elsewhere become tracked if Modrinth knows them (any version).
    identify::recognize(mods_dir, index, game_version, loader)?;
    let tracked = ModIndex::load(index)?;
    let listed = files::list(mods_dir, &tracked)?;
    progress(ProgressInfo::stage("Looking for new versions"));
    let loaders = modrinth::compatible_loaders(loader);
    let found: Vec<Result<Option<String>>> = std::thread::scope(|s| {
        let handles: Vec<_> = listed
            .iter()
            .map(|file| {
                let loaders = &loaders;
                s.spawn(move || match &file.tracked {
                    Some(m) => newest(&m.project_id, loaders, game_version),
                    None => Ok(None),
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(crate::Error::Other("lookup panicked".into())))
            })
            .collect()
    });
    let mut checks = Vec::with_capacity(listed.len());
    for (file, available) in listed.into_iter().zip(found) {
        let available = available?;
        checks.push(match file.tracked {
            Some(m) => ModCheck {
                file_name: file.file_name,
                title: m.title,
                project_id: Some(m.project_id),
                current: Some(m.version_number),
                available,
                dependency: m.dependency,
                enabled: file.enabled,
            },
            None => ModCheck {
                title: file.file_name.clone(),
                file_name: file.file_name,
                project_id: None,
                current: None,
                available: None,
                dependency: false,
                enabled: file.enabled,
            },
        });
    }
    // Chosen mods first, missing ones on top, then by name.
    checks.sort_by(|a, b| {
        (a.dependency, !a.missing(), a.title.to_lowercase()).cmp(&(
            b.dependency,
            !b.missing(),
            b.title.to_lowercase(),
        ))
    });
    Ok(checks)
}

/// The newest version number of `project_id` for the target, None when it has none.
fn newest(project_id: &str, loaders: &[&str], game_version: &str) -> Result<Option<String>> {
    let versions = modrinth::project_versions(project_id, loaders, game_version)?;
    Ok(modrinth::pick_version(&versions, loaders, game_version).map(|v| v.version_number.clone()))
}

/// What [`apply`] did.
#[derive(Debug, Default)]
pub struct Applied {
    pub installed: Vec<InstalledMod>,
    /// Mods turned off: no version for the new target.
    pub disabled: Vec<String>,
    pub failed: Vec<Failed>,
}

/// Switch the mods in `checks` (from [`check`] for the same target) over to it.
pub fn apply(
    mods_dir: &Path,
    index: &Path,
    checks: &[ModCheck],
    game_version: &str,
    loader: LoaderKind,
    progress: Progress,
) -> Result<Applied> {
    let mut applied = Applied::default();
    let projects: Vec<&str> = checks
        .iter()
        .filter(|c| c.available.is_some())
        .filter_map(|c| c.project_id.as_deref())
        .collect();
    let dependencies: HashSet<String> = checks
        .iter()
        .filter(|c| c.dependency)
        .filter_map(|c| c.project_id.clone())
        .collect();
    if !projects.is_empty() {
        let (installed, failed) =
            super::install_many(&projects, game_version, loader, mods_dir, index, progress)?;
        applied.installed = installed;
        applied.failed = failed;
        // Installed side by side they all count as chosen: keep what were dependencies so.
        let mut updated = ModIndex::load(index)?;
        for m in updated.mods.clone() {
            if dependencies.contains(&m.project_id) && !m.dependency {
                updated = updated.with(InstalledMod {
                    dependency: true,
                    ..m
                });
            }
        }
        updated.save(index)?;
    }
    // Mods with no version there, or whose download failed, would stop the game from starting.
    let failed: HashSet<&str> = applied.failed.iter().map(|(p, _)| p.as_str()).collect();
    for c in checks.iter().filter(|c| c.enabled) {
        let failed = c.project_id.as_deref().is_some_and(|p| failed.contains(p));
        if c.missing() || failed {
            super::set_enabled(mods_dir, &c.file_name, false)?;
            applied.disabled.push(c.title.clone());
        }
    }
    Ok(applied)
}
