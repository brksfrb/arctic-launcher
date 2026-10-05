//! The list of classes a start loads, kept per game folder so the Arctic Client can load
//! them ahead on other cores (its `Preloader`) on the next starts.
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

fn path_arg(path: PathBuf) -> String {
    // The JVM's option syntax uses ':' itself; forward slashes keep a Windows path from tripping on it.
    path.to_string_lossy().replace('\\', "/")
}

/// JVM flags for this start: use the recorded list when it fits, else record a new one.
/// Only for Java 17 and up (older JVMs have another logging syntax).
pub fn flags(game_dir: &Path, game_version: &str, java_major: u32) -> Vec<String> {
    if java_major < 17 {
        return Vec::new();
    }
    let dir = game_dir.join(DIR);
    let key = key(game_dir, game_version, java_major);
    let list = dir.join(LIST);
    if list.is_file() && fs::read_to_string(dir.join(KEY)).is_ok_and(|k| k.trim() == key) {
        return vec![format!("-Darctic.preload={}", path_arg(list))];
    }
    if fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    // Out of date: a new list is recorded by this start.
    let _ = fs::remove_file(&list);
    let _ = fs::remove_file(dir.join(KEY));
    // A file of its own each time: a log the last game still holds open must not stop this one (a JVM
    // that can't open its log doesn't start).
    if let Ok(entries) = fs::read_dir(&dir) {
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
        let first = flags(game, "26.2", 25);
        assert!(first.iter().any(|f| f.starts_with("-Xlog:class+load")));
        // The client wrote its list and the key it was told.
        let key = first
            .iter()
            .find_map(|f| f.strip_prefix("-Darctic.classkey="))
            .unwrap()
            .to_owned();
        fs::write(game.join(DIR).join(LIST), "a.B\n").unwrap();
        fs::write(game.join(DIR).join(KEY), &key).unwrap();
        let second = flags(game, "26.2", 25);
        assert_eq!(second.len(), 1);
        assert!(second[0].starts_with("-Darctic.preload="));
        // A mod added: record again.
        fs::write(game.join("mods/b.jar"), b"22").unwrap();
        let third = flags(game, "26.2", 25);
        assert!(third.iter().any(|f| f.starts_with("-Xlog:class+load")));
        assert!(!game.join(DIR).join(LIST).exists());
        assert!(flags(game, "26.2", 8).is_empty());
    }
}
