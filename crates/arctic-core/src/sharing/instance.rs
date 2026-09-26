//! An instance in a bundle: its Minecraft version, loader and the exact
//! Modrinth versions of its mods (no jars, so it stays tiny). Java paths
//! and JVM flags are never shared: they could run programs on the other PC.

use serde::{Deserialize, Serialize};

use super::client::{ClientPart, Part};
use crate::instances::{self, Instance, InstanceIcon, Loader};
use crate::mods::{self, ExactReport, Wanted};
use crate::storage::DataDirs;
use crate::{Error, Progress, Result};

const MAX_MODS: usize = 1000;
const MAX_NAME: usize = 48;
const MAX_TITLE: usize = 100;
const MAX_VERSION: usize = 64;
const MAX_ID: usize = 32;
const MIN_MEMORY_MB: u32 = 512;
const MAX_MEMORY_MB: u32 = 65536;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedMod {
    /// Modrinth project and version ids.
    pub project: String,
    pub version: String,
    pub title: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Installed only because another mod needs it.
    #[serde(default)]
    pub dependency: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstancePack {
    pub name: String,
    /// `None` only for a profile's Vanilla instance (it follows the Play tab).
    pub game_version: Option<String>,
    pub loader: Loader,
    #[serde(default)]
    pub icon: InstanceIcon,
    #[serde(default = "yes")]
    pub performance: bool,
    #[serde(default = "yes")]
    pub arctic_mod: bool,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    #[serde(default)]
    pub mods: Vec<SharedMod>,
    /// Arctic Client settings; profiles only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<serde_json::Value>,
}

/// An exported instance, and the mod files that couldn't be included
/// (added by hand, not from Modrinth).
#[derive(Debug, Clone, PartialEq)]
pub struct Exported {
    pub pack: InstancePack,
    pub left_out: Vec<String>,
}

impl InstancePack {
    /// `play_version` stands in for the Vanilla instance's version (it
    /// follows the Play tab), so a friend gets the version you play; `None`
    /// keeps it following the Play tab (profiles).
    pub fn export(
        dirs: &DataDirs,
        instance: &Instance,
        play_version: Option<&str>,
        with_client: bool,
    ) -> Result<Exported> {
        let game_dir = instance.game_dir(dirs);
        let mods_dir = game_dir.join("mods");
        let index = mods::index_path(&dirs.instance_dir(&instance.id));
        let performance = mods::performance::installed_files(&game_dir);
        let mut shared = Vec::new();
        let mut left_out = Vec::new();
        for file in mods::list(&mods_dir, &index)? {
            match file.tracked {
                Some(m) => shared.push(SharedMod {
                    project: m.project_id,
                    version: m.version_id,
                    title: m.title,
                    enabled: file.enabled,
                    dependency: m.dependency,
                }),
                None if performance.contains(&file.file_name)
                    || file.file_name == crate::arctic_mod::FILE_NAME => {}
                None => left_out.push(file.file_name),
            }
        }
        let client = if with_client {
            ClientPart::read(&game_dir, Part::All)
                .ok()
                .map(|c| serde_json::Value::Object(c.values))
        } else {
            None
        };
        let game_version = instance
            .version
            .clone()
            .or_else(|| play_version.map(str::to_owned));
        Ok(Exported {
            pack: Self {
                name: instance.name.clone(),
                game_version,
                loader: instance.loader.clone(),
                icon: instance.icon,
                performance: instance.performance,
                arctic_mod: instance.arctic_mod,
                max_memory_mb: instance.max_memory_mb,
                mods: shared,
                client,
            },
            left_out,
        })
    }

    /// Reject anything a normal export couldn't have made.
    pub fn check(self) -> Result<Self> {
        let name = clean_text(&self.name, MAX_NAME).ok_or_else(|| bad("its name"))?;
        if let Some(v) = &self.game_version
            && !is_version(v, b" ")
        {
            return Err(bad("its Minecraft version"));
        }
        if let Some(v) = self.loader.version()
            && !is_version(v, b"")
        {
            return Err(bad("its loader version"));
        }
        if self.mods.len() > MAX_MODS {
            return Err(bad("too many mods"));
        }
        if self.loader.kind().is_none() && !self.mods.is_empty() {
            return Err(bad("mods without a mod loader"));
        }
        let mods = self
            .mods
            .into_iter()
            .map(|m| {
                let ok = is_id(&m.project) && is_id(&m.version);
                let title = clean_text(&m.title, MAX_TITLE).unwrap_or_else(|| m.project.clone());
                ok.then_some(SharedMod { title, ..m })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| bad("a mod id"))?;
        let client = match self.client {
            Some(c) => Some(serde_json::Value::Object(
                ClientPart::check(Part::All, &c)?.values,
            )),
            None => None,
        };
        Ok(Self {
            name,
            mods,
            client,
            max_memory_mb: self
                .max_memory_mb
                .filter(|m| (MIN_MEMORY_MB..=MAX_MEMORY_MB).contains(m)),
            ..self
        })
    }

    /// Create it as a new instance and download its mods. A failed download
    /// removes the half-made instance again.
    pub fn install(&self, dirs: &DataDirs, progress: Progress) -> Result<(Instance, ExactReport)> {
        let version = self
            .game_version
            .as_deref()
            .ok_or_else(|| Error::Other("the share doesn't say which Minecraft version".into()))?;
        let name = instances::free_name(dirs, &self.name)?;
        let created = instances::create(
            dirs,
            &name,
            version,
            arctic_ready(dirs, &self.loader, version),
        )?;
        let instance = Instance {
            icon: self.icon,
            performance: self.performance,
            arctic_mod: self.arctic_mod,
            max_memory_mb: self.max_memory_mb,
            ..created
        };
        instance.save(dirs)?;
        match self.fill(dirs, &instance, progress) {
            Ok(report) => Ok((instance, report)),
            Err(e) => {
                let _ = instances::remove(dirs, &instance.id);
                Err(e)
            }
        }
    }

    /// Mods and client settings into an existing instance's folders.
    pub fn fill(
        &self,
        dirs: &DataDirs,
        instance: &Instance,
        progress: Progress,
    ) -> Result<ExactReport> {
        let game_dir = instance.game_dir(dirs);
        if let Some(client) = &self.client {
            ClientPart::check(Part::All, client)?.apply(&game_dir)?;
        }
        let (Some(kind), Some(version)) = (self.loader.kind(), instance.version.as_deref()) else {
            return Ok(ExactReport::default());
        };
        if self.mods.is_empty() {
            return Ok(ExactReport::default());
        }
        let wanted: Vec<Wanted> = self
            .mods
            .iter()
            .map(|m| Wanted {
                project_id: m.project.clone(),
                version_id: m.version.clone(),
                title: m.title.clone(),
                enabled: m.enabled,
                dependency: m.dependency,
            })
            .collect();
        mods::install_exact(
            &wanted,
            version,
            kind,
            &game_dir.join("mods"),
            &mods::index_path(&dirs.instance_dir(&instance.id)),
            progress,
        )
    }

    pub fn summary(&self) -> String {
        let version = self
            .game_version
            .as_deref()
            .unwrap_or("the Play tab's version");
        let loader = self.loader.kind().map_or("Vanilla", |k| k.label());
        match self.mods.len() {
            0 => format!("\"{}\": {loader} {version}", self.name),
            n => format!(
                "\"{}\": {loader} {version} with {n} mod{}",
                self.name,
                if n == 1 { "" } else { "s" }
            ),
        }
    }
}

/// A Fabric Loader older than the Arctic Client needs gets the one it needs.
fn arctic_ready(dirs: &DataDirs, loader: &Loader, game: &str) -> Loader {
    match loader {
        Loader::Fabric { version } if !crate::arctic_mod::loader_fits(game, version) => {
            crate::arctic_mod::client_loader(dirs, game)
                .filter(|v| crate::arctic_mod::loader_fits(game, v))
                .map_or_else(|| loader.clone(), |version| Loader::Fabric { version })
        }
        _ => loader.clone(),
    }
}

fn bad(what: &str) -> Error {
    Error::Other(format!(
        "this instance share can't be used: {what} looks wrong"
    ))
}

/// Trimmed, without control characters, 1..=max characters.
fn clean_text(s: &str, max: usize) -> Option<String> {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    let s = s.trim();
    (!s.is_empty() && s.chars().count() <= max).then(|| s.to_owned())
}

/// Version strings: letters, digits and `._+-` (plus `extra`).
fn is_version(s: &str, extra: &[u8]) -> bool {
    !s.is_empty()
        && s.len() <= MAX_VERSION
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b) || extra.contains(&b))
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ID && s.bytes().all(|b| b.is_ascii_alphanumeric())
}
