//! Offline accounts: no network, deterministic UUID like vanilla servers use.

use md5::{Digest, Md5};

use super::{Account, AccountKind, new_local_id};
use crate::{Error, Result};

const MAX_NAME_LEN: usize = 16;

/// Validate a Minecraft username: 1–16 chars of `[A-Za-z0-9_]`.
pub fn validate_username(name: &str) -> Result<()> {
    let valid_len = (1..=MAX_NAME_LEN).contains(&name.len());
    let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid_len && valid_chars {
        Ok(())
    } else {
        Err(Error::Other(
            "username must be 1-16 characters: letters, digits or _".into(),
        ))
    }
}

/// Same algorithm as Java's `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`,
/// so offline players keep their data on vanilla offline-mode servers.
pub fn offline_uuid(name: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    uuid::Builder::from_md5_bytes(bytes)
        .into_uuid()
        .simple()
        .to_string()
}

pub fn create(name: &str) -> Result<Account> {
    let name = name.trim();
    validate_username(name)?;
    Ok(Account {
        id: new_local_id(),
        username: name.to_owned(),
        uuid: offline_uuid(name),
        kind: AccountKind::Offline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_name_uuid() {
        // UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes(UTF_8))
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }

    #[test]
    fn validates_names() {
        assert!(validate_username("Steve_123").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("has space").is_err());
        assert!(validate_username("abcdefghijklmnopq").is_err());
    }

    #[test]
    fn create_trims() {
        let a = create("  Alex ").unwrap();
        assert_eq!(a.username, "Alex");
        assert_eq!(a.kind, AccountKind::Offline);
    }
}
