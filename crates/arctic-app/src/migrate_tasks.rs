//! Background jobs for moving from other launchers: finding what's there
//! (with sizes) and importing the chosen instances and HUD setups.

use std::collections::HashMap;
use std::path::PathBuf;

use arctic_core::ProgressInfo;
use arctic_core::instances::Instance;
use arctic_core::migrate::{self, Category, Found, FoundClient, Scan, Sizes};

use crate::tasks::{Event, ProgressSnapshot, Tasks};

const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(150);

/// What was found, with each instance's sizes (by key).
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub scan: Scan,
    pub sizes: HashMap<String, Sizes>,
}

/// One chosen instance.
#[derive(Debug, Clone)]
pub struct InstanceJob {
    pub found: Found,
    pub categories: Vec<Category>,
}

/// One chosen HUD setup.
#[derive(Debug, Clone)]
pub struct HudJob {
    pub client: FoundClient,
    pub on: Vec<String>,
    pub target: Instance,
}

/// How one item went.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub title: String,
    pub ok: bool,
    pub lines: Vec<String>,
}

impl Tasks {
    /// Look for other launchers (or in `folder`).
    pub fn migrate_scan(&self, folder: Option<PathBuf>) {
        self.run(move |t| {
            let scan = match &folder {
                Some(f) => migrate::scan_folder(f),
                None => migrate::scan(),
            };
            let sizes = scan
                .instances
                .iter()
                .map(Found::key)
                .zip(migrate::measure_all(&scan.instances))
                .collect();
            t.send(Event::MigrateScanned(folder, ScanResult { scan, sizes }));
        });
    }

    /// Ask for a folder to look in.
    pub fn migrate_pick_folder(&self) {
        self.run(|t| {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("Folder with instances")
                .pick_folder()
            {
                t.migrate_scan(Some(folder));
            }
        });
    }

    /// Import the chosen items one after another.
    pub fn migrate_run(&self, instances: Vec<InstanceJob>, huds: Vec<HudJob>) {
        self.run(move |t| {
            let dirs = t.dirs().clone();
            for job in instances {
                let label = format!("{} · {}", job.found.launcher.name(), job.found.name);
                // Tens of thousands of files: a few updates a second is plenty.
                let last = std::sync::Mutex::new(None::<std::time::Instant>);
                let progress = |p: ProgressInfo| {
                    let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                    let due =
                        last.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) || p.done == p.total;
                    if due {
                        *last = Some(std::time::Instant::now());
                        t.send(Event::MigrateProgress(
                            label.clone(),
                            ProgressSnapshot::from(p),
                        ));
                    }
                };
                let outcome =
                    match migrate::import(&dirs, &job.found, &job.categories, None, &progress) {
                        Ok(done) => Outcome {
                            title: format!("{label} → {}", done.instance.name),
                            ok: true,
                            lines: summary(&done),
                        },
                        Err(e) => Outcome {
                            title: label.clone(),
                            ok: false,
                            lines: vec![e.to_string()],
                        },
                    };
                t.send(Event::MigrateItemDone(outcome));
            }
            for job in huds {
                let label = format!(
                    "{} HUD ({})",
                    job.client.launcher.name(),
                    job.client.profile
                );
                let result = job
                    .client
                    .with_choices(&job.on)
                    .apply(&job.target.game_dir(&dirs));
                t.send(Event::MigrateItemDone(Outcome {
                    title: format!("{label} → {}", job.target.name),
                    ok: result.is_ok(),
                    lines: match result {
                        Ok(()) => job.client.notes.clone(),
                        Err(e) => vec![e.to_string()],
                    },
                }));
            }
            t.send(Event::MigrateFinished);
        });
    }
}

fn summary(done: &migrate::Imported) -> Vec<String> {
    let mut lines = vec![format!("{} files copied", done.files)];
    if done.mods > 0 {
        let rec = done.recognized.as_ref().map_or(0, |r| r.tracked);
        lines[0].push_str(&format!(
            ", {} mods ({rec} recognized on Modrinth)",
            done.mods
        ));
    }
    if done.kept > 0 {
        lines.push(format!(
            "{} files were already there; yours were kept",
            done.kept
        ));
    }
    lines.extend(done.added.iter().map(|a| format!("Added missing {a}")));
    lines.extend(
        done.turned_off
            .iter()
            .map(|m| format!("{m} doesn't run on this version: switched off")),
    );
    lines.extend(done.notes.iter().cloned());
    lines
}
