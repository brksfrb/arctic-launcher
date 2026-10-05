//! Replays the Arctic Client kept in an instance: `replay_recordings/*.mcpr`
//! (ReplayMod's format, plus Arctic's `arctic.json` and a thumbnail).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::error::IoContext;
use crate::{Error, Result};

/// Folder (in the game directory) the game saves replays to.
pub const FOLDER: &str = "replay_recordings";
const TRASH: &str = ".trash";
/// Metadata entries are small; anything bigger isn't one of ours.
const MAX_META_BYTES: u64 = 64 * 1024;
const MAX_THUMB_BYTES: u64 = 2 * 1024 * 1024;

/// A saved replay, for the list.
#[derive(Debug, Clone)]
pub struct Replay {
    pub path: PathBuf,
    /// The file name without `.mcpr`.
    pub name: String,
    /// The server's address, or the world's name.
    pub server: String,
    pub recorded: Option<SystemTime>,
    pub duration: Duration,
    pub mc_version: String,
    /// Moments marked while playing (each a 2D clip's end).
    pub clips: usize,
    pub size: u64,
    /// A small PNG of the moment it was kept, if there is one.
    pub thumbnail: Option<Vec<u8>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    #[serde(default)]
    server_name: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    date: u64,
    #[serde(default)]
    mcversion: String,
    #[serde(default)]
    singleplayer: bool,
}

#[derive(Deserialize, Default)]
struct Extras {
    #[serde(default)]
    moments: Vec<i64>,
}

/// Where the game writes the session it is recording (deleted when the session
/// ends; only a crash or a kill leaves it behind).
const TEMP: &str = ".arctic/replay-temp";

/// Empty the unfinished recordings in `game_dir`'s temp folder; the bytes freed.
/// Only call it when no game is running in that folder.
pub fn clear_temp(game_dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(game_dir.join(TEMP)) else {
        return 0;
    };
    let mut freed = 0;
    for entry in entries.flatten() {
        let size = entry.metadata().map_or(0, |m| m.len());
        if fs::remove_file(entry.path()).is_ok() {
            freed += size;
        }
    }
    freed
}

/// Every game folder of every profile's instances (for sweeps).
fn all_game_dirs(dirs: &crate::storage::DataDirs) -> Vec<PathBuf> {
    let mut roots = vec![dirs.instances()];
    if let Ok(profiles) = fs::read_dir(dirs.profiles_dir()) {
        roots.extend(profiles.flatten().map(|p| p.path().join("instances")));
    }
    roots.sort();
    roots.dedup();
    roots
        .iter()
        .filter_map(|root| fs::read_dir(root).ok())
        .flat_map(|entries| entries.flatten())
        .map(|instance| instance.path().join("minecraft"))
        .collect()
}

/// At launcher start nothing the launcher started is running: clear every
/// instance's leftover recordings. Returns the bytes freed.
pub fn clear_all_temp(dirs: &crate::storage::DataDirs) -> u64 {
    all_game_dirs(dirs).iter().map(|dir| clear_temp(dir)).sum()
}

/// The replays in `game_dir`, newest first (unreadable files are skipped).
pub fn list(game_dir: &Path) -> Vec<Replay> {
    let dir = game_dir.join(FOLDER);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Replay> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mcpr"))
        .filter_map(|p| read(&p).ok())
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.recorded));
    out
}

/// Read one replay's details.
pub fn read(path: &Path) -> Result<Replay> {
    let file = fs::File::open(path).at(path)?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{}: {e}", path.display())))?;
    let meta: Meta = serde_json::from_slice(
        &entry(&mut zip, "metaData.json", MAX_META_BYTES)?
            .ok_or_else(|| Error::Other(format!("{} is not a replay", path.display())))?,
    )?;
    let extras: Extras = entry(&mut zip, "arctic.json", MAX_META_BYTES)?
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let thumbnail = entry(&mut zip, "arctic/thumb.png", MAX_THUMB_BYTES)?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Replay {
        path: path.to_path_buf(),
        name,
        server: if meta.singleplayer && meta.server_name.is_empty() {
            "Singleplayer".into()
        } else {
            meta.server_name
        },
        recorded: (meta.date > 0).then(|| UNIX_EPOCH + Duration::from_millis(meta.date)),
        duration: Duration::from_millis(meta.duration),
        mc_version: meta.mcversion,
        clips: extras.moments.len(),
        size,
        thumbnail,
    })
}

fn entry(zip: &mut zip::ZipArchive<fs::File>, name: &str, max: u64) -> Result<Option<Vec<u8>>> {
    let Ok(file) = zip.by_name(name) else {
        return Ok(None);
    };
    if file.size() > max {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(file.size() as usize);
    file.take(max)
        .read_to_end(&mut out)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(Some(out))
}

/// Move a replay to `replay_recordings/.trash` (kept, not deleted).
pub fn trash(replay: &Replay) -> Result<()> {
    let dir = replay
        .path
        .parent()
        .map(|p| p.join(TRASH))
        .ok_or_else(|| Error::Other("replay has no folder".into()))?;
    fs::create_dir_all(&dir).at(&dir)?;
    let to = dir.join(replay.path.file_name().unwrap_or_default());
    fs::rename(&replay.path, &to).at(&to)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn leftover_recordings_are_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join(TEMP);
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("a.tmcpr"), vec![0u8; 1000]).unwrap();
        fs::write(temp.join("a.self"), vec![0u8; 24]).unwrap();
        // Saved replays are not touched.
        let saved = dir.path().join(FOLDER);
        fs::create_dir_all(&saved).unwrap();
        fs::write(saved.join("keep.mcpr"), b"x").unwrap();
        assert_eq!(clear_temp(dir.path()), 1024);
        assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
        assert!(saved.join("keep.mcpr").exists());
        assert_eq!(clear_temp(&dir.path().join("nothing")), 0);
    }

    #[test]
    fn every_profiles_instances_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let dirs = crate::storage::DataDirs::new(root.path());
        for game in [
            root.path().join("instances/a/minecraft"),
            root.path().join("profiles/p/instances/b/minecraft"),
        ] {
            let temp = game.join(TEMP);
            fs::create_dir_all(&temp).unwrap();
            fs::write(temp.join("x.tmcpr"), vec![0u8; 500]).unwrap();
        }
        assert_eq!(clear_all_temp(&dirs), 1000);
    }

    fn write_replay(dir: &Path, name: &str, meta: &str, extras: Option<&str>) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("metaData.json", opts).unwrap();
        zip.write_all(meta.as_bytes()).unwrap();
        if let Some(extras) = extras {
            zip.start_file("arctic.json", opts).unwrap();
            zip.write_all(extras.as_bytes()).unwrap();
        }
        zip.start_file("recording.tmcpr", opts).unwrap();
        zip.finish().unwrap();
        path
    }

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arctic-replays-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(dir.join(FOLDER)).unwrap();
        dir
    }

    #[test]
    fn lists_replays_newest_first_with_their_details() {
        let game = temp();
        let folder = game.join(FOLDER);
        write_replay(
            &folder,
            "old.mcpr",
            r#"{"serverName":"play.example.net","duration":65000,"date":1000,"mcversion":"26.3"}"#,
            Some(r#"{"moments":[1000,2000]}"#),
        );
        write_replay(
            &folder,
            "new.mcpr",
            r#"{"singleplayer":true,"duration":1000,"date":5000,"mcversion":"1.21.4"}"#,
            None,
        );
        fs::write(folder.join("notes.txt"), "x").unwrap();
        let list = list(&game);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "new");
        assert_eq!(list[0].server, "Singleplayer");
        assert_eq!(list[0].clips, 0);
        assert_eq!(list[1].server, "play.example.net");
        assert_eq!(list[1].duration, Duration::from_secs(65));
        assert_eq!(list[1].clips, 2);
        assert_eq!(list[1].mc_version, "26.3");
        fs::remove_dir_all(game).ok();
    }

    #[test]
    fn files_that_are_not_replays_are_skipped() {
        let game = temp();
        fs::write(game.join(FOLDER).join("broken.mcpr"), "not a zip").unwrap();
        assert!(list(&game).is_empty());
        fs::remove_dir_all(game).ok();
    }

    #[test]
    fn trash_keeps_the_file() {
        let game = temp();
        let path = write_replay(&game.join(FOLDER), "a.mcpr", r#"{"date":1}"#, None);
        let replay = read(&path).unwrap();
        trash(&replay).unwrap();
        assert!(!path.exists());
        assert!(game.join(FOLDER).join(TRASH).join("a.mcpr").exists());
        assert!(list(&game).is_empty());
        fs::remove_dir_all(game).ok();
    }
}
