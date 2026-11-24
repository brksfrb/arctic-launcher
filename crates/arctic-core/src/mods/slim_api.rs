//! Fabric API cut down to the parts the mods in the folder use.
//!
//! The Fabric API download is one jar holding about forty modules, and
//! Fabric loads every one of them on every start (about a second and a half
//! of the start). The performance mods use a handful. For an instance whose
//! mods are all the launcher's own, the big jar is put aside and only the
//! modules the other mods refer to (and the ones those need) go in its place,
//! together with a tiny stand-in that keeps the name `fabric-api` for mods
//! that ask for it. Everything is made on this computer from the file that was
//! downloaded, and undone before the launcher touches the mods again.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::IoContext;
use crate::loaders::LoaderKind;
use crate::migrate::deps;
use crate::storage::{load_json, save_json};

/// Where the big jar waits (next to the mods folder).
const STASH: &str = "mods-full-api";
/// What was done, so it can be undone (inside the mods folder; loaders ignore it).
const STATE: &str = ".arctic-slim-api.json";
/// Before 0.6.7: the kept modules for one exact set of jars. No longer read.
const OLD_CACHE: &str = ".arctic-slim-api-cache.json";
/// What each jar was found to use ([`Uses`]).
const USES: &str = ".arctic-slim-api-uses.json";
/// Jars remembered in [`USES`]: a few game versions' worth of performance mods.
const MAX_USES: usize = 64;
const STUB: &str = "fabric-api-stub.jar";
const CLASS_PREFIX: &[u8] = b"net/fabricmc/fabric/";

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct State {
    /// The big jar's file name.
    original: String,
    /// Files put into the mods folder.
    added: Vec<String>,
    /// What the other jars looked like when the choice was made.
    signature: String,
    /// How the stand-in was made ([`STUB_VERSION`]); older ones are made again.
    #[serde(default)]
    stub_version: u32,
}

/// 3: also keep modules required by bundled jars. Since 2, the stand-in
/// answers to both `fabric` and `fabric-api`.
const STUB_VERSION: u32 = 3;

fn state_path(mods_dir: &Path) -> PathBuf {
    mods_dir.join(STATE)
}

pub(super) fn stash_dir(mods_dir: &Path) -> PathBuf {
    mods_dir.parent().unwrap_or(mods_dir).join(STASH)
}

impl State {
    fn complete(&self, mods_dir: &Path) -> bool {
        !mods_dir.join(&self.original).exists()
            && stash_dir(mods_dir).join(&self.original).is_file()
            && self.added.iter().all(|name| mods_dir.join(name).is_file())
    }
}

/// Put the folder back as it was before [`apply`].
pub fn undo(mods_dir: &Path) -> Result<()> {
    let path = state_path(mods_dir);
    let Some(state) = load_json::<State>(&path)? else {
        return Ok(());
    };
    for name in &state.added {
        super::files::check_file_name(name)?;
        let file = mods_dir.join(name);
        if file.exists() {
            fs::remove_file(&file).at(&file)?;
        }
    }
    if !state.original.is_empty() {
        super::files::check_file_name(&state.original)?;
        let from = stash_dir(mods_dir).join(&state.original);
        let to = mods_dir.join(&state.original);
        if from.exists() && !to.exists() {
            fs::rename(&from, &to).at(&from)?;
        }
    }
    fs::remove_file(&path).at(&path)
}

/// Older builds recorded their changes last. In a launcher-owned mods folder,
/// recover a missing or truncated record from the original API jar's entries.
pub(super) fn recover_orphans(mods_dir: &Path) -> Result<()> {
    let state_file = state_path(mods_dir);
    let invalid = match load_json::<State>(&state_file) {
        Ok(Some(state)) if state.complete(mods_dir) => return Ok(()),
        Ok(Some(_)) => return undo(mods_dir),
        Ok(None) => false,
        Err(crate::Error::Json(_)) => true,
        Err(e) => return Err(e),
    };
    let stash = stash_dir(mods_dir);
    let parked = find_umbrella(&stash);
    let active = find_umbrella(mods_dir);
    for full in parked
        .iter()
        .map(|name| stash.join(name))
        .chain(active.iter().map(|name| mods_dir.join(name)))
    {
        let Ok(mut zip) = zip::ZipArchive::new(fs::File::open(&full).at(&full)?) else {
            continue;
        };
        let entries: Vec<_> = zip
            .file_names()
            .filter(|entry| {
                module_file_name(entry).is_some_and(|name| mods_dir.join(name).is_file())
            })
            .map(str::to_owned)
            .collect();
        for entry in entries {
            let name = module_file_name(&entry).expect("selected module entry");
            super::files::check_file_name(name)?;
            let mut bytes = Vec::new();
            zip.by_name(&entry)
                .map_err(|e| crate::Error::Other(format!("Fabric API: {e}")))?
                .read_to_end(&mut bytes)
                .at(&full)?;
            let loose = mods_dir.join(name);
            if fs::read(&loose).at(&loose)? == bytes {
                remove_generated(mods_dir, name)?;
            }
        }
    }
    remove_generated(mods_dir, STUB)?;
    if active.is_none()
        && let Some(name) = parked
    {
        let from = stash.join(&name);
        fs::rename(&from, mods_dir.join(name)).at(&from)?;
    }
    if invalid {
        fs::remove_file(&state_file).at(&state_file)?;
    }
    Ok(())
}

fn remove_generated(mods_dir: &Path, name: &str) -> Result<()> {
    super::files::check_file_name(name)?;
    let path = mods_dir.join(name);
    match fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(crate::Error::io(path, e)),
        Ok(()) => {
            log::info!("removed leftover slim Fabric API jar {name}");
            Ok(())
        }
        _ => Ok(()),
    }
}

fn module_file_name(entry: &str) -> Option<&str> {
    let name = entry.strip_prefix("META-INF/jars/")?.rsplit('/').next()?;
    name.ends_with(".jar").then_some(name)
}

/// One module inside the big jar.
struct Module {
    id: String,
    /// Path inside the big jar.
    entry: String,
    depends: Vec<String>,
    classes: Vec<String>,
}

/// The big jar's file name in `mods_dir`, if there is one.
fn find_umbrella(mods_dir: &Path) -> Option<String> {
    fs::read_dir(mods_dir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| {
            n.starts_with("fabric-api-")
                && n.ends_with(".jar")
                && n != STUB
                && crate::migrate::detect::mod_identity(&mods_dir.join(n))
                    .is_some_and(|(id, _)| matches!(id.as_str(), "fabric-api" | "fabric"))
        })
}

/// The big jar's modules, its version and its mod id (`fabric-api`; `fabric` in older releases,
/// such as the ones for 1.16).
fn read_modules(umbrella: &Path) -> Option<(Vec<Module>, String, String)> {
    let mut zip = zip::ZipArchive::new(fs::File::open(umbrella).ok()?).ok()?;
    let (version, id) = {
        let mut text = String::new();
        zip.by_name("fabric.mod.json")
            .ok()?
            .read_to_string(&mut text)
            .ok()?;
        let meta =
            serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()?;
        (
            meta.get("version")?.as_str()?.to_owned(),
            meta.get("id")?.as_str()?.to_owned(),
        )
    };
    let names: Vec<String> = zip
        .file_names()
        .filter(|n| n.starts_with("META-INF/jars/") && n.ends_with(".jar"))
        .map(str::to_owned)
        .collect();
    let mut modules = Vec::new();
    for entry in names {
        let mut bytes = Vec::new();
        zip.by_name(&entry).ok()?.read_to_end(&mut bytes).ok()?;
        let mut inner = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        let mut text = String::new();
        inner
            .by_name("fabric.mod.json")
            .ok()?
            .read_to_string(&mut text)
            .ok()?;
        let meta: serde_json::Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        let depends = meta
            .get("depends")
            .and_then(|d| d.as_object())
            .map(|d| d.keys().cloned().collect())
            .unwrap_or_default();
        let classes = inner
            .file_names()
            .filter_map(|n| {
                n.strip_suffix(".class")
                    .filter(|n| n.starts_with("net/fabricmc/fabric/"))
                    .map(str::to_owned)
            })
            .collect();
        modules.push(Module {
            id: meta.get("id")?.as_str()?.to_owned(),
            entry,
            depends,
            classes,
        });
    }
    Some((modules, version, id))
}

/// Every `net/fabricmc/fabric/...` class name mentioned in the class files of a jar
/// (and of the jars inside it).
fn referenced_classes(jar: impl Read + std::io::Seek, found: &mut BTreeSet<String>) {
    let Ok(mut zip) = zip::ZipArchive::new(jar) else {
        return;
    };
    for i in 0..zip.len() {
        let Ok(mut file) = zip.by_index(i) else {
            continue;
        };
        let name = file.name().to_owned();
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            continue;
        }
        if name.ends_with(".class") {
            scan_class(&bytes, found);
        } else if name.starts_with("META-INF/jars/") && name.ends_with(".jar") {
            referenced_classes(Cursor::new(bytes), found);
        }
    }
}

fn scan_class(bytes: &[u8], found: &mut BTreeSet<String>) {
    let mut from = 0;
    while let Some(at) = find(&bytes[from..], CLASS_PREFIX) {
        let start = from + at;
        let end = bytes[start..]
            .iter()
            .position(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'/' | b'$')))
            .map_or(bytes.len(), |p| start + p);
        if let Ok(name) = std::str::from_utf8(&bytes[start..end]) {
            found.insert(name.to_owned());
        }
        from = end.max(start + 1);
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// What the other jars in the folder look like (names, sizes, times).
fn signature(mods_dir: &Path, skip: &[&str]) -> String {
    let mut parts: Vec<String> = fs::read_dir(mods_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".jar") || skip.contains(&name.as_str()) || name == STUB {
                return None;
            }
            let meta = e.metadata().ok()?;
            let time = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Some(format!("{name}:{}:{time}", meta.len()))
        })
        .collect();
    parts.sort();
    parts.join("|")
}

/// What one jar asks of Fabric API: found by reading every class in it (and in the jars inside
/// it), which for a set of performance mods is a quarter of a second, so it is kept per jar.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JarUse {
    /// Name, size and modification time: another file is read again.
    key: String,
    /// Every `net/fabricmc/fabric/...` class name its classes mention.
    classes: BTreeSet<String>,
    /// The mod ids it requires.
    requires: BTreeSet<String>,
}

#[derive(Default, PartialEq, Eq, Serialize, Deserialize)]
struct Uses {
    jars: Vec<JarUse>,
}

fn jar_key(name: &str, meta: &fs::Metadata) -> Option<String> {
    let time = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!("{name}:{}:{time}", meta.len()))
}

fn read_use(path: &Path, key: String) -> JarUse {
    let mut classes = BTreeSet::new();
    if let Ok(file) = fs::File::open(path) {
        referenced_classes(file, &mut classes);
    }
    let (_, requires) = deps::fabric_ids(path, LoaderKind::Fabric);
    JarUse {
        key,
        classes,
        requires: requires.into_iter().collect(),
    }
}

/// The modules (ids) to keep: those the other jars refer to or declare, and what those need.
fn keep(mods_dir: &Path, umbrella: &str, modules: &[Module]) -> BTreeSet<String> {
    let owner: HashMap<&str, &str> = modules
        .iter()
        .flat_map(|m| m.classes.iter().map(|c| (c.as_str(), m.id.as_str())))
        .collect();
    let uses_file = mods_dir.join(USES);
    let mut known: Uses = fs::read_to_string(&uses_file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut used: Vec<JarUse> = Vec::new();
    let mut learned = false;
    for entry in fs::read_dir(mods_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".jar") || name == umbrella || name == STUB {
            continue;
        }
        let key = entry.metadata().ok().and_then(|meta| jar_key(&name, &meta));
        let found = key
            .as_ref()
            .and_then(|key| known.jars.iter().find(|jar| &jar.key == key));
        match (found, key) {
            (Some(jar), _) => used.push(jar.clone()),
            (None, key) => {
                // A file whose size or time can't be told is read every time.
                let jar = read_use(&entry.path(), key.clone().unwrap_or_default());
                if key.is_some() {
                    known.jars.push(jar.clone());
                    learned = true;
                }
                used.push(jar);
            }
        }
    }
    if learned {
        // Over the limit, the oldest entries that aren't this folder's jars go first.
        let mut spare = known.jars.len().saturating_sub(MAX_USES);
        known.jars.retain(|jar| {
            let drop = spare > 0 && !used.iter().any(|u| u.key == jar.key);
            spare -= usize::from(drop);
            !drop
        });
        if let Ok(text) = serde_json::to_string(&known) {
            let _ = fs::write(&uses_file, text);
        }
        let _ = fs::remove_file(mods_dir.join(OLD_CACHE));
    }
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    for jar in &used {
        wanted.extend(
            jar.requires
                .iter()
                .filter(|id| modules.iter().any(|m| &m.id == *id))
                .cloned(),
        );
        for class in &jar.classes {
            let id = owner
                .get(class.as_str())
                .or_else(|| owner.get(class.split('$').next().unwrap_or(class)));
            if let Some(id) = id {
                wanted.insert((*id).to_owned());
            }
        }
    }
    // What the kept modules need themselves.
    let mut todo: Vec<String> = wanted.iter().cloned().collect();
    while let Some(id) = todo.pop() {
        if let Some(module) = modules.iter().find(|m| m.id == id) {
            for dep in &module.depends {
                if modules.iter().any(|m| &m.id == dep) && wanted.insert(dep.clone()) {
                    todo.push(dep.clone());
                }
            }
        }
    }
    wanted
}

/// The big jar's file name while it is put aside (so that "is everything there" checks can count it).
pub fn stashed(mods_dir: &Path) -> Option<String> {
    stashed_jar(mods_dir)?
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

pub(super) fn stashed_jar(mods_dir: &Path) -> Option<PathBuf> {
    let state = load_json::<State>(&state_path(mods_dir)).ok()??;
    super::files::check_file_name(&state.original).ok()?;
    let path = stash_dir(mods_dir).join(state.original);
    path.is_file().then_some(path)
}

pub(super) fn generated_files(mods_dir: &Path) -> Vec<String> {
    load_json::<State>(&state_path(mods_dir))
        .ok()
        .flatten()
        .map_or_else(Vec::new, |state| state.added)
}

/// Replace the big Fabric API jar with the modules the other jars use.
/// Returns whether it did (or it was done already and nothing changed since).
/// Call [`undo`] before changing the folder's mods.
pub fn apply(mods_dir: &Path) -> Result<bool> {
    if let Some(state) = load_json::<State>(&state_path(mods_dir))? {
        let mut skip: Vec<&str> = state.added.iter().map(String::as_str).collect();
        skip.push(&state.original);
        if state.stub_version == STUB_VERSION
            && state.complete(mods_dir)
            && signature(mods_dir, &skip) == state.signature
        {
            return Ok(true);
        }
        // The folder's mods changed since: start from the whole thing again.
        undo(mods_dir)?;
    }
    let Some(umbrella) = find_umbrella(mods_dir) else {
        return Ok(false);
    };
    let path = mods_dir.join(&umbrella);
    let Some((modules, version, id)) = read_modules(&path) else {
        return Ok(false);
    };
    if modules.is_empty() {
        return Ok(false);
    }
    let sig = signature(mods_dir, &[umbrella.as_str()]);
    let wanted = keep(mods_dir, &umbrella, &modules);
    // Nothing to gain when nearly everything is used.
    if wanted.len() + 8 > modules.len() {
        return Ok(false);
    }
    let stash = stash_dir(mods_dir);
    fs::create_dir_all(&stash).at(&stash)?;
    let mut zip = zip::ZipArchive::new(fs::File::open(&path).at(&path)?)
        .map_err(|e| crate::Error::Other(format!("Fabric API: {e}")))?;
    let selected: Vec<_> = modules
        .iter()
        .filter(|m| wanted.contains(&m.id))
        .filter_map(|m| Some((module_file_name(&m.entry)?, &m.entry)))
        .collect();
    let mut added: Vec<String> = selected
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    added.push(STUB.to_owned());
    for name in &added {
        super::files::check_file_name(name)?;
        if mods_dir.join(name).exists() {
            return Ok(false);
        }
    }
    let state = State {
        original: umbrella.clone(),
        added,
        signature: sig,
        stub_version: STUB_VERSION,
    };
    save_json(&state_path(mods_dir), &state)?;
    for (name, entry) in selected {
        let mut bytes = Vec::new();
        zip.by_name(entry)
            .map_err(|e| crate::Error::Other(format!("Fabric API: {e}")))?
            .read_to_end(&mut bytes)
            .at(&path)?;
        let out = mods_dir.join(name);
        fs::write(&out, bytes).at(&out)?;
    }
    let stub = mods_dir.join(STUB);
    write_stub(&stub, &id, &version)?;
    drop(zip);
    let moved = stash.join(&umbrella);
    fs::rename(&path, &moved).at(&path)?;
    log::info!(
        "Fabric API: {} of {} modules used by the other mods",
        wanted.len(),
        modules.len()
    );
    Ok(true)
}

/// A jar holding only the big jar's name (`id`), for mods that ask for it. It answers to the
/// other name too: mods made for old Fabric API ask for `fabric`, newer ones for `fabric-api`.
fn write_stub(path: &Path, id: &str, version: &str) -> Result<()> {
    let file = fs::File::create(path).at(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let other = if id == "fabric" {
        "fabric-api"
    } else {
        "fabric"
    };
    let meta = serde_json::json!({
        "schemaVersion": 1,
        "id": id,
        "provides": [other],
        "version": version,
        "name": "Fabric API (the parts in use)",
        "description": "Arctic keeps only the Fabric API modules the installed mods use, to start faster.",
    });
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .map_err(|e| crate::Error::Other(e.to_string()))?;
    zip.write_all(meta.to_string().as_bytes()).at(path)?;
    zip.finish()
        .map_err(|e| crate::Error::Other(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut out));
            for (name, data) in files {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        out
    }

    fn module(id: &str, depends: &[&str], class: &str) -> Vec<u8> {
        let deps: serde_json::Map<String, serde_json::Value> = depends
            .iter()
            .map(|d| ((*d).to_owned(), serde_json::json!("*")))
            .collect();
        let meta =
            serde_json::json!({"schemaVersion": 1, "id": id, "version": "1", "depends": deps});
        jar(&[
            ("fabric.mod.json", meta.to_string().as_bytes()),
            (&format!("{class}.class"), b"x"),
        ])
    }

    /// A Fabric API with ten modules; mod "user" uses module m3 (which needs m1).
    fn setup(dir: &Path) {
        let mods = dir.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..10 {
            let deps: Vec<&str> = if i == 3 { vec!["m1"] } else { vec![] };
            files.push((
                format!("META-INF/jars/m{i}-1.jar"),
                module(
                    &format!("m{i}"),
                    &deps,
                    &format!("net/fabricmc/fabric/api/m{i}/Thing"),
                ),
            ));
        }
        let meta = serde_json::json!({"schemaVersion": 1, "id": "fabric-api", "version": "0.1.0"});
        let owned = meta.to_string();
        let mut entries: Vec<(&str, &[u8])> = vec![("fabric.mod.json", owned.as_bytes())];
        for (n, d) in &files {
            entries.push((n.as_str(), d.as_slice()));
        }
        fs::write(mods.join("fabric-api-0.1.0.jar"), jar(&entries)).unwrap();
        let user_meta = serde_json::json!({"schemaVersion": 1, "id": "user", "version": "1", "depends": {"fabric-api": "*"}});
        fs::write(
            mods.join("user.jar"),
            jar(&[
                ("fabric.mod.json", user_meta.to_string().as_bytes()),
                ("a/B.class", b"..net/fabricmc/fabric/api/m3/Thing$Inner;.."),
            ]),
        )
        .unwrap();
    }

    #[test]
    fn keeps_only_what_is_used_and_undoes() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path());
        let mods = dir.path().join("mods");
        assert!(apply(&mods).unwrap());
        let mut names: Vec<String> = fs::read_dir(&mods)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".jar"))
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["fabric-api-stub.jar", "m1-1.jar", "m3-1.jar", "user.jar"]
        );
        assert!(stash_dir(&mods).join("fabric-api-0.1.0.jar").exists());
        undo(&mods).unwrap();
        let mut back: Vec<String> = fs::read_dir(&mods)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect();
        back.sort();
        assert_eq!(back, ["fabric-api-0.1.0.jar", "user.jar"]);
    }

    #[test]
    fn a_jar_is_read_once_for_as_long_as_it_is_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path());
        let mods = dir.path().join("mods");
        let (modules, ..) = read_modules(&mods.join("fabric-api-0.1.0.jar")).unwrap();
        let kept = keep(&mods, "fabric-api-0.1.0.jar", &modules);
        assert_eq!(
            kept.iter().map(String::as_str).collect::<Vec<_>>(),
            ["m1", "m3"]
        );
        let uses = fs::read(mods.join(USES)).unwrap();

        // The same name, size and time: what was read from it is used (not these bytes).
        let user = mods.join("user.jar");
        let (len, time) = {
            let meta = fs::metadata(&user).unwrap();
            (meta.len() as usize, meta.modified().unwrap())
        };
        fs::write(&user, vec![0u8; len]).unwrap();
        let file = fs::File::options().write(true).open(&user).unwrap();
        file.set_modified(time).unwrap();
        drop(file);
        assert_eq!(keep(&mods, "fabric-api-0.1.0.jar", &modules), kept);
        assert_eq!(
            fs::read(mods.join(USES)).unwrap(),
            uses,
            "nothing was written"
        );

        // Written again (another time): read again.
        let later = time + std::time::Duration::from_secs(2);
        let file = fs::File::options().write(true).open(&user).unwrap();
        file.set_modified(later).unwrap();
        drop(file);
        assert!(keep(&mods, "fabric-api-0.1.0.jar", &modules).is_empty());

        // Jars long gone make room; this folder's stay.
        let mut known: Uses = serde_json::from_slice(&fs::read(mods.join(USES)).unwrap()).unwrap();
        for i in 0..MAX_USES {
            let mut gone = known.jars[0].clone();
            gone.key = format!("gone-{i}.jar:1:1");
            known.jars.insert(0, gone);
        }
        fs::write(mods.join(USES), serde_json::to_string(&known).unwrap()).unwrap();
        fs::write(mods.join("new.jar"), b"not a jar").unwrap();
        keep(&mods, "fabric-api-0.1.0.jar", &modules);
        let known: Uses = serde_json::from_slice(&fs::read(mods.join(USES)).unwrap()).unwrap();
        assert_eq!(known.jars.len(), MAX_USES);
        assert!(
            known
                .jars
                .iter()
                .any(|jar| jar.key.starts_with("user.jar:"))
        );
        assert!(known.jars.iter().any(|jar| jar.key.starts_with("new.jar:")));
    }

    #[test]
    fn applying_again_changes_nothing_until_the_mods_change() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path());
        let mods = dir.path().join("mods");
        assert!(apply(&mods).unwrap());
        let stub_time = fs::metadata(mods.join("fabric-api-stub.jar"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(stashed(&mods).as_deref(), Some("fabric-api-0.1.0.jar"));
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(apply(&mods).unwrap());
        assert_eq!(
            fs::metadata(mods.join("fabric-api-stub.jar"))
                .unwrap()
                .modified()
                .unwrap(),
            stub_time,
            "nothing was rewritten"
        );
        // A mod added later is looked at: the whole jar comes back first.
        let meta = serde_json::json!({"schemaVersion": 1, "id": "extra", "version": "1"});
        fs::write(
            mods.join("extra.jar"),
            jar(&[
                ("fabric.mod.json", meta.to_string().as_bytes()),
                ("a/D.class", b"net/fabricmc/fabric/api/m7/Thing"),
            ]),
        )
        .unwrap();
        // (With three of the ten modules in use there is too little to gain: the whole jar stays.)
        assert!(!apply(&mods).unwrap());
        assert!(mods.join("fabric-api-0.1.0.jar").exists());
        assert!(!mods.join("m1-1.jar").exists());
    }

    #[test]
    fn leaves_the_jar_alone_when_most_of_it_is_used() {
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path());
        let mods = dir.path().join("mods");
        // A mod that names nearly every module.
        let body: String = (0..10)
            .map(|i| format!("net/fabricmc/fabric/api/m{i}/Thing "))
            .collect();
        fs::write(mods.join("big.jar"), jar(&[("a/C.class", body.as_bytes())])).unwrap();
        assert!(!apply(&mods).unwrap());
        assert!(mods.join("fabric-api-0.1.0.jar").exists());
    }

    fn stub_meta(path: &Path) -> serde_json::Value {
        let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut text = String::new();
        zip.by_name("fabric.mod.json")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn the_stand_in_answers_to_both_names() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.jar");
        write_stub(&old, "fabric", "0.42.0").unwrap();
        let meta = stub_meta(&old);
        assert_eq!(meta["id"], "fabric");
        assert_eq!(meta["provides"], serde_json::json!(["fabric-api"]));
        let new = dir.path().join("new.jar");
        write_stub(&new, "fabric-api", "0.100.0").unwrap();
        let meta = stub_meta(&new);
        assert_eq!(meta["id"], "fabric-api");
        assert_eq!(meta["provides"], serde_json::json!(["fabric"]));
    }
}
