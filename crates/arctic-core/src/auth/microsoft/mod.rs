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

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

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
///
/// One refresh per account at a time: Microsoft may rotate the refresh token, so a
/// second caller must use the first one's result instead of sending the old token.
pub fn refresh(cfg: &MsaConfig, account: &Account) -> Result<Account> {
    refresh_once(account, |account| {
        // Always Microsoft in builds without offline accounts.
        #[allow(irrefutable_let_patterns)]
        let AccountKind::Microsoft(session) = &account.kind else {
            return Ok(account.clone());
        };
        let tokens = oauth::refresh(cfg, &session.refresh_token.reveal())?;
        let refreshed = complete_login(tokens, cfg.is_live())?;
        Ok(Account {
            id: account.id.clone(),
            ..refreshed
        })
    })
}

type Latest = Arc<Mutex<Option<Account>>>;

/// Per account id: a lock, holding the newest session this run knows
/// (a refreshed one, or the retired one after a dead refresh token).
fn latest(id: &str) -> Latest {
    static SLOTS: OnceLock<Mutex<HashMap<String, Latest>>> = OnceLock::new();
    let mut slots = SLOTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    slots.entry(id.to_owned()).or_default().clone()
}

/// Run `do_refresh` for `account` under its lock, unless a caller that held the lock
/// first already refreshed (or retired) it.
fn refresh_once(
    account: &Account,
    do_refresh: impl FnOnce(&Account) -> Result<Account>,
) -> Result<Account> {
    if account.needs_relogin() {
        return Err(Error::ReauthRequired);
    }
    let slot = latest(&account.id);
    let mut held = slot.lock().unwrap_or_else(PoisonError::into_inner);
    let stale = session_expiry(account);
    // A newer session than the caller's (a later login has a later expiry and wins).
    let account = match &*held {
        Some(known) if known.needs_relogin() && session_expiry(known) >= stale => {
            return Err(Error::ReauthRequired);
        }
        Some(known) if session_expiry(known) > stale => known,
        _ => account,
    };
    if !account.needs_refresh(now_secs()) {
        return Ok(account.clone());
    }
    match do_refresh(account) {
        Ok(fresh) => {
            *held = Some(fresh.clone());
            Ok(fresh)
        }
        Err(e) => {
            if e.is_reauth_required() {
                let mut dead = account.clone();
                dead.retire();
                *held = Some(dead);
            }
            Err(e)
        }
    }
}

fn session_expiry(account: &Account) -> u64 {
    match &account.kind {
        AccountKind::Microsoft(s) => s.expires_at,
        #[cfg(any(feature = "offline-accounts", test))]
        AccountKind::Offline => 0,
    }
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
            relogin: false,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::secret::Secret;

    fn account(id: &str, expires_at: u64, refresh: &str) -> Account {
        Account {
            id: id.into(),
            username: "Steve".into(),
            uuid: "u".into(),
            kind: AccountKind::Microsoft(MicrosoftSession {
                refresh_token: Secret::new(refresh),
                access_token: Secret::new("a"),
                expires_at,
                xuid: None,
                relogin: false,
            }),
        }
    }

    fn refresh_of(account: &Account) -> String {
        match &account.kind {
            AccountKind::Microsoft(s) => s.refresh_token.reveal().to_string(),
            #[cfg(any(feature = "offline-accounts", test))]
            AccountKind::Offline => String::new(),
        }
    }

    #[test]
    fn concurrent_refreshes_hit_the_endpoint_once() {
        let calls = AtomicUsize::new(0);
        let old = account("single-flight", 10, "old");
        let results: Vec<Account> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        refresh_once(&old, |a| {
                            calls.fetch_add(1, Ordering::SeqCst);
                            assert_eq!(refresh_of(a), "old");
                            std::thread::sleep(Duration::from_millis(100));
                            Ok(account("single-flight", now_secs() + 3600, "rotated"))
                        })
                        .unwrap()
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap()).collect()
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(results.iter().all(|a| refresh_of(a) == "rotated"));
    }

    #[test]
    fn a_dead_token_is_not_sent_twice_until_signing_in_again() {
        let calls = AtomicUsize::new(0);
        let old = account("dead-token", 10, "old");
        let dead = |_: &Account| {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(Error::ReauthRequired)
        };
        assert!(refresh_once(&old, dead).unwrap_err().is_reauth_required());
        // A caller still holding the old copy is stopped without a request.
        assert!(refresh_once(&old, dead).unwrap_err().is_reauth_required());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // A new sign-in (later expiry) works again.
        let login = account("dead-token", 5, "new");
        let login = Account {
            kind: match login.kind {
                AccountKind::Microsoft(s) => AccountKind::Microsoft(MicrosoftSession {
                    expires_at: now_secs() + 3600,
                    ..s
                }),
                other => other,
            },
            ..login
        };
        assert_eq!(
            refresh_once(&login, dead).map(|a| refresh_of(&a)).unwrap(),
            "new"
        );
    }

    #[test]
    fn transient_failures_are_not_remembered() {
        let calls = AtomicUsize::new(0);
        let old = account("transient", 10, "old");
        for _ in 0..2 {
            let r = refresh_once(&old, |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(Error::Http("503".into()))
            });
            assert!(!r.unwrap_err().is_reauth_required());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
