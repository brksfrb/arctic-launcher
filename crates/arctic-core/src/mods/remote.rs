//! What linked instances and the missing-mods flow need from Modrinth and an
//! instance's `mods.json`, as plain data: exact versions with their file
//! hashes, a mod id (as a server names it) looked up as a Modrinth project,
//! and the index read and written as a whole.

use std::path::Path;

use super::index::ModIndex;
use super::modrinth::{self, Version};
use super::{InstalledMod, files};
use crate::loaders::LoaderKind;
use crate::{Error, Result};

/// Versions per Modrinth request (keeps the URL short).
const BATCH: usize = 100;
/// Longest mod id a server can name that is looked up.
const MAX_MOD_ID: usize = 64;

/// One downloadable file of a Modrinth version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub url: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
}

/// A Modrinth version: what it runs on and its files (primary first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteVersion {
    pub project_id: String,
    pub version_id: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<RemoteFile>,
}

impl RemoteVersion {
    /// Made for `game_version` and one of the loaders that run on `loader`.
    pub fn fits(&self, game_version: &str, loader: LoaderKind) -> bool {
        let loaders = modrinth::compatible_loaders(loader, game_version);
        self.game_versions.iter().any(|g| g == game_version)
            && self.loaders.iter().any(|l| loaders.contains(&l.as_str()))
    }

    /// The file with this SHA-512 (how a shared manifest names the file).
    pub fn file_with_sha512(&self, sha512: &str) -> Option<&RemoteFile> {
        self.files.iter().find(|f| {
            f.sha512
                .as_deref()
                .is_some_and(|h| h.eq_ignore_ascii_case(sha512))
        })
    }
}

fn to_remote(v: &Version) -> RemoteVersion {
    let mut files: Vec<RemoteFile> = v
        .files
        .iter()
        .map(|f| RemoteFile {
            project_id: v.project_id.clone(),
            version_id: v.id.clone(),
            version_number: v.version_number.clone(),
            file_name: f.filename.clone(),
            url: f.url.clone(),
            sha1: f.hashes.sha1.clone(),
            sha512: f.hashes.sha512.clone(),
            size: f.size,
        })
        .collect();
    // Primary first, the way installs pick it.
    if let Some(primary) = v.primary_file() {
        files.sort_by_key(|f| f.file_name != primary.filename);
    }
    RemoteVersion {
        project_id: v.project_id.clone(),
        version_id: v.id.clone(),
        game_versions: v.game_versions.clone(),
        loaders: v.loaders.clone(),
        files,
    }
}

/// Exact versions by id; ids Modrinth doesn't know are missing from the answer.
pub fn versions_by_id(ids: &[&str]) -> Result<Vec<RemoteVersion>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(BATCH) {
        out.extend(modrinth::versions(chunk)?.iter().map(to_remote));
    }
    Ok(out)
}

/// Titles and icons of projects (cosmetic: an empty map when Modrinth can't say).
pub fn project_titles(ids: &[&str]) -> std::collections::HashMap<String, (String, Option<String>)> {
    let mut out = std::collections::HashMap::new();
    for chunk in ids.chunks(BATCH) {
        match modrinth::projects(chunk) {
            Ok(projects) => {
                for p in projects {
                    if !p.title.trim().is_empty() {
                        out.insert(p.id.clone(), (p.title, modrinth::non_empty(p.icon_url)));
                    }
                }
            }
            Err(e) => log::warn!("could not fetch mod titles: {e}"),
        }
    }
    out
}

/// A mod a server named (its mod id, as in `fabric.mod.json` or `mods.toml`) as a Modrinth
/// project: tried as a slug (and with `_` as `-`), the way most mods are published.
/// `version_hint` (the version the server runs) picks the version whose
/// number holds it when there is one; otherwise the newest that fits. `None` when Modrinth
/// has no such project or no version of it fits this game version and loader.
pub fn find_for_mod_id(
    mod_id: &str,
    version_hint: Option<&str>,
    game_version: &str,
    loader: LoaderKind,
) -> Result<Option<(String, RemoteFile)>> {
    #[derive(serde::Deserialize)]
    struct Found {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        project_type: String,
    }
    let id =
        clean_mod_id(mod_id).ok_or_else(|| Error::Other(format!("{mod_id:?} is not a mod id")))?;
    for slug in slug_guesses(&id) {
        let url = format!("{}/project/{}", modrinth::API, modrinth::encode(&slug));
        let project: Found = match modrinth::get_api(&url, &format!("project {slug}")) {
            Ok(p) => p,
            Err(Error::Other(msg)) if msg.contains("not found") => continue,
            Err(e) => return Err(e),
        };
        if project.project_type != "mod" && !project.project_type.is_empty() {
            continue;
        }
        if let Some(file) = fitting_file(&project.id, version_hint, game_version, loader)? {
            return Ok(Some((title_or(&project.title, &id), file)));
        }
    }
    Ok(None)
}

fn title_or(title: &str, id: &str) -> String {
    if title.trim().is_empty() {
        id.to_owned()
    } else {
        title.to_owned()
    }
}

/// The file of the project's version for this game and loader (the hinted one if any).
fn fitting_file(
    project: &str,
    version_hint: Option<&str>,
    game_version: &str,
    loader: LoaderKind,
) -> Result<Option<RemoteFile>> {
    let loaders = modrinth::compatible_loaders(loader, game_version);
    let versions = modrinth::project_versions(project, &loaders, game_version)?;
    let picked = pick_with_hint(&versions, version_hint)
        .or_else(|| modrinth::pick_version(&versions, &loaders, game_version));
    Ok(picked
        .map(to_remote)
        .and_then(|v| v.files.into_iter().next())
        .filter(|f| files::check_file_name(&f.file_name).is_ok()))
}

/// A version whose number holds the server's version (`0.5.1` in `mc1.21-0.5.1`).
fn pick_with_hint<'a>(versions: &'a [Version], hint: Option<&str>) -> Option<&'a Version> {
    let hint = hint?.trim();
    if hint.is_empty() || hint.len() > MAX_MOD_ID {
        return None;
    }
    versions
        .iter()
        .filter(|v| v.primary_file().is_some())
        .find(|v| v.version_number == hint)
        .or_else(|| {
            versions
                .iter()
                .filter(|v| v.primary_file().is_some())
                .find(|v| v.version_number.contains(hint))
        })
}

/// Mod ids are lowercase letters, digits, `_` and `-` (Fabric and Forge both).
pub fn clean_mod_id(id: &str) -> Option<String> {
    let id = id.trim().to_ascii_lowercase();
    (!id.is_empty()
        && id.len() <= MAX_MOD_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'))
    .then_some(id)
}

/// Slugs to try for a mod id: itself, and with `_` as `-` (Modrinth slugs use dashes).
fn slug_guesses(id: &str) -> Vec<String> {
    let mut out = vec![id.to_owned()];
    let dashed = id.replace('_', "-");
    if dashed != id {
        out.push(dashed);
    }
    out
}

/// Everything in an instance's `mods.json` (empty when there is none).
pub fn index_entries(index: &Path) -> Result<Vec<InstalledMod>> {
    Ok(ModIndex::load(index)?.mods)
}

/// Write an instance's `mods.json` as a whole.
pub fn save_index(index: &Path, mods: Vec<InstalledMod>) -> Result<()> {
    ModIndex { mods }.save(index)
}

/// A plain `*.jar` file name (nothing that could leave the mods folder).
pub fn is_jar_name(name: &str) -> bool {
    files::check_file_name(name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_ids_are_cleaned_and_guessed() {
        assert_eq!(clean_mod_id(" Create "), Some("create".into()));
        assert_eq!(clean_mod_id("../x"), None);
        assert_eq!(clean_mod_id(""), None);
        assert_eq!(clean_mod_id("a b"), None);
        assert_eq!(
            slug_guesses("cloth_config"),
            vec!["cloth_config", "cloth-config"]
        );
        assert_eq!(slug_guesses("sodium"), vec!["sodium"]);
    }

    #[test]
    fn jar_names_stay_plain() {
        assert!(is_jar_name("sodium-0.6.jar"));
        assert!(!is_jar_name("../evil.jar"));
        assert!(!is_jar_name("dir/evil.jar"));
        assert!(!is_jar_name("evil.exe"));
    }
}
