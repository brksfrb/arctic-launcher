//! Application state and event handling. Layout lives in `ui/`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use arctic_core::auth::avatar::{self, Face};
use arctic_core::auth::microsoft::{DeviceCode, MsaConfig};
use arctic_core::auth::{Account, AccountStore};
use arctic_core::instances::Instance;
use arctic_core::launch::logparse::{Level, LogLine};
use arctic_core::launch::{GameEvent, GameHandle};
use arctic_core::profiles::ProfileStore;
use arctic_core::settings::{GameStartAction, Settings, ThemeMode};
use arctic_core::storage::DataDirs;
use arctic_core::update::UpdateInfo;
use arctic_core::versions::{self, VersionManifest};
use eframe::egui;

use crate::art::splash::Splash;
use crate::motion::RateMeter;
use crate::session::ProfileData;
use crate::startup::StartupOptions;
use crate::tasks::{Event, LaunchId, LoginAttempt, ProgressSnapshot, Tasks};
use crate::theme::{self, Palette};
use crate::toasts::{Kind, ToastAction, Toasts};
use crate::ui::{InstancesUi, ProfileDialog};

/// Backdrop frame interval when focused / unfocused.
const FRAME_FOCUSED: Duration = Duration::from_millis(33);
const FRAME_UNFOCUSED: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Play,
    Accounts,
    Instances,
    Skins,
    Together,
    Logs,
    Settings,
    About,
}

impl Tab {
    pub const ALL: [Tab; 8] = [
        Tab::Play,
        Tab::Accounts,
        Tab::Instances,
        Tab::Skins,
        Tab::Together,
        Tab::Logs,
        Tab::Settings,
        Tab::About,
    ];
}

/// Which log the Logs tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Game,
    Launcher,
}

/// Game log lines kept in memory (older ones are dropped).
const GAME_LOG_LINES: usize = 20_000;

pub enum ManifestState {
    Loading,
    Ready(VersionManifest),
    Failed(String),
}

pub enum LaunchState {
    Idle,
    Preparing {
        progress: ProgressSnapshot,
        meter: RateMeter,
    },
    /// Process started, waiting for the game window.
    Starting {
        game: GameHandle,
    },
    Running {
        game: GameHandle,
    },
}

/// The "Add account" dialog.
pub enum AddAccount {
    Closed,
    Choose,
    Offline {
        name: String,
    },
    Microsoft {
        attempt: LoginAttempt,
        cancel: Arc<AtomicBool>,
        device_code: Option<DeviceCode>,
        browser: bool,
    },
    Failed(String),
}

pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available {
        info: UpdateInfo,
        dismissed: bool,
    },
    Installing {
        info: UpdateInfo,
        done: u64,
        total: u64,
    },
    /// Installed but still running the old binary until restart.
    Installed {
        required: bool,
    },
    /// `retry` is set when an install failed, so a required update keeps
    /// blocking Play and the banner can offer to try again.
    Failed {
        error: String,
        retry: Option<UpdateInfo>,
    },
}

pub struct ArcticApp {
    /// Launcher-wide layout (shared downloads, caches).
    pub(crate) root: DataDirs,
    pub(crate) profiles: ProfileStore,
    /// Layout scoped to the active profile.
    pub(crate) dirs: DataDirs,
    pub(crate) tasks: Tasks,
    events: Receiver<Event>,
    pub(crate) tab: Tab,
    pub(crate) tab_changed_at: f64,
    pub(crate) settings: Settings,
    saved_settings: Settings,
    pub(crate) accounts: AccountStore,
    faces: HashMap<String, Face>,
    pub(crate) instance: Instance,
    pub(crate) custom_instances: Vec<Instance>,
    pub(crate) manifest: ManifestState,
    pub(crate) installed: HashSet<String>,
    pub(crate) launch: LaunchState,
    pub(crate) add_account: AddAccount,
    pub(crate) remove_confirm: Option<String>,
    pub(crate) profile_dialog: ProfileDialog,
    /// Instances tab state (pages, mod browser, create dialog).
    pub(crate) inst: InstancesUi,
    pub(crate) update: UpdateState,
    pub(crate) toasts: Toasts,
    pub(crate) msa_configured: bool,
    pub(crate) version_filter: String,
    pub(crate) installed_only: bool,
    pub(crate) game_log: VecDeque<LogLine>,
    /// Bumped whenever `game_log` changes (for the Logs tab cache).
    pub(crate) game_log_rev: u64,
    pub(crate) launcher_log: Vec<LogLine>,
    pub(crate) launcher_log_seen: u64,
    /// Filtered line indices for the Logs tab, and what they were built for.
    pub(crate) log_cache: Option<(crate::ui::LogViewKey, Vec<usize>)>,
    pub(crate) log_source: LogSource,
    pub(crate) log_min_level: Level,
    pub(crate) log_search: String,
    pub(crate) log_follow: bool,
    minimized_for_game: bool,
    /// (start time, button center) of the Play-press snowflake burst.
    pub(crate) play_burst: Option<(f64, egui::Pos2)>,
    /// Current launch; events from older launches are ignored.
    launch_id: LaunchId,
    /// Game events that overtook their `Launched` event.
    early_game_events: Vec<GameEvent>,
    splash: Splash,
    applied_theme: Option<ThemeMode>,
    theme_fade: Option<crate::theme_fade::ThemeFade>,
    discord: crate::discord::Discord,
    pub(crate) onboarding: Option<crate::ui::Onboarding>,
    pub(crate) together: crate::ui::TogetherUi,
    pub(crate) skins: crate::ui::SkinsUi,
    devshot: crate::devshot::DevShot,
    /// Maximize on the first frame (creating the window maximized is
    /// unreliable on Windows: wrong restore size, flicker).
    maximize_pending: bool,
    /// Launch requested on the command line, run once versions are loaded.
    pending_launch: Option<String>,
    login_attempts: LoginAttempt,
}

impl ArcticApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        root: DataDirs,
        profiles: ProfileStore,
        startup: StartupOptions,
    ) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let dirs = profiles.scoped(&root);
        let tasks = Tasks::new(tx, cc.egui_ctx.clone(), dirs.clone());
        let mut toasts = Toasts::default();
        let data = ProfileData::load(&dirs, &tasks, &mut toasts);
        tasks.load_manifest();
        let update = if data.settings.check_updates_on_start {
            tasks.check_update(data.settings.update_channel);
            UpdateState::Checking
        } else {
            UpdateState::Idle
        };
        let intro = data.settings.intro && !startup.no_intro;
        let mut app = Self {
            splash: Splash::new(intro),
            applied_theme: None,
            theme_fade: None,
            discord: crate::discord::Discord::new(),
            onboarding: None,
            together: crate::ui::TogetherUi::default(),
            skins: crate::ui::SkinsUi::default(),
            devshot: crate::devshot::DevShot::from_env(),
            maximize_pending: data.settings.start_maximized,
            pending_launch: startup.launch.clone(),
            msa_configured: MsaConfig::load(&dirs).is_ok(),
            installed: versions::installed_versions(&dirs),
            custom_instances: Vec::new(),
            saved_settings: data.settings.clone(),
            settings: data.settings.clone(),
            root,
            profiles,
            dirs,
            tasks,
            events: rx,
            tab: startup.tab.unwrap_or(Tab::Play),
            tab_changed_at: 0.0,
            accounts: AccountStore::default(),
            faces: HashMap::new(),
            instance: Instance::vanilla_default(),
            manifest: ManifestState::Loading,
            launch: LaunchState::Idle,
            add_account: AddAccount::Closed,
            remove_confirm: None,
            profile_dialog: ProfileDialog::Closed,
            inst: InstancesUi::default(),
            update,
            toasts,
            version_filter: String::new(),
            installed_only: false,
            game_log: VecDeque::new(),
            game_log_rev: 0,
            launcher_log: Vec::new(),
            launcher_log_seen: 0,
            log_cache: None,
            log_source: LogSource::Game,
            log_min_level: Level::Debug,
            log_search: String::new(),
            log_follow: true,
            minimized_for_game: false,
            play_burst: None,
            launch_id: 0,
            early_game_events: Vec::new(),
            login_attempts: 0,
        };
        app.apply_profile_data(data);
        app.maybe_start_onboarding(startup.launch.is_some());
        if let Some(query) = &startup.account {
            match app.accounts.find(query).map(|a| a.id.clone()) {
                Some(id) => {
                    app.accounts.set_active(&id);
                }
                None => app
                    .toasts
                    .push(Kind::Error, format!("No account named '{query}'"), ""),
            }
        }
        app
    }

    /// Replace all profile-scoped state (used at start and on switch).
    pub(crate) fn apply_profile_data(&mut self, data: ProfileData) {
        self.saved_settings = data.settings.clone();
        self.settings = data.settings;
        self.accounts = data.accounts;
        self.instance = data.instance;
        self.custom_instances = data.custom_instances;
        self.faces = data.faces;
        self.game_log.clear();
        self.game_log_rev += 1;
        self.log_cache = None;
        // Instance pages and mod listings belong to the old profile.
        self.inst = InstancesUi::default();
        // Theme is per profile: force a re-apply on the next frame.
        self.applied_theme = None;
        self.theme_fade = None;
    }

    /// Save settings now if they changed.
    pub(crate) fn persist_settings(&mut self) {
        if self.settings != self.saved_settings {
            match self.settings.save(&self.dirs) {
                Ok(()) => self.saved_settings = self.settings.clone(),
                Err(e) => self
                    .toasts
                    .push(Kind::Error, "Could not save settings", e.to_string()),
            }
        }
    }

    pub(crate) fn palette(&self) -> &'static Palette {
        match &self.theme_fade {
            Some(fade) => fade.palette(self.settings.theme),
            None => theme::palette(self.settings.theme),
        }
    }

    /// Apply theme changes, cross-fading from the previous theme.
    fn update_theme(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let now = ctx.input(|i| i.time);
        let mut changed = false;
        if self.applied_theme != Some(self.settings.theme) {
            // The first apply (startup, profile switch) is instant.
            self.theme_fade = self
                .applied_theme
                .map(|from| crate::theme_fade::ThemeFade::new(from, now));
            self.applied_theme = Some(self.settings.theme);
            changed = true;
        }
        if let Some(fade) = &mut self.theme_fade {
            if !fade.update(now) {
                self.theme_fade = None;
            }
            ctx.request_repaint();
            changed = true;
        }
        if changed {
            theme::apply(ctx, self.palette());
            crate::titlebar::apply(frame, self.palette());
        }
    }

    pub(crate) fn set_tab(&mut self, tab: Tab, now: f64) {
        if self.tab != tab {
            self.tab = tab;
            self.tab_changed_at = now;
        }
    }

    pub(crate) fn face(&self, account: &Account) -> Face {
        self.faces
            .get(&account.uuid)
            .cloned()
            .unwrap_or_else(avatar::default_face)
    }

    pub(crate) fn save_accounts(&mut self) {
        if let Err(e) = self.accounts.save(&self.dirs) {
            self.toasts
                .push(Kind::Error, "Could not save accounts", e.to_string());
        }
    }

    pub(crate) fn add_signed_in(&mut self, account: Account) {
        if account.is_microsoft() {
            self.tasks.fetch_face(account.uuid.clone());
        }
        self.toasts.push(
            Kind::Success,
            format!("Signed in as {}", account.username),
            "",
        );
        self.accounts.upsert(account);
        self.save_accounts();
    }

    /// A required update blocks Play from the moment it is found until the
    /// new version is actually running, including after a failed install.
    pub(crate) fn update_required(&self) -> bool {
        match &self.update {
            UpdateState::Available { info, .. }
            | UpdateState::Installing { info, .. }
            | UpdateState::Failed {
                retry: Some(info), ..
            } => info.required,
            UpdateState::Installed { required } => *required,
            _ => false,
        }
    }

    pub(crate) fn start_update(&mut self, info: UpdateInfo) {
        self.tasks.install_update(info.clone());
        self.update = UpdateState::Installing {
            done: 0,
            total: info.asset_size,
            info,
        };
    }

    pub(crate) fn start_login(&mut self, browser: bool) {
        self.cancel_login();
        self.login_attempts += 1;
        let attempt = self.login_attempts;
        let cancel = Arc::new(AtomicBool::new(false));
        if browser {
            self.tasks.login_browser(attempt, cancel.clone());
        } else {
            self.tasks.login_device_code(attempt, cancel.clone());
        }
        self.add_account = AddAccount::Microsoft {
            attempt,
            cancel,
            device_code: None,
            browser,
        };
    }

    pub(crate) fn cancel_login(&mut self) {
        if let AddAccount::Microsoft { cancel, .. } = &self.add_account {
            cancel.store(true, Ordering::Relaxed);
            self.add_account = AddAccount::Choose;
        }
    }

    fn is_current_login(&self, attempt: LoginAttempt) -> bool {
        matches!(self.add_account, AddAccount::Microsoft { attempt: a, .. } if a == attempt)
    }

    /// Start preparing + launching the selected version for the active account.
    pub(crate) fn launch_selected(&mut self) {
        let ManifestState::Ready(manifest) = &self.manifest else {
            return;
        };
        // Vanilla follows the Play-tab version; custom instances pin theirs.
        let instance = self.selected_instance().clone();
        let version_id = if instance.is_default() {
            self.settings.last_version.clone()
        } else {
            instance.version.clone()
        };
        let Some(version) = version_id
            .as_deref()
            .and_then(|id| manifest.find(id))
            .cloned()
        else {
            self.toasts.push(
                Kind::Error,
                "Unknown Minecraft version",
                format!(
                    "{} needs a version that isn't in Mojang's list.",
                    instance.name
                ),
            );
            return;
        };
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        self.launch_id += 1;
        self.discord.game_started(
            format!("Minecraft {}", version.id),
            instance.loader.label().to_owned(),
        );
        self.early_game_events.clear();
        self.push_game_log(vec![LogLine {
            level: Level::Info,
            text: format!(
                "──── Launching {} ({}) as {} ────",
                instance.name, version.id, account.username
            ),
        }]);
        self.launch = LaunchState::Preparing {
            progress: ProgressSnapshot {
                stage: "Starting".into(),
                ..ProgressSnapshot::default()
            },
            meter: RateMeter::default(),
        };
        self.tasks.launch(
            self.launch_id,
            version,
            instance,
            account,
            self.settings.clone(),
        );
    }

    /// `--launch` from the command line: pick the version, then Play.
    fn run_pending_launch(&mut self) {
        let Some(query) = self.pending_launch.take() else {
            return;
        };
        let ManifestState::Ready(manifest) = &self.manifest else {
            return;
        };
        let id = match query.as_str() {
            "latest" | "latest-release" => Some(manifest.latest.release.clone()),
            "latest-snapshot" => Some(manifest.latest.snapshot.clone()),
            other => manifest.find(other).map(|v| v.id.clone()),
        };
        match id {
            Some(id) if self.accounts.active().is_some() && !self.update_required() => {
                self.settings.last_version = Some(id);
                self.launch_selected();
            }
            Some(id) => {
                self.settings.last_version = Some(id);
                self.toasts
                    .push(Kind::Info, "Choose an account to play", "");
            }
            None => self
                .toasts
                .push(Kind::Error, format!("Unknown version '{query}'"), ""),
        }
    }

    fn handle_event(&mut self, event: Event, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        match event {
            Event::Manifest(Ok(m)) => {
                if self.settings.last_version.is_none() {
                    self.settings.last_version = Some(m.latest.release.clone());
                }
                self.manifest = ManifestState::Ready(m);
                self.run_pending_launch();
            }
            Event::Manifest(Err(e)) => self.manifest = ManifestState::Failed(e),
            Event::LaunchProgress(snapshot) => {
                if let LaunchState::Preparing { progress, meter } = &mut self.launch {
                    meter.push(now, snapshot.bytes_done);
                    *progress = snapshot;
                }
            }
            Event::UpdateProgress(p) => {
                if let UpdateState::Installing { done, total, .. } = &mut self.update {
                    (*done, *total) = (p.bytes_done, p.bytes_total);
                }
            }
            Event::AccountRefreshed(account) => {
                self.accounts.upsert(account);
                self.save_accounts();
            }
            Event::Launched(id, _) | Event::Game(id, _) if id != self.launch_id => {}
            Event::Launched(_, Ok((game, _log_file, version))) => {
                self.installed.insert(version);
                self.launch = LaunchState::Starting { game };
                // Replay events from a game that was faster than our bookkeeping.
                for event in std::mem::take(&mut self.early_game_events) {
                    self.on_game_event(event, ctx);
                }
            }
            Event::Launched(_, Err(e)) => {
                self.launch = LaunchState::Idle;
                self.toasts.push(Kind::Error, "Launch failed", e);
            }
            Event::Game(_, event) if matches!(self.launch, LaunchState::Preparing { .. }) => {
                self.early_game_events.push(event);
            }
            Event::Game(_, event) => self.on_game_event(event, ctx),
            Event::DeviceCode(attempt, code) => {
                if let AddAccount::Microsoft {
                    attempt: current,
                    device_code,
                    ..
                } = &mut self.add_account
                    && *current == attempt
                {
                    *device_code = Some(code);
                }
            }
            // Results from cancelled or superseded attempts are ignored.
            Event::LoginFinished(attempt, _) if !self.is_current_login(attempt) => {}
            Event::LoginFinished(_, Ok(account)) => {
                self.add_account = AddAccount::Closed;
                self.add_signed_in(account);
            }
            Event::LoginFinished(_, Err(e)) => self.add_account = AddAccount::Failed(e),
            Event::Face(uuid, face) => {
                self.faces.insert(uuid, face);
            }
            e @ (Event::LoaderGames(..)
            | Event::LoaderVersions(..)
            | Event::ModSearch(..)
            | Event::ModProgress(..)
            | Event::ModInstalled(..)
            | Event::ModIcon(..)) => self.on_instances_event(e, ctx),
            Event::Share(id, event) => self.on_share_event(id, event),
            e @ (Event::SkinAccount(..) | Event::PlayerSkin(..) | Event::SkinFile(..)) => {
                self.on_skins_event(e)
            }
            Event::UpdateChecked(result) => self.on_update_checked(result),
            Event::UpdateInstalled(result) => self.on_update_installed(result),
        }
    }

    fn on_game_event(&mut self, event: GameEvent, ctx: &egui::Context) {
        match event {
            GameEvent::WindowReady => {
                let LaunchState::Starting { game } =
                    std::mem::replace(&mut self.launch, LaunchState::Idle)
                else {
                    return;
                };
                self.launch = LaunchState::Running { game };
                if self.settings.on_game_start == GameStartAction::Minimize {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.minimized_for_game = true;
                }
            }
            GameEvent::Output(lines) => self.push_game_log(lines),
            GameEvent::Exited { code } => {
                self.launch = LaunchState::Idle;
                if self.minimized_for_game {
                    self.minimized_for_game = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                match code {
                    Some(0) => {}
                    Some(c) => self.toasts.push_with_action(
                        Kind::Error,
                        "Minecraft closed unexpectedly",
                        format!("Exit code {c}."),
                        Some(ToastAction::ShowLogs),
                    ),
                    None => self.toasts.push(Kind::Info, "Minecraft was stopped", ""),
                }
            }
        }
    }

    fn push_game_log(&mut self, lines: Vec<LogLine>) {
        self.game_log_rev += 1;
        self.game_log.extend(lines);
        let excess = self.game_log.len().saturating_sub(GAME_LOG_LINES);
        self.game_log.drain(..excess);
    }

    fn on_update_checked(&mut self, result: Result<Option<UpdateInfo>, String>) {
        self.update = match result {
            Ok(Some(info)) => UpdateState::Available {
                info,
                dismissed: false,
            },
            Ok(None) => UpdateState::UpToDate,
            Err(error) => {
                log::warn!("update check failed: {error}");
                UpdateState::Failed { error, retry: None }
            }
        };
    }

    fn on_update_installed(&mut self, result: Result<(), String>) {
        let UpdateState::Installing { info, .. } = &self.update else {
            return;
        };
        self.update = match result {
            Ok(()) => UpdateState::Installed {
                required: info.required,
            },
            Err(error) => UpdateState::Failed {
                error,
                retry: Some(info.clone()),
            },
        };
    }

    /// Persist settings once the user stops dragging a control.
    fn autosave_settings(&mut self, ctx: &egui::Context) {
        let dragging = ctx.input(|i| i.pointer.any_down());
        if !dragging {
            self.persist_settings();
        }
    }

    /// Schedule the next frame for the animated backdrop. Must run inside
    /// the UI pass (repaint requests made outside it are dropped).
    fn pace_scenery(&self, ctx: &egui::Context) {
        if !self.settings.animations {
            return;
        }
        let focused = ctx.input(|i| i.focused);
        let game_running = matches!(
            self.launch,
            LaunchState::Starting { .. } | LaunchState::Running { .. }
        );
        match (focused, game_running) {
            (true, _) => ctx.request_repaint_after(FRAME_FOCUSED),
            (false, false) => ctx.request_repaint_after(FRAME_UNFOCUSED),
            // Game has focus: stay idle so it gets the CPU/GPU.
            (false, true) => {}
        }
    }
}

impl eframe::App for ArcticApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        while let Ok(event) = self.events.try_recv() {
            self.handle_event(event, &ctx);
        }
        if std::mem::take(&mut self.maximize_pending) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        self.update_theme(&ctx, frame);
        let playing = !matches!(self.launch, LaunchState::Idle);
        self.discord.sync(self.settings.discord_presence, playing);
        self.shell(ui);
        self.splash.show(&ctx, self.palette());
        self.autosave_settings(&ctx);
        self.pace_scenery(&ctx);
        self.devshot.update(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_login();
        if self.settings != self.saved_settings {
            let _ = self.settings.save(&self.dirs);
        }
    }
}
