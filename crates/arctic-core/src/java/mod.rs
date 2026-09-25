//! Managed Java runtimes, downloaded from Mojang's own runtime index so the
//! user never needs a system JDK. Each version JSON names the component it
//! needs (`javaVersion.component`); very old versions default to `jre-legacy`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::net::{Compressed, DownloadJob, download_all, get_json};
use crate::storage::{DataDirs, load_json, save_json};
use crate::versions::VersionJson;
use crate::{Error, Progress, Result};

pub const RUNTIME_INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
/// Component used by versions that predate `javaVersion` (Java 8).
pub const LEGACY_COMPONENT: &str = "jre-legacy";
const INDEX_CACHE: &str = "java_runtimes.json";
/// Written after a successful install; holds the manifest sha1.
const MARKER: &str = ".arctic-runtime";
/// Smaller files are fetched raw; decompression setup is not worth it.
const LZMA_MIN_SIZE: u64 = 64 * 1024;
const INDEX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// platform → component → available builds.
type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeBuild>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeBuild {
    manifest: ManifestRef,
    version: RuntimeVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestRef {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RuntimeFile {
    File {
        #[serde(default)]
        executable: bool,
        downloads: FileDownloads,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Debug, Deserialize)]
struct FileDownloads {
    raw: RawDownload,
    lzma: Option<RawDownload>,
}

#[derive(Debug, Deserialize)]
struct RawDownload {
    sha1: String,
    size: u64,
    url: String,
}

/// Mojang platform key for the host.
pub fn platform_key() -> &'static str {
    match (
        crate::versions::rules::current_os(),
        crate::versions::rules::current_arch(),
    ) {
        ("windows", "x86") => "windows-x86",
        ("windows", "arm64") => "windows-arm64",
        ("windows", _) => "windows-x64",
        ("osx", "arm64") => "mac-os-arm64",
        ("osx", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

/// Runtime component a version needs.
pub fn component_for(version: &VersionJson) -> &str {
    version
        .java_version
        .as_ref()
        .map(|j| j.component.as_str())
        .unwrap_or(LEGACY_COMPONENT)
}

/// Path of the java launcher inside an installed runtime. `javaw` on Windows
/// so no console window appears next to the game.
pub fn java_executable(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime_dir.join("bin").join("javaw.exe")
    } else {
        runtime_dir.join("bin").join("java")
    }
}

/// A runtime ready to install: the files still missing plus what to do
/// once they are downloaded. Lets the launch pipeline fetch runtime files in
/// the same parallel queue as libraries and assets.
pub struct RuntimePlan {
    pub java: PathBuf,
    pub jobs: Vec<DownloadJob>,
    dir: PathBuf,
    /// Manifest sha1 to record once installed (`None` = already current).
    marker: Option<String>,
    executables: Vec<PathBuf>,
}

impl RuntimePlan {
    /// Finalize after `jobs` were downloaded; returns the java executable.
    pub fn finish(self) -> Result<PathBuf> {
        set_executable(&self.executables)?;
        if let Some(sha) = &self.marker {
            let marker = self.dir.join(MARKER);
            fs::write(&marker, sha).at(&marker)?;
        }
        if self.java.is_file() {
            Ok(self.java)
        } else {
            Err(Error::Other(format!(
                "runtime installed but {} is missing",
                self.java.display()
            )))
        }
    }
}

/// Work out what installing/updating `component` needs.
///
/// Fast path: if the installed runtime matches the (cached) index entry, or
/// the index is unreachable, no jobs are returned.
pub fn plan_runtime(dirs: &DataDirs, component: &str) -> Result<RuntimePlan> {
    let dir = dirs.runtimes().join(component);
    let java = java_executable(&dir);
    let installed_sha = fs::read_to_string(dir.join(MARKER)).ok();
    let up_to_date = |dir: PathBuf, java: PathBuf| RuntimePlan {
        java,
        jobs: Vec::new(),
        dir,
        marker: None,
        executables: Vec::new(),
    };

    let build = match fetch_index(dirs) {
        Ok(index) => find_build(&index, component)?,
        Err(e) if installed_sha.is_some() && java.is_file() => {
            log::warn!("runtime index unavailable, using installed {component}: {e}");
            return Ok(up_to_date(dir, java));
        }
        Err(e) => return Err(e),
    };
    if installed_sha.as_deref() == Some(build.manifest.sha1.as_str()) && java.is_file() {
        return Ok(up_to_date(dir, java));
    }

    log::info!("installing Java runtime {component} {}", build.version.name);
    let manifest: RuntimeManifest = get_json(&build.manifest.url)?;
    let (jobs, executables) = collect_files(&dir, manifest)?;
    Ok(RuntimePlan {
        java,
        jobs,
        dir,
        marker: Some(build.manifest.sha1),
        executables,
    })
}

/// Make sure `component` is installed and return its java executable.
pub fn ensure_runtime(dirs: &DataDirs, component: &str, progress: Progress) -> Result<PathBuf> {
    let plan = plan_runtime(dirs, component)?;
    download_all("Java runtime", plan.jobs.clone(), progress)?;
    plan.finish()
}

/// Mojang's runtime index changes rarely; re-check it at most daily so
/// launches don't wait on a network round trip.
fn fetch_index(dirs: &DataDirs) -> Result<RuntimeIndex> {
    let cache = dirs.meta().join(INDEX_CACHE);
    if is_fresh(&cache, INDEX_MAX_AGE)
        && let Some(index) = load_json(&cache)?
    {
        return Ok(index);
    }
    match get_json::<RuntimeIndex>(RUNTIME_INDEX_URL) {
        Ok(index) => {
            let _ = save_json(&cache, &index);
            Ok(index)
        }
        Err(e) => load_json(&cache)?.ok_or(e),
    }
}

fn is_fresh(path: &Path, max_age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < max_age)
}

fn find_build(index: &RuntimeIndex, component: &str) -> Result<RuntimeBuild> {
    index
        .get(platform_key())
        .and_then(|components| components.get(component))
        .and_then(|builds| builds.first())
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "Mojang provides no '{component}' Java runtime for {}",
                platform_key()
            ))
        })
}

fn collect_files(
    dir: &Path,
    manifest: RuntimeManifest,
) -> Result<(Vec<DownloadJob>, Vec<PathBuf>)> {
    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    for (rel, file) in manifest.files {
        let dest = dir.join(&rel);
        match file {
            RuntimeFile::Directory => fs::create_dir_all(&dest).at(&dest)?,
            RuntimeFile::File {
                executable,
                downloads,
            } => {
                if executable {
                    executables.push(dest.clone());
                }
                // Prefer the LZMA stream for anything worth compressing;
                // runtimes are ~3-4x smaller that way.
                let lzma = downloads
                    .lzma
                    .filter(|_| downloads.raw.size >= LZMA_MIN_SIZE)
                    .map(|l| Compressed {
                        url: l.url,
                        sha1: l.sha1,
                        size: l.size,
                    });
                jobs.push(DownloadJob {
                    url: downloads.raw.url,
                    dest,
                    sha1: Some(downloads.raw.sha1),
                    size: Some(downloads.raw.size),
                    lzma,
                });
            }
            RuntimeFile::Link { target } => link(&dest, &target)?,
        }
    }
    Ok((jobs, executables))
}

/// Runtime manifests contain symlinks on Linux/macOS (e.g. `bin/` helpers).
#[cfg(unix)]
fn link(dest: &Path, target: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    if fs::symlink_metadata(dest).is_ok() {
        return Ok(());
    }
    std::os::unix::fs::symlink(target, dest).at(dest)
}

#[cfg(not(unix))]
fn link(dest: &Path, target: &str) -> Result<()> {
    log::debug!("skipping runtime link {} -> {target}", dest.display());
    Ok(())
}

#[cfg(unix)]
fn set_executable(paths: &[PathBuf]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for p in paths {
        fs::set_permissions(p, fs::Permissions::from_mode(0o755)).at(p)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_paths: &[PathBuf]) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_component_when_missing() {
        let v: VersionJson = serde_json::from_str(r#"{"id":"1.6.4","mainClass":"x"}"#).unwrap();
        assert_eq!(component_for(&v), LEGACY_COMPONENT);
    }

    #[test]
    fn parses_runtime_manifest() {
        let m: RuntimeManifest = serde_json::from_str(
            r#"{"files":{
                "bin":{"type":"directory"},
                "bin/javaw.exe":{"type":"file","executable":true,"downloads":{"raw":{"sha1":"a","size":1,"url":"u"}}},
                "legal/x":{"type":"link","target":"../y"}
            }}"#,
        )
        .unwrap();
        assert_eq!(m.files.len(), 3);
        assert!(matches!(
            m.files["bin/javaw.exe"],
            RuntimeFile::File {
                executable: true,
                ..
            }
        ));
    }

    #[test]
    fn freshness_check() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.json");
        assert!(!is_fresh(&p, Duration::from_secs(60)));
        fs::write(&p, "{}").unwrap();
        assert!(is_fresh(&p, Duration::from_secs(60)));
        assert!(!is_fresh(&p, Duration::ZERO));
    }

    #[test]
    fn missing_component_is_a_clear_error() {
        let index: RuntimeIndex = HashMap::new();
        let err = find_build(&index, "java-runtime-delta").unwrap_err();
        assert!(err.to_string().contains("java-runtime-delta"));
    }
}
