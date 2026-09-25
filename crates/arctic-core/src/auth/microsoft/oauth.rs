//! Microsoft identity platform (consumer tenant) OAuth 2.0 calls.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::MsaConfig;
use crate::net::{agent, read_json_any_status};
use crate::{Error, Result};

pub(super) const AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
pub(super) const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// How often the cancel flag is checked while waiting between polls.
const CANCEL_CHECK: Duration = Duration::from_millis(200);
const SLOW_DOWN_EXTRA: Duration = Duration::from_secs(5);

/// Shown to the user during device-code login.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsaTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

/// Either tokens or an OAuth error object — the token endpoint returns
/// `400` with `{"error": "..."}` while device-code login is pending.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TokenResponse {
    Ok(MsaTokens),
    Err {
        error: String,
        error_description: Option<String>,
    },
}

fn post_token_form(form: &[(&str, &str)]) -> Result<TokenResponse> {
    let mut resp = agent()
        .post(TOKEN_URL)
        .config()
        .http_status_as_error(false)
        .build()
        .send_form(form.iter().copied())?;
    read_json_any_status(&mut resp)
}

fn oauth_error(error: &str, description: Option<String>) -> Error {
    Error::Auth(description.unwrap_or_else(|| error.to_owned()))
}

pub(super) fn request_device_code(cfg: &MsaConfig) -> Result<DeviceCode> {
    let mut resp = agent()
        .post(DEVICE_CODE_URL)
        .send_form([("client_id", cfg.client_id.as_str()), ("scope", SCOPE)])?;
    Ok(resp.body_mut().read_json()?)
}

pub(super) fn poll_device_code(
    cfg: &MsaConfig,
    code: &DeviceCode,
    cancel: &AtomicBool,
) -> Result<MsaTokens> {
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.max(1));
    loop {
        sleep_cancellable(interval, cancel)?;
        if Instant::now() >= deadline {
            return Err(Error::Auth(
                "the login code expired, please try again".into(),
            ));
        }
        let form = [
            ("grant_type", DEVICE_GRANT),
            ("client_id", cfg.client_id.as_str()),
            ("device_code", code.device_code.as_str()),
        ];
        match post_token_form(&form)? {
            TokenResponse::Ok(tokens) => return Ok(tokens),
            TokenResponse::Err { error, .. } if error == "authorization_pending" => {}
            TokenResponse::Err { error, .. } if error == "slow_down" => interval += SLOW_DOWN_EXTRA,
            TokenResponse::Err {
                error,
                error_description,
            } => {
                return Err(oauth_error(&error, error_description));
            }
        }
    }
}

pub(super) fn exchange_code(
    cfg: &MsaConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<MsaTokens> {
    let form = [
        ("grant_type", "authorization_code"),
        ("client_id", cfg.client_id.as_str()),
        ("scope", SCOPE),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ];
    match post_token_form(&form)? {
        TokenResponse::Ok(tokens) => Ok(tokens),
        TokenResponse::Err {
            error,
            error_description,
        } => Err(oauth_error(&error, error_description)),
    }
}

pub(super) fn refresh(cfg: &MsaConfig, refresh_token: &str) -> Result<MsaTokens> {
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", cfg.client_id.as_str()),
        ("scope", SCOPE),
        ("refresh_token", refresh_token),
    ];
    match post_token_form(&form)? {
        TokenResponse::Ok(tokens) => Ok(tokens),
        TokenResponse::Err {
            error,
            error_description,
        } => Err(oauth_error(&error, error_description)),
    }
}

pub(super) fn sleep_cancellable(total: Duration, cancel: &AtomicBool) -> Result<()> {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Auth("login cancelled".into()));
        }
        std::thread::sleep(CANCEL_CHECK.min(end.saturating_duration_since(Instant::now())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pending_error_and_tokens() {
        let pending: TokenResponse =
            serde_json::from_str(r#"{"error":"authorization_pending","error_description":"x"}"#)
                .unwrap();
        assert!(
            matches!(pending, TokenResponse::Err { error, .. } if error == "authorization_pending")
        );
        let ok: TokenResponse = serde_json::from_str(
            r#"{"access_token":"a","refresh_token":"r","expires_in":3600,"token_type":"Bearer"}"#,
        )
        .unwrap();
        assert!(matches!(ok, TokenResponse::Ok(t) if t.refresh_token.as_deref() == Some("r")));
    }

    #[test]
    fn cancelled_sleep_errors() {
        let cancel = AtomicBool::new(true);
        assert!(sleep_cancellable(Duration::from_secs(5), &cancel).is_err());
    }
}
