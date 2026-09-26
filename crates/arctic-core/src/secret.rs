//! Secrets (account tokens, the proxy password, Arctic server keys) kept
//! encrypted, on disk and in memory; a [`Secret`] is only decrypted for the
//! moment it's used, into a buffer that's wiped when dropped.
//!
//! Decrypting takes all of: the stored value, this PC (Windows: DPAPI for
//! this Windows user plus the PC's Machine GUID; elsewhere: a random key file
//! only this user can read plus the system's machine id) and the key mixed in
//! by this program. A copied `accounts.json` is useless on its own. Malware
//! already running as the user could still repeat those steps; nothing local
//! can fully stop that.
//!
//! Stored as `"dpapi2:<base64>"` (Windows) or `"local1:<base64>"`. Older
//! values (plain, or `"dpapi:"` from the first encrypted builds) still load
//! and are saved in the current form.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::{Zeroize, Zeroizing};

#[cfg(windows)]
const PREFIX: &str = "dpapi2:";
#[cfg(not(windows))]
const PREFIX: &str = "local1:";
/// First encrypted builds (Windows, without the Machine GUID).
#[cfg(windows)]
const PREFIX_V1: &str = "dpapi:";
/// Mixed into every key, so other programs can't decrypt Arctic's values
/// just by asking the system.
const APP_KEY: &[u8] = b"arctic-launcher/secret/v1";

/// An encrypted value.
#[derive(Clone, Default)]
pub struct Secret {
    sealed: Vec<u8>,
}

impl Secret {
    /// Encrypt `plain`; the given string is wiped.
    pub fn new(plain: impl Into<String>) -> Self {
        let mut plain = plain.into();
        let sealed = if plain.is_empty() {
            Vec::new()
        } else {
            seal(plain.as_bytes())
        };
        plain.zeroize();
        Self { sealed }
    }

    /// The value, wiped from memory when the returned buffer is dropped.
    /// Empty if it can't be decrypted (a file from another PC or user).
    pub fn reveal(&self) -> Zeroizing<String> {
        if self.sealed.is_empty() {
            return Zeroizing::new(String::new());
        }
        let mut bytes = unseal(&self.sealed).unwrap_or_default();
        let text = String::from_utf8(std::mem::take(&mut bytes)).unwrap_or_default();
        bytes.zeroize();
        Zeroizing::new(text)
    }

    pub fn is_empty(&self) -> bool {
        self.sealed.is_empty()
    }

    /// False when it was encrypted on another PC or by another user.
    pub fn readable(&self) -> bool {
        self.sealed.is_empty() || unseal(&self.sealed).is_some()
    }

    fn to_stored(&self) -> String {
        format!("{PREFIX}{}", STANDARD.encode(&self.sealed))
    }

    fn from_stored(text: String) -> Self {
        if let Some(b64) = text.strip_prefix(PREFIX) {
            return Self {
                sealed: STANDARD.decode(b64).unwrap_or_default(),
            };
        }
        #[cfg(windows)]
        if let Some(b64) = text.strip_prefix(PREFIX_V1) {
            let old = STANDARD.decode(b64).unwrap_or_default();
            let mut plain = dpapi::unprotect(&old, APP_KEY).unwrap_or_default();
            let secret =
                Self::new(String::from_utf8(std::mem::take(&mut plain)).unwrap_or_default());
            plain.zeroize();
            return secret;
        }
        Self::new(text)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        *self.reveal() == *other.reveal()
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_stored())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::from_stored)
    }
}

/// For `String` fields that should only be encrypted on disk
/// (`#[serde(with = "crate::secret::on_disk")]`).
pub mod on_disk {
    use super::*;

    pub fn serialize<S: Serializer>(value: &str, s: S) -> Result<S::Ok, S::Error> {
        Secret::new(value).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        let secret = Secret::deserialize(d)?;
        Ok(secret.reveal().to_string())
    }
}

/// Like [`on_disk`], for `Option<String>`.
pub mod on_disk_opt {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
        value.as_deref().map(Secret::new).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        let secret = Option::<Secret>::deserialize(d)?;
        Ok(secret.map(|s| s.reveal().to_string()))
    }
}

/// Does this JSON text still hold a value that isn't in the current
/// encrypted form? (Then the file is saved again.)
pub fn has_plain(json: &str, fields: &[&str]) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return false;
    };
    fn walk(v: &serde_json::Value, fields: &[&str]) -> bool {
        match v {
            serde_json::Value::Object(map) => map.iter().any(|(k, v)| {
                (fields.contains(&k.as_str())
                    && v.as_str()
                        .is_some_and(|s| !s.is_empty() && !s.starts_with(PREFIX)))
                    || walk(v, fields)
            }),
            serde_json::Value::Array(list) => list.iter().any(|v| walk(v, fields)),
            _ => false,
        }
    }
    walk(&value, fields)
}

#[cfg(windows)]
fn seal(plain: &[u8]) -> Vec<u8> {
    dpapi::protect(plain, &windows_key()).unwrap_or_default()
}

#[cfg(windows)]
fn unseal(sealed: &[u8]) -> Option<Vec<u8>> {
    dpapi::unprotect(sealed, &windows_key())
}

/// The program's key plus this PC's Machine GUID.
#[cfg(windows)]
fn windows_key() -> Vec<u8> {
    let mut key = APP_KEY.to_vec();
    key.extend_from_slice(dpapi::machine_guid().as_bytes());
    key
}

#[cfg(not(windows))]
fn seal(plain: &[u8]) -> Vec<u8> {
    local::seal(plain).unwrap_or_default()
}

#[cfg(not(windows))]
fn unseal(sealed: &[u8]) -> Option<Vec<u8>> {
    local::unseal(sealed)
}

#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
    };

    fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        }
    }

    /// Copy DPAPI's output and free it (wiping the plain text first).
    fn take(out: CRYPT_INTEGER_BLOB, wipe: bool) -> Vec<u8> {
        // SAFETY: DPAPI filled `out` with a LocalAlloc'd buffer of cbData bytes.
        unsafe {
            let slice = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
            let copy = slice.to_vec();
            if wipe {
                slice.fill(0);
            }
            LocalFree(out.pbData as _);
            copy
        }
    }

    pub fn protect(plain: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        let input = blob(plain);
        let entropy = blob(key);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: all pointers are valid for the call; `out` is freed in `take`.
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        (ok != 0).then(|| take(out, false))
    }

    pub fn unprotect(sealed: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        let input = blob(sealed);
        let entropy = blob(key);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: as above.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        (ok != 0).then(|| take(out, true))
    }

    /// `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (set when Windows
    /// is installed), or empty if it can't be read.
    pub fn machine_guid() -> String {
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (key, name) = (
            wide("SOFTWARE\\Microsoft\\Cryptography"),
            wide("MachineGuid"),
        );
        let mut buf = [0u16; 64];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: the strings are NUL-terminated and `buf` holds `len` bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if status != 0 {
            return String::new();
        }
        let chars = (len as usize / 2).saturating_sub(1).min(buf.len());
        String::from_utf16_lossy(&buf[..chars])
    }
}

/// Linux and macOS: AES-256-GCM with a key made from a random key file
/// (readable only by this user), the system's machine id and the program's
/// key.
#[cfg(not(windows))]
mod local {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    use sha2::{Digest, Sha256};
    use zeroize::Zeroize;

    const NONCE_LEN: usize = 12;
    const KEY_LEN: usize = 32;

    fn key_file() -> Option<std::path::PathBuf> {
        dirs::data_local_dir().map(|d| d.join("arctic-launcher").join(".secret-key"))
    }

    /// The random key, created on first use.
    fn file_key() -> Option<Vec<u8>> {
        let path = key_file()?;
        if let Ok(bytes) = std::fs::read(&path)
            && bytes.len() == KEY_LEN
        {
            return Some(bytes);
        }
        let mut key = vec![0u8; KEY_LEN];
        getrandom::fill(&mut key).ok()?;
        std::fs::create_dir_all(path.parent()?).ok()?;
        write_private(&path, &key)?;
        Some(key)
    }

    #[cfg(unix)]
    fn write_private(path: &std::path::Path, bytes: &[u8]) -> Option<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .ok()?;
        f.write_all(bytes).ok()
    }

    #[cfg(not(unix))]
    fn write_private(path: &std::path::Path, bytes: &[u8]) -> Option<()> {
        std::fs::write(path, bytes).ok()
    }

    fn machine_id() -> String {
        ["/etc/machine-id", "/var/lib/dbus/machine-id"]
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    }

    fn cipher() -> Option<Aes256Gcm> {
        let mut file = file_key()?;
        let mut hasher = Sha256::new();
        hasher.update(&file);
        hasher.update(machine_id().as_bytes());
        hasher.update(super::APP_KEY);
        let mut key = hasher.finalize().to_vec();
        file.zeroize();
        let cipher = Aes256Gcm::new_from_slice(&key).ok();
        key.zeroize();
        cipher
    }

    pub fn seal(plain: &[u8]) -> Option<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).ok()?;
        let sealed = cipher()?.encrypt(Nonce::from_slice(&nonce), plain).ok()?;
        Some([nonce.as_slice(), &sealed].concat())
    }

    pub fn unseal(sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() <= NONCE_LEN {
            return None;
        }
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        cipher()?.decrypt(Nonce::from_slice(nonce), body).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_hides_the_value() {
        let s = Secret::new("M.C123-refresh-token");
        assert_eq!(&*s.reveal(), "M.C123-refresh-token");
        let stored = serde_json::to_string(&s).unwrap();
        assert!(stored.starts_with(&format!("\"{PREFIX}")));
        assert!(!stored.contains("refresh-token"));
        let back: Secret = serde_json::from_str(&stored).unwrap();
        assert_eq!(back, s);
        assert!(!format!("{s:?}").contains("refresh"));
    }

    #[test]
    fn plain_values_from_old_files_still_load() {
        let s: Secret = serde_json::from_str("\"old-plain-token\"").unwrap();
        assert_eq!(&*s.reveal(), "old-plain-token");
        let json = r#"{"accounts":[{"kind":{"refresh_token":"abc","access_token":"dpapi:xyz"}}]}"#;
        assert_eq!(
            has_plain(json, &["refresh_token", "access_token"]),
            cfg!(windows)
        );
    }

    #[test]
    fn a_value_from_elsewhere_reads_as_empty() {
        // Not a DPAPI blob made here (like a file copied from another PC).
        let s: Secret = serde_json::from_str(&format!("\"{PREFIX}AAAAAAAAAAA=\"")).unwrap();
        assert!(!s.readable());
        assert_eq!(&*s.reveal(), "");
    }

    #[test]
    fn empty_stays_empty() {
        let s = Secret::new("");
        assert!(s.is_empty());
        assert_eq!(&*s.reveal(), "");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn values_from_the_first_encrypted_builds_move_to_the_new_form() {
        let old = dpapi::protect(b"legacy-token", APP_KEY).unwrap();
        let stored = format!("\"{PREFIX_V1}{}\"", STANDARD.encode(old));
        let s: Secret = serde_json::from_str(&stored).unwrap();
        assert_eq!(&*s.reveal(), "legacy-token");
        assert!(
            serde_json::to_string(&s)
                .unwrap()
                .starts_with(&format!("\"{PREFIX}"))
        );
    }

    #[test]
    fn reads_the_machine_guid() {
        assert_eq!(dpapi::machine_guid().len(), 36);
    }
}
