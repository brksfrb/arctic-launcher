//! The Performance switch: well-known optimization mods (Sodium, Lithium,
//! …) added to Vanilla instances, which run on Fabric underneath, and to
//! Fabric and Quilt instances (leaving out any the player already has, or
//! has something in place of). They're downloaded from Modrinth for the
//! exact version rather than bundled, and tracked in their own index so user
//! mods are never touched.

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
    ("nmDcB62a", "ModernFix"),
    ("AANobbMI", "Sodium"),
    ("gvQqBUqZ", "Lithium"),
    ("uXXizFIs", "FerriteCore"),
    ("5ZwdcRci", "ImmediatelyFast"),
    ("NNAgCjsB", "EntityCulling"),
];

/// The mod ids (as their jars declare them) each one goes by, then mods that
/// do the same job in its place: with any of them already in a modded
/// instance, it's left out.
fn provided_by(project: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match project {
        "nmDcB62a" => (&["modernfix"], &[]),
        "AANobbMI" => (&["sodium"], &["optifabric", "canvas", "embeddium", "rubidium"]),
        "gvQqBUqZ" => (&["lithium"], &["canary", "radium"]),
        "uXXizFIs" => (&["ferritecore"], &[]),
        "5ZwdcRci" => (&["immediatelyfast"], &[]),
        "NNAgCjsB" => (&["entityculling"], &[]),
        super::packs::IRIS => (&["iris"], &["optifabric", "oculus"]),
        _ => (&[], &[]),
    }
}

/// Ids of the mods in the folder the player put there (not this switch's).
fn players_mod_ids(mods_dir: &Path, ours: &[String]) -> std::collections::HashSet<String> {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Default::default();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name.ends_with(".jar") && !ours.contains(&name)).then(|| e.path())
        })
        .filter_map(|path| crate::migrate::detect::mod_identity(&path).map(|(id, _)| id))
        .collect()
}

/// Leave out of `wanted` what the player already has; take out of the
/// folder ours they've since added themselves. Returns whether anything of
/// ours was taken out.
fn leave_players_mods(game_dir: &Path, wanted: &mut Vec<(&str, &str)>) -> Result<bool> {
    let index = index_path(game_dir);
    let mods_dir = game_dir.join("mods");
    let ours = ModIndex::load(&index)?;
    let our_files: Vec<String> = ours.mods.iter().map(|m| m.file_name.clone()).collect();
    let theirs = players_mod_ids(&mods_dir, &our_files);
    if theirs.is_empty() {
        return Ok(false);
    }
    let has = |project: &str| {
        let (ids, replacements) = provided_by(project);
        ids.iter().chain(replacements).any(|id| theirs.contains(*id))
    };
    wanted.retain(|(project, name)| {
        let keep = !has(project);
        if !keep {
            log::info!("{name}: the instance already has it (or a mod in its place); not added");
        }
        keep
    });
    let mut index_now = ours.clone();
    let mut removed = false;
    for m in &ours.mods {
        if has(&m.project_id) {
            super::files::delete(&mods_dir, &m.file_name)?;
            index_now = index_now.without_file(&m.file_name);
            removed = true;
        }
    }
    if removed {
        index_now.save(&index)?;
    }
    Ok(removed)
}

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

/// Where another game version's set waits while this one is in use, so
/// switching back is a move instead of a new download.
fn stash_dir(game_dir: &Path, game_version: &str, shaders: bool) -> PathBuf {
    let key = if shaders {
        format!("{game_version}-shaders")
    } else {
        game_version.to_owned()
    };
    game_dir.join(".arctic").join("performance").join(key)
}

/// Move the current set (files, index, state) out of the way, into its stash.
fn stash(game_dir: &Path, state: &State) -> Result<()> {
    let index = index_path(game_dir);
    if state.game_version.is_empty() || !index.exists() {
        return Ok(());
    }
    let to = stash_dir(game_dir, &state.game_version, state.shaders);
    if to.exists() {
        std::fs::remove_dir_all(&to).map_err(|e| crate::Error::io(&to, e))?;
    }
    std::fs::create_dir_all(&to).map_err(|e| crate::Error::io(&to, e))?;
    let mods_dir = game_dir.join("mods");
    for m in ModIndex::load(&index)?.mods {
        let from = mods_dir.join(&m.file_name);
        if from.exists() {
            std::fs::rename(&from, to.join(&m.file_name))
                .map_err(|e| crate::Error::io(&from, e))?;
        }
    }
    for file in [index, state_path(game_dir)] {
        if let Some(name) = file.file_name() {
            std::fs::rename(&file, to.join(name)).map_err(|e| crate::Error::io(&file, e))?;
        }
    }
    Ok(())
}

/// Bring back a stashed set for this version if it's complete and recent;
/// true when it's in place.
fn unstash(game_dir: &Path, game_version: &str, shaders: bool) -> Result<bool> {
    let from = stash_dir(game_dir, game_version, shaders);
    let index = from.join("arctic-performance.json");
    let Some(state) = load_json::<State>(&from.join("arctic-performance-state.json"))? else {
        return Ok(false);
    };
    if !index.exists()
        || crate::auth::now_secs().saturating_sub(state.checked) >= RECHECK_SECS
        || !all_present(&index, &from)?
    {
        return Ok(false);
    }
    let mods_dir = game_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| crate::Error::io(&mods_dir, e))?;
    for m in ModIndex::load(&index)?.mods {
        let file = from.join(&m.file_name);
        std::fs::rename(&file, mods_dir.join(&m.file_name))
            .map_err(|e| crate::Error::io(&file, e))?;
    }
    std::fs::rename(&index, index_path(game_dir)).map_err(|e| crate::Error::io(&index, e))?;
    let state_file = from.join("arctic-performance-state.json");
    std::fs::rename(&state_file, state_path(game_dir))
        .map_err(|e| crate::Error::io(&state_file, e))?;
    let _ = std::fs::remove_dir_all(&from);
    Ok(true)
}

/// Install (or update) the performance mods for `game_version` (and Iris
/// when `shaders` is on), or remove them when neither is wanted. Offline,
/// mods already installed for this version are kept.
/// `modded`: an instance with its own Fabric/Quilt mods, where those the
/// player already has (or something in their place) are left out.
pub fn sync(
    game_dir: &Path,
    game_version: &str,
    enabled: bool,
    shaders: bool,
    modded: bool,
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
    if modded && state.game_version == game_version && state.shaders == shaders {
        leave_players_mods(game_dir, &mut wanted)?;
        if wanted.is_empty() {
            return Ok(());
        }
    }
    let mods_dir = game_dir.join("mods");
    let index = index_path(game_dir);
    if state.game_version != game_version || state.shaders != shaders {
        // Another version's set: keep it for later, and bring back this one's if we have it.
        stash(game_dir, &state)?;
        remove_all(game_dir)?;
        if unstash(game_dir, game_version, shaders)? {
            return Ok(());
        }
    } else if crate::auth::now_secs().saturating_sub(state.checked) < RECHECK_SECS
        && all_present(&index, &mods_dir)?
    {
        return Ok(());
    }
    if modded {
        leave_players_mods(game_dir, &mut wanted)?;
        if wanted.is_empty() {
            return Ok(());
        }
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

        sync(dir.path(), "26.3", false, false, false, &|_| {}).unwrap();
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
        sync(dir.path(), "26.3", true, false, false, &|_| {}).unwrap();
        assert!(mods.join("sodium.jar").exists());
    }

    /// Put a checked set for `version` in place, as a finished install would.
    fn installed_set(game_dir: &Path, version: &str, file: &str) {
        let mods = game_dir.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join(file), version.as_bytes()).unwrap();
        ModIndex::default()
            .with(tracked(file))
            .save(&index_path(game_dir))
            .unwrap();
        let state = State {
            game_version: version.into(),
            shaders: false,
            checked: crate::auth::now_secs(),
        };
        save_json(&state_path(game_dir), &state).unwrap();
    }

    #[test]
    fn switching_versions_back_reuses_the_earlier_set() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        installed_set(dir.path(), "26.2", "sodium-26.2.jar");
        // To 26.3: the 26.2 set is stashed (26.3's has to be downloaded; fake it).
        stash(
            dir.path(),
            &load_json(&state_path(dir.path())).unwrap().unwrap(),
        )
        .unwrap();
        assert!(!mods.join("sodium-26.2.jar").exists());
        installed_set(dir.path(), "26.3", "sodium-26.3.jar");
        // Back to 26.2 through sync: no network, the stashed set returns.
        sync(dir.path(), "26.2", true, false, false, &|_| {}).unwrap();
        assert_eq!(
            std::fs::read(mods.join("sodium-26.2.jar")).unwrap(),
            b"26.2"
        );
        assert!(!mods.join("sodium-26.3.jar").exists());
        assert!(
            stash_dir(dir.path(), "26.3", false)
                .join("sodium-26.3.jar")
                .exists()
        );
        let state: State = load_json(&state_path(dir.path())).unwrap().unwrap();
        assert_eq!(state.game_version, "26.2");
    }
}
