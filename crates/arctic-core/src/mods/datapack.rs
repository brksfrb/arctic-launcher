//! Modrinth data packs are installed into a single saved world.

use std::fs;
use std::path::{Path, PathBuf};

use super::{modrinth, packs};
use crate::{Error, Progress, Result};

pub(super) const LOADERS: &[&str] = &["datapack"];

/// Saved world folder names, sorted for the picker. Only direct saves with a level.dat qualify.
pub fn worlds(game_dir: &Path) -> Result<Vec<String>> {
    let saves = game_dir.join("saves");
    let entries = match fs::read_dir(&saves) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(saves, e)),
    };
    let mut worlds: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|folder| install_dir(game_dir, folder).is_ok())
        .collect();
    worlds.sort_by_cached_key(|folder| folder.to_lowercase());
    Ok(worlds)
}

/// Resolve a picker choice within this instance. Never follow world or data pack folder links.
pub fn install_dir(game_dir: &Path, world: &str) -> Result<PathBuf> {
    let mut parts = Path::new(world).components();
    if !matches!(parts.next(), Some(std::path::Component::Normal(name)) if name == world)
        || parts.next().is_some()
    {
        return Err(Error::DataPackWorldMissing);
    }
    let saves = game_dir.join("saves");
    let world = saves.join(world);
    if !real_dir(&saves) || !real_dir(&world) || !world.join("level.dat").is_file() {
        return Err(Error::DataPackWorldMissing);
    }
    let target = world.join("datapacks");
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_dir() => Ok(target),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(target),
        _ => Err(Error::DataPackWorldMissing),
    }
}

fn real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

pub fn install(
    version_id: &str,
    game_dir: &Path,
    world: &str,
    game: &str,
    allow_other_game: bool,
    progress: Progress,
) -> Result<()> {
    let version = modrinth::version(version_id)?;
    let other_game = packs::check_install_version(LOADERS, &version, game, allow_other_game)?;
    let plan = packs::plan_archives(version, LOADERS, game, other_game, &Default::default())?;
    let dest = install_dir(game_dir, world)?;
    packs::download_archives(&plan, &dest, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_pack_target_is_inside_an_existing_save() {
        let instance = tempfile::tempdir().unwrap();
        let game = instance.path();
        assert!(worlds(game).unwrap().is_empty());
        let world = game.join("saves/World One");
        fs::create_dir_all(&world).unwrap();
        fs::create_dir(game.join("saves/Not A World")).unwrap();
        fs::write(world.join("level.dat"), []).unwrap();
        assert_eq!(worlds(game).unwrap(), ["World One"]);
        assert_eq!(
            install_dir(game, "World One").unwrap(),
            world.join("datapacks")
        );
        for bad in [
            "",
            "missing",
            "Not A World",
            "World One/.",
            "../saves/World One",
        ] {
            assert!(install_dir(game, bad).is_err(), "{bad}");
        }
        assert!(!world.join("datapacks").exists());
        fs::remove_file(world.join("level.dat")).unwrap();
        assert!(install_dir(game, "World One").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn data_pack_target_cannot_follow_world_or_pack_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let game = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("level.dat"), []).unwrap();
        let saves = game.path().join("saves");
        fs::create_dir(&saves).unwrap();
        symlink(outside.path(), saves.join("Linked")).unwrap();
        assert!(install_dir(game.path(), "Linked").is_err());

        let world = saves.join("Real");
        fs::create_dir(&world).unwrap();
        fs::write(world.join("level.dat"), []).unwrap();
        symlink(outside.path(), world.join("datapacks")).unwrap();
        assert!(install_dir(game.path(), "Real").is_err());
    }
}
