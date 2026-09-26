//! Xbox Live → XSTS → Minecraft services.

use serde::Deserialize;
use serde_json::json;

use crate::net::{agent, read_json_any_status};
use crate::{Error, Result};

const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

pub(super) struct XboxToken {
    pub token: String,
    pub user_hash: String,
    pub xuid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
    xid: Option<String>,
}

#[derive(Deserialize)]
struct XstsError {
    #[serde(rename = "XErr")]
    xerr: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct McLogin {
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
pub(super) struct McProfile {
    pub id: String,
    pub name: String,
}

impl TryFrom<XboxResponse> for XboxToken {
    type Error = Error;
    fn try_from(r: XboxResponse) -> Result<Self> {
        let xui = r
            .display_claims
            .xui
            .into_iter()
            .next()
            .ok_or_else(|| Error::Auth("Xbox response had no user claims".into()))?;
        Ok(Self {
            token: r.token,
            user_hash: xui.uhs,
            xuid: xui.xid,
        })
    }
}

/// Xbox Live user token. Identity-platform tokens are sent as `d=<token>`;
/// login.live.com tokens use `t=`, with the other forms as fallbacks.
pub(super) fn xbox_live(msa_access_token: &str, live: bool) -> Result<XboxToken> {
    let prefixes: &[&str] = if live { &["t=", "d=", ""] } else { &["d="] };
    let mut last = None;
    for prefix in prefixes {
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("{prefix}{msa_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let mut resp = agent()
            .post(XBL_URL)
            .header("Accept", "application/json")
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(&body)?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            let parsed: XboxResponse = resp.body_mut().read_json()?;
            return parsed.try_into();
        }
        last = Some(status);
    }
    Err(Error::Auth(format!(
        "Xbox Live sign-in failed (HTTP {})",
        last.unwrap_or(0)
    )))
}

pub(super) fn xsts(xbl_token: &str) -> Result<XboxToken> {
    let body = json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });
    let mut resp = agent()
        .post(XSTS_URL)
        .header("Accept", "application/json")
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(&body)?;
    if resp.status() == 401 {
        let err: XstsError = read_json_any_status(&mut resp).unwrap_or(XstsError { xerr: None });
        return Err(Error::Auth(xsts_error_message(err.xerr)));
    }
    if !resp.status().is_success() {
        return Err(Error::Auth(format!(
            "XSTS authorization failed (HTTP {})",
            resp.status()
        )));
    }
    let parsed: XboxResponse = read_json_any_status(&mut resp)?;
    parsed.try_into()
}

fn xsts_error_message(xerr: Option<u64>) -> String {
    match xerr {
        Some(2148916233) => {
            "This Microsoft account has no Xbox profile. Sign in once at xbox.com to create one."
                .into()
        }
        Some(2148916235) => "Xbox Live is not available in your country.".into(),
        Some(2148916236 | 2148916237) => {
            "This account needs adult verification (South Korea).".into()
        }
        Some(2148916238) => {
            "This is a child account; an adult must add it to a Microsoft family.".into()
        }
        Some(code) => format!("Xbox authorization failed (XErr {code})."),
        None => "Xbox authorization failed.".into(),
    }
}

pub(super) fn minecraft_login(user_hash: &str, xsts_token: &str) -> Result<McLogin> {
    let body = json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") });
    let mut resp = agent()
        .post(MC_LOGIN_URL)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(&body)?;
    match resp.status().as_u16() {
        200 => read_json_any_status(&mut resp),
        403 => Err(Error::Auth(
            "Minecraft services rejected this app. New Azure app IDs must be approved by Mojang \
             (see docs/microsoft-auth.md)."
                .into(),
        )),
        code => Err(Error::Auth(format!("Minecraft login failed (HTTP {code})"))),
    }
}

pub(super) fn profile(mc_access_token: &str) -> Result<McProfile> {
    let mut resp = agent()
        .get(MC_PROFILE_URL)
        .header("Authorization", format!("Bearer {mc_access_token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    match resp.status().as_u16() {
        200 => read_json_any_status(&mut resp),
        404 => Err(Error::Auth(
            "This account does not own Minecraft: Java Edition.".into(),
        )),
        code => Err(Error::Auth(format!(
            "could not fetch Minecraft profile (HTTP {code})"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xbox_response() {
        let r: XboxResponse = serde_json::from_str(
            r#"{"IssueInstant":"x","NotAfter":"y","Token":"tok","DisplayClaims":{"xui":[{"uhs":"hash"}]}}"#,
        )
        .unwrap();
        let t: XboxToken = r.try_into().unwrap();
        assert_eq!((t.token.as_str(), t.user_hash.as_str()), ("tok", "hash"));
        assert!(t.xuid.is_none());
    }

    #[test]
    fn friendly_xsts_errors() {
        assert!(xsts_error_message(Some(2148916238)).contains("child"));
        assert!(xsts_error_message(Some(1)).contains("XErr 1"));
    }
}
