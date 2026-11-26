//! Instances: isolated game directories.
//!
//! Every profile has the built-in "Arctic" instance (see [`builtin`]): the
//! Arctic Client and Polarium on Fabric, following the version picked on the
//! Play tab. User-created instances (Vanilla or a mod loader) live next to it
//! under `instances/<id>/` and reuse the shared libraries, assets and runtimes.

mod builtin;
mod duplicate;

pub use builtin::{
    ARCTIC_ID, BuiltinMigration, LEGACY_DEFAULT_ID, ensure_default, fallback_default, peek_default,
};

pub(crate) use duplicate::copy_entry;
pub use duplicate::duplicate;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::loaders::LoaderKind;
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

const INSTANCE_FILE: &str = "instance.json";
/// Game directory inside an instance (what vanilla calls `.minecraft`).
const GAME_DIR: &str = "minecraft";

/// Which mod loader an instance runs, with the loader's own version. Loader
/// profiles are layered on the instance's Minecraft version at launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Loader {
    Vanilla,
    Fabric { version: String },
    Quilt { version: String },
    NeoForge { version: String },
    Forge { version: String },
}

impl Loader {
    pub fn new(kind: Option<LoaderKind>, version: String) -> Self {
        match kind {
            None => Loader::Vanilla,
            Some(LoaderKind::Fabric) => Loader::Fabric { version },
            Some(LoaderKind::Quilt) => Loader::Quilt { version },
            Some(LoaderKind::NeoForge) => Loader::NeoForge { version },
            Some(LoaderKind::Forge) => Loader::Forge { version },
        }
    }

    pub fn kind(&self) -> Option<LoaderKind> {
        match self {
            Loader::Vanilla => None,
            Loader::Fabric { .. } => Some(LoaderKind::Fabric),
            Loader::Quilt { .. } => Some(LoaderKind::Quilt),
            Loader::NeoForge { .. } => Some(LoaderKind::NeoForge),
            Loader::Forge { .. } => Some(LoaderKind::Forge),
        }
    }

    /// The loader's own version (`None` for vanilla).
    pub fn version(&self) -> Option<&str> {
        match self {
            Loader::Vanilla => None,
            Loader::Fabric { version }
            | Loader::Quilt { version }
            | Loader::NeoForge { version }
            | Loader::Forge { version } => Some(version),
        }
    }

    pub fn label(&self) -> &'static str {
        self.kind().map_or("Vanilla", LoaderKind::label)
    }

    /// Default snowflake for instances of this loader, so different kinds
    /// are recognisable at a glance (users can override it).
    pub fn default_icon(&self) -> InstanceIcon {
        let (style, color) = match self.kind() {
            None => (FlakeStyle::Classic, None),
            Some(LoaderKind::Fabric) => (FlakeStyle::Stellar, Some([0xdb, 0xd0, 0xb4])),
            Some(LoaderKind::Quilt) => (FlakeStyle::Plate, Some([0xa7, 0x8b, 0xfa])),
            Some(LoaderKind::NeoForge) => (FlakeStyle::Crystal, Some([0xf4, 0x8c, 0x36])),
            Some(LoaderKind::Forge) => (FlakeStyle::Dendrite, Some([0x9c, 0xa3, 0xaf])),
        };
        InstanceIcon { style, color }
    }
}

/// Snowflake shapes used as instance icons.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlakeStyle {
    #[default]
    Classic,
    Stellar,
    Dendrite,
    Plate,
    Star,
    Crystal,
}

impl FlakeStyle {
    pub const ALL: [FlakeStyle; 6] = [
        FlakeStyle::Classic,
        FlakeStyle::Stellar,
        FlakeStyle::Dendrite,
        FlakeStyle::Plate,
        FlakeStyle::Star,
        FlakeStyle::Crystal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FlakeStyle::Classic => "Classic",
            FlakeStyle::Stellar => "Stellar",
            FlakeStyle::Dendrite => "Dendrite",
            FlakeStyle::Plate => "Plate",
            FlakeStyle::Star => "Star",
            FlakeStyle::Crystal => "Crystal",
        }
    }
}

/// How an instance is drawn: flake style plus an optional custom color
/// (`None` = the theme's accent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceIcon {
    pub style: FlakeStyle,
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub loader: Loader,
    /// Pinned Minecraft version. `None` for the default instance, which
    /// follows the Play tab selection.
    pub version: Option<String>,
    /// Per-instance memory override in MiB (falls back to global settings).
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    /// Icon shown for this instance; defaults to the loader's style.
    #[serde(default)]
    pub icon: InstanceIcon,
    /// Install Arctic's companion mod (Fabric/Quilt/Forge, supported versions). Always on for
    /// the built-in Arctic instance and always off for plain Vanilla ones (see [`Instance::normalize`]).
    #[serde(default = "enabled")]
    pub arctic_mod: bool,
    /// Java executable for this instance (falls back to the launcher setting).
    #[serde(default)]
    pub java_path: Option<std::path::PathBuf>,
    /// Extra JVM flags added after the launcher-wide ones.
    #[serde(default)]
    pub jvm_args: String,
    /// Vanilla instances: add performance mods (Sodium, Lithium, …).
    #[serde(default = "enabled")]
    pub performance: bool,
    /// Vanilla instances: add Iris so shader packs work.
    #[serde(default)]
    pub shaders: bool,
    /// Extra mods the player switched on (Modrinth project ids from `mods::performance::OPTIONAL`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<String>,
    /// Keeps its own settings (HUD, keys, FOV, server list) instead of sharing
    /// them with the profile's other instances (see `shared`).
    #[serde(default)]
    pub own_settings: bool,
    /// Game folder picked by the player (worlds, mods, packs live there); `None`: inside the
    /// instance's own folder. Never shared: a path on this computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_folder: Option<PathBuf>,
    /// The built-in Arctic instance (one per profile, can't be removed or copied). Its folder id
    /// is `arctic`, or `vanilla` for one converted from the old default (see [`builtin`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub builtin: bool,
}

fn enabled() -> bool {
    true
}

impl Instance {
    /// A fresh built-in Arctic instance (what new players start with).
    pub fn arctic_default() -> Self {
        Self {
            id: ARCTIC_ID.into(),
            name: ARCTIC_NAME.into(),
            loader: Loader::Vanilla,
            version: None,
            max_memory_mb: None,
            icon: Loader::Vanilla.default_icon(),
            arctic_mod: true,
            java_path: None,
            jvm_args: String::new(),
            performance: true,
            shaders: false,
            extras: Vec::new(),
            own_settings: false,
            game_folder: None,
            builtin: true,
        }
    }

    /// The built-in Arctic instance: follows the Play tab, can't be removed or copied.
    pub fn is_default(&self) -> bool {
        self.builtin
    }

    /// Whether games of this instance run with the Arctic Client (the global "Play with Arctic
    /// Client" setting aside): always for the built-in Arctic instance, never for plain Vanilla
    /// ones, and the instance's switch for loaders.
    pub fn arctic_client(&self) -> bool {
        if self.builtin {
            true
        } else if self.loader.kind().is_none() {
            false
        } else {
            self.arctic_mod
        }
    }

    /// Whether the player gets an Arctic Client switch for this instance: not on the Arctic
    /// instance (always on) nor on Vanilla ones (Vanilla stays vanilla); loaders keep theirs.
    pub fn has_arctic_switch(&self) -> bool {
        !self.builtin && self.loader.kind().is_some()
    }

    /// What kind of instance this is, for lists ("Arctic", "Vanilla", "Fabric", …).
    pub fn kind_label(&self) -> &'static str {
        if self.builtin {
            ARCTIC_NAME
        } else {
            self.loader.label()
        }
    }

    /// Makes the stored flags match what the instance is: the Arctic instance always has the
    /// client and follows the Play tab, plain Vanilla never has the client.
    pub fn normalize(&mut self) {
        self.arctic_mod = self.arctic_client();
        if self.builtin {
            self.version = None;
        }
    }

    pub fn normalized(mut self) -> Self {
        self.normalize();
        self
    }

    pub fn game_dir(&self, dirs: &DataDirs) -> PathBuf {
        match &self.game_folder {
            Some(folder) => folder.clone(),
            None => self.default_game_dir(dirs),
        }
    }

    /// The game folder inside the instance's own folder (used unless one was picked).
    pub fn default_game_dir(&self, dirs: &DataDirs) -> PathBuf {
        dirs.instance_dir(&self.id).join(GAME_DIR)
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(
            &dirs.instance_dir(&self.id).join(INSTANCE_FILE),
            &self.clone().normalized(),
        )
    }
}

/// The built-in Arctic instance's stock name.
pub const ARCTIC_NAME: &str = "Arctic";

/// An instance's `instance.json`, normalized (`None` if there is none).
fn read(dirs: &DataDirs, id: &str) -> Result<Option<Instance>> {
    Ok(
        load_json::<Instance>(&dirs.instance_dir(id).join(INSTANCE_FILE))?
            .map(Instance::normalized),
    )
}

/// Load the built-in Arctic instance, creating it (or moving an older default over to it)
/// when it isn't there yet. See [`ensure_default`].
pub fn load_default(dirs: &DataDirs) -> Result<Instance> {
    ensure_default(dirs).map(|(instance, _)| instance)
}

/// Removed instances are moved here instead of being deleted.
const TRASH_DIR: &str = ".trash";
const MAX_NAME_LEN: usize = 48;

/// Create a new instance (folders included) and return it.
pub fn create(dirs: &DataDirs, name: &str, game_version: &str, loader: Loader) -> Result<Instance> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(Error::Other(format!(
            "instance names must be 1-{MAX_NAME_LEN} characters"
        )));
    }
    let instance = Instance {
        id: unique_id(dirs, name),
        name: name.to_owned(),
        icon: loader.default_icon(),
        loader,
        version: Some(game_version.to_owned()),
        max_memory_mb: None,
        arctic_mod: true,
        java_path: None,
        jvm_args: String::new(),
        performance: true,
        shaders: false,
        extras: Vec::new(),
        own_settings: false,
        game_folder: None,
        builtin: false,
    }
    .normalized();
    let mods = instance.game_dir(dirs).join("mods");
    std::fs::create_dir_all(&mods).map_err(|e| Error::io(&mods, e))?;
    instance.save(dirs)?;
    Ok(instance)
}

/// `name` (trimmed to fit), or `name (2)`… if an instance already uses it.
pub fn free_name(dirs: &DataDirs, name: &str) -> Result<String> {
    let name: String = name.trim().chars().take(MAX_NAME_LEN).collect();
    let name = if name.is_empty() {
        "Imported".to_owned()
    } else {
        name
    };
    let mut taken: Vec<String> = list_custom(dirs)?
        .into_iter()
        .map(|i| i.name.to_lowercase())
        .collect();
    taken.push(ARCTIC_NAME.to_lowercase());
    if let Some(default) = peek_default(dirs) {
        taken.push(default.name.to_lowercase());
    }
    if !taken.contains(&name.to_lowercase()) {
        return Ok(name);
    }
    let stem: String = name.chars().take(MAX_NAME_LEN - 5).collect();
    Ok((2..)
        .map(|n| format!("{stem} ({n})"))
        .find(|n| !taken.contains(&n.to_lowercase()))
        .unwrap_or(stem))
}

/// How an instance switches to another game folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderChange {
    /// Its worlds, mods and settings go along (the new folder must be empty).
    MoveFiles,
    /// The new folder is used as it is (one that already holds a game, say); nothing is moved.
    UseAsIs,
}

/// Whether a folder holds nothing (so files can be moved into it).
pub fn folder_is_empty(folder: &Path) -> bool {
    std::fs::read_dir(folder).map_or(true, |mut entries| entries.next().is_none())
}

/// `instance` with its game in `folder` (`None`: back inside its own folder), saved. With
/// [`FolderChange::MoveFiles`] the current game folder's contents are copied there and then
/// removed from the old place (the instance must not be running). A folder the player picked is
/// emptied but kept; nothing outside the game folder is touched.
pub fn set_game_folder(
    dirs: &DataDirs,
    instance: &Instance,
    folder: Option<&Path>,
    change: FolderChange,
    progress: &dyn Fn(u64, u64),
) -> Result<Instance> {
    let updated = Instance {
        game_folder: folder
            .map(Path::to_path_buf)
            .filter(|f| *f != instance.default_game_dir(dirs)),
        ..instance.clone()
    };
    let (from, to) = (instance.game_dir(dirs), updated.game_dir(dirs));
    if let Some(folder) = folder
        && !folder.is_absolute()
    {
        return Err(Error::Other("Pick a full folder path.".into()));
    }
    if from == to {
        return Ok(instance.clone());
    }
    if to.starts_with(&from) || from.starts_with(&to) {
        return Err(Error::Other(
            "Pick a folder that isn't inside the game folder (or around it).".into(),
        ));
    }
    if change == FolderChange::MoveFiles {
        if !folder_is_empty(&to) {
            return Err(Error::Other(
                "Pick an empty folder to move the files into, or use the folder as it is.".into(),
            ));
        }
        if from.is_dir() {
            crate::storage::location::copy_dir(&from, &to, progress)?;
            empty_folder(&from);
        }
    }
    std::fs::create_dir_all(&to).map_err(|e| Error::io(&to, e))?;
    updated.save(dirs)?;
    Ok(updated)
}

/// Remove what's in `folder`, keeping the folder (it may be one the player made).
fn empty_folder(folder: &Path) {
    for entry in std::fs::read_dir(folder)
        .map(|e| e.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let path = entry.path();
        let gone = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(e) = gone {
            log::warn!("couldn't remove {} after moving it: {e}", path.display());
        }
    }
}

/// Move an instance folder (worlds and mods included) to `instances/.trash`.
pub fn remove(dirs: &DataDirs, id: &str) -> Result<()> {
    // Read only: a failing move to the Arctic instance mustn't block removing other instances.
    if peek_default(dirs).is_some_and(|d| d.id == id)
        || read(dirs, id).ok().flatten().is_some_and(|i| i.builtin)
    {
        return Err(Error::Other("the Arctic instance cannot be removed".into()));
    }
    let from = dirs.instance_dir(id);
    let trash = dirs.instances().join(TRASH_DIR);
    std::fs::create_dir_all(&trash).map_err(|e| Error::io(&trash, e))?;
    let to = trash.join(format!("{id}-{}", crate::auth::now_secs()));
    std::fs::rename(&from, &to).map_err(|e| Error::io(&from, e))
}

/// A removed instance, in `instances/.trash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trashed {
    pub dir: std::path::PathBuf,
    pub name: String,
    /// When it was removed (Unix seconds), from the folder's name.
    pub removed: u64,
}

/// How many removed instances there are (folders only; cheap enough for every frame).
pub fn trash_len(dirs: &DataDirs) -> usize {
    std::fs::read_dir(dirs.instances().join(TRASH_DIR))
        .map_or(0, |e| e.flatten().filter(|e| e.path().is_dir()).count())
}

/// Removed instances, last removed first.
pub fn trashed(dirs: &DataDirs) -> Vec<Trashed> {
    let Ok(entries) = std::fs::read_dir(dirs.instances().join(TRASH_DIR)) else {
        return Vec::new();
    };
    let mut out: Vec<Trashed> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let dir = e.path();
            let folder = e.file_name().to_string_lossy().into_owned();
            let removed = folder
                .rsplit_once('-')
                .and_then(|(_, at)| at.parse().ok())
                .unwrap_or(0);
            let name = load_json::<Instance>(&dir.join(INSTANCE_FILE))
                .ok()
                .flatten()
                .map_or_else(
                    || {
                        folder
                            .rsplit_once('-')
                            .map_or(folder.clone(), |(id, _)| id.to_owned())
                    },
                    |i| i.name,
                );
            Trashed { dir, name, removed }
        })
        .collect();
    out.sort_by_key(|t| std::cmp::Reverse(t.removed));
    out
}

/// The trash folder `dir` is in, if it really is one of its removed instances.
fn in_trash(dirs: &DataDirs, dir: &Path) -> Result<()> {
    let trash = dirs.instances().join(TRASH_DIR);
    if dir.parent() == Some(trash.as_path()) && dir.is_dir() {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "{} isn't in the trash",
            dir.display()
        )))
    }
}

/// Bring a removed instance back (under a new id if its old one is taken now).
pub fn restore(dirs: &DataDirs, dir: &Path) -> Result<Instance> {
    in_trash(dirs, dir)?;
    let file = dir.join(INSTANCE_FILE);
    let mut instance: Instance = load_json::<Instance>(&file)?
        .map(Instance::normalized)
        .ok_or_else(|| Error::Other("it has no instance.json".into()))?;
    // Only one built-in Arctic instance: one brought back from the trash is an ordinary one.
    let demoted = instance.builtin;
    if demoted {
        instance.builtin = false;
        instance.version = load_json::<crate::settings::Settings>(&dirs.settings_file())
            .ok()
            .flatten()
            .and_then(|s| s.last_version);
    }
    let id = if dirs.instance_dir(&instance.id).exists() || instance.id.is_empty() {
        unique_id(dirs, &instance.name)
    } else {
        instance.id.clone()
    };
    let to = dirs.instance_dir(&id);
    std::fs::rename(dir, &to).map_err(|e| Error::io(dir, e))?;
    if id != instance.id || demoted {
        instance.id = id;
        instance.save(dirs)?;
    }
    Ok(instance)
}

/// Delete a removed instance for good.
pub fn delete_trashed(dirs: &DataDirs, dir: &Path) -> Result<()> {
    in_trash(dirs, dir)?;
    std::fs::remove_dir_all(dir).map_err(|e| Error::io(dir, e))
}

fn unique_id(dirs: &DataDirs, name: &str) -> String {
    let slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base =
        if slug.is_empty() || slug == LEGACY_DEFAULT_ID || slug == ARCTIC_ID || slug == "trash" {
            "instance".to_owned()
        } else {
            slug
        };
    let taken = |id: &str| dirs.instance_dir(id).exists();
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or(base)
}

/// An instance by id or (case-insensitive) name, including the default.
pub fn find(dirs: &DataDirs, query: &str) -> Result<Instance> {
    let query = query.trim();
    let default = load_default(dirs)?;
    if query.eq_ignore_ascii_case(&default.id) || query.eq_ignore_ascii_case(&default.name) {
        return Ok(default);
    }
    if let Some(found) = list_custom(dirs)?
        .into_iter()
        .find(|i| i.id == query || i.name.eq_ignore_ascii_case(query))
    {
        return Ok(found);
    }
    // "arctic" is the built-in instance whatever the player renamed it to.
    if query.eq_ignore_ascii_case(ARCTIC_ID) {
        return Ok(default);
    }
    Err(Error::Other(format!(
        "no instance named '{query}' (see `arctic instances list`)"
    )))
}

/// User-created instances (everything except the default).
pub fn list_custom(dirs: &DataDirs) -> Result<Vec<Instance>> {
    let Ok(entries) = std::fs::read_dir(dirs.instances()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join(INSTANCE_FILE);
        match load_json::<Instance>(&path) {
            Ok(Some(i)) if !i.is_default() => out.push(i.normalized()),
            Ok(_) => {}
            Err(e) => log::warn!("skipping broken instance {}: {e}", path.display()),
        }
    }
    out.sort_by_key(|i| i.name.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_instances_are_listed_restored_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        let first = create(&dirs, "Mine", "1.21.11", Loader::Vanilla).unwrap();
        remove(&dirs, &first.id).unwrap();
        assert!(list_custom(&dirs).unwrap().is_empty());
        let trash = trashed(&dirs);
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].name, "Mine");
        // Another instance took its id meanwhile: it comes back under a new one.
        let other = create(&dirs, "Mine", "1.20.1", Loader::Vanilla).unwrap();
        assert_eq!(other.id, first.id);
        let back = restore(&dirs, &trash[0].dir).unwrap();
        assert_ne!(back.id, first.id);
        assert_eq!(list_custom(&dirs).unwrap().len(), 2);
        assert!(trashed(&dirs).is_empty());
        // Gone for good.
        remove(&dirs, &back.id).unwrap();
        let trash = trashed(&dirs);
        delete_trashed(&dirs, &trash[0].dir).unwrap();
        assert!(trashed(&dirs).is_empty());
        // Only folders in the trash.
        assert!(delete_trashed(&dirs, &dirs.instance_dir(&other.id)).is_err());
    }

    #[test]
    fn game_folder_moves_with_its_files_or_is_used_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        let inst = create(&dirs, "Mine", "1.21.11", Loader::Vanilla).unwrap();
        let world = inst.game_dir(&dirs).join("saves/W/level.dat");
        std::fs::create_dir_all(world.parent().unwrap()).unwrap();
        std::fs::write(&world, b"w").unwrap();

        let picked = dir.path().join("D/Games/Mine");
        let moved = set_game_folder(
            &dirs,
            &inst,
            Some(&picked),
            FolderChange::MoveFiles,
            &|_, _| {},
        )
        .unwrap();
        assert_eq!(moved.game_dir(&dirs), picked);
        assert!(picked.join("saves/W/level.dat").is_file());
        assert!(!world.exists());
        assert_eq!(
            find(&dirs, "Mine").unwrap().game_dir(&dirs),
            picked,
            "saved"
        );

        // Not into a folder with things in it, unless it's used as it is.
        let busy = dir.path().join("busy");
        std::fs::create_dir_all(busy.join("saves")).unwrap();
        assert!(
            set_game_folder(
                &dirs,
                &moved,
                Some(&busy),
                FolderChange::MoveFiles,
                &|_, _| {}
            )
            .is_err()
        );
        let used = set_game_folder(
            &dirs,
            &moved,
            Some(&busy),
            FolderChange::UseAsIs,
            &|_, _| {},
        )
        .unwrap();
        assert_eq!(used.game_dir(&dirs), busy);
        assert!(
            picked.join("saves/W/level.dat").is_file(),
            "left where it was"
        );

        // Back inside its own folder, files along; the picked folder stays (emptied).
        let back =
            set_game_folder(&dirs, &moved, None, FolderChange::MoveFiles, &|_, _| {}).unwrap();
        assert_eq!(back.game_folder, None);
        assert!(back.game_dir(&dirs).join("saves/W/level.dat").is_file());
        assert!(picked.is_dir() && folder_is_empty(&picked));
        assert!(
            set_game_folder(
                &dirs,
                &back,
                Some(&back.game_dir(&dirs).join("x")),
                FolderChange::UseAsIs,
                &|_, _| {}
            )
            .is_err()
        );
    }

    #[test]
    fn create_and_remove_custom_instances() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let loader = Loader::Fabric {
            version: "0.16.9".into(),
        };
        let a = create(&dirs, "My Fabric Pack", "1.21.4", loader.clone()).unwrap();
        assert_eq!(a.id, "my-fabric-pack");
        assert!(a.game_dir(&dirs).join("mods").is_dir());
        assert_eq!(a.icon, loader.default_icon());
        let b = create(&dirs, "My Fabric Pack", "1.21.4", loader).unwrap();
        assert_eq!(b.id, "my-fabric-pack-2");
        assert_eq!(list_custom(&dirs).unwrap().len(), 2);
        remove(&dirs, &a.id).unwrap();
        assert_eq!(list_custom(&dirs).unwrap().len(), 1);
        assert!(remove(&dirs, &load_default(&dirs).unwrap().id).is_err());
        assert!(create(&dirs, "  ", "1.21.4", Loader::Vanilla).is_err());
    }

    #[test]
    fn loader_serde_is_backwards_compatible() {
        let v: Loader = serde_json::from_str(r#"{"type":"vanilla"}"#).unwrap();
        assert_eq!(v, Loader::Vanilla);
        let f: Loader = serde_json::from_str(r#"{"type":"neo_forge","version":"21.4.1"}"#).unwrap();
        assert_eq!(f.kind(), Some(LoaderKind::NeoForge));
        assert_eq!(f.version(), Some("21.4.1"));
    }

    #[test]
    fn icon_defaults_for_old_instance_files() {
        let json = r#"{"id":"x","name":"X","loader":{"type":"vanilla"},"version":null}"#;
        let instance: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(instance.icon, InstanceIcon::default());
        let custom: InstanceIcon =
            serde_json::from_str(r#"{"style":"dendrite","color":[255,0,128]}"#).unwrap();
        assert_eq!(custom.style, FlakeStyle::Dendrite);
        assert_eq!(custom.color, Some([255, 0, 128]));
    }

    #[test]
    fn default_instance_is_created_once() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let first = load_default(&dirs).unwrap();
        assert!(first.is_default());
        assert_eq!(load_default(&dirs).unwrap(), first);
        assert!(
            first
                .game_dir(&dirs)
                .ends_with("instances/arctic/minecraft")
        );
    }

    #[test]
    fn custom_list_excludes_default() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        load_default(&dirs).unwrap();
        assert!(list_custom(&dirs).unwrap().is_empty());
        let custom = Instance {
            id: "abc".into(),
            name: "Test".into(),
            version: Some("1.21.4".into()),
            builtin: false,
            ..Instance::arctic_default()
        };
        custom.save(&dirs).unwrap();
        let custom = custom.normalized();
        assert_eq!(list_custom(&dirs).unwrap(), vec![custom]);
    }
}
