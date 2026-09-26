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
    pub on_game_start: GameStartAction,
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
    /// Minecraft's own menus (the Arctic HUD still works).
    Classic,
}

impl ClientStyle {
    pub const ALL: [ClientStyle; 3] = [Self::Arctic, Self::Aurora, Self::Classic];

    /// The id the Arctic mod knows the style by.
    pub fn id(self) -> &'static str {
        match self {
            Self::Arctic => "arctic",
            Self::Aurora => "aurora",
            Self::Classic => "classic",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Arctic => "Arctic",
            Self::Aurora => "Aurora",
            Self::Classic => "Classic",
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
            on_game_start: GameStartAction::KeepOpen,
            discord_presence: true,
            onboarded: false,
            tray: true,
            favorite_versions: Vec::new(),
            client_style: ClientStyle::Arctic,
            client_style_set: 0,
            client_fancy: false,
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
