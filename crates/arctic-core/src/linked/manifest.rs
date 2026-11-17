//! The shared manifest: an instance's Minecraft version, loader, exact
//! Modrinth mod files (with their hashes) and its config files. Nothing else
//! of the instance is ever in it: no worlds, options, key bindings, server
//! list, accounts, Java paths or JVM flags.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

use super::paths::{self, CONFIG_DIR, Refused};
use crate::instances::Loader;
use crate::mods::{self, remote::RemoteVersion};
use crate::{Error, Result};

/// Manifest format version. A reader refuses anything newer.
pub const FORMAT: u32 = 1;
/// Largest manifest (JSON) read or sent; the server's limit too.
pub const MAX_MANIFEST: usize = 3 * 1024 * 1024;
pub const MAX_MODS: usize = 1000;
pub const MAX_CONFIGS: usize = 1000;
/// A config file larger than this stays home.
pub const MAX_CONFIG_FILE: u64 = 256 * 1024;
/// All config files together.
pub const MAX_CONFIG_TOTAL: u64 = 1536 * 1024;
const MAX_NAME: usize = 48;
const MAX_TITLE: usize = 100;
const MAX_ID: usize = 32;
const MAX_VERSION: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// [`FORMAT`] of the writer.
    pub arctic_link: u32,
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
    #[serde(default)]
    pub mods: Vec<LinkedMod>,
    #[serde(default)]
    pub configs: Vec<ConfigFile>,
}

/// One mod: an exact Modrinth version and the hashes of its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedMod {
    /// Modrinth project and version ids.
    pub project: String,
    pub version: String,
    pub title: String,
    /// The file's name in the mods folder.
    pub file: String,
    pub size: u64,
    /// Lowercase hex.
    pub sha1: String,
    pub sha512: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Installed only because another mod needs it.
    #[serde(default)]
    pub dependency: bool,
}

fn yes() -> bool {
    true
}

/// One config file, carried whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFile {
    /// `config/…`, `/` between folders (see [`paths::check_config_path`]).
    pub path: String,
    pub size: u64,
    /// Lowercase hex SHA-256 of the contents.
    pub sha256: String,
    /// The contents, base64.
    pub data: String,
}

impl ConfigFile {
    pub fn new(path: String, bytes: &[u8]) -> Self {
        Self {
            path,
            size: bytes.len() as u64,
            sha256: sha256_hex(bytes),
            data: STANDARD.encode(bytes),
        }
    }

    /// The contents, checked against the size and hash.
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let bytes = STANDARD
            .decode(&self.data)
            .map_err(|_| Error::Other(format!("{} is damaged in the manifest", self.path)))?;
        if bytes.len() as u64 != self.size || sha256_hex(&bytes) != self.sha256 {
            return Err(Error::Checksum(self.path.clone()));
        }
        Ok(bytes)
    }
}

/// Something of the instance that isn't in the manifest, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut {
    /// A mod file name or a `config/…` path.
    pub name: String,
    pub reason: LeftOutReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftOutReason {
    /// A jar added by hand (or from another site): only Modrinth mods are shared.
    NotFromModrinth,
    /// The jar isn't the file Modrinth has for that version (changed, or another build).
    DiffersFromModrinth,
    /// A config file over [`MAX_CONFIG_FILE`], or past [`MAX_CONFIG_TOTAL`].
    TooBig,
    Config(Refused),
}

impl LeftOutReason {
    pub fn message(self) -> &'static str {
        match self {
            LeftOutReason::NotFromModrinth => "not from Modrinth",
            LeftOutReason::DiffersFromModrinth => "not the file Modrinth has for it",
            LeftOutReason::TooBig => "too big",
            LeftOutReason::Config(r) => r.message(),
        }
    }
}

/// A manifest built from an instance, and what was left out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub manifest: Manifest,
    pub left_out: Vec<LeftOut>,
}

/// What [`build`] reads.
pub struct Source<'a> {
    pub name: &'a str,
    pub game_version: &'a str,
    pub loader: &'a Loader,
    pub game_dir: &'a Path,
    /// The instance's `mods.json`.
    pub index: &'a Path,
    pub include_configs: bool,
}

/// Mod files the launcher puts in every instance itself (the Arctic Client, performance mods):
/// never shared, never part of a diff.
pub fn managed_files(game_dir: &Path) -> HashSet<String> {
    let mut out: HashSet<String> = mods::performance::installed_files(game_dir)
        .into_iter()
        .collect();
    out.insert(crate::arctic_mod::FILE_NAME.to_owned());
    out.insert(mods::polarium::FILE_NAME.to_owned());
    out
}

/// The manifest of an instance as it is on disk, offline: Modrinth mods with the hashes of the
/// files there; jars added by hand left out. [`verify`] then checks the hashes with Modrinth.
pub fn build(src: &Source) -> Result<Built> {
    let mods_dir = src.game_dir.join("mods");
    let managed = managed_files(src.game_dir);
    let mut out = Built {
        manifest: Manifest {
            arctic_link: FORMAT,
            name: src.name.to_owned(),
            game_version: src.game_version.to_owned(),
            loader: src.loader.clone(),
            mods: Vec::new(),
            configs: Vec::new(),
        },
        left_out: Vec::new(),
    };
    for file in mods::list(&mods_dir, src.index)? {
        if managed.contains(&file.file_name) {
            continue;
        }
        let Some(tracked) = file.tracked else {
            out.left_out.push(LeftOut {
                name: file.file_name,
                reason: LeftOutReason::NotFromModrinth,
            });
            continue;
        };
        let path = mod_path(&mods_dir, &file.file_name, file.enabled);
        let (sha1, sha512, size) = hash_file(&path)?;
        out.manifest.mods.push(LinkedMod {
            project: tracked.project_id,
            version: tracked.version_id,
            title: tracked.title,
            file: file.file_name,
            size,
            sha1,
            sha512,
            enabled: file.enabled,
            dependency: tracked.dependency,
        });
    }
    if src.include_configs {
        collect_configs(src.game_dir, &mut out)?;
    }
    out.manifest.mods.sort_by(|a, b| a.project.cmp(&b.project));
    out.manifest.configs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Keep only the mods whose file is the one Modrinth has for their version (by SHA-512);
/// the others move to the left-out list. `versions` is Modrinth's answer for the manifest's
/// version ids.
pub fn verify(mut built: Built, versions: &[RemoteVersion]) -> Built {
    let mut kept = Vec::new();
    for m in std::mem::take(&mut built.manifest.mods) {
        let ok = versions.iter().any(|v| {
            v.version_id == m.version
                && v.project_id == m.project
                && v.file_with_sha512(&m.sha512).is_some()
        });
        if ok {
            kept.push(m);
        } else {
            built.left_out.push(LeftOut {
                name: m.file,
                reason: LeftOutReason::DiffersFromModrinth,
            });
        }
    }
    built.manifest.mods = kept;
    built
}

fn collect_configs(game_dir: &Path, out: &mut Built) -> Result<()> {
    let root = game_dir.join(CONFIG_DIR);
    let mut files = Vec::new();
    walk(&root, 1, &mut files);
    files.sort();
    let mut total = 0u64;
    for file in files {
        let Some(rel) = paths::relative(game_dir, &file) else {
            continue;
        };
        if let Err(why) = paths::check_config_path(&rel) {
            out.left_out.push(LeftOut {
                name: rel,
                reason: LeftOutReason::Config(why),
            });
            continue;
        }
        let size = std::fs::metadata(&file)
            .map(|m| m.len())
            .unwrap_or(u64::MAX);
        if size > MAX_CONFIG_FILE
            || total + size > MAX_CONFIG_TOTAL
            || out.manifest.configs.len() >= MAX_CONFIGS
        {
            out.left_out.push(LeftOut {
                name: rel,
                reason: LeftOutReason::TooBig,
            });
            continue;
        }
        let bytes = std::fs::read(&file).map_err(|e| Error::io(&file, e))?;
        total += bytes.len() as u64;
        out.manifest.configs.push(ConfigFile::new(rel, &bytes));
    }
    Ok(())
}

/// Files under `dir`, never following links.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&entry.path(), depth + 1, out);
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
}

/// Where a mod file is, switched on or off.
pub fn mod_path(mods_dir: &Path, file_name: &str, enabled: bool) -> PathBuf {
    if enabled {
        mods_dir.join(file_name)
    } else {
        mods_dir.join(format!("{file_name}.disabled"))
    }
}

/// (SHA-1, SHA-512, size) of a file, lowercase hex.
pub fn hash_file(path: &Path) -> Result<(String, String, u64)> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut size = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::io(path, e))?;
        if n == 0 {
            break;
        }
        sha1.update(&buf[..n]);
        sha512.update(&buf[..n]);
        size += n as u64;
    }
    Ok((
        hex::encode(sha1.finalize()),
        hex::encode(sha512.finalize()),
        size,
    ))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

impl Manifest {
    pub fn to_json(&self) -> Result<String> {
        let text = serde_json::to_string(self)?;
        if text.len() > MAX_MANIFEST {
            return Err(Error::Other(
                "the instance is too big to link: leave out some config files".into(),
            ));
        }
        Ok(text)
    }

    /// Parse and check a manifest from the server or a file.
    pub fn from_json(text: &[u8]) -> Result<Self> {
        if text.len() > MAX_MANIFEST {
            return Err(bad("it is too big"));
        }
        let value: serde_json::Value =
            serde_json::from_slice(text).map_err(|_| bad("it isn't JSON"))?;
        let format = value
            .get("arctic_link")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| bad("it isn't a linked instance"))?;
        if format > u64::from(FORMAT) {
            return Err(Error::Other(
                "this link was made by a newer Arctic; update the launcher to use it".into(),
            ));
        }
        let manifest: Manifest =
            serde_json::from_value(value).map_err(|e| bad(&format!("damaged ({e})")))?;
        manifest.check()
    }

    /// Reject anything a normal build couldn't have made: odd ids, names or hashes, a path
    /// outside `config/`, duplicates.
    pub fn check(self) -> Result<Self> {
        let name = clean_text(&self.name, MAX_NAME).ok_or_else(|| bad("its name"))?;
        if !is_version(&self.game_version) {
            return Err(bad("its Minecraft version"));
        }
        if let Some(v) = self.loader.version()
            && !is_version(v)
        {
            return Err(bad("its loader version"));
        }
        if self.mods.len() > MAX_MODS || self.configs.len() > MAX_CONFIGS {
            return Err(bad("too many files"));
        }
        if self.loader.kind().is_none() && !self.mods.is_empty() {
            return Err(bad("mods without a mod loader"));
        }
        let mut projects = HashSet::new();
        let mut files = HashSet::new();
        let mut mods = Vec::with_capacity(self.mods.len());
        for m in self.mods {
            let ok = is_id(&m.project)
                && is_id(&m.version)
                && mods::remote::is_jar_name(&m.file)
                && is_hex(&m.sha1, 40)
                && is_hex(&m.sha512, 128)
                && projects.insert(m.project.clone())
                && files.insert(m.file.to_ascii_lowercase());
            if !ok {
                return Err(bad("a mod entry"));
            }
            let title = clean_text(&m.title, MAX_TITLE).unwrap_or_else(|| m.project.clone());
            mods.push(LinkedMod {
                title,
                sha1: m.sha1.to_ascii_lowercase(),
                sha512: m.sha512.to_ascii_lowercase(),
                ..m
            });
        }
        let mut paths_seen = HashSet::new();
        let mut total = 0u64;
        for c in &self.configs {
            paths::check_config_path(&c.path)
                .map_err(|why| bad(&format!("the config path {:?} ({})", c.path, why.message())))?;
            if !paths_seen.insert(c.path.to_ascii_lowercase())
                || c.size > MAX_CONFIG_FILE
                || !is_hex(&c.sha256, 64)
            {
                return Err(bad(&format!("the config file {:?}", c.path)));
            }
            total += c.size;
            c.bytes()?;
        }
        if total > MAX_CONFIG_TOTAL {
            return Err(bad("its config files are too big"));
        }
        Ok(Self {
            arctic_link: FORMAT,
            name,
            mods,
            ..self
        })
    }

    pub fn summary_counts(&self) -> (usize, usize) {
        (self.mods.len(), self.configs.len())
    }
}

fn bad(what: &str) -> Error {
    Error::Other(format!("this linked instance can't be used: {what}"))
}

fn clean_text(s: &str, max: usize) -> Option<String> {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    let s = s.trim();
    (!s.is_empty() && s.chars().count() <= max).then(|| s.to_owned())
}

fn is_version(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_VERSION
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+- ".contains(&b))
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ID && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}
