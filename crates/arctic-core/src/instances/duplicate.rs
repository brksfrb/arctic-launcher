//! Duplicating an instance: a copy with a folder of its own, holding the same mods, packs, settings and
//! configs, and the worlds when asked.

use std::path::Path;

use super::{Instance, free_name, unique_id};
use crate::storage::DataDirs;
use crate::{Error, Result};

/// Folders of the game folder that don't come along (they are made again, or are only history).
const LEFT_BEHIND: &[&str] = &["logs", "crash-reports", ".mixin.out"];
/// Where the worlds are.
const WORLDS: &str = "saves";

/// A copy of `source` named "<name> copy" (or the next free name), in a folder of its own. The game folder
/// is copied (except logs, crash reports and, unless `with_worlds`, the worlds); the settings, loader and
/// version are the same. The source must not be running. A copy that fails halfway is removed again.
pub fn duplicate(dirs: &DataDirs, source: &Instance, with_worlds: bool) -> Result<Instance> {
    if source.is_default() || source.version.is_none() {
        return Err(Error::Other(
            "the Vanilla instance follows the newest release: make a new instance instead".into(),
        ));
    }
    let name = free_name(dirs, &format!("{} copy", source.name))?;
    let mut copy = source.clone();
    copy.id = unique_id(dirs, &name);
    copy.name = name;
    // Always a folder of its own, even when the original lives in one the player picked.
    copy.game_folder = None;
    let from = source.game_dir(dirs);
    let to = copy.game_dir(dirs);
    let done = copy_game_folder(&from, &to, with_worlds).and_then(|()| copy.save(dirs));
    if let Err(e) = done {
        let _ = std::fs::remove_dir_all(dirs.instance_dir(&copy.id));
        return Err(e);
    }
    Ok(copy)
}

fn copy_game_folder(from: &Path, to: &Path, with_worlds: bool) -> Result<()> {
    std::fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        // Nothing to copy yet (never started): an empty copy.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Error::io(from, e)),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let skip = LEFT_BEHIND.contains(&name_text.as_ref())
            || (!with_worlds && name_text == WORLDS)
            || (name_text.starts_with("hs_err_pid") && name_text.ends_with(".log"));
        if !skip {
            copy_entry(&entry.path(), &to.join(&name))?;
        }
    }
    Ok(())
}

/// A file or a whole folder (links are left out: they could point anywhere).
fn copy_entry(from: &Path, to: &Path) -> Result<()> {
    let kind = std::fs::symlink_metadata(from)
        .map_err(|e| Error::io(from, e))?
        .file_type();
    if kind.is_symlink() {
        return Ok(());
    }
    if kind.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
        for entry in std::fs::read_dir(from)
            .map_err(|e| Error::io(from, e))?
            .flatten()
        {
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| Error::io(from, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{self, Loader};

    fn setup() -> (tempfile::TempDir, DataDirs, Instance) {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        let source = instances::create(&dirs, "Survival", "1.21.11", Loader::Vanilla).unwrap();
        let game = source.game_dir(&dirs);
        std::fs::write(game.join("options.txt"), "fov:90").unwrap();
        std::fs::write(game.join("mods").join("a.jar"), "jar").unwrap();
        std::fs::create_dir_all(game.join("config")).unwrap();
        std::fs::write(game.join("config").join("sodium.json"), "{}").unwrap();
        std::fs::create_dir_all(game.join("saves").join("World1")).unwrap();
        std::fs::write(game.join("saves").join("World1").join("level.dat"), "w").unwrap();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(game.join("logs").join("latest.log"), "log").unwrap();
        (dir, dirs, source)
    }

    #[test]
    fn a_copy_has_the_mods_and_settings_but_not_the_worlds_unless_asked() {
        let (_dir, dirs, source) = setup();
        let copy = duplicate(&dirs, &source, false).unwrap();
        assert_eq!(copy.name, "Survival copy");
        assert_ne!(copy.id, source.id);
        assert_eq!(copy.version, source.version);
        let game = copy.game_dir(&dirs);
        assert_eq!(
            std::fs::read_to_string(game.join("options.txt")).unwrap(),
            "fov:90"
        );
        assert!(game.join("mods").join("a.jar").is_file());
        assert!(game.join("config").join("sodium.json").is_file());
        assert!(!game.join("saves").exists());
        assert!(!game.join("logs").exists());
        // The original is untouched.
        assert!(source.game_dir(&dirs).join("saves").join("World1").is_dir());
    }

    #[test]
    fn the_worlds_come_along_when_asked() {
        let (_dir, dirs, source) = setup();
        let copy = duplicate(&dirs, &source, true).unwrap();
        let world = copy
            .game_dir(&dirs)
            .join("saves")
            .join("World1")
            .join("level.dat");
        assert_eq!(std::fs::read_to_string(world).unwrap(), "w");
    }

    #[test]
    fn copying_twice_gives_the_next_free_name() {
        let (_dir, dirs, source) = setup();
        let first = duplicate(&dirs, &source, false).unwrap();
        let second = duplicate(&dirs, &source, false).unwrap();
        assert_eq!(first.name, "Survival copy");
        assert_eq!(second.name, "Survival copy (2)");
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn the_default_instance_cannot_be_copied() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        let vanilla = instances::load_default(&dirs).unwrap();
        assert!(duplicate(&dirs, &vanilla, false).is_err());
    }
}
