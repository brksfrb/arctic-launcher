//! Arctic doesn't support CurseForge: the one place that recognises CurseForge-only formats
//! (a modpack zip, an app instance folder, a link) so every import entry can refuse them.
//!
//! Plain `.jar` files are never looked at here: a jar is a jar wherever it came from.

use std::fs;
use std::io::{Read, Seek};
use std::path::Path;

use crate::error::{Error, Result};

/// What the player is told (translated in the launcher under `instances.curseforge_not_supported`).
pub const MESSAGE: &str =
    "Arctic doesn't support CurseForge. Look for this pack on Modrinth instead.";

/// The CurseForge app's instance metadata file.
const INSTANCE_FILE: &str = "minecraftinstance.json";
/// A manifest bigger than this isn't read.
const MAX_MANIFEST: u64 = 16 * 1024 * 1024;

fn blocked() -> Error {
    Error::Other(MESSAGE.into())
}

/// Whether this zip is a CurseForge modpack: a `manifest.json` (at the root or inside a single
/// root folder) with `"manifestType": "minecraftModpack"` and `files` with `projectID`/`fileID`.
/// A zip holding `modrinth.index.json` is a Modrinth pack and never matches.
pub fn zip_is_curseforge_pack<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> bool {
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    if names.iter().any(|n| n == "modrinth.index.json") {
        return false;
    }
    let manifests = names.iter().filter(|n| {
        let n = n.as_str();
        n == "manifest.json"
            || n.strip_suffix("/manifest.json")
                .is_some_and(|dir| !dir.is_empty() && !dir.contains('/'))
    });
    for name in manifests {
        let Ok(entry) = zip.by_name(name) else {
            continue;
        };
        if entry.size() > MAX_MANIFEST {
            continue;
        }
        let mut text = Vec::new();
        if entry.take(MAX_MANIFEST).read_to_end(&mut text).is_err() {
            continue;
        }
        if is_curseforge_manifest(&text) {
            return true;
        }
    }
    false
}

fn is_curseforge_manifest(text: &[u8]) -> bool {
    // A byte-order mark is common in manifests written on Windows.
    let text = text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(text);
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(text) else {
        return false;
    };
    json.get("manifestType").and_then(|v| v.as_str()) == Some("minecraftModpack")
        && json
            .get("files")
            .and_then(|v| v.as_array())
            .is_some_and(|files| {
                files
                    .iter()
                    .any(|f| f.get("projectID").is_some() && f.get("fileID").is_some())
            })
}

/// Whether the file at `path` is a CurseForge modpack zip. Anything that isn't a readable zip is not.
pub fn is_curseforge_pack(path: &Path) -> bool {
    fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|mut z| zip_is_curseforge_pack(&mut z))
}

/// Whether this folder is a CurseForge app instance (it holds `minecraftinstance.json`).
pub fn is_curseforge_instance(dir: &Path) -> bool {
    dir.join(INSTANCE_FILE).is_file()
}

/// Whether this text is a CurseForge link: the `curseforge:` scheme, or a CurseForge web or CDN address.
pub fn is_curseforge_link(link: &str) -> bool {
    let link = link.trim();
    let lower = link.to_ascii_lowercase();
    if lower.starts_with("curseforge:") {
        return true;
    }
    let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    ["curseforge.com", "forgecdn.net"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Refuse a CurseForge modpack zip (anything else, even a non-zip, passes).
pub fn check_file(path: &Path) -> Result<()> {
    if is_curseforge_pack(path) {
        Err(blocked())
    } else {
        Ok(())
    }
}

/// Refuse a CurseForge app instance folder.
pub fn check_dir(dir: &Path) -> Result<()> {
    if is_curseforge_instance(dir) {
        Err(blocked())
    } else {
        Ok(())
    }
}

/// Refuse a CurseForge link.
pub fn check_link(link: &str) -> Result<()> {
    if is_curseforge_link(link) {
        Err(blocked())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
