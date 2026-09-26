//! HUD setups from LabyMod and Lunar Client, turned into Arctic Client
//! settings. Positions keep their screen corner (or middle) and offset;
//! widgets Arctic has no match for are listed, not guessed.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::{FoundClient, Launcher, Scan};
use crate::sharing::{ClientPart, Part};

/// Arctic's HUD widgets (they're all written, so nothing unexpected shows).
const WIDGETS: [&str; 19] = [
    "fps",
    "cps",
    "coords",
    "keystrokes",
    "armor",
    "effects",
    "direction",
    "ping",
    "clock",
    "memory",
    "server",
    "day",
    "biome",
    "speed",
    "held",
    "target",
    "combo",
    "reach",
    "movement",
];
/// The mod's current HUD layout format.
const HUD_VERSION: i64 = 2;
const MIN_SCALE: f64 = 0.5;
const MAX_SCALE: f64 = 2.5;

const LABYMOD: &[(&str, &str)] = &[
    ("fps", "fps"),
    ("coordinates", "coords"),
    ("ping", "ping"),
    ("speed", "speed"),
    ("click_test", "cps"),
    ("fdirection", "direction"),
    ("direction", "direction"),
    ("biome", "biome"),
    ("clock", "clock"),
    ("memory", "memory"),
    ("server_address", "server"),
    ("potion", "effects"),
    ("main_hand", "held"),
    ("keyStrokes", "keystrokes"),
    ("combo", "combo"),
    ("range", "reach"),
    ("toggleSneak", "movement"),
    ("helmet", "armor"),
    ("chest", "armor"),
    ("legs", "armor"),
    ("feet", "armor"),
];

const LUNAR: &[(&str, &str)] = &[
    ("FPS", "fps"),
    ("CPS", "cps"),
    ("COORDINATES", "coords"),
    ("KEYSTROKES", "keystrokes"),
    ("ARMORSTATUS", "armor"),
    ("POTION_EFFECTS", "effects"),
    ("DIRECTION_HUD", "direction"),
    ("PING", "ping"),
    ("CLOCK", "clock"),
    ("MEMORY", "memory"),
    ("SERVER_ADDRESS", "server"),
    ("DAY_COUNTER", "day"),
    ("COMBO", "combo"),
    ("REACH_DISPLAY", "reach"),
];

/// A widget whose on/off state the client didn't save.
#[derive(Debug, Clone, PartialEq)]
pub struct Unsure {
    /// Arctic's widget id.
    pub widget: String,
    /// The client's name for it.
    pub label: String,
    /// Our guess: on if it was moved somewhere.
    pub guess: bool,
    slot: Value,
}

impl FoundClient {
    /// The settings with the player's answers for the unsure widgets.
    pub fn with_choices(&self, on: &[String]) -> ClientPart {
        let mut values = self.settings.values.clone();
        if let Some(Value::Object(hud)) = values.get_mut("hud") {
            for u in &self.unsure {
                let mut slot = u.slot.clone();
                slot["enabled"] = Value::Bool(on.contains(&u.widget));
                hud.insert(u.widget.clone(), slot);
            }
        }
        ClientPart {
            part: Part::All,
            values,
        }
    }

    /// Our guesses, for when nobody is asked (CLI default).
    pub fn guesses(&self) -> Vec<String> {
        self.unsure
            .iter()
            .filter(|u| u.guess)
            .map(|u| u.widget.clone())
            .collect()
    }
}

/// LabyMod 4's HUD (`labymod/hud/default.json`) and its zoom/fullbright keys.
pub fn labymod(configs: &Path, scan: &mut Scan) {
    let Some(hud_json) = super::detect::read_json(&configs.join("labymod/hud/default.json")) else {
        return;
    };
    let Some(widgets) = hud_json.get("configs").and_then(Value::as_object) else {
        return;
    };
    let mut hud = blank_hud();
    let mut left_out = Vec::new();
    for (their, w) in widgets {
        let Some(ours) = LABYMOD.iter().find(|(t, _)| t == their).map(|(_, o)| *o) else {
            if w.get("enabled").and_then(Value::as_bool) == Some(true) {
                left_out.push(their.clone());
            }
            continue;
        };
        let on = w.get("enabled").and_then(Value::as_bool) == Some(true);
        let already_on = hud[ours]["enabled"] == Value::Bool(true);
        if !on || already_on {
            continue;
        }
        // Docked next to the hotbar, or never moved (LabyMod keeps those at
        // 0,0 and arranges them itself): Arctic arranges them too.
        let docked = w.get("dropzoneId").and_then(Value::as_str).is_some();
        let unmoved = w.get("x").and_then(Value::as_f64).unwrap_or(0.0) == 0.0
            && w.get("y").and_then(Value::as_f64).unwrap_or(0.0) == 0.0;
        let area = w
            .get("areaIdentifier")
            .and_then(Value::as_str)
            .unwrap_or("TOP_LEFT");
        let x = w.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = w.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        let background = w
            .pointer("/background/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let scale = w.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        hud.insert(
            ours.into(),
            slot(
                true,
                (!docked && !unmoved).then(|| anchor(area, x, y)),
                scale,
                background,
            ),
        );
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    let settings = super::detect::read_json(&configs.join("labymod/settings.json"));
    if let Some(key) = settings
        .as_ref()
        .and_then(|s| s.pointer("/ingame/zoom/zoomKey").and_then(Value::as_str))
        .and_then(key_name)
    {
        values.insert("zoomKey".into(), json!(key));
    }
    if let Some(fb) = super::detect::read_json(&configs.join("fullbright/settings.json")) {
        if let Some(on) = fb.get("enabled").and_then(Value::as_bool) {
            values.insert("fullbright".into(), json!(on));
        }
        if let Some(key) = fb
            .pointer("/configMeta/enabled.hotkey")
            .and_then(Value::as_str)
            .and_then(key_name)
        {
            values.insert("fullbrightKey".into(), json!(key));
        }
    }
    push(
        scan,
        Launcher::LabyMod,
        "LabyMod",
        values,
        Vec::new(),
        left_out,
    );
}

/// Lunar's per-profile `mods.json`. Lunar saves only what differs from its
/// defaults, so widgets without a saved on/off become questions.
pub fn lunar(game_settings: &Path, scan: &mut Scan) {
    let profiles: Vec<String> =
        super::detect::read_json(&game_settings.join("profile_manager.json"))
            .and_then(|v| v.as_array().cloned())
            .map(|list| {
                list.iter()
                    .filter_map(|p| p.get("name").and_then(Value::as_str).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|| vec!["Default".into()]);
    for profile in profiles {
        // Profile names become folder names; never leave the settings folder.
        if profile.contains(['/', '\\']) || profile.contains("..") {
            continue;
        }
        let Some(mods) = super::detect::read_json(&game_settings.join(&profile).join("mods.json"))
        else {
            continue;
        };
        lunar_profile(&profile, &mods, scan);
    }
}

fn lunar_profile(profile: &str, mods: &Value, scan: &mut Scan) {
    let mut hud = blank_hud();
    let mut unsure = Vec::new();
    for (their, ours) in LUNAR {
        let Some(m) = mods.get(*their) else {
            continue;
        };
        let place = match (
            m.get("position").and_then(Value::as_str),
            m.get("x"),
            m.get("y"),
        ) {
            (Some(pos), x, y) if x.is_some() || y.is_some() => Some(anchor(
                pos,
                x.and_then(Value::as_f64).unwrap_or(0.0),
                y.and_then(Value::as_f64).unwrap_or(0.0),
            )),
            _ => None,
        };
        let scale = m.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        match m.get("enabled").and_then(Value::as_bool) {
            Some(on) => {
                hud.insert((*ours).into(), slot(on, place, scale, true));
            }
            None => unsure.push(Unsure {
                widget: (*ours).into(),
                label: title(their),
                guess: place.is_some(),
                slot: slot(false, place, scale, true),
            }),
        }
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    for (their, key) in [
        ("TOGGLE_SNEAK", "toggleSneak"),
        ("FREELOOK", "freelookEnabled"),
        ("ZOOM", "zoomEnabled"),
    ] {
        if let Some(on) = mods
            .pointer(&format!("/{their}/enabled"))
            .and_then(Value::as_bool)
        {
            values.insert(key.into(), json!(on));
        }
    }
    push(scan, Launcher::Lunar, profile, values, unsure, Vec::new());
}

/// Arctic's one-line widgets: 13 px tall, stacked 2 px apart, kept 4 px
/// from the screen edge (the mod's TextWidget, Hud.GAP and Hud.MARGIN).
const TEXT_WIDGETS: [&str; 13] = [
    "fps",
    "cps",
    "coords",
    "direction",
    "ping",
    "clock",
    "memory",
    "server",
    "day",
    "biome",
    "speed",
    "reach",
    "combo",
];
const ROW: i64 = 13 + 2;
const MARGIN: i64 = 4;

/// Clients stack text lines closer than Arctic's boxes (LabyMod: 10 px), so
/// one-line widgets pinned in the same column are pushed apart just enough
/// not to overlap, in their order, and kept off the very edge.
fn restack(hud: &mut Map<String, Value>) {
    let int = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    let pinned = |v: &Value| v["enabled"] == Value::Bool(true) && v["placed"] == Value::Bool(true);
    for id in TEXT_WIDGETS {
        if let Some(slot) = hud.get_mut(id).filter(|v| pinned(v)) {
            for (anchor, offset) in [("ax", "dx"), ("ay", "dy")] {
                if int(slot, anchor) != 1 && int(slot, offset) < MARGIN {
                    slot[offset] = json!(MARGIN);
                }
            }
        }
    }
    // Columns: same corner and the same distance from the side.
    let mut columns: Vec<((i64, i64, i64), Vec<(i64, String)>)> = Vec::new();
    for id in TEXT_WIDGETS {
        let Some(slot) = hud.get(id).filter(|v| pinned(v)) else {
            continue;
        };
        let key = (int(slot, "ax"), int(slot, "ay"), int(slot, "dx"));
        let entry = (int(slot, "dy"), id.to_owned());
        match columns.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(entry),
            None => columns.push((key, vec![entry])),
        }
    }
    for ((_, ay, _), mut list) in columns {
        if ay == 1 {
            continue;
        }
        // Measured inward from the top or the bottom edge.
        list.sort();
        let mut next = i64::MIN;
        for (dy, id) in list {
            let placed = dy.max(next);
            if let Some(slot) = hud.get_mut(&id) {
                slot["dy"] = json!(placed);
            }
            next = placed + ROW;
        }
    }
}

/// Two widgets pinned to the very same spot would sit on top of each
/// other: the later ones are arranged automatically instead.
fn unstack(hud: &mut Map<String, Value>) {
    let mut taken: Vec<(Value, Value, Value, Value)> = Vec::new();
    for w in WIDGETS {
        let Some(slot) = hud.get_mut(w) else {
            continue;
        };
        if slot["enabled"] != Value::Bool(true) || slot["placed"] != Value::Bool(true) {
            continue;
        }
        let spot = (
            slot["ax"].clone(),
            slot["ay"].clone(),
            slot["dx"].clone(),
            slot["dy"].clone(),
        );
        if taken.contains(&spot) {
            slot["placed"] = Value::Bool(false);
        } else {
            taken.push(spot);
        }
    }
}

fn push(
    scan: &mut Scan,
    launcher: Launcher,
    profile: &str,
    values: Map<String, Value>,
    unsure: Vec<Unsure>,
    left_out: Vec<String>,
) {
    match ClientPart::check(Part::All, &Value::Object(values)) {
        Ok(settings) => {
            let mut notes = Vec::new();
            if !left_out.is_empty() {
                notes.push(format!("No Arctic match for: {}", left_out.join(", ")));
            }
            scan.clients.push(super::FoundClient {
                launcher,
                profile: profile.to_owned(),
                settings,
                unsure,
                notes,
            });
        }
        Err(e) => scan.seen.push((launcher, format!("{profile}: {e}"))),
    }
}

fn blank_hud() -> Map<String, Value> {
    WIDGETS
        .iter()
        .map(|w| ((*w).to_owned(), slot(false, None, 1.0, true)))
        .collect()
}

/// (ax, ay, dx, dy) from a screen area and an offset.
type Anchor = (i64, i64, i64, i64);

fn slot(enabled: bool, place: Option<Anchor>, scale: f64, background: bool) -> Value {
    let (ax, ay, dx, dy) = place.unwrap_or((0, 0, 0, 0));
    json!({
        "enabled": enabled,
        "placed": place.is_some(),
        "ax": ax, "ay": ay, "dx": dx, "dy": dy,
        "scale": scale.clamp(MIN_SCALE, MAX_SCALE),
        "background": background,
    })
}

/// `TOP_LEFT`, `topRight`, `middle_left`, `bottomCenterLeft`… plus the
/// offset from that area, as Arctic's anchor and inward offsets.
fn anchor(area: &str, x: f64, y: f64) -> Anchor {
    let a = area.to_ascii_lowercase().replace(['_', '-'], "");
    let ay = if a.starts_with("top") {
        0
    } else if a.starts_with("bottom") {
        2
    } else {
        1
    };
    let rest = a
        .trim_start_matches("top")
        .trim_start_matches("bottom")
        .trim_start_matches("middle");
    let ax = if rest.starts_with("center") || rest.is_empty() {
        1
    } else if rest.starts_with("right") {
        2
    } else {
        0
    };
    let inward = |edge: i64, v: f64| match edge {
        0 => v.max(0.0).round() as i64,
        2 => (-v).max(0.0).round() as i64,
        _ => v.round() as i64,
    };
    (
        ax,
        ay,
        inward(ax, x).clamp(-4096, 4096),
        inward(ay, y).clamp(-4096, 4096),
    )
}

/// LabyMod key names (`C`, `LEFT_ALT`, `F4`) as Minecraft's.
fn key_name(key: &str) -> Option<String> {
    let k = key.trim().to_ascii_lowercase();
    if k.is_empty() || k == "none" || k.contains("mouse") {
        return None;
    }
    let name = format!("key.keyboard.{}", k.replace('_', "."));
    name.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.')
        .then_some(name)
}

/// `POTION_EFFECTS` → `Potion Effects`.
fn title(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_string() + &c.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
