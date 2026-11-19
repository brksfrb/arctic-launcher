//! A one-launch retry and crash evidence for LWJGL's core native library.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::IoContext;
use crate::net::{DownloadJob, sha1_file};
use crate::versions::RuleEnv;
use crate::{Result, crash};

pub(super) const DEBUG_FLAGS: [&str; 2] = [
    "-Dorg.lwjgl.util.Debug=true",
    "-Dorg.lwjgl.util.DebugLoader=true",
];

#[derive(Debug, Clone)]
pub(super) struct CoreNative {
    jar: DownloadJob,
    entry: String,
}

impl CoreNative {
    pub(super) fn for_library(name: &str, jar: &DownloadJob, env: &RuleEnv) -> Option<Self> {
        let (version, classifier) = name.strip_prefix("org.lwjgl:lwjgl:")?.split_once(':')?;
        let (os, library) = match env.os {
            "windows" => ("windows", "lwjgl.dll"),
            "osx" => ("macos", "liblwjgl.dylib"),
            _ => ("linux", "liblwjgl.so"),
        };
        let (arch, suffix) = match env.arch {
            "x86_64" => ("x64", ""),
            "x86" => ("x86", "-x86"),
            arch => (arch, "-arm64"),
        };
        if classifier != format!("natives-{os}{suffix}") {
            return None;
        }
        let namespaced =
            semver::Version::parse(version).is_ok_and(|v| (v.major, v.minor) >= (3, 4));
        let entry = if namespaced {
            format!("{os}/{arch}/org/lwjgl/{library}")
        } else if env.os == "windows" && env.arch == "x86" {
            "lwjgl32.dll".into()
        } else {
            library.into()
        };
        Some(Self {
            jar: jar.clone(),
            entry,
        })
    }

    fn evidence(&self, shared: &Path) -> String {
        let path = &self.jar.dest;
        let size =
            fs::metadata(path).map_or_else(|e| e.to_string(), |m| format!("{} bytes", m.len()));
        let hash = sha1_file(path).unwrap_or_else(|_| "unavailable".into());
        let expected = self.jar.sha1.as_deref().unwrap_or("unavailable");
        let entry = match File::open(path)
            .ok()
            .and_then(|file| zip::ZipArchive::new(file).ok())
        {
            Some(mut jar) => {
                if jar
                    .by_name(&self.entry)
                    .is_ok_and(|entry| entry.is_file() && entry.size() > 0)
                {
                    "present"
                } else {
                    "missing"
                }
            }
            None => "jar unreadable",
        };
        format!(
            "jar={} ({size}), SHA-1={hash}, expected={expected}, entry {}={entry}",
            display_path(shared, path),
            self.entry
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct Launch {
    shared: PathBuf,
    extract: PathBuf,
    marker: PathBuf,
    core: Option<CoreNative>,
    request: Option<String>,
    main_class: String,
}

impl Launch {
    pub(super) fn new(
        shared: PathBuf,
        natives: &Path,
        core: Option<CoreNative>,
        main_class: String,
    ) -> Self {
        Self {
            shared,
            extract: natives.join("lwjgl"),
            marker: natives.with_file_name(".arctic-lwjgl-retry"),
            core,
            request: None,
            main_class,
        }
    }

    fn lock(&self) -> Result<File> {
        let path = self.marker.with_extension("lock");
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .at(&path)?;
        file.lock().at(&path)?;
        Ok(file)
    }

    fn state(&self) -> Option<String> {
        match fs::read_to_string(&self.marker) {
            Ok(state) => Some(state),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                log::warn!("reading LWJGL retry request: {e}");
                None
            }
        }
    }

    fn write_state(&self, state: &str) -> Result<()> {
        let parent = self
            .marker
            .parent()
            .expect("marker is in a version directory");
        let mut file = tempfile::NamedTempFile::new_in(parent).at(parent)?;
        file.write_all(state.as_bytes()).at(file.path())?;
        file.persist(&self.marker)
            .map_err(|e| crate::Error::io(&self.marker, e.error))?;
        Ok(())
    }

    /// Serialize repair preparations for the same version and capture the request they service.
    pub(super) fn prepare_retry(&mut self) -> Result<Option<File>> {
        let pending = |state: Option<String>| state.filter(|id| uuid::Uuid::parse_str(id).is_ok());
        if pending(self.state()).is_none() {
            return Ok(None);
        }
        let lock = self.lock()?;
        self.request = pending(self.state());
        Ok(self.request.as_ref().map(|_| lock))
    }

    /// Only one process gets debug flags for a given request, even if two copies prepared it.
    pub(super) fn start_retry(&self) -> Result<Option<File>> {
        let Some(request) = &self.request else {
            return Ok(None);
        };
        let lock = self.lock()?;
        Ok(self
            .state()
            .as_ref()
            .filter(|state| *state == request)
            .map(|_| lock))
    }

    pub(super) fn add_debug_flags(&self, args: &mut Vec<String>) {
        let main = args
            .iter()
            .position(|arg| *arg == self.main_class)
            .unwrap_or(0);
        // Put these after the player's JVM options so the retry always logs the loader.
        args.splice(main..main, DEBUG_FLAGS.map(str::to_owned));
    }

    pub(super) fn retry_started(&self, _lock: File) {
        if let Some(request) = &self.request
            && let Err(e) = self.write_state(&format!("started:{request}"))
        {
            log::warn!("recording LWJGL retry launch: {e}");
        }
    }

    pub(super) fn finish_retry(&self) {
        let Some(request) = &self.request else { return };
        let finished = (|| {
            let _lock = self.lock()?;
            if self.state().as_deref() == Some(&format!("started:{request}")) {
                fs::remove_file(&self.marker).at(&self.marker)?;
            }
            Ok::<_, crate::Error>(())
        })();
        if let Err(e) = finished {
            log::warn!("clearing LWJGL retry request: {e}");
        }
    }

    pub(super) fn request_retry(&self) {
        let result = self
            .lock()
            .and_then(|_lock| self.write_state(&uuid::Uuid::new_v4().to_string()));
        if let Err(e) = result {
            log::warn!("queueing LWJGL retry: {e}");
        }
    }

    pub(super) fn clean_extract(&self) {
        if let Err(e) = fs::remove_dir_all(&self.extract)
            && e.kind() != io::ErrorKind::NotFound
        {
            log::warn!(
                "couldn't clear LWJGL extract folder {}: {e}",
                self.extract.display()
            );
        }
    }

    pub(super) fn evidence(&self) -> String {
        let jar = self.core.as_ref().map_or_else(
            || "core natives jar missing from version metadata for this platform".into(),
            |core| core.evidence(&self.shared),
        );
        let probe =
            |dir| tempfile::tempfile_in(dir).map_or_else(|e| format!("no ({e})"), |_| "yes".into());
        let (writable, files) = match fs::read_dir(&self.extract) {
            Ok(entries) => {
                let mut files: Vec<_> = entries
                    .flatten()
                    .take(13)
                    .map(|entry| {
                        let size = fs::symlink_metadata(entry.path())
                            .map_or_else(|_| "?".into(), |m| m.len().to_string());
                        let name = entry.file_name();
                        let name: String = name.to_string_lossy().chars().take(100).collect();
                        format!("{name:?} ({size} B)")
                    })
                    .collect();
                files.sort();
                if files.len() > 12 {
                    files.truncate(12);
                    files.push("…".into());
                }
                (
                    probe(self.extract.as_path()),
                    format!("[{}]", files.join(", ")),
                )
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => (
                format!(
                    "missing (parent writable: {})",
                    probe(self.extract.parent().unwrap())
                ),
                "[]".into(),
            ),
            Err(e) => (probe(self.extract.as_path()), format!("unavailable ({e})")),
        };
        format!(
            "{}{jar} | extract={}, writable={writable}, files={files}",
            crash::LWJGL_EVIDENCE,
            display_path(&self.shared, &self.extract),
        )
    }
}

fn display_path(shared: &Path, path: &Path) -> String {
    path.strip_prefix(shared).map_or_else(
        |_| path.display().to_string(),
        |relative| format!("<shared>/{}", relative.display()),
    )
}

/// Keep a good jar in place. A forced download replaces a bad or unverifiable jar atomically.
pub(super) fn replacements(jars: Vec<DownloadJob>) -> Vec<DownloadJob> {
    let mut checked = 0;
    let jobs: Vec<_> = jars
        .into_iter()
        .filter(|job| {
            let Some(expected) = &job.sha1 else {
                return true;
            };
            if let Ok(actual) = sha1_file(&job.dest) {
                checked += 1;
                return !expected.eq_ignore_ascii_case(&actual);
            }
            true
        })
        .collect();
    log::info!(
        "LWJGL library check: {checked} checksummed, {} to fetch",
        jobs.len()
    );
    jobs
}
