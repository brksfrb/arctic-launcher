//! Minecraft services API for the signed-in player's skin and capes, and
//! public lookups of other players' skins.
//!
//! All `token` arguments are the Minecraft access token of a Microsoft
//! account (see `auth::ensure_fresh`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

use super::{MAX_SKIN_BYTES, Variant};
use crate::net::agent;
use crate::{Error, Result};

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const NAME_LOOKUP_URL: &str = "https://api.mojang.com/users/profiles/minecraft";
const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
const MAX_JSON_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<ProfileSkin>,
    #[serde(default)]
    pub capes: Vec<ProfileCape>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProfileSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProfileCape {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub alias: String,
}

impl Profile {
    pub fn active_skin(&self) -> Option<&ProfileSkin> {
        self.skins.iter().find(|s| s.state == "ACTIVE")
    }

    pub fn active_cape(&self) -> Option<&ProfileCape> {
        self.capes.iter().find(|c| c.state == "ACTIVE")
    }
}

impl ProfileSkin {
    pub fn variant(&self) -> Variant {
        if self.variant.eq_ignore_ascii_case("slim") {
            Variant::Slim
        } else {
            Variant::Classic
        }
    }
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(default, rename = "errorMessage")]
    error_message: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// The signed-in player's profile, skins and capes.
pub fn profile(token: &str) -> Result<Profile> {
    let resp = agent()
        .get(PROFILE_URL)
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    read_profile(resp)
}

/// Upload a skin PNG and make it active.
pub fn upload_skin(token: &str, variant: Variant, png: &[u8]) -> Result<Profile> {
    super::decode(png)?;
    let boundary = format!("----arctic{}", uuid::Uuid::new_v4().simple());
    let body = multipart(&boundary, variant, png);
    let resp = agent()
        .post(&format!("{PROFILE_URL}/skins"))
        .header("Authorization", &format!("Bearer {token}"))
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .config()
        .http_status_as_error(false)
        .build()
        .send(&body[..])?;
    read_profile(resp)
}

/// Go back to the default skin.
pub fn reset_skin(token: &str) -> Result<Profile> {
    let resp = agent()
        .delete(&format!("{PROFILE_URL}/skins/active"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    read_profile(resp)
}

/// Show one of the player's capes, or hide capes with `None`.
pub fn set_cape(token: &str, cape_id: Option<&str>) -> Result<Profile> {
    let url = format!("{PROFILE_URL}/capes/active");
    let auth = format!("Bearer {token}");
    let resp = match cape_id {
        Some(id) => agent()
            .put(&url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(serde_json::json!({ "capeId": id }))?,
        None => agent()
            .delete(&url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .call()?,
    };
    read_profile(resp)
}

/// Download a texture (skin or cape PNG) from Mojang's texture server.
pub fn texture(url: &str) -> Result<Vec<u8>> {
    if !url.starts_with("http://textures.minecraft.net/")
        && !url.starts_with("https://textures.minecraft.net/")
    {
        return Err(Error::Other("unexpected texture host".into()));
    }
    // Published as http://; the host supports https.
    let url = url.replacen("http://", "https://", 1);
    let mut resp = agent().get(&url).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_SKIN_BYTES as u64)
        .read_to_vec()?)
}

/// Any player's current skin by in-game name (public API, no sign-in).
pub fn player_skin(username: &str) -> Result<(Vec<u8>, Variant)> {
    let name = username.trim();
    if name.is_empty()
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::Other("That isn't a valid Minecraft name.".into()));
    }
    #[derive(Deserialize)]
    struct Lookup {
        id: String,
    }
    let resp = agent()
        .get(&format!("{NAME_LOOKUP_URL}/{name}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    if resp.status().as_u16() == 404 || resp.status().as_u16() == 204 {
        return Err(Error::Other(format!("No player named {name}.")));
    }
    let lookup: Lookup = read_ok(resp)?;
    let (url, variant) = session_skin(&lookup.id)?
        .ok_or_else(|| Error::Other(format!("{name} uses a default skin.")))?;
    Ok((texture(&url)?, variant))
}

/// Skin URL and model from the public session profile.
fn session_skin(uuid: &str) -> Result<Option<(String, Variant)>> {
    #[derive(Deserialize)]
    struct Session {
        properties: Vec<Property>,
    }
    #[derive(Deserialize)]
    struct Property {
        name: String,
        value: String,
    }
    #[derive(Deserialize)]
    struct Payload {
        textures: Textures,
    }
    #[derive(Deserialize)]
    struct Textures {
        #[serde(rename = "SKIN")]
        skin: Option<Skin>,
    }
    #[derive(Deserialize)]
    struct Skin {
        url: String,
        metadata: Option<Metadata>,
    }
    #[derive(Deserialize)]
    struct Metadata {
        model: Option<String>,
    }
    let session: Session = crate::net::get_json(&format!("{SESSION_URL}/{uuid}"))?;
    let Some(prop) = session.properties.iter().find(|p| p.name == "textures") else {
        return Ok(None);
    };
    let decoded = STANDARD
        .decode(&prop.value)
        .map_err(|e| Error::Other(format!("bad textures payload: {e}")))?;
    let payload: Payload = serde_json::from_slice(&decoded)?;
    Ok(payload.textures.skin.map(|s| {
        let slim = s
            .metadata
            .and_then(|m| m.model)
            .is_some_and(|m| m == "slim");
        (
            s.url,
            if slim {
                Variant::Slim
            } else {
                Variant::Classic
            },
        )
    }))
}

fn multipart(boundary: &str, variant: Variant, png: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(png.len() + 512);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{}\r\n",
            variant.api_name()
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn read_profile(resp: ureq::http::Response<ureq::Body>) -> Result<Profile> {
    read_ok(resp)
}

/// Parse a successful JSON response, or turn an error status into a
/// message people can act on.
fn read_ok<T: serde::de::DeserializeOwned>(
    mut resp: ureq::http::Response<ureq::Body>,
) -> Result<T> {
    let status = resp.status().as_u16();
    let body = resp
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()?;
    if (200..300).contains(&status) {
        return Ok(serde_json::from_slice(&body)?);
    }
    let detail = serde_json::from_slice::<ApiError>(&body)
        .ok()
        .and_then(|e| e.error_message.or(e.error))
        .unwrap_or_default();
    Err(Error::Other(status_message(status, &detail)))
}

fn status_message(status: u16, detail: &str) -> String {
    match status {
        401 => "Your sign-in expired. Sign in to this account again.".into(),
        403 => "This account can't change skins (does it own Minecraft?).".into(),
        404 => "This account has no Minecraft profile yet.".into(),
        429 => "Too many changes at once. Wait a minute and try again.".into(),
        400 if !detail.is_empty() => format!("Mojang rejected the skin: {detail}"),
        _ if !detail.is_empty() => format!("Mojang returned {status}: {detail}"),
        _ => format!("Mojang returned an error ({status})."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_parses_and_finds_active_items() {
        let json = r#"{"id":"abc","name":"Steve",
            "skins":[{"id":"s1","state":"ACTIVE","url":"http://textures.minecraft.net/texture/1","variant":"SLIM","alias":"x"}],
            "capes":[{"id":"c1","state":"INACTIVE","url":"u","alias":"Migrator"},{"id":"c2","state":"ACTIVE","url":"u2","alias":"Vanilla"}]}"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.active_skin().map(|s| s.variant()), Some(Variant::Slim));
        assert_eq!(p.active_cape().map(|c| c.alias.as_str()), Some("Vanilla"));
        let bare: Profile = serde_json::from_str(r#"{"id":"a","name":"b"}"#).unwrap();
        assert!(bare.active_skin().is_none());
    }

    #[test]
    fn multipart_body_has_both_fields() {
        let body = multipart("B", Variant::Slim, b"PNGDATA");
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with("--B\r\n"));
        assert!(text.contains("name=\"variant\"\r\n\r\nslim\r\n"));
        assert!(text.contains(
            "filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\nPNGDATA\r\n--B--\r\n"
        ));
    }

    #[test]
    fn errors_are_readable() {
        assert!(status_message(401, "").contains("Sign in"));
        assert!(status_message(400, "Invalid skin").contains("Invalid skin"));
        assert_eq!(status_message(500, ""), "Mojang returned an error (500).");
    }

    #[test]
    fn texture_host_is_restricted() {
        assert!(texture("https://evil.example/x.png").is_err());
    }
}
