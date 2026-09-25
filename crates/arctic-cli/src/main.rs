//! `arctic`: headless command-line interface to Arctic Launcher.
//!
//! Exit codes: 0 success, 1 error, 2 usage error (from clap), and with
//! `launch --wait` the game's own exit code.

mod cli;
mod commands;
mod output;

use arctic_core::profiles::ProfileStore;
use arctic_core::storage::DataDirs;
use clap::Parser;
use serde_json::json;

use crate::cli::{Cli, Command};
use crate::commands::Ctx;
use crate::output::Out;

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        log::set_logger(&STDERR_LOGGER).ok();
        log::set_max_level(log::LevelFilter::Info);
    }
    let out = Out::new(cli.json);
    let code = match run(&cli, out) {
        Ok(code) => code,
        Err((out, e)) => {
            out.end_progress();
            if out.json {
                println!("{}", json!({"event": "error", "message": e.to_string()}));
            } else {
                eprintln!("error: {e}");
            }
            1
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli, out: Out) -> Result<i32, (Out, arctic_core::Error)> {
    let dirs = match &cli.data_dir {
        Some(dir) => DataDirs::new(dir),
        None => match DataDirs::resolve() {
            Ok(d) => d,
            Err(e) => return Err((out, e)),
        },
    };
    let scoped = match select_profile(&dirs, cli.profile.as_deref()) {
        Ok(scoped) => scoped,
        Err(e) => return Err((out, e)),
    };
    let ctx = Ctx {
        root: dirs,
        dirs: scoped,
        out,
    };
    let result = match &cli.command {
        Command::Versions(args) => commands::versions::list(&ctx, args),
        Command::Install(args) => commands::versions::install(&ctx, args),
        Command::Launch(args) => commands::launch::run(&ctx, args),
        Command::Accounts(cmd) => commands::accounts::run(&ctx, cmd),
        Command::Profiles(cmd) => commands::profiles::run(&ctx, cmd),
        Command::Java { version } => commands::misc::java_for(&ctx, version),
        Command::Paths => commands::misc::paths(&ctx),
        Command::Update { beta } => commands::misc::update_check(&ctx, *beta),
        Command::Open(args) => commands::misc::open(&ctx, args),
        Command::Instances(cmd) => commands::instances::run(&ctx, cmd),
        Command::Mods(cmd) => commands::mods::run(&ctx, cmd),
    };
    result.map_err(|e| (ctx.out, e))
}

/// Resolve the profile to work in (`--profile`, else the active one).
fn select_profile(root: &DataDirs, query: Option<&str>) -> arctic_core::Result<DataDirs> {
    root.ensure()?;
    let store = ProfileStore::load_or_init(root)?;
    let profile = match query {
        Some(q) => store.find(q).ok_or_else(|| {
            arctic_core::Error::Other(format!(
                "no profile matches '{q}' (see `arctic profiles list`)"
            ))
        })?,
        None => store.active(),
    };
    let scoped = root.with_profile(&profile.id);
    scoped.ensure()?;
    Ok(scoped)
}

/// Minimal logger for `--verbose`.
struct StderrLogger;
static STDERR_LOGGER: StderrLogger = StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("arctic") || metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}
