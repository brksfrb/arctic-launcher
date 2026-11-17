//! Removed worlds: moved (renamed, never copied) into `saves/.trash` next to a small JSON note
//! saying where they came from. They can be restored, deleted for good, and are purged after
//! [`TRASH_KEEP_DAYS`] days. `saves` is on the same drive as the world, so a move can't half-copy.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{World, dir_size, free_name, read_world};
use crate::error::IoContext;
use crate::storage::{load_json, save_json};
use crate::{Error, Result};

pub(super) const TRASH_DIR: &str = ".trash";
/// Trashed worlds older than this are deleted for good.
pub const TRASH_KEEP_DAYS: u64 = 14;
const KEEP_SECS: u64 = TRASH_KEEP_DAYS * 86_400;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Note {
    instance: String,
    folder: String,
    name: String,
    original_path: PathBuf,
    deleted_at: u64,
}

/// A world in an instance's trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trashed {
    /// The folder in `.trash`.
    pub dir: PathBuf,
    /// Folder name it had in `saves`.
    pub folder: String,
    /// Name shown in game.
    pub name: String,
    /// Instance it came from (empty when unknown: worlds trashed by older versions).
    pub instance: String,
    pub original_path: PathBuf,
    /// Unix seconds; 0 when unknown.
    pub deleted_at: u64,
    pub size: u64,
}

fn trash_dir(saves: &Path) -> PathBuf {
    saves.join(TRASH_DIR)
}

pub(super) fn note_path(dir: &Path) -> PathBuf {
    let mut name = dir.as_os_str().to_os_string();
    name.push(".json");
    PathBuf::from(name)
}

/// Move a world to `saves/.trash`. On any error the world stays where it was.
pub fn trash(world: &World, saves: &Path, instance: &str) -> Result<()> {
    trash_at(world, saves, instance, crate::auth::now_secs())
}

/// Like [`trash`], but refuses a starred world: unstar it first.
pub fn trash_unless_favorite(
    world: &World,
    saves: &Path,
    instance: &str,
    favorites: &super::Favorites,
) -> Result<()> {
    if favorites.contains(&world.folder) {
        return Err(Error::Other(format!(
            "\"{}\" is a favorite; remove it from favorites before deleting it",
            world.name
        )));
    }
    trash(world, saves, instance)
}

pub(super) fn trash_at(world: &World, saves: &Path, instance: &str, now: u64) -> Result<()> {
    let root = trash_dir(saves);
    fs::create_dir_all(&root).at(&root)?;
    let base = format!("{}-{now}", world.folder);
    let to = (0..)
        .map(|n| match n {
            0 => root.join(&base),
            n => root.join(format!("{base}-{n}")),
        })
        .find(|p| !p.exists() && !note_path(p).exists())
        .expect("an unbounded range");
    let note = Note {
        instance: instance.to_owned(),
        folder: world.folder.clone(),
        name: world.name.clone(),
        original_path: world.path.clone(),
        deleted_at: now,
    };
    // The note first: without it the world would still be restorable (the folder name says
    // enough), but with it we know the original name.
    let note_file = note_path(&to);
    save_json(&note_file, &note)?;
    if let Err(e) = fs::rename(&world.path, &to) {
        let _ = fs::remove_file(&note_file);
        return Err(Error::io(&world.path, e));
    }
    Ok(())
}

/// Worlds in the trash, most recently deleted first.
pub fn list_trash(saves: &Path) -> Vec<Trashed> {
    let Ok(entries) = fs::read_dir(trash_dir(saves)) else {
        return Vec::new();
    };
    let mut out: Vec<Trashed> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| read_entry(&e.path()))
        .collect();
    out.sort_by_key(|t| std::cmp::Reverse(t.deleted_at));
    out
}

fn read_entry(dir: &Path) -> Trashed {
    let entry_name = dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let note = load_json::<Note>(&note_path(dir)).ok().flatten();
    // Without a note (trashed by an older version): "<folder>-<unix seconds>".
    let guessed = entry_name
        .rsplit_once('-')
        .and_then(|(folder, at)| Some((folder.to_owned(), at.parse::<u64>().ok()?)));
    let (folder, deleted_at) = match (&note, guessed) {
        (Some(n), _) => (n.folder.clone(), n.deleted_at),
        (None, Some((folder, at))) => (folder, at),
        (None, None) => (entry_name.clone(), 0),
    };
    let name = note
        .as_ref()
        .map(|n| n.name.clone())
        .or_else(|| read_world(dir).map(|w| w.name))
        .unwrap_or_else(|| folder.clone());
    Trashed {
        dir: dir.to_path_buf(),
        original_path: note
            .as_ref()
            .map_or_else(PathBuf::new, |n| n.original_path.clone()),
        instance: note.map_or_else(String::new, |n| n.instance),
        folder,
        name,
        deleted_at,
        size: dir_size(dir),
    }
}

/// `dir` must be a folder directly inside `saves/.trash`.
fn check(saves: &Path, dir: &Path) -> Result<()> {
    if dir.parent() == Some(trash_dir(saves).as_path()) && dir.is_dir() {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "{} isn't in the world trash",
            dir.display()
        )))
    }
}

/// Bring a trashed world back into `saves`; returns the folder name it got (not the old one when
/// that name is taken again now: the existing world is never touched).
pub fn restore(item: &Trashed, saves: &Path) -> Result<String> {
    check(saves, &item.dir)?;
    fs::create_dir_all(saves).at(saves)?;
    let name = free_name(saves, &item.folder);
    let to = saves.join(&name);
    fs::rename(&item.dir, &to).at(&item.dir)?;
    let _ = fs::remove_file(note_path(&item.dir));
    Ok(name)
}

/// Delete a trashed world for good.
pub fn delete_forever(item: &Trashed, saves: &Path) -> Result<()> {
    check(saves, &item.dir)?;
    fs::remove_dir_all(&item.dir).at(&item.dir)?;
    let _ = fs::remove_file(note_path(&item.dir));
    Ok(())
}

/// Delete what has been in the trash longer than [`TRASH_KEEP_DAYS`] days; returns how many.
/// Entries whose age is unknown are kept. A failure on one never stops the others.
pub fn purge_expired(saves: &Path, now: u64) -> usize {
    let mut purged = 0;
    for item in list_trash(saves) {
        if item.deleted_at == 0 || now.saturating_sub(item.deleted_at) <= KEEP_SECS {
            continue;
        }
        match delete_forever(&item, saves) {
            Ok(()) => purged += 1,
            Err(e) => log::warn!("couldn't purge trashed world {}: {e}", item.dir.display()),
        }
    }
    // Notes whose folder is gone (a crash between the two steps).
    if let Ok(entries) = fs::read_dir(trash_dir(saves)) {
        for path in entries.flatten().map(|e| e.path()) {
            let Some(entry) = path
                .to_str()
                .and_then(|p| p.strip_suffix(".json"))
                .map(Path::new)
            else {
                continue;
            };
            if path.is_file() && !entry.exists() {
                let _ = fs::remove_file(&path);
            }
        }
    }
    purged
}
