//! Vanilla `options.txt` representations that differ between game versions.

use std::collections::BTreeMap;

use crate::loaders::util::compare_versions;

type Values = BTreeMap<String, String>;

const CLOUDS: &str = "renderClouds";
const GRAPHICS: &str = "graphicsMode";
const PRESET: &str = "graphicsPreset";
const FANCY_GRAPHICS: &str = "fancyGraphics";
// The graphics split data fix still runs on files with an older version line.
const GRAPHICS_SPLIT_DATA_VERSION: i64 = 4651;
const PRESET_OPTIONS: &[&str] = &[
    CLOUDS,
    "ao",
    "biomeBlendRadius",
    "cloudRange",
    "cutoutLeaves",
    "entityDistanceScaling",
    "entityShadows",
    "improvedTransparency",
    "maxAnisotropyBit",
    "menuBackgroundBlurriness",
    "mipmapLevels",
    "particles",
    "prioritizeChunkUpdates",
    "renderDistance",
    "simulationDistance",
    "textureFiltering",
    "weatherRadius",
];

pub(crate) fn canonicalize(values: &mut Values) {
    let clouds = take(values, &[CLOUDS, "clouds"], |value| match value {
        "false" | "off" => Some("false"),
        "fast" => Some("fast"),
        "true" | "fancy" => Some("true"),
        _ => None,
    });
    values.extend(clouds.map(|value| (CLOUDS.to_owned(), value)));

    let graphics_mode = take(values, &[GRAPHICS, FANCY_GRAPHICS], |value| match value {
        "0" | "false" | "fast" => Some("fast"),
        "1" | "true" | "fancy" => Some("fancy"),
        "2" | "fabulous" => Some("fabulous"),
        _ => None,
    });
    let preset = take(values, &[PRESET], |value| match value {
        "fast" => Some("fast"),
        "fancy" => Some("fancy"),
        "fabulous" => Some("fabulous"),
        "custom" => Some("custom"),
        _ => None,
    });
    if let Some(value) = preset {
        values.insert(PRESET.to_owned(), value);
    } else {
        values.extend(graphics_mode.map(|value| (GRAPHICS.to_owned(), value)));
    }

    values.retain(|key, value| {
        let atom = unquote(value);
        let normalized = match key.as_str() {
            "ao" => match atom {
                "false" | "0" => "0".to_owned(),
                "1" => "1".to_owned(),
                "true" | "2" => "2".to_owned(),
                _ => return false,
            },
            "mainHand" if matches!(atom, "left" | "right") => atom.to_owned(),
            "mainHand" => return false,
            "soundDevice" => {
                let name = serde_json::from_str::<String>(value).unwrap_or_else(|_| value.clone());
                if name.contains(['\r', '\n']) {
                    return false;
                }
                quote(&name)
            }
            "lang" => atom.to_ascii_lowercase(),
            "guiScale" | "renderDistance" | "simulationDistance" | "maxFps" => {
                let Ok(number) = atom.parse::<i32>() else {
                    return false;
                };
                number.to_string()
            }
            "gamma" | "fov" => {
                let Some(number) = atom.parse::<f64>().ok().filter(|n| n.is_finite()) else {
                    return false;
                };
                crate::game_defaults::format_float(number)
            }
            _ => return true,
        };
        *value = normalized;
        true
    });
}

fn take(
    values: &mut Values,
    keys: &[&str],
    parse: impl Fn(&str) -> Option<&'static str>,
) -> Option<String> {
    let mut first = None;
    for key in keys {
        if let Some(value) = values.remove(*key) {
            first = first.or_else(|| parse(unquote(&value)).map(str::to_owned));
        }
    }
    first
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("a string can always be serialized as JSON")
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}

/// Mojang used YYwWWx identifiers for snapshots before 26.1.
fn since(game: Option<&str>, release: &str, snapshot: &str) -> bool {
    match game {
        Some(id) if id.as_bytes().get(2) == Some(&b'w') => id >= snapshot,
        Some(id) if id.starts_with(|c: char| c.is_ascii_digit()) => {
            compare_versions(id, release).is_ge()
        }
        Some(_) => false,
        None => true,
    }
}

/// The value an older client can represent and will save back to disk.
/// Remember it in canonical form so sharing can distinguish a lossy conversion
/// from a change the player made.
fn project(
    values: &mut Values,
    projected: &mut Values,
    key: &str,
    convert: impl FnOnce(&str) -> String,
) {
    if let Some(value) = values.get_mut(key) {
        let converted = convert(value);
        if *value != converted {
            projected.insert(key.to_owned(), converted.clone());
            *value = converted;
        }
    }
}

/// Overlay the canonical preferences in the target client's format. The
/// version line and game-specific values in `file` stay with that file.
pub(crate) fn apply(values: &Values, file: &mut Values, game: Option<&str>) -> Values {
    let names = crate::game_defaults_keys::names_keys(file);
    let (mut outgoing, mut projected) = crate::game_defaults_keys::for_file(values, game, names);
    canonicalize(&mut outgoing);
    // The 22w12a client was the first to save these strings with a JSON codec.
    let json = since(game, "1.19-pre1", "22w12a");
    if !since(game, "1.8.1-pre4", "15w31a") {
        project(&mut outgoing, &mut projected, CLOUDS, |value| {
            if value == "fast" { "true" } else { value }.to_owned()
        });
    }
    if let Some(clouds) = outgoing.remove(CLOUDS) {
        file.remove("clouds");
        file.remove(CLOUDS);
        let key = if since(game, "1.8-pre1", "14w25a") {
            CLOUDS
        } else {
            "clouds"
        };
        outgoing.insert(key.to_owned(), if json { quote(&clouds) } else { clouds });
    }

    if since(game, "1.19.3-rc1", "23w03a") {
        project(&mut outgoing, &mut projected, "ao", |value| {
            if value == "1" { "2" } else { value }.to_owned()
        });
        if let Some(value) = outgoing.get_mut("ao") {
            *value = (*value != "0").to_string();
        }
    }

    if json {
        if let Some(value) = outgoing.get_mut("mainHand") {
            *value = quote(value);
        }
        project(&mut outgoing, &mut projected, "gamma", |value| {
            crate::game_defaults::format_float(value.parse::<f64>().unwrap().clamp(0.0, 1.0))
        });
        project(&mut outgoing, &mut projected, "fov", |value| {
            let degrees = (value.parse::<f64>().unwrap() * 40.0 + 70.0) as i64;
            crate::game_defaults::fov_stored(degrees)
        });
        clamp(&mut outgoing, &mut projected, "maxFps", 10, 260);
    } else if let Some(value) = outgoing.get_mut("soundDevice") {
        *value = serde_json::from_str::<String>(value).expect("canonical sound device");
    }
    if !since(game, "1.11-pre1", "16w43a")
        && let Some(lang) = outgoing.get_mut("lang")
        && let Some((language, region)) = lang.rsplit_once('_')
    {
        *lang = format!("{language}_{}", region.to_ascii_uppercase());
    }

    if since(game, "1.13-pre1", "18w21a") {
        clamp(&mut outgoing, &mut projected, "guiScale", 0, i32::MAX - 1);
    } else {
        // These clients use the value to index a four-entry GUI scale label table.
        clamp(&mut outgoing, &mut projected, "guiScale", 0, 3);
    }
    clamp(&mut outgoing, &mut projected, "renderDistance", 2, 32);
    clamp(&mut outgoing, &mut projected, "simulationDistance", 5, 32);
    apply_graphics(&mut outgoing, file, game, &mut projected);
    file.extend(outgoing);
    projected
}

fn apply_graphics(
    outgoing: &mut Values,
    file: &mut Values,
    game: Option<&str>,
    projected: &mut Values,
) {
    let preset = outgoing.remove(PRESET);
    let mode = outgoing.remove(GRAPHICS);
    let target_has_presets = since(game, "1.21.11-pre1", "25w41a");
    let preset_override =
        target_has_presets && PRESET_OPTIONS.iter().any(|key| outgoing.contains_key(*key));
    if preset.is_none() && mode.is_none() && !preset_override {
        return;
    }

    let setting = |key: &str| {
        outgoing
            .get(key)
            .or_else(|| file.get(key))
            .map(String::as_str)
    };
    let inferred = match (setting("improvedTransparency"), setting("cutoutLeaves")) {
        (Some("true"), _) => "fabulous",
        (_, Some("false")) => "fast",
        _ => "fancy",
    };
    let fallback = mode
        .as_deref()
        .or(preset.as_deref().filter(|p| *p != "custom"))
        .unwrap_or(inferred);
    let index = graphics_index(fallback);
    for key in [GRAPHICS, FANCY_GRAPHICS, PRESET] {
        file.remove(key);
    }

    if target_has_presets {
        // Minecraft reapplies its preset during startup, overwriting individual options.
        let selected = preset
            .as_deref()
            .filter(|preset| fits_preset(preset, outgoing))
            .unwrap_or("custom");
        outgoing.insert(PRESET.to_owned(), quote(selected));
        if let Some(mode) = mode.as_deref() {
            for (key, value) in split_graphics(graphics_index(mode)) {
                outgoing
                    .entry(key.to_owned())
                    .or_insert_with(|| value.to_owned());
            }
        }
        if file
            .get("version")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
            < GRAPHICS_SPLIT_DATA_VERSION
        {
            // Feed the old graphics mode to Minecraft's migration, which sets Custom
            // and populates four split options itself.
            outgoing.insert(GRAPHICS.to_owned(), index.to_string());
            if selected != "custom" {
                projected.insert(PRESET.to_owned(), "custom".to_owned());
            }
            for (key, value) in split_graphics(index) {
                if outgoing.get(key).is_some_and(|v| v != value) {
                    projected.insert(key.to_owned(), value.to_owned());
                }
            }
        }
        return;
    }

    let echoed = if since(game, "1.16-pre1", "20w23a") {
        outgoing.insert(GRAPHICS.to_owned(), index.to_string());
        fallback
    } else {
        outgoing.insert(FANCY_GRAPHICS.to_owned(), (index != 0).to_string());
        if index == 0 { "fast" } else { "fancy" }
    };
    if mode.as_deref() != Some(echoed) {
        projected.insert(GRAPHICS.to_owned(), echoed.to_owned());
    }
}

fn fits_preset(preset: &str, values: &Values) -> bool {
    let (clouds, render, simulation) = match preset {
        "fast" => ("fast", "8", "6"),
        "fancy" => ("true", "16", "12"),
        "fabulous" => ("true", "32", "12"),
        _ => return true,
    };
    [
        (CLOUDS, clouds),
        ("renderDistance", render),
        ("simulationDistance", simulation),
    ]
    .into_iter()
    .all(|(key, expected)| {
        values
            .get(key)
            .is_none_or(|value| unquote(value) == expected)
    })
}

fn clamp(values: &mut Values, projected: &mut Values, key: &str, min: i32, max: i32) {
    project(values, projected, key, |value| {
        value.parse::<i32>().unwrap().clamp(min, max).to_string()
    });
}

fn graphics_index(mode: &str) -> i32 {
    match mode {
        "fast" => 0,
        "fabulous" => 2,
        _ => 1,
    }
}

fn split_graphics(index: i32) -> [(&'static str, &'static str); 4] {
    // Mirrors Minecraft's OptionsGraphicsModeSplitFix.
    [
        ("cutoutLeaves", if index == 0 { "false" } else { "true" }),
        ("weatherRadius", if index == 0 { "5" } else { "10" }),
        ("vignette", if index == 0 { "false" } else { "true" }),
        (
            "improvedTransparency",
            if index == 2 { "true" } else { "false" },
        ),
    ]
}

pub(crate) fn merge(known: &mut Values, mut captured: Values) {
    if captured.contains_key(PRESET) {
        known.remove(GRAPHICS);
    } else if let Some(mode) = captured.get(GRAPHICS) {
        known.remove(PRESET);
        // A changed old graphics setting also changes the options it used to control.
        if known.get(GRAPHICS) != Some(mode) {
            for (key, value) in split_graphics(graphics_index(mode)) {
                captured
                    .entry(key.to_owned())
                    .or_insert_with(|| value.to_owned());
            }
        }
    } else if PRESET_OPTIONS.iter().any(|key| {
        captured
            .get(*key)
            .is_some_and(|value| known.get(*key) != Some(value))
    }) && let Some(preset) = known.get_mut(PRESET)
    {
        *preset = "custom".to_owned();
    }
    known.extend(captured);
}
