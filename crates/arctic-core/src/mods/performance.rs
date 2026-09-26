//! The Performance switch: well-known optimization mods (Sodium, Lithium,
//! …) added to Vanilla instances, which run on Fabric underneath. They're
//! downloaded from Modrinth for the exact version rather than bundled, and
//! tracked in their own index so user mods are never touched.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::index::ModIndex;
use crate::Progress;
use crate::Result;
use crate::loaders::LoaderKind;
use crate::storage::{load_json, save_json};

/// (Modrinth project id, name). Each is skipped if it has no build yet
/// for the version; required dependencies come along automatically.
pub const MODS: &[(&str, &str)] = &[
    ("AANobbMI", "Sodium"),
    ("gvQqBUqZ", "Lithium"),
    ("uXXizFIs", "FerriteCore"),
    ("5ZwdcRci", "ImmediatelyFast"),
    ("NNAgCjsB", "EntityCulling"),
];

/// Look for new builds (and mods that were missing) at most this often.
const RECHECK_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    game_version: String,
    /// Iris was included (shaders on).
    #[serde(default)]
    shaders: bool,
    /// Unix seconds of the last successful check.
    checked: u64,
}

fn index_path(game_dir: &Path) -> PathBuf {
    game_dir.join("arctic-performance.json")
}

fn state_path(game_dir: &Path) -> PathBuf {
    game_dir.join("arctic-performance-state.json")
}

/// Install (or update) the performance mods for `game_version` (and Iris
/// when `shaders` is on), or remove them when neither is wanted. Offline,
/// mods already installed for this version are kept.
pub fn sync(
    game_dir: &Path,
    game_version: &str,
    enabled: bool,
    shaders: bool,
    progress: Progress,
) -> Result<()> {
    let state: State = load_json(&state_path(game_dir))?.unwrap_or_default();
    let mut wanted: Vec<(&str, &str)> = if enabled { MODS.to_vec() } else { Vec::new() };
    if shaders {
        wanted.push((super::packs::IRIS, "Iris"));
    }
    if wanted.is_empty() {
        return remove_all(game_dir);
    }
    let mods_dir = game_dir.join("mods");
    let index = index_path(game_dir);
    if state.game_version != game_version || state.shaders != shaders {
        remove_all(game_dir)?;
    } else if crate::auth::now_secs().saturating_sub(state.checked) < RECHECK_SECS
        && all_present(&index, &mods_dir)?
    {
        return Ok(());
    }
    let ids: Vec<&str> = wanted.iter().map(|(id, _)| *id).collect();
    let (installed, failed) = super::install_many(
        &ids,
        game_version,
        LoaderKind::Fabric,
        &mods_dir,
        &index,
        progress,
    )?;
    let mut reached = !installed.is_empty();
    for (id, e) in failed {
        let name = wanted
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(id.as_str(), |(_, n)| n);
        match e {
            crate::Error::Http(e) => log::info!("{name}: can't reach Modrinth: {e}"),
            e => {
                // Usually "no version for this Minecraft yet".
                reached = true;
                log::info!("{name} skipped for {game_version}: {e}");
            }
        }
    }
    if reached {
        save_json(
            &state_path(game_dir),
            &State {
                game_version: game_version.to_owned(),
                shaders,
                checked: crate::auth::now_secs(),
            },
        )?;
    }
    Ok(())
}

fn all_present(index: &Path, mods_dir: &Path) -> Result<bool> {
    let tracked = ModIndex::load(index)?;
    Ok(tracked
        .mods
        .iter()
        .all(|m| super::files::exists(mods_dir, &m.file_name)))
}

/// Delete every mod the Performance switch installed.
fn remove_all(game_dir: &Path) -> Result<()> {
    let index = index_path(game_dir);
    if !index.exists() {
        return Ok(());
    }
    let mods_dir = game_dir.join("mods");
    for m in ModIndex::load(&index)?.mods {
        super::files::delete(&mods_dir, &m.file_name)?;
    }
    std::fs::remove_file(&index).map_err(|e| crate::Error::io(&index, e))?;
    let state = state_path(game_dir);
    if state.exists() {
        std::fs::remove_file(&state).map_err(|e| crate::Error::io(&state, e))?;
    }
    Ok(())
}

/// Names of the performance mods currently installed in `game_dir`.
pub fn installed(game_dir: &Path) -> Vec<String> {
    ModIndex::load(&index_path(game_dir))
        .map(|i| {
            i.mods
                .into_iter()
                .filter(|m| !m.dependency)
                .map(|m| m.title)
                .collect()
        })
        .unwrap_or_default()
}

/// File names of the performance mods in `game_dir` (they're managed by
/// the switch, not by the player).
pub fn installed_files(game_dir: &Path) -> Vec<String> {
    ModIndex::load(&index_path(game_dir))
        .map(|i| i.mods.into_iter().map(|m| m.file_name).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::InstalledMod;

    fn tracked(file: &str) -> InstalledMod {
        InstalledMod {
            project_id: "p".into(),
            version_id: "v".into(),
            title: "Sodium".into(),
            version_number: "1".into(),
            file_name: file.into(),
            icon_url: None,
            dependency: false,
        }
    }

    #[test]
    fn turning_off_removes_only_its_own_mods() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium.jar"), b"x").unwrap();
        std::fs::write(mods.join("mine.jar"), b"y").unwrap();
        ModIndex::default()
            .with(tracked("sodium.jar"))
            .save(&index_path(dir.path()))
            .unwrap();
        assert_eq!(installed(dir.path()), vec!["Sodium".to_owned()]);

        sync(dir.path(), "26.3", false, false, &|_| {}).unwrap();
        assert!(!mods.join("sodium.jar").exists());
        assert!(mods.join("mine.jar").exists());
        assert!(installed(dir.path()).is_empty());
    }

    #[test]
    fn recent_check_for_the_same_version_skips_the_network() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium.jar"), b"x").unwrap();
        ModIndex::default()
            .with(tracked("sodium.jar"))
            .save(&index_path(dir.path()))
            .unwrap();
        let state = State {
            game_version: "26.3".into(),
            shaders: false,
            checked: crate::auth::now_secs(),
        };
        save_json(&state_path(dir.path()), &state).unwrap();
        // Would fail (and log) if it tried Modrinth with a fake version.
        sync(dir.path(), "26.3", true, false, &|_| {}).unwrap();
        assert!(mods.join("sodium.jar").exists());
    }
}
