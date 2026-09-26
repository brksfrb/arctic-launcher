//! Microsoft account login.
//!
//! Chain: MSA OAuth (device code *or* browser + PKCE) → Xbox Live user token
//! → XSTS token → Minecraft services token → Minecraft profile.
//!
//! Requires an Azure app registration; see `docs/microsoft-auth.md`. The
//! client ID is read from the environment or a local config file and is
//! never committed.

mod browser;
mod config;
mod oauth;
mod xbox;

pub use config::{CLIENT_ID_ENV, MsaConfig};
pub use oauth::{DeviceCode, MsaTokens};

use std::sync::atomic::AtomicBool;

use super::{Account, AccountKind, MicrosoftSession, new_local_id, now_secs};
use crate::{Error, Result};

/// Step 1 of the device-code flow: get a code for the user to enter at
/// `verification_uri`.
pub fn start_device_code(cfg: &MsaConfig) -> Result<DeviceCode> {
    oauth::request_device_code(cfg)
}

/// Step 2 of the device-code flow: poll until the user finishes (or cancels),
/// then complete the Xbox/Minecraft chain.
pub fn finish_device_code(
    cfg: &MsaConfig,
    code: &DeviceCode,
    cancel: &AtomicBool,
) -> Result<Account> {
    let tokens = oauth::poll_device_code(cfg, code, cancel)?;
    complete_login(tokens, cfg.is_live())
}

/// Browser flow: opens the system browser and waits for the loopback redirect.
/// Live-style client IDs can't redirect back to us, so they open the code
/// page with the code already filled in and wait the same way.
pub fn login_with_browser(cfg: &MsaConfig, cancel: &AtomicBool) -> Result<Account> {
    if cfg.is_live() {
        let code = start_device_code(cfg)?;
        let url = format!("{}?otc={}", code.verification_uri, code.user_code);
        open::that_detached(&url)
            .map_err(|e| Error::Other(format!("could not open the browser: {e}")))?;
        return finish_device_code(cfg, &code, cancel);
    }
    let tokens = browser::authorize(cfg, cancel)?;
    complete_login(tokens, false)
}

/// Mint a fresh Minecraft token from the stored MSA refresh token.
pub fn refresh(cfg: &MsaConfig, account: &Account) -> Result<Account> {
    let AccountKind::Microsoft(session) = &account.kind else {
        return Ok(account.clone());
    };
    let tokens = oauth::refresh(cfg, &session.refresh_token.reveal())?;
    let refreshed = complete_login(tokens, cfg.is_live())?;
    Ok(Account {
        id: account.id.clone(),
        ..refreshed
    })
}

fn complete_login(tokens: MsaTokens, live: bool) -> Result<Account> {
    let xbl = xbox::xbox_live(&tokens.access_token, live)?;
    let xsts = xbox::xsts(&xbl.token)?;
    let mc = xbox::minecraft_login(&xsts.user_hash, &xsts.token)?;
    let profile = xbox::profile(&mc.access_token)?;
    let refresh_token = tokens
        .refresh_token
        .ok_or_else(|| Error::Auth("Microsoft did not return a refresh token".into()))?;
    Ok(Account {
        id: new_local_id(),
        username: profile.name,
        uuid: profile.id,
        kind: AccountKind::Microsoft(MicrosoftSession {
            refresh_token: crate::secret::Secret::new(refresh_token),
            access_token: crate::secret::Secret::new(mc.access_token),
            expires_at: now_secs() + mc.expires_in,
            xuid: xsts.xuid,
        }),
    })
}
