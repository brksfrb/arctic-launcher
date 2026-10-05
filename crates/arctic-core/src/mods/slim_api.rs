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

/// Where the big jar waits (next to the mods folder).
const STASH: &str = "mods-full-api";
/// What was done, so it can be undone (inside the mods folder; loaders ignore it).
const STATE: &str = ".arctic-slim-api.json";
const CACHE: &str = ".arctic-slim-api-cache.json";
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
}

fn state_path(mods_dir: &Path) -> PathBuf {
    mods_dir.join(STATE)
}

fn stash_dir(mods_dir: &Path) -> PathBuf {
    mods_dir.parent().unwrap_or(mods_dir).join(STASH)
}

/// Put the folder back as it was before [`apply`].
pub fn undo(mods_dir: &Path) -> Result<()> {
    let path = state_path(mods_dir);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let state: State = serde_json::from_str(&text).unwrap_or_default();
    for name in &state.added {
        let file = mods_dir.join(name);
        if file.exists() {
            fs::remove_file(&file).at(&file)?;
        }
    }
    if !state.original.is_empty() {
        let from = stash_dir(mods_dir).join(&state.original);
        let to = mods_dir.join(&state.original);
        if from.exists() && !to.exists() {
            fs::rename(&from, &to).at(&from)?;
        }
    }
    fs::remove_file(&path).at(&path)
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
        .find(|n| n.starts_with("fabric-api-") && n.ends_with(".jar") && n != STUB)
}

fn read_modules(umbrella: &Path) -> Option<(Vec<Module>, String)> {
    let mut zip = zip::ZipArchive::new(fs::File::open(umbrella).ok()?).ok()?;
    let version = {
        let mut text = String::new();
        zip.by_name("fabric.mod.json")
            .ok()?
            .read_to_string(&mut text)
            .ok()?;
        serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
            .ok()?
            .get("version")?
            .as_str()?
            .to_owned()
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
    Some((modules, version))
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

/// The modules (ids) to keep: those the other jars refer to or declare, and what those need.
fn keep(mods_dir: &Path, umbrella: &str, modules: &[Module]) -> BTreeSet<String> {
    let owner: HashMap<&str, &str> = modules
        .iter()
        .flat_map(|m| m.classes.iter().map(|c| (c.as_str(), m.id.as_str())))
        .collect();
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for entry in fs::read_dir(mods_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".jar") || name == umbrella || name == STUB {
            continue;
        }
        let Ok(file) = fs::File::open(entry.path()) else {
            continue;
        };
        referenced_classes(file, &mut referenced);
        // What the jar says it needs.
        if let Ok(file) = fs::File::open(entry.path())
            && let Ok(mut zip) = zip::ZipArchive::new(file)
            && let Ok(mut meta) = zip.by_name("fabric.mod.json")
        {
            let mut text = String::new();
            if meta.read_to_string(&mut text).is_ok()
                && let Ok(json) =
                    serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
                && let Some(depends) = json.get("depends").and_then(|d| d.as_object())
            {
                for id in depends.keys() {
                    if modules.iter().any(|m| &m.id == id) {
                        wanted.insert(id.clone());
                    }
                }
            }
        }
    }
    for class in &referenced {
        let id = owner
            .get(class.as_str())
            .or_else(|| owner.get(class.split('$').next().unwrap_or(class)));
        if let Some(id) = id {
            wanted.insert((*id).to_owned());
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

#[derive(Serialize, Deserialize)]
struct Cache {
    signature: String,
    keep: BTreeSet<String>,
}

fn cached(mods_dir: &Path, sig: &str) -> Option<BTreeSet<String>> {
    let text = fs::read_to_string(mods_dir.join(CACHE)).ok()?;
    let cache: Cache = serde_json::from_str(&text).ok()?;
    (cache.signature == sig).then_some(cache.keep)
}

fn store(mods_dir: &Path, sig: &str, keep: &BTreeSet<String>) {
    let cache = Cache {
        signature: sig.to_owned(),
        keep: keep.clone(),
    };
    if let Ok(text) = serde_json::to_string(&cache) {
        let _ = fs::write(mods_dir.join(CACHE), text);
    }
}

/// The big jar's file name while it is put aside (so that "is everything there" checks can count it).
pub fn stashed(mods_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(state_path(mods_dir)).ok()?;
    let state: State = serde_json::from_str(&text).ok()?;
    (!state.original.is_empty()).then_some(state.original)
}

/// Replace the big Fabric API jar with the modules the other jars use.
/// Returns whether it did (or it was done already and nothing changed since).
/// Call [`undo`] before changing the folder's mods.
pub fn apply(mods_dir: &Path) -> Result<bool> {
    if let Ok(text) = fs::read_to_string(state_path(mods_dir))
        && let Ok(state) = serde_json::from_str::<State>(&text)
    {
        let mut skip: Vec<&str> = state.added.iter().map(String::as_str).collect();
        skip.push(&state.original);
        if signature(mods_dir, &skip) == state.signature {
            return Ok(true);
        }
        // The folder's mods changed since: start from the whole thing again.
        undo(mods_dir)?;
    }
    let Some(umbrella) = find_umbrella(mods_dir) else {
        return Ok(false);
    };
    let path = mods_dir.join(&umbrella);
    let Some((modules, version)) = read_modules(&path) else {
        return Ok(false);
    };
    if modules.is_empty() {
        return Ok(false);
    }
    let sig = signature(mods_dir, &[umbrella.as_str()]);
    let wanted = match cached(mods_dir, &sig) {
        Some(ids) => ids,
        None => {
            let ids = keep(mods_dir, &umbrella, &modules);
            store(mods_dir, &sig, &ids);
            ids
        }
    };
    // Nothing to gain when nearly everything is used.
    if wanted.len() + 8 > modules.len() {
        return Ok(false);
    }
    let stash = stash_dir(mods_dir);
    fs::create_dir_all(&stash).at(&stash)?;
    let mut zip = zip::ZipArchive::new(fs::File::open(&path).at(&path)?)
        .map_err(|e| crate::Error::Other(format!("Fabric API: {e}")))?;
    let mut added = Vec::new();
    for module in modules.iter().filter(|m| wanted.contains(&m.id)) {
        let mut bytes = Vec::new();
        zip.by_name(&module.entry)
            .map_err(|e| crate::Error::Other(format!("Fabric API: {e}")))?
            .read_to_end(&mut bytes)
            .at(&path)?;
        let name = module
            .entry
            .rsplit('/')
            .next()
            .unwrap_or(&module.entry)
            .to_owned();
        let out = mods_dir.join(&name);
        fs::write(&out, bytes).at(&out)?;
        added.push(name);
    }
    let stub = mods_dir.join(STUB);
    write_stub(&stub, &version)?;
    added.push(STUB.to_owned());
    drop(zip);
    let moved = stash.join(&umbrella);
    if moved.exists() {
        fs::remove_file(&moved).at(&moved)?;
    }
    fs::rename(&path, &moved).at(&path)?;
    let state = State {
        original: umbrella.clone(),
        added,
        signature: sig,
    };
    let text = serde_json::to_string_pretty(&state).unwrap_or_default();
    let state_file = state_path(mods_dir);
    fs::write(&state_file, text).at(&state_file)?;
    log::info!(
        "Fabric API: {} of {} modules used by the other mods",
        wanted.len(),
        modules.len()
    );
    Ok(true)
}

/// A jar holding only the `fabric-api` name, for mods that ask for it.
fn write_stub(path: &Path, version: &str) -> Result<()> {
    let file = fs::File::create(path).at(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let meta = serde_json::json!({
        "schemaVersion": 1,
        "id": "fabric-api",
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
}
