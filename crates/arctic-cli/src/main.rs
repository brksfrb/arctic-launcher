//! `arctic`: headless command-line interface to Arctic Launcher.
//!
//! Exit codes: 0 success, 1 error, 2 usage error (from clap), and with
//! `launch --wait` the game's own exit code.

mod cli;
mod commands;
mod output;

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
    if let Err(e) = dirs.ensure() {
        return Err((out, e));
    }
    let ctx = Ctx { dirs, out };
    let result = match &cli.command {
        Command::Versions(args) => commands::versions::list(&ctx, args),
        Command::Install(args) => commands::versions::install(&ctx, args),
        Command::Launch(args) => commands::launch::run(&ctx, args),
        Command::Accounts(cmd) => commands::accounts::run(&ctx, cmd),
        Command::Java { version } => commands::misc::java_for(&ctx, version),
        Command::Paths => commands::misc::paths(&ctx),
        Command::Update { beta } => commands::misc::update_check(&ctx, *beta),
        Command::Open(args) => commands::misc::open(&ctx, args),
    };
    result.map_err(|e| (ctx.out, e))
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
