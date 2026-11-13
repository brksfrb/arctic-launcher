//! Background jobs for moving from other launchers: finding what's there
//! (with sizes) and importing the chosen instances and HUD setups.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arctic_core::ProgressInfo;
use arctic_core::instances::Instance;
use arctic_core::migrate::{self, Category, Found, FoundClient, Scan, Sizes};

use crate::tasks::{Event, ProgressSnapshot, Tasks};
use crate::{t, tf};
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
    /// The job to run again, when it failed.
    pub retry: Option<InstanceJob>,
}

impl Tasks {
    /// Look for other launchers (or in `folder`). The list goes out as soon as it's known;
    /// measuring every world and pack tree of every instance can take a while on a big
    /// install, so the sizes follow in a second event.
    pub fn migrate_scan(&self, folder: Option<PathBuf>) {
        self.run(move |t| {
            let scan = match &folder {
                Some(f) => migrate::scan_folder(f),
                None => migrate::scan(),
            };
            let instances = scan.instances.clone();
            t.send(Event::MigrateScanned(
                folder,
                ScanResult {
                    scan,
                    sizes: HashMap::new(),
                },
            ));
            let sizes = instances
                .iter()
                .map(Found::key)
                .zip(migrate::measure_all(&instances))
                .collect();
            t.send(Event::MigrateSized(sizes));
        });
    }

    /// Ask for a folder to look in.
    pub fn migrate_pick_folder(&self) {
        self.run(|t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("tasks.folder_with_instances"))
                .pick_folder();
            t.note_if_picker_missing(&picked);
            if let Some(folder) = picked {
                t.migrate_scan(Some(folder));
            }
        });
    }

    /// Import the chosen items one after another. `cancel` stops between files and items;
    /// what was finished stays, a half-copied instance is removed.
    pub fn migrate_run(
        &self,
        instances: Vec<InstanceJob>,
        huds: Vec<HudJob>,
        cancel: Arc<AtomicBool>,
    ) {
        self.run(move |t| {
            let flag = cancel.clone();
            arctic_core::net::cancellable(cancel, || {
                t.migrate_run_blocking(instances, huds, &flag)
            });
        });
    }

    fn migrate_run_blocking(
        &self,
        instances: Vec<InstanceJob>,
        huds: Vec<HudJob>,
        cancel: &AtomicBool,
    ) {
        let t = self;
        {
            let dirs = t.dirs().clone();
            for job in instances {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
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
                            retry: None,
                        },
                        Err(e) if e.is_cancelled() => Outcome {
                            title: label.clone(),
                            ok: false,
                            lines: vec![t!("tasks.cancelled_nothing_of_it_was_kept").into()],
                            retry: Some(job.clone()),
                        },
                        Err(e) => {
                            // Paths and OS error codes go to the log; the row says it plainly.
                            log::warn!("import of {label}: {}", e.chain());
                            Outcome {
                                title: label.clone(),
                                ok: false,
                                lines: vec![e.plain()],
                                retry: Some(job.clone()),
                            }
                        }
                    };
                t.send(Event::MigrateItemDone(outcome));
            }
            for job in huds {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let label = tf!(
                    "tasks.hud",
                    name = job.client.launcher.name(),
                    profile = job.client.profile
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
                        Err(e) => vec![e.plain()],
                    },
                    retry: None,
                }));
            }
            t.send(Event::MigrateFinished);
        }
    }
}

fn summary(done: &migrate::Imported) -> Vec<String> {
    let mut lines = vec![tf!("tasks.files_copied", files = done.files)];
    if done.mods > 0 {
        let rec = done.recognized.as_ref().map_or(0, |r| r.tracked);
        lines[0].push_str(&tf!(
            "tasks.mods_recognized_on_modrinth",
            mods = done.mods,
            rec = rec
        ));
    }
    if done.kept > 0 {
        lines.push(tf!(
            "tasks.files_were_already_there_yours_were_kept",
            kept = done.kept
        ));
    }
    lines.extend(done.added.iter().map(|a| tf!("tasks.added_missing", a = a)));
    lines.extend(
        done.turned_off
            .iter()
            .map(|m| tf!("tasks.doesnt_run_on_this_version_switched_off", m = m)),
    );
    lines.extend(done.notes.iter().cloned());
    lines.extend(migrate::skipped_note(&done.skipped));
    lines
}
