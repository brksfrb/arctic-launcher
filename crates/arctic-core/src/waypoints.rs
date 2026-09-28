//! Arctic Client waypoints: `config/arctic-waypoints.json` in an instance's
//! game folder, a map from world (a server address, or `sp:` + a
//! singleplayer world's name) to its waypoints. The game reads and writes
//! the same file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const FILE: &str = "arctic-waypoints.json";
/// The game keeps at most this many per world.
pub const MAX_PER_WORLD: usize = 50;
const DEFAULT_COLOR: i32 = 0xFF7D_D3FC_u32 as i32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default = "overworld")]
    pub dim: String,
    #[serde(default = "default_color")]
    pub color: i32,
    #[serde(default = "yes")]
    pub shown: bool,
}

fn overworld() -> String {
    "overworld".into()
}

fn default_color() -> i32 {
    DEFAULT_COLOR
}

fn yes() -> bool {
    true
}

pub type ByWorld = BTreeMap<String, Vec<Waypoint>>;

pub fn path(game_dir: &Path) -> PathBuf {
    game_dir.join("config").join(FILE)
}

/// Everything saved (empty when there's no file yet).
pub fn load(game_dir: &Path) -> Result<ByWorld> {
    let file = path(game_dir);
    match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| Error::Other(format!("{} can't be read: {e}", file.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ByWorld::new()),
        Err(e) => Err(Error::io(&file, e)),
    }
}

pub fn save(game_dir: &Path, all: &ByWorld) -> Result<()> {
    let file = path(game_dir);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let text = serde_json::to_string_pretty(all)?;
    std::fs::write(&file, text).map_err(|e| Error::io(&file, e))?;
    Ok(())
}

/// Add a waypoint to a world; refuses a full world or a name already used there.
pub fn add(all: &mut ByWorld, world: &str, waypoint: Waypoint) -> Result<()> {
    let name = waypoint.name.trim();
    if world.trim().is_empty() || name.is_empty() {
        return Err(Error::Other("a waypoint needs a world and a name".into()));
    }
    let list = all.entry(world.trim().to_owned()).or_default();
    if list.len() >= MAX_PER_WORLD {
        return Err(Error::Other(format!(
            "{world} already has {MAX_PER_WORLD} waypoints"
        )));
    }
    if list.iter().any(|w| w.name.eq_ignore_ascii_case(name)) {
        return Err(Error::Other(format!("{world} already has \"{name}\"")));
    }
    list.push(Waypoint {
        name: name.to_owned(),
        ..waypoint
    });
    Ok(())
}

/// Remove waypoints by name (any case) from a world; how many went.
pub fn remove(all: &mut ByWorld, world: &str, name: &str) -> usize {
    let Some(list) = all.get_mut(world) else {
        return 0;
    };
    let before = list.len();
    list.retain(|w| !w.name.eq_ignore_ascii_case(name.trim()));
    let removed = before - list.len();
    if list.is_empty() {
        all.remove(world);
    }
    removed
}

pub fn new(name: &str, x: i32, y: i32, z: i32, dim: Option<&str>) -> Waypoint {
    Waypoint {
        name: name.to_owned(),
        x,
        y,
        z,
        dim: dim.unwrap_or("overworld").to_owned(),
        color: DEFAULT_COLOR,
        shown: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_game_writes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(
            path(dir.path()),
            r#"{"sp:My World": [{"name": "Home", "x": 1, "y": 64, "z": -3, "dim": "overworld", "color": -8531972, "shown": true}],
               "mc.example.net": [{"name": "Base", "x": 100, "y": 70, "z": 5}]}"#,
        )
        .unwrap();
        let all = load(dir.path()).unwrap();
        assert_eq!(all["sp:My World"][0].name, "Home");
        assert_eq!(all["mc.example.net"][0].dim, "overworld");
        assert!(all["mc.example.net"][0].shown);
    }

    #[test]
    fn add_remove_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut all = load(dir.path()).unwrap();
        add(&mut all, "sp:W", new(" Mine ", 1, 2, 3, None)).unwrap();
        assert!(add(&mut all, "sp:W", new("mine", 0, 0, 0, None)).is_err());
        assert!(add(&mut all, "", new("x", 0, 0, 0, None)).is_err());
        save(dir.path(), &all).unwrap();
        let mut back = load(dir.path()).unwrap();
        assert_eq!(back["sp:W"][0].name, "Mine");
        assert_eq!(remove(&mut back, "sp:W", "MINE"), 1);
        assert!(back.is_empty());
    }

    #[test]
    fn a_full_world_refuses_more() {
        let mut all = ByWorld::new();
        for i in 0..MAX_PER_WORLD {
            add(&mut all, "w", new(&format!("p{i}"), 0, 0, 0, None)).unwrap();
        }
        assert!(add(&mut all, "w", new("one more", 0, 0, 0, None)).is_err());
    }
}
