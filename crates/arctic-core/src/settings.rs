//! User settings persisted to `settings.json`. Contains no secrets.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};
use crate::update::UpdateChannel;

pub const MIN_MEMORY_MB: u32 = 512;
pub const DEFAULT_MAX_MEMORY_MB: u32 = 4096;
pub const DEFAULT_WIDTH: u32 = 854;
pub const DEFAULT_HEIGHT: u32 = 480;
const MIN_WIDTH: u32 = 320;
const MIN_HEIGHT: u32 = 240;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `-Xmx` in MiB.
    pub max_memory_mb: u32,
    /// `-Xms` in MiB.
    pub min_memory_mb: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    /// Maximize the game window once it opens.
    pub game_maximized: bool,
    /// Prepare and start the game without using everything the PC has: a few downloads at a
    /// time and the game starting at low priority, for computers that freeze under load.
    pub gentle_launch: bool,
    /// On by default: a launcher waiting in the tray restarts into an installed update by itself.
    /// Off: use the update on the next launch, or with "Restart now" in Settings.
    pub restart_hidden_for_updates: bool,
    /// Whether the one-time quiet-update default migration has been applied.
    #[serde(default)]
    pub quiet_updates_migrated: bool,
    /// Extra JVM flags, whitespace separated.
    pub extra_jvm_args: String,
    /// Use this `java(w).exe` instead of a managed runtime.
    pub java_override: Option<PathBuf>,
    /// Version last selected on the Play tab (Vanilla instance).
    pub last_version: Option<String>,
    /// Instance last selected on the Play tab (`None` = Vanilla).
    pub last_instance: Option<String>,
    /// Offer snapshots in version pickers.
    pub show_snapshots: bool,
    /// Offer old alpha/beta versions in version pickers.
    pub show_old_versions: bool,
    pub update_channel: UpdateChannel,
    pub check_updates_on_start: bool,
    /// New versions download and install by themselves; the launcher switches to them when it
    /// restarts, or right away while it waits hidden in the tray with no game running.
    pub auto_install_updates: bool,
    /// The launcher version that last ran (to say "Updated to …" once after an update).
    pub launcher_version_seen: String,
    /// After an update, show what's new and what's fixed in a popup.
    pub show_release_notes: bool,
    /// The third-party client loaded into Bedrock when it is started from the launcher (`latite`,
    /// `flarial`), or none.
    pub bedrock_client: Option<String>,
    /// Animated backdrop (aurora, snow, shooting stars).
    pub animations: bool,
    /// Animated capes stand still on their first frame in the launcher (a
    /// viewer's choice; it doesn't change what anyone else sees).
    pub reduce_cape_motion: bool,
    /// The 3D player preview (Skins, Cosmetics) never turns by itself; only dragging turns it.
    pub lock_preview_rotation: bool,
    /// Ids of animated capes whose wearer turned "Animate" off: the still
    /// image is published instead. Capes not listed animate.
    pub capes_still: Vec<String>,
    /// Short snowflake intro when the launcher opens.
    pub intro: bool,
    /// Open the launcher window maximized.
    pub start_maximized: bool,
    pub theme: ThemeMode,
    /// What's behind the launcher.
    pub backdrop: Backdrop,
    /// The image used when the backdrop is [`Backdrop::Custom`].
    pub custom_backdrop: Option<PathBuf>,
    pub on_game_start: GameStartAction,
    /// What happens to a crash report when the game crashes.
    pub crash_reports: CrashReports,
    /// How to reach the player about a crash they reported (a Discord name or an e-mail): optional, sent with
    /// the report.
    pub crash_contact: String,
    /// Show what you're playing on Discord.
    pub discord_presence: bool,
    /// Also show the server address on Discord. Off unless chosen: an IP,
    /// localhost, or a private network is still written as "Private server".
    /// Ignored while [`Settings::discord_presence`] is off.
    pub discord_show_server: bool,
    /// First-run setup finished (or skipped) for this profile.
    pub onboarded: bool,
    /// Keep running in the system tray when the window is closed.
    pub tray: bool,
    /// The first-close question about the tray was asked (or the choice was made in setup).
    pub tray_asked: bool,
    /// The setup's "how did you hear about us?" question was shown (it is asked once).
    pub heard_asked: bool,
    /// Versions starred in the version picker.
    pub favorite_versions: Vec<String>,
    /// Menu style of the Arctic Client in game.
    pub client_style: ClientStyle,
    /// When `client_style` was picked (Unix seconds; 0 = never). The game
    /// adopts a newer pick, but keeps a style changed in game until then.
    pub client_style_set: u64,
    /// Fancy mode of the Arctic Client: smooth font and rounded shapes.
    /// Picked together with `client_style` (same timestamp).
    pub client_fancy: bool,
    /// Friends may see which server you're on (last known from the Arctic
    /// server; until then, the game doesn't say).
    pub share_server_with_friends: bool,
    /// Ask before starting Minecraft just to join a server (the dialog's "Don't show again" turns it off).
    pub confirm_join_launch: bool,
    /// Quitting the launcher also closes games it started (off: they keep running).
    pub exit_games_with_launcher: bool,
    /// Mentions of Arctic's supporter (About, playing together).
    pub show_sponsor: bool,
    /// A "Quick tip" card in the sidebar: one feature at a time.
    pub quick_tips: bool,
    /// Games start with the Arctic Client (the mod). Off: no instance gets it, whatever its own switch says.
    pub play_with_client: bool,
    /// How big the launcher's interface is, in percent on top of the system's display scaling;
    /// 0 is Auto (a little smaller on crowded screens, see arctic-app's zoom.rs).
    pub interface_size: u16,
    /// Laptops with two graphics cards: run the game on the high-performance
    /// one (Windows' per-program choice; one the player made is kept).
    pub high_performance_gpu: bool,
    /// Proximity voice chat.
    pub voice: VoiceSettings,
    /// What this profile's instances share (server list, client and game settings).
    pub shared: crate::shared::SharedSettings,
}

/// Proximity voice chat: off until turned on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    /// Push-to-talk (the key is set in game); otherwise voice activation.
    pub push_to_talk: bool,
    /// Voice activation threshold, dBFS.
    pub threshold_db: f32,
    /// Device names; `None` = the system default.
    pub input: Option<String>,
    pub output: Option<String>,
    /// Everyone's volume and your microphone's (1 = unchanged).
    pub volume: f32,
    pub mic_gain: f32,
    /// Only hear (and be heard by) friends.
    pub friends_only: bool,
    /// Players (UUIDs) you muted.
    pub muted: Vec<String>,
    /// Also talk with Simple Voice Chat players on servers that run it.
    pub simple_voice_chat: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            push_to_talk: false,
            threshold_db: -45.0,
            input: None,
            output: None,
            volume: 1.0,
            mic_gain: 1.0,
            friends_only: false,
            muted: Vec::new(),
            simple_voice_chat: false,
        }
    }
}

/// How the Arctic Client styles Minecraft's menus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientStyle {
    /// Deep night blue with ice accents.
    #[default]
    Arctic,
    /// Violet sky with northern-light greens.
    Aurora,
    /// Plain black and white, no colors.
    Classic,
    /// Minecraft's own menus (the Arctic HUD still works).
    Vanilla,
}

impl ClientStyle {
    pub const ALL: [ClientStyle; 4] = [Self::Arctic, Self::Aurora, Self::Classic, Self::Vanilla];

    /// The id the Arctic mod knows the style by.
    pub fn id(self) -> &'static str {
        match self {
            Self::Arctic => "arctic",
            Self::Aurora => "aurora",
            Self::Classic => "classic",
            Self::Vanilla => "vanilla",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Arctic => "Arctic",
            Self::Aurora => "Aurora",
            Self::Classic => "Classic",
            Self::Vanilla => "Vanilla",
        }
    }
}

/// What the launcher window does once the game window is up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStartAction {
    #[default]
    KeepOpen,
    /// Minimize while playing, restore when the game closes.
    Minimize,
}

/// Whether crash reports are sent to Arctic: never on its own unless chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrashReports {
    /// Offer to send each one.
    #[default]
    Ask,
    Always,
    Never,
}

/// The launcher's background.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    /// The painted arctic night (aurora, mountains, snow).
    // "world" was saved by a test build that made the picture the default.
    #[default]
    #[serde(alias = "world")]
    Scenery,
    /// Your own Minecraft: the newest screenshot from the instance, or the
    /// title screen panorama of its version, dimmed.
    Picture,
    /// An image the player chose ([`Settings::custom_backdrop`]), dimmed like the screenshot.
    Custom,
}

/// Launcher color theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Aurora night.
    #[default]
    Default,
    Dark,
    Light,
    /// The aurora night through frosted glass cards.
    Glass,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_memory_mb: DEFAULT_MAX_MEMORY_MB,
            min_memory_mb: MIN_MEMORY_MB,
            window_width: DEFAULT_WIDTH,
            window_height: DEFAULT_HEIGHT,
            fullscreen: false,
            game_maximized: false,
            gentle_launch: false,
            restart_hidden_for_updates: true,
            quiet_updates_migrated: true,
            extra_jvm_args: String::new(),
            java_override: None,
            last_version: None,
            last_instance: None,
            show_snapshots: false,
            show_old_versions: false,
            update_channel: UpdateChannel::Stable,
            check_updates_on_start: true,
            auto_install_updates: true,
            launcher_version_seen: String::new(),
            show_release_notes: true,
            bedrock_client: None,
            animations: true,
            reduce_cape_motion: false,
            lock_preview_rotation: false,
            capes_still: Vec::new(),
            intro: true,
            start_maximized: false,
            theme: ThemeMode::Default,
            backdrop: Backdrop::Scenery,
            custom_backdrop: None,
            on_game_start: GameStartAction::KeepOpen,
            crash_reports: CrashReports::Ask,
            crash_contact: String::new(),
            discord_presence: true,
            discord_show_server: false,
            onboarded: false,
            tray: true,
            tray_asked: false,
            heard_asked: false,
            favorite_versions: Vec::new(),
            client_style: ClientStyle::Arctic,
            client_style_set: 0,
            client_fancy: false,
            share_server_with_friends: false,
            confirm_join_launch: true,
            exit_games_with_launcher: false,
            show_sponsor: true,
            quick_tips: true,
            play_with_client: true,
            interface_size: 0,
            high_performance_gpu: true,
            voice: VoiceSettings::default(),
            shared: crate::shared::SharedSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(dirs: &DataDirs) -> Result<Self> {
        Self::load_with_save(dirs, |settings| settings.save(dirs))
    }

    fn load_with_save(dirs: &DataDirs, save: impl FnOnce(&Self) -> Result<()>) -> Result<Self> {
        let mut settings = load_json::<Self>(&dirs.settings_file())?
            .unwrap_or_default()
            .sanitized();
        if !settings.quiet_updates_migrated {
            settings.restart_hidden_for_updates = true;
            settings.quiet_updates_migrated = true;
            if let Err(error) = save(&settings) {
                // Keep the loaded preferences usable, and retry migration on the next load/save.
                settings.quiet_updates_migrated = false;
                log::warn!("couldn't persist quiet-update settings migration: {error}");
            }
        }
        Ok(settings)
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        let mut settings = self.clone().sanitized();
        // A successful save commits the migration while preserving any later user choice.
        settings.quiet_updates_migrated = true;
        save_json(&dirs.settings_file(), &settings)
    }

    /// Clamp values into ranges the JVM / game will accept.
    pub fn sanitized(self) -> Self {
        let max_memory_mb = self.max_memory_mb.max(MIN_MEMORY_MB);
        Self {
            max_memory_mb,
            min_memory_mb: self.min_memory_mb.clamp(MIN_MEMORY_MB, max_memory_mb),
            window_width: self.window_width.max(MIN_WIDTH),
            window_height: self.window_height.max(MIN_HEIGHT),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_updates_migrate_once_and_preserve_later_choices() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        save_json(
            &dirs.settings_file(),
            &serde_json::json!({"restart_hidden_for_updates": false}),
        )
        .unwrap();
        let mut settings = Settings::load(&dirs).unwrap();
        assert!(settings.restart_hidden_for_updates);
        assert!(settings.quiet_updates_migrated);
        // The migration is persisted even before any UI settings changes.
        let persisted: Settings = load_json(&dirs.settings_file()).unwrap().unwrap();
        assert!(persisted.quiet_updates_migrated);
        settings.restart_hidden_for_updates = false;
        settings.save(&dirs).unwrap();
        assert!(!Settings::load(&dirs).unwrap().restart_hidden_for_updates);
    }

    #[test]
    fn migration_save_failure_preserves_settings_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let original = serde_json::json!({
            "restart_hidden_for_updates": false,
            "max_memory_mb": 8192,
            "theme": "light",
            "favorite_versions": ["1.21.1"],
            "auto_install_updates": false
        });
        save_json(&dirs.settings_file(), &original).unwrap();
        let expected: Settings = serde_json::from_value(original.clone()).unwrap();
        let settings = Settings::load_with_save(&dirs, |migrated| {
            assert!(migrated.quiet_updates_migrated);
            Err(crate::Error::io(
                dirs.settings_file(),
                std::io::Error::from(std::io::ErrorKind::StorageFull),
            ))
        })
        .unwrap();
        assert_eq!(
            settings,
            Settings {
                restart_hidden_for_updates: true,
                ..expected
            }
        );
        let persisted: serde_json::Value = load_json(&dirs.settings_file()).unwrap().unwrap();
        assert_eq!(persisted, original);

        let retried = Settings::load(&dirs).unwrap();
        assert!(retried.quiet_updates_migrated);
        assert_eq!(retried.max_memory_mb, settings.max_memory_mb);
        let persisted: Settings = load_json(&dirs.settings_file()).unwrap().unwrap();
        assert_eq!(persisted, retried);
    }

    #[test]
    fn successful_save_after_migration_failure_preserves_opt_out() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        save_json(
            &dirs.settings_file(),
            &serde_json::json!({"restart_hidden_for_updates": false}),
        )
        .unwrap();
        let mut settings = Settings::load_with_save(&dirs, |_| {
            Err(crate::Error::io(
                dirs.settings_file(),
                std::io::Error::from(std::io::ErrorKind::StorageFull),
            ))
        })
        .unwrap();
        assert!(settings.restart_hidden_for_updates);
        assert!(!settings.quiet_updates_migrated);

        settings.restart_hidden_for_updates = false;
        settings.save(&dirs).unwrap();

        let persisted: Settings = load_json(&dirs.settings_file()).unwrap().unwrap();
        assert!(persisted.quiet_updates_migrated);
        assert!(!persisted.restart_hidden_for_updates);
        let reloaded = Settings::load(&dirs).unwrap();
        assert!(reloaded.quiet_updates_migrated);
        assert!(!reloaded.restart_hidden_for_updates);
    }

    #[test]
    fn new_settings_restart_hidden_by_default() {
        assert!(Settings::default().restart_hidden_for_updates);
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Settings::load(&DataDirs::new(dir.path()))
                .unwrap()
                .restart_hidden_for_updates
        );
    }

    #[test]
    fn playing_with_the_client_is_on_unless_turned_off() {
        assert!(Settings::default().play_with_client);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(
            old.play_with_client,
            "settings saved before the switch existed keep the client"
        );
        let off: Settings = serde_json::from_str(r#"{"play_with_client": false}"#).unwrap();
        assert!(!off.play_with_client);
    }

    #[test]
    fn sanitize_clamps_memory_and_resolution() {
        let s = Settings {
            max_memory_mb: 100,
            min_memory_mb: 9000,
            window_width: 1,
            window_height: 1,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.max_memory_mb, MIN_MEMORY_MB);
        assert_eq!(s.min_memory_mb, MIN_MEMORY_MB);
        assert_eq!((s.window_width, s.window_height), (MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn theme_serializes_lowercase() {
        let s: Settings = serde_json::from_str(r#"{"theme": "light"}"#).unwrap();
        assert_eq!(s.theme, ThemeMode::Light);
        assert_eq!(Settings::default().theme, ThemeMode::Default);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str(r#"{"max_memory_mb": 6144}"#).unwrap();
        assert_eq!(s.max_memory_mb, 6144);
        assert_eq!(s.window_width, DEFAULT_WIDTH);
        assert!(
            !s.discord_show_server,
            "showing the server on Discord stays off until chosen"
        );
    }

    #[test]
    fn preview_rotation_lock_defaults_off_and_persists() {
        assert!(!Settings::default().lock_preview_rotation);
        let old: Settings = serde_json::from_str(r#"{"max_memory_mb": 6144}"#).unwrap();
        assert!(!old.lock_preview_rotation);
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let s = Settings {
            lock_preview_rotation: true,
            ..Settings::default()
        };
        s.save(&dirs).unwrap();
        assert!(Settings::load(&dirs).unwrap().lock_preview_rotation);
    }

    #[test]
    fn save_then_load() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let s = Settings {
            fullscreen: true,
            ..Settings::default()
        };
        s.save(&dirs).unwrap();
        assert_eq!(Settings::load(&dirs).unwrap(), s);
    }
}
