//! Install exact Modrinth versions (a friend's shared mod list), so both
//! sides end up with the very same files. Nothing is re-resolved: a version
//! that's gone or doesn't fit is reported, not swapped for another.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::index::ModIndex;
use super::modrinth::{self, Version};
use super::{InstalledMod, files, prepare_one, record, with_project_info};
use crate::loaders::LoaderKind;
use crate::net::download_all;
use crate::{Progress, ProgressInfo, Result};

/// Versions per Modrinth request (keeps the URL short).
const BATCH: usize = 100;

/// One mod to install, exactly as it was on the other side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub project_id: String,
    pub version_id: String,
    /// For messages only.
    pub title: String,
    pub enabled: bool,
    pub dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExactReport {
    pub installed: Vec<InstalledMod>,
    pub skipped: Vec<Skipped>,
}

/// Download `wanted` into `mods_dir` and track them in `index`.
pub fn install_exact(
    wanted: &[Wanted],
    game_version: &str,
    loader: LoaderKind,
    mods_dir: &Path,
    index: &Path,
    progress: Progress,
) -> Result<ExactReport> {
    progress(ProgressInfo::stage("Looking up mods"));
    let mut found: HashMap<String, Version> = HashMap::new();
    for chunk in wanted.chunks(BATCH) {
        let ids: Vec<&str> = chunk.iter().map(|w| w.version_id.as_str()).collect();
        for v in modrinth::versions(&ids)? {
            found.insert(v.id.clone(), v);
        }
    }
    let loaders = modrinth::compatible_loaders(loader);
    let mut report = ExactReport::default();
    let mut plan = Vec::new();
    let mut seen = HashSet::new();
    for w in wanted {
        match check(w, found.get(&w.version_id), &loaders, game_version) {
            Err(reason) => report.skipped.push(Skipped {
                title: w.title.clone(),
                reason,
            }),
            Ok(_) if !seen.insert(w.project_id.clone()) => {}
            Ok(version) => plan.push((prepare_one(version, w.dependency, mods_dir)?, w.enabled)),
        }
    }
    if plan.is_empty() {
        return Ok(report);
    }
    std::fs::create_dir_all(mods_dir).map_err(|e| crate::Error::io(mods_dir, e))?;
    let (entries, jobs): (Vec<_>, Vec<_>) = plan
        .iter()
        .map(|((m, j), _)| (m.clone(), j.clone()))
        .unzip();
    let entries = with_project_info(entries);
    download_all("Downloading mods", jobs, progress)?;
    let current = ModIndex::load(index)?;
    record(&current, &entries, mods_dir)?.save(index)?;
    for ((m, _), enabled) in &plan {
        if !enabled {
            files::set_enabled(mods_dir, &m.file_name, false)?;
        }
    }
    report.installed = entries;
    Ok(report)
}

/// The version to install, or why not.
fn check<'a>(
    w: &Wanted,
    version: Option<&'a Version>,
    loaders: &[&str],
    game_version: &str,
) -> std::result::Result<&'a Version, String> {
    let version = version.ok_or("no longer on Modrinth")?;
    if version.project_id != w.project_id {
        return Err("the share names the wrong mod for this version".into());
    }
    if !version.is_compatible(loaders, game_version) {
        return Err(format!("not made for {game_version} with this loader"));
    }
    let file = version.primary_file().ok_or("has no file to download")?;
    files::check_file_name(&file.filename).map_err(|e| e.to_string())?;
    Ok(version)
}
