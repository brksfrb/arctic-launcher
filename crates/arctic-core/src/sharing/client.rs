//! Arctic Client settings (`config/arctic.json` in a game folder) in a
//! bundle: an allow-list of keys, each checked and rebuilt, so a bundle can
//! only carry values the mod accepts. The proxy and hidden players never
//! leave the PC.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::{Error, Result};

const CONFIG_FILE: &str = "config/arctic.json";
const LOCK_FILE: &str = "config/arctic.lock";
const MAX_WIDGETS: usize = 64;
const MAX_WIDGET_ID: usize = 40;
const MAX_KEY_NAME: usize = 64;
const MAX_OFFSET: i64 = 4096;
const MAX_HUD_VERSION: i64 = 100;
/// Hud.MIN_SCALE / MAX_SCALE in the mod.
const MIN_SCALE: f64 = 0.5;
const MAX_SCALE: f64 = 2.5;
const STYLES: [&str; 3] = ["arctic", "aurora", "classic"];
const CROSSHAIR_STYLES: [&str; 4] = ["cross", "dot", "circle", "cross-dot"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    Bool,
    Key,
    Style,
    Hud,
    HudVersion,
    Crosshair,
}

/// Every key a bundle may carry, and how it's checked.
const RULES: &[(&str, Rule)] = &[
    ("hud", Rule::Hud),
    ("hudVersion", Rule::HudVersion),
    ("crosshair", Rule::Crosshair),
    ("style", Rule::Style),
    ("fancy", Rule::Bool),
    ("showCosmetics", Rule::Bool),
    ("fullbright", Rule::Bool),
    ("zoomEnabled", Rule::Bool),
    ("freelookEnabled", Rule::Bool),
    ("zoomKey", Rule::Key),
    ("freelookKey", Rule::Key),
    ("fullbrightKey", Rule::Key),
    ("emoteKey", Rule::Key),
    ("toggleSprint", Rule::Bool),
    ("toggleSneak", Rule::Bool),
    ("chatTimestamps", Rule::Bool),
    ("chatStack", Rule::Bool),
    ("lowFire", Rule::Bool),
    ("clearWeather", Rule::Bool),
    ("confirmLeave", Rule::Bool),
];

/// Which part of the settings a bundle holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Widget positions, sizes and which are on.
    Hud,
    Crosshair,
    /// Everything shareable: HUD, crosshair, style, features and keys.
    All,
}

impl Part {
    fn keys(self) -> Vec<&'static str> {
        match self {
            Part::Hud => vec!["hud", "hudVersion"],
            Part::Crosshair => vec!["crosshair"],
            Part::All => RULES.iter().map(|(k, _)| *k).collect(),
        }
    }
}

/// Checked client settings, ready to write into a game folder.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientPart {
    pub part: Part,
    pub values: Map<String, Value>,
}

impl ClientPart {
    /// Keep the keys `part` allows, each checked; anything else is dropped,
    /// a bad value is an error.
    pub fn check(part: Part, data: &Value) -> Result<Self> {
        let data = data
            .as_object()
            .ok_or_else(|| bad("the settings aren't an object"))?;
        let mut values = Map::new();
        for key in part.keys() {
            let Some(value) = data.get(key) else {
                continue;
            };
            let rule = RULES
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, r)| *r)
                .unwrap_or(Rule::Bool);
            let clean =
                check_value(rule, value).ok_or_else(|| bad(&format!("bad value for {key}")))?;
            values.insert(key.to_owned(), clean);
        }
        if values.is_empty() {
            return Err(bad("there are no settings in it"));
        }
        Ok(Self { part, values })
    }

    /// Read `part` from a game folder's settings.
    pub fn read(game_dir: &Path, part: Part) -> Result<Self> {
        let current = load(&config_path(game_dir))?;
        if current.is_empty() {
            return Err(Error::Other(
                "no Arctic Client settings yet: start the game with Arctic once".into(),
            ));
        }
        Self::check(part, &Value::Object(current))
    }

    /// Write these values into a game folder's settings, keeping the rest.
    /// Refused while that game runs: it saves its own copy on exit.
    pub fn apply(&self, game_dir: &Path) -> Result<()> {
        if in_use(game_dir) {
            return Err(Error::Other(
                "Minecraft is running from that instance; close it first (it saves its own settings when it quits)"
                    .into(),
            ));
        }
        let path = config_path(game_dir);
        let mut current = load(&path)?;
        for (k, v) in &self.values {
            current.insert(k.clone(), v.clone());
        }
        crate::storage::save_json(&path, &Value::Object(current))
    }

    /// Number of HUD widgets turned on, if the bundle has a HUD.
    pub fn widgets_on(&self) -> Option<usize> {
        let hud = self.values.get("hud")?.as_object()?;
        Some(
            hud.values()
                .filter(|s| s.get("enabled").and_then(Value::as_bool) == Some(true))
                .count(),
        )
    }
}

/// The Arctic Client holds `config/arctic.lock` while its game runs.
pub fn in_use(game_dir: &Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .open(game_dir.join(LOCK_FILE))
    else {
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

pub fn config_path(game_dir: &Path) -> PathBuf {
    game_dir.join(CONFIG_FILE)
}

/// The settings file as an object (empty if missing or unreadable).
fn load(path: &Path) -> Result<Map<String, Value>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(Error::io(path, e)),
    }
}

fn bad(what: &str) -> Error {
    Error::Other(format!(
        "these Arctic Client settings can't be used: {what}"
    ))
}

fn check_value(rule: Rule, v: &Value) -> Option<Value> {
    match rule {
        Rule::Bool => v.as_bool().map(Value::Bool),
        Rule::Key => v
            .as_str()
            .filter(|s| is_key_name(s))
            .map(|s| Value::String(s.to_owned())),
        Rule::Style => v
            .as_str()
            .filter(|s| STYLES.contains(s))
            .map(|s| Value::String(s.to_owned())),
        Rule::HudVersion => int(v, 1, MAX_HUD_VERSION).map(Value::from),
        Rule::Hud => check_hud(v),
        Rule::Crosshair => check_crosshair(v),
    }
}

/// Minecraft key names: `key.keyboard.left.alt`, `key.mouse.4`.
fn is_key_name(s: &str) -> bool {
    s.len() <= MAX_KEY_NAME
        && (s.starts_with("key.keyboard.") || s.starts_with("key.mouse."))
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
}

fn check_hud(v: &Value) -> Option<Value> {
    let slots = v.as_object()?;
    if slots.len() > MAX_WIDGETS {
        return None;
    }
    let mut out = Map::new();
    for (id, slot) in slots {
        let id_ok = !id.is_empty()
            && id.len() <= MAX_WIDGET_ID
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
        if !id_ok {
            return None;
        }
        out.insert(id.clone(), check_slot(slot)?);
    }
    Some(Value::Object(out))
}

/// A HudSlot: every field rebuilt, missing ones take the mod's defaults.
fn check_slot(v: &Value) -> Option<Value> {
    let s = v.as_object()?;
    let flag = |k: &str, default: bool| match s.get(k) {
        None => Some(default),
        Some(x) => x.as_bool(),
    };
    let num = |k: &str, lo: i64, hi: i64| match s.get(k) {
        None => Some(0),
        Some(x) => int(x, lo, hi),
    };
    let scale = match s.get("scale") {
        None => 1.0,
        Some(x) => x.as_f64().filter(|f| (MIN_SCALE..=MAX_SCALE).contains(f))?,
    };
    Some(serde_json::json!({
        "enabled": flag("enabled", false)?,
        "placed": flag("placed", false)?,
        "ax": num("ax", 0, 2)?,
        "ay": num("ay", 0, 2)?,
        "dx": num("dx", -MAX_OFFSET, MAX_OFFSET)?,
        "dy": num("dy", -MAX_OFFSET, MAX_OFFSET)?,
        "scale": scale,
        "background": flag("background", true)?,
    }))
}

/// CrosshairConfig, with the ranges the in-game editor allows.
fn check_crosshair(v: &Value) -> Option<Value> {
    let c = v.as_object()?;
    let style = match c.get("style") {
        None => "cross",
        Some(s) => s.as_str().filter(|s| CROSSHAIR_STYLES.contains(s))?,
    };
    let num = |k: &str, lo: i64, hi: i64, default: i64| match c.get(k) {
        None => Some(default),
        Some(x) => int(x, lo, hi),
    };
    let flag = |k: &str, default: bool| match c.get(k) {
        None => Some(default),
        Some(x) => x.as_bool(),
    };
    Some(serde_json::json!({
        "enabled": flag("enabled", false)?,
        "style": style,
        "size": num("size", 1, 12, 5)?,
        "gap": num("gap", 0, 8, 2)?,
        "thickness": num("thickness", 1, 4, 1)?,
        "color": num("color", i64::from(i32::MIN), i64::from(i32::MAX), -1)?,
        "outline": flag("outline", true)?,
    }))
}

fn int(v: &Value, lo: i64, hi: i64) -> Option<i64> {
    v.as_i64().filter(|n| (lo..=hi).contains(n))
}
