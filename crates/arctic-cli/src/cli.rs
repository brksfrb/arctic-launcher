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
