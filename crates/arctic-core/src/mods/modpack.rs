//! Modrinth modpacks (`.mrpack`): a zip with `modrinth.index.json` listing
//! the files to download, plus `overrides/` (and `client-overrides/`) to copy
//! into the game folder. Installing one creates a new instance.

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::modrinth::{self, Version};
use crate::error::IoContext;
use crate::instances::{self, Instance, Loader};
use crate::loaders::LoaderKind;
use crate::net::{DownloadJob, download_all};
use crate::storage::DataDirs;
use crate::{Error, Progress, ProgressInfo, Result};

/// Hosts the Modrinth pack format allows downloads from.
const ALLOWED_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];
const INDEX: &str = "modrinth.index.json";
/// Largest pack archive we accept.
const MAX_PACK_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndex {
    pub name: String,
    #[serde(default)]
    pub version_id: String,
    pub files: Vec<PackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

impl PackIndex {
    /// Minecraft version and loader (kind + version) the pack needs.
    pub fn target(&self) -> Result<(String, Option<(LoaderKind, String)>)> {
        let game = self.dependencies.get("minecraft").cloned().ok_or_else(|| {
            Error::Other("the modpack doesn't say which Minecraft version it needs".into())
        })?;
        let loader = [
            ("fabric-loader", LoaderKind::Fabric),
            ("quilt-loader", LoaderKind::Quilt),
            ("neoforge", LoaderKind::NeoForge),
            ("forge", LoaderKind::Forge),
        ]
        .into_iter()
        .find_map(|(key, kind)| self.dependencies.get(key).map(|v| (kind, v.clone())));
        Ok((game, loader))
    }
}

/// Newest release of a modpack project on Modrinth: (url, file name, sha1, size).
pub fn latest_file(project_id: &str) -> Result<(String, String, Option<String>, Option<u64>)> {
    let versions: Vec<Version> = modrinth::get_api(
        &format!(
            "https://api.modrinth.com/v2/project/{}/version",
            modrinth::encode(project_id)
        ),
        &format!("modpack {project_id}"),
    )?;
    let best = versions
        .iter()
        .filter(|v| {
            v.primary_file()
                .is_some_and(|f| f.filename.ends_with(".mrpack"))
        })
        .min_by(|a, b| {
            rank(&a.version_type)
                .cmp(&rank(&b.version_type))
                .then_with(|| b.date_published.cmp(&a.date_published))
        })
        .ok_or_else(|| Error::Other("this modpack has no downloadable version".into()))?;
    let file = best
        .primary_file()
        .ok_or_else(|| Error::Other("this modpack has no file".into()))?;
    Ok((
        file.url.clone(),
        file.filename.clone(),
        file.hashes.sha1.clone(),
        file.size,
    ))
}

fn rank(version_type: &str) -> u8 {
    match version_type {
        "release" => 0,
        "beta" => 1,
        _ => 2,
    }
}

/// Download a Modrinth modpack project and install it as a new instance.
pub fn install_from_modrinth(
    dirs: &DataDirs,
    project_id: &str,
    progress: Progress,
) -> Result<Instance> {
    progress(ProgressInfo::stage("Finding the modpack"));
    let (url, file_name, sha1, size) = latest_file(project_id)?;
    check_url(&url)?;
    let pack = dirs.cache().join("modpacks").join(sanitize(&file_name));
    download_all(
        "Downloading modpack",
        vec![DownloadJob {
            url,
            dest: pack.clone(),
            sha1,
            size,
            lzma: None,
        }],
        progress,
    )?;
    install_file(dirs, &pack, progress)
}

/// Install a `.mrpack` file as a new instance.
pub fn install_file(dirs: &DataDirs, pack: &Path, progress: Progress) -> Result<Instance> {
    let size = fs::metadata(pack).at(pack)?.len();
    if size > MAX_PACK_BYTES {
        return Err(Error::Other("the modpack file is too large".into()));
    }
    let mut zip = zip::ZipArchive::new(fs::File::open(pack).at(pack)?)
        .map_err(|e| Error::Other(format!("not a modpack file: {e}")))?;
    let index: PackIndex = {
        let entry = zip
            .by_name(INDEX)
            .map_err(|_| Error::Other("not a Modrinth modpack (no modrinth.index.json)".into()))?;
        serde_json::from_reader(entry)?
    };
    let (game, loader) = index.target()?;
    let loader = match loader {
        Some((kind, version)) => Loader::new(Some(kind), version),
        None => Loader::Vanilla,
    };
    let instance = instances::create(dirs, &index.name, &game, loader)?;
    let game_dir = instance.game_dir(dirs);
    let result = populate(&index, &mut zip, &game_dir, progress);
    if let Err(e) = result {
        // Don't leave a half-installed instance behind.
        let _ = instances::remove(dirs, &instance.id);
        return Err(e);
    }
    Ok(instance)
}

fn populate(
    index: &PackIndex,
    zip: &mut zip::ZipArchive<fs::File>,
    game_dir: &Path,
    progress: Progress,
) -> Result<()> {
    let jobs = download_jobs(index, game_dir)?;
    download_all("Downloading mods", jobs, progress)?;
    progress(ProgressInfo::stage("Copying modpack files"));
    extract_overrides(zip, "overrides/", game_dir)?;
    extract_overrides(zip, "client-overrides/", game_dir)
}

/// Download jobs for every client file, with path and host checks.
pub fn download_jobs(index: &PackIndex, game_dir: &Path) -> Result<Vec<DownloadJob>> {
    let mut jobs = Vec::with_capacity(index.files.len());
    for file in &index.files {
        let client = file
            .env
            .as_ref()
            .and_then(|e| e.get("client"))
            .is_none_or(|c| c != "unsupported");
        if !client {
            continue;
        }
        let dest = game_dir.join(safe_relative(&file.path)?);
        let url = file
            .downloads
            .iter()
            .find(|u| check_url(u).is_ok())
            .ok_or_else(|| Error::Other(format!("{} has no allowed download link", file.path)))?;
        jobs.push(DownloadJob {
            url: url.clone(),
            dest,
            sha1: file.hashes.get("sha1").cloned(),
            size: file.file_size,
            lzma: None,
        });
    }
    Ok(jobs)
}

/// Only https downloads from the hosts the pack format allows.
fn check_url(url: &str) -> Result<()> {
    let host = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    if ALLOWED_HOSTS.contains(&host) {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "download from an unexpected host: {url}"
        )))
    }
}

/// A relative path that stays inside the game folder.
fn safe_relative(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    let ok = !path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(p.to_path_buf())
    } else {
        Err(Error::Other(format!("unsafe path in modpack: {path}")))
    }
}

fn extract_overrides(
    zip: &mut zip::ZipArchive<fs::File>,
    prefix: &str,
    game_dir: &Path,
) -> Result<()> {
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
        let Some(inner) = entry.enclosed_name() else {
            continue;
        };
        let Ok(relative) = inner.strip_prefix(prefix) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let out = game_dir.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).at(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let mut dst = fs::File::create(&out).at(&out)?;
        std::io::copy(&mut entry, &mut dst).at(&out)?;
    }
    Ok(())
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ".-_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn index(json: &str) -> PackIndex {
        serde_json::from_str(json).unwrap()
    }

    const PACK: &str = r#"{
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Test Pack",
        "files": [
            {"path": "mods/a.jar", "hashes": {"sha1": "aa", "sha512": "x"}, "downloads": ["https://cdn.modrinth.com/data/a.jar"], "fileSize": 10},
            {"path": "mods/server-only.jar", "hashes": {"sha1": "bb"}, "env": {"client": "unsupported", "server": "required"}, "downloads": ["https://cdn.modrinth.com/b.jar"], "fileSize": 5}
        ],
        "dependencies": {"minecraft": "1.21.4", "fabric-loader": "0.16.9"}
    }"#;

    #[test]
    fn reads_target_and_client_files() {
        let idx = index(PACK);
        let (game, loader) = idx.target().unwrap();
        assert_eq!(game, "1.21.4");
        assert_eq!(loader, Some((LoaderKind::Fabric, "0.16.9".into())));
        let jobs = download_jobs(&idx, Path::new("game")).unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].dest.ends_with("mods/a.jar"));
        assert_eq!(jobs[0].sha1.as_deref(), Some("aa"));
    }

    #[test]
    fn rejects_escaping_paths_and_foreign_hosts() {
        let evil = PACK.replace("mods/a.jar\"", "../../evil.jar\"");
        assert!(download_jobs(&index(&evil), Path::new("game")).is_err());
        let foreign = PACK.replace(
            "https://cdn.modrinth.com/data/a.jar",
            "https://evil.example/a.jar",
        );
        assert!(download_jobs(&index(&foreign), Path::new("game")).is_err());
        assert!(check_url("http://cdn.modrinth.com/x").is_err());
        assert!(check_url("https://cdn.modrinth.com.evil.example/x").is_err());
        assert!(safe_relative("C:/Windows/x").is_err());
    }

    #[test]
    fn overrides_are_extracted_safely() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("p.mrpack");
        let mut zip = zip::ZipWriter::new(fs::File::create(&pack).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file(INDEX, opts).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("overrides/config/a.toml", opts).unwrap();
        zip.write_all(b"x=1").unwrap();
        zip.start_file("client-overrides/options.txt", opts)
            .unwrap();
        zip.write_all(b"fov:1").unwrap();
        zip.start_file("overrides/../../escape.txt", opts).unwrap();
        zip.write_all(b"no").unwrap();
        zip.finish().unwrap();
        let game = dir.path().join("game");
        let mut archive = zip::ZipArchive::new(fs::File::open(&pack).unwrap()).unwrap();
        extract_overrides(&mut archive, "overrides/", &game).unwrap();
        extract_overrides(&mut archive, "client-overrides/", &game).unwrap();
        assert_eq!(fs::read(game.join("config/a.toml")).unwrap(), b"x=1");
        assert!(game.join("options.txt").is_file());
        assert!(!dir.path().join("escape.txt").exists());
    }
}
