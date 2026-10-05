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
    /// Modpacks from Modrinth (or a .mrpack file), each installed as a new instance.
    #[command(subcommand)]
    Modpacks(ModpacksCommand),
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
    /// Arctic Client waypoints (per world; the game shows them on its Compass).
    #[command(subcommand)]
    Waypoints(WaypointsCommand),
    /// Saved multiplayer servers: who's online, ping and MOTD. Join one
    /// with `arctic launch --server <address>`.
    #[command(subcommand)]
    Servers(ServersCommand),
    /// Friends: who's online and where, requests, invites, your profile.
    #[command(subcommand)]
    Friends(FriendsCommand),
    /// Default game settings (FOV, render distance, volumes, keys…) that
    /// every new instance starts with.
    #[command(subcommand)]
    Defaults(DefaultsCommand),
    /// Your Minecraft skin and capes (what every server shows).
    #[command(subcommand)]
    Skin(SkinCommand),
    /// Your Arctic look: skin, cape and cosmetics other Arctic players see.
    #[command(subcommand)]
    Look(LookCommand),
    /// Your skin library (the Cosmetics tab's skins) and the community gallery.
    #[command(subcommand)]
    Skins(SkinsCommand),
    /// Screenshots from every instance.
    #[command(subcommand)]
    Screenshots(ScreenshotsCommand),
    /// Saved replays (Arctic Client). Watch one with `arctic launch --replay FILE`.
    #[command(subcommand)]
    Replays(ReplaysCommand),
    /// Show an instance's last game log, or the launcher's.
    Logs(LogsArgs),
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

#[derive(Debug, Subcommand)]
pub enum WaypointsCommand {
    /// Every waypoint, or one world's.
    List {
        /// A server address, or `sp:` + a singleplayer world's name.
        #[arg(long)]
        world: Option<String>,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Add a waypoint.
    Add {
        name: String,
        #[arg(allow_hyphen_values = true)]
        x: i32,
        #[arg(allow_hyphen_values = true)]
        y: i32,
        #[arg(allow_hyphen_values = true)]
        z: i32,
        /// A server address, or `sp:` + a singleplayer world's name.
        #[arg(long)]
        world: String,
        /// overworld (default), the_nether, the_end, …
        #[arg(long)]
        dim: Option<String>,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Remove a waypoint by name.
    Remove {
        name: String,
        #[arg(long)]
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

impl WaypointsCommand {
    pub fn instance(&self) -> Option<&str> {
        match self {
            Self::List { instance, .. }
            | Self::Add { instance, .. }
            | Self::Remove { instance, .. } => instance.as_deref(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum DefaultsCommand {
    Show,
    /// Set one `options.txt` key (`fov 1.0`, `renderDistance 16`,
    /// `soundCategory_music 0.0`, `autoJump false`).
    Set {
        key: String,
        value: String,
    },
    /// Leave a key at the game's own default.
    Unset {
        key: String,
    },
    /// Use an instance's current settings as the defaults.
    Capture {
        #[arg(long)]
        instance: Option<String>,
    },
    /// Put the defaults into an existing instance now.
    Apply {
        #[arg(long)]
        instance: Option<String>,
    },
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum FriendsCommand {
    /// Friends with who's online and where, plus requests and invites.
    List {
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Ask someone to be friends: their friend code, or a Microsoft
    /// account's name.
    Add {
        name: String,
        #[arg(short, long)]
        account: Option<String>,
    },
    Accept {
        name: String,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Unfriend, decline or cancel a request.
    Remove {
        name: String,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Invite a friend to a server or your play-together world.
    Invite {
        name: String,
        #[arg(long, conflicts_with = "together")]
        server: Option<String>,
        #[arg(long)]
        together: Option<String>,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Show the chat with a friend, or send them a message.
    Chat {
        name: String,
        /// Send this instead of showing the chat.
        #[arg(long)]
        send: Option<String>,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Make a recovery code for moving offline accounts to another PC
    /// (shown once).
    Recovery {
        #[arg(short, long)]
        account: Option<String>,
    },
    /// On a new PC: bring an offline account's profile over.
    Restore {
        code: String,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Show or change your profile: name, privacy, linked accounts.
    Profile {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_name = "true|false")]
        share_online: Option<bool>,
        #[arg(long, value_name = "true|false")]
        share_server: Option<bool>,
        #[arg(long, value_name = "true|false")]
        invites: Option<bool>,
        #[arg(long, value_name = "true|false")]
        show_accounts: Option<bool>,
        /// Link another of your saved accounts into this profile.
        #[arg(long, value_name = "ACCOUNT")]
        link: Option<String>,
        /// Take an account out of this profile.
        #[arg(long, value_name = "ACCOUNT")]
        unlink: Option<String>,
        #[arg(short, long)]
        account: Option<String>,
    },
}

impl FriendsCommand {
    pub fn account(&self) -> Option<&str> {
        match self {
            Self::List { account }
            | Self::Add { account, .. }
            | Self::Accept { account, .. }
            | Self::Remove { account, .. }
            | Self::Invite { account, .. }
            | Self::Recovery { account }
            | Self::Chat { account, .. }
            | Self::Restore { account, .. }
            | Self::Profile { account, .. } => account.as_deref(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum ServersCommand {
    /// The instance's server list (as in Minecraft), each pinged.
    List {
        #[arg(long)]
        instance: Option<String>,
        /// Only list them; don't contact the servers.
        #[arg(long)]
        no_ping: bool,
    },
    /// Ping any server address (`host` or `host:port`).
    Ping { address: String },
    /// The public server list: shuffled every time, nothing paid or pinned.
    Browse {
        /// Only servers that need a Microsoft account.
        #[arg(long, conflicts_with = "cracked")]
        premium: bool,
        /// Only servers that let any name in.
        #[arg(long)]
        cracked: bool,
        /// Pick one at random for me.
        #[arg(long)]
        random: bool,
    },
    /// Add a server to an instance's multiplayer list.
    Add {
        address: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Move a server up or down an instance's multiplayer list.
    Move {
        address: String,
        /// Put it in front of this server (default: the end of the list).
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        instance: Option<String>,
    },
    /// List your own server publicly: prints a code to put in its MOTD.
    Submit {
        address: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Up to 5 (`--tag smp --tag pvp`).
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Sign in to Arctic as this account (default: the active one).
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Check that your submission's code is in the MOTD (then it waits
    /// for approval).
    Verify {
        id: String,
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Admin: review submissions (needs ARCTIC_ADMIN_KEY).
    Review {
        /// Which to list: pending, unverified, listed, curated, rejected.
        #[arg(long, default_value = "pending")]
        state: String,
        #[arg(long, value_name = "ID")]
        approve: Option<String>,
        #[arg(long, value_name = "ID")]
        reject: Option<String>,
        #[arg(long, value_name = "ID")]
        remove: Option<String>,
    },
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
    /// Your default game settings for new instances.
    Options,
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
    /// Show an instance's settings.
    Show {
        /// Instance id or name.
        instance: String,
    },
    /// Change an instance: name, version, loader, memory, icon, Java...
    Edit(InstanceEditArgs),
    /// Move an instance to the trash (instances/.trash).
    Remove {
        /// Instance id or name.
        instance: String,
    },
}

#[derive(Debug, Args)]
pub struct InstanceEditArgs {
    /// Instance id or name.
    pub instance: String,
    /// New name.
    #[arg(long)]
    pub name: Option<String>,
    /// Minecraft version, `latest` or `latest-snapshot`.
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long, value_enum)]
    pub loader: Option<LoaderArg>,
    /// Loader version (default: newest stable for the version).
    #[arg(long)]
    pub loader_version: Option<String>,
    /// Maximum memory, like `4G` or `6144M`; `default` uses the launcher setting.
    #[arg(long)]
    pub memory: Option<String>,
    /// Icon shape.
    #[arg(long, value_enum)]
    pub icon: Option<IconArg>,
    /// Icon color as `#rrggbb`, or `default` for the theme's.
    #[arg(long)]
    pub color: Option<String>,
    /// Arctic Client (Fabric and Quilt instances).
    #[arg(long, value_enum)]
    pub arctic_client: Option<Switch>,
    /// Java executable, or `default` for the launcher's choice.
    #[arg(long)]
    pub java: Option<String>,
    /// Extra JVM flags (after the launcher-wide ones); an empty value clears them.
    #[arg(long, allow_hyphen_values = true)]
    pub jvm_args: Option<String>,
    /// Vanilla instances: performance mods (Sodium, Lithium, ...).
    #[arg(long, value_enum)]
    pub performance: Option<Switch>,
    /// Vanilla instances: Iris, so shader packs work.
    #[arg(long, value_enum)]
    pub shaders: Option<Switch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Switch {
    On,
    Off,
}

impl Switch {
    pub fn on(self) -> bool {
        self == Switch::On
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum IconArg {
    Classic,
    Stellar,
    Dendrite,
    Plate,
    Star,
    Crystal,
}

#[derive(Debug, Subcommand)]
pub enum ModpacksCommand {
    /// Search Modrinth for modpacks.
    Search {
        #[arg(default_value = "")]
        query: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Install a modpack as a new instance: a Modrinth slug or project id, or a .mrpack file.
    Install { pack: String },
}

#[derive(Debug, Subcommand)]
pub enum SkinsCommand {
    /// List your skin library.
    List,
    /// Add a skin PNG to the library.
    Add {
        file: std::path::PathBuf,
        /// Name in the library (default: the file's name).
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_enum)]
        model: Option<SkinModel>,
    },
    /// Rename a library skin.
    Rename {
        /// Skin id or name.
        skin: String,
        name: String,
    },
    /// Set a library skin's arm model.
    Model {
        /// Skin id or name.
        skin: String,
        #[arg(value_enum)]
        model: SkinModel,
    },
    /// Remove a skin from the library.
    Remove {
        /// Skin id or name.
        skin: String,
    },
    /// Save a library skin as a PNG file.
    Export {
        /// Skin id or name.
        skin: String,
        file: std::path::PathBuf,
    },
    /// Wear a library skin as your Arctic look (`none`: your Minecraft skin).
    Wear {
        /// Skin id or name, or `none`.
        skin: String,
        #[arg(long)]
        account: Option<String>,
    },
    /// Browse the community gallery.
    Gallery {
        /// Search names and authors.
        #[arg(default_value = "")]
        query: String,
        /// Newest first instead of most used.
        #[arg(long)]
        new: bool,
        /// Page (24 skins each), from 1.
        #[arg(long, default_value_t = 1)]
        page: usize,
    },
    /// Add a gallery skin to your library (`--wear` also puts it on).
    Take {
        /// Gallery id (from `arctic skins gallery`).
        id: String,
        #[arg(long)]
        wear: bool,
        #[arg(long)]
        account: Option<String>,
    },
    /// Share a library skin to the community gallery.
    Share {
        /// Skin id or name.
        skin: String,
        /// Name shown in the gallery (default: the library name).
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        account: Option<String>,
    },
    /// Report a gallery skin (hidden after several reports).
    Report {
        id: String,
        #[arg(long)]
        account: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ScreenshotsCommand {
    /// List screenshots, newest first.
    List {
        /// Only this instance's.
        #[arg(short, long)]
        instance: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },
    /// Move a screenshot to its folder's .trash.
    Remove {
        /// File name (like `2026-09-28_14.35.51.png`) or path.
        file: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ReplaysCommand {
    /// List an instance's replays, newest first.
    List {
        /// Instance id or name (default: Vanilla).
        #[arg(short, long)]
        instance: Option<String>,
    },
    /// Move a replay to the replays folder's .trash.
    Remove {
        /// Replay name (without .mcpr) or path.
        replay: String,
        #[arg(short, long)]
        instance: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    /// Instance whose last game log to show (default: Vanilla).
    #[arg(short, long)]
    pub instance: Option<String>,
    /// The launcher's own log instead.
    #[arg(long, conflicts_with = "instance")]
    pub launcher: bool,
    /// Only the last N lines.
    #[arg(short = 'n', long)]
    pub lines: Option<usize>,
    /// Only warnings and errors (`warn`) or errors (`error`).
    #[arg(long, value_enum)]
    pub level: Option<LogLevelArg>,
    /// Keep printing new lines as they're written.
    #[arg(short, long)]
    pub follow: bool,
    /// Print the log file's path instead.
    #[arg(long)]
    pub path: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LogLevelArg {
    Warn,
    Error,
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

    /// Join this server right away (`host` or `host:port`).
    #[arg(long, value_name = "ADDRESS", conflicts_with = "world")]
    pub server: Option<String>,

    /// Open this world right away (its folder name in `saves`; 1.20+).
    #[arg(long, value_name = "FOLDER")]
    pub world: Option<String>,

    /// Watch this replay (an .mcpr file) once the game is up (Arctic Client).
    #[arg(long, value_name = "FILE", conflicts_with_all = ["server", "world"])]
    pub replay: Option<std::path::PathBuf>,

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
