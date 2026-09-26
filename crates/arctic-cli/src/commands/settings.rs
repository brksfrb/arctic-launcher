//! `arctic settings …`: the launcher settings (settings.json), by key.

use arctic_core::settings::Settings;
use arctic_core::{Error, Result};
use serde_json::{Map, Value, json};

use super::Ctx;
use crate::cli::SettingsCommand;

/// Bookkeeping the launcher keeps for itself.
const INTERNAL: [&str; 4] = [
    "last_version",
    "last_instance",
    "client_style_set",
    "favorite_versions",
];
/// Picking these tells the game to adopt the launcher's choice.
const CLIENT_LOOK: [&str; 2] = ["client_style", "client_fancy"];

pub fn run(ctx: &Ctx, command: &SettingsCommand) -> Result<i32> {
    let settings = Settings::load(&ctx.dirs)?;
    let map = to_map(&settings)?;
    match command {
        SettingsCommand::Show => show(ctx, &map),
        SettingsCommand::Get { key } => {
            let value = public(&map, key)?;
            ctx.out
                .emit(json!({ key.as_str(): value }), || plain(value));
        }
        SettingsCommand::Set { key, value } => {
            let parsed = parse(key, value, public(&map, key)?);
            let mut map = map;
            map.insert(key.clone(), parsed);
            let mut updated: Settings = serde_json::from_value(Value::Object(map))
                .map_err(|e| Error::Other(format!("invalid value for {key}: {e}")))?;
            if CLIENT_LOOK.contains(&key.as_str()) {
                updated.client_style_set = arctic_core::auth::now_secs();
            }
            updated.save(&ctx.dirs)?;
            // Show what was stored (values can be clamped).
            let saved = to_map(&Settings::load(&ctx.dirs)?)?;
            let value = &saved[key.as_str()];
            ctx.out.emit(json!({ key.as_str(): value }), || {
                format!("{key} = {}", plain(value))
            });
        }
    }
    Ok(0)
}

fn show(ctx: &Ctx, map: &Map<String, Value>) {
    let visible: Map<String, Value> = map
        .iter()
        .filter(|(k, _)| !INTERNAL.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let width = visible.keys().map(String::len).max().unwrap_or(0);
    ctx.out.emit(Value::Object(visible.clone()), || {
        visible
            .iter()
            .map(|(k, v)| format!("{k:<width$}  {}", plain(v)))
            .collect::<Vec<_>>()
            .join("\n")
    });
}

fn to_map(settings: &Settings) -> Result<Map<String, Value>> {
    match serde_json::to_value(settings) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(Error::Other(
            "settings didn't serialize to an object".into(),
        )),
    }
}

fn public<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a Value> {
    map.get(key)
        .filter(|_| !INTERNAL.contains(&key))
        .ok_or_else(|| {
            Error::Other(format!(
                "unknown setting '{key}' (see `arctic settings show`)"
            ))
        })
}

/// Text for text settings; otherwise JSON when it parses (numbers,
/// true/false), else text. `none` clears an optional path.
fn parse(key: &str, value: &str, current: &Value) -> Value {
    if key == "java_override" && (value.is_empty() || value == "none") {
        return Value::Null;
    }
    if current.is_string() {
        return Value::String(value.to_owned());
    }
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
}

fn plain(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "none".into(),
        other => other.to_string(),
    }
}
