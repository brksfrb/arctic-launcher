//! Route native loader failures back to the managed runtime that produced them.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::{crash, java};

pub(super) const DETACHED: &str = "[Arctic diagnostics] detached runtime run: ";

pub(super) fn mark_after_crash(
    runtime: &Path,
    log: &Path,
    game_dir: &Path,
    since: SystemTime,
) -> Option<String> {
    let evidence = runtime_failure(runtime, log, game_dir, since)?;
    request(runtime, &uuid::Uuid::new_v4().to_string());
    Some(evidence)
}

/// A detached CLI process has no watcher. Check its last run before preparing this instance
/// again, using the run id to avoid repairing it again if preparation succeeds but spawn doesn't.
pub(super) fn recover_detached(runtime: &Path, log: &Path, game_dir: &Path) {
    let first = File::open(log)
        .ok()
        .and_then(|file| BufReader::new(file).lines().next()?.ok());
    let Some(first) = first else { return };
    let Some((id, at)) = first.strip_prefix(DETACHED).and_then(|s| s.split_once(' ')) else {
        return;
    };
    let (Ok(_), Ok(at)) = (uuid::Uuid::parse_str(id), at.parse::<u64>()) else {
        return;
    };
    let Some(since) = SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(at)) else {
        return;
    };
    if runtime_failure(runtime, log, game_dir, since).is_some() {
        request(runtime, id);
    }
}

fn request(runtime: &Path, id: &str) {
    if let Err(e) = java::request_repair(runtime, id) {
        log::warn!("queueing Java runtime verification: {e}");
    }
}

fn runtime_failure(
    runtime: &Path,
    log: &Path,
    game_dir: &Path,
    since: SystemTime,
) -> Option<String> {
    let original = normalized(&std::path::absolute(runtime).ok()?.to_string_lossy());
    #[cfg(windows)]
    let named = {
        let path = std::path::absolute(
            runtime
                .parent()?
                .join(".named")
                .join(runtime.file_name()?)
                .join(super::named_runtime::FOLDER),
        )
        .ok()?;
        normalized(&path.to_string_lossy())
    };
    let find = |path: &Path| {
        crate::java::file_metadata(path).ok()??;
        BufReader::new(File::open(path).ok()?)
            .lines()
            .map_while(Result::ok)
            .find(|line| {
                let Some(path) = crash::runtime_native_error(line) else {
                    return false;
                };
                let prefix = normalized(&line[..path.end()]);
                prefix.ends_with(&original) || {
                    #[cfg(windows)]
                    {
                        prefix.ends_with(&named)
                    }
                    #[cfg(not(windows))]
                    {
                        false
                    }
                }
            })
    };
    let fresh = |p: &Path| {
        fs::symlink_metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|at| at >= since)
    };
    find(log)
        .or_else(|| {
            let logs = game_dir.join("logs");
            java::require_directory(&logs).ok()?;
            let latest = logs.join("latest.log");
            fresh(&latest).then(|| find(&latest)).flatten()
        })
        .or_else(|| {
            let reports = game_dir.join("crash-reports");
            java::require_directory(&reports).ok()?;
            fs::read_dir(reports)
                .ok()?
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
                .map(|e| e.path())
                .filter(|p| fresh(p))
                .find_map(|p| find(&p))
        })
}

fn normalized(path: &str) -> String {
    let path = path.replace('\\', "/");
    if cfg!(windows) {
        path.to_ascii_lowercase()
            .replace("//?/unc/", "//")
            .replace("//?/", "")
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_report_schedules_repair_for_the_runtime_that_failed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let runtime = root.join("runtimes/java-runtime-epsilon");
        fs::create_dir_all(&runtime).unwrap();
        let reports = root.join("crash-reports");
        fs::create_dir(&reports).unwrap();
        let log = root.join("game.log");
        let started = SystemTime::now() - Duration::from_secs(1);
        let report = reports.join("crash-test.txt");
        fs::write(
            &report,
            "java.lang.UnsatisfiedLinkError: /usr/lib/libfoo.so: undefined symbol",
        )
        .unwrap();
        assert!(mark_after_crash(&runtime, &log, root, started).is_none());

        let print = |dir: &Path| {
            format!(
                "java.lang.UnsatisfiedLinkError: {}: The specified procedure could not be found",
                dir.join("bin/management.dll").display()
            )
        };
        fs::write(
            &report,
            print(&root.join("someone-else/runtimes/java-runtime-epsilon")),
        )
        .unwrap();
        assert!(mark_after_crash(&runtime, &log, root, started).is_none());

        #[cfg(windows)]
        let reported_runtime = root.join("runtimes/.named/java-runtime-epsilon/Arctic Client");
        #[cfg(not(windows))]
        let reported_runtime = runtime.clone();
        fs::write(&report, print(&reported_runtime)).unwrap();
        assert!(
            mark_after_crash(&runtime, &log, root, started + Duration::from_secs(600)).is_none()
        );
        assert_eq!(
            mark_after_crash(&runtime, &log, root, started),
            Some(print(&reported_runtime))
        );
        assert!(
            fs::read_dir(&runtime)
                .unwrap()
                .any(|entry| { entry.unwrap().file_name() == ".arctic-runtime-repair" })
        );
    }
}
