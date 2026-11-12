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

/// An encrypted value. When it can't be encrypted (the key file can't be written) the value is kept in
/// memory for this run only: it is never written out, and never written as an empty "sealed" value.
#[derive(Clone, Default)]
pub struct Secret {
    sealed: Vec<u8>,
    unsealed: Option<Zeroizing<String>>,
}

static SEAL_PROBLEM: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static KEY_PROBLEM: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn slot(
    slot: &'static std::sync::Mutex<Option<String>>,
) -> std::sync::MutexGuard<'static, Option<String>> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Sealing failed: say why, once until it works again (never the secret itself).
fn seal_failed(reason: String) {
    let mut problem = slot(&SEAL_PROBLEM);
    if problem.as_deref() != Some(reason.as_str()) {
        log::warn!(
            "can't encrypt saved logins ({reason}); they stay in memory for this session and are not saved"
        );
        *problem = Some(reason);
    }
}

/// Something that must be saved can't be (the data folder isn't writable): shown in the UI.
pub(crate) fn key_unsaved(reason: String) {
    *slot(&KEY_PROBLEM) = Some(reason);
}

pub(crate) fn key_saved() {
    *slot(&KEY_PROBLEM) = None;
}

/// A one-line reason saved logins or offline keys can't be kept on this PC right now, if so.
pub fn problem() -> Option<String> {
    slot(&SEAL_PROBLEM)
        .clone()
        .or_else(|| slot(&KEY_PROBLEM).clone())
}

impl Secret {
    /// Encrypt `plain`; the given string is wiped.
    pub fn new(plain: impl Into<String>) -> Self {
        let mut plain = plain.into();
        if plain.is_empty() {
            return Self::default();
        }
        let this = match seal(plain.as_bytes()) {
            Ok(sealed) if !sealed.is_empty() => {
                *slot(&SEAL_PROBLEM) = None;
                Self {
                    sealed,
                    unsealed: None,
                }
            }
            Ok(_) => {
                seal_failed("encryption returned nothing".into());
                Self::in_memory(&plain)
            }
            Err(reason) => {
                seal_failed(reason);
                Self::in_memory(&plain)
            }
        };
        plain.zeroize();
        this
    }

    fn in_memory(plain: &str) -> Self {
        Self {
            sealed: Vec::new(),
            unsealed: Some(Zeroizing::new(plain.to_owned())),
        }
    }

    /// Holds a value that couldn't be encrypted, so it can't be saved.
    pub fn unsaveable(&self) -> bool {
        self.unsealed.is_some()
    }

    /// The value, wiped from memory when the returned buffer is dropped.
    /// Empty if it can't be decrypted (a file from another PC or user).
    pub fn reveal(&self) -> Zeroizing<String> {
        if let Some(plain) = &self.unsealed {
            return Zeroizing::new(plain.to_string());
        }
        if self.sealed.is_empty() {
            return Zeroizing::new(String::new());
        }
        let mut bytes = unseal(&self.sealed).unwrap_or_default();
        let text = String::from_utf8(std::mem::take(&mut bytes)).unwrap_or_default();
        bytes.zeroize();
        Zeroizing::new(text)
    }

    pub fn is_empty(&self) -> bool {
        self.sealed.is_empty() && self.unsealed.is_none()
    }

    /// False when it was encrypted on another PC or by another user.
    pub fn readable(&self) -> bool {
        self.sealed.is_empty() || unseal(&self.sealed).is_some()
    }

    /// Nothing at all for an empty value: never a prefix with nothing sealed behind it.
    fn to_stored(&self) -> String {
        if self.sealed.is_empty() {
            return String::new();
        }
        format!("{PREFIX}{}", STANDARD.encode(&self.sealed))
    }

    fn from_stored(text: String) -> Self {
        if let Some(b64) = text.strip_prefix(PREFIX) {
            return Self {
                sealed: STANDARD.decode(b64).unwrap_or_default(),
                unsealed: None,
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
        if self.unsealed.is_some() {
            return Err(serde::ser::Error::custom(format!(
                "can't encrypt it to save it ({})",
                problem().unwrap_or_else(|| "no reason known".into())
            )));
        }
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
        // A value that can't be encrypted is left out, not saved empty (it stays usable in memory).
        value
            .as_deref()
            .map(Secret::new)
            .filter(|secret| !secret.unsaveable())
            .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        let secret = Option::<Secret>::deserialize(d)?;
        Ok(secret.map(|s| s.reveal().to_string()))
    }
}

/// For `Option<String>` fields saved as they are (not encrypted) that older builds saved encrypted: a
/// plain value is returned as it is, never sent through encryption (which can fail).
pub mod plain_or_sealed_opt {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        Ok(text.map(|text| {
            if text.starts_with(PREFIX) || is_old_prefix(&text) {
                Secret::from_stored(text).reveal().to_string()
            } else {
                text
            }
        }))
    }

    #[cfg(windows)]
    fn is_old_prefix(text: &str) -> bool {
        text.starts_with(PREFIX_V1)
    }

    #[cfg(not(windows))]
    fn is_old_prefix(_: &str) -> bool {
        false
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

/// Tests: from now on this thread's key file can't be created (its folder is under a plain file).
#[cfg(all(test, not(windows)))]
pub(crate) fn break_key_file_for_this_thread(root: &std::path::Path) -> std::path::PathBuf {
    let blocker = root.join("not-a-folder");
    std::fs::write(&blocker, b"x").unwrap();
    let path = blocker.join("sub").join(".secret-key");
    local::TEST_KEY_FILE.with(|p| *p.borrow_mut() = Some(path.clone()));
    path
}

#[cfg(windows)]
fn seal(plain: &[u8]) -> Result<Vec<u8>, String> {
    dpapi::protect(plain, &windows_key()).ok_or_else(|| "Windows refused to encrypt (DPAPI)".into())
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
fn seal(plain: &[u8]) -> Result<Vec<u8>, String> {
    local::seal(plain)
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

    #[cfg(test)]
    thread_local! {
        pub static TEST_KEY_FILE: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
    }

    pub fn key_file() -> Option<std::path::PathBuf> {
        #[cfg(test)]
        if let Some(path) = TEST_KEY_FILE.with(|p| p.borrow().clone()) {
            return Some(path);
        }
        dirs::data_local_dir().map(|d| d.join("arctic-launcher").join(".secret-key"))
    }

    /// The random key, created on first use. The error says which file and why.
    fn file_key() -> Result<Vec<u8>, String> {
        let path = key_file().ok_or("no data folder known for this user")?;
        if let Ok(bytes) = std::fs::read(&path)
            && bytes.len() == KEY_LEN
        {
            return Ok(bytes);
        }
        let mut key = vec![0u8; KEY_LEN];
        getrandom::fill(&mut key).map_err(|e| format!("no random numbers: {e}"))?;
        let parent = path.parent().ok_or("key file has no folder")?;
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("can't create {}: {e}", parent.display()))?;
        write_private(&path, &key).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        Ok(key)
    }

    #[cfg(unix)]
    fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(bytes)
    }

    #[cfg(not(unix))]
    fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, bytes)
    }

    fn machine_id() -> String {
        ["/etc/machine-id", "/var/lib/dbus/machine-id"]
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    }

    fn cipher() -> Result<Aes256Gcm, String> {
        let mut file = file_key()?;
        let mut hasher = Sha256::new();
        hasher.update(&file);
        hasher.update(machine_id().as_bytes());
        hasher.update(super::APP_KEY);
        let mut key = hasher.finalize().to_vec();
        file.zeroize();
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string());
        key.zeroize();
        cipher
    }

    pub fn seal(plain: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|e| format!("no random numbers: {e}"))?;
        let sealed = cipher()?
            .encrypt(Nonce::from_slice(&nonce), plain)
            .map_err(|e| e.to_string())?;
        Ok([nonce.as_slice(), &sealed].concat())
    }

    pub fn unseal(sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() <= NONCE_LEN {
            return None;
        }
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        cipher().ok()?.decrypt(Nonce::from_slice(nonce), body).ok()
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
        let fields = ["refresh_token", "access_token"];
        // A plain value anywhere: the file gets saved again, encrypted.
        let json = r#"{"accounts":[{"kind":{"refresh_token":"abc","access_token":"dpapi:xyz"}}]}"#;
        assert!(has_plain(json, &fields));
        // Everything in the current form: left alone.
        let sealed = format!(
            r#"{{"accounts":[{{"kind":{{"refresh_token":"{PREFIX}abc","access_token":""}}}}]}}"#
        );
        assert!(!has_plain(&sealed, &fields));
    }

    #[test]
    fn a_value_from_elsewhere_reads_as_empty() {
        // Not a DPAPI blob made here (like a file copied from another PC).
        let s: Secret = serde_json::from_str(&format!("\"{PREFIX}AAAAAAAAAAA=\"")).unwrap();
        assert!(!s.readable());
        assert_eq!(&*s.reveal(), "");
    }

    #[cfg(not(windows))]
    #[test]
    fn a_secret_that_cannot_be_sealed_is_never_saved_empty() {
        let dir = tempfile::tempdir().unwrap();
        let key_file = break_key_file_for_this_thread(dir.path());
        let s = Secret::new("M.C123-refresh-token");
        // Still usable for this run...
        assert_eq!(&*s.reveal(), "M.C123-refresh-token");
        assert!(!s.is_empty() && s.unsaveable());
        // ...but saving it is an error naming the cause, never an empty "local1:" value.
        let err = serde_json::to_string(&s).unwrap_err().to_string();
        assert!(err.contains("can't encrypt"), "{err}");
        assert!(!err.contains("refresh-token"));
        let reason = problem().unwrap();
        assert!(
            reason.contains(&*key_file.parent().unwrap().to_string_lossy()),
            "{reason}"
        );
        assert!(!reason.contains("refresh-token"));
        // An optional field (the session token) is left out instead.
        #[derive(Serialize)]
        struct Holder {
            #[serde(with = "on_disk_opt")]
            token: Option<String>,
        }
        let json = serde_json::to_string(&Holder {
            token: Some("session-token".into()),
        })
        .unwrap();
        assert_eq!(json, r#"{"token":null}"#);
        // An empty value is saved as nothing, with no prefix.
        assert_eq!(serde_json::to_string(&Secret::new("")).unwrap(), "\"\"");
    }

    #[cfg(not(windows))]
    #[test]
    fn a_plain_stored_value_reads_back_even_when_sealing_is_broken() {
        let dir = tempfile::tempdir().unwrap();
        break_key_file_for_this_thread(dir.path());
        #[derive(Deserialize)]
        struct Holder {
            #[serde(with = "plain_or_sealed_opt")]
            key: Option<String>,
        }
        let h: Holder = serde_json::from_str(r#"{"key":"abcdef0123456789"}"#).unwrap();
        assert_eq!(h.key.as_deref(), Some("abcdef0123456789"));
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
