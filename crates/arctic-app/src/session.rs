//! Loading everything that belongs to one profile, and switching profiles.

use std::collections::HashMap;

use arctic_core::auth::AccountStore;
use arctic_core::auth::avatar::{self, Face};
use arctic_core::instances::{self, Instance};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;

use crate::app::{AddAccount, ArcticApp, LaunchState};
use crate::tasks::Tasks;
use crate::toasts::{Kind, Toasts};

/// A profile's data, loaded from its folder.
pub struct ProfileData {
    pub settings: Settings,
    pub accounts: AccountStore,
    pub instance: Instance,
    pub custom_instances: Vec<Instance>,
    pub faces: HashMap<String, Face>,
}

impl ProfileData {
    /// Load (creating folders as needed). Problems become toasts and fall
    /// back to defaults, so a broken file never blocks the launcher.
    pub fn load(dirs: &DataDirs, tasks: &Tasks, toasts: &mut Toasts) -> Self {
        let mut note = |what: &str, e: arctic_core::Error| {
            log::error!("{what}: {e}");
            toasts.push(Kind::Error, what, e.to_string());
        };
        if let Err(e) = dirs.ensure() {
            note("Could not create data folders", e);
        }
        let settings = Settings::load(dirs).unwrap_or_else(|e| {
            note("Settings were unreadable, using defaults", e);
            Settings::default()
        });
        let accounts = AccountStore::load(dirs).unwrap_or_else(|e| {
            note("Accounts file was unreadable", e);
            AccountStore::default()
        });
        let instance = instances::load_default(dirs).unwrap_or_else(|e| {
            note("Could not load the Vanilla instance", e);
            Instance::vanilla_default()
        });
        let custom_instances = instances::list_custom(dirs).unwrap_or_default();
        let faces = load_faces(dirs, &accounts, tasks);
        Self {
            settings,
            accounts,
            instance,
            custom_instances,
            faces,
        }
    }
}

/// Cached avatars for instant display; refresh stale Microsoft ones.
fn load_faces(dirs: &DataDirs, accounts: &AccountStore, tasks: &Tasks) -> HashMap<String, Face> {
    let mut faces = HashMap::new();
    for account in accounts.accounts.iter().filter(|a| a.is_microsoft()) {
        if let Some(face) = avatar::cached_face(dirs, &account.uuid) {
            faces.insert(account.uuid.clone(), face);
        }
        if avatar::needs_refresh(dirs, &account.uuid) {
            tasks.fetch_face(account.uuid.clone());
        }
    }
    faces
}

impl ArcticApp {
    /// Why switching profiles is not possible right now, if it isn't.
    pub(crate) fn profile_switch_blocked(&self) -> Option<&'static str> {
        if !matches!(self.launch, LaunchState::Idle) {
            Some("Close Minecraft before switching profiles.")
        } else if !matches!(self.add_account, AddAccount::Closed) {
            Some("Finish adding the account first.")
        } else {
            None
        }
    }

    /// Save the current profile's state and load another one.
    pub(crate) fn switch_profile(&mut self, id: &str) {
        if let Some(reason) = self.profile_switch_blocked() {
            self.toasts.push(Kind::Info, reason, "");
            return;
        }
        if id == self.profiles.active {
            return;
        }
        self.persist_settings();
        if !self.profiles.set_active(id) {
            return;
        }
        if let Err(e) = self.profiles.save(&self.root) {
            self.toasts
                .push(Kind::Error, "Could not save profiles", e.to_string());
        }
        self.dirs = self.profiles.scoped(&self.root);
        self.tasks = self.tasks.with_dirs(self.dirs.clone());
        let data = ProfileData::load(&self.dirs, &self.tasks, &mut self.toasts);
        self.apply_profile_data(data);
        self.maybe_start_onboarding(false);
        let name = self.profiles.active().name.clone();
        self.toasts
            .push(Kind::Success, format!("Switched to {name}"), "");
    }
}
