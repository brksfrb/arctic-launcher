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
    matches_by_name(kind, game_dir, known).into_keys().collect()
}

/// A project can be installed even when a filename matches more than one project.
pub type PackMatches = HashMap<String, Option<Vec<PathBuf>>>;

pub fn matches_by_name(kind: ProjectType, game_dir: &Path, known: &[PackFile]) -> PackMatches {
    find_matches(kind, game_dir, known, None)
}

/// Paths that can be removed together. A data pack installed into multiple worlds
/// needs a separate world choice, so Discover leaves it alone.
pub fn removable_files<'a>(
    matches: &'a PackMatches,
    kind: ProjectType,
    project: &str,
) -> Option<&'a [PathBuf]> {
    let paths = matches.get(project)?.as_deref()?;
    let first = paths.first()?;
    (kind != ProjectType::DataPack || paths.iter().all(|p| p.parent() == first.parent()))
        .then_some(paths)
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
        self.matches(kind, game_dir, known).into_keys().collect()
    }

    pub fn matches(
        &mut self,
        kind: ProjectType,
        game_dir: &Path,
        known: &[PackFile],
    ) -> PackMatches {
        find_matches(kind, game_dir, known, Some(self))
    }

    fn hash_files(&mut self, files: &HashMap<PathBuf, fs::Metadata>) {
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

fn find_matches(
    kind: ProjectType,
    game_dir: &Path,
    known: &[PackFile],
    scan: Option<&mut PackScan>,
) -> PackMatches {
    let mut tracked = HashMap::new();
    let mut files = HashMap::new();
    for dir in pack_dirs(kind, game_dir) {
        let path = dir.join(INDEX);
        match load_json::<Vec<PackFile>>(&path) {
            Ok(Some(entries)) => {
                tracked.insert(dir.clone(), entries);
            }
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

    let hashes = scan.map(|scan| {
        if known
            .iter()
            .chain(tracked.values().flatten())
            .any(|p| p.sha1.is_some())
        {
            scan.hash_files(&files);
        } else {
            scan.hashes.clear();
        }
        &scan.hashes
    });
    let mut matches = PackMatches::new();
    for path in files.keys() {
        let Some((dir, name)) = path.parent().zip(path.file_name()) else {
            continue;
        };
        let hash = hashes.and_then(|hashes| hashes.get(path));
        let named_here = tracked.get(dir).into_iter().flatten().chain(known);
        let mut projects: HashSet<_> = named_here
            .filter(|entry| name == std::ffi::OsStr::new(&entry.file_name))
            .map(|entry| entry.project_id.as_str())
            .collect();
        if let Some(hash) = hash {
            projects.extend(
                known
                    .iter()
                    .chain(tracked.values().flatten())
                    .filter(|entry| {
                        entry
                            .sha1
                            .as_ref()
                            .is_some_and(|sha1| sha1.eq_ignore_ascii_case(&hash.sha1))
                    })
                    .map(|entry| entry.project_id.as_str()),
            );
        }
        let unique = projects.len() == 1;
        for project in projects {
            let matched = matches
                .entry(project.to_owned())
                .or_insert_with(|| Some(Vec::new()));
            if !unique {
                *matched = None;
            } else if let Some(paths) = matched {
                paths.push(path.clone());
            }
        }
    }
    for paths in matches.values_mut().flatten() {
        paths.sort();
    }
    matches
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
    fn removal_finds_renamed_files_and_excludes_ambiguous_matches() {
        let game = tempfile::tempdir().unwrap();
        let dir = game.path().join("resourcepacks");
        fs::create_dir(&dir).unwrap();
        let renamed = dir.join("renamed.zip");
        fs::write(&renamed, b"x").unwrap();
        let kind = ProjectType::ResourcePack;
        let mut scan = PackScan::default();
        let matches = scan.matches(kind, game.path(), &[file("first", "old.zip")]);
        assert_eq!(
            removable_files(&matches, kind, "first"),
            Some(std::slice::from_ref(&renamed))
        );

        let matches = scan.matches(
            kind,
            game.path(),
            &[file("first", "old.zip"), file("second", "other.zip")],
        );
        for project in ["first", "second"] {
            assert!(matches.contains_key(project));
            assert!(removable_files(&matches, kind, project).is_none());
        }
    }

    #[test]
    fn data_pack_removal_requires_a_single_matched_world() {
        let game = tempfile::tempdir().unwrap();
        let kind = ProjectType::DataPack;
        let first = game.path().join("saves/First");
        let second = game.path().join("saves/Second");
        let path = first.join("datapacks/pack.zip");
        for world in [&first, &second] {
            fs::create_dir_all(world.join("datapacks")).unwrap();
            fs::write(world.join("level.dat"), []).unwrap();
        }
        let known = [file("pack", "pack.zip")];
        fs::write(&path, b"x").unwrap();
        let matches = matches_by_name(kind, game.path(), &known);
        assert_eq!(
            removable_files(&matches, kind, "pack"),
            Some(std::slice::from_ref(&path))
        );

        fs::write(second.join("datapacks/pack.zip"), b"x").unwrap();
        let matches = matches_by_name(kind, game.path(), &known);
        assert!(matches.contains_key("pack"));
        assert!(removable_files(&matches, kind, "pack").is_none());
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
