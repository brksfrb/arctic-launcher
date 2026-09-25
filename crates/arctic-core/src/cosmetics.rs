//! Client for the Arctic cosmetics server (see the `arctic-cosmetics`
//! crate). Sign-in proves account ownership through Mojang's session
//! server, the same handshake a Minecraft server uses.

use serde::Deserialize;

use crate::net::agent;
use crate::{Error, Result};

pub const DEFAULT_URL: &str = "https://cosmetics.arcticlauncher.com";
const JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Server to use: `ARCTIC_COSMETICS_URL` or the default.
pub fn base_url() -> String {
    std::env::var(crate::arctic_mod::COSMETICS_URL_ENV)
        .ok()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Me {
    pub owned: Vec<String>,
    pub equipped: Equipped,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Equipped {
    pub cape: Option<String>,
}

pub fn catalog(base: &str) -> Result<Vec<CatalogItem>> {
    get(&format!("{base}/v1/catalog"), None)
}

/// A cosmetic's texture (PNG).
pub fn texture(base: &str, id: &str) -> Result<Vec<u8>> {
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(Error::Other("bad cosmetic id".into()));
    }
    let mut resp = agent()
        .get(&format!("{base}/v1/textures/{id}.png"))
        .call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()?)
}

/// Sign in with a Minecraft access token; returns a cosmetics session token.
pub fn sign_in(base: &str, access_token: &str, uuid: &str, name: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Challenge {
        server_id: String,
    }
    #[derive(Deserialize)]
    struct Session {
        token: String,
    }
    let challenge: Challenge = post(&format!("{base}/v1/auth/challenge"), &serde_json::json!({}))?;
    let joined = agent()
        .post(JOIN_URL)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({
            "accessToken": access_token,
            "selectedProfile": uuid.replace('-', ""),
            "serverId": challenge.server_id,
        }))?;
    if !joined.status().is_success() {
        return Err(Error::Auth(format!(
            "Mojang refused the session ({})",
            joined.status().as_u16()
        )));
    }
    let session: Session = post(
        &format!("{base}/v1/auth/verify"),
        &serde_json::json!({ "name": name, "server_id": challenge.server_id }),
    )?;
    Ok(session.token)
}

pub fn me(base: &str, token: &str) -> Result<Me> {
    get(&format!("{base}/v1/me"), Some(token))
}

/// Equip a cape, or none.
pub fn equip(base: &str, token: &str, cape: Option<&str>) -> Result<()> {
    let resp = agent()
        .put(&format!("{base}/v1/me/equipped"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({ "cape": cape }))?;
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        403 => Err(Error::Other("You don't own that cape.".into())),
        s => Err(Error::Other(format!("The Arctic server returned {s}."))),
    }
}

fn get<T: serde::de::DeserializeOwned>(url: &str, token: Option<&str>) -> Result<T> {
    let mut req = agent().get(url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let mut resp = req.call()?;
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

fn post<T: serde::de::DeserializeOwned>(url: &str, body: &serde_json::Value) -> Result<T> {
    let mut resp = agent().post(url).send_json(body)?;
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_me() {
        let me: Me = serde_json::from_str(
            r#"{"uuid":"x","owned":["aurora","glacier"],"equipped":{"cape":"aurora"}}"#,
        )
        .unwrap();
        assert_eq!(me.equipped.cape.as_deref(), Some("aurora"));
        let none: Me = serde_json::from_str(r#"{"owned":[],"equipped":{"cape":null}}"#).unwrap();
        assert_eq!(none.equipped.cape, None);
    }

    #[test]
    fn rejects_odd_ids() {
        assert!(texture("http://127.0.0.1:1", "../x").is_err());
    }
}
