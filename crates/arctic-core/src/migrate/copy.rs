//! Copy an instance from another launcher into Arctic: a new instance (or
//! Arctic's Vanilla one), then its files by category. Other launchers'
//! own files, caches and anything account-related are never copied, and
//! nothing already in Arctic is overwritten.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Found;
use super::detect::jar_fits;
use crate::instances::{self, Instance, Loader};
use crate::loaders::{self, LoaderKind};
use crate::mods::{self, Recognized};
use crate::storage::DataDirs;
use crate::versions::VersionManifest;
use crate::{Error, Progress, ProgressInfo, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Mods,
    Worlds,
    ResourcePacks,
    ShaderPacks,
    /// Game options, servers, key binds, mod configs and mod data.
    Settings,
    Screenshots,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Mods,
        Category::Worlds,
        Category::ResourcePacks,
        Category::ShaderPacks,
        Category::Settings,
        Category::Screenshots,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Mods => "Mods",
            Category::Worlds => "Worlds",
            Category::ResourcePacks => "Resource packs",
            Category::ShaderPacks => "Shader packs",
            Category::Settings => "Options, servers & mod configs",
            Category::Screenshots => "Screenshots",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Category::Mods => "mods",
            Category::Worlds => "worlds",
            Category::ResourcePacks => "resourcepacks",
            Category::ShaderPacks => "shaderpacks",
            Category::Settings => "settings",
            Category::Screenshots => "screenshots",
        }
    }
}

/// Size and count of each category.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sizes {
    pub bytes: HashMap<Category, u64>,
    /// Mods, worlds, packs or screenshots (files and folders for settings).
    pub counts: HashMap<Category, usize>,
    /// Mods left out because they're for another loader (shared folders).
    pub other_loader: Vec<String>,
    /// Mods whose own files say they don't run on this Minecraft version:
    /// they come switched off.
    pub turned_off: Vec<String>,
}

/// What happens to some mods, for the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ModNotes {
    other_loader: Vec<String>,
    turned_off: Vec<String>,
}

/// Never copied, at the top of a game folder (lowercase).
const SKIP_DIRS: &[&str] = &[
    "logs",
    "crash-reports",
    "debug",
    ".fabric",
    ".cache",
    "cache",
    "natives",
    "versions",
    "libraries",
    "assets",
    "runtime",
    "bin",
    "webcache",
    "webcache2",
    "downloads",
    "server-resource-packs",
    ".mixin.out",
    "labymod-neo",
    "tlloader",
    "_norton_",
    "quickplay",
    "staging",
    "backups",
    "disabledmods",
    "jarmods",
    "blclient-mod-profiles",
    "badlion client",
    ".lunarclient",
    "mods",
    "saves",
    "resourcepacks",
    "texturepacks",
    "shaderpacks",
    "screenshots",
];
const SKIP_FILES: &[&str] = &[
    "usercache.json",
    "usernamecache.json",
    "realms_persistence.json",
    "instance.json",
    "instance.cfg",
    "instance.png",
    "mmc-pack.json",
    "minecraftinstance.json",
    "manifest.json",
    "modlist.html",
    "profile.json",
    "config.json",
    "treatment_tags.json",
    "servers.dat_old",
    "tlauncherprofiles.json",
    "tempoptifinestore-1.0.json",
    "options.txt.bak",
];
/// Kept from a folder several launchers share (`.minecraft`).
const SHARED_KEEP: &[&str] = &[
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "hotbar.nbt",
    "config",
];

fn is_secret(name: &str) -> bool {
    [
        "account",
        "token",
        "credential",
        "msa",
        "session",
        "password",
        "auth",
    ]
    .iter()
    .any(|w| name.contains(w))
}

/// Settings entries (top-level files and folders) that come along.
fn settings_entries(found: &Found) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(&found.game_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            let is_dir = e.path().is_dir();
            if found.shared {
                return SHARED_KEEP.contains(&name.as_str());
            }
            let skipped = if is_dir {
                SKIP_DIRS.contains(&name.as_str())
            } else {
                SKIP_FILES.contains(&name.as_str())
                    || name.starts_with("launcher_")
                    || name.starts_with("tlauncher")
                    || [".log", ".lck", ".exe", ".dll", ".tmp"]
                        .iter()
                        .any(|x| name.ends_with(x))
                    || name.starts_with("clientid")
            };
            !skipped && !is_secret(&name)
        })
        .map(|e| e.path())
        .collect()
}

/// Mod jars to copy: (source, file name to write). In a shared folder,
/// jars clearly made for another loader are left out.
/// `thorough`: read each jar (version ranges, duplicates); previews skip
/// it, since opening thousands of jars takes a while.
fn mod_files(found: &Found, thorough: bool) -> (Vec<(PathBuf, String)>, ModNotes) {
    let kind = found.loader.as_ref().map(|l| l.kind);
    let mut out = Vec::new();
    let mut notes = ModNotes::default();
    let mut add_dir = |dir: &Path, disable: bool| {
        // Only the folder itself: Lunar's `dependencies` holds pieces
        // unpacked from mods that already carry them.
        let files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|e| e.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        let filter = found.shared && found.mods_dir.is_none();
        for path in files {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let jar = name.ends_with(".jar") || name.ends_with(".jar.disabled");
            if !jar || !path.is_file() {
                continue;
            }
            if filter && !jar_fits(&path, kind) {
                notes.other_loader.push(name);
                continue;
            }
            // The loader would refuse it and stop the game: bring it switched off.
            let wrong_game = thorough
                && kind.and_then(|k| super::ranges::game_fits(&path, &found.game_version, k))
                    == Some(false);
            if wrong_game && name.ends_with(".jar") {
                notes.turned_off.push(name.clone());
            }
            let dest = if (disable || wrong_game) && name.ends_with(".jar") {
                format!("{name}.disabled")
            } else {
                name
            };
            out.push((path, dest));
        }
    };
    if kind.is_some() {
        add_dir(&found.mods_dir(), false);
        if let Some(dir) = &found.disabled_mods_dir {
            add_dir(dir, true);
        }
        for jar in &found.extra_jars {
            if let Some(name) = jar.file_name() {
                out.push((jar.clone(), name.to_string_lossy().into_owned()));
            }
        }
    }
    let out = if thorough {
        super::detect::newest_only(out)
    } else {
        out
    };
    (out, notes)
}

/// Does `found` bring any mods?
pub(super) fn has_mods(found: &Found) -> bool {
    !mod_files(found, false).0.is_empty()
}

/// A file to copy.
struct Item {
    src: PathBuf,
    /// Path under the target game folder.
    dest: PathBuf,
    cat: Category,
    size: u64,
}

/// Every file to copy.
fn plan(found: &Found, thorough: bool) -> (Vec<Item>, ModNotes) {
    let mut items = Vec::new();
    let (mods, notes) = mod_files(found, thorough);
    for (src, name) in mods {
        let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        items.push(Item {
            src,
            dest: PathBuf::from("mods").join(name),
            cat: Category::Mods,
            size,
        });
    }
    // A folder shared by several launchers' profiles: its worlds, packs and
    // screenshots come once, with the entry that goes into Vanilla.
    if found.shared && !found.into_vanilla {
        for entry in settings_entries(found) {
            let name = entry.file_name().unwrap_or_default().to_owned();
            walk(&entry, Path::new(&name), Category::Settings, &mut items);
        }
        return (items, notes);
    }
    let g = &found.game_dir;
    for world in crate::worlds::list(&g.join("saves")) {
        let name = world.path.file_name().unwrap_or_default().to_owned();
        walk(
            &world.path,
            &PathBuf::from("saves").join(name),
            Category::Worlds,
            &mut items,
        );
    }
    for (dir, cat) in [
        ("resourcepacks", Category::ResourcePacks),
        ("texturepacks", Category::ResourcePacks),
        ("shaderpacks", Category::ShaderPacks),
        ("screenshots", Category::Screenshots),
    ] {
        let target = if dir == "texturepacks" {
            "resourcepacks"
        } else {
            dir
        };
        walk(&g.join(dir), Path::new(target), cat, &mut items);
    }
    for (dir, target) in &found.extra_dirs {
        let cat = if *target == "shaderpacks" {
            Category::ShaderPacks
        } else {
            Category::ResourcePacks
        };
        walk(dir, Path::new(target), cat, &mut items);
    }
    for entry in settings_entries(found) {
        let name = entry.file_name().unwrap_or_default().to_owned();
        walk(&entry, Path::new(&name), Category::Settings, &mut items);
    }
    (items, notes)
}

/// A top-level file or folder; a link there is followed (people point
/// `resourcepacks` at another folder), links further in are not.
fn walk(src: &Path, dest: &Path, cat: Category, out: &mut Vec<Item>) {
    let Ok(meta) = std::fs::metadata(src) else {
        return;
    };
    if meta.is_file() {
        out.push(Item {
            src: src.to_path_buf(),
            dest: dest.to_path_buf(),
            cat,
            size: meta.len(),
        });
    } else if meta.is_dir() {
        walk_dir(src, dest, cat, out);
    }
}

/// Listing a folder already gives each entry's type and size (on Windows
/// without asking again per file), which keeps big worlds quick.
fn walk_dir(src: &Path, dest: &Path, cat: Category, out: &mut Vec<Item>) {
    let Ok(entries) = std::fs::read_dir(src) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let Ok(kind) = e.file_type() else {
            continue;
        };
        // A running game's lock and symlinks out of the folder stay behind.
        if name == "session.lock" || kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk_dir(&e.path(), &dest.join(&name), cat, out);
        } else if kind.is_file() {
            out.push(Item {
                src: e.path(),
                dest: dest.join(&name),
                cat,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }
}

/// Sizes of several instances at once (one thread each, a few at a time).
pub fn measure_all(found: &[Found]) -> Vec<Sizes> {
    const THREADS: usize = 6;
    let mut out = vec![Sizes::default(); found.len()];
    for (chunk_in, chunk_out) in found.chunks(THREADS).zip(out.chunks_mut(THREADS)) {
        std::thread::scope(|s| {
            for (f, slot) in chunk_in.iter().zip(chunk_out.iter_mut()) {
                s.spawn(move || *slot = measure(f));
            }
        });
    }
    out
}

/// What importing `found` would copy, per category.
pub fn measure(found: &Found) -> Sizes {
    let (items, notes) = plan(found, false);
    let mut sizes = Sizes {
        other_loader: notes.other_loader,
        turned_off: notes.turned_off,
        ..Sizes::default()
    };
    let mut units: HashMap<Category, std::collections::HashSet<PathBuf>> = HashMap::new();
    for item in &items {
        *sizes.bytes.entry(item.cat).or_default() += item.size;
        let unit: PathBuf = item
            .dest
            .components()
            .take(if item.cat == Category::Settings { 1 } else { 2 })
            .collect();
        units.entry(item.cat).or_default().insert(unit);
    }
    for (cat, set) in units {
        sizes.counts.insert(cat, set.len());
    }
    sizes
}

/// What an import did.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub instance: Instance,
    /// A new instance (not Arctic's Vanilla one).
    pub created: bool,
    pub files: usize,
    /// Files left alone because Arctic already had them.
    pub kept: usize,
    pub mods: usize,
    pub recognized: Option<Recognized>,
    pub other_loader: Vec<String>,
    /// Mods that came switched off (made for another Minecraft version).
    pub turned_off: Vec<String>,
    /// Required mods the pack was missing, downloaded from Modrinth.
    pub added: Vec<String>,
    pub notes: Vec<String>,
}

/// Copy `found` into Arctic, taking only `categories`.
pub fn import(
    dirs: &DataDirs,
    found: &Found,
    categories: &[Category],
    name: Option<&str>,
    progress: Progress,
) -> Result<Imported> {
    progress(ProgressInfo::stage("Checking the version"));
    let (instance, created, notes) = target(dirs, found, name)?;
    let result = fill(dirs, found, categories, &instance, progress);
    match result {
        Ok(mut done) => {
            done.created = created;
            done.notes.splice(0..0, notes);
            Ok(done)
        }
        Err(e) => {
            if created {
                let _ = instances::remove(dirs, &instance.id);
            }
            Err(e)
        }
    }
}

fn target(
    dirs: &DataDirs,
    found: &Found,
    name: Option<&str>,
) -> Result<(Instance, bool, Vec<String>)> {
    if found.into_vanilla {
        return Ok((instances::load_default(dirs)?, false, Vec::new()));
    }
    let manifest = VersionManifest::fetch(dirs)?;
    if manifest.find(&found.game_version).is_none() {
        return Err(Error::Other(format!(
            "Minecraft {} isn't a version Mojang offers, so it can't be set up",
            found.game_version
        )));
    }
    let mut notes = Vec::new();
    let loader = match &found.loader {
        None => Loader::Vanilla,
        Some(l) => {
            let version = loader_version(l.kind, l.version.as_deref(), &found.game_version)?;
            let version = arctic_ready(dirs, l.kind, version, &found.game_version, &mut notes);
            Loader::new(Some(l.kind), version)
        }
    };
    let name = instances::free_name(dirs, name.unwrap_or(&found.name))?;
    let created = instances::create(dirs, &name, &found.game_version, loader)?;
    let instance = Instance {
        max_memory_mb: found.memory_mb,
        // The other launcher had its own mods; don't add ours on top.
        performance: found.loader.is_none() && created.performance,
        ..created
    };
    instance.save(dirs)?;
    Ok((instance, true, notes))
}

/// A Fabric Loader too old for the Arctic Client is moved up to the one it
/// needs (newer Fabric Loaders keep running older mods).
fn arctic_ready(
    dirs: &DataDirs,
    kind: LoaderKind,
    version: String,
    game: &str,
    notes: &mut Vec<String>,
) -> String {
    if kind != LoaderKind::Fabric || crate::arctic_mod::loader_fits(game, &version) {
        return version;
    }
    match crate::arctic_mod::client_loader(dirs, game) {
        Some(newer) if crate::arctic_mod::loader_fits(game, &newer) => {
            notes.push(format!(
                "Fabric Loader updated from {version} to {newer} for the Arctic Client"
            ));
            newer
        }
        _ => version,
    }
}

/// The exact loader version, or the newest stable one for "latest".
fn loader_version(kind: LoaderKind, wanted: Option<&str>, game: &str) -> Result<String> {
    let available = loaders::loader_versions(kind, game)?;
    match wanted {
        Some(v) if available.iter().any(|a| a.id == v) => Ok(v.to_owned()),
        Some(v) => Err(Error::Other(format!(
            "{} {v} for Minecraft {game} isn't available to download",
            kind.label()
        ))),
        None => available
            .iter()
            .find(|a| a.stable)
            .or_else(|| available.first())
            .map(|a| a.id.clone())
            .ok_or_else(|| {
                Error::Other(format!("{} doesn't support Minecraft {game}", kind.label()))
            }),
    }
}

fn fill(
    dirs: &DataDirs,
    found: &Found,
    categories: &[Category],
    instance: &Instance,
    progress: Progress,
) -> Result<Imported> {
    let game_dir = instance.game_dir(dirs);
    let (items, mod_notes) = plan(found, true);
    let items: Vec<_> = items
        .into_iter()
        .filter(|i| categories.contains(&i.cat))
        .collect();
    let renames = world_renames(&items, &game_dir);
    let total: u64 = items.iter().map(|i| i.size).sum();
    let (mut done_bytes, mut files, mut kept, mut mods) = (0u64, 0usize, 0usize, 0usize);
    for (
        i,
        Item {
            src,
            dest,
            cat,
            size,
        },
    ) in items.iter().enumerate()
    {
        let dest = rename_world(dest, &renames);
        let to = game_dir.join(&dest);
        if to.exists() {
            kept += 1;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            std::fs::copy(src, &to).map_err(|e| Error::io(src, e))?;
            files += 1;
            if *cat == Category::Mods {
                mods += 1;
            }
        }
        done_bytes += *size;
        progress(ProgressInfo {
            stage: "Copying files",
            done: i as u64 + 1,
            total: items.len() as u64,
            bytes_done: done_bytes,
            bytes_total: total,
        });
    }
    let mut notes = found.notes.clone();
    let mut added = Vec::new();
    if let (Some(l), true) = (&found.loader, mods > 0) {
        progress(ProgressInfo::stage("Checking required mods"));
        let mods_dir = game_dir.join("mods");
        let index = mods::index_path(&dirs.instance_dir(&instance.id));
        for m in super::deps::missing(&mods_dir, l.kind) {
            // Old mods name Fabric API by its former id.
            let project = if m.id == "fabric" {
                "fabric-api"
            } else {
                m.id.as_str()
            };
            match mods::install(project, &found.game_version, l.kind, &mods_dir, &index, progress) {
                Ok(installed) => added.extend(installed.into_iter().map(|i| format!("{} (needed by {})", i.title, m.by))),
                Err(_) => notes.push(format!(
                    "{} needs \"{}\", which isn't in the pack or on Modrinth; the game may not start without it",
                    m.by, m.id
                )),
            }
        }
    }
    let recognized = match (&found.loader, mods > 0) {
        (Some(l), true) => {
            progress(ProgressInfo::stage("Recognizing mods on Modrinth"));
            let index = mods::index_path(&dirs.instance_dir(&instance.id));
            match mods::recognize(&game_dir.join("mods"), &index, &found.game_version, l.kind) {
                Ok(r) => Some(r),
                Err(e) => {
                    notes.push(format!(
                        "Mods were copied but not looked up on Modrinth ({e})"
                    ));
                    None
                }
            }
        }
        _ => None,
    };
    Ok(Imported {
        instance: instance.clone(),
        created: false,
        files,
        kept,
        mods,
        recognized,
        other_loader: mod_notes.other_loader,
        turned_off: mod_notes.turned_off,
        added,
        notes,
    })
}

/// World folders whose name Arctic already uses get ` (2)`… instead.
fn world_renames(items: &[Item], game_dir: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for item in items {
        if item.cat != Category::Worlds {
            continue;
        }
        let Some(name) = item
            .dest
            .components()
            .nth(1)
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
        else {
            continue;
        };
        if out.contains_key(&name) || !game_dir.join("saves").join(&name).exists() {
            continue;
        }
        let free = (2..)
            .map(|n| format!("{name} ({n})"))
            .find(|n| !game_dir.join("saves").join(n).exists())
            .unwrap_or_else(|| name.clone());
        out.insert(name, free);
    }
    out
}

fn rename_world(dest: &Path, renames: &HashMap<String, String>) -> PathBuf {
    let mut parts: Vec<String> = dest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.first().map(String::as_str) == Some("saves")
        && let Some(new) = parts.get(1).and_then(|n| renames.get(n))
    {
        parts[1] = new.clone();
    }
    parts.iter().collect()
}
