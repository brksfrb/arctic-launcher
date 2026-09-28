//! Badlion Client mod profiles (`.minecraft/BLClient-Mod-Profiles*/*.json`,
//! plain JSON). Each HUD mod has boxes on a 32 × 32 grid over the screen:
//! `(xBox + xOffset) / 32` of the width, `(yBox + yOffset) / 32` of the
//! height, for its top-left, centre and bottom-right.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::{Anchor, HUD_VERSION, blank_hud, push, restack, slot, unstack};
use crate::migrate::{Launcher, Scan};

const GRID: f64 = 32.0;
/// Screen size assumed when the profile doesn't say (it saves 0 until used).
const DEFAULT_DISPLAY: (f64, f64) = (1920.0, 1080.0);

const BADLION: &[(&str, &str)] = &[
    ("showFPS", "fps"),
    ("showCPS", "cps"),
    ("coordinates", "coords"),
    ("keyStroke", "keystrokes"),
    ("armorStatus", "armor"),
    ("potionStatus", "effects"),
    ("showDirection", "direction"),
    ("showPing", "ping"),
    ("showTimeIcon", "clock"),
    ("reachDisplay", "reach"),
    ("toggleSneak", "movement"),
    ("showArrows", "arrows"),
];

/// On-screen mods that aren't HUD widgets (or are parts of one).
const NOT_WIDGETS: &[&str] = &["crosshair", "fovChanger", "notification", "scoreboard"];

pub fn scan(dot_minecraft: &Path, scan: &mut Scan) {
    let Ok(entries) = std::fs::read_dir(dot_minecraft) else {
        return;
    };
    let mut folders: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("BLClient-Mod-Profiles"))
        })
        .collect();
    folders.sort();
    for folder in folders {
        let suffix = folder
            .file_name()
            .map(|n| {
                n.to_string_lossy()
                    .trim_start_matches("BLClient-Mod-Profiles")
                    .trim_start_matches('-')
                    .to_owned()
            })
            .unwrap_or_default();
        let Ok(files) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut files: Vec<_> = files
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for file in files {
            let Some(json) = crate::migrate::detect::read_json(&file) else {
                continue;
            };
            let stem = file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = if suffix.is_empty() {
                stem
            } else {
                format!("{stem} ({suffix})")
            };
            profile(&name, &json, scan);
        }
    }
}

fn profile(name: &str, p: &Value, scan: &mut Scan) {
    if !p.is_object() || p.get("version").is_none() {
        return;
    }
    let gui = gui_size(p);
    let mut hud = blank_hud();
    for (their, ours) in BADLION {
        let Some(m) = p.get(*their) else {
            continue;
        };
        if !is_on(m) || hud[*ours]["enabled"] == Value::Bool(true) {
            continue;
        }
        let background = m
            .pointer("/backgroundColor/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        hud.insert((*ours).into(), slot(true, place(m, gui), 1.0, background));
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    if let Some(c) = p.get("crosshair").filter(|c| is_on(c)) {
        values.insert("crosshair".into(), crosshair(c));
    }
    if let Some(t) = p.get("toggleSneak").filter(|t| is_on(t)) {
        let mode = t
            .get("toggleSneakMode")
            .and_then(Value::as_str)
            .unwrap_or("SPRINT")
            .to_ascii_uppercase();
        values.insert(
            "toggleSprint".into(),
            json!(!mode.contains("SNEAK") || mode.contains("SPRINT")),
        );
        values.insert(
            "toggleSneak".into(),
            json!(mode.contains("SNEAK") || mode == "BOTH"),
        );
    }
    for (their, key) in [
        ("perspective", "freelookEnabled"),
        ("fullbright", "fullbright"),
    ] {
        if let Some(on) = p
            .get(their)
            .and_then(|m| m.get("enabled"))
            .and_then(Value::as_bool)
        {
            values.insert(key.into(), json!(on));
        }
    }
    let left_out = left_out(p);
    let mut profile = name.to_owned();
    if p.get("active").and_then(Value::as_bool) == Some(true) {
        profile.push_str(" · active");
    }
    push(
        scan,
        Launcher::Badlion,
        &profile,
        values,
        Vec::new(),
        left_out,
    );
}

fn is_on(m: &Value) -> bool {
    m.get("enabled").and_then(Value::as_bool) == Some(true)
}

/// GUI size of the screen the layout was made on (Minecraft's automatic
/// GUI scale), so offsets come out in GUI pixels.
fn gui_size(p: &Value) -> (f64, f64) {
    let dim = |keys: [&str; 2]| {
        keys.iter()
            .find_map(|k| p.get(*k).and_then(Value::as_f64).filter(|v| *v > 0.0))
    };
    let width = dim(["lastDisplayWidth", "startDisplayX"]);
    let height = dim(["lastDisplayHeight", "startDisplayY"]);
    // v2 keeps the display size on each mod instead.
    let from_mod = || {
        p.as_object()?.values().find_map(|m| {
            let x = m.get("startDisplayX")?.as_f64().filter(|v| *v > 0.0)?;
            let y = m.get("startDisplayY")?.as_f64().filter(|v| *v > 0.0)?;
            Some((x, y))
        })
    };
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        _ => from_mod().unwrap_or(DEFAULT_DISPLAY),
    };
    let mut scale = 1.0;
    while w / (scale + 1.0) >= 320.0 && h / (scale + 1.0) >= 240.0 {
        scale += 1.0;
    }
    (w / scale, h / scale)
}

/// The mod's box as screen fractions, pinned to the nearest edge like the
/// Arctic Client's HUD editor does. Never-placed mods (all zero) stay
/// arranged automatically.
fn place(m: &Value, (w, h): (f64, f64)) -> Option<Anchor> {
    let corner = |key: &str| {
        let b = m.get(key)?;
        let x = b.get("xBox")?.as_f64()? + b.get("xOffset")?.as_f64()?;
        let y = b.get("yBox")?.as_f64()? + b.get("yOffset")?.as_f64()?;
        Some(((x / GRID).clamp(0.0, 1.0), (y / GRID).clamp(0.0, 1.0)))
    };
    let (x0, y0) = corner("topLeftBox")?;
    let (x1, y1) = corner("bottomRightBox")?;
    if x1 <= x0 && y1 <= y0 {
        return None;
    }
    let (ax, dx) = axis(x0, x1, w);
    let (ay, dy) = axis(y0, y1, h);
    Some((ax, ay, dx, dy))
}

/// Anchor (start, centre, end) and inward offset in GUI pixels for a box
/// spanning `lo..hi` (fractions) of a screen `size` GUI pixels long.
fn axis(lo: f64, hi: f64, size: f64) -> (i64, i64) {
    let center = (lo + hi) / 2.0;
    let px = |f: f64| (f * size).round() as i64;
    if center < 1.0 / 3.0 {
        (0, px(lo))
    } else if center > 2.0 / 3.0 {
        (2, px(1.0 - hi))
    } else {
        (1, px(center - 0.5))
    }
}

/// Badlion's crosshair fields as Arctic's (clamped by the settings check).
fn crosshair(c: &Value) -> Value {
    let num = |k: &str, default: f64| c.get(k).and_then(Value::as_f64).unwrap_or(default);
    let flag = |k: &str| c.pointer(&format!("/{k}/value")).and_then(Value::as_bool) == Some(true);
    let dot = flag("dot");
    let style = match c.get("selected").and_then(Value::as_str).unwrap_or("CROSS") {
        s if s.eq_ignore_ascii_case("dot") => "dot",
        s if s.to_ascii_uppercase().contains("CIRCLE") => "circle",
        _ if dot => "cross-dot",
        _ => "cross",
    };
    let size = num("width", 3.0).max(num("height", 3.0));
    json!({
        "enabled": true,
        "style": style,
        "size": (size.round() as i64).clamp(1, 12),
        "gap": (num("gap", 2.0).round() as i64).clamp(0, 8),
        "thickness": (num("thickness", 1.0).round() as i64).clamp(1, 4),
        "color": color(c.pointer("/crosshairColor/color")).unwrap_or(-1),
        "outline": flag("outline"),
    })
}

/// `{red, green, blue, alpha}` as signed bytes (-1 = 255) → ARGB int.
fn color(c: Option<&Value>) -> Option<i64> {
    let c = c?;
    let byte = |k: &str| c.get(k).and_then(Value::as_i64).map(|v| (v & 0xff) as u32);
    let argb =
        (byte("alpha")? << 24) | (byte("red")? << 16) | (byte("green")? << 8) | byte("blue")?;
    Some(i64::from(argb as i32))
}

/// Switched-on HUD mods Arctic has no widget for ("Arrows", "Food").
fn left_out(p: &Value) -> Vec<String> {
    let Some(map) = p.as_object() else {
        return Vec::new();
    };
    map.iter()
        .filter(|(k, m)| {
            is_on(m)
                && m.get("centerBox").is_some()
                && !NOT_WIDGETS.contains(&k.as_str())
                && !k.starts_with("armorStatusPiece")
                && !BADLION.iter().any(|(t, _)| t == k)
        })
        .map(|(k, _)| words(k))
        .collect()
}

/// `showEnchantedGapple` → `Enchanted Gapple`.
fn words(key: &str) -> String {
    let key = key.strip_prefix("show").unwrap_or(key);
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(if i == 0 { c.to_ascii_uppercase() } else { c });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: i64, xo: f64, y: i64, yo: f64) -> Value {
        json!({"xBox": x, "xOffset": xo, "yBox": y, "yOffset": yo})
    }

    #[test]
    fn grid_boxes_become_edge_anchors() {
        let gui = (480.0, 270.0);
        // Top-left FPS.
        let fps = json!({"topLeftBox": b(1, 0.24, 0, 0.56), "bottomRightBox": b(3, 0.91, 1, 0.88)});
        assert_eq!(place(&fps, gui), Some((0, 0, 19, 5)));
        // Keystrokes in the top-right corner, reaching the edge.
        let keys =
            json!({"topLeftBox": b(29, 0.12, 0, 0.0), "bottomRightBox": b(32, 0.05, 6, 0.98)});
        assert_eq!(place(&keys, gui), Some((2, 0, 0, 0)));
        // Centred at the top.
        let dir = json!({"topLeftBox": b(12, 0.88, 0, 0.0), "bottomRightBox": b(19, 0.05, 1, 0.3)});
        assert_eq!(place(&dir, gui).map(|a| (a.0, a.1)), Some((1, 0)));
        // Bottom-right armor.
        let armor =
            json!({"topLeftBox": b(30, 0.78, 28, 0.09), "bottomRightBox": b(32, 0.0, 32, 0.0)});
        assert_eq!(place(&armor, gui).map(|a| (a.0, a.1)), Some((2, 2)));
        // Never placed.
        let zero = json!({"topLeftBox": b(0, 0.0, 0, 0.0), "bottomRightBox": b(0, 0.0, 0, 0.0)});
        assert_eq!(place(&zero, gui), None);
    }

    #[test]
    fn gui_size_uses_the_saved_display() {
        assert_eq!(
            gui_size(&json!({"version": 3, "lastDisplayWidth": 0})),
            (480.0, 270.0)
        );
        let v2 =
            json!({"version": 2, "showFPS": {"startDisplayX": 1280.0, "startDisplayY": 720.0}});
        assert_eq!(gui_size(&v2), (1280.0 / 3.0, 240.0));
    }

    #[test]
    fn reads_a_profile() {
        let p = json!({
            "version": 3, "active": true,
            "showFPS": {"enabled": true, "backgroundColor": {"enabled": false},
                "topLeftBox": b(1, 0.24, 0, 0.56), "bottomRightBox": b(3, 0.91, 1, 0.88), "centerBox": b(2, 0.5, 1, 0.2)},
            "showCPS": {"enabled": false, "centerBox": b(30, 0.2, 5, 0.6)},
            "toggleSneak": {"enabled": true, "toggleSneakMode": "SPRINT", "centerBox": b(27, 0.7, 0, 0.2),
                "topLeftBox": b(26, 0.0, 0, 0.0), "bottomRightBox": b(29, 0.4, 0, 0.5)},
            "showArrows": {"enabled": true, "centerBox": b(19, 0.6, 31, 0.6)},
            "showBlockInfo": {"enabled": true, "centerBox": b(10, 0.6, 31, 0.6)},
            "armorStatusPieceZero": {"enabled": true, "centerBox": b(0, 0.4, 23, 0.0)},
            "crosshair": {"enabled": true, "selected": "CROSS", "width": 4.0, "height": 3.0, "gap": 2.0,
                "thickness": 1.0, "dot": {"value": true}, "outline": {"value": false},
                "crosshairColor": {"color": {"red": -1, "green": 0, "blue": 0, "alpha": -1}}},
            "fullbright": {"enabled": true}
        });
        let mut scan = Scan::default();
        profile("Default", &p, &mut scan);
        let found = &scan.clients[0];
        assert_eq!(found.profile, "Default · active");
        let v = &found.settings.values;
        assert_eq!(v["hud"]["fps"]["enabled"], json!(true));
        assert_eq!(v["hud"]["fps"]["background"], json!(false));
        assert_eq!(v["hud"]["cps"]["enabled"], json!(false));
        assert_eq!(v["hud"]["movement"]["enabled"], json!(true));
        assert_eq!(v["toggleSprint"], json!(true));
        assert_eq!(v["fullbright"], json!(true));
        assert_eq!(v["crosshair"]["style"], json!("cross-dot"));
        assert_eq!(v["crosshair"]["size"], json!(4));
        assert_eq!(v["crosshair"]["color"], json!(0xFFFF_0000_u32 as i32));
        assert_eq!(v["hud"]["arrows"]["enabled"], json!(true));
        assert_eq!(found.notes, vec!["No Arctic match for: Block Info"]);
    }
}
