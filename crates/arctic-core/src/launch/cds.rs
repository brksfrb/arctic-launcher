//! EXPERIMENT (off by default): a per-game-folder AppCDS archive of the previous launch.
//!
//! Enabled only with `ARCTIC_EXPERIMENT_APPCDS=1`. The JVM writes a dynamic CDS archive of the
//! classes it loaded (from the boot, platform and app loaders) when the game exits; the next
//! start maps it instead of parsing and verifying those classes again.
//!
//! The JVM validates an archive against its own build and the classpath, and falls back to a
//! normal start when it doesn't fit (`-Xshare:auto`). The launcher adds a stricter check of its
//! own ([`key`]): any change to the Java build, the game version, the loader, the JVM flags that
//! shape the heap, or a classpath/mods jar (name, size, modification time) discards the archive.
//! See `docs/experiment-launch-appcds.md`.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Environment variable that turns the experiment on (value `1`).
pub const ENV: &str = "ARCTIC_EXPERIMENT_APPCDS";
const DIR: &str = ".arctic-startup";
const ARCHIVE: &str = "appcds.jsa";
const KEY: &str = "appcds.key";
/// `-XX:+AutoCreateSharedArchive` needs JDK 19; `-XX:ArchiveClassesAtExit` needs JDK 13.
const AUTO_CREATE_MIN: u32 = 19;
const DYNAMIC_MIN: u32 = 13;

pub fn enabled() -> bool {
    std::env::var(ENV).is_ok_and(|v| v == "1")
}

/// What the game start is, as far as the archive is concerned.
pub struct Launch<'a> {
    pub game_dir: &'a Path,
    pub game_version: &'a str,
    pub loader: &'a str,
    pub java: &'a Path,
    pub java_major: u32,
    /// Every argument after the java executable.
    pub args: &'a [String],
}

fn fingerprint(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(m) => {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{}:{}:{t}", path.display(), m.len())
        }
        // A missing file is part of the key too: it appearing changes the key.
        Err(_) => format!("{}:missing", path.display()),
    }
}

/// The classpath: the value after `-cp` / `-classpath` / `--class-path`.
fn classpath(args: &[String]) -> Option<&str> {
    args.windows(2)
        .find(|w| matches!(w[0].as_str(), "-cp" | "-classpath" | "--class-path"))
        .map(|w| w[1].as_str())
}

/// Flags that change what the JVM loads or how the archive maps; a change in any discards it.
fn shaping(arg: &str) -> bool {
    arg.starts_with("-javaagent:")
        || arg.starts_with("-agentpath:")
        || arg.starts_with("--add-")
        || arg.starts_with("--module-path")
        || arg.starts_with("-p")
        || arg.starts_with("--patch-module")
        || arg.starts_with("-XX:+Use")
        || arg.starts_with("-XX:-Use")
        || arg.starts_with("-XX:ObjectAlignment")
        || arg.starts_with("-XX:+UnlockExperimental")
}

/// Everything the archive is only valid for. `None` when it can't be computed safely.
fn key(l: &Launch) -> Option<String> {
    let cp = classpath(l.args)?;
    let mut parts = vec![
        format!("v1|{}|{}|java{}", l.game_version, l.loader, l.java_major),
        fingerprint(l.java),
    ];
    // The runtime's own `release` file names the exact build.
    if let Some(home) = l.java.parent().and_then(Path::parent)
        && let Ok(release) = fs::read_to_string(home.join("release"))
    {
        parts.push(release);
    }
    for entry in cp.split(if cfg!(windows) { ';' } else { ':' }) {
        parts.push(fingerprint(Path::new(entry)));
    }
    // Mods and the client agent are loaded by the loader, not the classpath: they still change what runs.
    if let Ok(entries) = fs::read_dir(l.game_dir.join("mods")) {
        let mut jars: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
            .map(|e| fingerprint(&e.path()))
            .collect();
        jars.sort();
        parts.extend(jars);
    }
    let mut flags: Vec<&str> = l
        .args
        .iter()
        .map(String::as_str)
        .filter(|a| shaping(a))
        .collect();
    flags.sort_unstable();
    parts.extend(flags.iter().map(|f| f.to_string()));
    let digest = Sha256::digest(parts.join("\n").as_bytes());
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn path_arg(p: PathBuf) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The user chose their own class sharing: leave it alone.
fn user_owns_cds(args: &[String]) -> bool {
    args.iter().any(|a| {
        a.starts_with("-Xshare")
            || a.contains("SharedArchiveFile")
            || a.contains("ArchiveClassesAtExit")
            || a.contains("AutoCreateSharedArchive")
            || a.contains("DumpLoadedClassList")
    })
}

/// JVM flags to put before the main class. Empty (a normal start) whenever anything is off.
pub fn flags(l: &Launch) -> Vec<String> {
    if !enabled() || l.java_major < DYNAMIC_MIN || user_owns_cds(l.args) {
        return Vec::new();
    }
    let Some(key) = key(l) else {
        return Vec::new();
    };
    let dir = l.game_dir.join(DIR);
    if fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    let (archive, key_file) = (dir.join(ARCHIVE), dir.join(KEY));
    let valid = archive.is_file() && fs::read_to_string(&key_file).is_ok_and(|k| k.trim() == key);
    if !valid {
        // Stale (or never recorded): drop it, so this start records a fresh one at exit.
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_file(&key_file);
        if fs::write(&key_file, &key).is_err() {
            return Vec::new();
        }
    }
    let path = path_arg(archive);
    let mut flags = vec!["-Xshare:auto".to_owned()];
    if l.java_major >= AUTO_CREATE_MIN {
        // The JVM itself re-creates the archive when it rejects it, and never fails the start.
        flags.push("-XX:+AutoCreateSharedArchive".into());
        flags.push(format!("-XX:SharedArchiveFile={path}"));
    } else if valid {
        flags.push(format!("-XX:SharedArchiveFile={path}"));
    } else {
        flags.push(format!("-XX:ArchiveClassesAtExit={path}"));
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The environment is process-wide.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct Fixture {
        dir: tempfile::TempDir,
        args: Vec<String>,
        java: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path();
        fs::create_dir_all(g.join("mods")).unwrap();
        fs::write(g.join("mods/a.jar"), b"1").unwrap();
        fs::write(g.join("client.jar"), b"jar").unwrap();
        fs::create_dir_all(g.join("rt/bin")).unwrap();
        fs::write(g.join("rt/bin/java"), b"j").unwrap();
        fs::write(g.join("rt/release"), "JAVA_VERSION=\"21.0.1\"").unwrap();
        let args = vec![
            "-Xmx2G".into(),
            "-cp".into(),
            g.join("client.jar").to_string_lossy().into_owned(),
            "net.minecraft.Main".into(),
        ];
        Fixture {
            java: g.join("rt/bin/java"),
            args,
            dir,
        }
    }

    fn run(f: &Fixture, version: &str, major: u32) -> Vec<String> {
        flags(&Launch {
            game_dir: f.dir.path(),
            game_version: version,
            loader: "vanilla",
            java: &f.java,
            java_major: major,
            args: &f.args,
        })
    }

    fn with_env<T>(on: bool, f: impl FnOnce() -> T) -> T {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised by ENV_LOCK; no other test reads this variable.
        unsafe {
            if on {
                std::env::set_var(ENV, "1")
            } else {
                std::env::remove_var(ENV)
            }
        }
        let out = f();
        unsafe { std::env::remove_var(ENV) };
        out
    }

    #[test]
    fn off_by_default() {
        let f = fixture();
        assert!(with_env(false, || run(&f, "1.21", 21)).is_empty());
    }

    #[test]
    fn old_java_and_user_cds_are_left_alone() {
        let mut f = fixture();
        assert!(with_env(true, || run(&f, "1.8.9", 8)).is_empty());
        f.args.insert(0, "-XX:SharedArchiveFile=/x.jsa".into());
        assert!(with_env(true, || run(&f, "1.21", 21)).is_empty());
    }

    #[test]
    fn java_17_records_then_uses_and_21_auto_creates() {
        let f = fixture();
        let first = with_env(true, || run(&f, "1.20.1", 17));
        assert!(
            first
                .iter()
                .any(|a| a.starts_with("-XX:ArchiveClassesAtExit="))
        );
        // Not recorded yet (no file): record again.
        let again = with_env(true, || run(&f, "1.20.1", 17));
        assert_eq!(first, again);
        // The JVM wrote it: now it is used.
        fs::write(f.dir.path().join(DIR).join(ARCHIVE), b"jsa").unwrap();
        let used = with_env(true, || run(&f, "1.20.1", 17));
        assert!(used.iter().any(|a| a.starts_with("-XX:SharedArchiveFile=")));
        assert!(!used.iter().any(|a| a.contains("ArchiveClassesAtExit")));
        let auto = with_env(true, || run(&f, "1.20.1", 21));
        assert!(auto.contains(&"-XX:+AutoCreateSharedArchive".to_owned()));
    }

    #[test]
    fn a_stale_archive_is_ignored_and_removed() {
        let f = fixture();
        let g = f.dir.path();
        with_env(true, || run(&f, "1.20.1", 17));
        let archive = g.join(DIR).join(ARCHIVE);
        fs::write(&archive, b"jsa").unwrap();
        let used = with_env(true, || run(&f, "1.20.1", 17));
        assert!(archive.exists() && used.iter().any(|a| a.contains("SharedArchiveFile")));

        // Each change below must discard the archive and record anew.
        let stale = |change: &dyn Fn(&Fixture)| {
            fs::write(&archive, b"jsa").unwrap();
            with_env(true, || run(&f, "1.20.1", 17));
            fs::write(&archive, b"jsa").unwrap();
            change(&f);
            let out = with_env(true, || run(&f, "1.20.1", 17));
            assert!(!archive.exists(), "stale archive kept");
            assert!(out.iter().any(|a| a.contains("ArchiveClassesAtExit")));
        };
        stale(&|f| fs::write(f.dir.path().join("mods/b.jar"), b"2").unwrap());
        stale(&|f| fs::write(f.dir.path().join("mods/a.jar"), b"changed!").unwrap());
        stale(&|f| fs::write(f.dir.path().join("client.jar"), b"jar2").unwrap());
        stale(&|f| fs::write(f.dir.path().join("rt/release"), "JAVA_VERSION=\"21.0.2\"").unwrap());
        stale(&|f| fs::write(&f.java, b"new java build").unwrap());

        // A different game version or Java major: another key.
        fs::write(&archive, b"jsa").unwrap();
        with_env(true, || run(&f, "1.20.1", 17));
        fs::write(&archive, b"jsa").unwrap();
        assert!(
            with_env(true, || run(&f, "1.20.2", 17))
                .iter()
                .any(|a| a.contains("ArchiveClassesAtExit"))
        );
        assert!(!archive.exists());
    }

    #[test]
    fn a_changed_agent_flag_discards_the_archive() {
        let mut f = fixture();
        with_env(true, || run(&f, "26.1", 21));
        let archive = f.dir.path().join(DIR).join(ARCHIVE);
        fs::write(&archive, b"jsa").unwrap();
        f.args.insert(0, "-javaagent:/x.jar".into());
        with_env(true, || run(&f, "26.1", 17));
        assert!(!archive.exists());
    }

    #[test]
    fn no_classpath_means_no_archive() {
        let mut f = fixture();
        f.args = vec!["-Xmx1G".into(), "Main".into()];
        assert!(with_env(true, || run(&f, "1.20.1", 21)).is_empty());
    }
}
