//! Bedrock worlds (`com.mojang\minecraftWorlds\<id>`): list them, export and
//! import `.mcworld` files, back them up and remove them (to a trash folder,
//! never deleted outright).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::{Player, archive};
use crate::error::IoContext;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// Folder name in `minecraftWorlds` (the game makes a random one).
    pub id: String,
    /// The name shown in game (`levelname.txt`), or the folder name.
    pub name: String,
    pub path: PathBuf,
    pub last_played: Option<SystemTime>,
    pub size: u64,
    /// `world_icon.jpeg`, if the world has one.
    pub icon: Option<PathBuf>,
}

/// A player's worlds, most recently played first.
pub fn list(player: &Player) -> Vec<World> {
    list_in(&player.worlds_dir())
}

/// Worlds in a `minecraftWorlds` folder, most recently played first.
pub fn list_in(worlds_dir: &Path) -> Vec<World> {
    let Ok(entries) = fs::read_dir(worlds_dir) else {
        return Vec::new();
    };
    let mut worlds: Vec<World> = entries
        .flatten()
        .filter_map(|e| read_world(&e.path()))
        .collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    worlds
}

/// A world folder (one with a `level.dat`), or `None`.
pub fn read_world(path: &Path) -> Option<World> {
    let level = fs::metadata(path.join("level.dat")).ok()?;
    let id = path.file_name()?.to_string_lossy().into_owned();
    let name = fs::read_to_string(path.join("levelname.txt"))
        .ok()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    // The game rewrites the database while a world is played.
    let played = fs::metadata(path.join("db"))
        .and_then(|m| m.modified())
        .ok()
        .into_iter()
        .chain(level.modified().ok())
        .max();
    Some(World {
        id,
        name,
        path: path.to_path_buf(),
        last_played: played,
        size: archive::dir_size(path),
        icon: Some(path.join("world_icon.jpeg")).filter(|p| p.is_file()),
    })
}

/// Save a world as a `.mcworld` file (which the game opens to import it).
pub fn export(world: &World, to: &Path) -> Result<()> {
    archive::zip_dir(&world.path, to)
}

/// Back a world up as `<name>-<time>.mcworld` in `backups_dir`; returns the file.
pub fn backup(world: &World, backups_dir: &Path) -> Result<PathBuf> {
    let name = archive::safe_name(&world.name, "World");
    let path = backups_dir.join(format!("{name}-{}.mcworld", crate::auth::now_secs()));
    export(world, &path)?;
    Ok(path)
}

/// Import a world into the player's worlds: a `.mcworld` / `.zip` file, or
/// a world folder. Returns the world's name.
pub fn import(from: &Path, player: &Player) -> Result<String> {
    let worlds_dir = player.worlds_dir();
    fs::create_dir_all(&worlds_dir).at(&worlds_dir)?;
    let id = new_id(&worlds_dir);
    let partial = worlds_dir.join(format!(".{id}.importing"));
    if from.is_dir() {
        if !from.join("level.dat").is_file() {
            return Err(Error::Other(format!(
                "{} is not a Bedrock world",
                from.display()
            )));
        }
        let _ = fs::remove_dir_all(&partial);
        archive::copy_dir(from, &partial)?;
    } else {
        let mut zip = archive::open(from)?;
        let prefix = archive::dirs_with(&zip, "level.dat", 1)
            .into_iter()
            .next()
            .ok_or_else(|| Error::Other("no Bedrock world (level.dat) in this file".into()))?;
        archive::unpack(&mut zip, &prefix, &partial, &[])?;
    }
    if !partial.join("db").is_dir() && partial.join("region").is_dir() {
        let _ = fs::remove_dir_all(&partial);
        return Err(Error::Other(
            "this is a Java Edition world; Bedrock can't open it".into(),
        ));
    }
    let target = worlds_dir.join(&id);
    fs::rename(&partial, &target).at(&target)?;
    Ok(read_world(&target).map_or(id, |w| w.name))
}

/// Move a world to the player's trash folder (kept, not deleted).
pub fn trash(world: &World, player: &Player) -> Result<()> {
    archive::trash(&world.path, &player.trash_dir().join("worlds"))
}

/// Rename a world (what the game shows; the folder stays).
pub fn rename(world: &World, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("a world needs a name".into()));
    }
    let path = world.path.join("levelname.txt");
    fs::write(&path, name).at(&path)
}

/// A folder name like the game's (12 letters and digits) that isn't taken.
fn new_id(worlds_dir: &Path) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    loop {
        let bytes = uuid::Uuid::new_v4().into_bytes();
        let id: String = bytes[..12]
            .iter()
            .map(|b| CHARS[*b as usize % CHARS.len()] as char)
            .collect();
        if !worlds_dir.join(&id).exists() {
            return id;
        }
    }
}
