//! What a profile's instances share: the server list, the Arctic Client's
//! settings (HUD, crosshair, keys, waypoints) and Minecraft's settings
//! (keys, video, sound...). Kept in step whenever a game starts and after
//! it quits: the newest copy wins and goes to every other instance.
//!
//! Minecraft's settings go through [`Defaults`], which converts keys
//! between old and new games, so a 1.8.9 instance and a 26.x one can share
//! them. The first time a file of an instance is replaced it's kept next to
//! it as `*.before-sharing`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::game_defaults::Defaults;
use crate::instances::{self, Instance};
use crate::storage::DataDirs;
use crate::{Error, Result};

/// Where the shared copies live, in the profile's folder.
const DIR: &str = "shared";
/// Arctic Client files shared as they are (paths in the game folder).
const CLIENT_FILES: [&str; 3] = [
    "config/arctic.json",
    "config/arctic-crosshair.png",
    "config/arctic-waypoints.json",
];
const OPTIONS: &str = "options.txt";
const SHARED_OPTIONS: &str = "options.json";
const BACKUP: &str = ".before-sharing";

/// What this profile's instances share (all on by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SharedSettings {
    /// The multiplayer server list.
    pub servers: bool,
    /// Arctic Client: HUD, crosshair, keys, looks settings, waypoints.
    pub client: bool,
    /// Minecraft's own settings: keys, video, sound, FOV...
    pub game: bool,
}

impl Default for SharedSettings {
    fn default() -> Self {
        Self {
            servers: true,
            client: true,
            game: true,
        }
    }
}

impl SharedSettings {
    fn any(self) -> bool {
        self.servers || self.client || self.game
    }
}

/// Bring every instance in step. `launching`: the instance about to start
/// and its Minecraft version (Minecraft's settings are written for it).
/// Games that are running aren't written to; they're picked up after.
pub fn sync(
    dirs: &DataDirs,
    shared: SharedSettings,
    launching: Option<(&Instance, &str)>,
) -> Result<()> {
    if !shared.any() {
        return Ok(());
    }
    let mut all = vec![instances::load_default(dirs)?];
    all.extend(instances::list_custom(dirs)?);
    // Instances that keep their own settings neither give nor get any.
    all.retain(|i| !i.own_settings);
    let folders: Vec<GameFolder> = all
        .iter()
        .map(|i| {
            let dir = i.game_dir(dirs);
            GameFolder {
                busy: crate::sharing::client::in_use(&dir),
                dir,
            }
        })
        .collect();
    let store = dirs.profile_root().join(DIR);
    std::fs::create_dir_all(&store).map_err(|e| Error::io(&store, e))?;
    if shared.servers {
        let shared_servers = store.join(crate::servers::SERVERS_FILE);
        if !shared_servers.exists() {
            // First time: every instance's servers, none lost.
            let sources: Vec<PathBuf> = newest_first(&folders, crate::servers::SERVERS_FILE);
            crate::servers::merge_files(&sources, &shared_servers)?;
        }
        sync_file(&folders, &shared_servers, crate::servers::SERVERS_FILE)?;
    }
    if shared.client {
        for rel in CLIENT_FILES {
            let name = Path::new(rel).file_name().unwrap_or_default();
            sync_file(&folders, &store.join(name), rel)?;
        }
    }
    if shared.game {
        sync_options(
            &folders,
            &store.join(SHARED_OPTIONS),
            launching
                .filter(|(i, _)| !i.own_settings)
                .map(|(i, v)| (i.game_dir(dirs), v)),
        )?;
    }
    Ok(())
}

struct GameFolder {
    dir: PathBuf,
    /// The game is running there (its files aren't written).
    busy: bool,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Instances' copies of `rel`, newest first.
fn newest_first(folders: &[GameFolder], rel: &str) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = folders
        .iter()
        .map(|f| f.dir.join(rel))
        .filter_map(|p| Some((modified(&p)?, p)))
        .collect();
    found.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Newest copy wins: it becomes the shared one, then goes to every idle
/// instance that has something else.
fn sync_file(folders: &[GameFolder], shared: &Path, rel: &str) -> Result<()> {
    let mut best = std::fs::read(shared).ok();
    let mut best_time = modified(shared);
    for folder in folders {
        let path = folder.dir.join(rel);
        let Some(time) = modified(&path) else {
            continue;
        };
        if best_time.is_some_and(|b| time <= b) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if best.as_ref() != Some(&bytes) {
            best = Some(bytes);
            best_time = Some(time);
        }
    }
    let Some(best) = best else {
        return Ok(());
    };
    if std::fs::read(shared).ok().as_ref() != Some(&best) {
        write(shared, &best)?;
    }
    for folder in folders.iter().filter(|f| !f.busy) {
        let path = folder.dir.join(rel);
        let current = std::fs::read(&path).ok();
        if current.as_ref() == Some(&best) {
            continue;
        }
        if current.is_some() {
            keep_backup(&path)?;
        } else if !folder.dir.exists() {
            // An instance never started yet gets it on its first start.
            continue;
        }
        write(&path, &best)?;
    }
    Ok(())
}

/// Minecraft's settings: the newest `options.txt` (in any instance) is
/// remembered, and written into the instance about to start, converted
/// for its version.
fn sync_options(
    folders: &[GameFolder],
    shared: &Path,
    launching: Option<(PathBuf, &str)>,
) -> Result<()> {
    let mut known: Defaults = crate::storage::load_json(shared)?.unwrap_or_default();
    let shared_time = modified(shared);
    if let Some(newest) = newest_first(folders, OPTIONS).into_iter().next()
        && modified(&newest).is_some_and(|t| shared_time.is_none_or(|s| t > s))
        && let Ok(captured) = Defaults::capture(newest.parent().unwrap_or(Path::new(".")))
        && captured.values != known.values
        && !captured.is_empty()
    {
        known = captured;
        crate::storage::save_json(shared, &known)?;
    }
    let Some((game_dir, version)) = launching else {
        return Ok(());
    };
    if known.is_empty() || crate::sharing::client::in_use(&game_dir) {
        return Ok(());
    }
    let options = game_dir.join(OPTIONS);
    if options.exists() {
        keep_backup(&options)?;
    }
    known.apply_to(&game_dir, Some(version))?;
    Ok(())
}

/// The instance's own copy, the first time sharing replaces it.
fn keep_backup(path: &Path) -> Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(BACKUP);
    let backup = path.with_file_name(name);
    if !backup.exists() && path.exists() {
        std::fs::copy(path, &backup).map_err(|e| Error::io(&backup, e))?;
    }
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".arctic-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::Loader;

    fn setup() -> (tempfile::TempDir, DataDirs, Instance, Instance) {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(tmp.path());
        dirs.ensure().unwrap();
        let a = instances::load_default(&dirs).unwrap();
        let b = instances::create(&dirs, "B", "1.8.9", Loader::new(None, String::new())).unwrap();
        for i in [&a, &b] {
            std::fs::create_dir_all(i.game_dir(&dirs).join("config")).unwrap();
        }
        (tmp, dirs, a, b)
    }

    fn later() {
        // File times need to differ.
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    #[test]
    fn server_lists_merge_once_then_follow_the_newest() {
        let (_tmp, dirs, a, b) = setup();
        crate::servers::add(&a.game_dir(&dirs), "A", "a.example.net").unwrap();
        crate::servers::add(&b.game_dir(&dirs), "B", "b.example.net").unwrap();
        sync(&dirs, SharedSettings::default(), None).unwrap();
        let names = |i: &Instance| -> Vec<String> {
            crate::servers::list(&i.game_dir(&dirs))
                .unwrap()
                .into_iter()
                .map(|s| s.address)
                .collect()
        };
        assert_eq!(names(&a).len(), 2);
        assert_eq!(names(&a), names(&b));
        later();
        crate::servers::add(&b.game_dir(&dirs), "C", "c.example.net").unwrap();
        sync(&dirs, SharedSettings::default(), None).unwrap();
        assert!(names(&a).contains(&"c.example.net".to_owned()));
        // B's list before sharing was kept.
        assert!(
            b.game_dir(&dirs)
                .join("servers.dat.before-sharing")
                .exists()
                || a.game_dir(&dirs)
                    .join("servers.dat.before-sharing")
                    .exists()
        );
    }

    #[test]
    fn an_instance_with_its_own_settings_is_left_alone() {
        let (_tmp, dirs, a, mut b) = setup();
        b.own_settings = true;
        b.save(&dirs).unwrap();
        let file = |i: &Instance| i.game_dir(&dirs).join("config/arctic.json");
        std::fs::write(file(&a), "{\"a\":1}").unwrap();
        later();
        std::fs::write(file(&b), "{\"b\":1}").unwrap();
        sync(&dirs, SharedSettings::default(), Some((&b, "1.8.9"))).unwrap();
        assert_eq!(std::fs::read_to_string(file(&a)).unwrap(), "{\"a\":1}");
        assert_eq!(std::fs::read_to_string(file(&b)).unwrap(), "{\"b\":1}");
    }

    #[test]
    fn client_settings_follow_the_newest_and_skip_what_is_off() {
        let (_tmp, dirs, a, b) = setup();
        let file = |i: &Instance| i.game_dir(&dirs).join("config/arctic.json");
        std::fs::write(file(&a), "{\"old\":1}").unwrap();
        later();
        std::fs::write(file(&b), "{\"new\":1}").unwrap();
        let off = SharedSettings {
            client: false,
            ..SharedSettings::default()
        };
        sync(&dirs, off, None).unwrap();
        assert_eq!(std::fs::read_to_string(file(&a)).unwrap(), "{\"old\":1}");
        sync(&dirs, SharedSettings::default(), None).unwrap();
        assert_eq!(std::fs::read_to_string(file(&a)).unwrap(), "{\"new\":1}");
        assert_eq!(
            std::fs::read_to_string(a.game_dir(&dirs).join("config/arctic.json.before-sharing"))
                .unwrap(),
            "{\"old\":1}"
        );
    }

    #[test]
    fn minecraft_settings_go_to_the_starting_game_in_its_format() {
        let (_tmp, dirs, a, b) = setup();
        std::fs::write(
            a.game_dir(&dirs).join(OPTIONS),
            "fov:0.5\nkey_key.jump:key.keyboard.space\n",
        )
        .unwrap();
        sync(&dirs, SharedSettings::default(), Some((&b, "1.8.9"))).unwrap();
        let text = std::fs::read_to_string(b.game_dir(&dirs).join(OPTIONS)).unwrap();
        assert!(text.contains("fov:0.5"), "{text}");
        // 1.8.9 stores keys as LWJGL 2 codes.
        assert!(text.contains("key_key.jump:57"), "{text}");
    }
}
