//! Background work. Every slow operation runs on its own thread and reports
//! back through `Event`s, so the UI thread never blocks on the network or
//! the game process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use crate::t;
use arctic_core::auth::avatar::{self, Face};
use arctic_core::auth::microsoft::{self, DeviceCode, MsaConfig};
use arctic_core::auth::{self, Account, now_secs};
use arctic_core::cosmetics::{self, SuggestionReceipt, SuggestionReplies};
use arctic_core::instances::Instance;
use arctic_core::launch::progress::{LaunchProgress, LaunchSteps};
use arctic_core::launch::{self, GameEvent, GameHandle, LaunchRequest};
use arctic_core::loaders::{self, LoaderKind, LoaderVersion};
use arctic_core::mods::{self, InstalledMod, SearchPage, SearchQuery, malware};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;
use arctic_core::update::{self, UpdateChannel, UpdateInfo};
use arctic_core::versions::{VersionEntry, VersionManifest};
use arctic_core::{ProgressInfo, Result};
use eframe::egui;
/// Translate compatibility failures from the exact-version install paths.
pub(crate) fn content_install_error(error: arctic_core::Error) -> String {
    match error {
        arctic_core::Error::IncompatibleContentVersion => {
            t!("instances.hub.incompatible_blocked").into()
        }
        arctic_core::Error::DataPackWorldMissing => t!("instances.hub.world_missing").into(),
        other => other.to_string(),
    }
}

/// Owned copy of a core `ProgressInfo`, sendable to the UI thread.
#[derive(Debug, Clone, Default)]
pub struct ProgressSnapshot {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Overall launch percent, 0..=100. `None` for jobs that are a single stage.
    /// 100 only after every stage has finished.
    pub percent: Option<u8>,
}

impl From<ProgressInfo<'_>> for ProgressSnapshot {
    fn from(p: ProgressInfo<'_>) -> Self {
        Self {
            stage: p.stage.to_owned(),
            done: p.done,
            total: p.total,
            bytes_done: p.bytes_done,
            bytes_total: p.bytes_total,
            percent: None,
        }
    }
}

/// Identifies one login attempt so results from a cancelled attempt are ignored.
pub type LoginAttempt = u64;
/// Identifies one launch, so game events can be matched to it.
pub type LaunchId = u64;

pub(crate) type Outcome<T> = std::result::Result<T, String>;

/// The most of a log that is sent to the staff (its end).
const SENT_LOG_BYTES: usize = 300 * 1024;

/// What moving an instance's mods to another version did.
#[derive(Debug, Clone)]
pub struct Retargeted {
    /// The loader build for the new version.
    pub loader_version: String,
    pub updated: usize,
    pub disabled: Vec<String>,
    pub failed: Vec<String>,
}

pub(crate) enum MalwareAction {
    Disable(usize),
    Dismiss(usize),
    Proceed,
    Cancel,
}

pub(crate) struct ModMalwareWarning {
    pub instance: String,
    pub findings: Vec<malware::Finding>,
    pub disable_error: Option<String>,
    pub dismiss_error: Option<String>,
    pub respond: Sender<MalwareAction>,
}

pub enum Event {
    /// A file dialog never appeared (no desktop portal or zenity on Linux).
    PickerMissing,
    Manifest(Outcome<VersionManifest>),
    /// Preparing a launch (id, progress).
    LaunchProgress(LaunchId, ProgressSnapshot),
    ModMalwareWarning(ModMalwareWarning),
    LaunchCancelled(LaunchId),
    UpdateProgress(ProgressSnapshot),
    AccountRefreshed {
        previous: Account,
        fresh: Account,
    },
    /// Game process started (window not necessarily up yet). Carries the
    /// version id that was actually launched.
    /// The game, its log file, its version and how Java was started.
    Launched(LaunchId, Outcome<(GameHandle, PathBuf, String, String)>),
    /// Sent from the game's watcher threads; may overtake `Launched`.
    Game(LaunchId, GameEvent),
    DeviceCode(LoginAttempt, DeviceCode),
    LoginFinished(LoginAttempt, Outcome<Account>),
    Face(String, Face),
    UpdateChecked(Outcome<Option<UpdateInfo>>),
    /// A proxy test finished (the settings tested, result).
    ProxyTested(arctic_core::proxy::ProxySettings, Outcome<()>),
    UpdateInstalled(Outcome<()>),
    /// The background updater installed a new version (`true`: a required one).
    UpdateApplied(bool),
    /// What's new since the version that ran before an update.
    ReleaseNotes(Outcome<update::ReleaseNotes>),
    /// Minecraft versions a loader supports.
    LoaderGames(LoaderKind, Outcome<Vec<String>>),
    /// The versions of a modpack project (project id, result).
    PackVersions(String, Outcome<Vec<mods::modpack::PackVersion>>),
    /// What a modpack version contains (version id, result).
    PackPreview(
        String,
        Outcome<std::sync::Arc<mods::pack_preview::PackPreview>>,
    ),
    /// Loader versions for (loader, Minecraft version).
    LoaderVersions(LoaderKind, String, Outcome<Vec<LoaderVersion>>),
    /// Mod search results for request id.
    ModSearch(u64, Outcome<SearchPage>),
    /// The filter lists of the discovery page.
    HubTags(Outcome<mods::discovery::Tags>),
    /// Installed mod scan (instance, request, changed snapshot).
    ModsScanned(String, u64, Outcome<Option<crate::mod_tasks::ModSnapshot>>),
    ModProgress(ProgressSnapshot),
    /// (instance id, project id, result)
    ModInstalled(String, String, Outcome<Vec<InstalledMod>>),
    ContentProgress(crate::content_tasks::ContentProgress),
    ContentInstalled(crate::content_tasks::ContentReport),
    /// What an instance's mods become on another version: (instance id, version, result).
    /// The newer builds of an instance's mods (from `mod_updates_check`).
    ModUpdatesChecked(String, Outcome<Vec<arctic_core::mods::ModUpdate>>),
    /// Mods brought up to date: how many were installed, and the projects that couldn't be.
    ModUpdatesApplied(String, Outcome<(usize, Vec<String>)>),
    RetargetChecked(
        String,
        String,
        Outcome<Vec<arctic_core::mods::retarget::ModCheck>>,
    ),
    /// An instance's mods moved to another version: (instance id, version, result).
    Retargeted(String, String, Outcome<Retargeted>),
    /// What an instance's mods become under another loader: (instance id, loader, result).
    LoaderChecked(
        String,
        LoaderKind,
        Outcome<Vec<arctic_core::mods::retarget::ModCheck>>,
    ),
    /// An instance moved to another loader: (instance id, loader, result).
    LoaderSwitched(String, LoaderKind, Outcome<Retargeted>),
    /// Icon bytes for a URL.
    ModIcon(String, Outcome<Vec<u8>>),
    ModGalleryImage(String, Outcome<Arc<[u8]>>),
    /// Fallback gallery URL for a Discover project with no featured search image.
    HubBanner(String, Option<String>),
    /// Skin and capes of an account (account id, result).
    SkinAccount(String, Outcome<crate::skin_tasks::AccountSkin>),
    MinecraftSkinSynced(
        PathBuf,
        String,
        Outcome<crate::skin_tasks::SyncedMinecraftSkin>,
    ),
    /// Another player's skin by name.
    PlayerSkin(String, Outcome<(Vec<u8>, arctic_core::skins::Variant)>),
    /// The Arctic look and catalog (account id, result).
    ArcticLook(String, Outcome<crate::skin_tasks::LoadedArcticState>),
    /// The result of publishing a change to an account's look.
    ArcticLookSaved(String, Outcome<crate::skin_tasks::ArcticState>),
    /// Public capes and cosmetics, available without an account.
    ArcticCatalog(Outcome<crate::skin_tasks::LoadedArcticState>),
    /// A picked cape image, or `None` if cancelled.
    CapeFile(Outcome<Option<Vec<u8>>>),
    /// The OptiFine cape of a player name (a Minecraft-layout PNG), `None` when there is none.
    OptifineCape(String, Outcome<Option<Vec<u8>>>),
    /// A picked skin file (name, bytes), or `None` if cancelled.
    SkinFile(Outcome<Option<(String, Vec<u8>)>>),
    SkinPngSaved(Outcome<Option<()>>),
    /// A world import, backup or other world job finished (instance id, message).
    WorldsDone(String, Outcome<String>),
    /// A modpack install or import finished (`None` = file dialog cancelled).
    ModpackInstalled(Outcome<Option<Instance>>),
    /// An instance was duplicated (the copy).
    InstanceDuplicated(Outcome<Instance>),
    /// Gallery search results (request id, result).
    Gallery(u64, Outcome<crate::gallery_tasks::GalleryResult>),
    /// A gallery skin was downloaded: (name, PNG, model, wear it).
    GalleryTaken(
        Option<uuid::Uuid>,
        Outcome<(String, Vec<u8>, arctic_core::skins::Variant)>,
    ),
    /// A gallery share or report finished (message).
    GalleryDone(Outcome<String>),
    /// A crash report was sent.
    CrashReportSent(Outcome<()>),
    SuggestionSent(PathBuf, Outcome<(Option<crate::ui::suggest::Report>, bool)>),
    SuggestionReplies(
        uuid::Uuid,
        Vec<(uuid::Uuid, Option<SuggestionReplies>)>,
        bool,
    ),
    /// A log sent to the staff: the code to give them.
    LogSent(Outcome<String>),
    /// Other launchers looked through (the folder, if one was picked).
    MigrateScanned(Option<PathBuf>, crate::migrate_tasks::ScanResult),
    MigrateProgress(String, ProgressSnapshot),
    MigrateItemDone(crate::migrate_tasks::Outcome),
    MigrateFinished,
    /// Choosing folders (see `storage_tasks`).
    Storage(crate::storage_tasks::StorageEvent),
    /// Pack search results for request id.
    PackSearch(u64, Outcome<SearchPage>),
    /// (instance id, project id, result)
    PackInstalled(String, String, Outcome<crate::pack_tasks::PackDone>),
    /// The selected instance's server list (refresh request id).
    ServerList(u64, Outcome<Vec<arctic_core::servers::Server>>),
    /// (request id, address, status)
    ServerStatus(u64, String, Outcome<arctic_core::servers::Status>),
    /// The address typed into "Direct join" answered (or not).
    DirectPing(String, Outcome<arctic_core::servers::Status>),
    /// Profile and friends (refresh).
    Friends(Outcome<crate::friend_tasks::FriendsView>),
    /// A friends action finished (what to say, or empty).
    FriendDone(Outcome<String>),
    /// (friend profile, conversation)
    ChatHistory(String, Outcome<Vec<arctic_core::friends::Message>>),
    ChatSent(Outcome<arctic_core::friends::Message>),
    /// (attachment id, PNG)
    ChatImage(String, Outcome<Vec<u8>>),
    /// (polled after this id, new messages)
    ChatNew(i64, Outcome<Vec<arctic_core::friends::Message>>),
    /// A new recovery code (shown once; offline accounts only).
    #[cfg(feature = "offline-accounts")]
    RecoveryCode(Outcome<String>),
    PublicServers(Outcome<Vec<arctic_core::servers::public::PublicServer>>),
    ServerSubmitted(Outcome<arctic_core::servers::public::Submitted>),
    /// A Bedrock tab job (see `bedrock_tasks`).
    Bedrock(crate::bedrock_tasks::BedrockEvent),
    Screenshots(Vec<arctic_core::screenshots::Shot>),
    ScreenshotThumb(PathBuf, Outcome<Vec<u8>>),
    ScreenshotCopied(Outcome<()>),
    /// The image chosen for the launcher's background (`None`: the dialog was closed).
    BackdropPicked(Option<PathBuf>),
    /// The game asked to add an account (show the sign-in).
    AddAccountFromGame,
    /// The game asked to add an offline account with this name.
    AddOfflineFromGame(String),
    /// The game's skin picker: wear this library skin (`None`: the Minecraft skin).
    WearSkinFromGame(Option<String>),
    /// A share code was made.
    ShareCode(Outcome<String>),
    /// A share file was saved (`None` = cancelled).
    ShareSaved(Outcome<Option<PathBuf>>),
    /// A shared bundle was read from a file or code (`None` = cancelled).
    ShareLoaded(Outcome<Option<arctic_core::sharing::Bundle>>),
    ShareProgress(ProgressSnapshot),
    ShareImported(Outcome<crate::share_tasks::Imported>),
    /// Mod files of an instance looked up on Modrinth for sharing (instance id, files now known).
    ShareModsLookedUp(String, Outcome<usize>),
    /// Play-together session update.
    Share(arctic_share::SessionId, arctic_share::ShareEvent),
}

#[derive(Default)]
struct AccountRefresh {
    /// The account from the UI and its replacement, while other jobs may still have the old copy.
    last: Option<(Account, Account)>,
}

type AccountRefreshes = HashMap<String, Arc<Mutex<AccountRefresh>>>;

/// Handle used by the UI to start background jobs.
#[derive(Clone)]
pub struct Tasks {
    tx: Sender<Event>,
    ctx: egui::Context,
    dirs: DataDirs,
    /// The in-game account switcher's bridge, passed to launched games.
    bridge: Option<Arc<arctic_core::bridge::BridgeInfo>>,
    content_lock: Arc<Mutex<()>>,
    account_refreshes: Arc<Mutex<AccountRefreshes>>,
}

/// How a game starts beyond its instance: which copy it is, and where it
/// goes right away.
#[derive(Debug, Clone, Default)]
pub struct StartAt {
    pub copy: u32,
    pub quick_play: Option<arctic_core::launch::QuickPlay>,
    /// This game's own bridge info (with its join token), instead of the launcher-wide one.
    pub bridge: Option<arctic_core::bridge::BridgeInfo>,
}

impl Tasks {
    pub fn new(tx: Sender<Event>, ctx: egui::Context, dirs: DataDirs) -> Self {
        Self {
            tx,
            ctx,
            dirs,
            bridge: None,
            content_lock: Arc::default(),
            account_refreshes: Arc::default(),
        }
    }

    /// Same channel, different data folders (after a profile switch).
    pub fn with_dirs(&self, dirs: DataDirs) -> Self {
        Self {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
            dirs,
            bridge: self.bridge.clone(),
            content_lock: Arc::clone(&self.content_lock),
            account_refreshes: Arc::default(),
        }
    }

    pub fn with_bridge(mut self, bridge: Option<Arc<arctic_core::bridge::BridgeInfo>>) -> Self {
        self.bridge = bridge;
        self
    }

    /// Installs that share a mod index or pack settings must not race.
    pub(crate) fn content_lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.content_lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn fresh_account(&self, account: Account) -> Result<Account> {
        self.refresh_account(account, false)
    }

    /// Serialize refresh for each Microsoft account across game requests, launches and other jobs.
    pub(crate) fn refresh_account(&self, account: Account, force: bool) -> Result<Account> {
        if !account.is_microsoft() {
            return if force {
                Err(arctic_core::Error::OfflineSessionRefresh)
            } else {
                Ok(account)
            };
        }
        let slot = {
            let mut refreshes = self
                .account_refreshes
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            Arc::clone(refreshes.entry(account.id.clone()).or_default())
        };
        let mut refresh = slot.lock().unwrap_or_else(|e| e.into_inner());
        let current = match &refresh.last {
            Some((previous, fresh)) if previous == &account => {
                if !fresh.needs_refresh(now_secs()) {
                    return Ok(fresh.clone());
                }
                fresh
            }
            _ => &account,
        };
        let (fresh, changed) = auth::refresh_session(&self.dirs, current, force)?;
        if changed {
            self.send(Event::AccountRefreshed {
                previous: current.clone(),
                fresh: fresh.clone(),
            });
            refresh.last = Some((account, fresh.clone()));
        }
        Ok(fresh)
    }

    pub(crate) fn dirs(&self) -> &DataDirs {
        &self.dirs
    }

    pub(crate) fn run(&self, job: impl FnOnce(&Tasks) + Send + 'static) {
        let me = self.clone();
        std::thread::spawn(move || job(&me));
    }

    /// Send the setup's optional "how did you hear about us?" answer (nothing happens if it fails).
    pub fn send_heard(&self, source: &'static str, other: String) {
        self.run(move |_| {
            let base = arctic_core::cosmetics::base_url();
            if let Err(e) = arctic_core::cosmetics::send_heard(&base, source, &other) {
                log::debug!("could not send the answer: {e}");
            }
        });
    }

    /// Send a log to the staff in the background (names and folders are taken out first).
    pub fn send_log(&self, source: String, game: String, text: String) {
        self.run(move |t| {
            let base = arctic_core::cosmetics::base_url();
            let log = arctic_core::crash::scrubbed_shared_log(&text, SENT_LOG_BYTES);
            let result = arctic_core::cosmetics::send_log(
                &base,
                &arctic_core::cosmetics::SharedLog {
                    launcher: arctic_core::APP_VERSION,
                    game: &game,
                    os: std::env::consts::OS,
                    source: &source,
                    log: &log,
                },
            )
            .map_err(|e| e.to_string());
            t.send(Event::LogSent(result));
        });
    }

    /// Send an idea or bug report in the background.
    pub fn send_suggestion(&self, suggestion: arctic_core::cosmetics::Suggestion) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = cosmetics::send_suggestion(&base, &suggestion)
                .map(|receipt| {
                    let Some(receipt) = receipt else {
                        return (None, false);
                    };
                    let report = crate::ui::suggest::Report::new(base, receipt, suggestion);
                    let saved = match report.save(&t.dirs) {
                        Ok(()) => true,
                        Err(e) => {
                            log::warn!("could not save sent report: {e}");
                            false
                        }
                    };
                    (Some(report), saved)
                })
                .map_err(|e| e.to_string());
            t.send(Event::SuggestionSent(
                t.dirs.profile_root().to_owned(),
                result,
            ));
        });
    }

    pub(crate) fn check_suggestion_replies(
        &self,
        request: uuid::Uuid,
        reports: Vec<(uuid::Uuid, String, SuggestionReceipt)>,
    ) {
        self.run(move |t| {
            let mut updates = Vec::new();
            let mut failed = false;
            for (key, server, receipt) in reports {
                match cosmetics::suggestion_replies(&server, &receipt) {
                    Ok(replies) => updates.push((key, replies)),
                    Err(e) => {
                        log::debug!("could not check report replies: {e}");
                        failed = true;
                        break;
                    }
                }
            }
            t.send(Event::SuggestionReplies(request, updates, failed));
        });
    }

    /// Callback for the play-together service, feeding the event loop.
    pub fn share_sink(
        &self,
    ) -> impl Fn(arctic_share::SessionId, arctic_share::ShareEvent) + Send + Sync + 'static {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        move |id, event| {
            let _ = tx.send(Event::Share(id, event));
            ctx.request_repaint();
        }
    }

    /// After a file dialog returned nothing: tell the player when that is because the system has no
    /// file picker (Linux without a desktop portal, zenity or kdialog), not because they cancelled.
    pub(crate) fn note_if_picker_missing<T>(&self, picked: &Option<T>) {
        if picked.is_none() && !file_picker_available() {
            self.send(Event::PickerMissing);
        }
    }

    /// Make the window draw a frame now (something outside the UI needs it to look).
    pub(crate) fn wake(&self) {
        self.ctx.request_repaint();
    }

    pub(crate) fn send(&self, event: Event) {
        // The receiver only disappears when the app is closing.
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }

    pub fn load_manifest(&self) {
        self.run(|t| {
            // The last copy shows right away; the fresh one replaces it.
            let cached = VersionManifest::cached(&t.dirs);
            let have_cached = cached.is_some();
            if let Some(manifest) = cached {
                t.send(Event::Manifest(Ok(manifest)));
            }
            match VersionManifest::fetch_for_launch(&t.dirs) {
                Ok(manifest) => t.send(Event::Manifest(Ok(manifest))),
                // Offline with a copy on screen: keep using it.
                Err(_) if have_cached => {}
                Err(e) => t.send(Event::Manifest(Err(e.to_string()))),
            }
        });
    }

    pub fn launch(
        &self,
        id: LaunchId,
        version: VersionEntry,
        instance: Instance,
        account: Account,
        settings: Settings,
        start: StartAt,
    ) {
        let play_started = std::time::Instant::now();
        self.run(move |t| {
            let result = t.launch_blocking(id, &version, &instance, account, &settings, start);
            if result.is_ok() {
                log::info!(
                    "[startup] Play to Java process: {} ms",
                    play_started.elapsed().as_millis()
                );
            }
            // The toast shows this string; it has to be the whole cause chain, not just Display.
            let result = match result {
                Ok(Some((game, log, java))) => Ok((game, log, version.id.clone(), java)),
                Ok(None) => {
                    t.send(Event::LaunchCancelled(id));
                    return;
                }
                Err(err) => Err(err.chain()),
            };
            t.send(Event::Launched(id, result));
        });
    }

    fn launch_blocking(
        &self,
        id: LaunchId,
        version: &VersionEntry,
        instance: &Instance,
        account: Account,
        settings: &Settings,
        start: StartAt,
    ) -> Result<Option<(GameHandle, PathBuf, String)>> {
        // Launches can share installer caches, and a new copy snapshots the instance's mods.
        let mut content = self.content_lock();
        let tracker = Mutex::new(LaunchProgress::new(LaunchSteps::from_instance(instance)));
        let progress = |p: ProgressInfo| {
            let percent = tracker
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .observe(p.stage, p.done, p.total, p.bytes_done, p.bytes_total);
            let mut snapshot = ProgressSnapshot::from(p);
            snapshot.percent = Some(percent);
            self.send(Event::LaunchProgress(id, snapshot));
        };
        if account.needs_refresh(now_secs()) {
            progress(ProgressInfo::stage(t!("tasks.refreshing_microsoft_login")));
        }
        let account = self.fresh_account(account)?;
        let req = LaunchRequest {
            dirs: &self.dirs,
            version,
            instance,
            account: &account,
            settings,
            bridge: start.bridge.as_ref().or(self.bridge.as_deref()),
            copy: start.copy,
            quick_play: start.quick_play,
        };
        let prepared = launch::prepare_watched(&req, &progress)?;
        if settings.check_mods_for_malware {
            progress(ProgressInfo::stage(t!("tasks.checking_mods_for_malware")));
            let mods = prepared.plan.game_dir.join("mods");
            let instance_mods = instance.game_dir(&self.dirs).join("mods");
            let mut findings = malware::scan_for_launch(&self.dirs, &mods);
            let mut disable_error = None;
            let mut dismiss_error = None;
            if !findings.is_empty() {
                loop {
                    let (respond, decision) = std::sync::mpsc::channel();
                    self.send(Event::ModMalwareWarning(ModMalwareWarning {
                        instance: instance.name.clone(),
                        findings: findings.clone(),
                        disable_error: disable_error.take(),
                        dismiss_error: dismiss_error.take(),
                        respond,
                    }));
                    // Keep the game directory reserved while the dialog is open,
                    // but let other launches and content operations proceed.
                    drop(content);
                    let action = decision.recv().unwrap_or(MalwareAction::Cancel);
                    if matches!(action, MalwareAction::Cancel) {
                        return Ok(None);
                    }
                    content = self.content_lock();
                    if let MalwareAction::Disable(index) = action
                        && let Some(finding) = findings.get(index)
                    {
                        disable_error = malware::disable(finding, &instance_mods)
                            .err()
                            .map(|err| err.to_string());
                    }
                    if let MalwareAction::Dismiss(index) = action
                        && let Some(finding) = findings.get(index)
                    {
                        // Failing to save a dismissal must not abort the launch.
                        dismiss_error = malware::dismiss(&self.dirs, finding)
                            .err()
                            .map(|err| err.to_string());
                    }
                    let current = malware::scan_for_launch(&self.dirs, &mods);
                    if matches!(action, MalwareAction::Dismiss(_)) && current.is_empty() {
                        break;
                    }
                    if matches!(action, MalwareAction::Proceed)
                        && current.iter().all(|finding| findings.contains(finding))
                    {
                        break;
                    }
                    findings = current;
                }
            }
        }
        let log_file = prepared.plan.log_file.clone();
        let java = prepared.plan.java_summary();
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let game = prepared.spawn(move |event| {
            let _ = tx.send(Event::Game(id, event));
            ctx.request_repaint();
        })?;
        drop(content);
        // Every stage finished and the process started. A failure above never sends 100%.
        let percent = tracker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .finish();
        self.send(Event::LaunchProgress(
            id,
            ProgressSnapshot {
                stage: t!("tasks.starting").into(),
                percent: Some(percent),
                ..ProgressSnapshot::default()
            },
        ));
        Ok(Some((game, log_file, java)))
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

    pub fn loader_games(&self, kind: LoaderKind) {
        self.run(move |t| {
            let result = loaders::game_versions(kind).map_err(|e| e.to_string());
            t.send(Event::LoaderGames(kind, result));
        });
    }

    pub fn loader_versions(&self, kind: LoaderKind, game: String) {
        self.run(move |t| {
            let result = loaders::loader_versions(kind, &game).map_err(|e| e.to_string());
            t.send(Event::LoaderVersions(kind, game, result));
        });
    }

    /// Categories, game versions and loaders for the discovery filters.
    pub fn hub_tags(&self) {
        self.run(move |t| {
            let result = mods::discovery::tags().map_err(|e| e.to_string());
            t.send(Event::HubTags(result));
        });
    }

    pub fn mod_search(&self, request: u64, query: SearchQuery) {
        self.run(move |t| {
            let result = mods::search(&query).map_err(|e| e.to_string());
            t.send(Event::ModSearch(request, result));
        });
    }

    /// Copy an instance (worlds too, if asked) into a new one.
    pub fn duplicate_instance(&self, source: Instance, with_worlds: bool) {
        self.run(move |t| {
            let result = arctic_core::instances::duplicate(&t.dirs, &source, with_worlds)
                .map_err(|e| e.to_string());
            t.send(Event::InstanceDuplicated(result));
        });
    }

    /// Install a Modrinth modpack as a new instance: the version given, or the newest release.
    pub fn install_modpack(&self, project_id: String, version_id: Option<String>) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = mods::modpack::install_version(
                &t.dirs,
                &project_id,
                version_id.as_deref(),
                &progress,
            )
            .map(Some)
            .map_err(|e| e.to_string());
            t.send(Event::ModpackInstalled(result));
        });
    }

    /// List the versions of a modpack project for the picker.
    pub fn pack_versions(&self, project_id: String) {
        self.run(move |t| {
            let result = mods::modpack::pack_versions(&project_id).map_err(|e| e.to_string());
            t.send(Event::PackVersions(project_id, result));
        });
    }

    /// Read what a modpack version contains, without installing it.
    pub fn pack_preview(&self, version_id: String) {
        self.run(move |t| {
            let result = mods::pack_preview::preview(&t.dirs, &version_id)
                .map(std::sync::Arc::new)
                .map_err(|e| e.to_string());
            t.send(Event::PackPreview(version_id, result));
        });
    }

    /// Ask for a `.mrpack` file and install it as a new instance.
    pub fn import_modpack_file(&self) {
        self.install_modpack_file(None);
    }

    /// Install a `.mrpack` file as a new instance (asking for the file when `path` is `None`).
    pub fn install_modpack_file(&self, path: Option<std::path::PathBuf>) {
        self.run(move |t| {
            let picked = path.or_else(|| {
                rfd::FileDialog::new()
                    .set_title(t!("tasks.choose_a_modpack"))
                    .add_filter(t!("tasks.modrinth_modpack"), &["mrpack", "zip"])
                    .pick_file()
            });
            t.note_if_picker_missing(&picked);
            let result = match picked {
                None => Ok(None),
                Some(path) => {
                    let progress =
                        |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
                    mods::modpack::install_file(&t.dirs, &path, &progress)
                        .map(Some)
                        .map_err(|e| e.to_string())
                }
            };
            t.send(Event::ModpackInstalled(result));
        });
    }

    /// Look for newer builds of the mods Arctic installed into `instance`.
    pub fn mod_updates_check(&self, instance: Instance) {
        self.run(move |t| {
            let result = match instance.loader.kind() {
                Some(loader) => mods::available_updates(
                    &instance.version.clone().unwrap_or_default(),
                    loader,
                    &instance.game_dir(&t.dirs).join("mods"),
                    &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                )
                .map_err(|e| e.to_string()),
                None => Ok(Vec::new()),
            };
            t.send(Event::ModUpdatesChecked(instance.id, result));
        });
    }

    /// Install the newest builds of `projects` (each replacing the old one).
    pub fn mod_updates_apply(&self, instance: Instance, projects: Vec<String>) {
        self.run(move |t| {
            let _guard = t.content_lock();
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = match instance.loader.kind() {
                Some(loader) => {
                    let refs: Vec<&str> = projects.iter().map(String::as_str).collect();
                    mods::install_many(
                        &refs,
                        &instance.version.clone().unwrap_or_default(),
                        loader,
                        &instance.game_dir(&t.dirs).join("mods"),
                        &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                        &progress,
                    )
                    .map(|(installed, failed)| {
                        (
                            installed.len(),
                            failed.into_iter().map(|(project, _)| project).collect(),
                        )
                    })
                    .map_err(|e| e.to_string())
                }
                None => Err(t!("tasks.vanilla_instances_cant_load_mods").to_owned()),
            };
            t.send(Event::ModUpdatesApplied(instance.id, result));
        });
    }

    /// Look up what the instance's mods become on `version`.
    pub fn retarget_check(&self, instance: Instance, version: String) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = match instance.loader.kind() {
                Some(loader) => mods::retarget::check(
                    &instance.game_dir(&t.dirs).join("mods"),
                    &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                    &version,
                    loader,
                    &progress,
                )
                .map_err(|e| e.to_string()),
                None => Ok(Vec::new()),
            };
            t.send(Event::RetargetChecked(instance.id, version, result));
        });
    }

    /// Move the instance's mods to `version` (from a check for it) and find its loader build.
    pub fn retarget_apply(
        &self,
        instance: Instance,
        version: String,
        checks: Vec<arctic_core::mods::retarget::ModCheck>,
    ) {
        self.run(move |t| {
            let _guard = t.content_lock();
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = (|| -> Outcome<Retargeted> {
                let loader = instance
                    .loader
                    .kind()
                    .ok_or(t!("tasks.vanilla_instances_have_no_mods_to_move"))?;
                // The loader build first: a version without one changes nothing.
                let loader_version = arctic_core::loaders::default_loader_version(loader, &version)
                    .map_err(|e| e.to_string())?;
                let applied = mods::retarget::apply(
                    &instance.game_dir(&t.dirs).join("mods"),
                    &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                    &checks,
                    &version,
                    loader,
                    &progress,
                )
                .map_err(|e| e.to_string())?;
                Ok(Retargeted {
                    loader_version,
                    updated: applied.installed.len(),
                    disabled: applied.disabled,
                    failed: applied.failed.into_iter().map(|(p, _)| p).collect(),
                })
            })();
            t.send(Event::Retargeted(instance.id, version, result));
        });
    }

    /// Look up what the instance's mods become under `loader` (same Minecraft version).
    pub fn loader_check(&self, instance: Instance, loader: LoaderKind) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = mods::retarget::check(
                &instance.game_dir(&t.dirs).join("mods"),
                &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                &instance.version.clone().unwrap_or_default(),
                loader,
                &progress,
            )
            .map_err(|e| e.to_string());
            t.send(Event::LoaderChecked(instance.id, loader, result));
        });
    }

    /// Move the instance's mods to `loader` (from a check for it) and find its newest build.
    pub fn loader_switch(
        &self,
        instance: Instance,
        loader: LoaderKind,
        checks: Vec<arctic_core::mods::retarget::ModCheck>,
    ) {
        self.run(move |t| {
            let _guard = t.content_lock();
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let result = (|| -> Outcome<Retargeted> {
                let game = instance.version.clone().unwrap_or_default();
                // The build first: a loader without one for this version changes nothing.
                let loader_version = arctic_core::loaders::default_loader_version(loader, &game)
                    .map_err(|e| e.to_string())?;
                let applied = mods::retarget::apply(
                    &instance.game_dir(&t.dirs).join("mods"),
                    &mods::index_path(&t.dirs.instance_dir(&instance.id)),
                    &checks,
                    &game,
                    loader,
                    &progress,
                )
                .map_err(|e| e.to_string())?;
                Ok(Retargeted {
                    loader_version,
                    updated: applied.installed.len(),
                    disabled: applied.disabled,
                    failed: applied.failed.into_iter().map(|(p, _)| p).collect(),
                })
            })();
            t.send(Event::LoaderSwitched(instance.id, loader, result));
        });
    }

    /// Install a mod (and what it needs); `replacing` (an older copy's file) is switched off
    /// once it's in.
    pub fn mod_install(&self, instance: Instance, project_id: String, replacing: Option<String>) {
        self.mod_install_selected(instance, project_id, replacing, None);
    }

    pub fn mod_install_version(&self, instance: Instance, project_id: String, version_id: String) {
        self.mod_install_selected(instance, project_id, None, Some(version_id));
    }

    fn mod_install_selected(
        &self,
        instance: Instance,
        project_id: String,
        replacing: Option<String>,
        version_id: Option<String>,
    ) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| t.send(Event::ModProgress(ProgressSnapshot::from(p)));
            let _guard = t.content_lock();
            let game = instance.version.clone().unwrap_or_default();
            let result = match instance.loader.kind() {
                Some(loader) => {
                    let dir = t.dirs.instance_dir(&instance.id);
                    let mods_dir = instance.game_dir(&t.dirs).join("mods");
                    let install = if version_id.is_some() {
                        mods::install_version
                    } else {
                        mods::install
                    };
                    let result = install(
                        version_id.as_deref().unwrap_or(&project_id),
                        &game,
                        loader,
                        &mods_dir,
                        &mods::index_path(&dir),
                        &progress,
                    );
                    // Unless the "new" one is that very file.
                    if let (Ok(installed), Some(old)) = (&result, &replacing)
                        && !installed.iter().any(|m| &m.file_name == old)
                        && let Err(e) = mods::set_enabled(&mods_dir, old, false)
                    {
                        log::warn!("could not turn off {old}: {e}");
                    }
                    result.map_err(content_install_error)
                }
                None => Err(t!("tasks.vanilla_instances_cant_load_mods").to_owned()),
            };
            t.send(Event::ModInstalled(instance.id, project_id, result));
        });
    }

    pub fn mod_gallery_image(&self, url: String) {
        self.run(move |t| {
            let result = mods::gallery_image(&url, &t.dirs.cache().join("gallery"))
                .map(Arc::from)
                .map_err(|e| e.to_string());
            t.send(Event::ModGalleryImage(url, result));
        });
    }

    pub fn hub_banner(&self, project: String) {
        self.run(move |t| {
            let url = mods::project_detail(&project)
                .ok()
                .and_then(|detail| detail.banner_url().map(str::to_owned));
            t.send(Event::HubBanner(project, url));
        });
    }

    pub fn mod_icon(&self, url: String) {
        self.run(move |t| {
            let result = mods::icon(&url, &t.dirs.cache().join("icons")).map_err(|e| e.to_string());
            t.send(Event::ModIcon(url, result));
        });
    }

    pub fn test_proxy(&self, proxy: arctic_core::proxy::ProxySettings) {
        self.run(move |t| {
            let result = arctic_core::net::test_proxy(&proxy).map_err(|e| e.to_string());
            t.send(Event::ProxyTested(proxy, result));
        });
    }

    pub fn check_update(&self, channel: UpdateChannel) {
        self.run(move |t| {
            t.send(Event::UpdateChecked(
                update::check(channel).map_err(|e| e.to_string()),
            ));
        });
    }

    /// [`Self::check_update`] after `delay` (at start: once the window is up).
    pub fn check_update_after(&self, channel: UpdateChannel, delay: std::time::Duration) {
        self.run(move |t| {
            std::thread::sleep(delay);
            t.send(Event::UpdateChecked(
                update::check(channel).map_err(|e| e.to_string()),
            ));
        });
    }

    /// What changed since `since`, the version that ran before an update.
    pub fn load_release_notes(&self, channel: UpdateChannel, since: String) {
        self.run(move |t| {
            t.send(Event::ReleaseNotes(
                update::notes_since(channel, &since).map_err(|e| e.to_string()),
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

/// Whether a file dialog can open here. Only Linux lacks one by default: the dialog needs a
/// desktop portal, or zenity or kdialog, which minimal installs don't have.
pub(crate) fn file_picker_available() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let in_path = |name: &str| {
        std::env::var_os("PATH") // i18n-ignore
            .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
    };
    in_path("zenity")
        || in_path("kdialog")
        || [
            "/usr/libexec/xdg-desktop-portal",
            "/usr/lib/xdg-desktop-portal",
            "/usr/lib/x86_64-linux-gnu/xdg-desktop-portal",
            "/usr/lib/aarch64-linux-gnu/xdg-desktop-portal",
        ]
        .iter()
        .any(|p| std::path::Path::new(p).is_file())
}
