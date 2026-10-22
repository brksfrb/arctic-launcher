//! Minecraft: Bedrock Edition on Windows: find the installed game (Release
//! and Preview), start it, switch it to another version, and look after its
//! worlds, packs and server list.
//!
//! Since 1.21.120 Bedrock is a GDK game: the package lives in
//! `C:\XboxGames\…`, and its data in `%APPDATA%\Minecraft Bedrock\Users\`:
//! a `Shared` folder for packs and one folder per Xbox account for worlds,
//! settings and the server list. Older (UWP) installs kept everything in the
//! package's `LocalState`, which is still read when it's there.
//!
//! Bedrock is only on Windows here: elsewhere nothing is installed and the
//! folders are empty.

mod archive;
pub mod client;
mod fe3;
#[cfg(windows)]
mod package;
pub mod packs;
pub mod servers;
pub mod versions;
pub mod worlds;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The two Bedrock games on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    #[default]
    Release,
    Preview,
}

impl Edition {
    pub const ALL: [Edition; 2] = [Edition::Release, Edition::Preview];

    pub fn name(self) -> &'static str {
        match self {
            Edition::Release => "Bedrock",
            Edition::Preview => "Bedrock Preview",
        }
    }

    /// The Windows package family.
    pub fn package_family(self) -> &'static str {
        match self {
            Edition::Release => "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
            Edition::Preview => "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe",
        }
    }

    /// The Microsoft Store page (where the game is bought or installed).
    pub fn store_url(self) -> &'static str {
        match self {
            Edition::Release => "ms-windows-store://pdp/?productid=9NBLGGH2JHXJ",
            Edition::Preview => "ms-windows-store://pdp/?productid=9P5X4QVLC2XR",
        }
    }

    /// The link that starts the game (or brings it forward).
    pub fn uri(self) -> &'static str {
        match self {
            Edition::Release => "minecraft:",
            Edition::Preview => "minecraft-preview:",
        }
    }

    /// The data folder's name in `%APPDATA%`.
    fn data_folder(self) -> &'static str {
        match self {
            Edition::Release => "Minecraft Bedrock",
            Edition::Preview => "Minecraft Bedrock Preview",
        }
    }

    /// "release" / "preview" (also "bedrock", "beta").
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "release" | "bedrock" | "stable" => Some(Edition::Release),
            "preview" | "beta" | "bedrock-preview" => Some(Edition::Preview),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Edition::Release => "release",
            Edition::Preview => "preview",
        }
    }
}

/// An installed Bedrock game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub edition: Edition,
    /// The game's version ("1.26.52.3").
    pub version: String,
    /// Where the package is installed.
    pub path: PathBuf,
    /// A GDK install (1.21.120 and newer); `false` for an old UWP one.
    pub gdk: bool,
}

/// Whether Bedrock can be played from this launcher on this computer.
pub fn supported() -> bool {
    cfg!(windows)
}

/// The installed Bedrock games (Release first).
pub fn installed() -> Vec<Install> {
    Edition::ALL.into_iter().filter_map(find).collect()
}

/// The installed game of one edition, if any.
pub fn find(edition: Edition) -> Option<Install> {
    #[cfg(windows)]
    {
        let (package_version, path) = package::find(edition.package_family())?;
        Some(Install {
            edition,
            version: versions::from_package_version(&package_version),
            // Only the GDK game has this file; the older app type has an AppxManifest.xml instead.
            gdk: path.join("MicrosoftGame.Config").is_file(),
            path,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = edition;
        None
    }
}

/// Start the game (or bring it forward when it's already running).
pub fn launch(edition: Edition) -> Result<()> {
    if !supported() {
        return Err(Error::Other(format!(
            "Minecraft {} can only be played on Windows here",
            edition.name()
        )));
    }
    #[cfg(windows)]
    {
        if find(edition).is_none() {
            return Err(Error::Other(format!(
                "{} isn't installed. Get it from the Microsoft Store first.",
                edition.name()
            )));
        }
        match package::launch(edition.package_family()) {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!(
                "starting {} directly failed ({e}), using its link",
                edition.name()
            ),
        }
        open::that_detached(edition.uri())
            .map_err(|e| Error::Other(format!("couldn't start {}: {e}", edition.name())))
    }
    #[cfg(not(windows))]
    unreachable!()
}

/// Start the game, and when `client` is chosen load it into the game once it is up (Windows). The
/// loading runs on its own thread: this returns as soon as the game is started, and `report` hears what
/// happened (a client that can't be downloaded or loaded never stops the game from starting). The
/// handle is for callers that must wait for it (the command line).
pub fn launch_with_client(
    dirs: &crate::storage::DataDirs,
    edition: Edition,
    client: Option<client::Client>,
    report: impl Fn(String) + Send + 'static,
) -> Result<Option<std::thread::JoinHandle<()>>> {
    launch(edition)?;
    let Some(client) = client else {
        return Ok(None);
    };
    let dirs = dirs.clone();
    Ok(Some(std::thread::spawn(move || {
        match attach(&dirs, edition, client, &report) {
            Ok(()) => report(format!("{} is loaded", client.name())),
            Err(e) => {
                log::warn!("{}: {e}", client.name());
                report(format!("{} couldn't be loaded: {e}", client.name()));
            }
        }
    })))
}

/// Wait for the game's window, then load the client. The game needs a few seconds after its process
/// appears before its graphics are up, as every client launcher also waits.
fn attach(
    dirs: &crate::storage::DataDirs,
    edition: Edition,
    client: client::Client,
    report: &impl Fn(String),
) -> Result<()> {
    report(format!("Getting {}", client.name()));
    let dll = client::prepare(dirs, client)?;
    #[cfg(windows)]
    {
        let install =
            find(edition).ok_or_else(|| Error::Other("the game isn't installed".into()))?;
        if !install.gdk {
            return Err(Error::Other(
                "clients load into the newer GDK game (1.21.120 and later) only".into(),
            ));
        }
        let mut pid = None;
        for _ in 0..120 {
            pid = package::process_id(&install.path, edition.package_family());
            if pid.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let pid = pid.ok_or_else(|| Error::Other("the game didn't start".into()))?;
        // Its window shows up when its graphics do; loading a client while the game is still reading its
        // resources makes the game report errors about them.
        for _ in 0..120 {
            if package::has_window(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        std::thread::sleep(std::time::Duration::from_secs(CLIENT_DELAY_SECS));
        client::inject(pid, &dll)
    }
    #[cfg(not(windows))]
    {
        let _ = (edition, dll);
        Err(Error::Other("clients can only be loaded on Windows".into()))
    }
}

/// Seconds between the game's window showing and the client being loaded into it (the game keeps loading
/// its resources for a while after the window is up).
#[cfg(windows)]
const CLIENT_DELAY_SECS: u64 = 12;

/// Is the game running now?
pub fn running(edition: Edition) -> bool {
    #[cfg(windows)]
    {
        find(edition)
            .is_some_and(|install| package::running(&install.path, edition.package_family()))
    }
    #[cfg(not(windows))]
    {
        let _ = edition;
        false
    }
}

/// Gaming Services' Store page (GDK games can't be installed without it).
pub const GAMING_SERVICES_URL: &str = "ms-windows-store://pdp/?productid=9MWPM2CQNLHN";

/// Is Gaming Services installed? It installs and unlocks GDK games, so
/// switching versions needs it.
pub fn gaming_services() -> bool {
    #[cfg(windows)]
    {
        package::find("Microsoft.GamingServices_8wekyb3d8bbwe").is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Open the Microsoft Store on the game's page.
pub fn open_store(edition: Edition) -> Result<()> {
    open::that_detached(edition.store_url())
        .map_err(|e| Error::Other(format!("couldn't open the Microsoft Store: {e}")))
}

/// One `com.mojang` folder: a player's (worlds, settings, server list) or
/// the shared one (packs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// The folder's name under `Users` ("Shared", an account's number), or
    /// "UWP" for an old install's data.
    pub id: String,
    pub com_mojang: PathBuf,
}

impl Player {
    /// A short name for lists.
    pub fn label(&self) -> String {
        match self.id.as_str() {
            "Shared" => "Shared".into(),
            "UWP" => "Before 1.21.120 (UWP)".into(),
            id => {
                let tail: String = id
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                format!("Account …{tail}")
            }
        }
    }

    pub fn worlds_dir(&self) -> PathBuf {
        self.com_mojang.join("minecraftWorlds")
    }

    /// Where removed worlds and packs go (kept, on the same drive).
    pub fn trash_dir(&self) -> PathBuf {
        self.com_mojang.join("arctic-trash")
    }
}

/// Where a Bedrock edition keeps its data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folders {
    /// `%APPDATA%\Minecraft Bedrock` (or Preview), when it exists.
    pub root: Option<PathBuf>,
    /// Per-account folders, most recently played first (then the old UWP data).
    pub players: Vec<Player>,
    /// Where packs are installed.
    pub packs: Option<Player>,
}

impl Folders {
    /// The account folder with this id, or the first one.
    pub fn player(&self, id: Option<&str>) -> Option<&Player> {
        id.and_then(|id| self.players.iter().find(|p| p.id == id))
            .or(self.players.first())
    }
}

/// The data folders of an edition on this computer.
pub fn folders(edition: Edition) -> Folders {
    if !supported() {
        return Folders::default();
    }
    let root = dirs::config_dir().map(|d| d.join(edition.data_folder()));
    let legacy = (edition == Edition::Release)
        .then(dirs::data_local_dir)
        .flatten()
        .map(|d| {
            d.join("Packages")
                .join(edition.package_family())
                .join("LocalState/games/com.mojang")
        });
    folders_in(root.as_deref(), legacy.as_deref())
}

/// [`folders`] for a given data root and old UWP `com.mojang` folder.
pub fn folders_in(root: Option<&Path>, legacy: Option<&Path>) -> Folders {
    let mut players = Vec::new();
    let mut packs = None;
    let root = root.filter(|r| r.is_dir());
    if let Some(users) = root.map(|r| r.join("Users"))
        && let Ok(entries) = std::fs::read_dir(&users)
    {
        let mut found: Vec<(Option<std::time::SystemTime>, Player)> = Vec::new();
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            let com_mojang = entry.path().join("games").join("com.mojang");
            if !com_mojang.is_dir() {
                continue;
            }
            let player = Player { id, com_mojang };
            if player.id.eq_ignore_ascii_case("Shared") {
                packs = Some(player);
            } else {
                let played = std::fs::metadata(player.worlds_dir())
                    .and_then(|m| m.modified())
                    .ok();
                found.push((played, player));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        players.extend(found.into_iter().map(|(_, p)| p));
    }
    if let Some(legacy) = legacy.filter(|l| l.is_dir()) {
        let player = Player {
            id: "UWP".into(),
            com_mojang: legacy.to_path_buf(),
        };
        if packs.is_none() {
            packs = Some(player.clone());
        }
        players.push(player);
    }
    // A Preview (or a fresh install) may keep everything in the root.
    if players.is_empty()
        && let Some(root) = root
    {
        let com_mojang = root.join("games").join("com.mojang");
        if com_mojang.is_dir() {
            let player = Player {
                id: "Shared".into(),
                com_mojang,
            };
            packs.get_or_insert_with(|| player.clone());
            players.push(player);
        }
    }
    Folders {
        root: root.map(Path::to_path_buf),
        players,
        packs,
    }
}

#[cfg(test)]
mod tests;
