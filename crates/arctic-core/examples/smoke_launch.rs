//! End-to-end smoke test of the vanilla pipeline with an offline account.
//!
//! ```text
//! cargo run -p arctic-core --example smoke_launch -- <version|latest> [username] [--spawn]
//! ```
//! Set `ARCTIC_DATA_DIR` to keep test downloads out of your real data folder.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use arctic_core::ProgressInfo;
use arctic_core::auth::offline;
use arctic_core::instances;
use arctic_core::launch::{self, GameEvent, LaunchRequest};
use arctic_core::settings::Settings;
use arctic_core::storage::DataDirs;
use arctic_core::versions::VersionManifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wanted = args.first().map(String::as_str).unwrap_or("latest");
    let username = args
        .get(1)
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .unwrap_or("ArcticTester");
    let spawn = args.iter().any(|a| a == "--spawn");

    let dirs = DataDirs::resolve()?;
    dirs.ensure()?;
    println!("data dir: {}", dirs.root().display());

    let manifest = VersionManifest::fetch(&dirs)?;
    let id = if wanted == "latest" {
        manifest.latest.release.clone()
    } else {
        wanted.to_owned()
    };
    let entry = manifest.find(&id).ok_or(format!("unknown version {id}"))?;
    let instance = instances::load_default(&dirs)?;
    let account = offline::create(username)?;
    let settings = Settings::default();

    let last_pct = AtomicU64::new(u64::MAX);
    let started = Instant::now();
    let progress = |p: ProgressInfo| {
        let pct = (p.bytes_done * 100)
            .checked_div(p.bytes_total)
            .unwrap_or(100);
        if pct.is_multiple_of(10) && last_pct.swap(pct, Ordering::Relaxed) != pct {
            let secs = started.elapsed().as_secs_f64().max(0.001);
            println!(
                "  {:>5.1}s {}: {}/{} files, {:.1}/{:.1} MB ({:.1} MB/s)",
                secs,
                p.stage,
                p.done,
                p.total,
                mb(p.bytes_done),
                mb(p.bytes_total),
                mb(p.bytes_done) / secs
            );
        }
    };
    let plan = launch::prepare(
        &LaunchRequest {
            dirs: &dirs,
            version: entry,
            instance: &instance,
            account: &account,
            settings: &settings,
        },
        &progress,
    )?;
    println!("prepared {id} in {:.1?}", started.elapsed());
    println!("{}", plan.redacted_command());

    if spawn {
        let (tx, rx) = mpsc::channel();
        let spawned_at = Instant::now();
        let game = launch::spawn(&plan, move |e| {
            let _ = tx.send(e);
        })?;
        println!(
            "spawned pid {} — log: {}",
            game.pid(),
            plan.log_file.display()
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(GameEvent::WindowReady) => {
                    println!("window ready after {:.1?}", spawned_at.elapsed());
                    break;
                }
                Ok(GameEvent::Exited { code }) => {
                    println!("game exited early with {code:?}");
                    return Ok(());
                }
                Ok(GameEvent::Output(_)) => {}
                Err(_) => {
                    println!("no window marker within 60s");
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(5));
        game.kill();
        while let Ok(event) = rx.recv_timeout(Duration::from_secs(10)) {
            if let GameEvent::Exited { code } = event {
                println!("stopped (exit {code:?})");
                break;
            }
        }
    }
    Ok(())
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}
