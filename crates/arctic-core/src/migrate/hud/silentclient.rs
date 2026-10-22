//! Silent Client HUD layouts, in the game folder (`.minecraft`). The 1.8.9 client keeps one text file per
//! config in `SilentClient-Configs/` (`MOD:FPS:true`, `POS:FPS:0.93:0.15`, positions as fractions of the
//! screen); the newer one keeps JSON in `silentclient/` (`config.json`, `configs/<name>.json`) with each
//! mod on a screen anchor and an offset on a 1080-high canvas.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::{Anchor, HUD_VERSION, anchor, blank_hud, gui_of, push, restack, slot, unstack};
use crate::migrate::{Launcher, Scan};

/// Screen size assumed (neither format saves it).
const DISPLAY: (f64, f64) = (1920.0, 1080.0);
/// Height of the newer client's canvas.
const CANVAS_HEIGHT: f64 = 1080.0;

/// The 1.8.9 client's names.
const OLD: &[(&str, &str)] = &[
    ("FPS", "fps"),
    ("CPS", "cps"),
    ("Toggle Sprint", "movement"),
    ("Keystrokes", "keystrokes"),
    ("Armor Status", "armor"),
    ("Memory", "memory"),
    ("Potion Hud", "effects"),
    ("Ping", "ping"),
    ("Pack Display", "pack"),
    ("Clock", "clock"),
    ("Speedometer", "speed"),
    ("Reach Display", "reach"),
    ("Day Counter", "day"),
    ("Server Address", "server"),
    ("Combo Counter", "combo"),
    ("Coordinates", "coords"),
    ("Player Counter", "players"),
];

/// The newer client's ids.
const NEW: &[(&str, &str)] = &[
    ("fps", "fps"),
    ("cps", "cps"),
    ("toggle_sprint", "movement"),
    ("keystrokes", "keystrokes"),
    ("armor_status", "armor"),
    ("memory", "memory"),
    ("potion_status", "effects"),
    ("ping", "ping"),
    ("pack_display", "pack"),
    ("clock", "clock"),
    ("speedometer", "speed"),
    ("reach_display", "reach"),
    ("day_counter", "day"),
    ("server_address", "server"),
    ("combo_counter", "combo"),
    ("coordinates", "coords"),
    ("saturation", "food"),
    ("playtime", "session"),
];

/// HUD mods Arctic has no widget for (named when they're on).
const OTHER_HUD_OLD: &[(&str, &str)] = &[("BlockInfo", "Block Info")];
const OTHER_HUD_NEW: &[(&str, &str)] = &[
    ("watermark", "Watermark"),
    ("player_model", "Player Model"),
    ("mouse_strokes", "Mouse Strokes"),
];

pub fn scan(dot_minecraft: &Path, scan: &mut Scan) {
    old(dot_minecraft, scan);
    new(dot_minecraft, scan);
}

fn old(game_dir: &Path, scan: &mut Scan) {
    let Ok(entries) = std::fs::read_dir(game_dir.join("SilentClient-Configs")) else {
        return;
    };
    let active = crate::migrate::detect::read_json(&game_dir.join("silent_settings.json"))
        .and_then(|s| s.get("config").and_then(Value::as_str).map(str::to_owned));
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_active = file
            .file_name()
            .is_some_and(|n| Some(n.to_string_lossy().into_owned()) == active);
        old_profile(&stem, &text, is_active, scan);
    }
}

fn old_profile(name: &str, text: &str, active: bool, scan: &mut Scan) {
    // Per mod: switched on?, and where (a screen fraction).
    let mut on: Vec<(String, bool)> = Vec::new();
    let mut at: Vec<(String, (f64, f64))> = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.trim().split(':').collect();
        match parts.as_slice() {
            ["MOD", their, flag] => on.push(((*their).into(), flag.eq_ignore_ascii_case("true"))),
            ["POS", their, x, y] => {
                if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
                    at.push(((*their).into(), (x, y)));
                }
            }
            _ => {}
        }
    }
    if on.is_empty() {
        return;
    }
    let gui = gui_of(DISPLAY);
    let state = |their: &str| on.iter().find(|(n, _)| n == their).map(|(_, f)| *f);
    let mut hud = blank_hud();
    for (their, ours) in OLD {
        if state(their) != Some(true) || hud[*ours]["enabled"] == Value::Bool(true) {
            continue;
        }
        let place = at
            .iter()
            .find(|(n, _)| n == their)
            .and_then(|(_, f)| fraction_place(*f, gui));
        hud.insert((*ours).into(), slot(true, place, 1.0, true));
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    for (their, key) in [
        ("FullBright", "fullbright"),
        ("Zoom", "zoomEnabled"),
        ("Perspective", "freelookEnabled"),
    ] {
        if let Some(flag) = state(their) {
            values.insert(key.into(), json!(flag));
        }
    }
    let left_out = left_out(OTHER_HUD_OLD, |k| state(k) == Some(true));
    finish(scan, name, active, values, left_out);
}

/// The element's top-left as screen fractions: nearest edge by thirds (its size isn't saved, so a box on
/// the far side is pinned to that edge).
fn fraction_place((fx, fy): (f64, f64), gui: (f64, f64)) -> Option<Anchor> {
    if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
        return None;
    }
    let (ax, dx) = third(fx, gui.0);
    let (ay, dy) = third(fy, gui.1);
    Some((ax, ay, dx, dy))
}

fn third(fraction: f64, size: f64) -> (i64, i64) {
    if fraction < 1.0 / 3.0 {
        (0, (fraction * size).round() as i64)
    } else if fraction > 2.0 / 3.0 {
        (2, 0)
    } else {
        (1, ((fraction - 0.5) * size).round() as i64)
    }
}

fn new(game_dir: &Path, scan: &mut Scan) {
    let folder = game_dir.join("silentclient");
    let mut files = vec![("Current".to_owned(), folder.join("config.json"))];
    if let Ok(entries) = std::fs::read_dir(folder.join("configs")) {
        let mut named: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        named.sort();
        for file in named {
            if let Some(stem) = file.file_stem() {
                files.push((stem.to_string_lossy().into_owned(), file));
            }
        }
    }
    for (name, file) in files {
        if let Some(json) = crate::migrate::detect::read_json(&file) {
            new_profile(&name, &json, scan);
        }
    }
}

fn new_profile(name: &str, p: &Value, scan: &mut Scan) {
    let Some(mods) = p.get("mods").and_then(Value::as_object) else {
        return;
    };
    let gui = gui_of(DISPLAY);
    let canvas_to_gui = gui.1 / CANVAS_HEIGHT;
    let mut hud = blank_hud();
    for (their, ours) in NEW {
        let Some(m) = mods.get(*their).filter(|m| is_on(m)) else {
            continue;
        };
        if hud[*ours]["enabled"] == Value::Bool(true) {
            continue;
        }
        hud.insert(
            (*ours).into(),
            slot(true, new_place(m, canvas_to_gui), 1.0, true),
        );
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    for (their, key) in [
        ("fullbright", "fullbright"),
        ("zoom", "zoomEnabled"),
        ("freelook", "freelookEnabled"),
    ] {
        if let Some(m) = mods.get(their) {
            values.insert(key.into(), json!(is_on(m)));
        }
    }
    let left_out = left_out(OTHER_HUD_NEW, |k| mods.get(k).is_some_and(is_on));
    finish(scan, name, false, values, left_out);
}

fn is_on(m: &Value) -> bool {
    m.get("enabled").and_then(Value::as_bool) == Some(true)
}

/// A mod on its anchor with an offset (canvas pixels): never placed means the client arranges it.
fn new_place(m: &Value, canvas_to_gui: f64) -> Option<Anchor> {
    if m.get("positioned").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let area = m.get("anchor").and_then(Value::as_str)?;
    let x = m.get("offsetX").and_then(Value::as_f64).unwrap_or(0.0);
    let y = m.get("offsetY").and_then(Value::as_f64).unwrap_or(0.0);
    Some(anchor(area, x * canvas_to_gui, y * canvas_to_gui))
}

fn left_out(other: &[(&str, &str)], on: impl Fn(&str) -> bool) -> Vec<String> {
    other
        .iter()
        .filter(|(k, _)| on(k))
        .map(|(_, label)| (*label).to_owned())
        .collect()
}

fn finish(
    scan: &mut Scan,
    name: &str,
    active: bool,
    values: Map<String, Value>,
    left_out: Vec<String>,
) {
    let profile = if active {
        format!("{name} · active")
    } else {
        name.to_owned()
    };
    push(
        scan,
        Launcher::SilentClient,
        &profile,
        values,
        Vec::new(),
        left_out,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_text_format_is_read() {
        let text = "MOD:FPS:true\r\nPOS:FPS:0.003:0.0056\r\nMOD:CPS:true\r\nPOS:CPS:0.93125:0.17\r\n\
                    MOD:Keystrokes:false\r\nPOS:Keystrokes:0.5:0.5\r\nMOD:FullBright:true\r\nMOD:BlockInfo:true\r\n";
        let mut scan = Scan::default();
        old_profile("PvP", text, true, &mut scan);
        let found = &scan.clients[0];
        assert_eq!(found.launcher, Launcher::SilentClient);
        assert_eq!(found.profile, "PvP · active");
        let v = &found.settings.values;
        assert_eq!(v["hud"]["fps"]["enabled"], json!(true));
        assert_eq!(v["hud"]["fps"]["ax"], json!(0));
        assert_eq!(v["hud"]["cps"]["ax"], json!(2));
        assert_eq!(v["hud"]["keystrokes"]["enabled"], json!(false));
        assert_eq!(v["fullbright"], json!(true));
        assert_eq!(found.notes, vec!["No Arctic match for: Block Info"]);
    }

    #[test]
    fn a_text_file_without_mods_is_ignored() {
        let mut scan = Scan::default();
        old_profile("x", "just words", false, &mut scan);
        assert!(scan.clients.is_empty());
    }

    #[test]
    fn the_json_format_uses_anchors_and_offsets() {
        let p = json!({"mods": {
            "fps": {"enabled": true, "anchor": "TOP_LEFT", "offsetX": 0.0, "offsetY": 50.0, "positioned": true},
            "clock": {"enabled": true, "anchor": "BOTTOM_RIGHT", "offsetX": -40.0, "offsetY": -20.0, "positioned": true},
            "ping": {"enabled": true, "anchor": "TOP_RIGHT", "positioned": false},
            "cps": {"enabled": false, "anchor": "TOP_RIGHT", "positioned": true},
            "watermark": {"enabled": true}
        }});
        let mut scan = Scan::default();
        new_profile("Current", &p, &mut scan);
        let found = &scan.clients[0];
        let v = &found.settings.values;
        assert_eq!(v["hud"]["fps"]["ay"], json!(0));
        assert_eq!(v["hud"]["fps"]["dy"], json!(13));
        assert_eq!(v["hud"]["clock"]["ax"], json!(2));
        assert_eq!(v["hud"]["clock"]["ay"], json!(2));
        assert_eq!(v["hud"]["ping"]["placed"], json!(false));
        assert_eq!(v["hud"]["cps"]["enabled"], json!(false));
        assert_eq!(found.notes, vec!["No Arctic match for: Watermark"]);
    }
}
