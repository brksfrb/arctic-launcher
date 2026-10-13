//! Where the launcher keeps its files when the player picked a folder (Settings → Storage).
//!
//! The choice is one small file, `location.txt`, in the default data folder. It is read before
//! anything else, so everything (game files, Java, instances, accounts, settings) follows it.
//! Moving copies the folder's contents, points `location.txt` at the copy and leaves a note in
//! the new folder; the next start removes the old copy (nothing in it is open by then).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::IoContext;
use crate::{Error, Result};

/// In the default data folder: the folder the player picked instead.
pub const LOCATION_FILE: &str = "location.txt";
/// In the new folder after a move: the old folder, removed on the next start.
const CLEANUP_FILE: &str = "old-location.txt";
/// Not moved: downloads and thumbnails the launcher fetches again when needed.
const NOT_MOVED: &[&str] = &["cache"];
/// Tries at removing the old folder (a file can still be held briefly by the previous process).
const CLEANUP_TRIES: u32 = 5;
const CLEANUP_PAUSE: std::time::Duration = std::time::Duration::from_millis(800);

/// How the data folder was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootSource {
    /// `ARCTIC_DATA_DIR`.
    Env(PathBuf),
    /// `portable.txt` next to the launcher: `data` beside it.
    Portable(PathBuf),
    /// Picked in Settings → Storage.
    Chosen(PathBuf),
    /// Picked, but not there now (an unplugged drive): the default folder is used meanwhile.
    ChosenMissing { chosen: PathBuf, using: PathBuf },
    /// The usual place.
    Default(PathBuf),
}

impl RootSource {
    /// The folder in use.
    pub fn root(&self) -> &Path {
        match self {
            Self::Env(p) | Self::Portable(p) | Self::Chosen(p) | Self::Default(p) => p,
            Self::ChosenMissing { using, .. } => using,
        }
    }

    /// The player can move it from the launcher (not with the env var or portable mode).
    pub fn movable(&self) -> bool {
        matches!(
            self,
            Self::Chosen(_) | Self::Default(_) | Self::ChosenMissing { .. }
        )
    }
}

/// The usual data folder (`%LOCALAPPDATA%\ArcticLauncher`, `~/.local/share/ArcticLauncher`,
/// `~/Library/Application Support/ArcticLauncher`).
pub fn default_root() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|d| d.join(super::DIR_NAME))
        .ok_or_else(|| Error::Other("could not determine local app data directory".into()))
}

/// The folder `location.txt` points at, if any.
pub fn chosen(default: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(default.join(LOCATION_FILE)).ok()?;
    let path = PathBuf::from(text.trim());
    (path.is_absolute()).then_some(path)
}

/// Pick the data folder: `chosen` when it's there, otherwise the default (and say so).
pub(super) fn from_choice(default: PathBuf) -> RootSource {
    match chosen(&default) {
        Some(path) if path == default => RootSource::Default(default),
        Some(path) if path.is_dir() => RootSource::Chosen(path),
        Some(path) => RootSource::ChosenMissing {
            chosen: path,
            using: default,
        },
        None => RootSource::Default(default),
    }
}

/// Remember `folder` as the data folder (the default folder: forget the choice).
pub fn set_chosen(default: &Path, folder: &Path) -> Result<()> {
    let file = default.join(LOCATION_FILE);
    if folder == default {
        return match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::io(&file, e)),
            _ => Ok(()),
        };
    }
    fs::create_dir_all(default).at(default)?;
    fs::write(&file, folder.to_string_lossy().as_bytes()).at(&file)
}

/// What a folder offered as the new data folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Nothing (or only our own `location.txt`): the files can be moved there.
    Empty,
    /// Arctic's files already (another copy, or a folder used before): use them as they are.
    ArcticData,
    /// Other things: pick an empty folder or make one inside it.
    Other,
}

/// Look at `folder` before moving there.
pub fn inspect(folder: &Path) -> Target {
    let Ok(entries) = fs::read_dir(folder) else {
        return Target::Empty;
    };
    let names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != LOCATION_FILE)
        .collect();
    if names.is_empty() {
        Target::Empty
    } else if names.iter().any(|n| n == "profiles.json") {
        Target::ArcticData
    } else {
        Target::Other
    }
}

/// Why `to` can't hold the data now in `from`, if it can't.
pub fn check_move(from: &Path, to: &Path) -> Option<String> {
    if !to.is_absolute() {
        return Some("Pick a full folder path.".into());
    }
    let (from_c, to_c) = (normal(from), normal(to));
    if from_c == to_c {
        return Some("That's already where Arctic keeps its files.".into());
    }
    if to_c.starts_with(&from_c) {
        return Some("Pick a folder outside Arctic's current folder.".into());
    }
    if from_c.starts_with(&to_c) {
        return Some("Pick a folder that doesn't contain Arctic's current folder.".into());
    }
    None
}

/// `path` with `..` and symlinks resolved as far as it exists (a new folder doesn't yet).
fn normal(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut at = path.to_path_buf();
    loop {
        if let Ok(real) = fs::canonicalize(&at) {
            return missing
                .into_iter()
                .rev()
                .fold(real, |p: PathBuf, part| p.join(part));
        }
        match (at.file_name().map(ToOwned::to_owned), at.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name);
                at = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Bytes that a move of `from` copies.
pub fn size_to_move(from: &Path) -> u64 {
    top_entries(from).iter().map(|p| tree_size(p)).sum()
}

fn top_entries(from: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(from) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            !NOT_MOVED.contains(&name.as_str()) && name != LOCATION_FILE && name != CLEANUP_FILE
        })
        .map(|e| e.path())
        .collect()
}

fn tree_size(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::read_dir(path)
            .map(|entries| entries.flatten().map(|e| tree_size(&e.path())).sum())
            .unwrap_or(0),
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

/// Copy the data in `from` to the empty folder `to`, then make `to` the data folder. `progress`
/// gets (bytes copied, bytes in all). The old folder is removed on the next start
/// ([`finish_move`]); until the launcher restarts it keeps using `from`.
pub fn move_data(
    default: &Path,
    from: &Path,
    to: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<()> {
    if let Some(why) = check_move(from, to) {
        return Err(Error::Other(why));
    }
    if inspect(to) != Target::Empty {
        return Err(Error::Other(
            "Pick an empty folder (or make a new one).".into(),
        ));
    }
    let existed = to.exists();
    fs::create_dir_all(to).at(to)?;
    let total = size_to_move(from);
    let mut done = 0u64;
    let copied = top_entries(from).iter().try_for_each(|entry| {
        let name = entry.file_name().unwrap_or_default();
        copy_tree(entry, &to.join(name), &mut done, total, progress)
    });
    if let Err(e) = copied {
        // Only what this move made: the folder was empty (or not there) before.
        if existed {
            for leftover in top_entries(to) {
                let _ = fs::remove_dir_all(&leftover).or_else(|_| fs::remove_file(&leftover));
            }
        } else {
            let _ = fs::remove_dir_all(to);
        }
        return Err(e);
    }
    let note = to.join(CLEANUP_FILE);
    fs::write(&note, from.to_string_lossy().as_bytes()).at(&note)?;
    set_chosen(default, to)
}

/// Copy the folder `from` into `to` (made if missing), with (bytes copied, bytes in all).
pub(crate) fn copy_dir(from: &Path, to: &Path, progress: &dyn Fn(u64, u64)) -> Result<()> {
    let total = tree_size(from);
    let mut done = 0;
    copy_tree(from, to, &mut done, total, progress)
}

fn copy_tree(
    from: &Path,
    to: &Path,
    done: &mut u64,
    total: u64,
    progress: &dyn Fn(u64, u64),
) -> Result<()> {
    let meta = fs::symlink_metadata(from).at(from)?;
    if meta.is_dir() {
        fs::create_dir_all(to).at(to)?;
        for entry in fs::read_dir(from).at(from)?.flatten() {
            copy_tree(
                &entry.path(),
                &to.join(entry.file_name()),
                done,
                total,
                progress,
            )?;
        }
        return Ok(());
    }
    if meta.file_type().is_symlink() {
        // Links in the data folder are the launcher's own shortcuts: the target is copied instead.
        if !from.exists() {
            return Ok(());
        }
    }
    fs::copy(from, to).at(to)?;
    *done += meta.len();
    progress(*done, total);
    Ok(())
}

/// Use the Arctic files already in `to` as they are (nothing copied, nothing removed).
pub fn adopt(default: &Path, to: &Path) -> Result<()> {
    if inspect(to) != Target::ArcticData {
        return Err(Error::Other(
            "That folder doesn't hold Arctic's files.".into(),
        ));
    }
    set_chosen(default, to)
}

/// After a move: remove the old folder named in the new one's note. Only a folder that looks
/// like Arctic's data is removed, never one inside or around the new folder, and the default
/// folder keeps `location.txt`. Returns the folder removed.
pub fn finish_move(root: &Path) -> Option<PathBuf> {
    let note = root.join(CLEANUP_FILE);
    let old = PathBuf::from(fs::read_to_string(&note).ok()?.trim());
    let (old_c, root_c) = (normal(&old), normal(root));
    let looks_like_data = old.join("profiles.json").is_file() || old.join("shared").is_dir();
    if !old.is_absolute()
        || old_c == root_c
        || old_c.starts_with(&root_c)
        || root_c.starts_with(&old_c)
        || !looks_like_data
    {
        log::warn!("not removing the old data folder {}", old.display());
        let _ = fs::remove_file(&note);
        return None;
    }
    for attempt in 1..=CLEANUP_TRIES {
        let mut left = 0;
        for entry in fs::read_dir(&old)
            .map(|e| e.flatten().collect::<Vec<_>>())
            .unwrap_or_default()
        {
            if entry.file_name() == LOCATION_FILE {
                continue;
            }
            let path = entry.path();
            let gone = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if gone.is_err() {
                left += 1;
            }
        }
        if left == 0 {
            // The default folder stays (it holds location.txt); a picked one goes entirely.
            let _ = fs::remove_dir(&old);
            let _ = fs::remove_file(&note);
            log::info!("removed the old data folder {}", old.display());
            return Some(old);
        }
        if attempt < CLEANUP_TRIES {
            std::thread::sleep(CLEANUP_PAUSE);
        }
    }
    log::warn!(
        "couldn't remove all of the old data folder {}: trying again next start",
        old.display()
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(dir: &Path) {
        fs::create_dir_all(dir.join("shared/libraries/a")).unwrap();
        fs::write(dir.join("shared/libraries/a/lib.jar"), b"jar").unwrap();
        fs::create_dir_all(dir.join("profiles/default/instances/vanilla/minecraft/saves/W"))
            .unwrap();
        fs::write(
            dir.join("profiles/default/instances/vanilla/minecraft/saves/W/level.dat"),
            b"world",
        )
        .unwrap();
        fs::write(dir.join("profiles.json"), b"{}").unwrap();
        fs::create_dir_all(dir.join("cache")).unwrap();
        fs::write(dir.join("cache/big.zip"), b"cache").unwrap();
    }

    #[test]
    fn move_copies_everything_but_the_cache_then_the_next_start_removes_the_old_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let default = tmp.path().join("default");
        data(&default);
        let to = tmp.path().join("D/Arctic");
        let seen = std::cell::Cell::new(0);
        move_data(&default, &default, &to, &|done, total| {
            assert!(done <= total);
            seen.set(done);
        })
        .unwrap();
        assert_eq!(seen.get(), size_to_move(&default));
        assert!(
            to.join("profiles/default/instances/vanilla/minecraft/saves/W/level.dat")
                .is_file()
        );
        assert!(to.join("shared/libraries/a/lib.jar").is_file());
        assert!(!to.join("cache").exists());
        assert_eq!(from_choice(default.clone()), RootSource::Chosen(to.clone()));

        assert_eq!(finish_move(&to), Some(default.clone()));
        assert!(!default.join("shared").exists());
        assert!(default.join(LOCATION_FILE).is_file(), "the pointer stays");
        assert!(finish_move(&to).is_none(), "once");
    }

    #[test]
    fn moving_back_to_the_default_forgets_the_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let default = tmp.path().join("default");
        let picked = tmp.path().join("picked");
        data(&picked);
        set_chosen(&default, &picked).unwrap();
        assert_eq!(inspect(&default), Target::Empty, "only location.txt there");
        move_data(&default, &picked, &default, &|_, _| {}).unwrap();
        assert_eq!(
            from_choice(default.clone()),
            RootSource::Default(default.clone())
        );
        assert_eq!(finish_move(&default), Some(picked.clone()));
        assert!(!picked.exists());
    }

    #[test]
    fn refuses_folders_inside_or_around_the_current_one_and_busy_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("data");
        data(&from);
        assert!(check_move(&from, &from.join("sub")).is_some());
        assert!(check_move(&from, tmp.path()).is_some());
        assert!(check_move(&from, &from).is_some());
        let busy = tmp.path().join("busy");
        fs::create_dir_all(&busy).unwrap();
        fs::write(busy.join("photo.png"), b"x").unwrap();
        assert_eq!(inspect(&busy), Target::Other);
        assert!(move_data(&from, &from, &busy, &|_, _| {}).is_err());
        assert!(
            busy.join("photo.png").is_file(),
            "nothing of theirs touched"
        );
    }

    #[test]
    fn a_missing_choice_falls_back_to_the_default_and_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        let default = tmp.path().join("default");
        let gone = tmp.path().join("unplugged");
        set_chosen(&default, &gone).unwrap();
        assert_eq!(
            from_choice(default.clone()),
            RootSource::ChosenMissing {
                chosen: gone,
                using: default
            }
        );
    }

    #[test]
    fn adopting_uses_an_existing_copy_without_touching_files() {
        let tmp = tempfile::tempdir().unwrap();
        let default = tmp.path().join("default");
        let other = tmp.path().join("other");
        data(&other);
        assert_eq!(inspect(&other), Target::ArcticData);
        adopt(&default, &other).unwrap();
        assert_eq!(from_choice(default), RootSource::Chosen(other.clone()));
        assert!(other.join("cache/big.zip").is_file());
    }

    #[test]
    fn the_old_folder_is_kept_when_it_does_not_look_like_arctic_data() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("new");
        let stranger = tmp.path().join("stranger");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&stranger).unwrap();
        fs::write(stranger.join("thesis.docx"), b"x").unwrap();
        fs::write(
            root.join(CLEANUP_FILE),
            stranger.to_string_lossy().as_bytes(),
        )
        .unwrap();
        assert!(finish_move(&root).is_none());
        assert!(stranger.join("thesis.docx").is_file());
    }
}
