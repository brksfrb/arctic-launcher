//! Command-line definitions.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Arctic Launcher from the command line: list, install and launch vanilla
/// Minecraft, manage accounts, and open the launcher.
#[derive(Debug, Parser)]
#[command(name = "arctic", version, about, long_about = None)]
pub struct Cli {
    /// Use this data directory instead of the default
    /// (%LOCALAPPDATA%\ArcticLauncher).
    #[arg(long, global = true, value_name = "DIR", env = "ARCTIC_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Profile to use (name or id). Defaults to the active profile.
    #[arg(
        short = 'p',
        long,
        global = true,
        value_name = "PROFILE",
        env = "ARCTIC_PROFILE"
    )]
    pub profile: Option<String>,

    /// Machine-readable output: one JSON object per line.
    #[arg(long, global = true)]
    pub json: bool,

    /// Print launcher log messages to stderr.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List Minecraft versions.
    Versions(VersionsArgs),
    /// Download a version (and its Java runtime) without launching it.
    Install(InstallArgs),
    /// Launch a version (downloads what's missing first).
    Launch(LaunchArgs),
    /// Manage accounts.
    #[command(subcommand)]
    Accounts(AccountsCommand),
    /// Manage profiles (separate accounts, settings, instances and worlds).
    #[command(subcommand)]
    Profiles(ProfilesCommand),
    /// Print the Java executable a version would use (installing it if needed).
    Java {
        /// Version id, `latest` or `latest-snapshot`.
        version: String,
    },
    /// Show where Arctic keeps its files.
    Paths,
    /// Check GitHub for a newer launcher release.
    Update {
        /// Include beta (pre-release) builds.
        #[arg(long)]
        beta: bool,
    },
    /// Open the launcher window (optionally straight into a launch).
    Open(OpenArgs),
    /// Manage instances (separate game folders with their own version and mods).
    #[command(subcommand)]
    Instances(InstancesCommand),
    /// Find and manage mods from Modrinth in an instance.
    #[command(subcommand)]
    Mods(ModsCommand),
    /// Play together without a server (peer to peer).
    #[command(subcommand)]
    Together(TogetherCommand),
    /// Send the launcher and games through a SOCKS5 proxy.
    #[command(subcommand)]
    Proxy(ProxyCommand),
    /// Resource packs and shaders from Modrinth.
    #[command(subcommand)]
    Packs(PacksCommand),
    /// An instance's singleplayer worlds: list, import, back up, remove.
    #[command(subcommand)]
    Worlds(WorldsCommand),
    /// Your Minecraft skin and capes (what every server shows).
    #[command(subcommand)]
    Skin(SkinCommand),
    /// Your Arctic look: skin, cape and cosmetics other Arctic players see.
    #[command(subcommand)]
    Look(LookCommand),
    /// Bring instances and HUD setups over from other launchers and clients.
    #[command(subcommand)]
    Migrate(MigrateCommand),
    /// Share an instance, HUD layout, crosshair, client settings or profile.
    Share(ShareArgs),
    /// Use something shared: a code, share text or an exported .json file.
    Import {
        /// `abcd-efgh`, `arctic1.…` text, or a file path.
        source: String,
        /// For HUD, crosshair and client settings: the instance to apply to
        /// (default: Vanilla).
        #[arg(long)]
        instance: Option<String>,
    },
    /// Launcher settings (memory, window, theme, client style...).
    #[command(subcommand)]
    Settings(SettingsCommand),
    /// Explain why Minecraft crashed (from a log or crash report).
    Crash {
        /// A log or crash report file (default: the instance's last game log).
        file: Option<std::path::PathBuf>,
        /// Instance whose last game log to read (default: Vanilla).
        #[arg(long)]
        instance: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum PackKindArg {
    Resource,
    Shader,
}

#[derive(Debug, Subcommand)]
pub enum PacksCommand {
    /// Search Modrinth for packs that fit the instance's Minecraft version.
    Search {
        query: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Install a pack (by slug or id) and switch it on. Shaders bring Iris.
    Install {
        project: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
        /// Install without switching it on.
        #[arg(long)]
        keep_off: bool,
    },
    /// List the instance's packs (on/off).
    List {
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Switch a pack on or off.
    Use {
        file: String,
        #[arg(value_parser = ["on", "off"])]
        state: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Delete a pack.
    Remove {
        file: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorldsCommand {
    /// List worlds, most recently played first.
    List {
        #[arg(long)]
        instance: Option<String>,
    },
    /// Other launchers and instances on this PC that have worlds to import.
    Sources {
        #[arg(long)]
        instance: Option<String>,
    },
    /// Copy in a world folder or a .zip holding one.
    Import {
        path: std::path::PathBuf,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Zip a world into the instance's backups folder.
    Backup {
        /// Folder name or the name shown in game.
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Move a world to saves/.trash (kept, not deleted).
    Remove {
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

impl WorldsCommand {
    pub fn instance(&self) -> Option<&str> {
        match self {
            Self::List { instance }
            | Self::Sources { instance }
            | Self::Import { instance, .. }
            | Self::Backup { instance, .. }
            | Self::Remove { instance, .. } => instance.as_deref(),
        }
    }
}

/// Arm width for a skin (guessed from the image when left out).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum SkinModel {
    Classic,
    Slim,
}

#[derive(Debug, Subcommand)]
pub enum SkinCommand {
    /// Show the skin, the equipped cape and the capes you own.
    Show {
        #[arg(long)]
        account: Option<String>,
    },
    /// Upload a skin PNG.
    Set {
        file: std::path::PathBuf,
        #[arg(long, value_enum)]
        model: Option<SkinModel>,
        #[arg(long)]
        account: Option<String>,
    },
    /// Go back to the default skin.
    Reset {
        #[arg(long)]
        account: Option<String>,
    },
    /// Equip a cape you own (by name or id), or `none`.
    Cape {
        cape: String,
        #[arg(long)]
        account: Option<String>,
    },
}

impl SkinCommand {
    pub fn account(&self) -> Option<&str> {
        match self {
            Self::Show { account }
            | Self::Set { account, .. }
            | Self::Reset { account }
            | Self::Cape { account, .. } => account.as_deref(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum LookCommand {
    /// Show your look plus the capes and cosmetics you can pick.
    Show {
        #[arg(long)]
        account: Option<String>,
    },
    /// Use a skin PNG for Arctic players, or `none` for your Minecraft skin.
    Skin {
        file: std::path::PathBuf,
        #[arg(long, value_enum)]
        model: Option<SkinModel>,
        #[arg(long)]
        account: Option<String>,
    },
    /// Wear a cape: a preset id or name, a cape PNG, or `none`.
    Cape {
        cape: String,
        #[arg(long)]
        account: Option<String>,
    },
    /// Wear exactly these cosmetics (one per slot); no ids takes them all off.
    Wear {
        ids: Vec<String>,
        #[arg(long)]
        account: Option<String>,
    },
}

impl LookCommand {
    pub fn account(&self) -> Option<&str> {
        match self {
            Self::Show { account }
            | Self::Skin { account, .. }
            | Self::Cape { account, .. }
            | Self::Wear { account, .. } => account.as_deref(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    /// Show what can be brought over (with sizes).
    List {
        /// Look in this folder instead (portable MultiMC, moved instances…).
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
    },
    /// Bring one instance over (by key, name or part of the key).
    Instance {
        which: String,
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
        /// Leave out: mods, worlds, resourcepacks, shaderpacks, settings, screenshots.
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
        /// Name for the new instance.
        #[arg(long)]
        name: Option<String>,
    },
    /// Bring every instance over.
    All {
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
    },
    /// Use a client's HUD layout and keys (LabyMod, Lunar).
    Hud {
        which: String,
        /// Instance to apply to (default: Vanilla).
        #[arg(long)]
        instance: Option<String>,
        /// Widgets to turn on where the client didn't save it (Lunar),
        /// like `fps,coords`; default: the ones that were moved.
        #[arg(long, value_delimiter = ',')]
        on: Option<Vec<String>>,
    },
}

#[derive(Debug, clap::Args)]
pub struct ShareArgs {
    /// What to share.
    #[arg(value_enum)]
    pub what: ShareWhat,
    /// Instance to share (or read the HUD/crosshair/settings from).
    #[arg(long)]
    pub instance: Option<String>,
    /// A short code (needs a signed-in account), one line of text, or a file.
    #[arg(long = "as", value_enum, default_value_t = ShareAs::Code)]
    pub r#as: ShareAs,
    /// File to write with `--as file`.
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
    /// Account that creates the code.
    #[arg(long)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShareWhat {
    /// Version, loader and mods (exact Modrinth versions; no files).
    Instance,
    /// HUD layout: which widgets are on, where, and how big.
    Hud,
    Crosshair,
    /// All Arctic Client settings: HUD, crosshair, style, features, keys.
    Client,
    /// Launcher settings, every instance and its client settings.
    Profile,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShareAs {
    Code,
    Text,
    File,
}

#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Show every setting.
    Show,
    /// Print one setting.
    Get { key: String },
    /// Change one setting (restart an open launcher to see it there).
    Set { key: String, value: String },
}

#[derive(Debug, Subcommand)]
pub enum ProxyCommand {
    /// Show the current proxy.
    Show,
    /// Use a SOCKS5 proxy (the proxy resolves server names too).
    Set {
        host: String,
        #[arg(default_value_t = arctic_core::proxy::DEFAULT_PORT)]
        port: u16,
        #[arg(long)]
        username: Option<String>,
        /// Kept in this profile's proxy.json on this PC only.
        #[arg(long, env = "ARCTIC_PROXY_PASSWORD", hide_env_values = true)]
        password: Option<String>,
    },
    /// Stop using the proxy.
    Off,
    /// Check that the proxy works (reaches Mojang's services through it).
    Test,
}

#[derive(Debug, Subcommand)]
pub enum TogetherCommand {
    /// Share the world you open to LAN; prints an invite code.
    Host {
        /// Share this LAN port instead of finding the open world.
        #[arg(long)]
        port: Option<u16>,
    },
    /// Join a friend's world by invite code (it appears in the LAN list).
    Join {
        /// The code your friend shared (arctic-...).
        code: String,
    },
}

/// Mod loader for `instances create`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum LoaderArg {
    Vanilla,
    Fabric,
    Quilt,
    Neoforge,
    Forge,
}

#[derive(Debug, Subcommand)]
pub enum InstancesCommand {
    /// List instances.
    List,
    /// Create an instance.
    Create {
        name: String,
        /// Minecraft version, `latest` or `latest-snapshot`.
        #[arg(long, default_value = "latest")]
        version: String,
        #[arg(long, value_enum, default_value = "vanilla")]
        loader: LoaderArg,
        /// Loader version (default: newest stable).
        #[arg(long)]
        loader_version: Option<String>,
    },
    /// Move an instance to the trash (instances/.trash).
    Remove {
        /// Instance id or name.
        instance: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModsCommand {
    /// Search Modrinth for mods that fit an instance.
    Search {
        query: String,
        /// Instance id or name.
        #[arg(short, long)]
        instance: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Install a mod (and its required dependencies) by slug or project id.
    Install {
        project: String,
        #[arg(short, long)]
        instance: String,
    },
    /// List the mods in an instance.
    List {
        #[arg(short, long)]
        instance: String,
    },
    /// Remove a mod by file name.
    Remove {
        file: String,
        #[arg(short, long)]
        instance: String,
    },
    /// Match the instance's mod files to Modrinth by hash (mods copied in
    /// from elsewhere), so they show names and can be updated and shared.
    Recognize {
        #[arg(short, long)]
        instance: String,
    },
    /// Turn a mod on or off without removing it.
    Toggle {
        file: String,
        #[arg(short, long)]
        instance: String,
        /// `on` or `off`.
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
}

#[derive(Debug, Args)]
pub struct VersionsArgs {
    /// Include snapshots and old alpha/beta versions.
    #[arg(long)]
    pub all: bool,
    /// Only versions that are already downloaded.
    #[arg(long)]
    pub installed: bool,
    /// Show at most this many versions.
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Version id, `latest` or `latest-snapshot`.
    #[arg(default_value = "latest")]
    pub version: String,
}

#[derive(Debug, Args)]
pub struct LaunchArgs {
    /// Version id, `latest` or `latest-snapshot`. Custom instances use
    /// their own version instead.
    #[arg(default_value = "latest")]
    pub version: String,

    /// Launch this instance (id or name) with its version and mods.
    #[arg(short, long)]
    pub instance: Option<String>,

    /// Play as a saved account (username, UUID or id). Defaults to the
    /// active account.
    #[arg(short, long)]
    pub account: Option<String>,

    /// Play offline with this username (not saved unless --save).
    #[cfg(feature = "offline-accounts")]
    #[arg(long, value_name = "USERNAME", conflicts_with = "account")]
    pub offline: Option<String>,

    /// Save the --offline account to the account list.
    #[cfg(feature = "offline-accounts")]
    #[arg(long, requires = "offline")]
    pub save: bool,

    /// Maximum memory, e.g. `4G`, `6144M` or `6144`.
    #[arg(short, long, value_name = "SIZE")]
    pub memory: Option<String>,

    /// Window width.
    #[arg(long)]
    pub width: Option<u32>,

    /// Window height.
    #[arg(long)]
    pub height: Option<u32>,

    /// Start in fullscreen.
    #[arg(long)]
    pub fullscreen: bool,

    /// Use this java executable instead of the managed runtime.
    #[arg(long, value_name = "PATH")]
    pub java: Option<PathBuf>,

    /// Stay attached: print the game's output and exit with its exit code.
    #[arg(short, long)]
    pub wait: bool,

    /// Prepare everything and print the command line, but don't start.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum AccountsCommand {
    /// List saved accounts (the active one is marked).
    List,
    /// Add an offline account and make it active.
    #[cfg(feature = "offline-accounts")]
    AddOffline { username: String },
    /// Sign in with Microsoft (device code by default; works over SSH).
    Login {
        /// Open the system browser instead of showing a code.
        #[arg(long)]
        browser: bool,
    },
    /// Make an account the active one.
    Use {
        /// Username, UUID or id.
        account: String,
    },
    /// Remove an account.
    Remove {
        /// Username, UUID or id.
        account: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfilesCommand {
    /// List profiles (the active one is marked).
    List,
    /// Create a profile.
    Create {
        name: String,
        /// Also make it the active profile.
        #[arg(long)]
        switch: bool,
    },
    /// Make a profile the active one (used by the launcher too).
    Use { profile: String },
    /// Rename a profile.
    Rename { profile: String, new_name: String },
    /// Remove a profile (its folder is moved to profiles/.trash).
    Remove { profile: String },
}

#[derive(Debug, Args)]
pub struct OpenArgs {
    /// Start launching this version as soon as the window opens.
    #[arg(long, value_name = "VERSION")]
    pub launch: Option<String>,
    /// Account to select (username, UUID or id).
    #[arg(long)]
    pub account: Option<String>,
    /// Tab to show: play, accounts, instances, logs, settings, about.
    #[arg(long)]
    pub tab: Option<String>,
    /// Skip the intro animation.
    #[arg(long)]
    pub no_intro: bool,
}
