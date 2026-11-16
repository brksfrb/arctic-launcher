//! Applying an approved diff, all or nothing.
//!
//! 1. Every new mod file is downloaded into a staging folder inside the game
//!    folder and checked (size, SHA-1 and SHA-512) against what the manifest
//!    says; config files are decoded and checked (SHA-256) there too. A bad
//!    download or hash stops here, before anything in the instance changed.
//! 2. The files being replaced or removed are moved into a backup folder, the
//!    staged ones moved into place, `mods.json` written, and the caller's own
//!    step run (saving the instance and link state). Every step is journaled;
//!    any failure undoes the journal in reverse, so the instance is as it was.
//! 3. The backup and staging folders are deleted.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::diff::{ConfigChange, Diff, ModChange};
use super::manifest::{self, LinkedMod};
use super::paths;
use crate::mods::InstalledMod;
use crate::mods::remote::{self, RemoteVersion};
use crate::net::DownloadJob;
use crate::{Error, Result};

/// Mod files only come from Modrinth's CDN.
const MODRINTH_CDN: &str = "https://cdn.modrinth.com/";
/// Work folder inside the game folder (same drive, so moves are renames).
const WORK_DIR: &str = ".arctic-link";

/// One mod file to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub project: String,
    pub version: String,
    pub version_number: String,
    pub url: String,
    /// Name in the mods folder.
    pub file: String,
    pub size: u64,
    pub sha1: String,
    pub sha512: String,
}

/// What an apply did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub configs: usize,
    pub target_changed: bool,
}

/// The mods a diff adds or updates.
pub fn incoming(diff: &Diff) -> Vec<&LinkedMod> {
    diff.mods
        .iter()
        .filter_map(|m| match &m.change {
            ModChange::Add(m) | ModChange::Update { to: m, .. } => Some(m),
            ModChange::Remove { .. } => None,
        })
        .collect()
}

/// Where each incoming mod comes from: Modrinth's file of that exact version with the
/// manifest's SHA-512. A version Modrinth doesn't have, or whose files don't include that
/// one, is an error (nothing is swapped for something else).
pub fn plan_downloads(mods: &[&LinkedMod], versions: &[RemoteVersion]) -> Result<Vec<Download>> {
    mods.iter()
        .map(|m| {
            let version = versions
                .iter()
                .find(|v| v.version_id == m.version)
                .ok_or_else(|| Error::Other(format!("{} is no longer on Modrinth", m.title)))?;
            if version.project_id != m.project {
                return Err(Error::Other(format!(
                    "{}: the version belongs to another project",
                    m.title
                )));
            }
            let file = version.file_with_sha512(&m.sha512).ok_or_else(|| {
                Error::Other(format!(
                    "{}: Modrinth has no file with the shared hash",
                    m.title
                ))
            })?;
            if !file.url.starts_with(MODRINTH_CDN) || !remote::is_jar_name(&file.file_name) {
                return Err(Error::Other(format!(
                    "{}: Modrinth's file can't be used",
                    m.title
                )));
            }
            if file
                .sha1
                .as_deref()
                .is_some_and(|h| !h.eq_ignore_ascii_case(&m.sha1))
            {
                return Err(Error::Checksum(m.title.clone()));
            }
            Ok(Download {
                project: m.project.clone(),
                version: m.version.clone(),
                version_number: file.version_number.clone(),
                url: file.url.clone(),
                file: file.file_name.clone(),
                size: m.size,
                sha1: m.sha1.clone(),
                sha512: m.sha512.clone(),
            })
        })
        .collect()
}

/// [`plan_downloads`] with Modrinth's answer.
pub fn plan_from_modrinth(diff: &Diff) -> Result<Vec<Download>> {
    let mods = incoming(diff);
    if mods.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<&str> = mods.iter().map(|m| m.version.as_str()).collect();
    plan_downloads(&mods, &remote::versions_by_id(&ids)?)
}

/// The instance an apply changes.
pub struct Target<'a> {
    pub game_dir: &'a Path,
    /// Its `mods.json`.
    pub index: &'a Path,
}

/// Fetches download jobs (`net::download_all` in the app; a fake in tests).
pub type Fetch<'a> = &'a dyn Fn(Vec<DownloadJob>) -> Result<()>;

/// Apply the accepted part of `diff` (see [`Diff::selected`]). `downloads` from
/// [`plan_downloads`]; `commit` runs last inside the transaction (save the instance and the
/// link state there): its failure undoes everything too.
pub fn apply(
    target: &Target,
    diff: &Diff,
    downloads: &[Download],
    fetch: Fetch,
    commit: &mut dyn FnMut() -> Result<()>,
) -> Result<Report> {
    apply_with(target, diff, downloads, fetch, commit, &mut |_| Ok(()))
}

/// Only adding files can happen while the game runs from the instance (it holds its jars).
pub fn needs_game_closed(diff: &Diff) -> bool {
    diff.target.is_some()
        || !diff.configs.is_empty()
        || diff
            .mods
            .iter()
            .any(|m| !matches!(m.change, ModChange::Add(_)))
}

pub(super) fn apply_with(
    target: &Target,
    diff: &Diff,
    downloads: &[Download],
    fetch: Fetch,
    commit: &mut dyn FnMut() -> Result<()>,
    step: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<Report> {
    let diff = diff.selected();
    if needs_game_closed(&diff) && crate::sharing::client::in_use(target.game_dir) {
        return Err(Error::Other(
            "Minecraft is running from this instance; close it first".into(),
        ));
    }
    let work = Work::new(target.game_dir)?;
    let result = (|| {
        let staged = stage(&work, &diff, downloads, fetch)?;
        let mut journal = Journal::default();
        match swap(target, &work, &diff, downloads, &staged, &mut journal, step)
            .and_then(|report| commit().map(|()| report))
        {
            Ok(report) => Ok(report),
            Err(e) => {
                journal.undo();
                Err(e)
            }
        }
    })();
    work.clean();
    result
}

/// The staging and backup folders of one apply.
struct Work {
    staging: PathBuf,
    backup: PathBuf,
}

impl Work {
    fn new(game_dir: &Path) -> Result<Self> {
        static RUNS: AtomicU64 = AtomicU64::new(0);
        let n = RUNS.fetch_add(1, Ordering::Relaxed);
        let root = game_dir.join(WORK_DIR);
        let id = format!("{}-{n}", std::process::id());
        let work = Self {
            staging: root.join(format!("staging-{id}")),
            backup: root.join(format!("backup-{id}")),
        };
        for dir in [&work.staging, &work.backup] {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        Ok(work)
    }

    fn clean(&self) {
        let _ = std::fs::remove_dir_all(&self.staging);
        let _ = std::fs::remove_dir_all(&self.backup);
        if let Some(root) = self.staging.parent() {
            // Only when empty (another apply may be using it).
            let _ = std::fs::remove_dir(root);
        }
    }
}

/// Staged files: the mods by their download index, the configs by path.
struct Staged {
    mods: Vec<PathBuf>,
    configs: Vec<(String, PathBuf)>,
}

fn stage(work: &Work, diff: &Diff, downloads: &[Download], fetch: Fetch) -> Result<Staged> {
    let mods_dir = work.staging.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| Error::io(&mods_dir, e))?;
    for m in super::apply::incoming(diff) {
        if !downloads
            .iter()
            .any(|d| d.project == m.project && d.sha512 == m.sha512)
        {
            return Err(Error::Other(format!("{} has no download", m.title)));
        }
    }
    let mut paths = Vec::new();
    let mut jobs = Vec::new();
    for (i, d) in downloads.iter().enumerate() {
        if !remote::is_jar_name(&d.file) || !d.url.starts_with(MODRINTH_CDN) {
            return Err(Error::Other(format!("refused download {:?}", d.file)));
        }
        // Numbered: two mods can't collide on a name in staging.
        let dest = mods_dir.join(format!("{i}.jar"));
        jobs.push(DownloadJob {
            url: d.url.clone(),
            dest: dest.clone(),
            sha1: Some(d.sha1.clone()),
            size: Some(d.size),
            lzma: None,
        });
        paths.push(dest);
    }
    if !jobs.is_empty() {
        fetch(jobs)?;
    }
    for (d, path) in downloads.iter().zip(&paths) {
        let (sha1, sha512, size) = manifest::hash_file(path)?;
        if size != d.size || sha1 != d.sha1 || sha512 != d.sha512 {
            log::warn!("linked: {} failed its hash check", d.file);
            return Err(Error::Checksum(d.file.clone()));
        }
    }
    let mut configs = Vec::new();
    for (i, c) in diff.configs.iter().enumerate() {
        let file = match &c.change {
            ConfigChange::Add(f) | ConfigChange::Change(f) => f,
            ConfigChange::Remove { .. } => continue,
        };
        paths::check_config_path(&file.path)
            .map_err(|why| Error::Other(format!("{}: {}", file.path, why.message())))?;
        let bytes = file.bytes()?;
        let dest = work.staging.join(format!("config-{i}"));
        std::fs::write(&dest, &bytes).map_err(|e| Error::io(&dest, e))?;
        configs.push((file.path.clone(), dest));
    }
    Ok(Staged {
        mods: paths,
        configs,
    })
}

/// Steps done, to undo in reverse.
#[derive(Default)]
struct Journal {
    steps: Vec<Undo>,
}

enum Undo {
    /// Move a backed-up file back.
    Restore { backup: PathBuf, to: PathBuf },
    /// Remove a file that was put in place.
    Remove(PathBuf),
    /// Put `mods.json` back as it was (`None`: there was none).
    Index(PathBuf, Option<Vec<u8>>),
}

impl Journal {
    fn undo(&mut self) {
        while let Some(step) = self.steps.pop() {
            let result = match &step {
                Undo::Restore { backup, to } => std::fs::rename(backup, to),
                Undo::Remove(path) => std::fs::remove_file(path),
                Undo::Index(path, Some(bytes)) => std::fs::write(path, bytes),
                Undo::Index(path, None) => std::fs::remove_file(path),
            };
            if let Err(e) = result {
                log::error!("linked: could not undo a step: {e}");
            }
        }
    }

    /// Move `from` (if it exists) into the backup folder.
    fn back_up(&mut self, from: &Path, work: &Work) -> Result<()> {
        if std::fs::symlink_metadata(from).is_err() {
            return Ok(());
        }
        let backup = work.backup.join(self.steps.len().to_string());
        std::fs::rename(from, &backup).map_err(|e| Error::io(from, e))?;
        self.steps.push(Undo::Restore {
            backup,
            to: from.to_path_buf(),
        });
        Ok(())
    }

    fn place(&mut self, staged: &Path, to: &Path) -> Result<()> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        std::fs::rename(staged, to).map_err(|e| Error::io(to, e))?;
        self.steps.push(Undo::Remove(to.to_path_buf()));
        Ok(())
    }
}

fn swap(
    target: &Target,
    work: &Work,
    diff: &Diff,
    downloads: &[Download],
    staged: &Staged,
    journal: &mut Journal,
    step: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<Report> {
    let mods_dir = target.game_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| Error::io(&mods_dir, e))?;
    let mut report = Report {
        target_changed: diff.target.is_some(),
        ..Report::default()
    };
    let mut index = remote::index_entries(target.index)?;
    // Old files out first: an update may keep the very same file name.
    for m in &diff.mods {
        if let ModChange::Remove { local, .. } | ModChange::Update { local, .. } = &m.change {
            step(&local.file)?;
            for enabled in [true, false] {
                journal.back_up(&manifest::mod_path(&mods_dir, &local.file, enabled), work)?;
            }
            index.retain(|e| e.project_id != local.project);
            match &m.change {
                ModChange::Remove { .. } => report.removed += 1,
                _ => report.updated += 1,
            }
        }
    }
    for m in &diff.mods {
        let (wanted, enabled) = match &m.change {
            ModChange::Add(w) => (w, w.enabled),
            ModChange::Update { local, to } => (to, local.enabled),
            ModChange::Remove { .. } => continue,
        };
        let i = downloads
            .iter()
            .position(|d| d.project == wanted.project && d.sha512 == wanted.sha512)
            .ok_or_else(|| Error::Other(format!("{} has no download", wanted.title)))?;
        let d = &downloads[i];
        step(&d.file)?;
        // A file of the same name (added by hand, say) makes way, into the backup.
        for on in [true, false] {
            journal.back_up(&manifest::mod_path(&mods_dir, &d.file, on), work)?;
        }
        journal.place(
            &staged.mods[i],
            &manifest::mod_path(&mods_dir, &d.file, enabled),
        )?;
        index.retain(|e| e.project_id != d.project && e.file_name != d.file);
        index.push(InstalledMod {
            project_id: d.project.clone(),
            version_id: d.version.clone(),
            title: wanted.title.clone(),
            version_number: d.version_number.clone(),
            file_name: d.file.clone(),
            icon_url: None,
            dependency: wanted.dependency,
        });
        if matches!(m.change, ModChange::Add(_)) {
            report.added += 1;
        }
    }
    for c in &diff.configs {
        let path = paths::resolve(target.game_dir, c.change.path())?;
        step(c.change.path())?;
        journal.back_up(&path, work)?;
        if let Some((_, file)) = staged.configs.iter().find(|(p, _)| p == c.change.path()) {
            journal.place(file, &path)?;
        }
        report.configs += 1;
    }
    step("mods.json")?;
    // The index and the copy of it that saving keeps (`mods.json.bak`) both go back as they were.
    let mut backup_copy = target.index.as_os_str().to_os_string();
    backup_copy.push(".bak");
    for path in [PathBuf::from(backup_copy), target.index.to_path_buf()] {
        let before = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(Error::io(path, e)),
        };
        journal.steps.push(Undo::Index(path, before));
    }
    remote::save_index(target.index, index)?;
    step("commit")?;
    Ok(report)
}
