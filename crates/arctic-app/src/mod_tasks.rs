//! Installed mod lists and compatibility checks run away from the UI thread.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use arctic_core::instances::Instance;
use arctic_core::loaders::LoaderKind;
use arctic_core::migrate::Clash;
use arctic_core::mods::{self, ModFile};
use arctic_core::storage::DataDirs;

use crate::tasks::{Event, Outcome, Tasks};

pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(2);
// Also distinguishes replies arriving after the UI was reset by a profile switch.
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScanKey {
    mods_dir: PathBuf,
    index: PathBuf,
    loader: Option<LoaderKind>,
    game: Option<String>,
}

impl ScanKey {
    pub fn new(instance: &Instance, dirs: &DataDirs) -> Self {
        Self {
            mods_dir: instance.game_dir(dirs).join("mods"),
            index: mods::index_path(&dirs.instance_dir(&instance.id)),
            loader: instance.loader.kind(),
            game: instance.version.clone(),
        }
    }
}

pub(crate) struct ModSnapshot {
    pub files: Vec<ModFile>,
    pub clashes: Vec<Clash>,
    stamp: u64,
}

/// One cache per instance. A refresh keeps the previous list visible, and edits
/// during a scan queue one follow-up instead of spawning competing scans.
#[derive(Default)]
pub(crate) struct ModScan {
    key: Option<ScanKey>,
    pub data: Option<Arc<ModSnapshot>>,
    pub error: Option<String>,
    request: Option<u64>,
    dirty: bool,
    checked_at: Option<Instant>,
}

impl ModScan {
    pub fn begin(&mut self, key: ScanKey, force: bool) -> Option<(u64, Option<u64>)> {
        if self.key.as_ref() != Some(&key) {
            *self = Self {
                key: Some(key),
                ..Self::default()
            };
        }
        if self.request.is_some() {
            self.dirty |= force;
            return None;
        }
        if !force && self.checked_at.is_some_and(|t| t.elapsed() < POLL_INTERVAL) {
            return None;
        }
        let request = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
        self.request = Some(request);
        self.error = None;
        let previous = (!force)
            .then(|| self.data.as_ref().map(|d| d.stamp))
            .flatten();
        Some((request, previous))
    }

    /// True means a local edit happened during this scan: discard its stale
    /// result and immediately scan again, even if the user changed tabs.
    pub fn complete(&mut self, request: u64, result: Outcome<Option<ModSnapshot>>) -> bool {
        if self.request != Some(request) {
            return false;
        }
        self.request = None;
        if std::mem::take(&mut self.dirty) {
            return true;
        }
        self.checked_at = Some(Instant::now());
        match result {
            Ok(Some(data)) => self.data = Some(Arc::new(data)),
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
        false
    }
}

impl Tasks {
    pub(crate) fn scan_mods(&self, id: String, request: u64, key: ScanKey, previous: Option<u64>) {
        self.run(move |t| t.send(Event::ModsScanned(id, request, scan(&key, previous))));
    }
}

fn scan(key: &ScanKey, previous: Option<u64>) -> Outcome<Option<ModSnapshot>> {
    // Save the fingerprint before reading: a concurrent folder change will
    // differ on the next poll instead of being mistaken for this result.
    let stamp = stamp(key)?;
    if previous == Some(stamp) {
        return Ok(None);
    }
    let files = mods::list(&key.mods_dir, &key.index)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|f| {
            f.file_name != arctic_core::arctic_mod::FILE_NAME
                && f.file_name != mods::polarium::FILE_NAME
        })
        .collect();
    let clashes = match (key.loader, key.game.as_deref()) {
        (Some(loader), Some(game)) => arctic_core::migrate::clashes(&key.mods_dir, loader, game),
        _ => Vec::new(),
    };
    Ok(Some(ModSnapshot {
        files,
        clashes,
        stamp,
    }))
}

fn stamp(key: &ScanKey) -> Outcome<u64> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    folder_stamp(&key.mods_dir)?.hash(&mut hasher);
    // A Modrinth lookup can change titles/icons without changing any JAR.
    file_stamp(&key.index)?.hash(&mut hasher);
    Ok(hasher.finish())
}

fn file_stamp(path: &Path) -> Outcome<Option<(u64, Option<std::time::SystemTime>)>> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(Some((meta.len(), meta.modified().ok()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Metadata only, but still disk I/O: call from a worker or explicit preflight,
/// never while drawing a page.
pub(crate) fn folder_stamp(dir: &Path) -> Outcome<u64> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => Some(entries),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut files = Vec::new();
    for entry in entries.into_iter().flatten() {
        let entry = entry.map_err(|e| e.to_string())?;
        files.push((entry.file_name(), file_stamp(&entry.path())?));
    }
    files.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    files.hash(&mut hasher);
    Ok(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(path: &Path) -> ScanKey {
        ScanKey {
            mods_dir: path.join("mods"),
            index: path.join("mods.json"),
            loader: Some(LoaderKind::Fabric),
            game: Some("1.21.11".into()),
        }
    }

    #[test]
    fn edits_during_scan_are_coalesced_and_stale_results_discarded() {
        let key = key(Path::new("test"));
        let mut state = ModScan::default();
        let (request, _) = state.begin(key.clone(), false).unwrap();
        assert!(state.begin(key.clone(), false).is_none());
        assert!(state.begin(key.clone(), true).is_none());
        assert!(state.begin(key.clone(), true).is_none());
        assert!(state.complete(
            request,
            Ok(Some(ModSnapshot {
                files: vec![],
                clashes: vec![],
                stamp: 1
            }))
        ));
        assert!(state.data.is_none());
        let (next, _) = state.begin(key.clone(), true).unwrap();
        assert_ne!(request, next);
        assert!(!state.complete(request, Err("late reply".into())));
        assert!(state.error.is_none());
        assert!(!state.complete(
            next,
            Ok(Some(ModSnapshot {
                files: vec![],
                clashes: vec![],
                stamp: 2
            }))
        ));
        assert_eq!(state.data.as_ref().unwrap().stamp, 2);
        assert!(state.begin(key, false).is_none());
    }

    #[test]
    fn folder_version_and_profile_switches_reject_old_results() {
        let mut key = key(Path::new("first-profile"));
        let mut state = ModScan::default();
        let (first, _) = state.begin(key.clone(), false).unwrap();
        key.game = Some("26.3".into());
        let (second, _) = state.begin(key.clone(), false).unwrap();
        assert!(!state.complete(first, Err("old game".into())));
        assert!(state.begin(key.clone(), false).is_none());
        assert!(!state.complete(second, Err("current game".into())));
        assert_eq!(state.error.as_deref(), Some("current game"));
        state = ModScan::default();
        let (third, _) = state.begin(key, false).unwrap();
        assert_ne!(third, second);
        assert!(!state.complete(second, Err("old profile".into())));
        assert!(state.error.is_none());
    }

    #[test]
    fn unchanged_results_and_errors_keep_the_cached_list() {
        let key = key(Path::new("test"));
        let mut state = ModScan::default();
        let (first, _) = state.begin(key.clone(), false).unwrap();
        state.complete(
            first,
            Ok(Some(ModSnapshot {
                files: vec![],
                clashes: vec![],
                stamp: 9,
            })),
        );
        let cached = state.data.clone().unwrap();
        state.checked_at = None;
        let (second, previous) = state.begin(key.clone(), false).unwrap();
        assert_eq!(previous, Some(9));
        state.complete(second, Ok(None));
        assert!(Arc::ptr_eq(&cached, state.data.as_ref().unwrap()));
        let (third, _) = state.begin(key, true).unwrap();
        state.complete(third, Err("unavailable".into()));
        assert!(Arc::ptr_eq(&cached, state.data.as_ref().unwrap()));
        assert_eq!(state.error.as_deref(), Some("unavailable"));
    }

    #[test]
    fn large_folder_updates_and_index_changes_are_detected() {
        let temp = tempfile::tempdir().unwrap();
        let key = key(temp.path());
        assert!(scan(&key, None).unwrap().unwrap().files.is_empty());
        std::fs::create_dir(&key.mods_dir).unwrap();
        for n in 0..500 {
            std::fs::write(key.mods_dir.join(format!("mod-{n}.jar")), b"test").unwrap();
        }
        let first = scan(&key, None).unwrap().unwrap();
        assert_eq!(first.files.len(), 500);
        assert!(scan(&key, Some(first.stamp)).unwrap().is_none());
        mods::set_enabled(&key.mods_dir, "mod-0.jar", false).unwrap();
        let second = scan(&key, Some(first.stamp)).unwrap().unwrap();
        assert!(
            !second
                .files
                .iter()
                .find(|m| m.file_name == "mod-0.jar")
                .unwrap()
                .enabled
        );
        std::fs::write(&key.index, br#"{"mods":[]}"#).unwrap();
        let third = scan(&key, Some(second.stamp)).unwrap().unwrap();
        std::fs::remove_file(key.mods_dir.join("mod-1.jar")).unwrap();
        assert_eq!(
            scan(&key, Some(third.stamp)).unwrap().unwrap().files.len(),
            499
        );
        std::fs::write(temp.path().join("not-a-directory"), b"file").unwrap();
        let bad = ScanKey {
            mods_dir: temp.path().join("not-a-directory"),
            ..key
        };
        assert!(scan(&bad, None).is_err());
    }
}
