//! Default Minecraft settings for anything new: FOV, render distance, GUI
//! scale, music, autojump, keys… kept per launcher profile and written
//! into a game folder's `options.txt` the first time it's played (the game
//! fills in everything not set). They can be copied from an instance you
//! already set up, edited, applied to an existing instance, and shared as
//! a code.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

const FILE: &str = "default-options.json";
const OPTIONS: &str = "options.txt";
pub const MAX_KEYS: usize = 400;
const MAX_KEY: usize = 80;
const MAX_VALUE: usize = 2000;

/// Keys that describe one world, server or moment rather than a
/// preference, or that the game manages itself.
const SKIPPED: &[&str] = &[
    "version",
    "lastServer",
    "tutorialStep",
    "joinedFirstServer",
    "onboardAccessibility",
    "skipMultiplayerWarning",
    "skipRealms32bitWarning",
    "hideBundleTutorial",
    "syncChunkWrites",
    "resourcePacks",
    "incompatibleResourcePacks",
    "hideServerAddress",
    "startedCleanly",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defaults {
    /// `options.txt` keys and values, exactly as the game writes them.
    pub values: BTreeMap<String, String>,
}

/// How the editor shows a common setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Toggle,
    /// Whole numbers from `min` to `max`.
    Whole {
        min: i64,
        max: i64,
    },
    /// A 0–1 fraction shown as a percentage.
    Percent,
    /// Minecraft's FOV: stored as -1..1 for 30..110 degrees.
    Fov,
    /// The game's GUI scale (0 = auto).
    GuiScale,
}

pub struct Common {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// Shown when not set (Minecraft's own default).
    pub game_default: &'static str,
}

/// The settings most people change, for the editor. Anything else can be
/// copied in from an instance.
pub const COMMON: &[Common] = &[
    Common {
        key: "fov",
        label: "Field of view",
        kind: Kind::Fov,
        game_default: "0.0",
    },
    Common {
        key: "renderDistance",
        label: "Render distance",
        kind: Kind::Whole { min: 2, max: 32 },
        game_default: "12",
    },
    Common {
        key: "simulationDistance",
        label: "Simulation distance",
        kind: Kind::Whole { min: 5, max: 32 },
        game_default: "12",
    },
    Common {
        key: "guiScale",
        label: "GUI scale",
        kind: Kind::GuiScale,
        game_default: "0",
    },
    Common {
        key: "maxFps",
        label: "Max framerate",
        kind: Kind::Whole { min: 10, max: 260 },
        game_default: "120",
    },
    Common {
        key: "gamma",
        label: "Brightness",
        kind: Kind::Percent,
        game_default: "0.5",
    },
    Common {
        key: "mouseSensitivity",
        label: "Mouse sensitivity",
        kind: Kind::Percent,
        game_default: "0.5",
    },
    Common {
        key: "autoJump",
        label: "Auto-jump",
        kind: Kind::Toggle,
        game_default: "false",
    },
    Common {
        key: "bobView",
        label: "View bobbing",
        kind: Kind::Toggle,
        game_default: "true",
    },
    Common {
        key: "enableVsync",
        label: "VSync",
        kind: Kind::Toggle,
        game_default: "true",
    },
    Common {
        key: "fullscreen",
        label: "Fullscreen",
        kind: Kind::Toggle,
        game_default: "false",
    },
    Common {
        key: "rawMouseInput",
        label: "Raw mouse input",
        kind: Kind::Toggle,
        game_default: "true",
    },
    Common {
        key: "toggleSprint",
        label: "Toggle sprint",
        kind: Kind::Toggle,
        game_default: "false",
    },
    Common {
        key: "toggleCrouch",
        label: "Toggle sneak",
        kind: Kind::Toggle,
        game_default: "false",
    },
    Common {
        key: "soundCategory_master",
        label: "Master volume",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "soundCategory_music",
        label: "Music",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "soundCategory_record",
        label: "Jukebox / note blocks",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "soundCategory_weather",
        label: "Weather",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "soundCategory_hostile",
        label: "Hostile creatures",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "soundCategory_player",
        label: "Players",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "chatOpacity",
        label: "Chat opacity",
        kind: Kind::Percent,
        game_default: "1.0",
    },
    Common {
        key: "narrator",
        label: "Narrator",
        kind: Kind::Whole { min: 0, max: 3 },
        game_default: "0",
    },
];

impl Kind {
    /// A stored value as the editor shows it.
    pub fn show(self, value: &str) -> String {
        let num = value.trim().parse::<f64>().ok();
        match (self, num) {
            (Kind::Toggle, _) => if value == "true" { "On" } else { "Off" }.into(),
            (Kind::Fov, Some(v)) => format!("{}°", fov_degrees(v)),
            (Kind::Percent, Some(v)) => format!("{}%", (v * 100.0).round() as i64),
            (Kind::GuiScale, Some(0.0)) => "Auto".into(),
            (Kind::Whole { .. } | Kind::GuiScale, Some(v)) => format!("{}", v as i64),
            _ => value.to_owned(),
        }
    }
}

/// Minecraft stores FOV as -1..1 for 30..110 degrees.
pub fn fov_degrees(stored: f64) -> i64 {
    (70.0 + stored.clamp(-1.0, 1.0) * 40.0).round() as i64
}

pub fn fov_stored(degrees: i64) -> String {
    format_float((degrees.clamp(30, 110) as f64 - 70.0) / 40.0)
}

/// How Minecraft writes floats (`0.5`, `1.0`).
pub fn format_float(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{s}.0")
    }
}

impl Defaults {
    pub fn load(dirs: &DataDirs) -> Result<Self> {
        Ok(load_json::<Self>(&dirs.profile_root().join(FILE))?
            .unwrap_or_default()
            .checked())
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.profile_root().join(FILE), &self.clone().checked())
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Only well-formed keys and values, at most [`MAX_KEYS`].
    pub fn checked(mut self) -> Self {
        self.values
            .retain(|k, v| valid_key(k) && valid_value(v) && !SKIPPED.contains(&k.as_str()));
        while self.values.len() > MAX_KEYS {
            let last = self.values.keys().next_back().cloned();
            if let Some(k) = last {
                self.values.remove(&k);
            }
        }
        self
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        if !valid_key(key) || SKIPPED.contains(&key) {
            return Err(Error::Other(format!("\"{key}\" can't be a default")));
        }
        if !valid_value(value) {
            return Err(Error::Other("that value can't be used".into()));
        }
        self.values.insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    /// Every preference in a game folder's `options.txt` (keys, volumes,
    /// video…), leaving out world- and server-specific ones.
    pub fn capture(game_dir: &Path) -> Result<Self> {
        let path = game_dir.join(OPTIONS);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        Ok(Self {
            values: parse(&text),
        }
        .checked())
    }

    /// Write the defaults into a game folder that has no `options.txt`
    /// yet (its first start), as Minecraft `game` stores them (`None`:
    /// the newest). `Ok(true)` when written.
    pub fn apply_if_new(&self, game_dir: &Path, game: Option<&str>) -> Result<bool> {
        let path = game_dir.join(OPTIONS);
        if self.is_empty() || path.exists() {
            return Ok(false);
        }
        std::fs::create_dir_all(game_dir).map_err(|e| Error::io(game_dir, e))?;
        let values = crate::game_defaults_keys::for_game(&self.values, game);
        std::fs::write(&path, render(&values)).map_err(|e| Error::io(&path, e))?;
        Ok(true)
    }

    /// Put the defaults over an existing `options.txt` (everything else in
    /// it stays). Refused while that game runs: it would write its own
    /// settings back on exit.
    pub fn apply_to(&self, game_dir: &Path, game: Option<&str>) -> Result<usize> {
        if crate::sharing::client::in_use(game_dir) {
            return Err(Error::Other(
                "close that game first; it saves its own settings when it quits".into(),
            ));
        }
        let path = game_dir.join(OPTIONS);
        let mut values = match std::fs::read_to_string(&path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(Error::io(&path, e)),
        };
        let order = key_order(&std::fs::read_to_string(&path).unwrap_or_default());
        for (k, v) in crate::game_defaults_keys::for_game(&self.values, game) {
            values.insert(k, v);
        }
        std::fs::create_dir_all(game_dir).map_err(|e| Error::io(game_dir, e))?;
        std::fs::write(&path, render_ordered(&values, &order)).map_err(|e| Error::io(&path, e))?;
        Ok(self.values.len())
    }
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= MAX_KEY
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

fn valid_value(v: &str) -> bool {
    v.len() <= MAX_VALUE && !v.contains(['\n', '\r'])
}

/// `key:value` lines (the value may itself contain colons).
fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn key_order(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, _)| k.trim().to_owned())
        .collect()
}

fn render(values: &BTreeMap<String, String>) -> String {
    values.iter().map(|(k, v)| format!("{k}:{v}\n")).collect()
}

/// Keep the file's own key order, new keys after.
fn render_ordered(values: &BTreeMap<String, String>, order: &[String]) -> String {
    let mut out = String::new();
    for k in order {
        if let Some(v) = values.get(k) {
            out.push_str(&format!("{k}:{v}\n"));
        }
    }
    for (k, v) in values {
        if !order.contains(k) {
            out.push_str(&format!("{k}:{v}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_preferences_not_world_state() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(OPTIONS),
            "version:4671\nfov:1.0\nrenderDistance:16\nlastServer:mc.x.net\nkey_key.jump:key.keyboard.space\nsoundCategory_music:0.0\nbad key:1\n",
        )
        .unwrap();
        let d = Defaults::capture(dir.path()).unwrap();
        assert_eq!(d.values.get("fov").map(String::as_str), Some("1.0"));
        assert_eq!(
            d.values.get("key_key.jump").map(String::as_str),
            Some("key.keyboard.space")
        );
        assert!(!d.values.contains_key("version"));
        assert!(!d.values.contains_key("lastServer"));
        assert!(!d.values.contains_key("bad key"));
    }

    #[test]
    fn applies_only_to_new_folders_unless_asked() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Defaults::default();
        d.set("fov", "1.0").unwrap();
        d.set("autoJump", "false").unwrap();
        assert!(d.set("version", "1").is_err());
        let game = dir.path().join("new");
        assert!(d.apply_if_new(&game, None).unwrap());
        let text = std::fs::read_to_string(game.join(OPTIONS)).unwrap();
        assert!(text.contains("fov:1.0") && text.contains("autoJump:false"));
        // Already played: left alone…
        std::fs::write(game.join(OPTIONS), "renderDistance:8\nfov:0.0\n").unwrap();
        assert!(!d.apply_if_new(&game, None).unwrap());
        // …until applied on purpose; the rest of the file stays, in order.
        assert_eq!(d.apply_to(&game, None).unwrap(), 2);
        let text = std::fs::read_to_string(game.join(OPTIONS)).unwrap();
        assert_eq!(text, "renderDistance:8\nfov:1.0\nautoJump:false\n");
    }

    #[test]
    fn shows_and_stores_values() {
        assert_eq!(Kind::Fov.show("1.0"), "110°");
        assert_eq!(Kind::Fov.show("0.0"), "70°");
        assert_eq!(fov_stored(90), "0.5");
        assert_eq!(fov_stored(110), "1.0");
        assert_eq!(Kind::Percent.show("0.25"), "25%");
        assert_eq!(Kind::GuiScale.show("0"), "Auto");
        assert_eq!(Kind::Toggle.show("true"), "On");
        assert_eq!(format_float(1.0), "1.0");
    }
}
