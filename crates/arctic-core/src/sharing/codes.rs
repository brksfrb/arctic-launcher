//! Short share codes (`k3zb-bhzh`), stored on the Arctic server.

use serde::Deserialize;
use serde_json::Value;

use super::{Bundle, MAX_JSON};
use crate::cosmetics::server_error;
use crate::net::agent;
use crate::{Error, Result};

/// Same alphabet as the server: no look-alike characters.
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
const CODE_LEN: usize = 8;

/// `code` as the server stores it, if it can be one.
pub fn clean(code: &str) -> Option<String> {
    let code: String = code
        .trim()
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

/// `abcdefgh` → `abcd-efgh`.
pub fn pretty(code: &str) -> String {
    let (a, b) = code.split_at(code.len().min(CODE_LEN / 2));
    format!("{a}-{b}")
}

/// Upload a bundle; returns its (pretty) code. `token` is an Arctic
/// session token (see `cosmetics::token_for`).
pub fn create(base: &str, token: &str, bundle: &Bundle) -> Result<String> {
    #[derive(Deserialize)]
    struct Created {
        code: String,
    }
    let mut resp = agent()
        .post(&format!("{base}/v1/shares"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(bundle.to_value())?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    let created: Created = resp.body_mut().with_config().limit(4096).read_json()?;
    let code = clean(&created.code)
        .ok_or_else(|| Error::Other("the Arctic server sent a strange code".into()))?;
    Ok(pretty(&code))
}

/// The bundle behind a code, fully checked.
pub fn fetch(base: &str, code: &str) -> Result<Bundle> {
    let code = clean(code).ok_or_else(|| Error::Other("that's not a share code".into()))?;
    let mut resp = agent()
        .get(&format!("{base}/v1/shares/{code}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Err(Error::Other(format!(
            "no share has the code {}",
            pretty(&code)
        )));
    }
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    let value: Value = resp
        .body_mut()
        .with_config()
        .limit(MAX_JSON as u64)
        .read_json()?;
    Bundle::from_value(&value)
}
