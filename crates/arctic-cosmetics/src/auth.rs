//! Sign-in without passwords: the client asks for a challenge, tells
//! Mojang's session server it "joined" that challenge (only possible with
//! a valid game session), and we confirm with `hasJoined`, exactly like a
//! Minecraft server does. The result is a signed, expiring token.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

/// How long a challenge may be answered.
const CHALLENGE_TTL: Duration = Duration::from_secs(60);
/// Session tokens last a week.
pub const TOKEN_TTL_SECS: u64 = 7 * 24 * 3600;
const MAX_PENDING: usize = 10_000;

type HmacSha256 = Hmac<Sha256>;

/// Outstanding challenges (server ids).
#[derive(Default)]
pub struct Challenges {
    pending: Mutex<HashMap<String, Instant>>,
}

impl Challenges {
    pub fn issue(&self) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|_, at| at.elapsed() < CHALLENGE_TTL);
        if pending.len() < MAX_PENDING {
            pending.insert(id.clone(), Instant::now());
        }
        id
    }

    /// Consume a challenge; false if unknown or expired.
    pub fn take(&self, id: &str) -> bool {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending
            .remove(id)
            .is_some_and(|at| at.elapsed() < CHALLENGE_TTL)
    }
}

/// `uuid.expiry.signature`
pub fn sign(secret: &[u8], uuid: &str, expires: u64) -> String {
    format!("{uuid}.{expires}.{}", mac(secret, uuid, expires))
}

/// The player's uuid if the token is authentic and unexpired.
pub fn verify(secret: &[u8], token: &str, now: u64) -> Option<String> {
    let mut parts = token.splitn(3, '.');
    let (uuid, expires, sig) = (parts.next()?, parts.next()?, parts.next()?);
    let expires: u64 = expires.parse().ok()?;
    if expires < now || !is_uuid(uuid) {
        return None;
    }
    let mut m = HmacSha256::new_from_slice(secret).ok()?;
    m.update(format!("{uuid}.{expires}").as_bytes());
    m.verify_slice(&hex::decode(sig).ok()?).ok()?;
    Some(uuid.to_owned())
}

fn mac(secret: &[u8], uuid: &str, expires: u64) -> String {
    let mut m = HmacSha256::new_from_slice(secret).expect("HMAC takes any key length");
    m.update(format!("{uuid}.{expires}").as_bytes());
    hex::encode(m.finalize().into_bytes())
}

/// 32 lowercase hex characters (Mojang uuid without dashes).
pub fn is_uuid(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Ask Mojang whether `name` joined `server_id`; returns (uuid, name).
pub fn has_joined(
    session_url: &str,
    name: &str,
    server_id: &str,
) -> Result<Option<(String, String)>, String> {
    #[derive(serde::Deserialize)]
    struct Profile {
        id: String,
        name: String,
    }
    let valid_name = !name.is_empty()
        && name.len() <= 16
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_name {
        return Ok(None);
    }
    let url = format!("{session_url}?username={name}&serverId={server_id}");
    let mut resp = ureq::get(&url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    match resp.status().as_u16() {
        200 => {
            let p: Profile = resp.body_mut().read_json().map_err(|e| e.to_string())?;
            Ok(Some((p.id.to_ascii_lowercase(), p.name)))
        }
        204 | 403 | 404 => Ok(None),
        s => Err(format!("session server returned {s}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "069a79f444e94726a5befca90e38aaf5";

    #[test]
    fn tokens_round_trip_and_expire() {
        let t = sign(b"secret", UUID, 100);
        assert_eq!(verify(b"secret", &t, 50).as_deref(), Some(UUID));
        assert_eq!(verify(b"secret", &t, 101), None);
        assert_eq!(verify(b"other", &t, 50), None);
        let forged = t.replace("100", "999");
        assert_eq!(verify(b"secret", &forged, 50), None);
        assert_eq!(verify(b"secret", "garbage", 0), None);
    }

    #[test]
    fn challenges_are_single_use() {
        let c = Challenges::default();
        let id = c.issue();
        assert!(c.take(&id));
        assert!(!c.take(&id));
        assert!(!c.take("unknown"));
    }

    #[test]
    fn uuid_check() {
        assert!(is_uuid(UUID));
        assert!(!is_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf5"));
        assert!(!is_uuid("ZZZa79f444e94726a5befca90e38aaf5"));
    }
}
