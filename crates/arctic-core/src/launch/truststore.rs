//! Java 8's certificate authorities, brought up to date.
//!
//! Minecraft 1.16.5 and older run on Mojang's Java 8, which is 8u51 from 2015: its list of
//! trusted certificate authorities is too old for today's HTTPS, so the game can't reach Mojang's
//! session server (OptiFine's cape editor: "Cannot contact authentication server"), skins, Realms
//! or Arctic's own servers. Such a game gets a truststore made from this computer's certificate
//! authorities (kept up to date by the operating system), in the JKS format Java 8 reads.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha1::{Digest, Sha1};

use crate::storage::DataDirs;

const FILE: &str = "java8-truststore.jks";
/// The store's password: only there for Java's integrity check, the certificates are public.
const PASSWORD: &str = "changeit";
/// Made again this often, so authorities the system adds or removes follow.
const REFRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// JVM flags for a game on Java `java_major`: the truststore for Java 8 and older, nothing for
/// newer Java (its own authorities are recent enough) or when the store can't be made.
pub fn flags(dirs: &DataDirs, java_major: u32) -> Vec<String> {
    if java_major > 8 {
        return Vec::new();
    }
    match ensure(&dirs.cache()) {
        Ok(path) => vec![
            format!("-Djavax.net.ssl.trustStore={}", path.display()),
            format!("-Djavax.net.ssl.trustStorePassword={PASSWORD}"),
            "-Djavax.net.ssl.trustStoreType=JKS".to_owned(),
        ],
        Err(e) => {
            log::warn!("Java 8 truststore: {e}");
            Vec::new()
        }
    }
}

fn ensure(cache: &Path) -> std::io::Result<PathBuf> {
    let path = cache.join(FILE);
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < REFRESH);
    if fresh {
        return Ok(path);
    }
    let found = rustls_native_certs::load_native_certs();
    let certs: Vec<&[u8]> = found.certs.iter().map(|c| c.as_ref()).collect();
    if certs.is_empty() {
        return Err(std::io::Error::other(format!(
            "no certificate authorities found on this computer ({} errors)",
            found.errors.len()
        )));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    std::fs::create_dir_all(cache)?;
    let tmp = path.with_extension("jks.tmp");
    std::fs::write(&tmp, jks(&certs, now))?;
    std::fs::rename(&tmp, &path)?;
    log::info!("Java 8 truststore: {} certificate authorities", certs.len());
    Ok(path)
}

/// A JKS keystore (version 2) of trusted certificates, sealed with [`PASSWORD`].
fn jks(certs: &[&[u8]], timestamp_ms: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xFEED_FEEDu32.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&(certs.len() as u32).to_be_bytes());
    for (i, der) in certs.iter().enumerate() {
        // 2: a trusted certificate entry.
        out.extend_from_slice(&2u32.to_be_bytes());
        java_utf(&mut out, &format!("arctic-ca-{i}"));
        out.extend_from_slice(&timestamp_ms.to_be_bytes());
        java_utf(&mut out, "X.509");
        out.extend_from_slice(&(der.len() as u32).to_be_bytes());
        out.extend_from_slice(der);
    }
    // Java's integrity check: SHA-1 over the password (UTF-16BE), "Mighty Aphrodite" and the store.
    let mut sha = Sha1::new();
    for unit in PASSWORD.encode_utf16() {
        sha.update(unit.to_be_bytes());
    }
    sha.update(b"Mighty Aphrodite");
    sha.update(&out);
    out.extend_from_slice(&sha.finalize());
    out
}

/// A string as Java's DataOutput.writeUTF writes it (ASCII here, so plain bytes).
fn java_utf(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_has_its_header_entries_and_seal() {
        let store = jks(&[b"abc", b"de"], 7);
        assert_eq!(&store[..4], &[0xFE, 0xED, 0xFE, 0xED]);
        assert_eq!(u32::from_be_bytes(store[8..12].try_into().unwrap()), 2);
        let body = &store[..store.len() - 20];
        let mut sha = Sha1::new();
        for unit in PASSWORD.encode_utf16() {
            sha.update(unit.to_be_bytes());
        }
        sha.update(b"Mighty Aphrodite");
        sha.update(body);
        assert_eq!(&store[store.len() - 20..], sha.finalize().as_slice());
    }

    #[test]
    fn newer_java_gets_no_flags() {
        let dir = tempfile::tempdir().unwrap();
        assert!(flags(&DataDirs::new(dir.path()), 17).is_empty());
    }
}
