//! Singleplayer worlds in an instance's `saves` folder: list them, import
//! from other launchers, instances or .zip files, back them up and remove
//! them (to a trash folder, never deleted outright).

mod nbt;
mod sources;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use sources::{Source, sources};

use crate::error::IoContext;
use crate::{Error, Result};

/// Folder inside the instance directory for world backups.
pub const BACKUPS_DIR: &str = "backups";
const TRASH_DIR: &str = ".trash";
/// Refuse to unpack archives larger than this (a zip bomb guard).
const MAX_UNPACKED_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// Folder name inside `saves`.
    pub folder: String,
    /// Name shown in game (`LevelName` from level.dat), or the folder name.
    pub name: String,
    pub path: PathBuf,
    pub last_played: Option<SystemTime>,
    pub size: u64,
    /// `icon.png`, if the world has one.
    pub icon: Option<PathBuf>,
}

/// Worlds in `saves_dir`, most recently played first. A missing folder is empty.
pub fn list(saves_dir: &Path) -> Vec<World> {
    let Ok(entries) = fs::read_dir(saves_dir) else {
        return Vec::new();
    };
    let mut worlds: Vec<World> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy() != TRASH_DIR)
        .filter_map(|e| read_world(&e.path()))
        .collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    worlds
}

/// A world folder (one with a `level.dat`), or `None`.
pub fn read_world(path: &Path) -> Option<World> {
    let level = path.join("level.dat");
    let meta = fs::metadata(&level).ok()?;
    let folder = path.file_name()?.to_string_lossy().into_owned();
    let name = fs::read(&level)
        .ok()
        .and_then(|bytes| nbt::level_name(&bytes))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| folder.clone());
    let icon = Some(path.join("icon.png")).filter(|p| p.is_file());
    Some(World {
        folder,
        name,
        path: path.to_path_buf(),
        last_played: meta.modified().ok(),
        size: dir_size(path),
        icon,
    })
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

/// A folder name in `saves_dir` based on `wanted` that isn't taken yet.
fn free_name(saves_dir: &Path, wanted: &str) -> String {
    let clean: String = wanted
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let base = clean.trim().trim_matches('.').to_owned();
    let base = if base.is_empty() {
        "World".to_owned()
    } else {
        base
    };
    if !saves_dir.join(&base).exists() {
        return base;
    }
    (2..)
        .map(|n| format!("{base} ({n})"))
        .find(|name| !saves_dir.join(name).exists())
        .unwrap_or(base)
}

/// Copy a world folder into `saves_dir`; returns the new folder name.
pub fn import_folder(world: &Path, saves_dir: &Path) -> Result<String> {
    if !world.join("level.dat").is_file() {
        return Err(Error::Other(format!(
            "{} is not a Minecraft world",
            world.display()
        )));
    }
    let wanted = world
        .file_name()
        .map_or("World".into(), |n| n.to_string_lossy().into_owned());
    let name = free_name(saves_dir, &wanted);
    let target = saves_dir.join(&name);
    let partial = saves_dir.join(format!(".{name}.importing"));
    let _ = fs::remove_dir_all(&partial);
    copy_dir(world, &partial)?;
    fs::rename(&partial, &target).at(&target)?;
    Ok(name)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).at(to)?;
    for entry in fs::read_dir(from).at(from)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        // The game's lock file belongs to the running copy only.
        if entry.file_name() == "session.lock" {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => copy_dir(&src, &dst)?,
            Ok(t) if t.is_file() => {
                fs::copy(&src, &dst).at(&src)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Unpack a world from a .zip (with `level.dat` at the root or one folder
/// down) into `saves_dir`; returns the new folder name.
pub fn import_zip(zip_path: &Path, saves_dir: &Path) -> Result<String> {
    let file = fs::File::open(zip_path).at(zip_path)?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("not a zip file: {e}")))?;
    let prefix = world_prefix(&mut zip)?;
    let wanted = match prefix.trim_end_matches('/') {
        "" => zip_path
            .file_stem()
            .map_or("World".into(), |s| s.to_string_lossy().into_owned()),
        dir => dir.rsplit('/').next().unwrap_or(dir).to_owned(),
    };
    let name = free_name(saves_dir, &wanted);
    let partial = saves_dir.join(format!(".{name}.importing"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).at(&partial)?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
        // `enclosed_name` rejects absolute paths and `..` (zip slip).
        let Some(inner) = entry.enclosed_name() else {
            continue;
        };
        let Ok(relative) = inner.strip_prefix(&prefix) else {
            continue;
        };
        if relative.as_os_str().is_empty() || relative.ends_with("session.lock") {
            continue;
        }
        let out = partial.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).at(&out)?;
            continue;
        }
        total += entry.size();
        if total > MAX_UNPACKED_BYTES {
            let _ = fs::remove_dir_all(&partial);
            return Err(Error::Other("the archive is too large".into()));
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let mut dst = fs::File::create(&out).at(&out)?;
        std::io::copy(&mut entry, &mut dst).at(&out)?;
    }
    let target = saves_dir.join(&name);
    fs::rename(&partial, &target).at(&target)?;
    Ok(name)
}

/// Folder inside the archive that holds `level.dat` ("" for the root).
fn world_prefix(zip: &mut zip::ZipArchive<fs::File>) -> Result<String> {
    let mut best: Option<String> = None;
    for name in zip.file_names() {
        if let Some(dir) = name.strip_suffix("level.dat") {
            let depth = dir.matches('/').count();
            if depth <= 1 && best.as_ref().is_none_or(|b| dir.len() < b.len()) {
                best = Some(dir.to_owned());
            }
        }
    }
    best.ok_or_else(|| Error::Other("no Minecraft world (level.dat) in this archive".into()))
}

/// Zip a world into `backups_dir`; returns the archive path.
pub fn backup(world: &World, backups_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(backups_dir).at(backups_dir)?;
    let stamp = crate::auth::now_secs();
    let path = backups_dir.join(format!(
        "{}-{stamp}.zip",
        free_name(backups_dir, &world.folder)
    ));
    let file = fs::File::create(&path).at(&path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    add_dir(&mut zip, &world.path, &world.folder, options)?;
    zip.finish().map_err(|e| Error::Other(e.to_string()))?;
    Ok(path)
}

fn add_dir(
    zip: &mut zip::ZipWriter<fs::File>,
    dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in fs::read_dir(dir).at(dir)?.flatten() {
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if entry.file_name() == "session.lock" {
            continue;
        }
        if path.is_dir() {
            add_dir(zip, &path, &name, options)?;
        } else {
            zip.start_file(name, options)
                .map_err(|e| Error::Other(e.to_string()))?;
            let mut buf = Vec::new();
            fs::File::open(&path)
                .at(&path)?
                .read_to_end(&mut buf)
                .at(&path)?;
            zip.write_all(&buf).at(&path)?;
        }
    }
    Ok(())
}

/// Move a world to `saves/.trash` (kept, not deleted).
pub fn trash(world: &World, saves_dir: &Path) -> Result<()> {
    let trash = saves_dir.join(TRASH_DIR);
    fs::create_dir_all(&trash).at(&trash)?;
    let to = trash.join(format!("{}-{}", world.folder, crate::auth::now_secs()));
    fs::rename(&world.path, &to).at(&world.path)
}

#[cfg(test)]
mod tests;
