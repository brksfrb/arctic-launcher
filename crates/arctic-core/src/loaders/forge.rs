//! Installing Forge / NeoForge from their installer jars.
//!
//! Modern installers (Forge 1.12.2 late builds and 1.13+, all NeoForge)
//! carry `install_profile.json` + `version.json` and a list of processors
//! that turn the vanilla client jar into the patched one. Older Forge
//! installers (`install` + `versionInfo`) are handled by `legacy_forge`.
//!
//! A successful install writes a marker to `meta/loaders/<installer>.json`
//! so later launches skip the installer (and its network access) entirely.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::forge_meta::InstallerSource;
use super::installer::{self, Installer};
use super::legacy_forge;
use super::processors::{self, Context, Processor, lib_path, resolve_data_value};
use super::util::{load_saved_profile, parse_profile, save_profile};
use crate::net::{DownloadJob, download_all};
use crate::storage::{DataDirs, load_json, save_json};
use crate::versions::{Library, VersionJson, maven_path, merge};
use crate::{Error, Progress, Result};

#[derive(Debug, Deserialize)]
struct InstallProfile {
    /// Path of the version JSON inside the installer.
    json: Option<String>,
    #[serde(default)]
    data: HashMap<String, SidedValue>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
}

#[derive(Debug, Deserialize)]
struct SidedValue {
    client: Option<String>,
}

/// Written after a successful install.
#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    profile_id: String,
    /// Files (relative to `shared/`) the profile needs that no download
    /// can restore: extracted and patched jars.
    files: Vec<String>,
}

fn marker_path(dirs: &DataDirs, source: &InstallerSource) -> PathBuf {
    let stem = source.file_name.trim_end_matches(".jar");
    dirs.meta().join("loaders").join(format!("{stem}.json"))
}

/// The installed profile, if a previous install is still intact.
fn installed(dirs: &DataDirs, source: &InstallerSource) -> Option<VersionJson> {
    let marker: Marker = load_json(&marker_path(dirs, source)).ok().flatten()?;
    let shared = dirs.shared();
    if !marker.files.iter().all(|f| shared.join(f).is_file()) {
        return None;
    }
    load_saved_profile(dirs, &marker.profile_id)
}

/// Install from the first installer in `sources` that can be downloaded.
pub fn install(
    dirs: &DataDirs,
    sources: &[InstallerSource],
    vanilla: &VersionJson,
    java: &Path,
    progress: Progress,
) -> Result<VersionJson> {
    if let Some(profile) = sources.iter().find_map(|s| installed(dirs, s)) {
        return Ok(merge(vanilla.clone(), profile));
    }
    let mut last_err = Error::Other("no installer to download".into());
    for source in sources {
        match installer::fetch(dirs, source, progress) {
            Ok(inst) => return install_from(dirs, source, inst, vanilla, java, progress),
            Err(e) => {
                log::info!("installer {} unavailable: {e}", source.url);
                last_err = e;
            }
        }
    }
    Err(Error::Other(format!(
        "the installer could not be downloaded (is this version available?): {last_err}"
    )))
}

fn install_from(
    dirs: &DataDirs,
    source: &InstallerSource,
    mut inst: Installer,
    vanilla: &VersionJson,
    java: &Path,
    progress: Progress,
) -> Result<VersionJson> {
    let root: Value = inst.read_json("install_profile.json")?;
    let (raw_profile, files) = if legacy_forge::is_legacy(&root) {
        legacy_forge::install(dirs, &mut inst, &root)?
    } else {
        let profile: InstallProfile = serde_json::from_value(root)
            .map_err(|e| Error::Other(format!("unsupported installer format: {e}")))?;
        install_modern(dirs, source, &mut inst, &profile, vanilla, java, progress)?
    };
    let parsed = parse_profile(&raw_profile)?;
    let profile_id = save_profile(dirs, &raw_profile)?;
    let marker = Marker { profile_id, files };
    save_json(&marker_path(dirs, source), &marker)?;
    Ok(merge(vanilla.clone(), parsed))
}

fn install_modern(
    dirs: &DataDirs,
    source: &InstallerSource,
    inst: &mut Installer,
    profile: &InstallProfile,
    vanilla: &VersionJson,
    java: &Path,
    progress: Progress,
) -> Result<(Value, Vec<String>)> {
    let json_name = profile.json.as_deref().unwrap_or("/version.json");
    let version_raw: Value = inst.read_json(json_name)?;
    let version: VersionJson = parse_profile(&version_raw)?;
    let libraries = dirs.libraries();

    // Files bundled in the installer's `maven/` folder (no download URL).
    let mut files = extract_bundled(inst, &libraries, &version.libraries)?;
    files.extend(extract_bundled(inst, &libraries, &profile.libraries)?);

    let client: Vec<&Processor> = profile
        .processors
        .iter()
        .filter(|p| p.runs_on_client())
        .collect();
    if !client.is_empty() {
        download_all(
            "Downloading installer libraries",
            library_jobs(dirs, &profile.libraries),
            progress,
        )?;
        let work = installer::cache_dir(dirs).join(source.file_name.trim_end_matches(".jar"));
        let ctx = context(dirs, inst, profile, vanilla, java, &work)?;
        processors::run_all(&ctx, &client, progress)?;
        files.extend(produced_files(dirs, profile)?);
        if let Err(e) = fs::remove_dir_all(&work) {
            log::debug!("could not clean {}: {e}", work.display());
        }
    }
    ensure_local_libraries(&libraries, &version.libraries)?;
    let shared = dirs.shared();
    let mut rel: Vec<String> = files
        .iter()
        .filter_map(|p| p.strip_prefix(&shared).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    rel.sort();
    rel.dedup();
    Ok((version_raw, rel))
}

/// Library path of an entry with an empty download URL.
fn local_path(libraries: &Path, lib: &Library) -> Option<PathBuf> {
    let artifact = lib.downloads.as_ref()?.artifact.as_ref()?;
    if !artifact.url.is_empty() {
        return None;
    }
    let rel = artifact.path.clone().or_else(|| maven_path(&lib.name))?;
    Some(libraries.join(rel))
}

fn extract_bundled(
    inst: &mut Installer,
    libraries: &Path,
    libs: &[Library],
) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for lib in libs {
        let (Some(dest), Some(rel)) = (local_path(libraries, lib), maven_path(&lib.name)) else {
            continue;
        };
        if inst.extract(&format!("maven/{rel}"), &dest)? {
            out.push(dest);
        }
    }
    Ok(out)
}

fn library_jobs(dirs: &DataDirs, libs: &[Library]) -> Vec<DownloadJob> {
    libs.iter()
        .filter_map(|lib| {
            let a = lib.downloads.as_ref()?.artifact.as_ref()?;
            if a.url.is_empty() {
                return None;
            }
            let rel = a.path.clone().or_else(|| maven_path(&lib.name))?;
            Some(DownloadJob {
                url: a.url.clone(),
                dest: dirs.libraries().join(rel),
                sha1: a.sha1.clone(),
                size: a.size,
                lzma: None,
            })
        })
        .collect()
}

/// Every URL-less library of the final profile must exist by now.
fn ensure_local_libraries(libraries: &Path, libs: &[Library]) -> Result<()> {
    match libs
        .iter()
        .filter_map(|l| local_path(libraries, l))
        .find(|p| !p.is_file())
    {
        Some(missing) => Err(Error::Other(format!(
            "the installer did not produce {}",
            missing.display()
        ))),
        None => Ok(()),
    }
}

fn context(
    dirs: &DataDirs,
    inst: &mut Installer,
    profile: &InstallProfile,
    vanilla: &VersionJson,
    java: &Path,
    work: &Path,
) -> Result<Context> {
    let libraries = dirs.libraries();
    let vanilla_dir = dirs.version_dir(&vanilla.id);
    let mut vars = HashMap::from([
        ("SIDE".to_owned(), "client".to_owned()),
        ("MINECRAFT_VERSION".to_owned(), vanilla.id.clone()),
        ("ROOT".to_owned(), dirs.shared().display().to_string()),
        ("INSTALLER".to_owned(), inst.path.display().to_string()),
        ("LIBRARY_DIR".to_owned(), libraries.display().to_string()),
        (
            "MINECRAFT_JAR".to_owned(),
            vanilla_dir
                .join(format!("{}.jar", vanilla.id))
                .display()
                .to_string(),
        ),
    ]);
    let mut extract = |name: &str| -> Result<PathBuf> {
        let dest = work.join(name.trim_start_matches('/'));
        if !inst.extract(name, &dest)? {
            return Err(Error::Other(format!("installer has no {name}")));
        }
        Ok(dest)
    };
    for (key, value) in &profile.data {
        if let Some(raw) = &value.client {
            vars.insert(
                key.clone(),
                resolve_data_value(raw, &libraries, &mut extract)?,
            );
        }
    }
    Ok(Context {
        vars,
        libraries,
        java: java.to_path_buf(),
        vanilla_json: vanilla_dir.join(format!("{}.json", vanilla.id)),
    })
}

/// Library files named by `data` that exist after the processors ran
/// (the patched client jar and friends).
fn produced_files(dirs: &DataDirs, profile: &InstallProfile) -> Result<Vec<PathBuf>> {
    let libraries = dirs.libraries();
    let mut out = Vec::new();
    for value in profile.data.values() {
        let Some(coords) = value
            .client
            .as_deref()
            .and_then(|v| v.strip_prefix('['))
            .and_then(|v| v.strip_suffix(']'))
        else {
            continue;
        };
        let path = lib_path(&libraries, coords)?;
        if path.is_file() {
            out.push(path);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = r#"{
        "spec": 1, "profile": "NeoForge", "version": "neoforge-21.1.251", "minecraft": "1.21.1",
        "json": "/version.json",
        "data": {
            "MC_SLIM": {"client": "[net.minecraft:client:1.21.1-20240808.144430:slim]", "server": "[x:y:1]"},
            "MC_SLIM_SHA": {"client": "'de86'", "server": "'00'"},
            "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"}
        },
        "processors": [
            {"sides": ["server"], "jar": "a:b:1", "args": []},
            {"jar": "net.neoforged.installertools:installertools:2.1.2", "classpath": ["x:y:1"],
             "args": ["--task", "DOWNLOAD_MOJMAPS", "--version", "1.21.1", "--side", "{SIDE}"]}
        ],
        "libraries": [
            {"name": "net.neoforged:neoforge:21.1.251:universal",
             "downloads": {"artifact": {"path": "net/neoforged/neoforge/21.1.251/neoforge-21.1.251-universal.jar",
                "url": "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.251/neoforge-21.1.251-universal.jar",
                "sha1": "abc", "size": 10}}},
            {"name": "net.minecraftforge:forge:1.12.2-14.23.5.2860",
             "downloads": {"artifact": {"path": "net/minecraftforge/forge/1.12.2-14.23.5.2860/forge-1.12.2-14.23.5.2860.jar",
                "url": "", "sha1": "def", "size": 20}}}
        ]
    }"#;

    #[test]
    fn parses_install_profile() {
        let p: InstallProfile = serde_json::from_str(PROFILE).unwrap();
        assert_eq!(p.json.as_deref(), Some("/version.json"));
        assert_eq!(p.data["MC_SLIM_SHA"].client.as_deref(), Some("'de86'"));
        let client: Vec<&Processor> = p.processors.iter().filter(|p| p.runs_on_client()).collect();
        assert_eq!(client.len(), 1);
        assert_eq!(client[0].classpath, ["x:y:1"]);
    }

    #[test]
    fn splits_downloadable_and_bundled_libraries() {
        let p: InstallProfile = serde_json::from_str(PROFILE).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let jobs = library_jobs(&dirs, &p.libraries);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].sha1.as_deref(), Some("abc"));
        let libs = dirs.libraries();
        let local = local_path(&libs, &p.libraries[1]).unwrap();
        assert!(local.ends_with("forge-1.12.2-14.23.5.2860.jar"));
        assert!(local_path(&libs, &p.libraries[0]).is_none());
        assert!(ensure_local_libraries(&libs, &p.libraries).is_err());
        fs::create_dir_all(local.parent().unwrap()).unwrap();
        fs::write(&local, b"jar").unwrap();
        assert!(ensure_local_libraries(&libs, &p.libraries).is_ok());
    }

    #[test]
    fn marker_short_circuits_reinstall() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let source = InstallerSource {
            url: "https://example.invalid/x-installer.jar".into(),
            file_name: "x-installer.jar".into(),
        };
        assert!(installed(&dirs, &source).is_none());
        let raw =
            serde_json::json!({"id": "neoforge-1", "mainClass": "M", "inheritsFrom": "1.21.1"});
        let id = save_profile(&dirs, &raw).unwrap();
        let patched = dirs.libraries().join("p.jar");
        fs::create_dir_all(dirs.libraries()).unwrap();
        fs::write(&patched, b"x").unwrap();
        let marker = Marker {
            profile_id: id,
            files: vec!["libraries/p.jar".into()],
        };
        save_json(&marker_path(&dirs, &source), &marker).unwrap();
        assert_eq!(installed(&dirs, &source).unwrap().id, "neoforge-1");
        fs::remove_file(&patched).unwrap();
        assert!(installed(&dirs, &source).is_none());
    }
}
