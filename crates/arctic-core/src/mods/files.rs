//! The mods folder itself: listing, enabling/disabling, deleting, plus the
//! on-disk icon cache.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

use super::ModFile;
use super::index::ModIndex;
use super::modrinth::{status_error, user_agent};
use crate::error::IoContext;
use crate::net::agent;
use crate::{Error, Result};

const JAR: &str = ".jar";
const DISABLED: &str = ".disabled";
/// Icons are small PNG/WebP files; anything larger is not an icon.
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// `name` without a trailing `.disabled`.
pub(super) fn base_name(name: &str) -> &str {
    name.strip_suffix(DISABLED).unwrap_or(name)
}

/// Reject anything that is not a plain `*.jar` file name, so names coming
/// from the network or the UI can never escape the mods folder.
pub(super) fn check_file_name(name: &str) -> Result<()> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control());
    if plain && name.len() > JAR.len() && name.to_ascii_lowercase().ends_with(JAR) {
        Ok(())
    } else {
        Err(Error::Other(format!("invalid mod file name: {name:?}")))
    }
}

/// A resource or shader pack file name: plain, and a `.zip`.
pub(super) fn check_pack_name(name: &str) -> Result<()> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control());
    if plain && name.to_ascii_lowercase().ends_with(".zip") {
        Ok(())
    } else {
        Err(Error::Other(format!("invalid pack file name: {name:?}")))
    }
}

fn enabled_path(mods_dir: &Path, base: &str) -> PathBuf {
    mods_dir.join(base)
}

fn disabled_path(mods_dir: &Path, base: &str) -> PathBuf {
    mods_dir.join(format!("{base}{DISABLED}"))
}

/// Every `*.jar` / `*.jar.disabled` file, joined with `index`, sorted by
/// title (or file name when untracked), case-insensitively.
pub(super) fn list(mods_dir: &Path, index: &ModIndex) -> Result<Vec<ModFile>> {
    let entries = match fs::read_dir(mods_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(mods_dir, e)),
    };
    let mut mods = Vec::new();
    for entry in entries {
        let entry = entry.at(mods_dir)?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let base = base_name(&name);
        let meta = entry.metadata().at(entry.path())?;
        if !meta.is_file() || check_file_name(base).is_err() {
            continue;
        }
        mods.push(ModFile {
            file_name: base.to_string(),
            enabled: base.len() == name.len(),
            size: meta.len(),
            tracked: index.by_file(base).cloned(),
        });
    }
    mods.sort_by_cached_key(|m| {
        let label = m.tracked.as_ref().map_or(&m.file_name, |t| &t.title);
        (label.to_lowercase(), m.file_name.to_lowercase(), !m.enabled)
    });
    Ok(mods)
}

/// Whether a mod file exists in either form.
pub(super) fn exists(mods_dir: &Path, base: &str) -> bool {
    enabled_path(mods_dir, base).is_file() || disabled_path(mods_dir, base).is_file()
}

/// Rename to/from `.jar.disabled`. Already in the wanted state is a no-op.
pub(super) fn set_enabled(mods_dir: &Path, file_name: &str, enabled: bool) -> Result<()> {
    let base = base_name(file_name);
    check_file_name(base)?;
    let (on, off) = (enabled_path(mods_dir, base), disabled_path(mods_dir, base));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if to.is_file() {
        return Ok(());
    }
    if !from.is_file() {
        return Err(Error::Other(format!(
            "mod {base} is not in the mods folder"
        )));
    }
    fs::rename(&from, &to).at(&to)
}

/// Delete a mod in whichever form(s) it exists. Missing is fine.
pub(super) fn delete(mods_dir: &Path, file_name: &str) -> Result<()> {
    let base = base_name(file_name);
    check_file_name(base)?;
    remove_if_present(&enabled_path(mods_dir, base))?;
    remove_if_present(&disabled_path(mods_dir, base))
}

/// Delete only the `.disabled` form (after a fresh copy was installed).
pub(super) fn delete_disabled_copy(mods_dir: &Path, file_name: &str) -> Result<()> {
    let base = base_name(file_name);
    check_file_name(base)?;
    remove_if_present(&disabled_path(mods_dir, base))
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Cache location of an icon URL.
fn icon_cache_path(url: &str, cache_dir: &Path) -> PathBuf {
    cache_dir.join(hex::encode(Sha1::digest(url.as_bytes())))
}

/// Icon bytes, from `cache_dir` when fetched before.
pub(super) fn icon(url: &str, cache_dir: &Path) -> Result<Vec<u8>> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::Other(format!("invalid icon URL: {url}")));
    }
    let path = icon_cache_path(url, cache_dir);
    match fs::read(&path) {
        Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(path, e)),
    }
    let bytes = fetch_icon(url)?;
    store(&path, &bytes)?;
    Ok(bytes)
}

fn fetch_icon(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent()
        .get(url)
        .header("User-Agent", user_agent())
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    if let Some(e) = status_error(resp.status().as_u16(), "icon") {
        return Err(e);
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_ICON_BYTES)
        .read_to_vec()?;
    if bytes.is_empty() {
        return Err(Error::Other(format!("empty icon at {url}")));
    }
    Ok(bytes)
}

/// Write via a temp file so a concurrent reader never sees half an icon.
fn store(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let tmp = path.with_extension("part");
    fs::write(&tmp, bytes).at(&tmp)?;
    fs::rename(&tmp, path).at(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::InstalledMod;

    fn tracked(project: &str, title: &str, file: &str) -> InstalledMod {
        InstalledMod {
            project_id: project.into(),
            version_id: "v".into(),
            title: title.into(),
            version_number: "1".into(),
            file_name: file.into(),
            icon_url: None,
            dependency: false,
        }
    }

    #[test]
    fn file_names_are_validated() {
        for ok in ["sodium-0.6.jar", "Fabric API.JAR"] {
            assert!(check_file_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            ".jar",
            "../x.jar",
            "a/b.jar",
            "a\\b.jar",
            "C:x.jar",
            "x.zip",
            ".hidden.jar",
        ] {
            assert!(check_file_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_folder_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mods = list(&dir.path().join("nope"), &ModIndex::default()).unwrap();
        assert!(mods.is_empty());
    }

    #[test]
    fn list_joins_index_filters_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        fs::write(d.join("zeta.jar"), b"zz").unwrap();
        fs::write(d.join("b-lib.jar.disabled"), b"b").unwrap();
        fs::write(d.join("readme.txt"), b"x").unwrap();
        fs::write(d.join("x.jar.part"), b"x").unwrap();
        fs::create_dir(d.join("folder.jar")).unwrap();
        let index = ModIndex {
            mods: vec![tracked("p", "Alpha Mod", "zeta.jar")],
        };
        let mods = list(d, &index).unwrap();
        let names: Vec<_> = mods
            .iter()
            .map(|m| (m.file_name.as_str(), m.enabled))
            .collect();
        assert_eq!(names, [("zeta.jar", true), ("b-lib.jar", false)]);
        assert_eq!(mods[0].tracked.as_ref().unwrap().title, "Alpha Mod");
        assert_eq!(mods[0].size, 2);
        assert!(mods[1].tracked.is_none());
    }

    #[test]
    fn enable_disable_is_idempotent_and_delete_handles_both_forms() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        fs::write(d.join("m.jar"), b"m").unwrap();
        set_enabled(d, "m.jar", false).unwrap();
        set_enabled(d, "m.jar", false).unwrap();
        assert!(d.join("m.jar.disabled").is_file() && !d.join("m.jar").exists());
        set_enabled(d, "m.jar.disabled", true).unwrap();
        assert!(d.join("m.jar").is_file());
        assert!(set_enabled(d, "gone.jar", true).is_err());
        assert!(set_enabled(d, "../m.jar", true).is_err());
        fs::write(d.join("m.jar.disabled"), b"old").unwrap();
        delete_disabled_copy(d, "m.jar").unwrap();
        assert!(d.join("m.jar").is_file() && !d.join("m.jar.disabled").exists());
        set_enabled(d, "m.jar", false).unwrap();
        delete(d, "m.jar").unwrap();
        assert!(!exists(d, "m.jar"));
        delete(d, "m.jar").unwrap();
    }

    #[test]
    fn icon_served_from_cache_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://cdn.example.invalid/icon.png";
        store(&icon_cache_path(url, dir.path()), b"png").unwrap();
        assert_eq!(icon(url, dir.path()).unwrap(), b"png");
        assert!(icon("file:///etc/passwd", dir.path()).is_err());
    }
}
