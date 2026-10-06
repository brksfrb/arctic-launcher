//! What a start can reuse from the one before, kept per game folder for the Arctic Client:
//! the list of classes a start loads (so it can load them ahead on other cores, its
//! `Preloader`) and the classes themselves as Fabric and Mixin leave them (its `MixinCache`).
//!
//! The first start after anything changed (the game version, a mod, the Java) records the
//! list: the JVM logs every class it loads and the client turns that into a list once the
//! title screen is up. Later starts hand the client the list. A list belongs to the mods it
//! was recorded with; any change throws it away and the next start records a new one.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const DIR: &str = ".arctic-startup";
const LIST: &str = "classes.txt";
const KEY: &str = "classes.key";
const LOG_PREFIX: &str = "class-load-";
const PACK_PREFIX: &str = "mixin-";

/// What the list depends on: the game version, the Java, and every jar in the mods folder.
fn key(game_dir: &Path, game_version: &str, java_major: u32) -> String {
    let mut parts = vec![format!("{game_version}|java{java_major}")];
    if let Ok(entries) = fs::read_dir(game_dir.join("mods")) {
        let mut jars: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".jar") {
                    return None;
                }
                let meta = e.metadata().ok()?;
                let time = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                Some(format!("{name}:{}:{time}", meta.len()))
            })
            .collect();
        jars.sort();
        parts.extend(jars);
    }
    let digest = Sha256::digest(parts.join("\n").as_bytes());
    digest.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// What the saved classes depend on, on top of the list's key: the settings the mods read as
/// they start (Mixin plugins choose which mixins apply from them). Their contents, not their
/// dates: a file a mod rewrites unchanged on every start must not throw the pack away.
fn pack_key(game_dir: &Path, base: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    if let Ok(entries) = fs::read_dir(game_dir.join("config")) {
        let mut files: Vec<_> = entries.flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files {
            let path = entry.path();
            // The Arctic Client's own files (session, waypoints, settings) change as it runs and
            // choose no mixins (it has no Mixin plugin).
            if entry.file_name().to_string_lossy().starts_with("arctic") {
                continue;
            }
            let small = entry.metadata().is_ok_and(|m| m.is_file() && m.len() <= CONFIG_MAX);
            if small && let Ok(bytes) = fs::read(&path) {
                hasher.update(entry.file_name().to_string_lossy().as_bytes());
                hasher.update(&bytes);
            }
        }
    }
    hasher.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// Settings files bigger than this are not looked at (they are data, not settings).
const CONFIG_MAX: u64 = 512 * 1024;

fn path_arg(path: PathBuf) -> String {
    // The JVM's option syntax uses ':' itself; forward slashes keep a Windows path from tripping on it.
    path.to_string_lossy().replace('\\', "/")
}

/// JVM flags for this start: use what was recorded when it fits, else record it again.
/// Only for Java 17 and up (older JVMs have another logging syntax).
///
/// `keep_classes`: also keep the classes as Fabric and Mixin leave them (the client's `MixinCache`).
/// Only for instances whose mods the launcher chose itself: that set is known and tested, a
/// player's own mods may do things at start that a saved class can't carry.
pub fn flags(game_dir: &Path, game_version: &str, java_major: u32, keep_classes: bool) -> Vec<String> {
    if java_major < 17 {
        return Vec::new();
    }
    // The client's jar is also a Java agent: it makes class loading take turns (so loading ahead
    // on another thread can't deadlock) and keeps the transformed classes. Without it neither is used.
    let agent = game_dir.join("mods").join(crate::arctic_mod::FILE_NAME);
    if !agent.is_file() {
        return Vec::new();
    }
    let dir = game_dir.join(DIR);
    if fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    let key = key(game_dir, game_version, java_major);
    let mut flags = vec![format!("-javaagent:{}", path_arg(agent))];
    flags.extend(class_list_flags(&dir, &key));
    if keep_classes {
        flags.push(format!("-Darctic.mixincache={}", path_arg(pack_path(game_dir, &dir, &key))));
    }
    flags
}

/// The game ended badly: whatever it read back from the saved classes is not trusted again.
pub(crate) fn drop_pack(game_dir: &Path) {
    if let Ok(entries) = fs::read_dir(game_dir.join(DIR)) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(PACK_PREFIX) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// The saved classes for exactly this game, mods and settings (older ones are removed).
fn pack_path(game_dir: &Path, dir: &Path, key: &str) -> PathBuf {
    let name = format!("{PACK_PREFIX}{}.pak", pack_key(game_dir, key));
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.starts_with(PACK_PREFIX) && !file.starts_with(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    dir.join(name)
}

fn class_list_flags(dir: &Path, key: &str) -> Vec<String> {
    let list = dir.join(LIST);
    if list.is_file() && fs::read_to_string(dir.join(KEY)).is_ok_and(|k| k.trim() == key) {
        return vec![format!("-Darctic.preload={}", path_arg(list))];
    }
    // Out of date: a new list is recorded by this start.
    let _ = fs::remove_file(&list);
    let _ = fs::remove_file(dir.join(KEY));
    // A file of its own each time: a log the last game still holds open must not stop this one (a JVM
    // that can't open its log doesn't start).
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(LOG_PREFIX) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let log = dir.join(format!("{LOG_PREFIX}{stamp}.log"));
    vec![
        format!("-Xlog:class+load=info:file={}", path_arg(log.clone())),
        format!("-Darctic.classlog={}", path_arg(log)),
        format!("-Darctic.classlist={}", path_arg(list)),
        format!("-Darctic.classkey={key}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_first_then_uses_the_list_until_the_mods_change() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), b"1").unwrap();
        fs::write(game.join("mods").join(crate::arctic_mod::FILE_NAME), b"jar").unwrap();
        let first = flags(game, "26.2", 25, true);
        assert!(first[0].starts_with("-javaagent:"));
        assert!(first.iter().any(|f| f.starts_with("-Xlog:class+load")));
        assert!(first.last().unwrap().starts_with("-Darctic.mixincache="));
        // The client wrote its list and the key it was told.
        let key = first
            .iter()
            .find_map(|f| f.strip_prefix("-Darctic.classkey="))
            .unwrap()
            .to_owned();
        fs::write(game.join(DIR).join(LIST), "a.B\n").unwrap();
        fs::write(game.join(DIR).join(KEY), &key).unwrap();
        let second = flags(game, "26.2", 25, true);
        assert_eq!(second.len(), 3);
        assert!(second[0].starts_with("-javaagent:"));
        assert!(second[1].starts_with("-Darctic.preload="));
        let pack = |flags: &[String]| flags.last().unwrap().clone();
        // A changed setting file: other saved classes.
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("config/a.json"), "{}").unwrap();
        let changed = flags(game, "26.2", 25, true);
        assert_ne!(pack(&second), pack(&changed));
        assert_eq!(pack(&changed), pack(&flags(game, "26.2", 25, true)));
        // A mod added: record again.
        fs::write(game.join("mods/b.jar"), b"22").unwrap();
        let third = flags(game, "26.2", 25, true);
        assert!(third.iter().any(|f| f.starts_with("-Xlog:class+load")));
        assert!(!game.join(DIR).join(LIST).exists());
        assert!(flags(game, "26.2", 8, true).is_empty());
        // Not for instances with the player's own mods.
        assert!(!flags(game, "26.2", 25, false).iter().any(|f| f.starts_with("-Darctic.mixincache=")));
        fs::write(game.join(DIR).join("mixin-x.pak"), b"1").unwrap();
        drop_pack(game);
        assert!(!game.join(DIR).join("mixin-x.pak").exists());
    }
}
