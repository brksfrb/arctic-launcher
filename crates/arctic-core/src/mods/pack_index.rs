//! Modrinth archive identities kept next to the installed files.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::{ProjectType, datapack};
use crate::net::sha1_file;
use crate::storage::{load_json, save_json};
use crate::{Error, Result};

const INDEX: &str = ".arctic-packs.json";
const LOCK: &str = ".arctic-packs.lock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFile {
    pub project_id: String,
    pub file_name: String,
    pub sha1: Option<String>,
}

/// Serialize installs into the same directory, including their index updates.
pub(super) fn lock(dir: &Path) -> Result<fs::File> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let path = dir.join(LOCK);
    let file = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| Error::io(&path, e))?;
    file.lock().map_err(|e| Error::io(&path, e))?;
    Ok(file)
}

/// Record downloaded files while holding the folder's install lock.
pub(super) fn record(dir: &Path, entries: impl IntoIterator<Item = PackFile>) -> Result<()> {
    let path = dir.join(INDEX);
    let mut index: Vec<PackFile> = load_json(&path)?.unwrap_or_default();
    for entry in entries {
        index.retain(|old| old.file_name != entry.file_name);
        index.push(entry);
    }
    save_json(&path, &index)
}

/// A cheap refresh immediately after installation. The slower scan also checks hashes.
pub fn installed_by_name(
    kind: ProjectType,
    game_dir: &Path,
    known: &[PackFile],
) -> HashSet<String> {
    installed(kind, game_dir, known, None)
}

#[derive(Default)]
pub struct PackScan {
    hashes: HashMap<PathBuf, CachedHash>,
}

struct CachedHash {
    size: u64,
    modified: Option<SystemTime>,
    sha1: String,
}

impl PackScan {
    /// Scan on a worker. Reuse hashes of unchanged files across polls.
    pub fn installed(
        &mut self,
        kind: ProjectType,
        game_dir: &Path,
        known: &[PackFile],
    ) -> HashSet<String> {
        installed(kind, game_dir, known, Some(self))
    }

    fn hashed_files(&mut self, files: &HashMap<PathBuf, fs::Metadata>) -> HashSet<&str> {
        self.hashes.retain(|path, _| files.contains_key(path));
        for (path, meta) in files {
            let modified = meta.modified().ok();
            let size = meta.len();
            if modified.is_some()
                && self
                    .hashes
                    .get(path)
                    .is_some_and(|cached| cached.size == size && cached.modified == modified)
            {
                continue;
            }
            self.hashes.remove(path);
            if let Ok(sha1) = sha1_file(path) {
                self.hashes.insert(
                    path.clone(),
                    CachedHash {
                        size,
                        modified,
                        sha1,
                    },
                );
            }
        }
        self.hashes
            .values()
            .map(|cached| cached.sha1.as_str())
            .collect()
    }
}

fn pack_dirs(kind: ProjectType, game_dir: &Path) -> Vec<PathBuf> {
    match kind {
        ProjectType::ResourcePack | ProjectType::Shader => kind
            .folder()
            .map(|dir| game_dir.join(dir))
            .into_iter()
            .collect(),
        ProjectType::DataPack => datapack::worlds(game_dir)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|world| datapack::install_dir(game_dir, &world).ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn installed(
    kind: ProjectType,
    game_dir: &Path,
    known: &[PackFile],
    scan: Option<&mut PackScan>,
) -> HashSet<String> {
    let dirs = pack_dirs(kind, game_dir);
    let mut tracked = Vec::new();
    let mut files = HashMap::new();
    for dir in dirs {
        let path = dir.join(INDEX);
        match load_json::<Vec<PackFile>>(&path) {
            Ok(Some(entries)) => tracked.extend(entries),
            Ok(None) => {}
            Err(e) => log::debug!("ignoring unreadable {}: {e}", path.display()),
        }
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(INDEX)
                && name != LOCK
                && !name.ends_with(".part")
                && let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                files.insert(entry.path(), meta);
            }
        }
    }

    let names: HashSet<_> = files.keys().filter_map(|path| path.file_name()).collect();
    let entries = tracked.iter().chain(known);
    let mut installed: HashSet<_> = entries
        .clone()
        .filter(|entry| names.contains(std::ffi::OsStr::new(&entry.file_name)))
        .map(|entry| entry.project_id.clone())
        .collect();
    if let Some(scan) = scan {
        let unmatched: Vec<_> = entries
            .filter(|entry| !installed.contains(&entry.project_id) && entry.sha1.is_some())
            .collect();
        if !unmatched.is_empty() {
            let hashes = scan.hashed_files(&files);
            installed.extend(unmatched.into_iter().filter_map(|entry| {
                let hash = entry.sha1.as_deref()?.to_ascii_lowercase();
                hashes
                    .contains(hash.as_str())
                    .then(|| entry.project_id.clone())
            }));
        }
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK_SHA1: &str = "11f6ad8ec52a2984abaafd7c3b516503785c2072";

    fn file(project_id: &str, file_name: &str) -> PackFile {
        PackFile {
            project_id: project_id.into(),
            file_name: file_name.into(),
            sha1: Some(PACK_SHA1.into()),
        }
    }

    #[test]
    fn tracked_pack_is_visible_by_name_without_hashing() {
        let game = tempfile::tempdir().unwrap();
        for kind in [ProjectType::ResourcePack, ProjectType::Shader] {
            let dir = game.path().join(kind.folder().unwrap());
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("pack.zip"), b"x").unwrap();
            record(&dir, [file(kind.id(), "pack.zip")]).unwrap();
            assert_eq!(
                installed_by_name(kind, game.path(), &[]),
                HashSet::from([kind.id().to_owned()])
            );
        }
    }

    #[test]
    fn stale_filename_matches_hash_until_that_file_is_deleted() {
        let game = tempfile::tempdir().unwrap();
        let dir = game.path().join("resourcepacks");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("original.zip"), b"x").unwrap();
        record(&dir, [file("tracked", "original.zip")]).unwrap();
        fs::rename(dir.join("original.zip"), dir.join("renamed.zip")).unwrap();
        fs::write(dir.join("other.zip"), b"other").unwrap();
        let kind = ProjectType::ResourcePack;
        let mut scan = PackScan::default();
        assert!(installed_by_name(kind, game.path(), &[]).is_empty());
        assert_eq!(
            scan.installed(kind, game.path(), &[]),
            HashSet::from(["tracked".into()])
        );
        fs::remove_file(dir.join("renamed.zip")).unwrap();
        assert!(scan.installed(kind, game.path(), &[]).is_empty());
    }

    #[test]
    fn untracked_packs_match_known_names_or_hashes() {
        let game = tempfile::tempdir().unwrap();
        let dir = game.path().join("shaderpacks");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("manual.zip"), b"manual").unwrap();
        fs::write(dir.join("renamed.zip"), b"x").unwrap();
        let kind = ProjectType::Shader;
        let known = [
            file("name-match", "manual.zip"),
            file("hash-match", "old.zip"),
        ];
        assert_eq!(
            installed_by_name(kind, game.path(), &known),
            HashSet::from(["name-match".into()])
        );
        assert_eq!(
            PackScan::default().installed(kind, game.path(), &known),
            HashSet::from(["name-match".into(), "hash-match".into()])
        );
    }

    #[test]
    fn data_packs_count_in_any_valid_world() {
        let game = tempfile::tempdir().unwrap();
        let first = game.path().join("saves/First/datapacks");
        let second = game.path().join("saves/Second/datapacks");
        let invalid = game.path().join("saves/Not a world/datapacks");
        for dir in [&first, &second, &invalid] {
            fs::create_dir_all(dir).unwrap();
        }
        for dir in [&first, &second] {
            fs::write(dir.parent().unwrap().join("level.dat"), []).unwrap();
        }
        fs::write(second.join("actual.zip"), b"x").unwrap();
        fs::write(invalid.join("fake.zip"), b"fake").unwrap();
        record(&second, [file("data-pack", "actual.zip")]).unwrap();
        record(&invalid, [file("fake-pack", "fake.zip")]).unwrap();
        assert_eq!(
            installed_by_name(ProjectType::DataPack, game.path(), &[]),
            HashSet::from(["data-pack".into()])
        );
    }
}
