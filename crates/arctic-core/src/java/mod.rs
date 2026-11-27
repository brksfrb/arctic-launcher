//! Managed Java runtimes, downloaded from Mojang's own runtime index so the
//! user never needs a system JDK. Each version JSON names the component it
//! needs (`javaVersion.component`); very old versions default to `jre-legacy`.

use std::collections::HashMap;
use std::fs::{self, File, Metadata};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::net::{
    Compressed, DownloadJob, download_all_with_replacements, get_mojang_json, part_path, sha1_file,
};
use crate::storage::{DataDirs, load_json, save_json};
use crate::versions::VersionJson;
use crate::{Error, Progress, Result};

pub const RUNTIME_INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
/// Component used by versions that predate `javaVersion` (Java 8).
pub const LEGACY_COMPONENT: &str = "jre-legacy";
const INDEX_CACHE: &str = "java_runtimes.json";
/// Written after a successful install, using the manifest SHA-1 alone for older launchers.
const MARKER: &str = ".arctic-runtime";
const REPAIR: &str = ".arctic-runtime-repair";
const REPAIR_ACK: &str = ".arctic-runtime-repair-ack";
const MAX_HASH_WORKERS: usize = 4;
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

pub(crate) fn native_x86_64() -> bool {
    if !cfg!(target_arch = "x86_64") {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::{c_char, c_int, c_void};

        unsafe extern "C" {
            fn sysctlbyname(
                name: *const c_char,
                oldp: *mut c_void,
                oldlenp: *mut usize,
                newp: *mut c_void,
                newlen: usize,
            ) -> c_int;
        }

        let mut translated: c_int = 0;
        let mut len = std::mem::size_of_val(&translated);
        // SAFETY: The name is NUL-terminated and the output buffer is len bytes.
        let result = unsafe {
            sysctlbyname(
                c"sysctl.proc_translated".as_ptr(),
                (&mut translated as *mut c_int).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        // Intel Macs may lack this sysctl.
        if result == 0 {
            translated == 0
        } else {
            io::Error::last_os_error().kind() == io::ErrorKind::NotFound
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        use windows_sys::Win32::System::SystemInformation::IMAGE_FILE_MACHINE_AMD64;
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        use windows_sys::core::BOOL;

        // SAFETY: kernel32 is loaded and both names are NUL-terminated.
        let query = unsafe {
            GetProcAddress(
                GetModuleHandleA(c"kernel32.dll".as_ptr().cast()),
                c"IsWow64Process2".as_ptr().cast(),
            )
        };
        // Windows versions without this API predate x86-64 emulation on ARM64.
        let Some(query) = query else { return true };
        let mut process = 0;
        let mut native = 0;
        // SAFETY: This is the signature of IsWow64Process2 and both outputs are valid.
        let result = unsafe {
            let query = std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> BOOL,
            >(query);
            query(GetCurrentProcess(), &mut process, &mut native)
        };
        result != 0 && native == IMAGE_FILE_MACHINE_AMD64
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    true
}

/// Mojang platform key for the host.
pub fn platform_key() -> &'static str {
    platform_key_for(false)
}

/// `rosetta`: an Apple Silicon Mac running an Intel game gets Intel Java.
fn platform_key_for(rosetta: bool) -> &'static str {
    let arch = if rosetta {
        "x86_64"
    } else {
        crate::versions::rules::current_arch()
    };
    match (crate::versions::rules::current_os(), arch) {
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
/// so no console window appears next to the game; on macOS Mojang's
/// runtimes are app bundles (`jre.bundle/Contents/Home`).
pub fn java_executable(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        runtime_dir.join("bin").join("javaw.exe")
    } else if cfg!(target_os = "macos") {
        runtime_dir
            .join("jre.bundle")
            .join("Contents")
            .join("Home")
            .join("bin")
            .join("java")
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
    /// Install marker to publish once every file is in place.
    marker: Option<String>,
    repair_request: String,
    executables: Vec<PathBuf>,
    // Held across planning, downloading and publishing, including across launcher processes.
    _lock: File,
}

impl RuntimePlan {
    /// Finalize after `jobs` were downloaded; returns the java executable.
    pub fn finish(self) -> Result<PathBuf> {
        set_executable(&self.executables)?;
        if !java_exists(&self.dir, &self.java)? {
            return Err(Error::Other(format!(
                "runtime installed but {} is missing",
                self.java.display()
            )));
        }
        if let Some(marker) = &self.marker {
            if !self.repair_request.is_empty() {
                write_marker(&self.dir.join(REPAIR_ACK), &self.repair_request)?;
            }
            write_marker(&self.dir.join(MARKER), marker)?;
        }
        Ok(self.java)
    }
}

/// Work out what installing/updating `component` needs.
pub fn plan_runtime(dirs: &DataDirs, component: &str) -> Result<RuntimePlan> {
    plan_runtime_for(dirs, component, false)
}

/// [`plan_runtime`]; `rosetta`: the Intel build, for an Intel game on an
/// Apple Silicon Mac (kept apart from the arm64 one).
pub fn plan_runtime_for(dirs: &DataDirs, component: &str, rosetta: bool) -> Result<RuntimePlan> {
    let dir = runtime_dir(dirs, component, rosetta)?;
    fs::create_dir_all(dirs.root()).at(dirs.root())?;
    let lock = lock_runtime(&dir)?;
    ensure_directory(&dir).at(&dir)?;
    let mut plan = RuntimePlan {
        java: java_executable(&dir),
        jobs: Vec::new(),
        dir,
        marker: None,
        repair_request: String::new(),
        executables: Vec::new(),
        _lock: lock,
    };
    let marker = plan.dir.join(MARKER);
    let installed = read_marker(&marker)?.unwrap_or_default();
    let (installed_sha, embedded_ack) = parse_marker(&installed);
    let request = read_marker(&plan.dir.join(REPAIR))?.unwrap_or_default();
    let java_present = java_exists(&plan.dir, &plan.java)?;
    let reason = if repair_pending(&plan.dir, &request, embedded_ack)? {
        Some("repair requested")
    } else if installed_sha.is_none() {
        Some("missing or invalid install stamp")
    } else if !java_present {
        Some("missing Java executable")
    } else {
        None
    };

    let build = match fetch_index(dirs) {
        Ok(index) => find_build(&index, component, rosetta)?,
        Err(e) if reason.is_none() => {
            log::warn!("runtime index unavailable, using installed {component}: {e}");
            return Ok(plan);
        }
        Err(e) => return Err(e),
    };
    if reason.is_none()
        && installed_sha.is_some_and(|sha| sha.eq_ignore_ascii_case(&build.manifest.sha1))
    {
        return Ok(plan);
    }

    let reason = reason.unwrap_or("manifest SHA-1 changed");
    log::info!(
        "verifying Java runtime {component} {}: {reason}",
        build.version.name
    );
    let manifest: RuntimeManifest = get_mojang_json(&build.manifest.url)?;
    // An interrupted install must no longer qualify for warm or offline reuse.
    write_marker(&marker, "")?;
    (plan.jobs, plan.executables) = collect_files(&plan.dir, manifest)?;
    plan.marker = Some(build.manifest.sha1);
    plan.repair_request = request;
    Ok(plan)
}

/// Make sure `component` is installed and return its java executable.
pub fn ensure_runtime(dirs: &DataDirs, component: &str, progress: Progress) -> Result<PathBuf> {
    let mut plan = plan_runtime(dirs, component)?;
    let replacements = std::mem::take(&mut plan.jobs);
    download_all_with_replacements("Java runtime", Vec::new(), replacements, progress)?;
    plan.finish()
}

pub(crate) fn runtime_dir(dirs: &DataDirs, component: &str, rosetta: bool) -> Result<PathBuf> {
    if !single_component(component) || component.starts_with('.') || component.contains(':') {
        return Err(Error::Other(format!(
            "Invalid Java runtime component: {component}"
        )));
    }
    Ok(if rosetta {
        dirs.runtimes().join(format!("{component}-x64"))
    } else {
        dirs.runtimes().join(component)
    })
}

pub(crate) fn managed_runtime(dirs: &DataDirs, java: &Path) -> Option<PathBuf> {
    let runtimes = dirs.runtimes();
    let name = java.strip_prefix(&runtimes).ok()?.components().next()?;
    let Component::Normal(name) = name else {
        return None;
    };
    let runtime = runtimes.join(name);
    (name != ".named"
        && java == java_executable(&runtime)
        && file_metadata(&runtime.join(MARKER)).is_ok_and(|marker| marker.is_some()))
    .then_some(runtime)
}

/// Hold this through process creation so a second launch can't install Java while we mirror it.
pub(crate) fn lock_runtime(dir: &Path) -> Result<File> {
    let root = dir.parent().expect("runtime has a parent");
    ensure_directory(root).at(root)?;
    let name = dir
        .file_name()
        .expect("runtime has a name")
        .to_string_lossy();
    let path = root.join(format!(".{name}.lock"));
    file_metadata(&path).at(&path)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .at(&path)?;
    lock.lock().at(&path)?;
    Ok(lock)
}

/// The runtime may have changed after preparation, while the launcher finished other setup.
pub(crate) fn check_runtime_ready(dir: &Path) -> Result<()> {
    require_directory(dir).at(dir)?;
    let installed = read_marker(&dir.join(MARKER))?.unwrap_or_default();
    let (sha, embedded_ack) = parse_marker(&installed);
    let request = read_marker(&dir.join(REPAIR))?.unwrap_or_default();
    if sha.is_none()
        || repair_pending(dir, &request, embedded_ack)?
        || !java_exists(dir, &java_executable(dir))?
    {
        return Err(Error::Other(
            "Java runtime verification is pending. Launch again to repair it.".into(),
        ));
    }
    Ok(())
}

pub(crate) fn request_repair(dir: &Path, id: &str) -> Result<()> {
    let _lock = lock_runtime(dir)?;
    require_directory(dir).at(dir)?;
    let installed = read_marker(&dir.join(MARKER))?.unwrap_or_default();
    let current = read_marker(&dir.join(REPAIR))?;
    let (_, embedded_ack) = parse_marker(&installed);
    if current.as_deref() == Some(id) || !repair_pending(dir, id, embedded_ack)? {
        return Ok(());
    }
    write_marker(&dir.join(REPAIR), id)?;
    log::warn!("Java runtime verification requested for {}", dir.display());
    Ok(())
}

fn parse_marker(installed: &str) -> (Option<&str>, &str) {
    let (sha, embedded_ack) = installed.split_once('\n').unwrap_or((installed, ""));
    let sha = sha.trim();
    let valid_sha = sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit());
    (valid_sha.then_some(sha), embedded_ack.trim())
}

fn repair_pending(dir: &Path, request: &str, embedded_ack: &str) -> Result<bool> {
    Ok(!request.is_empty()
        && request != embedded_ack
        && read_marker(&dir.join(REPAIR_ACK))?.as_deref() != Some(request))
}

#[cfg(any(windows, test))]
pub(crate) fn is_runtime_control_file(name: &std::ffi::OsStr) -> bool {
    [MARKER, REPAIR, REPAIR_ACK]
        .iter()
        .any(|stamp| name.eq_ignore_ascii_case(stamp))
}

fn java_exists(dir: &Path, java: &Path) -> Result<bool> {
    let mut parent = dir.to_path_buf();
    let rel = java.strip_prefix(dir).unwrap();
    for part in rel.parent().unwrap().components() {
        parent.push(part);
        match require_directory(&parent) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(Error::io(parent, e)),
        }
    }
    Ok(file_metadata(java).at(java)?.is_some())
}

fn read_marker(path: &Path) -> Result<Option<String>> {
    file_metadata(path)
        .at(path)?
        .map(|_| fs::read_to_string(path).at(path))
        .transpose()
}

fn write_marker(path: &Path, contents: &str) -> Result<()> {
    // Atomic replacement keeps readers from seeing a partially written stamp.
    file_metadata(path).at(path)?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap()).at(path)?;
    temp.write_all(contents.as_bytes()).at(path)?;
    temp.as_file().sync_all().at(path)?;
    temp.persist(path).map_err(|e| e.error).at(path)?;
    Ok(())
}

fn single_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(part)) if part == name)
        && components.next().is_none()
}

pub(crate) fn is_link(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Includes directory junctions and other reparse points.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    meta.is_symlink()
}

pub(crate) fn file_metadata(path: &Path) -> io::Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !is_link(&meta) => Ok(Some(meta)),
        Ok(_) => Err(io::Error::other(format!(
            "{} is not a regular file",
            path.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(crate) fn require_directory(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() && !is_link(&meta) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{} is not a regular directory",
            path.display()
        )))
    }
}

pub(crate) fn ensure_directory(path: &Path) -> io::Result<()> {
    match require_directory(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(path).or_else(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                require_directory(path)
            } else {
                Err(e)
            }
        }),
        result => result,
    }
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
    match get_mojang_json::<RuntimeIndex>(RUNTIME_INDEX_URL) {
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

fn find_build(index: &RuntimeIndex, component: &str, rosetta: bool) -> Result<RuntimeBuild> {
    let platform = platform_key_for(rosetta);
    index
        .get(platform)
        .and_then(|components| components.get(component))
        .and_then(|builds| builds.first())
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "Mojang provides no '{component}' Java runtime for {platform}"
            ))
        })
}

fn collect_files(
    dir: &Path,
    manifest: RuntimeManifest,
) -> Result<(Vec<DownloadJob>, Vec<PathBuf>)> {
    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    let mut candidates = Vec::new();
    for (rel, file) in manifest.files {
        let rel = Path::new(&rel);
        let mut dest = dir.to_path_buf();
        let mut parts = rel.components().peekable();
        if parts.peek().is_none() || !rel.components().all(|part| matches!(part, Component::Normal(name) if !cfg!(windows) || !name.to_string_lossy().contains(':'))) {
            return Err(Error::Other(format!("Invalid runtime manifest path: {}", rel.display())));
        }
        while let Some(part) = parts.next() {
            dest.push(part);
            if parts.peek().is_some() {
                ensure_directory(&dest).at(&dest)?;
            }
        }
        match file {
            RuntimeFile::Directory => ensure_directory(&dest).at(&dest)?,
            RuntimeFile::File {
                executable,
                downloads,
            } => {
                if executable {
                    executables.push(dest.clone());
                }
                candidates.push((dest, downloads));
            }
            RuntimeFile::Link { target } => link(&dest, &target)?,
        }
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(MAX_HASH_WORKERS)
        .min(candidates.len());
    let candidates = Mutex::new(candidates.into_iter());
    std::thread::scope(|scope| -> Result<()> {
        let workers: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| -> Result<Vec<DownloadJob>> {
                    let mut pending = Vec::new();
                    loop {
                        let candidate = candidates.lock().unwrap().next();
                        let Some((dest, downloads)) = candidate else {
                            return Ok(pending);
                        };
                        if let Some(job) = verify_file(dest, downloads)? {
                            pending.push(job);
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            jobs.extend(worker.join().unwrap()?);
        }
        Ok(())
    })?;
    Ok((jobs, executables))
}

fn verify_file(dest: PathBuf, downloads: FileDownloads) -> Result<Option<DownloadJob>> {
    if let Some(meta) = file_metadata(&dest).at(&dest)?
        && meta.len() == downloads.raw.size
        && sha1_file(&dest)?.eq_ignore_ascii_case(&downloads.raw.sha1)
    {
        return Ok(None);
    }
    let part = part_path(&dest);
    file_metadata(&part).at(&part)?;
    let lzma = downloads
        .lzma
        .filter(|_| downloads.raw.size >= LZMA_MIN_SIZE)
        .map(|l| Compressed {
            url: l.url,
            sha1: l.sha1,
            size: l.size,
        });
    Ok(Some(DownloadJob {
        url: downloads.raw.url,
        dest,
        sha1: Some(downloads.raw.sha1),
        size: Some(downloads.raw.size),
        lzma,
        md5: None,
    }))
}

/// Runtime manifests contain symlinks on Linux/macOS (e.g. `bin/` helpers).
#[cfg(unix)]
fn link(dest: &Path, target: &str) -> Result<()> {
    use std::os::unix::fs::symlink;

    match fs::symlink_metadata(dest) {
        Ok(meta) if meta.is_symlink() && fs::read_link(dest).at(dest)? == Path::new(target) => {
            return Ok(());
        }
        Ok(meta) if meta.is_symlink() || meta.is_file() => {}
        Ok(_) => {
            return Err(Error::Other(format!(
                "Runtime link is blocked: {}",
                dest.display()
            )));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(dest, e)),
    }
    let stage = tempfile::tempdir_in(dest.parent().unwrap()).at(dest)?;
    let link = stage.path().join("link");
    symlink(target, &link).at(dest)?;
    fs::rename(&link, dest).at(dest)
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

    const MANIFEST_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn warm_runtime_accepts_old_stamps_without_fetching_the_manifest() {
        use serde_json::json;

        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let component = "java-runtime-epsilon";
        let runtime = runtime_dir(&dirs, component, false).unwrap();
        let java = java_executable(&runtime);
        fs::create_dir_all(java.parent().unwrap()).unwrap();
        fs::write(&java, b"java").unwrap();
        save_json(
            &dirs.meta().join(INDEX_CACHE),
            &json!({
                (platform_key()): {(component): [{
                    "manifest": {"sha1": MANIFEST_SHA, "size": 0, "url": "invalid-manifest-url"},
                    "version": {"name": "25.0.1"}
                }]}
            }),
        )
        .unwrap();

        for suffix in ["", "\n", "\r\n", "\nprevious-crash"] {
            let stamp = format!("{MANIFEST_SHA}{suffix}");
            if suffix == "\nprevious-crash" {
                fs::write(runtime.join(REPAIR), "previous-crash").unwrap();
            }
            fs::write(runtime.join(MARKER), &stamp).unwrap();
            check_runtime_ready(&runtime).unwrap();
            let plan = plan_runtime(&dirs, component).unwrap();
            assert!(plan.jobs.is_empty());
            assert_eq!(plan.finish().unwrap(), java);
            assert_eq!(fs::read_to_string(runtime.join(MARKER)).unwrap(), stamp);
        }
    }

    #[test]
    fn repair_rechecks_the_installed_manifest_and_replaces_only_bad_files() {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;

        use serde_json::json;
        use sha1::{Digest, Sha1};

        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let component = "java-runtime-epsilon";
        let runtime = runtime_dir(&dirs, component, false).unwrap();
        let java = java_executable(&runtime);
        fs::create_dir_all(java.parent().unwrap()).unwrap();
        fs::write(&java, b"java").unwrap();
        let library = runtime.join("lib/management.dll");
        fs::create_dir(library.parent().unwrap()).unwrap();
        fs::write(&library, b"good").unwrap();
        fs::write(runtime.join(MARKER), MANIFEST_SHA).unwrap();

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        let file = |name: &str, bytes: &[u8]| {
            json!({
                "type": "file",
                "downloads": {"raw": {
                    "sha1": hex::encode(Sha1::digest(bytes)), "size": bytes.len(),
                    "url": format!("{url}/{name}"),
                }}
            })
        };
        let java_path = java
            .strip_prefix(&runtime)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let manifest = json!({
            "files": {
                (java_path): file("java", b"java"),
                "lib/management.dll": file("management", b"good")
            }
        })
        .to_string();
        let index = json!({
            (platform_key()): {(component): [{
                "manifest": {"sha1": MANIFEST_SHA, "size": manifest.len(), "url": format!("{url}/manifest")},
                "version": {"name": "25.0.1"}
            }]}
        });
        save_json(&dirs.meta().join(INDEX_CACHE), &index).unwrap();
        let server = std::thread::spawn(move || {
            for expected in ["/manifest", "/manifest", "/management"] {
                let (mut stream, _) = server.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = String::new();
                let mut reader = BufReader::new(&mut stream);
                reader.read_line(&mut request).unwrap();
                assert_eq!(request.split_whitespace().nth(1), Some(expected));
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                let body = if expected == "/manifest" {
                    &manifest
                } else {
                    "good"
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });

        fs::write(&library, b"bad!").unwrap();
        assert!(plan_runtime(&dirs, component).unwrap().jobs.is_empty());
        request_repair(&runtime, "crash-380").unwrap();
        assert!(check_runtime_ready(&runtime).is_err());

        // An interrupted repair must stay pending until all bytes have been published.
        let unfinished = plan_runtime(&dirs, component).unwrap();
        assert_eq!(unfinished.jobs.len(), 1);
        assert_eq!(unfinished.jobs[0].dest, library);
        drop(unfinished);
        assert!(check_runtime_ready(&runtime).is_err());
        assert_eq!(fs::read(&library).unwrap(), b"bad!");
        assert_eq!(ensure_runtime(&dirs, component, &|_| {}).unwrap(), java);
        server.join().unwrap();

        assert_eq!(fs::read(&library).unwrap(), b"good");
        assert_eq!(
            fs::read_to_string(runtime.join(MARKER)).unwrap(),
            MANIFEST_SHA
        );
        assert_eq!(
            fs::read_to_string(runtime.join(REPAIR_ACK)).unwrap(),
            "crash-380"
        );
        request_repair(&runtime, "crash-380").unwrap();
        assert!(check_runtime_ready(&runtime).is_ok());
        assert!(plan_runtime(&dirs, component).unwrap().jobs.is_empty());
    }

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
        let err = find_build(&index, "java-runtime-delta", false).unwrap_err();
        assert!(err.to_string().contains("java-runtime-delta"));
    }
}
