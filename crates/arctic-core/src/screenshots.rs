//! Screenshots from every instance, newest first. Deleting moves one to
//! `screenshots/.trash` (never erased outright).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::instances;
use crate::storage::DataDirs;
use crate::{Error, Result};

const TRASH: &str = ".trash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    pub instance_id: String,
    pub instance_name: String,
    pub taken: SystemTime,
    pub size: u64,
}

/// Every instance's screenshots, newest first.
pub fn list(dirs: &DataDirs) -> Vec<Shot> {
    let mut all = Vec::new();
    let mut instances: Vec<instances::Instance> = instances::list_custom(dirs).unwrap_or_default();
    if let Ok(vanilla) = instances::load_default(dirs) {
        instances.insert(0, vanilla);
    }
    for instance in instances {
        let dir = instance.game_dir(dirs).join("screenshots");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if !is_image(&path) {
                continue;
            }
            let Ok(meta) = e.metadata() else {
                continue;
            };
            all.push(Shot {
                taken: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                size: meta.len(),
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                path,
            });
        }
    }
    all.sort_by_key(|s| std::cmp::Reverse(s.taken));
    all
}

/// Where the title screen's panorama sits inside a client jar.
const PANORAMA: &str = "assets/minecraft/textures/gui/title/background/panorama_0.png";
/// Smaller than this is a placeholder (26.x jars carry a 1×1 panorama).
const PLACEHOLDER_BYTES: usize = 1024;
/// Screenshots bigger than this aren't used as a backdrop (odd files).
const MAX_BACKDROP_BYTES: u64 = 40 * 1024 * 1024;

/// A picture of the player's own Minecraft for the launcher's backdrop: the
/// newest screenshot in `game_dir`, or else the title panorama from the
/// installed `version`'s jar. Image file bytes (PNG/JPEG), and whether it's
/// the panorama (mostly sky: best cropped to the horizon).
pub fn backdrop(
    dirs: &DataDirs,
    game_dir: &Path,
    version: Option<&str>,
) -> Option<(Vec<u8>, bool)> {
    let newest = std::fs::read_dir(game_dir.join("screenshots"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_image(&e.path()))
        .filter_map(|e| Some((e.metadata().ok()?, e.path())))
        .filter(|(m, _)| m.len() <= MAX_BACKDROP_BYTES)
        .max_by_key(|(m, _)| m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
        .and_then(|(_, path)| std::fs::read(path).ok());
    newest
        .map(|b| (b, false))
        .or_else(|| panorama(dirs, version?).map(|b| (b, true)))
}

/// The title panorama's front face: from the version's downloaded assets
/// (newer versions only ship a placeholder in the jar), else from its jar.
fn panorama(dirs: &DataDirs, version: &str) -> Option<Vec<u8>> {
    panorama_from_assets(dirs, version).or_else(|| panorama_from_jar(dirs, version))
}

fn panorama_from_assets(dirs: &DataDirs, version: &str) -> Option<Vec<u8>> {
    let dir = dirs.version_dir(version);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{version}.json"))).ok()?).ok()?;
    let index_id = json.get("assetIndex")?.get("id")?.as_str()?;
    let index: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            dirs.assets()
                .join("indexes")
                .join(format!("{index_id}.json")),
        )
        .ok()?,
    )
    .ok()?;
    let hash = index
        .get("objects")?
        .get(PANORAMA.trim_start_matches("assets/"))?
        .get("hash")?
        .as_str()?;
    let object = dirs
        .assets()
        .join("objects")
        .join(hash.get(..2)?)
        .join(hash);
    let bytes = std::fs::read(object).ok()?;
    // A real picture, not a placeholder pixel.
    (bytes.len() > PLACEHOLDER_BYTES).then_some(bytes)
}

fn panorama_from_jar(dirs: &DataDirs, version: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let jar = dirs.version_dir(version).join(format!("{version}.jar"));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut entry = zip.by_name(PANORAMA).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    (bytes.len() > PLACEHOLDER_BYTES).then_some(bytes)
}

fn is_image(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
}

/// Move a screenshot to its folder's `.trash`.
pub fn trash(shot: &Shot) -> Result<()> {
    let dir = shot
        .path
        .parent()
        .ok_or_else(|| Error::Other("that screenshot has no folder".into()))?
        .join(TRASH);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let name = shot.path.file_name().unwrap_or_default();
    std::fs::rename(&shot.path, dir.join(name)).map_err(|e| Error::io(&shot.path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_newest_first_and_trashes() {
        let root = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(root.path().to_path_buf());
        let vanilla = instances::load_default(&dirs).unwrap();
        let shots = vanilla.game_dir(&dirs).join("screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        std::fs::write(shots.join("a.png"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(shots.join("b.png"), b"x").unwrap();
        std::fs::write(shots.join("notes.txt"), b"x").unwrap();
        let found = list(&dirs);
        assert_eq!(found.len(), 2);
        assert!(found[0].path.ends_with("b.png"));
        trash(&found[0]).unwrap();
        assert_eq!(list(&dirs).len(), 1);
        assert!(shots.join(".trash/b.png").exists());
    }
}
