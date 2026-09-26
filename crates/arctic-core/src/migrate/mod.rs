//! Move to Arctic from other launchers and clients: find their instances
//! (version, loader, mods, configs, worlds, packs, game options, memory)
//! and client HUD setups, then copy them in. Originals are never touched,
//! and account data never comes along.
//!
//! Formats were checked against real installs (official launcher,
//! TLauncher, ATLauncher, CurseForge, LabyMod, Lunar) and against the
//! open-source launchers' own code (Prism/MultiMC/PolyMC, Modrinth App,
//! GDLauncher). Anything that can't be carried over exactly is reported.

mod copy;
mod deps;
mod detect;
mod hud;
mod ranges;
mod sources;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use crate::loaders::LoaderKind;
use crate::sharing::ClientPart;

pub use copy::{Category, Imported, Sizes, import, measure, measure_all};
pub use hud::Unsure;
pub use sources::{scan, scan_folder};

/// Where something was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Launcher {
    Official,
    TLauncher,
    Prism,
    PolyMc,
    MultiMc,
    ModrinthApp,
    CurseForge,
    AtLauncher,
    GdLauncher,
    GdLauncherLegacy,
    LabyMod,
    Lunar,
    Feather,
    Badlion,
    /// A folder the player picked.
    Folder,
}

impl Launcher {
    pub fn name(self) -> &'static str {
        match self {
            Launcher::Official => "Minecraft Launcher",
            Launcher::TLauncher => "TLauncher",
            Launcher::Prism => "Prism Launcher",
            Launcher::PolyMc => "PolyMC",
            Launcher::MultiMc => "MultiMC",
            Launcher::ModrinthApp => "Modrinth App",
            Launcher::CurseForge => "CurseForge",
            Launcher::AtLauncher => "ATLauncher",
            Launcher::GdLauncher => "GDLauncher",
            Launcher::GdLauncherLegacy => "GDLauncher (old)",
            Launcher::LabyMod => "LabyMod",
            Launcher::Lunar => "Lunar Client",
            Launcher::Feather => "Feather Client",
            Launcher::Badlion => "Badlion Client",
            Launcher::Folder => "Folder",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Launcher::Official => "official",
            Launcher::TLauncher => "tlauncher",
            Launcher::Prism => "prism",
            Launcher::PolyMc => "polymc",
            Launcher::MultiMc => "multimc",
            Launcher::ModrinthApp => "modrinth",
            Launcher::CurseForge => "curseforge",
            Launcher::AtLauncher => "atlauncher",
            Launcher::GdLauncher => "gdlauncher",
            Launcher::GdLauncherLegacy => "gdlauncher-legacy",
            Launcher::LabyMod => "labymod",
            Launcher::Lunar => "lunar",
            Launcher::Feather => "feather",
            Launcher::Badlion => "badlion",
            Launcher::Folder => "folder",
        }
    }
}

/// A loader as another launcher names it; `latest` is resolved on import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundLoader {
    pub kind: LoaderKind,
    /// Exact version, or `None` for "the newest" (LabyMod, Lunar, Feather).
    pub version: Option<String>,
}

/// One instance (or profile) that can become an Arctic instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub launcher: Launcher,
    pub name: String,
    pub game_version: String,
    pub loader: Option<FoundLoader>,
    /// Where its worlds, configs, options and packs are.
    pub game_dir: PathBuf,
    /// Its mods, when they're not `game_dir/mods` (LabyMod, Lunar, Feather).
    pub mods_dir: Option<PathBuf>,
    /// Folder shared with other profiles (`.minecraft`): its mods may be
    /// for other versions, so only ones that fit are taken.
    pub shared: bool,
    pub memory_mb: Option<u32>,
    /// Jars to add as mods (OptiFine from a Forge+OptiFine version).
    pub extra_jars: Vec<PathBuf>,
    /// Mods kept switched off in a folder of their own (ATLauncher).
    pub disabled_mods_dir: Option<PathBuf>,
    /// More folders to bring along: (source, folder in the game dir).
    pub extra_dirs: Vec<(PathBuf, &'static str)>,
    /// Plain vanilla that follows the newest release: goes into Arctic's
    /// own Vanilla instance instead of a new one.
    pub into_vanilla: bool,
    /// What won't come over exactly, said plainly.
    pub notes: Vec<String>,
}

impl Found {
    /// Stable key for choosing it (CLI and UI).
    pub fn key(&self) -> String {
        format!("{}:{}", self.launcher.id(), self.name)
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.mods_dir
            .clone()
            .unwrap_or_else(|| self.game_dir.join("mods"))
    }
}

/// A client's HUD and feature settings, converted.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundClient {
    pub launcher: Launcher,
    /// Its profile ("Default", "Arena PvP"…).
    pub profile: String,
    pub settings: ClientPart,
    /// Widgets the client doesn't record as on or off (Lunar keeps only
    /// changes from its defaults); the player decides.
    pub unsure: Vec<Unsure>,
    pub notes: Vec<String>,
}

impl FoundClient {
    pub fn key(&self) -> String {
        format!("{}:{}", self.launcher.id(), self.profile)
    }
}

/// Everything found on this PC.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub instances: Vec<Found>,
    pub clients: Vec<FoundClient>,
    /// Launchers seen, including ones with nothing to import (and why).
    pub seen: Vec<(Launcher, String)>,
}
