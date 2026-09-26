//! Client for the Arctic cosmetics server (see the `arctic-cosmetics`
//! crate). A player's look (skin, model, cape) is chosen locally and
//! published; every Arctic client then shows it. Nothing is locked: the
//! server only checks who is publishing, so nobody can change someone
//! else's look. Microsoft accounts prove it through Mojang's session server
//! (like a Minecraft server does); offline names are claimed with a key the
//! launcher keeps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

use crate::auth::{Account, AccountKind, now_secs};
use crate::net::agent;
use crate::skins::Variant;
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

pub const DEFAULT_URL: &str = "https://cosmetics.arcticlauncher.com";
const JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const CREDENTIALS_FILE: &str = "cosmetics.json";
/// Reuse a session token until it's this close to expiring.
const TOKEN_MARGIN_SECS: u64 = 3600;

/// Server to use: `ARCTIC_COSMETICS_URL` or the default.
pub fn base_url() -> String {
    std::env::var(crate::arctic_mod::COSMETICS_URL_ENV)
        .ok()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// A cape offered to everyone.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Texture hash.
    pub texture: String,
}

/// A published look; textures are hashes (`texture()` fetches them).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Look {
    pub skin: Option<String>,
    #[serde(default)]
    pub model: String,
    pub cape: Option<String>,
}

impl Look {
    pub fn variant(&self) -> Variant {
        if self.model == "slim" {
            Variant::Slim
        } else {
            Variant::Classic
        }
    }
}

/// A texture to publish: a new image, or one the server already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Texture {
    Png(Vec<u8>),
    Hash(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapeChoice {
    Preset(String),
    Custom(Texture),
}

/// What to publish. `None` parts are removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewLook {
    pub skin: Option<(Texture, Variant)>,
    pub cape: Option<CapeChoice>,
}

impl NewLook {
    /// The look as currently published (references stored textures).
    pub fn from_look(look: &Look, presets: &[Preset]) -> Self {
        let skin = look
            .skin
            .clone()
            .map(|h| (Texture::Hash(h), look.variant()));
        let cape =
            look.cape
                .as_deref()
                .map(|hash| match presets.iter().find(|p| p.texture == hash) {
                    Some(p) => CapeChoice::Preset(p.id.clone()),
                    None => CapeChoice::Custom(Texture::Hash(hash.to_owned())),
                });
        Self { skin, cape }
    }
}

fn texture_json(texture: &Texture) -> serde_json::Value {
    match texture {
        Texture::Png(png) => serde_json::json!({ "png": STANDARD.encode(png) }),
        Texture::Hash(hash) => serde_json::json!({ "hash": hash }),
    }
}

pub fn catalog(base: &str) -> Result<Vec<Preset>> {
    get(&format!("{base}/v1/catalog"), None)
}

/// A texture by hash (PNG).
pub fn texture(base: &str, hash: &str) -> Result<Vec<u8>> {
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Other("bad texture hash".into()));
    }
    let mut resp = agent()
        .get(&format!("{base}/v1/textures/{hash}.png"))
        .call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()?)
}

pub fn my_look(base: &str, token: &str) -> Result<Look> {
    get(&format!("{base}/v1/look"), Some(token))
}

/// Replace the published look.
pub fn set_look(base: &str, token: &str, look: &NewLook) -> Result<Look> {
    let skin = look.skin.as_ref().map(|(texture, variant)| {
        let mut part = texture_json(texture);
        part["model"] = variant.api_name().into();
        part
    });
    let cape = look.cape.as_ref().map(|c| match c {
        CapeChoice::Preset(id) => serde_json::json!({ "preset": id }),
        CapeChoice::Custom(texture) => texture_json(texture),
    });
    let mut resp = agent()
        .put(&format!("{base}/v1/look"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({ "skin": skin, "cape": cape }))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

fn server_error(resp: &mut ureq::http::Response<ureq::Body>, status: u16) -> String {
    #[derive(Deserialize)]
    struct Body {
        error: String,
    }
    let detail = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json::<Body>()
        .map(|b| b.error)
        .unwrap_or_default();
    match (status, detail.is_empty()) {
        (_, false) => format!("Arctic: {detail}"),
        (429, true) => "Too many changes at once. Wait a minute and try again.".into(),
        _ => format!("The Arctic server returned {status}."),
    }
}

// ---- Sign-in ------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Credential {
    /// Offline accounts: the key that claims the name on the server.
    key: Option<String>,
    token: Option<String>,
    #[serde(default)]
    expires: u64,
}

/// Per-profile store of cosmetics keys and tokens (`cosmetics.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Credentials {
    accounts: HashMap<String, Credential>,
}

fn credentials_path(dirs: &DataDirs) -> PathBuf {
    dirs.profile_root().join(CREDENTIALS_FILE)
}

/// A session token for `account`, reusing a stored one while it's valid.
pub fn token_for(dirs: &DataDirs, base: &str, account: &Account) -> Result<String> {
    let path = credentials_path(dirs);
    let mut creds: Credentials = load_json(&path)?.unwrap_or_default();
    let entry = creds.accounts.entry(account.uuid.clone()).or_default();
    if let Some(token) = &entry.token
        && entry.expires > now_secs() + TOKEN_MARGIN_SECS
    {
        return Ok(token.clone());
    }
    let session = match &account.kind {
        AccountKind::Microsoft(ms) => {
            sign_in_microsoft(base, &ms.access_token, &account.uuid, &account.username)?
        }
        AccountKind::Offline => {
            let key = entry
                .key
                .get_or_insert_with(|| {
                    uuid::Uuid::new_v4().simple().to_string()
                        + &uuid::Uuid::new_v4().simple().to_string()
                })
                .clone();
            // Save the key before using it, so a claimed name is never lost.
            save_json(&path, &creds)?;
            let session = sign_in_offline(base, &account.username, &key)?;
            creds = load_json(&path)?.unwrap_or_default();
            session
        }
    };
    let entry = creds.accounts.entry(account.uuid.clone()).or_default();
    entry.token = Some(session.token.clone());
    entry.expires = session.expires;
    save_json(&path, &creds)?;
    Ok(session.token)
}

#[derive(Deserialize)]
struct Session {
    token: String,
    expires: u64,
}

fn sign_in_microsoft(base: &str, access_token: &str, uuid: &str, name: &str) -> Result<Session> {
    #[derive(Deserialize)]
    struct Challenge {
        server_id: String,
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
    post(
        &format!("{base}/v1/auth/verify"),
        &serde_json::json!({ "name": name, "server_id": challenge.server_id }),
    )
}

fn sign_in_offline(base: &str, name: &str, key: &str) -> Result<Session> {
    let mut resp = agent()
        .post(&format!("{base}/v1/auth/offline"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({ "name": name, "key": key }))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

/// Tell the Arctic mod in `game_dir` which session to use, so looks picked
/// in game are published as this player.
pub fn write_mod_session(game_dir: &Path, base: &str, token: &str) -> Result<()> {
    let path = game_dir.join("config").join("arctic-session.json");
    save_json(&path, &serde_json::json!({ "url": base, "token": token }))
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
    fn parses_looks() {
        let look: Look =
            serde_json::from_str(r#"{"skin":"ab","model":"slim","cape":null}"#).unwrap();
        assert_eq!(look.variant(), Variant::Slim);
        assert_eq!(look.cape, None);
        let empty: Look = serde_json::from_str(r#"{"skin":null,"cape":null}"#).unwrap();
        assert_eq!(empty.variant(), Variant::Classic);
    }

    #[test]
    fn rejects_odd_hashes() {
        assert!(texture("http://127.0.0.1:1", "../x").is_err());
    }
}
