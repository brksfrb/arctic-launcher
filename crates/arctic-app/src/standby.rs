//! "Ready to play": the game of the selected instance loads in the
//! background with its window hidden, so pressing Play only has to show it.
//! It waits for the player's Play; anything that would make it the wrong
//! game (another instance or version, account, settings, mods) throws it
//! away and starts a fresh one.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use arctic_core::auth::Account;
use arctic_core::instances::Instance;
use arctic_core::versions::VersionEntry;
use eframe::egui;

use crate::app::{ArcticApp, LogSource, ManifestState};
use crate::runs::RunState;
use crate::tasks::{LaunchId, StartAt};

/// How often the background game is looked after (seconds).
const CHECK_EVERY: f64 = 2.0;
/// Older than this and it's replaced while the launcher is idle: its sign-in goes stale.
const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);
/// A background game that dies this soon after starting counts as a failure.
pub(crate) const EARLY_EXIT: Duration = Duration::from_secs(25);
/// After this many failures no more are tried (until the launcher restarts).
const MAX_FAILURES: u32 = 2;
/// Settings that don't change the game: the launcher's looks and habits.
const NOT_THE_GAME: &[&str] = &[
    "theme",
    "backdrop",
    "animations",
    "intro",
    "start_maximized",
    "on_game_start",
    "discord_presence",
    "onboarded",
    "tray",
    "favorite_versions",
    "update_channel",
    "check_updates_on_start",
    "crash_reports",
    "keep_ready",
    "last_version",
    "show_snapshots",
    "show_old_versions",
];

impl ArcticApp {
    /// Start, replace or drop the background game, as the selected instance and settings ask.
    pub(crate) fn manage_standby(&mut self, ctx: &egui::Context) {
        if !cfg!(windows) {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.standby_check_at < CHECK_EVERY {
            return;
        }
        self.standby_check_at = now;
        ctx.request_repaint_after(Duration::from_secs_f64(CHECK_EVERY));

        let wanted = self.standby_target();
        let have = self.runs.standby().map(|r| {
            let old = r.started_ago().is_some_and(|age| age > MAX_AGE);
            (r.standby_key, old)
        });
        match (have, wanted) {
            // Nothing to keep ready, nothing running: fine.
            (None, None) => {}
            // The wrong game, or a stale one: drop it (a fresh one follows on a later check).
            (Some(_), None) => self.stop_standby(),
            (Some((key, old)), Some((_, _, _, wanted_key)))
                if key != wanted_key || (old && !self.runs.any_active()) =>
            {
                self.stop_standby();
            }
            (Some(_), Some(_)) => {}
            (None, Some((instance, version, account, key))) => {
                self.start_standby(instance, version, account, key);
            }
        }
    }

    /// What should be loaded in the background now, if anything: the
    /// selected instance, when nothing else is running and it's ready to go.
    fn standby_target(&self) -> Option<(Instance, VersionEntry, Account, u64)> {
        let s = &self.settings;
        // Fullscreen would flash a mode change; a game running already needs the memory.
        if !s.keep_ready
            || s.fullscreen
            || self.standby_fails >= MAX_FAILURES
            || self.runs.any_active()
        {
            return None;
        }
        let ManifestState::Ready(manifest) = &self.manifest else {
            return None;
        };
        let instance = self.selected_instance().clone();
        let version_id = if instance.is_default() {
            s.last_version.clone()
        } else {
            instance.version.clone()
        }?;
        let version = manifest.find(&version_id)?.clone();
        // Only what's installed: a download would show up as a launch nobody asked for.
        if !arctic_core::versions::installed_versions(&self.dirs).contains(&version.id) {
            return None;
        }
        let account = self.accounts.active()?.clone();
        let key = self.standby_key(&instance, &version.id, &account);
        Some((instance, version, account, key))
    }

    /// Hash of everything that makes a background game the right one.
    fn standby_key(&self, instance: &Instance, version_id: &str, account: &Account) -> u64 {
        let mut hasher = DefaultHasher::new();
        serde_json::to_string(instance)
            .unwrap_or_default()
            .hash(&mut hasher);
        version_id.hash(&mut hasher);
        account.id.hash(&mut hasher);
        if let Ok(serde_json::Value::Object(mut settings)) = serde_json::to_value(&self.settings) {
            for key in NOT_THE_GAME {
                settings.remove(*key);
            }
            serde_json::Value::Object(settings)
                .to_string()
                .hash(&mut hasher);
        }
        serde_json::to_string(&arctic_core::proxy::ProxySettings::load(&self.dirs))
            .unwrap_or_default()
            .hash(&mut hasher);
        mods_stamp(&instance.game_dir(&self.dirs).join("mods")).hash(&mut hasher);
        hasher.finish()
    }

    fn start_standby(
        &mut self,
        instance: Instance,
        version: VersionEntry,
        account: Account,
        key: u64,
    ) {
        let id = self.runs.start(
            &instance.id,
            &version.id,
            format!("{} · {}", instance.name, version.id),
            instance.game_dir(&self.dirs),
        );
        if let Some(run) = self.runs.get_mut(id) {
            run.standby = true;
            run.standby_key = key;
        }
        let start = StartAt {
            copy: 0,
            quick_play: None,
        };
        self.tasks
            .launch(id, version, instance, account, self.settings.clone(), start);
    }

    /// The background game, if it's the one Play would start.
    pub(crate) fn standby_match(
        &self,
        instance: &Instance,
        version_id: &str,
        account: &Account,
    ) -> Option<LaunchId> {
        let run = self.runs.standby()?;
        (run.standby_key == self.standby_key(instance, version_id, account)).then_some(run.id)
    }

    /// Play was pressed and the background game is the right one: show it.
    pub(crate) fn activate_standby(
        &mut self,
        id: LaunchId,
        instance: &Instance,
        version: &VersionEntry,
        account: &Account,
    ) {
        self.discord.game_started(
            format!("Minecraft {}", version.id),
            instance.loader.label().to_owned(),
        );
        self.run_note(
            id,
            format!(
                "──── {} ({}) was ready: showing it as {} ────",
                instance.name, version.id, account.username
            ),
        );
        let mut window_up = false;
        if let Some(run) = self.runs.get_mut(id) {
            run.standby = false;
            if let Some(game) = run.game() {
                game.reveal();
            }
            window_up = matches!(run.state, RunState::Running { .. });
        }
        self.log_source = LogSource::Game(id);
        // The window opened while hidden, so its "window is up" came and went unseen.
        if window_up
            && self.settings.on_game_start == arctic_core::settings::GameStartAction::Minimize
            && !self.minimized_for_game
        {
            self.tasks.minimize_launcher();
            self.minimized_for_game = true;
        }
    }

    /// The proxy covers Minecraft's login services for every game, but server
    /// connections only through the Arctic Client (Fabric/Quilt and Vanilla
    /// instances that have it on). Say so when a game that skips it starts.
    pub(crate) fn warn_if_proxy_misses_servers(&mut self, instance: &Instance, version_id: &str) {
        if arctic_core::proxy::ProxySettings::load(&self.dirs)
            .active()
            .is_none()
        {
            return;
        }
        let forge = matches!(
            instance.loader.kind(),
            Some(
                arctic_core::loaders::LoaderKind::Forge
                    | arctic_core::loaders::LoaderKind::NeoForge
            )
        );
        if instance.arctic_mod && !forge && arctic_core::arctic_mod::supports(version_id) {
            return;
        }
        self.toasts.push(
            crate::toasts::Kind::Error,
            "The proxy won't cover this game's servers",
            "Only Minecraft's login goes through it. Servers see your real address unless you play with the Arctic Client (Vanilla or Fabric instances).",
        );
    }

    /// End the background game (if any).
    pub(crate) fn stop_standby(&mut self) {
        let Some(id) = self.runs.standby().map(|r| r.id) else {
            return;
        };
        if let Some(run) = self.runs.get_mut(id) {
            run.standby_cancelled = true;
            if let Some(game) = run.game() {
                game.terminate();
            }
        }
    }
}

/// Names, sizes and times of a folder's files, hashed: changes when a mod is added, removed or replaced.
fn mods_stamp(dir: &std::path::Path) -> u64 {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let meta = e.metadata().ok();
            (
                e.file_name(),
                meta.as_ref().map(|m| m.len()),
                meta.and_then(|m| m.modified().ok()),
            )
        })
        .collect();
    entries.sort();
    let mut hasher = DefaultHasher::new();
    entries.hash(&mut hasher);
    hasher.finish()
}
