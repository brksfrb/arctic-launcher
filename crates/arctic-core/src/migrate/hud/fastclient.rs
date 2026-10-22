//! Fast Client HUD layouts: `<game dir>/profiles/<profile>/config/fast-client-hud/modules.json`.
//! One object per module (`"FPS": {enabled, hudX, hudY, defaultHudPosition, hudEdgeX, ...}`), positions in
//! pixels of the display divided by a render scale (1 up to 1080p). A module on its default position, or
//! pinned to an edge, is placed by its screen area; one moved freely keeps its place as a screen fraction.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::{Anchor, HUD_VERSION, blank_hud, gui_of, push, restack, slot, unstack};
use crate::migrate::{Launcher, Scan};

/// Screen size assumed when the launcher doesn't say.
const DEFAULT_DISPLAY: (f64, f64) = (1920.0, 1080.0);

const FAST: &[(&str, &str)] = &[
    ("FPS", "fps"),
    ("CPSCounter", "cps"),
    ("Coordinates", "coords"),
    ("Keystrokes", "keystrokes"),
    ("ArmorHUD", "armor"),
    ("PotionHUD", "effects"),
    ("DirectionHUD", "direction"),
    ("PingDisplay", "ping"),
    ("Clock", "clock"),
    ("Memory", "memory"),
    ("ServerInfo", "server"),
    ("DayCounter", "day"),
    ("Biome", "biome"),
    ("SpeedHUD", "speed"),
    ("ComboCounter", "combo"),
    ("ServerTPS", "tps"),
    ("Saturation", "food"),
    ("PackDisplay", "pack"),
    ("Sprint", "movement"),
];

/// HUD modules Arctic has no widget for (named when they're on).
const OTHER_HUD: &[(&str, &str)] = &[
    ("ArrayList", "Array List"),
    ("ScoreboardMod", "Scoreboard"),
    ("DamageIndicator", "Damage Indicator"),
    ("Waypoints", "Waypoints"),
];

/// The launcher's game folders: its settings say where, else `~/.fastclient`.
pub fn roots(launcher_data: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let settings =
        launcher_data.and_then(|d| crate::migrate::detect::read_json(&d.join("settings.json")));
    if let Some(dir) = settings
        .as_ref()
        .and_then(|s| s.get("gameDirectory").and_then(Value::as_str))
        .map(|s| expand(s, home))
        .filter(|p| p.is_dir())
    {
        out.push(dir);
    }
    if let Some(dir) = home.map(|h| h.join(".fastclient")).filter(|p| p.is_dir())
        && !out.contains(&dir)
    {
        out.push(dir);
    }
    out
}

/// `~/x` as a path under `home`.
fn expand(path: &str, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix('~'), home) {
        (Some(rest), Some(h)) => h.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(path),
    }
}

/// The window size the launcher starts the game with.
fn display(launcher_data: Option<&Path>) -> (f64, f64) {
    let settings =
        launcher_data.and_then(|d| crate::migrate::detect::read_json(&d.join("settings.json")));
    let dim = |key: &str| {
        settings
            .as_ref()
            .and_then(|s| s.get(key).and_then(Value::as_f64))
            .filter(|v| *v >= 320.0)
    };
    match (dim("gameWindowWidth"), dim("gameWindowHeight")) {
        (Some(w), Some(h)) => (w, h),
        _ => DEFAULT_DISPLAY,
    }
}

pub fn scan(game_root: &Path, launcher_data: Option<&Path>, scan: &mut Scan) {
    let names = profile_names(launcher_data);
    let display = display(launcher_data);
    let Ok(entries) = std::fs::read_dir(game_root.join("profiles")) else {
        return;
    };
    let mut folders: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    folders.sort();
    for folder in folders {
        let Some(id) = folder.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let file = folder
            .join("config")
            .join("fast-client-hud")
            .join("modules.json");
        let Some(json) = crate::migrate::detect::read_json(&file) else {
            continue;
        };
        let name = names
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(id, |(_, n)| n.clone());
        profile(&name, &json, display, scan);
    }
}

/// (id, name) of the launcher's profiles.
fn profile_names(launcher_data: Option<&Path>) -> Vec<(String, String)> {
    let Some(json) =
        launcher_data.and_then(|d| crate::migrate::detect::read_json(&d.join("profiles.json")))
    else {
        return Vec::new();
    };
    json.get("profiles")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    Some((
                        p.get("id")?.as_str()?.to_owned(),
                        p.get("name")?.as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn profile(name: &str, p: &Value, display: (f64, f64), scan: &mut Scan) {
    let Some(modules) = p.as_object() else {
        return;
    };
    if !modules.values().any(|m| m.get("hudX").is_some()) {
        return;
    }
    let gui = gui_of(display);
    let mut hud = blank_hud();
    for (their, ours) in FAST {
        let Some(m) = modules.get(*their).filter(|m| is_on(m)) else {
            continue;
        };
        if hud[*ours]["enabled"] == Value::Bool(true) {
            continue;
        }
        let background = m
            .pointer("/settings/background")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        hud.insert(
            (*ours).into(),
            slot(true, place(m, display, gui), 1.0, background),
        );
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    for (their, key) in [
        ("Fullbright", "fullbright"),
        ("Zoom", "zoomEnabled"),
        ("ToggleSneak", "toggleSneak"),
    ] {
        if let Some(m) = modules.get(their) {
            values.insert(key.into(), json!(is_on(m)));
        }
    }
    let left_out: Vec<String> = OTHER_HUD
        .iter()
        .filter(|(k, _)| modules.get(*k).is_some_and(is_on))
        .map(|(_, label)| (*label).to_owned())
        .collect();
    push(
        scan,
        Launcher::FastClient,
        name,
        values,
        Vec::new(),
        left_out,
    );
}

fn is_on(m: &Value) -> bool {
    m.get("enabled").and_then(Value::as_bool) == Some(true)
}

/// Where the module sits, or `None` for "arranged automatically": on its default position it follows
/// Fast Client's own layout, which Arctic arranges alike.
fn place(m: &Value, display: (f64, f64), gui: (f64, f64)) -> Option<Anchor> {
    if m.get("defaultHudPosition").and_then(Value::as_bool) != Some(false) {
        return None;
    }
    let int = |k: &str| m.get(k).and_then(Value::as_i64).unwrap_or(0);
    let x = m.get("hudX").and_then(Value::as_f64)?;
    let y = m.get("hudY").and_then(Value::as_f64)?;
    let (ax, dx) = axis(int("hudEdgeX"), x / display.0, gui.0);
    let (ay, dy) = axis(int("hudEdgeY"), y / display.1, gui.1);
    Some((ax, ay, dx, dy))
}

/// Anchor (start, centre, end) and inward offset in GUI pixels. `edge` is Fast Client's own pin (-1 start,
/// 1 end, 0 free); a free box is placed by where its top-left corner sits (its size isn't saved).
fn axis(edge: i64, fraction: f64, size: f64) -> (i64, i64) {
    let fraction = fraction.clamp(0.0, 1.0);
    match edge {
        -1 => (0, 0),
        1 => (2, 0),
        _ if fraction < 1.0 / 3.0 => (0, (fraction * size).round() as i64),
        _ if fraction > 2.0 / 3.0 => (2, 0),
        _ => (1, ((fraction - 0.5) * size).round() as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(on: bool, x: i64, y: i64, default: bool, edge: (i64, i64)) -> Value {
        json!({"enabled": on, "hudX": x, "hudY": y, "defaultHudPosition": default,
               "hudEdgeX": edge.0, "hudEdgeY": edge.1, "settings": {"background": false}})
    }

    #[test]
    fn edges_and_fractions_become_anchors() {
        let display = (1920.0, 1080.0);
        let gui = (480.0, 270.0);
        // Pinned top-left.
        let fps = module(true, 24, 22, false, (-1, -1));
        assert_eq!(place(&fps, display, gui), Some((0, 0, 0, 0)));
        // Pinned bottom-right.
        let mem = module(true, 1700, 1000, false, (1, 1));
        assert_eq!(place(&mem, display, gui), Some((2, 2, 0, 0)));
        // Free, in the top centre.
        let dir = module(true, 730, 8, false, (0, 0));
        assert_eq!(place(&dir, display, gui).map(|a| (a.0, a.1)), Some((1, 0)));
        // On its default position.
        let default = module(true, 0, 0, true, (0, 0));
        assert_eq!(place(&default, display, gui), None);
    }

    #[test]
    fn reads_a_profile() {
        let p = json!({
            "FPS": module(true, 24, 22, false, (-1, -1)),
            "Keystrokes": module(false, 890, 760, false, (0, 0)),
            "Coordinates": module(true, 10, 900, false, (-1, 1)),
            "ArrayList": module(true, 0, 0, true, (0, 0)),
            "Fullbright": {"enabled": true},
            "Zoom": {"enabled": false}
        });
        let mut scan = Scan::default();
        profile("26.3-Fabric", &p, (1920.0, 1080.0), &mut scan);
        let found = &scan.clients[0];
        assert_eq!(found.launcher, Launcher::FastClient);
        let v = &found.settings.values;
        assert_eq!(v["hud"]["fps"]["enabled"], json!(true));
        assert_eq!(v["hud"]["fps"]["background"], json!(false));
        assert_eq!(v["hud"]["keystrokes"]["enabled"], json!(false));
        assert_eq!(v["hud"]["coords"]["ay"], json!(2));
        assert_eq!(v["fullbright"], json!(true));
        assert_eq!(v["zoomEnabled"], json!(false));
        assert_eq!(found.notes, vec!["No Arctic match for: Array List"]);
    }

    #[test]
    fn a_file_without_positions_is_not_a_layout() {
        let mut scan = Scan::default();
        profile(
            "x",
            &json!({"unrelated": {"enabled": true}}),
            (1920.0, 1080.0),
            &mut scan,
        );
        assert!(scan.clients.is_empty());
    }

    #[test]
    fn home_paths_expand() {
        let home = Path::new("/home/a");
        assert_eq!(
            expand("~/.fastclient", Some(home)),
            home.join(".fastclient")
        );
        assert_eq!(expand("/games", Some(home)), PathBuf::from("/games"));
    }
}
