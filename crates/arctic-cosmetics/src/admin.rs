//! Who may moderate: named keys, so access can be shared with other people
//! and taken back one person at a time.

use sha2::{Digest, Sha256};

/// Shortest key accepted; anything shorter is ignored at startup.
pub const MIN_KEY_LEN: usize = 16;

#[derive(Default)]
pub struct Moderators {
    /// (name, SHA-256 of the key). Only hashes are kept in memory.
    keys: Vec<(String, [u8; 32])>,
}

fn hash(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

/// Equal without stopping at the first difference.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

impl Moderators {
    /// `ARCTIC_COSMETICS_ADMIN_KEYS` is `name:key,name:key`; the older
    /// single `ARCTIC_COSMETICS_ADMIN_KEY` counts as the moderator "admin".
    /// Entries that are malformed, repeat a name or have a short key are
    /// skipped (and said so, without the key).
    pub fn parse(list: Option<&str>, single: Option<&str>) -> Self {
        let mut keys: Vec<(String, [u8; 32])> = Vec::new();
        let mut add = |name: &str, key: &str| {
            let name = name.trim();
            let valid_name = !name.is_empty()
                && name.len() <= 32
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid_name || key.len() < MIN_KEY_LEN || keys.iter().any(|(n, _)| n == name) {
                log::warn!("admin key for {name:?} ignored (bad name, short key or repeated)");
                return;
            }
            keys.push((name.to_owned(), hash(key)));
        };
        for entry in list
            .unwrap_or_default()
            .split(',')
            .filter(|e| !e.trim().is_empty())
        {
            match entry.split_once(':') {
                Some((name, key)) => add(name, key.trim()),
                None => log::warn!("an admin key entry has no name:key shape; ignored"),
            }
        }
        if let Some(key) = single {
            add("admin", key.trim());
        }
        Self { keys }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The moderator this key belongs to. Every key is compared, so the
    /// time taken doesn't say how close a guess was or whose it nearly was.
    pub fn identify(&self, key: &str) -> Option<&str> {
        let given = hash(key);
        let mut found = None;
        for (name, stored) in &self.keys {
            if same(stored, &given) {
                found = Some(name.as_str());
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_identify_their_owner() {
        let m = Moderators::parse(
            Some("alice:aaaaaaaaaaaaaaaaaaaa, bob:bbbbbbbbbbbbbbbbbbbb"),
            None,
        );
        assert_eq!(m.identify("aaaaaaaaaaaaaaaaaaaa"), Some("alice"));
        assert_eq!(m.identify("bbbbbbbbbbbbbbbbbbbb"), Some("bob"));
        assert_eq!(m.identify("cccccccccccccccccccc"), None);
        assert_eq!(m.identify(""), None);
    }

    #[test]
    fn single_key_is_admin_and_bad_entries_are_skipped() {
        let m = Moderators::parse(
            Some(
                "short:abc,no-colon,alice:aaaaaaaaaaaaaaaaaaaa,alice:dddddddddddddddddddd,bad name:eeeeeeeeeeeeeeeeeeee",
            ),
            Some("admin-key-admin-key"),
        );
        assert_eq!(m.identify("admin-key-admin-key"), Some("admin"));
        assert_eq!(m.identify("aaaaaaaaaaaaaaaaaaaa"), Some("alice"));
        assert_eq!(m.identify("dddddddddddddddddddd"), None);
        assert_eq!(m.identify("eeeeeeeeeeeeeeeeeeee"), None);
        assert!(Moderators::parse(None, None).is_empty());
    }
}
