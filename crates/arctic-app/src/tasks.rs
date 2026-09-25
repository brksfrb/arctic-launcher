//! Background work. Every slow operation runs on its own thread and reports
//! back through `Event`s, so the UI thread never blocks on the network or
//! the game process.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use arctic_core::auth::avatar::{self, Face};
use arctic_core::auth::microsoft::{self, DeviceCode, MsaConfig};
use arctic_core::auth::{Account, now_secs};
use arctic_core::instances::Instance;
use arctic_core::launch::{self, GameEvent, GameHandle, LaunchRequest};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;
use arctic_core::update::{self, UpdateChannel, UpdateInfo};
use arctic_core::versions::{VersionEntry, VersionManifest};
use arctic_core::{ProgressInfo, Result};
use eframe::egui;

/// Owned copy of a core `ProgressInfo`, sendable to the UI thread.
#[derive(Debug, Clone, Default)]
pub struct ProgressSnapshot {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl From<ProgressInfo<'_>> for ProgressSnapshot {
    fn from(p: ProgressInfo<'_>) -> Self {
        Self {
            stage: p.stage.to_owned(),
            done: p.done,
            total: p.total,
            bytes_done: p.bytes_done,
            bytes_total: p.bytes_total,
        }
    }
}

/// Identifies one login attempt so results from a cancelled attempt are ignored.
pub type LoginAttempt = u64;
/// Identifies one launch, so game events can be matched to it.
pub type LaunchId = u64;

type Outcome<T> = std::result::Result<T, String>;

pub enum Event {
    Manifest(Outcome<VersionManifest>),
    LaunchProgress(ProgressSnapshot),
    UpdateProgress(ProgressSnapshot),
    AccountRefreshed(Account),
    /// Game process started (window not necessarily up yet). Carries the
    /// version id that was actually launched.
    Launched(LaunchId, Outcome<(GameHandle, PathBuf, String)>),
    /// Sent from the game's watcher threads; may overtake `Launched`.
    Game(LaunchId, GameEvent),
    DeviceCode(LoginAttempt, DeviceCode),
    LoginFinished(LoginAttempt, Outcome<Account>),
    Face(String, Face),
    UpdateChecked(Outcome<Option<UpdateInfo>>),
    UpdateInstalled(Outcome<()>),
}

/// Handle used by the UI to start background jobs.
#[derive(Clone)]
pub struct Tasks {
    tx: Sender<Event>,
    ctx: egui::Context,
    dirs: DataDirs,
}

impl Tasks {
    pub fn new(tx: Sender<Event>, ctx: egui::Context, dirs: DataDirs) -> Self {
        Self { tx, ctx, dirs }
    }

    /// Same channel, different data folders (after a profile switch).
    pub fn with_dirs(&self, dirs: DataDirs) -> Self {
        Self {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
            dirs,
        }
    }

    fn run(&self, job: impl FnOnce(&Tasks) + Send + 'static) {
        let me = self.clone();
        std::thread::spawn(move || job(&me));
    }

    fn send(&self, event: Event) {
        // The receiver only disappears when the app is closing.
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }

    pub fn load_manifest(&self) {
        self.run(|t| {
            let result = VersionManifest::fetch(&t.dirs).map_err(|e| e.to_string());
            t.send(Event::Manifest(result));
        });
    }

    pub fn launch(
        &self,
        id: LaunchId,
        version: VersionEntry,
        instance: Instance,
        account: Account,
        settings: Settings,
    ) {
        self.run(move |t| {
            let result = t.launch_blocking(id, &version, &instance, account, &settings);
            let result = result.map(|(game, log)| (game, log, version.id.clone()));
            t.send(Event::Launched(id, result.map_err(|e| e.to_string())));
        });
    }

    fn launch_blocking(
        &self,
        id: LaunchId,
        version: &VersionEntry,
        instance: &Instance,
        account: Account,
        settings: &Settings,
    ) -> Result<(GameHandle, PathBuf)> {
        let progress =
            |p: ProgressInfo| self.send(Event::LaunchProgress(ProgressSnapshot::from(p)));
        let account = if account.needs_refresh(now_secs()) {
            progress(ProgressInfo::stage("Refreshing Microsoft login"));
            let cfg = MsaConfig::load(&self.dirs)?;
            let fresh = microsoft::refresh(&cfg, &account)?;
            self.send(Event::AccountRefreshed(fresh.clone()));
            fresh
        } else {
            account
        };
        let req = LaunchRequest {
            dirs: &self.dirs,
            version,
            instance,
            account: &account,
            settings,
        };
        let plan = launch::prepare(&req, &progress)?;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let game = launch::spawn(&plan, move |event| {
            let _ = tx.send(Event::Game(id, event));
            ctx.request_repaint();
        })?;
        Ok((game, plan.log_file))
    }

    pub fn login_device_code(&self, attempt: LoginAttempt, cancel: Arc<AtomicBool>) {
        self.run(move |t| {
            let result = (|| {
                let cfg = MsaConfig::load(&t.dirs)?;
                let code = microsoft::start_device_code(&cfg)?;
                t.send(Event::DeviceCode(attempt, code.clone()));
                microsoft::finish_device_code(&cfg, &code, &cancel)
            })();
            t.send(Event::LoginFinished(
                attempt,
                result.map_err(|e| e.to_string()),
            ));
        });
    }

    pub fn login_browser(&self, attempt: LoginAttempt, cancel: Arc<AtomicBool>) {
        self.run(move |t| {
            let result = MsaConfig::load(&t.dirs)
                .and_then(|cfg| microsoft::login_with_browser(&cfg, &cancel));
            t.send(Event::LoginFinished(
                attempt,
                result.map_err(|e| e.to_string()),
            ));
        });
    }

    /// Refresh the player-head avatar of a Microsoft account (best effort).
    pub fn fetch_face(&self, uuid: String) {
        self.run(move |t| match avatar::fetch_face(&t.dirs, &uuid) {
            Ok(face) => t.send(Event::Face(uuid, face)),
            Err(e) => log::debug!("avatar for {uuid} unavailable: {e}"),
        });
    }

    pub fn check_update(&self, channel: UpdateChannel) {
        self.run(move |t| {
            t.send(Event::UpdateChecked(
                update::check(channel).map_err(|e| e.to_string()),
            ));
        });
    }

    pub fn install_update(&self, info: UpdateInfo) {
        self.run(move |t| {
            let progress =
                |p: ProgressInfo| t.send(Event::UpdateProgress(ProgressSnapshot::from(p)));
            let result = update::download_and_apply(&t.dirs, &info, &progress);
            t.send(Event::UpdateInstalled(result.map_err(|e| e.to_string())));
        });
    }
}
