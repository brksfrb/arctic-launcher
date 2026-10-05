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
    /// Animated backdrop (aurora, snow, shooting stars).
    pub animations: bool,
    /// Short snowflake intro when the launcher opens.
    pub intro: bool,
    /// Open the launcher window maximized.
    pub start_maximized: bool,
    pub theme: ThemeMode,
    /// What's behind the launcher.
    pub backdrop: Backdrop,
    pub on_game_start: GameStartAction,
    /// What happens to a crash report when the game crashes.
    pub crash_reports: CrashReports,
    /// Show what you're playing on Discord.
    pub discord_presence: bool,
    /// First-run setup finished (or skipped) for this profile.
    pub onboarded: bool,
    /// Keep running in the system tray when the window is closed.
    pub tray: bool,
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
    /// Quitting the launcher also closes games it started (off: they keep running).
    pub exit_games_with_launcher: bool,
    /// Mentions of Arctic's supporter (About, playing together).
    pub show_sponsor: bool,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_memory_mb: DEFAULT_MAX_MEMORY_MB,
            min_memory_mb: MIN_MEMORY_MB,
            window_width: DEFAULT_WIDTH,
            window_height: DEFAULT_HEIGHT,
            fullscreen: false,
            extra_jvm_args: String::new(),
            java_override: None,
            last_version: None,
            last_instance: None,
            show_snapshots: false,
            show_old_versions: false,
            update_channel: UpdateChannel::Stable,
            check_updates_on_start: true,
            animations: true,
            intro: true,
            start_maximized: false,
            theme: ThemeMode::Default,
            backdrop: Backdrop::Scenery,
            on_game_start: GameStartAction::KeepOpen,
            crash_reports: CrashReports::Ask,
            discord_presence: true,
            onboarded: false,
            tray: true,
            favorite_versions: Vec::new(),
            client_style: ClientStyle::Arctic,
            client_style_set: 0,
            client_fancy: false,
            share_server_with_friends: false,
            exit_games_with_launcher: false,
            show_sponsor: true,
            high_performance_gpu: true,
            voice: VoiceSettings::default(),
            shared: crate::shared::SharedSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(dirs: &DataDirs) -> Result<Self> {
        Ok(load_json::<Self>(&dirs.settings_file())?
            .unwrap_or_default()
            .sanitized())
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.settings_file(), &self.clone().sanitized())
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
