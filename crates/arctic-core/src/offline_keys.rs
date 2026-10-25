//! The keys that claim offline names on the Arctic server.
//!
//! A key is the only proof that an offline name is yours: lose it and the server refuses the name as
//! "used by another Arctic player" until a recovery code or a moderator frees it. So a key, once made, is
//! never destroyed. Every key lives in `cosmetics.json` and also in two ledgers that only ever grow (one in
//! the profile folder, one in the data folder, so deleting or resetting either place keeps it), and
//! `cosmetics.json` gets the key back from them if it ever lacks it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

const FILE: &str = "offline-keys.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    accounts: BTreeMap<String, Account>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Account {
    #[serde(default)]
    name: String,
    /// Every key the account has had, oldest first (a recovery makes a newer one; none is ever dropped).
    #[serde(default)]
    keys: Vec<String>,
}

/// Where the ledgers are (once if the profile folder is the data folder).
fn places(dirs: &DataDirs) -> Vec<PathBuf> {
    let mut places = vec![dirs.profile_root().join(FILE)];
    let shared = dirs.root().join(FILE);
    if !places.contains(&shared) {
        places.push(shared);
    }
    places
}

/// What a ledger holds. One that can't be read is set aside (kept, never overwritten) and starts again.
fn read(path: &PathBuf) -> Ledger {
    match load_json::<Ledger>(path) {
        Ok(ledger) => ledger.unwrap_or_default(),
        Err(e) => {
            log::warn!("{} is unreadable ({e}); keeping it aside", path.display());
            set_aside(path);
            Ledger::default()
        }
    }
}

/// Move an unreadable file out of the way without deleting it.
pub(crate) fn set_aside(path: &PathBuf) {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".unreadable-{}", crate::auth::now_secs()));
    let _ = std::fs::rename(path, PathBuf::from(name));
}

/// The newest key any ledger has for `uuid` (the ledger with the longest history wins).
pub fn recall(dirs: &DataDirs, uuid: &str) -> Option<String> {
    places(dirs)
        .iter()
        .filter_map(|p| read(p).accounts.remove(uuid))
        .max_by_key(|a| a.keys.len())
        .and_then(|a| a.keys.last().cloned())
}

/// Every key a ledger text holds for `uuid`, newest first (any ledger file, also from another profile).
pub fn keys_in_ledger(text: &str, uuid: &str) -> Vec<String> {
    let Ok(mut ledger) = serde_json::from_str::<Ledger>(text) else {
        return Vec::new();
    };
    let mut keys = ledger
        .accounts
        .remove(uuid)
        .map(|a| a.keys)
        .unwrap_or_default();
    keys.reverse();
    keys
}

/// Every key the ledgers hold for `uuid`, newest first.
pub fn history(dirs: &DataDirs, uuid: &str) -> Vec<String> {
    places(dirs)
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|text| keys_in_ledger(&text, uuid))
        .collect()
}

/// Files that may hold an offline key: `cosmetics.json` and the ledgers, with their backups and set-aside
/// copies, in the data folder, in every profile and in removed profiles.
pub fn stored_files(dirs: &DataDirs) -> Vec<PathBuf> {
    /// Largest file read (these are tiny).
    const MAX_BYTES: u64 = 4 << 20;
    let mut folders = vec![dirs.root().to_path_buf(), dirs.profile_root().to_path_buf()];
    let profiles = dirs.profiles_dir();
    for entry in std::fs::read_dir(&profiles).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == ".trash" {
            folders.extend(
                std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir()),
            );
        } else {
            folders.push(path);
        }
    }
    let mut files = Vec::new();
    for folder in folders {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = name.starts_with("cosmetics.json") || name.starts_with(FILE);
            let small = entry
                .metadata()
                .is_ok_and(|m| m.is_file() && m.len() <= MAX_BYTES);
            if ours && small && !files.contains(&entry.path()) {
                files.push(entry.path());
            }
        }
    }
    files
}

/// Write `key` into every ledger as the account's newest. Fails only when none could be written, so a
/// name is never claimed with a key that was saved nowhere.
pub fn remember(dirs: &DataDirs, uuid: &str, name: &str, key: &str) -> Result<()> {
    let mut saved = false;
    let mut last_error = None;
    for place in places(dirs) {
        let mut ledger = read(&place);
        let account = ledger.accounts.entry(uuid.to_owned()).or_default();
        if account.keys.last().map(String::as_str) == Some(key) && account.name == name {
            saved = true;
            continue;
        }
        account.name = name.to_owned();
        account.keys.retain(|k| k != key);
        account.keys.push(key.to_owned());
        match save_json(&place, &ledger) {
            Ok(()) => saved = true,
            Err(e) => {
                log::warn!("couldn't save the offline key to {}: {e}", place.display());
                last_error = Some(e);
            }
        }
    }
    if saved {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| Error::Other("no place to save the offline key".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, DataDirs) {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        (dir, dirs)
    }

    #[test]
    fn a_remembered_key_comes_back() {
        let (_dir, dirs) = dirs();
        assert_eq!(recall(&dirs, "u1"), None);
        remember(&dirs, "u1", "Alex", &"ab".repeat(32)).unwrap();
        assert_eq!(recall(&dirs, "u1"), Some("ab".repeat(32)));
    }

    #[test]
    fn a_newer_key_wins_but_the_old_one_is_kept() {
        let (_dir, dirs) = dirs();
        remember(&dirs, "u1", "Alex", &"ab".repeat(32)).unwrap();
        remember(&dirs, "u1", "Alex", &"cd".repeat(32)).unwrap();
        assert_eq!(recall(&dirs, "u1"), Some("cd".repeat(32)));
        let ledger = read(&places(&dirs)[0]);
        assert_eq!(ledger.accounts["u1"].keys.len(), 2);
    }

    #[test]
    fn losing_one_ledger_loses_nothing() {
        let (_dir, dirs) = dirs();
        remember(&dirs, "u1", "Alex", &"ab".repeat(32)).unwrap();
        for place in places(&dirs) {
            std::fs::remove_file(&place).unwrap();
            assert_eq!(recall(&dirs, "u1"), Some("ab".repeat(32)));
            // The next sign-in puts the key back where it went missing.
            remember(&dirs, "u1", "Alex", &"ab".repeat(32)).unwrap();
            assert!(place.is_file());
        }
    }

    #[test]
    fn an_unreadable_ledger_is_kept_aside_and_the_other_copy_serves() {
        let (_dir, dirs) = dirs();
        remember(&dirs, "u1", "Alex", &"ab".repeat(32)).unwrap();
        let first = places(&dirs)[0].clone();
        std::fs::write(&first, "{ not json").unwrap();
        let _ = std::fs::remove_file(first.with_extension("json.bak"));
        assert_eq!(recall(&dirs, "u1"), Some("ab".repeat(32)));
        let kept = std::fs::read_dir(first.parent().unwrap())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains(".unreadable-"));
        assert!(kept, "the unreadable file is set aside, not deleted");
    }
}
