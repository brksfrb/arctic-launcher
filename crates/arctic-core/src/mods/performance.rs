//! The Performance switch: well-known optimization mods (Sodium, Lithium,
//! …) added to Vanilla instances, which run on Fabric underneath, and to
//! Fabric and Quilt instances (leaving out any the player already has, or
//! has something in place of). They're downloaded from Modrinth for the
//! exact version rather than bundled, and tracked in their own index so user
//! mods are never touched.

mod dependencies;
mod settled;

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::index::ModIndex;
use crate::Progress;
use crate::Result;
use crate::error::IoContext;
use crate::loaders::LoaderKind;
use crate::storage::{load_json, save_json};

const ENTITY_CULLING: &str = "NNAgCjsB";

/// (Modrinth project id, name). Each is skipped if it has no build yet
/// for the version; required dependencies come along automatically.
pub const MODS: &[(&str, &str)] = &[
    ("nmDcB62a", "ModernFix"),
    ("AANobbMI", "Sodium"),
    ("gvQqBUqZ", "Lithium"),
    ("uXXizFIs", "FerriteCore"),
    ("5ZwdcRci", "ImmediatelyFast"),
    (ENTITY_CULLING, "EntityCulling"),
    ("RSeLon5O", "Particle Core"),
    ("g96Z4WVZ", "BadOptimizations"),
];

/// Extras the player can add to any Fabric/Quilt/Vanilla instance (Modrinth project id, name, what it
/// does): not part of the Performance set, each its own switch on the instance.
pub const OPTIONAL: &[(&str, &str, &str)] = &[(
    "kYuIpRLv",
    "Wavey Capes",
    "Capes that wave and flutter when you move.",
)];

/// The mod ids (as their jars declare them) each one goes by, then mods that
/// do the same job in its place: with any of them already in a modded
/// instance, it's left out.
fn provided_by(project: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match project {
        "nmDcB62a" => (&["modernfix"], &[]),
        "AANobbMI" => (
            &["sodium"],
            &["optifabric", "canvas", "embeddium", "rubidium"],
        ),
        "gvQqBUqZ" => (&["lithium"], &["canary", "radium"]),
        "uXXizFIs" => (&["ferritecore"], &[]),
        "5ZwdcRci" => (&["immediatelyfast"], &[]),
        ENTITY_CULLING => (&["entityculling"], &[]),
        "RSeLon5O" => (&["particle_core"], &[]),
        "g96Z4WVZ" => (&["badoptimizations"], &[]),
        dependencies::FABRIC_API => (&["fabric", "fabric-api", "quilted_fabric_api"], &[]),
        super::packs::IRIS => (&["iris"], &["optifabric", "oculus"]),
        "kYuIpRLv" => (&["waveycapes"], &[]),
        _ => (&[], &[]),
    }
}

pub(crate) fn performance_mod_name(id: &str) -> Option<&'static str> {
    MODS.iter()
        .find(|(project, _)| provided_by(project).0.contains(&id))
        .map(|(_, name)| *name)
}

pub(crate) fn is_performance_mod(id: &str) -> bool {
    performance_mod_name(id).is_some() || matches!(id, "fzzy_config" | "transition" | "trender")
}

fn supported_mods(game_version: &str) -> Vec<(&'static str, &'static str)> {
    // Modrinth's only EntityCulling build for 1.16 (Qek7k92S) is tagged 1.16.x.
    // Its jar accepts any 'fabric', but calls ClientTickEvents. The only Fabric API tagged
    // for 1.16 (RtAjPwHu) lacks that class even in the full jar. 1.16.1 has it.
    MODS.iter()
        .copied()
        .filter(|(project, _)| *project != ENTITY_CULLING || game_version != "1.16")
        .collect()
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
        ids.iter()
            .chain(replacements)
            .any(|id| theirs.contains(*id))
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

/// Forge 1.8.9, where Sodium and friends don't exist: the FPS mods that do (Modrinth project, name, and
/// words of a jar's file name that mean the player already has it). Patcher also fixes bugs and adds
/// quality-of-life tweaks; Entity Culling stops drawing what can't be seen.
pub const FORGE_189: &[(&str, &str, &[&str])] = &[
    ("YknNc5nN", "Patcher", &["patcher"]),
    (
        ENTITY_CULLING,
        "EntityCulling",
        &["entityculling", "entity_culling", "entity-culling"],
    ),
];

/// The only Forge game version with these mods (and with the Arctic Client for Forge).
pub fn forge_supported(game_version: &str) -> bool {
    game_version == "1.8.9"
}

fn forge_index_path(game_dir: &Path) -> PathBuf {
    game_dir.join("arctic-performance-forge.json")
}

/// Does a jar named like this (lowercase) already do the job of one of `words`?
fn named_like(file_name: &str, words: &[&str]) -> bool {
    let name = file_name.to_ascii_lowercase();
    name.ends_with(".jar") && words.iter().any(|w| name.contains(w))
}

/// The Forge 1.8.9 FPS mods, installed when `enabled` and taken out when not; the player's own copies
/// are left as they are (and left alone). Installed once and kept: no update check at every start.
pub fn sync_forge(
    game_dir: &Path,
    game_version: &str,
    enabled: bool,
    progress: Progress,
) -> Result<()> {
    let mods_dir = game_dir.join("mods");
    let index = forge_index_path(game_dir);
    let ours = ModIndex::load(&index)?;
    restore_offline_patcher(game_dir, &ours)?;
    if !enabled || !forge_supported(game_version) {
        if ours.mods.is_empty() {
            return Ok(());
        }
        for m in &ours.mods {
            super::files::delete(&mods_dir, &m.file_name)?;
        }
        return std::fs::remove_file(&index).map_err(|e| crate::Error::io(&index, e));
    }
    let our_files: Vec<&str> = ours.mods.iter().map(|m| m.file_name.as_str()).collect();
    let theirs: Vec<String> = std::fs::read_dir(&mods_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| !our_files.contains(&name.as_str()))
        .collect();
    let mut wanted: Vec<&str> = Vec::new();
    for (project, name, words) in FORGE_189 {
        if ours
            .by_project(project)
            .is_some_and(|m| super::files::exists(&mods_dir, &m.file_name))
        {
            continue;
        }
        if theirs.iter().any(|file| named_like(file, words)) {
            log::info!("{name}: the instance already has it; not added");
            continue;
        }
        wanted.push(*project);
    }
    if !wanted.is_empty() {
        // A failed EntityCulling install must not bypass OneConfig preparation for
        // an already-installed Patcher (the caller otherwise logs and launches).
        match super::install_many(
            &wanted,
            game_version,
            LoaderKind::Forge,
            &mods_dir,
            &index,
            progress,
        ) {
            Ok((_, failed)) => {
                for (id, e) in failed {
                    log::info!("{id} skipped for {game_version}: {e}");
                }
            }
            Err(e) => log::warn!("Forge performance mod installation failed: {e}"),
        }
    }
    if ForgeIndex(&index).has(PATCHER) {
        prepare_patcher(game_dir, || super::oneconfig::prepare(game_dir))?;
        tame_patcher(game_dir);
    }
    Ok(())
}

// Keep a skipped jar outside mods, separately from the player's .disabled choice.
fn offline_patcher(game_dir: &Path, file: &str) -> PathBuf {
    game_dir.join("arctic-patcher-offline").join(file)
}

fn restore_offline_patcher(game_dir: &Path, index: &ModIndex) -> Result<()> {
    use crate::error::IoContext;
    if let Some(m) = index.by_project(PATCHER) {
        super::files::check_file_name(&m.file_name)?;
        let parked = offline_patcher(game_dir, &m.file_name);
        let mods = game_dir.join("mods");
        if parked.is_file() && !super::files::exists(&mods, &m.file_name) {
            std::fs::create_dir_all(&mods).at(&mods)?;
            std::fs::rename(&parked, mods.join(&m.file_name)).at(&parked)?;
        }
    }
    Ok(())
}

fn prepare_patcher(game_dir: &Path, fetch: impl FnOnce() -> Result<()>) -> Result<()> {
    use crate::error::IoContext;
    let index = ModIndex::load(&forge_index_path(game_dir))?;
    let Some(m) = index.by_project(PATCHER) else {
        return Ok(());
    };
    super::files::check_file_name(&m.file_name)?;
    let jar = game_dir.join("mods").join(&m.file_name);
    if !jar.is_file() {
        return Ok(()); // Respect a Patcher the player disabled.
    }
    if let Err(e) = fetch() {
        let parked = offline_patcher(game_dir, &m.file_name);
        std::fs::create_dir_all(parked.parent().unwrap()).at(&parked)?;
        std::fs::rename(&jar, &parked).at(&jar)?;
        log::warn!(
            "Patcher: OneConfig preparation failed ({e}); launching without Arctic's Patcher this run; will retry next launch"
        );
    }
    Ok(())
}

/// Modrinth project of Patcher.
const PATCHER: &str = "YknNc5nN";

/// Is a project in our Forge index?
struct ForgeIndex<'a>(&'a Path);

impl ForgeIndex<'_> {
    fn has(&self, project: &str) -> bool {
        ModIndex::load(self.0).is_ok_and(|i| i.by_project(project).is_some())
    }
}

/// OneConfig's key for its menu (LWJGL 2 key code) when nothing changed it: Right Shift, which is also
/// Arctic's menu key (both opened, one after the other); and the key it is given here: Insert.
const ONECONFIG_KEY: u64 = 54;
const ONECONFIG_KEY_INSTEAD: u64 = 210;

/// Patcher brings OneConfig, and two of their defaults clash with the Arctic Client: OneConfig's menu
/// opens on Right Shift (Arctic's too), and Patcher's own font renderer replaces the game's, so
/// Arctic's fancy font never draws. Moved aside once, as soon as the game has written both files
/// (its first start); after that the player's choices stay.
fn tame_patcher(game_dir: &Path) {
    let marker = game_dir.join("arctic-patcher-settings.json");
    if marker.exists() {
        return;
    }
    let one_config = game_dir.join("OneConfig");
    let preferences = one_config.join("Preferences.json");
    let patcher = one_config
        .join("profiles")
        .join("Default Profile")
        .join("patcher.json");
    if !preferences.exists() || !patcher.exists() {
        return;
    }
    let done = (|| -> Result<()> {
        edit_json(&preferences, |json| {
            let keys = json.pointer_mut("/oneConfigKeyBind/keyBinds")?;
            let only_shift = keys
                .as_array()
                .is_some_and(|k| k.len() == 1 && k[0] == ONECONFIG_KEY);
            only_shift.then(|| *keys = serde_json::json!([ONECONFIG_KEY_INSTEAD]))
        })?;
        edit_json(&patcher, |json| {
            let object = json.as_object_mut()?;
            let mut changed = false;
            for key in ["optimizedFontRenderer", "cacheFontData"] {
                if object
                    .get(key)
                    .is_some_and(|v| v == &serde_json::Value::Bool(true))
                {
                    object.insert(key.to_owned(), serde_json::Value::Bool(false));
                    changed = true;
                }
            }
            changed.then_some(())
        })?;
        save_json(
            &marker,
            &serde_json::json!({"menu_key": "Insert", "font_renderer": false}),
        )
    })();
    if let Err(e) = done {
        log::warn!("couldn't adjust Patcher's settings: {e}");
    }
}

/// Read a JSON file, let `change` edit it (returning `Some` when it did), write it back if so.
fn edit_json<T>(
    path: &Path,
    change: impl FnOnce(&mut serde_json::Value) -> Option<T>,
) -> Result<()> {
    let text = std::fs::read_to_string(path).map_err(|e| crate::Error::io(path, e))?;
    let mut json: serde_json::Value = serde_json::from_str(&text)?;
    if change(&mut json).is_some() {
        let out = serde_json::to_string_pretty(&json)?;
        std::fs::write(path, out).map_err(|e| crate::Error::io(path, e))?;
    }
    Ok(())
}

/// Look for new builds (and mods that were missing) at most this often.
const RECHECK_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    game_version: String,
    /// Iris was included (shaders on).
    #[serde(default)]
    shaders: bool,
    /// The extras included (their project ids, sorted, joined by "+").
    #[serde(default)]
    extras: String,
    /// Unix seconds of the last successful check.
    checked: u64,
    /// A background check found newer builds: the next start installs them.
    #[serde(default)]
    update_waiting: bool,
    /// Modrinth had no build of any of these for the version (project ids joined by "+"), so
    /// nothing is installed: kept across a switch to another version and back, so that coming
    /// back doesn't ask for each of them again before the weekly look.
    #[serde(default)]
    none_for: String,
}

fn index_path(game_dir: &Path) -> PathBuf {
    game_dir.join("arctic-performance.json")
}

fn state_path(game_dir: &Path) -> PathBuf {
    game_dir.join("arctic-performance-state.json")
}

/// A recheck can run during a version switch. Serialize its state write with
/// sync, keeping its network request outside the lock.
fn lock(game_dir: &Path) -> Result<File> {
    let dir = game_dir.join(".arctic");
    fs::create_dir_all(&dir).at(&dir)?;
    let path = dir.join("performance.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .at(&path)?;
    file.lock().at(&path)?;
    Ok(file)
}

/// Where another game version's set waits while this one is in use, so
/// switching back can reuse the files already on disk.
fn stash_dir(game_dir: &Path, game_version: &str, shaders: bool, extras: &str) -> PathBuf {
    let mut key = if shaders {
        format!("{game_version}-shaders")
    } else {
        game_version.to_owned()
    };
    if !extras.is_empty() {
        key = format!("{key}-{extras}");
    }
    game_dir.join(".arctic").join("performance").join(key)
}

/// Move the current set (files, index, state) out of the way, into its stash.
fn stash(game_dir: &Path, state: &State) -> Result<()> {
    let index = index_path(game_dir);
    if state.game_version.is_empty() || (!index.exists() && state.none_for.is_empty()) {
        return Ok(());
    }
    let to = stash_dir(game_dir, &state.game_version, state.shaders, &state.extras);
    if !index.exists() {
        // The folder is named after what the state file says: nothing is removed unless that
        // is one plain name inside the stash folder (a state file can come with a pack).
        let stashes = game_dir.join(".arctic").join("performance");
        let plain = to.parent() == Some(stashes.as_path())
            && to
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(plain_name);
        if !plain {
            return Ok(());
        }
        // Nothing was installed: only that Modrinth was asked. A set left here from before
        // (too old to be used again, or incomplete) must not pass for this one's.
        if to.exists() {
            fs::remove_dir_all(&to).at(&to)?;
        }
        fs::create_dir_all(&to).at(&to)?;
        return save_json(&to.join("arctic-performance-state.json"), state);
    }
    // Keep files already moved if an earlier attempt was interrupted.
    fs::create_dir_all(&to).at(&to)?;
    let mods_dir = game_dir.join("mods");
    for m in ModIndex::load(&index)?.mods {
        super::files::check_file_name(&m.file_name)?;
        for name in [m.file_name.clone(), format!("{}.disabled", m.file_name)] {
            let from = mods_dir.join(&name);
            if from.is_file() {
                fs::rename(&from, to.join(&name)).at(&from)?;
            }
        }
    }
    save_json(&to.join("arctic-performance-state.json"), state)?;
    fs::rename(&index, to.join("arctic-performance.json")).at(&index)
}

/// One folder name and nothing that leads elsewhere.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control())
}

/// Bring back a stashed set for this version if it's complete and recent;
/// true when it's in place. `none_for`: the projects wanted now, for the stash of a version
/// none of them had a build for ([`State::none_for`]); `None` where that isn't kept.
fn unstash(
    game_dir: &Path,
    game_version: &str,
    shaders: bool,
    extras: &str,
    none_for: Option<&str>,
) -> Result<bool> {
    let from = stash_dir(game_dir, game_version, shaders, extras);
    let index = from.join("arctic-performance.json");
    let Some(state) = load_json::<State>(&from.join("arctic-performance-state.json"))? else {
        return Ok(false);
    };
    if crate::auth::now_secs().saturating_sub(state.checked) >= RECHECK_SECS {
        return Ok(false);
    }
    if !index.exists() {
        if state.none_for.is_empty() || none_for != Some(state.none_for.as_str()) {
            return Ok(false);
        }
        save_json(&state_path(game_dir), &state)?;
        let _ = fs::remove_dir_all(&from);
        return Ok(true);
    }
    if !all_present(&index, &from)? {
        return Ok(false);
    }
    let mods_dir = game_dir.join("mods");
    fs::create_dir_all(&mods_dir).at(&mods_dir)?;
    let tracked = ModIndex::load(&index)?;
    // Keep the complete cache until every file is restored. Publish ownership
    // first so a retry can clear any files already restored.
    tracked.save(&index_path(game_dir))?;
    for m in tracked.mods {
        super::files::check_file_name(&m.file_name)?;
        let mut name = m.file_name;
        if !from.join(&name).is_file() {
            name.push_str(".disabled");
        }
        let file = from.join(&name);
        let part = mods_dir.join(format!("{name}.part"));
        fs::copy(&file, &part).at(&file)?;
        // The same file as before it was put away, time included: what was worked out from
        // it then (which Fabric API modules it uses) still holds.
        if let Ok(modified) = fs::metadata(&file).and_then(|meta| meta.modified()) {
            let _ = File::options()
                .write(true)
                .open(&part)
                .and_then(|copy| copy.set_modified(modified));
        }
        fs::rename(&part, mods_dir.join(&name)).at(&part)?;
    }
    save_json(&state_path(game_dir), &state)?;
    let _ = fs::remove_dir_all(&from);
    Ok(true)
}

/// Install (or update) the performance mods for `game_version` (and Iris
/// when `shaders` is on), or remove them when neither is wanted. Offline,
/// mods already installed for this version are kept.
/// An explicit Fabric/Quilt loader also means the instance may have player
/// mods. Leave out anything the player already provides.
pub fn sync(
    game_dir: &Path,
    game_version: &str,
    enabled: bool,
    shaders: bool,
    extras: &[String],
    loader: Option<LoaderKind>,
    progress: Progress,
) -> Result<()> {
    sync_with_install(
        SyncTarget {
            game_dir,
            game_version,
            loader,
            launcher_owned_vanilla: false,
            progress,
        },
        enabled,
        shaders,
        extras,
        install_wanted,
    )
    .map(|_| ())
}

/// [`sync`] for a start. `Ok(true)`: everything wanted was already there and in order, found
/// without asking Modrinth (so the same folder and settings give the same answer again).
pub(crate) fn sync_for_launch(
    game_dir: &Path,
    game_version: &str,
    instance: &crate::instances::Instance,
    progress: Progress,
) -> Result<bool> {
    let loader = instance.loader.kind();
    sync_with_install(
        SyncTarget {
            game_dir,
            game_version,
            loader,
            launcher_owned_vanilla: loader.is_none() && instance.game_folder.is_none(),
            progress,
        },
        instance.performance,
        instance.shaders,
        &instance.extras,
        install_wanted,
    )
}

/// Everything the Performance switch does to a Fabric or Quilt game folder before a start: its
/// mods from Modrinth, Polarium, and for Vanilla instances the Fabric API cut down to what those
/// use. Best effort (no Sodium is better than no game: an unreachable Modrinth CDN, a locked or
/// truncated index), except for what would stop Fabric from starting.
///
/// All of it reads every jar in the folder more than once. A start that found nothing to change
/// is remembered with everything it depended on (see [`settled`]), and the next start with the
/// same folder, settings, game version and launcher skips it.
pub(crate) fn prepare_for_launch(
    game_dir: &Path,
    game_version: &str,
    instance: &crate::instances::Instance,
    launcher_owned_vanilla: bool,
    progress: Progress,
) -> Result<()> {
    let started = std::time::Instant::now();
    let settled = settled::Settled::new(game_dir, game_version, instance, launcher_owned_vanilla);
    let before = settled.inputs();
    if settled.holds(before.as_ref()) {
        log::debug!("performance mods: nothing changed since the last start");
        return Ok(());
    }
    // Only a run that needed nothing from Modrinth gives the same answer next time.
    let mut complete = match sync_for_launch(game_dir, game_version, instance, progress) {
        Ok(in_order) => in_order,
        Err(e) => {
            log::warn!("performance mods: {e}");
            false
        }
    };
    let synced = started.elapsed();
    let mods_dir = game_dir.join("mods");
    if let Err(e) = super::polarium::sync(&mods_dir, game_version, instance.performance) {
        log::warn!("polarium: {e}");
        complete = false;
    }
    // A copy that could not be put aside this time is tried again at the next start.
    complete &= !super::polarium::unusable_left(&mods_dir, game_version);
    let polarium = started.elapsed();
    // Vanilla instances (nothing in the folder but the launcher's own mods): load only
    // the Fabric API modules those use, which starts the game noticeably faster.
    if instance.loader.kind().is_none() {
        // A cut-down that failed is tried again at the next start.
        complete &= finish_vanilla(
            game_dir,
            game_version,
            launcher_owned_vanilla,
            instance.performance,
        )?;
    }
    let total = started.elapsed();
    if total >= SLOW_PREPARE {
        log::info!(
            "performance mods took {} ms: Modrinth set and its dependencies {}, Polarium {}, Fabric API {}",
            total.as_millis(),
            synced.as_millis(),
            (polarium - synced).as_millis(),
            (total - polarium).as_millis()
        );
    }
    if complete {
        settled.remember(before);
    }
    Ok(())
}

/// From here the parts of [`prepare_for_launch`] are worth a line in the log.
const SLOW_PREPARE: std::time::Duration = std::time::Duration::from_millis(100);

struct SyncTarget<'a> {
    game_dir: &'a Path,
    game_version: &'a str,
    loader: Option<LoaderKind>,
    launcher_owned_vanilla: bool,
    progress: Progress<'a>,
}

fn sync_with_install(
    target: SyncTarget<'_>,
    enabled: bool,
    shaders: bool,
    extras: &[String],
    install: impl FnOnce(
        &SyncTarget<'_>,
        &[&str],
    ) -> Result<(Vec<super::InstalledMod>, Vec<super::Failed>)>,
) -> Result<bool> {
    let SyncTarget {
        game_dir,
        game_version,
        loader,
        launcher_owned_vanilla,
        progress,
    } = target;
    let _lock = lock(game_dir)?;
    clear_pending(game_dir)?;
    let state: State = load_json(&state_path(game_dir))?.unwrap_or_default();
    let mods_dir = game_dir.join("mods");
    let mut wanted = if enabled {
        supported_mods(game_version)
    } else {
        Vec::new()
    };
    if shaders {
        wanted.push((super::packs::IRIS, "Iris"));
    }
    // The chosen extras that exist, in a fixed order (so the same choice is the same key).
    let chosen: Vec<&(&str, &str, &str)> = OPTIONAL
        .iter()
        .filter(|(id, _, _)| extras.iter().any(|e| e == id))
        .collect();
    wanted.extend(chosen.iter().map(|(id, name, _)| (*id, *name)));
    let extras_key = chosen
        .iter()
        .map(|(id, _, _)| *id)
        .collect::<Vec<_>>()
        .join("+");
    if wanted.is_empty() {
        return remove_all(game_dir).map(|()| true);
    }
    let modded = loader.is_some();
    let loader = loader.unwrap_or(LoaderKind::Fabric);
    // In an instance with the player's own mods what is asked for depends on those too.
    let none_key = (!modded).then(|| {
        let ids: Vec<&str> = wanted.iter().map(|(id, _)| *id).collect();
        ids.join("+")
    });
    let check = || {
        // The old set has already been stashed. Park any incompatible leftovers
        // before deciding which active jars need or provide dependencies.
        if launcher_owned_vanilla {
            park_incompatible_vanilla(game_dir, game_version)?;
        }
        dependencies::check(game_dir, game_version, loader, progress)
    };
    let same_set = state.game_version == game_version
        && state.shaders == shaders
        && state.extras == extras_key;
    let index = index_path(game_dir);
    let restored = if same_set {
        false
    } else {
        // Another version's set: keep it for later, and bring back this one's if we have it.
        super::slim_api::undo(&mods_dir)?;
        stash(game_dir, &state)?;
        remove_all(game_dir)?;
        unstash(
            game_dir,
            game_version,
            shaders,
            &extras_key,
            none_key.as_deref(),
        )?
    };
    if game_version == "1.16" {
        if let Some(m) = ModIndex::load(&index)?.by_project(ENTITY_CULLING) {
            super::slim_api::undo(&mods_dir)?;
            super::remove(&mods_dir, &index, &m.file_name)?;
        }
        if enabled {
            log::info!(
                "EntityCulling skipped for Minecraft 1.16: compatible Fabric API lacks lifecycle events v1"
            );
        }
    }
    if modded && (same_set || restored) {
        if same_set {
            super::slim_api::undo(&mods_dir)?;
        }
        leave_players_mods(game_dir, &mut wanted)?;
    }
    if restored || wanted.is_empty() {
        return Ok(check()? && !restored);
    }
    if same_set && !state.update_waiting && all_present(&index, &mods_dir)? {
        let due = crate::auth::now_secs().saturating_sub(state.checked) >= RECHECK_SECS;
        if due {
            // Due for a look at Modrinth: the game starts with what is here meanwhile,
            // and only a real update is installed (at the next start).
            let ids: Vec<String> = wanted.iter().map(|(id, _)| (*id).to_owned()).collect();
            recheck_later(game_dir.to_path_buf(), ids, game_version.to_owned(), state);
        }
        return Ok(check()? && !due);
    }
    // New or changed mods are about to be written: Fabric API goes back to the whole one first.
    super::slim_api::undo(&mods_dir)?;
    if modded {
        leave_players_mods(game_dir, &mut wanted)?;
        if wanted.is_empty() {
            check()?;
            return Ok(false);
        }
    }
    let ids: Vec<&str> = wanted.iter().map(|(id, _)| *id).collect();
    let install = install(&target, &ids);
    let checked = check();
    let (installed, failed) = install?;
    checked?;
    let mut reached = !installed.is_empty();
    let mut only_no_build = true;
    for (id, e) in failed {
        let name = wanted
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(id.as_str(), |(_, n)| n);
        match e {
            crate::Error::Http(e) => {
                only_no_build = false;
                log::info!("{name}: can't reach Modrinth: {e}");
            }
            e => {
                only_no_build &= super::resolve::is_no_version(&e);
                // Usually "no version for this Minecraft yet".
                reached = true;
                log::info!("{name} skipped for {game_version}: {e}");
            }
        }
    }
    if reached {
        // Modrinth said of every project that it has no build: worth keeping across a version
        // switch (an answer that could not be had or read is asked for again).
        let nothing = only_no_build && !index.exists();
        save_json(
            &state_path(game_dir),
            &State {
                game_version: game_version.to_owned(),
                shaders,
                extras: extras_key,
                checked: crate::auth::now_secs(),
                update_waiting: false,
                none_for: none_key.filter(|_| nothing).unwrap_or_default(),
            },
        )?;
    }
    Ok(false)
}

fn install_wanted(
    target: &SyncTarget<'_>,
    ids: &[&str],
) -> Result<(Vec<super::InstalledMod>, Vec<super::Failed>)> {
    let SyncTarget {
        game_dir,
        game_version,
        loader,
        progress,
        ..
    } = *target;
    let mods_dir = game_dir.join("mods");
    let current = ModIndex::load(&index_path(game_dir))?;
    let mut existing = super::installed_projects(&current, &mods_dir);
    if loader.is_some() {
        let our_files: Vec<String> = current.mods.iter().map(|m| m.file_name.clone()).collect();
        let player_ids = players_mod_ids(&mods_dir, &our_files);
        for project in MODS
            .iter()
            .map(|(id, _)| *id)
            .chain([dependencies::FABRIC_API])
        {
            if provided_by(project)
                .0
                .iter()
                .any(|id| player_ids.contains(*id))
            {
                existing.insert(project.to_owned());
            }
        }
    }
    progress(crate::ProgressInfo::stage("Finding mods"));
    let (plan, failed) = super::plan_many(ids, game_version, LoaderKind::Fabric, &existing);
    let installed = dependencies::install_plan(&plan, &current, game_dir, progress, |_| Ok(()))?;
    Ok((installed, failed))
}

fn pending_path(game_dir: &Path) -> PathBuf {
    game_dir.join(".arctic").join("performance-pending.json")
}

/// Some downloads can succeed before another fails. Remember new jars until
/// the install publishes its index, so interrupted installs leave no orphans.
fn record_pending(game_dir: &Path, mods: &[super::InstalledMod], current: &ModIndex) -> Result<()> {
    let mods_dir = game_dir.join("mods");
    let files: Vec<_> = mods
        .iter()
        .map(|m| &m.file_name)
        .filter(|name| current.by_file(name).is_none() && !super::files::exists(&mods_dir, name))
        .collect();
    save_json(&pending_path(game_dir), &files)
}

fn clear_pending(game_dir: &Path) -> Result<()> {
    let path = pending_path(game_dir);
    let Some(files) = load_json::<Vec<String>>(&path)? else {
        return Ok(());
    };
    let current = ModIndex::load(&index_path(game_dir))?;
    for name in files {
        if current.by_file(&name).is_none() {
            super::files::delete(&game_dir.join("mods"), &name)?;
        }
    }
    fs::remove_file(&path).at(&path)
}

fn all_present(index: &Path, mods_dir: &Path) -> Result<bool> {
    let tracked = ModIndex::load(index)?;
    let present = present_in(mods_dir);
    Ok(tracked.mods.iter().all(|m| present(&m.file_name)))
}

/// Is a file of the index there? Fabric API counts while it is cut down.
fn present_in(mods_dir: &Path) -> impl Fn(&str) -> bool {
    let stashed = super::slim_api::stashed(mods_dir);
    let mods_dir = mods_dir.to_path_buf();
    move |name| super::files::exists(&mods_dir, name) || stashed.as_deref() == Some(name)
}

/// Ask Modrinth for newer builds in the background (it can take seconds).
/// Nothing new: the set counts as checked. Something new: the check stays
/// due, so the next start installs it.
fn recheck_later(game_dir: PathBuf, ids: Vec<String>, game_version: String, state: State) {
    std::thread::spawn(move || {
        let mods_dir = game_dir.join("mods");
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let waiting = super::updates_waiting(
            &ids,
            &game_version,
            LoaderKind::Fabric,
            &mods_dir,
            &index_path(&game_dir),
            &present_in(&mods_dir),
        );
        match waiting {
            Ok(Some(waiting)) => {
                if let Err(e) = note_recheck(&game_dir, state, waiting) {
                    log::warn!("performance mods: couldn't note the check: {e}");
                }
            }
            Ok(None) => {
                log::info!("performance mods: Modrinth not reachable, checking again later")
            }
            Err(e) => log::warn!("performance mods: couldn't check for updates: {e}"),
        }
    });
}

fn note_recheck(game_dir: &Path, mut checked: State, waiting: bool) -> Result<()> {
    let _lock = lock(game_dir)?;
    let path = state_path(game_dir);
    if load_json::<State>(&path)?.as_ref() != Some(&checked) {
        return Ok(());
    }
    checked.update_waiting = waiting;
    if waiting {
        log::info!("performance mods: updates found, installed at the next start");
    } else {
        checked.checked = crate::auth::now_secs();
    }
    save_json(&path, &checked)
}

/// Delete every mod the Performance switch installed.
fn remove_all(game_dir: &Path) -> Result<()> {
    let index = index_path(game_dir);
    super::slim_api::undo(&game_dir.join("mods"))?;
    let mods_dir = game_dir.join("mods");
    if index.exists() {
        for m in ModIndex::load(&index)?.mods {
            super::files::delete(&mods_dir, &m.file_name)?;
        }
        fs::remove_file(&index).at(&index)?;
    }
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

/// Handle older, unrecorded API slimming before sync tries to move the set.
pub(crate) fn recover_vanilla_api(game_dir: &Path) -> Result<()> {
    let _lock = lock(game_dir)?;
    super::slim_api::recover_orphans(&game_dir.join("mods"))
}

pub(crate) fn finish_vanilla(
    game_dir: &Path,
    game_version: &str,
    launcher_owned: bool,
    slim_api: bool,
) -> Result<bool> {
    let _lock = lock(game_dir)?;
    if launcher_owned {
        park_incompatible_vanilla(game_dir, game_version)?;
    }
    let mods_dir = game_dir.join("mods");
    // `Ok(false)`: the Fabric API could not be cut down this time (the whole one is in place).
    let mut done = true;
    if slim_api && let Err(e) = super::slim_api::apply(&mods_dir) {
        log::warn!("couldn't cut Fabric API down: {e}");
        super::slim_api::undo(&mods_dir)?;
        done = false;
    }
    Ok(done)
}

/// The state and index can disagree after a crash or a launch from an older
/// Arctic build. Check the manifests before Fabric sees the active jars.
fn park_incompatible_vanilla(game_dir: &Path, game_version: &str) -> Result<()> {
    let mods_dir = game_dir.join("mods");
    let mut stale = incompatible_jars(game_dir, game_version)?;
    if stale.is_empty() {
        return Ok(());
    }
    if super::slim_api::stashed_jar(&mods_dir).is_some() {
        super::slim_api::undo(&mods_dir)?;
        stale = incompatible_jars(game_dir, game_version)?;
    }
    if stale.is_empty() {
        return Ok(());
    }
    let index = index_path(game_dir);
    let mut tracked = ModIndex::load(&index).unwrap_or_default();
    let old_count = tracked.mods.len();
    let mut jars: Vec<_> = stale
        .iter()
        .map(|jar| (jar.clone(), "incompatible Minecraft version"))
        .collect();
    // A stale set can include libraries with no Minecraft predicate of their own.
    // If every root is gone, its tracked dependencies have nothing left to serve.
    let roots = || {
        tracked
            .mods
            .iter()
            .filter(|m| !m.dependency)
            .map(|m| mods_dir.join(&m.file_name))
    };
    if roots().any(|root| stale.contains(&root))
        && !roots().any(|root| root.is_file() && !stale.contains(&root))
    {
        for dep in tracked.mods.iter().filter(|m| m.dependency) {
            super::files::check_file_name(&dep.file_name)?;
            let jar = mods_dir.join(&dep.file_name);
            if jar.is_file() && !stale.contains(&jar) {
                jars.push((jar, "dependency of the stale Performance set"));
            }
        }
    }
    let aside = game_dir
        .join(".arctic/performance-incompatible")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&aside).at(&aside)?;
    for (jar, reason) in jars {
        let Some(name) = jar.file_name() else {
            continue;
        };
        fs::rename(&jar, aside.join(name)).at(&jar)?;
        tracked.mods.retain(|m| m.file_name.as_str() != name);
        log::warn!(
            "performance mods: moved {} aside before launching Minecraft {game_version} ({reason})",
            name.to_string_lossy()
        );
    }
    if tracked.mods.len() < old_count
        && let Err(e) = tracked.save(&index)
    {
        log::warn!(
            "performance mods: couldn't update the index after moving stale jars aside: {e}"
        );
    }
    Ok(())
}

fn incompatible_jars(game_dir: &Path, game_version: &str) -> Result<Vec<PathBuf>> {
    let mods_dir = game_dir.join("mods");
    let files = match fs::read_dir(&mods_dir) {
        Ok(files) => files,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::Error::io(&mods_dir, e)),
    };
    let indexed = installed_files(game_dir);
    let generated = super::slim_api::generated_files(&mods_dir);
    let parked = super::slim_api::stashed_jar(&mods_dir);
    let mut jars = Vec::new();
    for jar in files
        .map(|e| e.map(|e| e.path()).at(&mods_dir))
        .chain(parked.into_iter().map(Ok))
    {
        let jar = jar?;
        let Some(name) = jar.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if super::files::check_file_name(name).is_err()
            || crate::migrate::ranges::game_fits(&jar, game_version, LoaderKind::Fabric)
                != Some(false)
        {
            continue;
        }
        let managed = indexed.iter().chain(&generated).any(|file| file == name)
            || matches!(
                name,
                crate::arctic_mod::FILE_NAME | super::polarium::FILE_NAME
            )
            || crate::migrate::detect::mod_identity(&jar).is_some_and(|(id, _)| {
                is_performance_mod(&id)
                    || matches!(
                        id.as_str(),
                        "iris"
                            | "waveycapes"
                            | "fabric-api"
                            | "fabric"
                            | "fabric-language-kotlin"
                            | "fabric-renderer-api-v1"
                    )
            });
        if managed {
            jars.push(jar);
        }
    }
    Ok(jars)
}

#[cfg(test)]
mod tests {
    #[test]
    fn patchers_clashing_defaults_are_changed_once() {
        let dir = tempfile::tempdir().unwrap();
        let one = dir.path().join("OneConfig");
        let profile = one.join("profiles").join("Default Profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            one.join("Preferences.json"),
            r#"{"oneConfigKeyBind":{"keyBinds":[54]},"enableBlur":true}"#,
        )
        .unwrap();
        std::fs::write(
            profile.join("patcher.json"),
            r#"{"optimizedFontRenderer":true,"cacheFontData":true,"shiftChat":false}"#,
        )
        .unwrap();
        super::tame_patcher(dir.path());
        let prefs: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(one.join("Preferences.json")).unwrap())
                .unwrap();
        assert_eq!(prefs["oneConfigKeyBind"]["keyBinds"][0], 210);
        assert_eq!(prefs["enableBlur"], true);
        let patcher: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(profile.join("patcher.json")).unwrap())
                .unwrap();
        assert_eq!(patcher["optimizedFontRenderer"], false);
        assert_eq!(patcher["cacheFontData"], false);
        assert_eq!(patcher["shiftChat"], false);
        // The player turns the font renderer back on: it stays that way.
        std::fs::write(
            profile.join("patcher.json"),
            r#"{"optimizedFontRenderer":true}"#,
        )
        .unwrap();
        super::tame_patcher(dir.path());
        assert!(
            std::fs::read_to_string(profile.join("patcher.json"))
                .unwrap()
                .contains("true")
        );
    }

    #[test]
    fn forge_jars_are_told_by_their_names() {
        let words: &[&str] = &["patcher"];
        assert!(super::named_like("Patcher-1.8.9-1.8.2.jar", words));
        assert!(super::named_like("polypatcher-1.10.4.jar", words));
        assert!(!super::named_like("OptiFine_1.8.9_HD_U_M5.jar", words));
        assert!(!super::named_like("patcher-notes.txt", words));
        assert!(super::forge_supported("1.8.9"));
        assert!(!super::forge_supported("1.12.2"));
    }

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
    fn offline_patcher_is_skipped_then_retried_without_touching_player_mods() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        for file in ["patcher.jar", "my-patcher.jar", "entityculling.jar"] {
            std::fs::write(mods.join(file), file.as_bytes()).unwrap();
        }
        let mut patcher = tracked("patcher.jar");
        patcher.project_id = PATCHER.into();
        let index = ModIndex::default().with(patcher);
        index.save(&forge_index_path(game)).unwrap();
        prepare_patcher(game, || Err(crate::Error::Http("mock timeout".into()))).unwrap();
        assert!(!mods.join("patcher.jar").exists());
        assert!(offline_patcher(game, "patcher.jar").is_file());
        assert!(mods.join("my-patcher.jar").is_file());
        assert!(mods.join("entityculling.jar").is_file());
        assert_eq!(ModIndex::load(&forge_index_path(game)).unwrap(), index);
        restore_offline_patcher(game, &index).unwrap();
        let mut retried = false;
        prepare_patcher(game, || {
            retried = true;
            Ok(())
        })
        .unwrap();
        assert!(retried);
        assert!(mods.join("patcher.jar").is_file());
        assert!(!offline_patcher(game, "patcher.jar").exists());
        super::super::files::set_enabled(&mods, "patcher.jar", false).unwrap();
        prepare_patcher(game, || panic!("respect player's disabled mod")).unwrap();
        assert!(mods.join("patcher.jar.disabled").is_file());
        ModIndex::default().save(&forge_index_path(game)).unwrap();
        prepare_patcher(game, || panic!("never prepare a player-owned Patcher")).unwrap();
        assert!(mods.join("my-patcher.jar").is_file());
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

        sync(dir.path(), "26.3", false, false, &[], None, &|_| {}).unwrap();
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
            extras: String::new(),
            checked: crate::auth::now_secs(),
            update_waiting: false,
            none_for: String::new(),
        };
        save_json(&state_path(dir.path()), &state).unwrap();
        // Would fail (and log) if it tried Modrinth with a fake version.
        sync(dir.path(), "26.3", true, false, &[], None, &|_| {}).unwrap();
        assert!(mods.join("sodium.jar").exists());
    }

    #[test]
    fn a_due_check_does_not_hold_up_the_start() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium.jar"), b"x").unwrap();
        ModIndex::default()
            .with(tracked("sodium.jar"))
            .save(&index_path(dir.path()))
            .unwrap();
        let state = State {
            game_version: "0.0-test".into(),
            shaders: false,
            extras: String::new(),
            checked: 0,
            update_waiting: false,
            none_for: String::new(),
        };
        save_json(&state_path(dir.path()), &state).unwrap();
        let start = std::time::Instant::now();
        sync(dir.path(), "0.0-test", true, false, &[], None, &|_| {}).unwrap();
        // The set is used as it is; Modrinth is asked on another thread.
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
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
            extras: String::new(),
            checked: crate::auth::now_secs(),
            update_waiting: false,
            none_for: String::new(),
        };
        save_json(&state_path(game_dir), &state).unwrap();
    }

    #[test]
    fn crash_378_skips_entityculling_only_for_minecraft_1_16() {
        let selected = |game| {
            supported_mods(game)
                .iter()
                .any(|(id, _)| *id == ENTITY_CULLING)
        };
        assert!(!selected("1.16"));
        assert!(selected("1.16.1"));
        assert!(selected("1.16.5"));
    }

    #[test]
    fn crash_378_repairs_checked_and_stashed_performance_sets_offline() {
        for stashed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let game = dir.path();
            let mods = game.join("mods");
            installed_set(game, "1.16", "entityculling.jar");
            std::fs::write(mods.join("fabric-api-0.13.1.jar"), b"api").unwrap();
            let mut culling = tracked("entityculling.jar");
            culling.project_id = ENTITY_CULLING.into();
            let mut api = tracked("fabric-api-0.13.1.jar");
            api.project_id = "P7dR8mSH".into();
            api.dependency = true;
            let index = ModIndex::default().with(culling).with(api.clone());
            index.save(&index_path(game)).unwrap();
            if stashed {
                stash(game, &load_json(&state_path(game)).unwrap().unwrap()).unwrap();
                installed_set(game, "1.16.1", "sodium-1.16.1.jar");
            }
            std::fs::write(mods.join("player-entityculling.jar"), b"player").unwrap();

            sync(game, "1.16", true, false, &[], None, &|_| {
                panic!("the existing set is repaired offline")
            })
            .unwrap();
            assert!(!mods.join("entityculling.jar").exists());
            assert!(mods.join("player-entityculling.jar").is_file());
            assert!(mods.join("fabric-api-0.13.1.jar").is_file());
            assert_eq!(ModIndex::load(&index_path(game)).unwrap().mods, [api]);
            let state: State = load_json(&state_path(game)).unwrap().unwrap();
            assert_eq!(state.game_version, "1.16");
        }
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
        sync(dir.path(), "26.2", true, false, &[], None, &|_| {}).unwrap();
        assert_eq!(
            std::fs::read(mods.join("sodium-26.2.jar")).unwrap(),
            b"26.2"
        );
        assert!(!mods.join("sodium-26.3.jar").exists());
        assert!(
            stash_dir(dir.path(), "26.3", false, "")
                .join("sodium-26.3.jar")
                .exists()
        );
        let state: State = load_json(&state_path(dir.path())).unwrap().unwrap();
        assert_eq!(state.game_version, "26.2");
    }

    fn fabric_jar(id: &str, depends: serde_json::Value, nested: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        let opts = zip::write::SimpleFileOptions::default();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", opts).unwrap();
        let jars: Vec<_> = nested
            .iter()
            .map(|(file, _)| serde_json::json!({"file": file}))
            .collect();
        let meta = serde_json::json!({
            "schemaVersion": 1, "id": id, "version": "1", "depends": depends, "jars": jars,
        });
        zip.write_all(meta.to_string().as_bytes()).unwrap();
        for (name, bytes) in nested {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    const API: &str = "fabric-api-1.jar";
    const RENDERER: &str = "fabric-renderer-api-v1-1.jar";

    fn vanilla_target<'a>(game_dir: &'a Path, game_version: &'a str) -> SyncTarget<'a> {
        SyncTarget {
            game_dir,
            game_version,
            loader: None,
            launcher_owned_vanilla: true,
            progress: &|_| {},
        }
    }

    fn write_fabric_api(mods: &Path) {
        use serde_json::json;
        let mut names = vec!["META-INF/jars/fabric-renderer-api-v1-1.jar".into()];
        names.extend((0..8).map(|i| format!("META-INF/jars/fabric-unused-{i}-1.jar")));
        let modules: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let id = if i == 0 {
                    "fabric-renderer-api-v1".into()
                } else {
                    format!("fabric-unused-{i}")
                };
                (name.as_str(), fabric_jar(&id, json!({}), &[]))
            })
            .collect();
        fs::write(
            mods.join(API),
            fabric_jar("fabric-api", json!({"minecraft": "~1.21"}), &modules),
        )
        .unwrap();
    }

    #[test]
    fn switch_to_unsupported_legacy_fabric_and_back_after_a_late_recheck() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mods = game.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let old = State {
            game_version: "1.8.9".into(),
            ..State::default()
        };
        save_json(&state_path(game), &old).unwrap();

        let expected = ["sodium.jar", "fzzy-config.jar", API];
        sync_with_install(
            vanilla_target(game, "1.21.1"),
            true,
            false,
            &[],
            |_, ids| {
                assert!(ids.contains(&"AANobbMI"));
                fs::write(
                    mods.join("sodium.jar"),
                    fabric_jar(
                        "sodium",
                        json!({"minecraft": "1.21.x", "fabric-renderer-api-v1": "*"}),
                        &[],
                    ),
                )
                .unwrap();
                // A dependency may have no Minecraft requirement of its own.
                fs::write(
                    mods.join("fzzy-config.jar"),
                    fabric_jar("fzzy_config", json!({}), &[]),
                )
                .unwrap();
                write_fabric_api(&mods);
                let installed: Vec<_> = expected
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let mut entry = tracked(name);
                        entry.project_id = format!("project-{i}");
                        entry.dependency = i != 0;
                        entry
                    })
                    .collect();
                ModIndex {
                    mods: installed.clone(),
                }
                .save(&index_path(game))
                .unwrap();
                Ok((installed, Vec::new()))
            },
        )
        .unwrap();
        assert!(super::super::slim_api::apply(&mods).unwrap());
        assert!(mods.join(RENDERER).is_file());
        assert!(mods.join("fabric-api-stub.jar").is_file());
        assert!(!mods.join(API).exists());

        // Model the network result arriving after the 1.21.1 install finished.
        note_recheck(game, old, false).unwrap();
        let state: State = load_json(&state_path(game)).unwrap().unwrap();
        assert_eq!(state.game_version, "1.21.1");
        sync_with_install(vanilla_target(game, "1.8.9"), true, false, &[], |_, ids| {
            assert_eq!(ids.len(), MODS.len());
            assert!(
                super::super::files::list(&mods, &ModIndex::default())
                    .unwrap()
                    .is_empty()
            );
            Ok((
                Vec::new(),
                ids.iter()
                    .map(|id| {
                        (
                            id.to_string(),
                            crate::Error::Other("no version for this Minecraft".into()),
                        )
                    })
                    .collect(),
            ))
        })
        .unwrap();
        assert!(
            super::super::files::list(&mods, &ModIndex::default())
                .unwrap()
                .is_empty()
        );
        let stash = stash_dir(game, "1.21.1", false, "");
        assert!(expected.iter().all(|name| stash.join(name).is_file()));
        assert_eq!(installed_files(game), Vec::<String>::new());
        sync_with_install(vanilla_target(game, "1.8.9"), true, false, &[], |_, _| {
            panic!("empty set is checked")
        })
        .unwrap();
        sync_with_install(vanilla_target(game, "1.21.1"), true, false, &[], |_, _| {
            panic!("use the cached set")
        })
        .unwrap();
        assert!(expected.iter().all(|name| mods.join(name).is_file()));
        assert!(!mods.join(RENDERER).exists());
        assert_eq!(installed_files(game), expected);
    }

    #[test]
    fn vanilla_guard_recovers_untracked_old_jars_and_slim_api() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mods = game.join("mods");
        fs::create_dir_all(&mods).unwrap();
        for (name, id, depends) in [
            (
                "old.jar",
                "sodium",
                json!({"minecraft": ["1.21.1", "~1.21.4"], "fabric-renderer-api-v1": "*"}),
            ),
            ("my-mod.jar", "user", json!({"minecraft": "1.21.1"})),
            (
                "new.jar",
                "sodium",
                json!({"minecraft": ["1.21.1", "1.8.x"]}),
            ),
            (
                "indexed.jar",
                "indexed",
                json!({"minecraft": "1.21.1", "missing_lib": "*"}),
            ),
            ("dependency.jar", "library", json!({})),
        ] {
            fs::write(mods.join(name), fabric_jar(id, depends, &[])).unwrap();
        }
        write_fabric_api(&mods);
        assert!(super::super::slim_api::apply(&mods).unwrap());
        fs::write(mods.join(".arctic-slim-api.json"), b"{").unwrap();
        fs::remove_file(mods.join("fabric-api-stub.jar")).unwrap();
        let mut dependency = tracked("dependency.jar");
        dependency.project_id = "dependency".into();
        dependency.dependency = true;
        ModIndex::default()
            .with(tracked("indexed.jar"))
            .with(dependency)
            .save(&index_path(game))
            .unwrap();
        save_json(
            &state_path(game),
            &State {
                game_version: "1.8.9".into(),
                checked: crate::auth::now_secs(),
                ..State::default()
            },
        )
        .unwrap();
        recover_vanilla_api(game).unwrap();
        sync_with_install(vanilla_target(game, "1.8.9"), true, false, &[], |_, _| {
            panic!("state says the version is checked")
        })
        .unwrap();
        park_incompatible_vanilla(game, "1.8.9").unwrap();
        assert!(!mods.join(RENDERER).exists());
        assert!(!mods.join(API).exists());
        assert!(!mods.join("old.jar").exists());
        assert!(!mods.join("indexed.jar").exists());
        assert!(!mods.join("dependency.jar").exists());
        assert!(
            fs::read_dir(game.join(".arctic/performance-incompatible"))
                .unwrap()
                .any(|entry| entry.unwrap().path().join("indexed.jar").is_file())
        );
        assert!(mods.join("new.jar").is_file());
        assert!(mods.join("my-mod.jar").is_file());
    }

    #[test]
    fn interrupted_downloads_leave_no_untracked_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mods = game.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("user.jar"), b"player").unwrap();
        let mut dependency = tracked("dep.jar");
        dependency.dependency = true;
        record_pending(
            game,
            &[tracked("user.jar"), dependency],
            &ModIndex::default(),
        )
        .unwrap();
        fs::write(mods.join("dep.jar"), b"downloaded before the error").unwrap();
        sync(game, "1.8.9", false, false, &[], None, &|_| {}).unwrap();
        assert!(!mods.join("dep.jar").exists());
        assert!(mods.join("user.jar").is_file());
        assert!(!pending_path(game).exists());
    }

    fn no_build(
        _: &SyncTarget<'_>,
        ids: &[&str],
    ) -> Result<(Vec<super::super::InstalledMod>, Vec<super::super::Failed>)> {
        let failed = ids
            .iter()
            .map(|id| {
                let none = crate::Error::Other(format!("{id} has no version for Fabric 0"));
                ((*id).to_owned(), none)
            })
            .collect();
        Ok((Vec::new(), failed))
    }

    fn must_not_ask(
        _: &SyncTarget<'_>,
        _: &[&str],
    ) -> Result<(Vec<super::super::InstalledMod>, Vec<super::super::Failed>)> {
        panic!("Modrinth was asked again")
    }

    #[test]
    fn a_version_without_builds_is_not_asked_about_again_after_switching_back() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        type Install = fn(
            &SyncTarget<'_>,
            &[&str],
        )
            -> Result<(Vec<super::super::InstalledMod>, Vec<super::super::Failed>)>;
        let run = |version, shaders, install: Install| {
            sync_with_install(vanilla_target(game, version), true, shaders, &[], install)
        };
        // Asking Modrinth is never "everything was in order already".
        assert!(!run("1.8.9", false, no_build).unwrap());
        assert!(run("1.8.9", false, must_not_ask).unwrap());
        assert!(
            !load_json::<State>(&state_path(game))
                .unwrap()
                .unwrap()
                .none_for
                .is_empty()
        );

        assert!(!run("0.0-other", false, no_build).unwrap());
        assert!(stash_dir(game, "1.8.9", false, "").is_dir());
        // Back: the answer of a moment ago is used, once (the stash is taken out).
        assert!(!run("1.8.9", false, must_not_ask).unwrap());
        assert!(!stash_dir(game, "1.8.9", false, "").exists());
        assert!(run("1.8.9", false, must_not_ask).unwrap());

        // A set left in the stash from long ago is not taken for the "no build" answer's.
        run("0.0-other", false, no_build).unwrap();
        let stale = stash_dir(game, "1.8.9", false, "");
        fs::write(stale.join("old-sodium.jar"), b"old").unwrap();
        ModIndex::default()
            .with(tracked("old-sodium.jar"))
            .save(&stale.join("arctic-performance.json"))
            .unwrap();
        // (As when it was found too old on the way to 1.8.9: left there, and Modrinth asked.)
        fs::remove_file(stale.join("arctic-performance-state.json")).unwrap();
        assert!(!run("1.8.9", false, no_build).unwrap());
        assert!(stale.join("old-sodium.jar").is_file());
        run("0.0-other", false, no_build).unwrap();
        assert!(!stale.join("old-sodium.jar").exists());
        assert!(!run("1.8.9", false, must_not_ask).unwrap());
        assert!(!game.join("mods/old-sodium.jar").exists());

        // A state file naming a folder outside the stash never gets anything removed.
        let precious = game.join("saves");
        fs::create_dir_all(&precious).unwrap();
        fs::write(precious.join("world"), b"keep").unwrap();
        for version in ["../../saves", "..", "a/../../../saves", ".hidden"] {
            let odd = State {
                game_version: version.into(),
                none_for: "x".into(),
                ..State::default()
            };
            stash(game, &odd).unwrap();
        }
        assert!(precious.join("world").is_file());
        assert!(plain_name("1.8.9-shaders-kYuIpRLv") && !plain_name("a\\b") && !plain_name(""));

        // Another set of mods (Iris too) was never asked about.
        let asked = std::cell::Cell::new(false);
        sync_with_install(
            vanilla_target(game, "0.0-other"),
            true,
            true,
            &[],
            |_, ids| {
                asked.set(ids.contains(&crate::mods::packs::IRIS));
                no_build(&vanilla_target(game, "0.0-other"), ids)
            },
        )
        .unwrap();
        assert!(asked.get());

        // An answer older than the weekly look is not used.
        run("0.0-other", false, no_build).unwrap();
        let stashed = stash_dir(game, "1.8.9", false, "").join("arctic-performance-state.json");
        let mut old: State = load_json(&stashed).unwrap().unwrap();
        old.checked -= RECHECK_SECS;
        save_json(&stashed, &old).unwrap();
        let asked = std::cell::Cell::new(false);
        sync_with_install(
            vanilla_target(game, "1.8.9"),
            true,
            false,
            &[],
            |target, ids| {
                asked.set(true);
                no_build(target, ids)
            },
        )
        .unwrap();
        assert!(asked.get());
    }

    #[test]
    fn modrinth_out_of_reach_is_not_kept_as_no_build() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        sync_with_install(vanilla_target(game, "1.8.9"), true, false, &[], |_, ids| {
            let mut failed: Vec<super::super::Failed> = ids
                .iter()
                .skip(1)
                .map(|id| {
                    let none = crate::Error::Other(format!("{id} has no version for Fabric 0"));
                    ((*id).to_owned(), none)
                })
                .collect();
            failed.push((ids[0].to_owned(), crate::Error::Http("offline".into())));
            Ok((Vec::new(), failed))
        })
        .unwrap();
        let state: State = load_json(&state_path(game)).unwrap().unwrap();
        assert!(state.none_for.is_empty());

        // Nor is an answer that could not be read.
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        sync_with_install(vanilla_target(game, "1.8.9"), true, false, &[], |_, ids| {
            let failed = ids
                .iter()
                .map(|id| ((*id).to_owned(), crate::Error::Other("bad JSON".into())))
                .collect();
            Ok((Vec::new(), failed))
        })
        .unwrap();
        let state: State = load_json(&state_path(game)).unwrap().unwrap();
        assert!(state.none_for.is_empty());
    }

    #[test]
    fn a_start_that_changed_nothing_is_skipped_until_something_changes() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        installed_set(game, "0.0-test", "sodium.jar");
        let mut instance = crate::instances::Instance::vanilla_default();
        let holds = |instance: &crate::instances::Instance, version: &str| {
            let settled = settled::Settled::new(game, version, instance, true);
            settled.holds(settled.inputs().as_ref())
        };
        let start = |instance: &crate::instances::Instance| {
            prepare_for_launch(game, "0.0-test", instance, true, &|_| {}).unwrap();
        };
        assert!(!holds(&instance, "0.0-test"));
        start(&instance);
        assert!(holds(&instance, "0.0-test"));

        // Another game version, another switch: not the start that was remembered.
        assert!(!holds(&instance, "0.0-other"));
        instance.shaders = true;
        assert!(!holds(&instance, "0.0-test"));
        instance.shaders = false;
        assert!(holds(&instance, "0.0-test"));

        // A jar that is not the file it was (here: its size), or a new one.
        let mods = game.join("mods");
        fs::write(mods.join("sodium.jar"), b"another build").unwrap();
        assert!(!holds(&instance, "0.0-test"));
        start(&instance);
        assert!(holds(&instance, "0.0-test"));
        fs::write(mods.join("players-own.jar"), b"added").unwrap();
        assert!(!holds(&instance, "0.0-test"));
        start(&instance);
        assert!(holds(&instance, "0.0-test"));

        // Downloads an interrupted install left behind are cleared by the real work.
        save_json(&pending_path(game), &vec!["left-behind.jar"]).unwrap();
        assert!(!holds(&instance, "0.0-test"));
        start(&instance);
        assert!(!pending_path(game).exists());
        // That start changed the folder, so it is the next one that is remembered.
        assert!(!holds(&instance, "0.0-test"));
        start(&instance);
        assert!(holds(&instance, "0.0-test"));

        // An update found in the background is installed by the next start.
        let mut state: State = load_json(&state_path(game)).unwrap().unwrap();
        state.update_waiting = true;
        save_json(&state_path(game), &state).unwrap();
        assert!(!holds(&instance, "0.0-test"));

        // The weekly look at Modrinth falls due without anything in the folder changing.
        state.update_waiting = false;
        state.checked = crate::auth::now_secs() - RECHECK_SECS + 3600;
        save_json(&state_path(game), &state).unwrap();
        let settled = settled::Settled::new(game, "0.0-test", &instance, true);
        settled.remember(settled.inputs());
        assert!(settled.holds(settled.inputs().as_ref()));
        state.checked -= 7200;
        save_json(&state_path(game), &state).unwrap();
        settled.remember(settled.inputs());
        assert!(!settled.holds(settled.inputs().as_ref()));
    }
}
