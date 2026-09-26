//! The launcher side of the in-game account switcher (see
//! `arctic_core::bridge`): answers the running game with this profile's
//! accounts, refreshing logins through the same task path as launches so
//! the app's account list stays the one source of truth.

use std::sync::{Arc, Mutex, PoisonError};

use arctic_core::auth::{Account, AccountKind, AccountStore};
use arctic_core::bridge::{self, AccountEntry, BridgeInfo, SessionGrant};
use arctic_core::cosmetics;

use crate::tasks::Tasks;

struct Shared {
    accounts: Vec<Account>,
    active: Option<String>,
    tasks: Tasks,
}

pub struct BridgeHost {
    pub info: Option<Arc<BridgeInfo>>,
    shared: Arc<Mutex<Shared>>,
}

struct AppAccounts(Arc<Mutex<Shared>>);

impl bridge::Accounts for AppAccounts {
    fn list(&self) -> Vec<AccountEntry> {
        let shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        shared
            .accounts
            .iter()
            .map(|a| AccountEntry {
                id: a.id.clone(),
                name: a.username.clone(),
                uuid: a.uuid.clone(),
                microsoft: matches!(a.kind, AccountKind::Microsoft(_)),
                active: shared.active.as_deref() == Some(a.id.as_str()),
            })
            .collect()
    }

    fn session(&self, id: &str) -> arctic_core::Result<SessionGrant> {
        let (account, tasks) = {
            let shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let account = shared
                .accounts
                .iter()
                .find(|a| a.id == id)
                .cloned()
                .ok_or_else(|| arctic_core::Error::Other("no such account".into()))?;
            (account, shared.tasks.clone())
        };
        let fresh = tasks.fresh_account(account)?;
        let arctic_token = cosmetics::token_for(tasks.dirs(), &cosmetics::base_url(), &fresh)
            .inspect_err(|e| log::info!("Arctic session for the switch: {e}"))
            .ok();
        Ok(SessionGrant {
            identity: fresh.identity(),
            arctic_token,
        })
    }

    fn add(&self) -> arctic_core::Result<()> {
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        tasks.send(crate::tasks::Event::AddAccountFromGame);
        Ok(())
    }
}

impl BridgeHost {
    /// Start listening (a failure just means no in-game switching).
    pub fn start(tasks: &Tasks) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            accounts: Vec::new(),
            active: None,
            tasks: tasks.clone(),
        }));
        let info = bridge::start(AppAccounts(Arc::clone(&shared)))
            .inspect_err(|e| log::warn!("account switcher bridge: {e}"))
            .ok()
            .map(Arc::new);
        Self { info, shared }
    }

    /// The accounts (and tasks, after a profile switch) to answer with.
    pub fn update(&self, store: &AccountStore, tasks: &Tasks) {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared.accounts = store.accounts.clone();
        shared.active = store.active.clone();
        shared.tasks = tasks.clone();
    }
}
