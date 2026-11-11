//! Bedrock packs (resource, behavior and skin packs) in the shared
//! `com.mojang` folder: list them, import `.mcpack` / `.mcaddon` files (or a
//! pack folder), and remove them (to a trash folder).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Player, archive};
use crate::error::IoContext;
use crate::{Error, Result};

/// What a pack changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PackKind {
    Resource,
    Behavior,
    Skin,
}

impl PackKind {
    pub const ALL: [PackKind; 3] = [PackKind::Resource, PackKind::Behavior, PackKind::Skin];

    pub fn name(self) -> &'static str {
        match self {
            PackKind::Resource => "Resource pack",
            PackKind::Behavior => "Behavior pack",
            PackKind::Skin => "Skin pack",
        }
    }

    fn folder(self, development: bool) -> &'static str {
        match (self, development) {
            (PackKind::Resource, false) => "resource_packs",
            (PackKind::Behavior, false) => "behavior_packs",
            (PackKind::Skin, false) => "skin_packs",
            (PackKind::Resource, true) => "development_resource_packs",
            (PackKind::Behavior, true) => "development_behavior_packs",
            (PackKind::Skin, true) => "development_skin_packs",
        }
    }

    /// The kind a manifest's modules declare.
    fn of_manifest(manifest: &Value) -> Option<Self> {
        let modules = manifest["modules"].as_array()?;
        let types: Vec<&str> = modules.iter().filter_map(|m| m["type"].as_str()).collect();
        if types.contains(&"skin_pack") {
            Some(PackKind::Skin)
        } else if types.contains(&"resources") {
            Some(PackKind::Resource)
        } else if types
            .iter()
            .any(|t| matches!(*t, "data" | "script" | "javascript" | "client_data"))
        {
            Some(PackKind::Behavior)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub kind: PackKind,
    pub name: String,
    pub description: String,
    /// "1.0.0".
    pub version: String,
    pub uuid: String,
    pub path: PathBuf,
    /// `pack_icon.png`, if the pack has one.
    pub icon: Option<PathBuf>,
    /// In a `development_*` folder (reloaded by the game without importing).
    pub development: bool,
    pub size: u64,
}

/// The packs installed for everyone, by kind, then name.
pub fn list(packs: &Player) -> Vec<Pack> {
    list_in(&packs.com_mojang)
}

/// [`list`] for a `com.mojang` folder.
pub fn list_in(com_mojang: &Path) -> Vec<Pack> {
    let mut out = Vec::new();
    for kind in PackKind::ALL {
        for development in [false, true] {
            let Ok(entries) = fs::read_dir(com_mojang.join(kind.folder(development))) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Some(pack) = read_pack(&entry.path(), kind, development) {
                    out.push(pack);
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

fn read_pack(dir: &Path, kind: PackKind, development: bool) -> Option<Pack> {
    let bytes = fs::read(dir.join("manifest.json")).ok()?;
    let manifest = parse_manifest(&bytes)?;
    let header = &manifest["header"];
    let lang = fs::read_to_string(dir.join("texts").join("en_US.lang")).unwrap_or_default();
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let name = translate(header["name"].as_str().unwrap_or(""), &lang);
    Some(Pack {
        kind,
        name: if name.trim().is_empty() { folder } else { name },
        description: translate(header["description"].as_str().unwrap_or(""), &lang),
        version: version_text(&header["version"]),
        uuid: header["uuid"].as_str().unwrap_or("").to_ascii_lowercase(),
        path: dir.to_path_buf(),
        icon: Some(dir.join("pack_icon.png")).filter(|p| p.is_file()),
        development,
        size: archive::dir_size(dir),
    })
}

/// A pack's name or description: `pack.name` style keys are looked up in its
/// `en_US.lang`; `§` codes are dropped.
fn translate(text: &str, lang: &str) -> String {
    let text = text.trim();
    let looked_up = lang.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == text).then(|| value.split('#').next().unwrap_or(value).trim())
    });
    let text = looked_up.unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

fn version_text(v: &Value) -> String {
    match v {
        Value::Array(parts) => parts
            .iter()
            .map(|p| p.as_u64().map_or_else(|| p.to_string(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join("."),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Bedrock's manifests may hold `//` and `/* */` comments and a BOM.
fn parse_manifest(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text)
        .ok()
        .or_else(|| serde_json::from_str(&strip_comments(text)).ok())
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Import packs from a `.mcpack` / `.mcaddon` / `.zip` file or a pack
/// folder into `packs`. A pack already there (same UUID) is replaced, the
/// old copy kept in the trash. Returns the names of the packs imported.
pub fn import(from: &Path, packs: &Player) -> Result<Vec<String>> {
    if from.is_dir() {
        return import_folder(from, packs).map(|name| vec![name]);
    }
    let mut zip = archive::open(from)?;
    let mut names = Vec::new();
    let dirs = archive::dirs_with(&zip, "manifest.json", 2);
    // A pack's subpacks hold manifests too: only the outermost count.
    let roots: Vec<String> = dirs
        .iter()
        .filter(|d| {
            !dirs
                .iter()
                .any(|other| other != *d && d.starts_with(other.as_str()))
        })
        .cloned()
        .collect();
    let mut world_template = false;
    for prefix in &roots {
        let Some(manifest) = archive::read_file(&mut zip, &format!("{prefix}manifest.json"))
            .and_then(|b| parse_manifest(&b))
        else {
            continue;
        };
        let Some(kind) = PackKind::of_manifest(&manifest) else {
            world_template |= manifest["modules"]
                .as_array()
                .is_some_and(|m| m.iter().any(|m| m["type"] == "world_template"));
            continue;
        };
        let staged = packs
            .com_mojang
            .join(format!(".arctic-import-{}", uuid::Uuid::new_v4().simple()));
        archive::unpack(&mut zip, prefix, &staged, &[])?;
        let result = place(&staged, kind, &manifest, packs);
        let _ = fs::remove_dir_all(&staged);
        names.push(result?);
    }
    // An .mcaddon may also carry whole .mcpack files.
    let inner: Vec<String> = zip
        .file_names()
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with(".mcpack") && n.matches('/').count() <= 1
        })
        .map(str::to_owned)
        .collect();
    for name in inner {
        let tmp = packs.com_mojang.join(format!(
            ".arctic-import-{}.mcpack",
            uuid::Uuid::new_v4().simple()
        ));
        let copied = (|| -> Result<()> {
            let mut entry = zip
                .by_name(&name)
                .map_err(|e| Error::Other(e.to_string()))?;
            let mut file = fs::File::create(&tmp).at(&tmp)?;
            std::io::copy(&mut entry, &mut file).at(&tmp)?;
            Ok(())
        })();
        let result = copied.and_then(|()| import(&tmp, packs));
        let _ = fs::remove_file(&tmp);
        names.extend(result?);
    }
    if names.is_empty() {
        return Err(Error::Other(if world_template {
            "this is a world template: open it with the game to make a world from it".into()
        } else {
            "no Bedrock pack (manifest.json) in this file".into()
        }));
    }
    Ok(names)
}

fn import_folder(from: &Path, packs: &Player) -> Result<String> {
    let manifest = fs::read(from.join("manifest.json"))
        .ok()
        .and_then(|b| parse_manifest(&b))
        .ok_or_else(|| Error::Other(format!("{} is not a Bedrock pack", from.display())))?;
    let kind = PackKind::of_manifest(&manifest)
        .ok_or_else(|| Error::Other("this kind of pack can't be imported here".into()))?;
    let staged = packs
        .com_mojang
        .join(format!(".arctic-import-{}", uuid::Uuid::new_v4().simple()));
    archive::copy_dir(from, &staged)?;
    let result = place(&staged, kind, &manifest, packs);
    let _ = fs::remove_dir_all(&staged);
    result
}

/// Move an unpacked pack into its folder, replacing one with the same UUID.
fn place(staged: &Path, kind: PackKind, manifest: &Value, packs: &Player) -> Result<String> {
    let uuid = manifest["header"]["uuid"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    let read = read_pack(staged, kind, false)
        .ok_or_else(|| Error::Other("the pack's manifest couldn't be read".into()))?;
    let dir = packs.com_mojang.join(kind.folder(false));
    fs::create_dir_all(&dir).at(&dir)?;
    if !uuid.is_empty()
        && let Some(old) = list(packs)
            .into_iter()
            .find(|p| p.kind == kind && !p.development && p.uuid == uuid)
    {
        trash(&old, packs)?;
    }
    let target = archive::free_path(&dir, &archive::safe_name(&read.name, "Pack"));
    fs::rename(staged, &target).at(&target)?;
    Ok(read.name)
}

/// Move a pack to the trash folder (kept, not deleted).
pub fn trash(pack: &Pack, packs: &Player) -> Result<()> {
    archive::trash(&pack.path, &packs.trash_dir().join("packs"))
}

/// Save a pack as a `.mcpack` file.
pub fn export(pack: &Pack, to: &Path) -> Result<()> {
    archive::zip_dir(&pack.path, to)
}

#[cfg(test)]
pub(crate) fn test_manifest(kind: &str, name: &str, uuid: &str) -> String {
    format!(
        r#"{{
  // made by a tool that writes comments
  "format_version": 2,
  "header": {{ "name": "{name}", "description": "pack.description", "uuid": "{uuid}", "version": [1, 2, 0] }},
  "modules": [ {{ "type": "{kind}", "uuid": "00000000-0000-0000-0000-000000000001", "version": [1, 0, 0] }} ]
}}"#
    )
}
