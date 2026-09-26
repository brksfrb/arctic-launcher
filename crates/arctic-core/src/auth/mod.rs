//! Accounts: offline and Microsoft, plus the multi-account store.
//!
//! Tokens live only in `accounts.json` inside the local data dir, encrypted
//! for this Windows user (see [`crate::secret`]), and stay encrypted in
//! memory until the moment they're used.

pub mod avatar;
pub mod microsoft;
pub mod offline;

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::secret::Secret;
use crate::storage::{DataDirs, load_json, save_json};

/// Refresh Minecraft tokens this long before they actually expire.
const REFRESH_MARGIN_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// Local, stable id (independent of the Minecraft UUID).
    pub id: String,
    /// In-game name.
    pub username: String,
    /// Minecraft profile UUID, without dashes.
    pub uuid: String,
    pub kind: AccountKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountKind {
    Offline,
    Microsoft(MicrosoftSession),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicrosoftSession {
    /// Microsoft (MSA) refresh token, used to mint new Minecraft tokens.
    pub refresh_token: Secret,
    /// Minecraft services access token passed to the game.
    pub access_token: Secret,
    /// Unix seconds when `access_token` expires.
    pub expires_at: u64,
    pub xuid: Option<String>,
}

/// Everything the launch pipeline needs to know about the player.
#[derive(Debug, Clone)]
pub struct LaunchIdentity {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    /// `msa` or `legacy`.
    pub user_type: &'static str,
    pub xuid: String,
}

impl Account {
    pub fn is_microsoft(&self) -> bool {
        matches!(self.kind, AccountKind::Microsoft(_))
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            AccountKind::Offline => "Offline",
            AccountKind::Microsoft(_) => "Microsoft",
        }
    }

    /// True when a Microsoft token is expired or about to expire.
    pub fn needs_refresh(&self, now: u64) -> bool {
        match &self.kind {
            AccountKind::Offline => false,
            AccountKind::Microsoft(s) => now + REFRESH_MARGIN_SECS >= s.expires_at,
        }
    }

    pub fn identity(&self) -> LaunchIdentity {
        match &self.kind {
            // Offline sessions have no token; the game accepts any placeholder.
            AccountKind::Offline => LaunchIdentity {
                username: self.username.clone(),
                uuid: self.uuid.clone(),
                access_token: "0".into(),
                user_type: "legacy",
                xuid: "0".into(),
            },
            AccountKind::Microsoft(s) => LaunchIdentity {
                username: self.username.clone(),
                uuid: self.uuid.clone(),
                access_token: s.access_token.reveal().to_string(),
                user_type: "msa",
                xuid: s.xuid.clone().unwrap_or_else(|| "0".into()),
            },
        }
    }
}

/// Persisted list of accounts plus the active selection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active: Option<String>,
}

impl AccountStore {
    pub fn load(dirs: &DataDirs) -> Result<Self> {
        let path = dirs.accounts_file();
        let store: Self = load_json(&path)?.unwrap_or_default();
        // Files from before encryption: save them encrypted right away.
        let plain = std::fs::read_to_string(&path)
            .is_ok_and(|text| crate::secret::has_plain(&text, &["refresh_token", "access_token"]));
        if plain {
            store.save(dirs)?;
        }
        Ok(store)
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.accounts_file(), self)
    }

    pub fn active(&self) -> Option<&Account> {
        let id = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Add an account (or replace one with the same kind + UUID, e.g. after
    /// re-login) and make it active.
    pub fn upsert(&mut self, account: Account) {
        let same =
            |a: &Account| a.uuid == account.uuid && a.is_microsoft() == account.is_microsoft();
        let existing_id = self.accounts.iter().find(|a| same(a)).map(|a| a.id.clone());
        let account = match existing_id {
            Some(id) => Account { id, ..account },
            None => account,
        };
        self.active = Some(account.id.clone());
        match self.accounts.iter_mut().find(|a| a.id == account.id) {
            Some(slot) => *slot = account,
            None => self.accounts.push(account),
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.accounts.retain(|a| a.id != id);
        if self.active.as_deref() == Some(id) {
            self.active = self.accounts.first().map(|a| a.id.clone());
        }
    }

    /// Find an account by local id, Minecraft UUID (with or without dashes)
    /// or username (case-insensitive).
    pub fn find(&self, query: &str) -> Option<&Account> {
        let q = query.trim();
        let uuid = q.replace('-', "").to_lowercase();
        self.accounts
            .iter()
            .find(|a| a.id == q || a.uuid.eq_ignore_ascii_case(&uuid))
            .or_else(|| {
                self.accounts
                    .iter()
                    .find(|a| a.username.eq_ignore_ascii_case(q))
            })
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        let exists = self.accounts.iter().any(|a| a.id == id);
        if exists {
            self.active = Some(id.to_owned());
        }
        exists
    }
}

/// Return `account` with a valid Minecraft token, refreshing a Microsoft
/// session when it is (about to be) expired. The bool says whether it
/// changed (and should be saved).
pub fn ensure_fresh(dirs: &DataDirs, account: &Account) -> Result<(Account, bool)> {
    if !account.needs_refresh(now_secs()) {
        return Ok((account.clone(), false));
    }
    let cfg = microsoft::MsaConfig::load(dirs)?;
    Ok((microsoft::refresh(&cfg, account)?, true))
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn new_local_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(uuid: &str, expires_at: u64) -> Account {
        Account {
            id: new_local_id(),
            username: "Steve".into(),
            uuid: uuid.into(),
            kind: AccountKind::Microsoft(MicrosoftSession {
                refresh_token: Secret::new("r"),
                access_token: Secret::new("a"),
                expires_at,
                xuid: None,
            }),
        }
    }

    #[test]
    fn upsert_replaces_same_profile_and_activates() {
        let mut store = AccountStore::default();
        let first = ms("u1", 10);
        let first_id = first.id.clone();
        store.upsert(first);
        store.upsert(ms("u1", 20));
        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.active.as_deref(), Some(first_id.as_str()));
        assert!(matches!(&store.accounts[0].kind, AccountKind::Microsoft(s) if s.expires_at == 20));
    }

    #[test]
    fn offline_and_microsoft_with_same_uuid_coexist() {
        let mut store = AccountStore::default();
        store.upsert(ms("u1", 10));
        store.upsert(offline::create("Steve").unwrap());
        assert_eq!(store.accounts.len(), 2);
    }

    #[test]
    fn remove_active_falls_back_to_first() {
        let mut store = AccountStore::default();
        let a = ms("a", 0);
        let a_id = a.id.clone();
        store.upsert(a);
        let b = ms("b", 0);
        let b_id = b.id.clone();
        store.upsert(b);
        assert!(store.set_active(&b_id));
        store.remove(&b_id);
        assert_eq!(store.active.as_deref(), Some(a_id.as_str()));
        store.remove(&a_id);
        assert!(store.active().is_none());
        assert!(!store.set_active("missing"));
    }

    #[test]
    fn find_by_name_uuid_or_id() {
        let mut store = AccountStore::default();
        let steve = offline::create("Steve").unwrap();
        let dashed = format!(
            "{}-{}-{}-{}-{}",
            &steve.uuid[..8],
            &steve.uuid[8..12],
            &steve.uuid[12..16],
            &steve.uuid[16..20],
            &steve.uuid[20..]
        );
        store.upsert(steve.clone());
        assert_eq!(store.find("steve").map(|a| &a.id), Some(&steve.id));
        assert_eq!(store.find(&dashed).map(|a| &a.id), Some(&steve.id));
        assert_eq!(store.find(&steve.id).map(|a| &a.id), Some(&steve.id));
        assert!(store.find("alex").is_none());
    }

    #[test]
    fn refresh_window() {
        let acc = ms("u", 1_000);
        assert!(acc.needs_refresh(1_000 - 60));
        assert!(!acc.needs_refresh(0));
        assert!(!offline::create("Alex").unwrap().needs_refresh(u64::MAX / 2));
    }

    #[test]
    fn identity_user_types() {
        assert_eq!(
            offline::create("Alex").unwrap().identity().user_type,
            "legacy"
        );
        assert_eq!(ms("u", 0).identity().user_type, "msa");
    }
}
