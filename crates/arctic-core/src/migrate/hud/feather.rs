//! Feather (now Dawn) module profiles: `feather/configuration/profiles/
//! <name>.json` in the game folder. A profile names a module preset (which
//! mods are on) and keeps only what the player changed, per mod. Widgets
//! nobody moved sit where Feather arranges them, so Arctic arranges them too.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::{HUD_VERSION, blank_hud, push, restack, slot, unstack};
use crate::migrate::{Launcher, Scan};

const FEATHER: &[(&str, &str)] = &[
    ("fps", "fps"),
    ("cps", "cps"),
    ("coordinates", "coords"),
    ("keystrokes", "keystrokes"),
    ("armorStatus", "armor"),
    ("potionEffects", "effects"),
    ("direction", "direction"),
    ("ping", "ping"),
    ("time", "clock"),
    ("systemresources", "memory"),
    ("serverAddress", "server"),
    ("speedMeter", "speed"),
    ("reachDisplay", "reach"),
    ("comboDisplay", "combo"),
    ("toggleSprint", "movement"),
    ("saturation", "food"),
    ("playtime", "session"),
    ("tps", "tps"),
    ("packdisplay", "pack"),
];

/// HUD modules Arctic has no widget for (named when they're on).
const OTHER_HUD: &[(&str, &str)] = &[
    ("itemInfo1", "Item Info"),
    ("itemCounter", "Item Counter"),
    ("stopwatch1", "Stopwatch"),
    ("mousestrokes", "Mouse Strokes"),
    ("armorBar", "Armor Bar"),
    ("totem", "Totem Counter"),
];

/// Modules each built-in preset switches on (from its `settings`), limited
/// to the ones read here.
const PRESETS: &[(&str, &[&str])] = &[
    (
        "default",
        &[
            "fps",
            "ping",
            "armorStatus",
            "potionEffects",
            "toggleSprint",
            "armorBar",
            "itemInfo1",
            "saturation",
        ],
    ),
    (
        "fps",
        &[
            "fps",
            "ping",
            "armorStatus",
            "potionEffects",
            "armorBar",
            "saturation",
            "totem",
        ],
    ),
    (
        "casual",
        &[
            "fps",
            "ping",
            "armorStatus",
            "potionEffects",
            "armorBar",
            "saturation",
        ],
    ),
];

const SELECTED: &str = "**SelectedPresets**";
const DATA: &str = "**ArbitraryData**";

/// Feather folders to look in: `.minecraft/feather` (Feather launcher) and
/// Dawn's synced game folders.
pub fn roots(dot_minecraft: Option<&Path>, homes: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = dot_minecraft
        .map(|m| m.join("feather"))
        .into_iter()
        .collect();
    for home in homes {
        let groups = home.join(".dawn").join("auto-sync").join("groups");
        if let Ok(entries) = std::fs::read_dir(groups) {
            let mut dirs: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path().join("feather"))
                .collect();
            dirs.sort();
            out.extend(dirs);
        }
    }
    out.retain(|d| d.join("configuration").join("profiles").is_dir());
    out
}

pub fn scan(feather: &Path, scan: &mut Scan) {
    let config = feather.join("configuration");
    let current = std::fs::read_to_string(config.join("current.profile"))
        .map(|s| s.trim().to_owned())
        .unwrap_or_default();
    let Ok(entries) = std::fs::read_dir(config.join("profiles")) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
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
        let mut name = stem.clone();
        if stem == current {
            name.push_str(" · active");
        }
        profile(&name, &json, scan);
    }
}

fn profile(name: &str, p: &Value, scan: &mut Scan) {
    let preset = p
        .pointer(&format!("/{SELECTED}/module/id"))
        .and_then(Value::as_str)
        .unwrap_or("default");
    let preset_on = PRESETS
        .iter()
        .find(|(id, _)| *id == preset)
        .map(|(_, on)| *on);
    let on = |module: &str| {
        changed_on(p, module)
            .unwrap_or_else(|| preset_on.is_some_and(|list| list.contains(&module)))
    };
    let mut hud = blank_hud();
    for (their, ours) in FEATHER {
        if on(their) && hud[*ours]["enabled"] != Value::Bool(true) {
            hud.insert((*ours).into(), slot(true, None, 1.0, background(p, their)));
        }
    }
    unstack(&mut hud);
    restack(&mut hud);
    let mut values = Map::new();
    values.insert("hud".into(), Value::Object(hud));
    values.insert("hudVersion".into(), json!(HUD_VERSION));
    values.insert("toggleSprint".into(), json!(on("toggleSprint")));
    for (their, key) in [("zoom", "zoomEnabled"), ("perspective", "freelookEnabled")] {
        if let Some(v) = changed_on(p, their) {
            values.insert(key.into(), json!(v));
        }
    }
    let left_out: Vec<String> = OTHER_HUD
        .iter()
        .filter(|(id, _)| on(id))
        .map(|(_, label)| (*label).to_owned())
        .collect();
    push(scan, Launcher::Feather, name, values, Vec::new(), left_out);
    if preset_on.is_none()
        && let Some(found) = scan.clients.last_mut()
    {
        found.notes.push(format!(
            "Preset \"{preset}\" isn't one Arctic knows; only mods you switched on or off yourself came along"
        ));
    }
}

/// On or off as the player set it (`true`, `"true"`), if they did.
fn changed_on(p: &Value, module: &str) -> Option<bool> {
    let m = p.get(module)?;
    [m.get("enabled"), m.get(DATA).and_then(|d| d.get("enabled"))]
        .into_iter()
        .flatten()
        .find_map(|v| match v {
            Value::Bool(b) => Some(*b),
            Value::String(s) => s.parse().ok(),
            _ => None,
        })
}

/// Feather's `background: "false"` / `displayMode: "justText"` hide the panel.
fn background(p: &Value, module: &str) -> bool {
    let Some(m) = p.get(module) else {
        return true;
    };
    let data = m.get(DATA).unwrap_or(m);
    let text = |k: &str| data.get(k).or_else(|| m.get(k)).and_then(Value::as_str);
    !(text("background") == Some("false") || text("displayMode") == Some("justText"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_and_changes() {
        // The live default profile: nothing changed from the default preset.
        let p = json!({
            SELECTED: {"module": {"id": "default"}},
            "fps": {DATA: {}}, "cps": {DATA: {"enabled": "true", "displayMode": "justText"}},
            "ping": {DATA: {"enabled": false}}, "zoom": {DATA: {"enabled": true}}
        });
        let mut scan = Scan::default();
        profile("default · active", &p, &mut scan);
        let v = &scan.clients[0].settings.values;
        assert_eq!(v["hud"]["fps"]["enabled"], json!(true));
        assert_eq!(v["hud"]["fps"]["placed"], json!(false));
        assert_eq!(v["hud"]["cps"]["enabled"], json!(true));
        assert_eq!(v["hud"]["cps"]["background"], json!(false));
        assert_eq!(v["hud"]["ping"]["enabled"], json!(false));
        assert_eq!(v["hud"]["armor"]["enabled"], json!(true));
        assert_eq!(v["hud"]["movement"]["enabled"], json!(true));
        assert_eq!(v["toggleSprint"], json!(true));
        assert_eq!(v["zoomEnabled"], json!(true));
        assert!(scan.clients[0].notes[0].contains("Item Info"));
    }

    #[test]
    fn unknown_preset_keeps_only_changes() {
        let p = json!({SELECTED: {"module": {"id": "pro"}}, "keystrokes": {"enabled": true}});
        let mut scan = Scan::default();
        profile("mine", &p, &mut scan);
        let found = &scan.clients[0];
        assert_eq!(
            found.settings.values["hud"]["keystrokes"]["enabled"],
            json!(true)
        );
        assert_eq!(found.settings.values["hud"]["fps"]["enabled"], json!(false));
        assert!(found.notes.iter().any(|n| n.contains("\"pro\"")));
    }

    #[test]
    fn finds_feather_and_dawn_folders() {
        let home = tempfile::tempdir().unwrap();
        let mc = home.path().join(".minecraft");
        std::fs::create_dir_all(mc.join("feather/configuration/profiles")).unwrap();
        let dawn = home
            .path()
            .join(".dawn/auto-sync/groups/default/feather/configuration/profiles");
        std::fs::create_dir_all(&dawn).unwrap();
        std::fs::write(
            dawn.join("default.json"),
            r#"{"fps":{"**ArbitraryData**":{}}}"#,
        )
        .unwrap();
        std::fs::write(dawn.join("../current.profile"), "default").unwrap();
        let found = roots(Some(&mc), &[home.path().to_path_buf()]);
        assert_eq!(found.len(), 2);
        let mut scan = Scan::default();
        super::scan(&found[1], &mut scan);
        assert_eq!(scan.clients[0].profile, "default · active");
    }
}
