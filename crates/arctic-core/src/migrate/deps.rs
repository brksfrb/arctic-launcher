//! Required mods that aren't there. Packs from other launchers are
//! sometimes missing one (the game then stops at startup); Arctic reads
//! every mod's required dependencies, counts mods bundled inside other
//! mods, and fetches what's missing from Modrinth when it can.

use std::collections::HashSet;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::loaders::LoaderKind;

/// Ids the loaders and the game provide themselves.
const BUILT_IN: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "forge",
    "neoforge",
    "fml",
    "javafml",
    "lowcodefml",
    "mixinextras",
    "mcp",
];
/// How deep to look inside jars bundled in jars.
const MAX_NESTING: u8 = 3;

/// A required mod nobody provides: (who needs it, what it needs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub by: String,
    pub id: String,
}

#[derive(Default)]
struct Ids {
    provided: HashSet<String>,
    required: Vec<(String, String)>,
}

/// Required mods that no jar in `mods_dir` (switched-off ones don't count)
/// provides.
pub fn missing(mods_dir: &Path, loader: LoaderKind) -> Vec<Missing> {
    let mut ids = Ids::default();
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".jar") {
            continue;
        }
        if let Ok(file) = std::fs::File::open(e.path()) {
            read_jar(file, 0, Some(loader), false, &mut ids);
        }
    }
    let mut seen = HashSet::new();
    ids.required
        .into_iter()
        .filter(|(_, id)| !BUILT_IN.contains(&id.as_str()) && !ids.provided.contains(id))
        .filter(|(_, id)| seen.insert(id.clone()))
        .map(|(by, id)| Missing { by, id })
        .collect()
}

/// Enabled jar paths, including paths whose names used an uppercase extension.
pub(crate) fn jars(mods_dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(mods_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
        })
}

/// Requirements and providers of a Fabric mod, including its bundled jars.
/// Quilt also recognizes the ids in Quilt's metadata.
pub(crate) fn fabric_ids(path: &Path, loader: LoaderKind) -> (HashSet<String>, HashSet<String>) {
    let mut ids = Ids::default();
    if let Ok(file) = std::fs::File::open(path) {
        read_jar(file, 0, Some(loader), true, &mut ids);
    }
    let required = ids
        .required
        .into_iter()
        .map(|(_, id)| id)
        .filter(|id| !BUILT_IN.contains(&id.as_str()))
        .collect();
    (ids.provided, required)
}

/// Ids a jar provides (itself, `provides`, bundled jars) and, for the
/// selected loader, what it requires. The launch check also counts the
/// requirements of bundled jars.
fn read_jar<R: Read + Seek>(
    reader: R,
    depth: u8,
    loader: Option<LoaderKind>,
    launch: bool,
    ids: &mut Ids,
) {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else {
        return;
    };
    let mut text = |name: &str| {
        let mut s = String::new();
        zip.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let mut nested: Vec<String> = Vec::new();
    let counts =
        |file: &str| loader.is_some_and(|l| super::detect::metadata_files(l).contains(&file));
    let quilt = (launch && loader == Some(LoaderKind::Quilt))
        .then(|| text("quilt.mod.json"))
        .flatten();
    if let Some(quilt) = &quilt
        && let Ok(json) = serde_json::from_str::<Value>(quilt)
        && let Some(meta) = json.get("quilt_loader")
    {
        if let Some(id) = meta.get("id").and_then(Value::as_str) {
            ids.provided.insert(id.to_owned());
        }
        ids.provided.extend(
            meta["provides"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p.as_str().or_else(|| p["id"].as_str()))
                .map(str::to_owned),
        );
        nested.extend(
            meta["jars"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|j| j.as_str().map(str::to_owned)),
        );
    }
    let fabric_needs = counts("fabric.mod.json")
        && (loader != Some(LoaderKind::Quilt) || launch)
        && quilt.is_none();
    if let Some(json) = quilt
        .is_none()
        .then(|| text("fabric.mod.json"))
        .flatten()
        .and_then(|t| {
            serde_json::from_str::<Value>(
                &t.trim_start_matches('\u{feff}')
                    .replace(['\n', '\r', '\t'], " "),
            )
            .ok()
        })
    {
        if launch && json["environment"] == "server" {
            return;
        }
        let id = json
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        ids.provided.insert(id.clone());
        for p in json
            .get("provides")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(p) = p.as_str() {
                ids.provided.insert(p.to_owned());
            }
        }
        if fabric_needs && let Some(deps) = json.get("depends").and_then(Value::as_object) {
            ids.required
                .extend(deps.keys().map(|d| (id.clone(), d.clone())));
        }
        nested.extend(
            json.get("jars")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|j| j.get("file").and_then(Value::as_str).map(str::to_owned)),
        );
    }
    if !launch {
        // The migration scan also checks Forge-family manifests.
        let neo = text("META-INF/neoforge.mods.toml");
        let forge = text("META-INF/mods.toml");
        let neo_needs = counts("META-INF/neoforge.mods.toml");
        let forge_needs = counts("META-INF/mods.toml") && !(neo_needs && neo.is_some());
        if let Some(toml) = neo {
            read_toml(&toml, neo_needs, ids);
        }
        if let Some(toml) = forge {
            read_toml(&toml, forge_needs, ids);
        }
        if let Some(meta) = text("META-INF/jarjar/metadata.json")
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            nested.extend(
                meta.get("jars")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|j| j.get("path").and_then(Value::as_str).map(str::to_owned)),
            );
        }
    }
    if depth >= MAX_NESTING {
        return;
    }
    for path in nested {
        let mut bytes = Vec::new();
        if zip
            .by_name(&path)
            .ok()
            .and_then(|mut f| f.read_to_end(&mut bytes).ok())
            .is_some()
        {
            read_jar(
                Cursor::new(bytes),
                depth + 1,
                if launch { loader } else { None },
                launch,
                ids,
            );
        }
    }
}

/// `[[mods]] modId` entries, and `required` / `mandatory` dependencies
/// that the client needs.
fn read_toml(toml: &str, top: bool, ids: &mut Ids) {
    let mut section = String::new();
    let mut dep = DepBlock::default();
    let finish = |dep: &mut DepBlock, ids: &mut Ids| {
        if top && dep.required && dep.client && !dep.id.is_empty() {
            ids.required.push((dep.owner.clone(), dep.id.clone()));
        }
        *dep = DepBlock::default();
    };
    for line in toml.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with("[[") {
            finish(&mut dep, ids);
            section = line.trim_matches(['[', ']']).trim().to_owned();
            if let Some(o) = section.strip_prefix("dependencies.") {
                dep.owner = o.trim_matches('"').to_owned();
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        if section == "mods" && k == "modId" {
            ids.provided.insert(v.to_owned());
        } else if section.starts_with("dependencies") {
            match k {
                "modId" => dep.id = v.to_owned(),
                "mandatory" => dep.required = v == "true",
                "type" => dep.required = v.eq_ignore_ascii_case("required"),
                "side" => dep.client = !v.eq_ignore_ascii_case("SERVER"),
                _ => {}
            }
        }
    }
    finish(&mut dep, ids);
}

struct DepBlock {
    owner: String,
    id: String,
    required: bool,
    client: bool,
}

impl Default for DepBlock {
    fn default() -> Self {
        Self {
            owner: String::new(),
            id: String::new(),
            required: false,
            client: true,
        }
    }
}
