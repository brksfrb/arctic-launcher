//! A whole profile in a bundle: launcher settings, instances (as mod
//! lists) and each instance's Arctic Client settings. Never accounts,
//! tokens, the proxy, Java paths or JVM flags.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::instance::InstancePack;
use crate::instances::{self, Instance};
use crate::mods::Skipped;
use crate::settings::Settings;
use crate::storage::DataDirs;
use crate::{Error, Progress, Result};

const MAX_INSTANCES: usize = 100;

/// Launcher settings a profile carries (look, behaviour, memory, window).
const SETTINGS: &[&str] = &[
    "animations",
    "intro",
    "start_maximized",
    "theme",
    "on_game_start",
    "discord_presence",
    "tray",
    "show_snapshots",
    "show_old_versions",
    "window_width",
    "window_height",
    "fullscreen",
    "max_memory_mb",
    "min_memory_mb",
    "client_style",
    "client_fancy",
    "check_updates_on_start",
    "favorite_versions",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilePack {
    pub settings: Map<String, Value>,
    /// The built-in Vanilla instance (its options and client settings).
    #[serde(default)]
    pub vanilla: Option<InstancePack>,
    #[serde(default)]
    pub instances: Vec<InstancePack>,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileReport {
    pub created: Vec<Instance>,
    /// Instances left alone because one with the same name exists.
    pub existing: Vec<String>,
    pub skipped_mods: Vec<Skipped>,
}

impl ProfilePack {
    /// This profile; also returns mod files that couldn't be included.
    pub fn export(dirs: &DataDirs) -> Result<(Self, Vec<String>)> {
        let settings = Settings::load(dirs)?;
        let all = to_map(&settings)?;
        let settings = SETTINGS
            .iter()
            .filter_map(|k| all.get(*k).map(|v| ((*k).to_owned(), v.clone())))
            .collect();
        let vanilla = InstancePack::export(dirs, &instances::load_default(dirs)?, None, true)?;
        let mut left_out = vanilla.left_out;
        let mut packs = Vec::new();
        for instance in instances::list_custom(dirs)? {
            let e = InstancePack::export(dirs, &instance, None, true)?;
            left_out.extend(
                e.left_out
                    .into_iter()
                    .map(|f| format!("{}: {f}", instance.name)),
            );
            packs.push(e.pack);
        }
        Ok((
            Self {
                settings,
                vanilla: Some(vanilla.pack),
                instances: packs,
            },
            left_out,
        ))
    }

    pub fn check(self) -> Result<Self> {
        if self.instances.len() > MAX_INSTANCES {
            return Err(Error::Other("this profile has too many instances".into()));
        }
        let settings: Map<String, Value> = self
            .settings
            .into_iter()
            .filter(|(k, _)| SETTINGS.contains(&k.as_str()))
            .collect();
        // Every value must fit the settings file.
        merged(&Settings::default(), &settings)?;
        let vanilla = self.vanilla.map(InstancePack::check).transpose()?;
        let instances = self
            .instances
            .into_iter()
            .map(|i| {
                let i = i.check()?;
                if i.game_version.is_none() {
                    return Err(Error::Other(
                        "an instance in this profile has no version".into(),
                    ));
                }
                Ok(i)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            settings,
            vanilla,
            instances,
        })
    }

    /// Apply to this PC's profile: settings, the Vanilla instance's options,
    /// and new instances (downloading their mods).
    pub fn import(&self, dirs: &DataDirs, progress: Progress) -> Result<ProfileReport> {
        let vanilla_dir = instances::load_default(dirs)?.game_dir(dirs);
        if self.vanilla.as_ref().is_some_and(|v| v.client.is_some())
            && super::client::in_use(&vanilla_dir)
        {
            return Err(Error::Other(
                "Minecraft is running from the Vanilla instance; close it first".into(),
            ));
        }
        let current = Settings::load(dirs)?;
        let mut updated = merged(&current, &self.settings)?;
        if updated.client_style != current.client_style
            || updated.client_fancy != current.client_fancy
        {
            updated.client_style_set = crate::auth::now_secs();
        }
        updated.save(dirs)?;
        let mut report = ProfileReport::default();
        if let Some(pack) = &self.vanilla {
            let default = instances::load_default(dirs)?;
            let vanilla = Instance {
                icon: pack.icon,
                performance: pack.performance,
                arctic_mod: pack.arctic_mod,
                max_memory_mb: pack.max_memory_mb,
                ..default
            };
            vanilla.save(dirs)?;
            pack.fill(dirs, &vanilla, progress)?;
        }
        let taken: Vec<String> = instances::list_custom(dirs)?
            .into_iter()
            .map(|i| i.name.to_lowercase())
            .collect();
        for pack in &self.instances {
            if taken.contains(&pack.name.to_lowercase()) {
                report.existing.push(pack.name.clone());
                continue;
            }
            let (instance, mods) = pack.install(dirs, progress)?;
            report.created.push(instance);
            report.skipped_mods.extend(mods.skipped);
        }
        Ok(report)
    }

    pub fn summary(&self) -> String {
        let mods: usize = self.instances.iter().map(|i| i.mods.len()).sum();
        format!(
            "A profile: {} setting{}, {} instance{} ({mods} mods)",
            self.settings.len(),
            if self.settings.len() == 1 { "" } else { "s" },
            self.instances.len(),
            if self.instances.len() == 1 { "" } else { "s" },
        )
    }
}

fn to_map(settings: &Settings) -> Result<Map<String, Value>> {
    match serde_json::to_value(settings)? {
        Value::Object(map) => Ok(map),
        _ => Err(Error::Other("settings aren't an object".into())),
    }
}

/// `base` with `values` on top.
fn merged(base: &Settings, values: &Map<String, Value>) -> Result<Settings> {
    let mut map = to_map(base)?;
    for (k, v) in values {
        map.insert(k.clone(), v.clone());
    }
    serde_json::from_value::<Settings>(Value::Object(map))
        .map(Settings::sanitized)
        .map_err(|e| Error::Other(format!("this profile's settings can't be used: {e}")))
}
