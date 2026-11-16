//! The link endpoints on the Arctic server (see docs/linked-instances.md).
//! A link is a manifest stored under a short code. Whoever made it got an
//! owner key back, which is the only thing that can change or delete it; no
//! account is needed to read one by its code.

use serde::Deserialize;
use serde_json::json;

use super::manifest::{MAX_MANIFEST, Manifest};
use crate::cosmetics::server_error;
use crate::net::agent;
use crate::sharing::codes;
use crate::{Error, Result};

/// Header that carries the owner key.
pub const KEY_HEADER: &str = "X-Arctic-Link-Key";
const MAX_SMALL_ANSWER: u64 = 16 * 1024;
const MIN_KEY: usize = 32;
const MAX_KEY: usize = 128;

/// A new link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    /// Clean code (`abcdefgh`); show it with `codes::pretty`.
    pub code: String,
    pub key: String,
    pub revision: u64,
}

/// A link as fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub revision: u64,
    /// Unix seconds of the last publish.
    pub updated: u64,
    pub manifest: Manifest,
}

fn url(base: &str, rest: &str) -> String {
    format!("{}/v1/links{rest}", base.trim_end_matches('/'))
}

fn code_of(code: &str) -> Result<String> {
    codes::clean(code).ok_or_else(|| Error::Other("that's not a link code".into()))
}

fn check(resp: &mut ureq::http::Response<ureq::Body>, status: u16) -> Result<()> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    Err(match status {
        404 | 410 => Error::Other("no link has that code (or its owner stopped sharing it)".into()),
        403 => Error::Other("this PC isn't the owner of that link".into()),
        409 => Error::Other(
            "the link changed elsewhere since you last published; open it again".into(),
        ),
        413 => Error::Other("the instance is too big to link: leave out some config files".into()),
        _ => Error::Other(server_error(resp, status)),
    })
}

/// Store `manifest` under a new code. `token`: an Arctic session if there is one (the
/// server may use it against abuse; links work without).
pub fn create(base: &str, token: Option<&str>, manifest: &Manifest) -> Result<Created> {
    #[derive(Deserialize)]
    struct Answer {
        code: String,
        key: String,
        #[serde(default)]
        revision: u64,
    }
    let body =
        json!({ "manifest": serde_json::from_str::<serde_json::Value>(&manifest.to_json()?)? });
    let mut req = agent().post(&url(base, ""));
    if let Some(token) = token {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }
    let mut resp = req
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(body)?;
    let status = resp.status().as_u16();
    check(&mut resp, status)?;
    let a: Answer = resp
        .body_mut()
        .with_config()
        .limit(MAX_SMALL_ANSWER)
        .read_json()?;
    let code = codes::clean(&a.code)
        .ok_or_else(|| Error::Other("the Arctic server sent a strange code".into()))?;
    if a.key.len() < MIN_KEY
        || a.key.len() > MAX_KEY
        || !a.key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(Error::Other("the Arctic server sent a strange key".into()));
    }
    Ok(Created {
        code,
        key: a.key,
        revision: a.revision.max(1),
    })
}

/// Publish a new revision. `base_revision` is the one this PC published last; the server
/// refuses (409) if the link moved on since.
pub fn publish(
    base: &str,
    code: &str,
    key: &str,
    base_revision: u64,
    manifest: &Manifest,
) -> Result<u64> {
    #[derive(Deserialize)]
    struct Answer {
        revision: u64,
    }
    let code = code_of(code)?;
    let body = json!({
        "manifest": serde_json::from_str::<serde_json::Value>(&manifest.to_json()?)?,
        "base_revision": base_revision,
    });
    let mut resp = agent()
        .put(&url(base, &format!("/{code}")))
        .header(KEY_HEADER, key)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(body)?;
    let status = resp.status().as_u16();
    check(&mut resp, status)?;
    let a: Answer = resp
        .body_mut()
        .with_config()
        .limit(MAX_SMALL_ANSWER)
        .read_json()?;
    Ok(a.revision)
}

/// The newest revision number, without the manifest (cheap to ask often).
pub fn head(base: &str, code: &str) -> Result<u64> {
    #[derive(Deserialize)]
    struct Answer {
        revision: u64,
    }
    let code = code_of(code)?;
    let mut resp = agent()
        .get(&url(base, &format!("/{code}/head")))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    let status = resp.status().as_u16();
    check(&mut resp, status)?;
    let a: Answer = resp
        .body_mut()
        .with_config()
        .limit(MAX_SMALL_ANSWER)
        .read_json()?;
    Ok(a.revision)
}

/// The link's newest manifest, fully checked.
pub fn fetch(base: &str, code: &str) -> Result<Fetched> {
    let code = code_of(code)?;
    let mut resp = agent()
        .get(&url(base, &format!("/{code}")))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    let status = resp.status().as_u16();
    check(&mut resp, status)?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_MANIFEST as u64 + MAX_SMALL_ANSWER)
        .read_to_vec()?;
    parse_fetched(&bytes)
}

/// `{"revision", "updated", "manifest"}` as the server answers a fetch.
pub fn parse_fetched(bytes: &[u8]) -> Result<Fetched> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| Error::Other("the Arctic server sent a damaged link".into()))?;
    let revision = value
        .get("revision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Error::Other("the Arctic server sent a damaged link".into()))?;
    let updated = value
        .get("updated")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let manifest = value
        .get("manifest")
        .ok_or_else(|| Error::Other("the Arctic server sent a damaged link".into()))?;
    let manifest = Manifest::from_json(manifest.to_string().as_bytes())?;
    Ok(Fetched {
        revision,
        updated,
        manifest,
    })
}

/// Stop sharing: the code stops working for everyone.
pub fn delete(base: &str, code: &str, key: &str) -> Result<()> {
    let code = code_of(code)?;
    let mut resp = agent()
        .delete(&url(base, &format!("/{code}")))
        .header(KEY_HEADER, key)
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    let status = resp.status().as_u16();
    if status == 404 || status == 410 {
        return Ok(());
    }
    check(&mut resp, status)
}
