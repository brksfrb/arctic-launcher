//! Application state and event handling. Layout lives in `ui/`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use arctic_core::auth::avatar::{self, Face};
use arctic_core::auth::microsoft::{DeviceCode, MsaConfig};
use arctic_core::auth::{Account, AccountStore};
use arctic_core::instances::Instance;
use arctic_core::launch::logparse::{Level, LogLine};
use arctic_core::profiles::ProfileStore;
use arctic_core::settings::{Settings, ThemeMode};
use arctic_core::storage::DataDirs;
use arctic_core::update::UpdateInfo;
use arctic_core::versions::{self, VersionManifest};
use eframe::egui;

use crate::art::splash::Splash;
use crate::session::ProfileData;
use crate::startup::StartupOptions;
use crate::tasks::{Event, LaunchId, LoginAttempt, Tasks};
use crate::theme::{self, Palette};
use crate::toasts::{Kind, Toasts};
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
    Screenshots,
    Logs,
    Settings,
    About,
}

impl Tab {
    pub const ALL: [Tab; 9] = [
        Tab::Play,
        Tab::Accounts,
        Tab::Instances,
        Tab::Skins,
        Tab::Together,
        Tab::Screenshots,
        Tab::Logs,
        Tab::Settings,
        Tab::About,
    ];
}

/// Which log the Logs tab shows: a game run's, or the launcher's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Game(LaunchId),
    Launcher,
}

pub enum ManifestState {
    Loading,
    Ready(VersionManifest),
    Failed(String),
}

/// The "Add account" dialog.
pub enum AddAccount {
    Closed,
    Choose,
    #[cfg(feature = "offline-accounts")]
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
    /// Saved proxy settings (the Network section edits a draft of them).
    pub(crate) proxy: arctic_core::proxy::ProxySettings,
    pub(crate) network: crate::ui::NetworkUi,
    /// Answers the running game's account switcher.
    pub(crate) bridge: crate::bridge_host::BridgeHost,
    /// The running sign-in was started from the game (it's told how it went).
    pub(crate) login_from_game: bool,
    pub(crate) voice: crate::voice::VoiceHub,
    pub(crate) voice_ui: crate::ui::VoiceSettingsUi,
    pub(crate) accounts: AccountStore,
    faces: HashMap<String, Face>,
    pub(crate) instance: Instance,
    pub(crate) custom_instances: Vec<Instance>,
    pub(crate) manifest: ManifestState,
    pub(crate) installed: HashSet<String>,
    pub(crate) add_account: AddAccount,
    pub(crate) remove_confirm: Option<String>,
    /// Instance whose performance mods are about to be turned off (asks first).
    pub(crate) performance_off_confirm: Option<String>,
    pub(crate) profile_dialog: ProfileDialog,
    /// Instances tab state (pages, mod browser, create dialog).
    pub(crate) inst: InstancesUi,
    pub(crate) update: UpdateState,
    pub(crate) toasts: Toasts,
    pub(crate) msa_configured: bool,
    pub(crate) version_filter: String,
    pub(crate) version_view: crate::ui::VersionView,
    /// Games started from here (several can run at once).
    pub(crate) runs: crate::runs::Runs,
    pub(crate) launcher_log: Vec<LogLine>,
    pub(crate) launcher_log_seen: u64,
    /// Filtered line indices for the Logs tab, and what they were built for.
    pub(crate) log_cache: Option<(crate::ui::LogViewKey, Vec<usize>)>,
    pub(crate) log_source: LogSource,
    pub(crate) log_min_level: Level,
    pub(crate) log_search: String,
    pub(crate) log_follow: bool,
    pub(crate) minimized_for_game: bool,
    /// (start time, button center) of the Play-press snowflake burst.
    pub(crate) play_burst: Option<(f64, egui::Pos2)>,
    splash: Splash,
    applied_theme: Option<ThemeMode>,
    theme_fade: Option<crate::theme_fade::ThemeFade>,
    /// Your own Minecraft behind the launcher (see `Settings::backdrop`).
    pub(crate) world_backdrop: crate::art::world::WorldBackdrop,
    /// The Play tab's recent worlds.
    pub(crate) recent_worlds: crate::ui::play_worlds::RecentWorlds,
    discord: crate::discord::Discord,
    pub(crate) onboarding: Option<crate::ui::Onboarding>,
    pub(crate) together: crate::ui::TogetherUi,
    pub(crate) sharing: crate::ui::ShareUi,
    pub(crate) shots: crate::ui::ScreenshotsUi,
    pub(crate) servers: crate::ui::ServersUi,
    pub(crate) discover: crate::ui::DiscoverUi,
    pub(crate) friends: crate::ui::FriendsUi,
    pub(crate) chat: crate::ui::ChatUi,
    pub(crate) game_defaults: crate::ui::DefaultsUi,
    pub(crate) migrate: Option<crate::ui::MigrateUi>,
    pub(crate) skins: crate::ui::SkinsUi,
    devshot: crate::devshot::DevShot,
    /// Command lines from later starts (see `single_instance`).
    forwarded: std::sync::mpsc::Receiver<Vec<String>>,
    tray: Option<crate::tray::Tray>,
    window_known: bool,
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
        forwarded: std::sync::mpsc::Receiver<Vec<String>>,
    ) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::fonts::install_ui(&cc.egui_ctx);
        // Ctrl +/- would scale the whole UI; the layout isn't made for that.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (tx, rx) = mpsc::channel();
        let dirs = profiles.scoped(&root);
        let tasks = Tasks::new(tx, cc.egui_ctx.clone(), dirs.clone());
        let bridge = crate::bridge_host::BridgeHost::start(&tasks);
        let tasks = tasks.with_bridge(bridge.info.clone());
        let mut toasts = Toasts::default();
        let data = ProfileData::load(&dirs, &tasks, &mut toasts);
        let voice = crate::voice::VoiceHub::new(dirs.clone(), data.settings.voice.clone());
        bridge.set_voice(voice.clone());
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
            world_backdrop: Default::default(),
            recent_worlds: Default::default(),
            discord: crate::discord::Discord::new(),
            onboarding: None,
            together: crate::ui::TogetherUi::default(),
            sharing: crate::ui::ShareUi::default(),
            shots: crate::ui::ScreenshotsUi::default(),
            servers: crate::ui::ServersUi::default(),
            discover: crate::ui::DiscoverUi::default(),
            friends: crate::ui::FriendsUi::default(),
            chat: crate::ui::ChatUi::default(),
            game_defaults: crate::ui::DefaultsUi::default(),
            migrate: None,
            skins: crate::ui::SkinsUi::default(),
            devshot: crate::devshot::DevShot::from_env(),
            forwarded,
            tray: None,
            window_known: false,
            maximize_pending: data.settings.start_maximized,
            pending_launch: startup.launch.clone(),
            msa_configured: MsaConfig::load(&dirs).is_ok(),
            installed: versions::installed_versions(&dirs),
            custom_instances: Vec::new(),
            saved_settings: data.settings.clone(),
            settings: data.settings.clone(),
            proxy: Default::default(),
            network: Default::default(),
            bridge,
            login_from_game: false,
            voice,
            voice_ui: crate::ui::VoiceSettingsUi::default(),
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
            add_account: AddAccount::Closed,
            remove_confirm: None,
            performance_off_confirm: None,
            profile_dialog: ProfileDialog::Closed,
            inst: InstancesUi::default(),
            update,
            toasts,
            version_filter: String::new(),
            version_view: crate::ui::VersionView::All,
            runs: crate::runs::Runs::default(),
            launcher_log: Vec::new(),
            launcher_log_seen: 0,
            log_cache: None,
            log_source: LogSource::Launcher,
            log_min_level: Level::Debug,
            log_search: String::new(),
            log_follow: true,
            minimized_for_game: false,
            play_burst: None,
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
    /// Settings and instances were changed on disk (a profile import).
    pub(crate) fn reload_after_import(&mut self) {
        if let Ok(settings) = Settings::load(&self.dirs) {
            self.saved_settings = settings.clone();
            self.settings = settings;
        }
        if let Ok(vanilla) = arctic_core::instances::load_default(&self.dirs) {
            self.instance = vanilla;
        }
        self.reload_instances();
    }

    pub(crate) fn apply_profile_data(&mut self, data: ProfileData) {
        self.saved_settings = data.settings.clone();
        self.settings = data.settings;
        self.accounts = data.accounts;
        self.instance = data.instance;
        self.custom_instances = data.custom_instances;
        self.faces = data.faces;
        self.network = crate::ui::NetworkUi {
            draft: data.proxy.clone(),
            ..Default::default()
        };
        self.proxy = data.proxy;
        self.bridge.update(&self.accounts, &self.tasks);
        self.runs.clear();
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
        self.bridge.update(&self.accounts, &self.tasks);
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
        self.offer_profile_link(&account);
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
        self.launch_selected_as(false, None);
    }

    /// Start the selected instance straight into a server or world.
    pub(crate) fn launch_into(&mut self, target: arctic_core::launch::QuickPlay) {
        if let arctic_core::launch::QuickPlay::Server(address) = &target {
            self.friends.my_server = Some(address.clone());
        }
        if let arctic_core::launch::QuickPlay::Duel { friend, .. } = &target {
            let who = friend.as_ref().map_or("a friend", |f| f.1.as_str());
            self.toasts.push(
                crate::toasts::Kind::Info,
                "Starting a duel",
                format!("The game makes an arena and invites {who}."),
            );
        }
        self.launch_selected_as(false, Some(target));
    }

    /// Start the selected instance again while it runs (a second account,
    /// say); each copy gets its own log.
    pub(crate) fn launch_another_copy(&mut self) {
        self.launch_selected_as(true, None);
    }

    fn launch_selected_as(
        &mut self,
        another_copy: bool,
        quick_play: Option<arctic_core::launch::QuickPlay>,
    ) {
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
        let copy = self
            .runs
            .active()
            .filter(|r| r.instance_id == instance.id)
            .count() as u32;
        // Vanilla on another version is a different game, not another copy.
        let same = instance.is_default().then_some(version.id.as_str());
        if self.runs.active_for(&instance.id, same).is_some() && !another_copy {
            self.toasts.push(
                Kind::Info,
                format!("{} is already running", instance.name),
                "Pick another instance to play more at once.",
            );
            return;
        }
        self.discord.game_started(
            format!("Minecraft {}", version.id),
            instance.loader.label().to_owned(),
        );
        let id = self.runs.start(
            &instance.id,
            &version.id,
            format!("{} · {}", instance.name, version.id),
            instance.game_dir(&self.dirs),
        );
        self.run_note(
            id,
            format!(
                "──── Launching {} ({}) as {} ────",
                instance.name, version.id, account.username
            ),
        );
        self.log_source = LogSource::Game(id);
        let start = crate::tasks::StartAt { copy, quick_play };
        self.tasks
            .launch(id, version, instance, account, self.settings.clone(), start);
    }

    /// Tray icon, close-to-tray and starts forwarded from other processes.
    fn window_and_tray(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if !self.window_known {
            self.window_known = true;
            crate::window::remember(frame);
            let wake_ctx = ctx.clone();
            crate::single_instance::on_wake(move || {
                crate::window::show();
                wake_ctx.request_repaint();
            });
        }
        if self.settings.tray && self.tray.is_none() && crate::window::is_known() {
            self.tray = crate::tray::Tray::new(ctx);
        }
        let tray_on = self.settings.tray && self.tray.is_some();
        if tray_on && !crate::tray::quitting() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.persist_settings();
            crate::window::hide();
        }
        let now = ctx.input(|i| i.time);
        let actions: Vec<_> = self
            .tray
            .as_ref()
            .map(|t| t.actions.try_iter().collect())
            .unwrap_or_default();
        for action in actions {
            match action {
                crate::tray::Action::Play => {
                    self.set_tab(Tab::Play, now);
                    if !self.runs.instance_active(&self.selected_instance().id) {
                        self.launch_selected();
                    }
                }
            }
        }
        while let Ok(args) = self.forwarded.try_recv() {
            let opts = StartupOptions::parse(args);
            if let Some(tab) = opts.tab {
                self.set_tab(tab, now);
            }
            if let Some(query) = &opts.account
                && let Some(id) = self.accounts.find(query).map(|a| a.id.clone())
            {
                self.accounts.set_active(&id);
            }
            if opts.launch.is_some() {
                self.pending_launch = opts.launch;
                self.run_pending_launch();
            }
        }
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
            Event::LaunchProgress(id, snapshot) => self.on_launch_progress(id, snapshot, now),
            Event::UpdateProgress(p) => {
                if let UpdateState::Installing { done, total, .. } = &mut self.update {
                    (*done, *total) = (p.bytes_done, p.bytes_total);
                }
            }
            Event::AccountRefreshed(account) => {
                self.accounts.upsert(account);
                self.save_accounts();
            }
            Event::Launched(id, result) => {
                let result = result.map(|(game, _log_file, version)| {
                    self.installed.insert(version);
                    game
                });
                self.on_launched(id, result, ctx);
            }
            Event::Game(id, event) => self.on_game_event(id, event, ctx),
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
                if std::mem::take(&mut self.login_from_game) {
                    self.bridge
                        .set_login_status("done", Some(&account.username), None);
                }
                self.add_signed_in(account);
                self.bridge.update(&self.accounts, &self.tasks);
            }
            Event::LoginFinished(_, Err(e)) => {
                if std::mem::take(&mut self.login_from_game) {
                    self.bridge
                        .set_login_status("failed", None, Some(e.as_str()));
                }
                self.add_account = AddAccount::Failed(e);
            }
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
            Event::WorldsDone(id, result) => self.on_worlds_done(id, result),
            Event::ModpackInstalled(result) => self.on_modpack_installed(result),
            Event::MigrateScanned(folder, result) => {
                self.on_migrate_scanned(folder.is_some(), result)
            }
            Event::MigrateProgress(label, p) => self.on_migrate_progress(label, p),
            Event::MigrateItemDone(outcome) => self.on_migrate_item(outcome),
            Event::MigrateFinished => self.on_migrate_finished(),
            Event::PackSearch(request, result) => self.on_pack_search(request, result),
            Event::PackInstalled(instance, project, result) => {
                self.on_pack_installed(instance, project, result)
            }
            Event::ServerList(request, list) => self.on_server_list(ctx, request, list),
            Event::ServerStatus(request, address, status) => {
                self.on_server_status(ctx, request, address, status)
            }
            Event::Friends(result) => self.on_friends(result),
            Event::FriendDone(result) => self.on_friend_done(result),
            #[cfg(feature = "offline-accounts")]
            Event::RecoveryCode(result) => self.on_recovery_code(result),
            Event::ChatHistory(friend, result) => self.on_chat_history(friend, result),
            Event::ChatSent(result) => self.on_chat_sent(result),
            Event::ChatImage(id, result) => self.on_chat_image(ctx, id, result),
            Event::ChatNew(after, result) => self.on_chat_new(after, result),
            Event::PublicServers(list) => self.on_public_servers(list),
            Event::ServerSubmitted(result) => self.on_server_submitted(result),
            Event::ServerVerified(result) => self.on_server_verified(result),
            Event::Screenshots(list) => self.on_screenshot_list(list),
            Event::ScreenshotThumb(path, result) => self.on_screenshot_thumb(ctx, path, result),
            Event::ScreenshotCopied(result) => self.on_screenshot_copied(ctx, result),
            Event::WearSkinFromGame(id) => self.wear_skin_from_game(id),
            Event::AddAccountFromGame => {
                // Straight to Microsoft's sign-in in the browser; the game
                // follows along over the bridge, the launcher stays hidden.
                if !matches!(self.add_account, AddAccount::Microsoft { .. }) {
                    self.start_login(true);
                }
                self.login_from_game = true;
                self.bridge.set_login_status("waiting", None, None);
            }
            Event::ShareCode(result) => self.on_share_code(result, ctx),
            Event::ShareSaved(result) => self.on_share_saved(result),
            Event::ShareLoaded(result) => self.on_share_loaded(result),
            Event::ShareProgress(p) => self.sharing.progress = Some(p),
            Event::ShareImported(result) => self.on_share_imported(result, now),
            e @ (Event::SkinAccount(..)
            | Event::PlayerSkin(..)
            | Event::SkinFile(..)
            | Event::ArcticLook(..)
            | Event::CapeFile(..)
            | Event::Gallery(..)
            | Event::GalleryTaken(..)
            | Event::GalleryDone(..)) => self.on_skins_event(e),
            Event::UpdateChecked(result) => self.on_update_checked(result),
            Event::ProxyTested(proxy, result) => self.on_proxy_tested(proxy, result),
            Event::UpdateInstalled(result) => self.on_update_installed(result),
        }
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
        let game_running = self.runs.any_game();
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
        self.window_and_tray(&ctx, frame);
        let playing = self.runs.any_active();
        let together = self.together_status();
        self.discord
            .sync(self.settings.discord_presence, playing, together);
        self.shell(ui);
        self.splash.show(&ctx, self.palette());
        self.autosave_settings(&ctx);
        self.pace_scenery(&ctx);
        if let Some(muted) = self.voice.take_muted() {
            self.settings.voice.muted = muted;
            self.persist_settings();
        }
        self.voice
            .update(&self.dirs, &self.settings.voice, self.accounts.active());
        self.voice.tick();
        self.together_bridge();
        if std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() == Ok("modpacks")
            && crate::devshot::enabled()
            && !self.inst.modpacks_open
            && self.tab != Tab::Instances
        {
            self.set_tab(Tab::Instances, 0.0);
            self.inst.modpacks_open = true;
        }
        if crate::devshot::enabled() {
            let tab = match std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() {
                Ok("app-settings") => Some(Tab::Settings),
                Ok("skins" | "gallery") => Some(Tab::Skins),
                _ => None,
            };
            if let Some(tab) = tab.filter(|t| *t != self.tab) {
                self.set_tab(tab, 0.0);
            }
        }
        if let Some(tab) = self
            .devshot
            .open_tab
            .take()
            .and_then(|i| Tab::ALL.get(i).copied())
        {
            self.set_tab(tab, 0.0);
        }
        if let Some(id) = self.devshot.open_instance.take() {
            self.set_tab(Tab::Instances, 0.0);
            self.open_instance(&id);
            match std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() {
                Ok("settings") => self.inst.page = crate::ui::InstancePage::Settings,
                Ok("replays") => self.inst.page = crate::ui::InstancePage::Replays,
                _ => {}
            }
        }
        if matches!(self.manifest, ManifestState::Ready(_)) {
            if let Some(address) = self.devshot.join.take() {
                self.launch_into(arctic_core::launch::QuickPlay::Server(address));
            } else if let Some(how) = self.devshot.launch.take() {
                if how == "duel" {
                    self.launch_into(arctic_core::launch::QuickPlay::Duel {
                        kit: "sword".into(),
                        friend: None,
                    });
                } else {
                    self.launch_selected();
                }
            }
        }
        self.devshot_share();
        self.devshot.update(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_login();
        if self.settings.exit_games_with_launcher {
            for run in self.runs.active() {
                if let Some(game) = run.game() {
                    game.terminate();
                }
            }
        }
        if self.settings != self.saved_settings {
            let _ = self.settings.save(&self.dirs);
        }
    }
}
