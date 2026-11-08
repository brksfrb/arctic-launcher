//! "Move to Arctic": pick instances and HUD setups found in other
//! launchers and clients, see what each brings (and its size), import.

use std::collections::{HashMap, HashSet};

use arctic_core::migrate::{Category, Found, FoundClient, Launcher};
use eframe::egui::{self, Id, Modal, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::migrate_tasks::{HudJob, InstanceJob, Outcome, ScanResult};
use crate::tasks::ProgressSnapshot;
use crate::toasts::Kind;
use crate::widgets;

const WIDTH: f32 = 620.0;
const LIST_HEIGHT: f32 = 380.0;
const MB: f64 = 1024.0 * 1024.0;

enum Stage {
    Scanning,
    Choose,
    Running,
    Done,
}

/// A HUD setup's choices.
struct HudChoice {
    on: HashSet<String>,
    target: String,
}

pub struct MigrateUi {
    stage: Stage,
    found: Option<ScanResult>,
    picked: HashSet<String>,
    categories: HashMap<String, HashSet<Category>>,
    huds: HashMap<String, HudChoice>,
    current: Option<(String, ProgressSnapshot)>,
    outcomes: Vec<Outcome>,
    total: usize,
    /// Sizes are still being measured (the list is already up).
    sizing: bool,
    /// Set by Cancel while the copy runs.
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ArcticApp {
    pub(crate) fn open_migrate(&mut self) {
        self.migrate = Some(MigrateUi {
            stage: Stage::Scanning,
            found: None,
            picked: HashSet::new(),
            categories: HashMap::new(),
            huds: HashMap::new(),
            current: None,
            outcomes: Vec::new(),
            total: 0,
            sizing: true,
            cancel: Default::default(),
        });
        self.tasks.migrate_scan(None);
    }

    /// The sizes of the found instances, measured after the list went up.
    pub(crate) fn on_migrate_sized(&mut self, sizes: HashMap<String, arctic_core::migrate::Sizes>) {
        let Some(ui) = &mut self.migrate else {
            return;
        };
        ui.sizing = false;
        let Some(found) = &mut ui.found else {
            return;
        };
        found.sizes.extend(sizes);
        for f in &found.scan.instances {
            if let Some(s) = found.sizes.get(&f.key()) {
                ui.categories
                    .entry(f.key())
                    .or_insert_with(|| default_categories(s));
            }
        }
    }

    pub(crate) fn on_migrate_scanned(&mut self, from_folder: bool, result: ScanResult) {
        let target = self.selected_instance().id.clone();
        let Some(ui) = &mut self.migrate else {
            return;
        };
        let merged = match (ui.found.take(), from_folder) {
            (Some(mut old), true) => {
                old.scan.instances.extend(result.scan.instances);
                old.scan.seen.extend(result.scan.seen);
                old.sizes.extend(result.sizes);
                old
            }
            _ => result,
        };
        // Categories are picked once the sizes are known (on_migrate_sized), so an
        // instance whose sizes are still being measured is not left with nothing chosen.
        for f in &merged.scan.instances {
            if let Some(s) = merged.sizes.get(&f.key()) {
                ui.categories
                    .entry(f.key())
                    .or_insert_with(|| default_categories(s));
            }
        }
        ui.sizing = true;
        for c in &merged.scan.clients {
            ui.huds.entry(c.key()).or_insert_with(|| HudChoice {
                on: c.guesses().into_iter().collect(),
                target: target.clone(),
            });
        }
        // Debug screenshots: `ARCTIC_DEVSHOT_MIGRATE_PICK=meteor,lunar:Default`.
        if crate::devshot::enabled()
            && let Ok(picks) = std::env::var("ARCTIC_DEVSHOT_MIGRATE_PICK")
        {
            let keys = merged
                .scan
                .instances
                .iter()
                .map(Found::key)
                .chain(merged.scan.clients.iter().map(FoundClient::key));
            for key in keys {
                if picks
                    .split(',')
                    .any(|p| key.to_lowercase().contains(&p.to_lowercase()))
                {
                    ui.picked.insert(key);
                }
            }
        }
        ui.found = Some(merged);
        ui.stage = Stage::Choose;
    }

    pub(crate) fn migrate_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut ui_state) = self.migrate.take() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let modal = Modal::new(Id::new("migrate")).frame(super::instances::dialog_frame(p)).show(ctx, |ui| {
            ui.set_width(WIDTH);
            widgets::lift_controls(ui, p);
            ui.label(RichText::new("Move to Arctic").size(22.0).strong().color(p.text));
            ui.label(
                RichText::new("Copies instances, mods, worlds and settings from other launchers. The originals stay as they are; accounts never come along.")
                    .color(p.muted),
            );
            ui.add_space(8.0);
            close = match ui_state.stage {
                Stage::Scanning => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Looking for other launchers…").color(p.muted));
                    });
                    false
                }
                Stage::Choose => self.migrate_choose(ui, &mut ui_state),
                Stage::Running => {
                    if migrate_progress(ui, p, &ui_state) {
                        ui_state
                            .cancel
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    false
                }
                Stage::Done => {
                    let (close, retry) = migrate_done(ui, p, &ui_state);
                    if let Some(i) = retry
                        && i < ui_state.outcomes.len()
                        && let Some(job) = ui_state.outcomes[i].retry.take()
                    {
                        ui_state.outcomes.remove(i);
                        ui_state.total = ui_state.outcomes.len() + 1;
                        ui_state.current = None;
                        ui_state.stage = Stage::Running;
                        ui_state.cancel = Default::default();
                        self.tasks
                            .migrate_run(vec![job], Vec::new(), ui_state.cancel.clone());
                    }
                    close
                }
            };
        });
        // The scan can be walked away from (its result is dropped); the copy has Cancel.
        let busy = matches!(ui_state.stage, Stage::Running);
        if !(close || (modal.should_close() && !busy)) {
            self.migrate = Some(ui_state);
        }
    }

    /// Returns true to close.
    fn migrate_choose(&mut self, ui: &mut egui::Ui, state: &mut MigrateUi) -> bool {
        let p = self.palette();
        let Some(found) = state.found.clone() else {
            return false;
        };
        let mut targets = vec![(self.instance.id.clone(), self.instance.name.clone())];
        targets.extend(
            self.custom_instances
                .iter()
                .map(|i| (i.id.clone(), i.name.clone())),
        );
        egui::ScrollArea::vertical().max_height(LIST_HEIGHT).show(ui, |ui| {
            let mut launchers: Vec<Launcher> = Vec::new();
            for f in &found.scan.instances {
                if !launchers.contains(&f.launcher) {
                    launchers.push(f.launcher);
                }
            }
            for launcher in launchers {
                ui.add_space(6.0);
                ui.label(RichText::new(launcher.name()).strong().color(p.text));
                for f in found.scan.instances.iter().filter(|f| f.launcher == launcher) {
                    instance_row(ui, p, f, found.sizes.get(&f.key()), state);
                }
            }
            if !found.scan.clients.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("HUD layouts and keys").strong().color(p.text));
                for c in &found.scan.clients {
                    hud_row(ui, p, c, state, &targets);
                }
            }
            if !found.scan.seen.is_empty() {
                ui.add_space(10.0);
                for (launcher, why) in &found.scan.seen {
                    ui.label(RichText::new(format!("{}: {why}", launcher.name())).small().color(p.muted));
                }
            }
            if found.scan.instances.is_empty() && found.scan.clients.is_empty() {
                ui.label(RichText::new("Nothing found. If your launcher keeps instances somewhere else, choose that folder.").color(p.muted));
            }
        });
        ui.add_space(10.0);
        let bytes: u64 = found
            .scan
            .instances
            .iter()
            .filter(|f| state.picked.contains(&f.key()))
            .map(|f| chosen_bytes(f, found.sizes.get(&f.key()), state))
            .sum();
        let count = state.picked.len();
        let mut close = false;
        crate::widgets::row(ui, |ui| {
            if widgets::button(ui, p, Some(Icon::Folder), "Choose folder…", false)
                .on_hover_text(
                    "A launcher's instances folder (portable MultiMC, moved CurseForge folder…)",
                )
                .clicked()
            {
                self.tasks.migrate_pick_folder();
            }
            ui.label(
                RichText::new(format!("{:.1} GB to copy", bytes as f64 / MB / 1024.0))
                    .color(p.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let go = ui
                    .add_enabled_ui(count > 0, |ui| {
                        widgets::button(
                            ui,
                            p,
                            Some(Icon::Import),
                            &format!("Bring over {count}"),
                            true,
                        )
                    })
                    .inner;
                if go.clicked() {
                    self.start_migrate(state, &found);
                }
                if widgets::button(ui, p, None, "Cancel", false).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    fn start_migrate(&mut self, state: &mut MigrateUi, found: &ScanResult) {
        let instances: Vec<InstanceJob> = found
            .scan
            .instances
            .iter()
            .filter(|f| state.picked.contains(&f.key()))
            .map(|f| InstanceJob {
                found: f.clone(),
                categories: state
                    .categories
                    .get(&f.key())
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default(),
            })
            .collect();
        let huds: Vec<HudJob> = found
            .scan
            .clients
            .iter()
            .filter(|c| state.picked.contains(&c.key()))
            .filter_map(|c| {
                let choice = state.huds.get(&c.key())?;
                Some(HudJob {
                    client: c.clone(),
                    on: choice.on.iter().cloned().collect(),
                    target: self.instance_by_id(&choice.target)?.clone(),
                })
            })
            .collect();
        state.total = instances.len() + huds.len();
        state.outcomes.clear();
        state.stage = Stage::Running;
        state.cancel = Default::default();
        self.tasks
            .migrate_run(instances, huds, state.cancel.clone());
    }

    pub(crate) fn on_migrate_progress(&mut self, label: String, progress: ProgressSnapshot) {
        if let Some(ui) = &mut self.migrate {
            ui.current = Some((label, progress));
        }
    }

    pub(crate) fn on_migrate_item(&mut self, outcome: Outcome) {
        if let Some(ui) = &mut self.migrate {
            ui.outcomes.push(outcome);
        }
    }

    pub(crate) fn on_migrate_finished(&mut self) {
        self.reload_instances();
        if let Ok(vanilla) = arctic_core::instances::load_default(&self.dirs) {
            self.instance = vanilla;
        }
        if let Some(ui) = &mut self.migrate {
            ui.stage = Stage::Done;
            ui.current = None;
            let failed = ui.outcomes.iter().filter(|o| !o.ok).count();
            let (kind, title) = if failed == 0 {
                (Kind::Success, "Everything came over".to_owned())
            } else {
                (Kind::Error, format!("{failed} couldn't come over"))
            };
            self.toasts
                .push(kind, title, "See the details in the window.");
        }
    }
}

fn instance_row(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    f: &Found,
    sizes: Option<&arctic_core::migrate::Sizes>,
    state: &mut MigrateUi,
) {
    let key = f.key();
    let mut on = state.picked.contains(&key);
    let loader = f.loader.as_ref().map_or("Vanilla".to_owned(), |l| {
        format!(
            "{} {}",
            l.kind.label(),
            l.version.as_deref().unwrap_or("(newest)")
        )
    });
    let what = if f.into_vanilla {
        "into your Vanilla instance".to_owned()
    } else {
        format!("{} · {loader}", f.game_version)
    };
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut on, RichText::new(&f.name).color(p.text))
            .changed()
        {
            if on {
                state.picked.insert(key.clone());
            } else {
                state.picked.remove(&key);
            }
        }
        ui.label(RichText::new(what).small().color(p.muted));
    });
    if !on {
        return;
    }
    ui.indent(key.clone(), |ui| {
        if sizes.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Measuring…").small().color(p.muted));
            });
            return;
        }
        let chosen = state.categories.entry(key.clone()).or_default();
        ui.horizontal_wrapped(|ui| {
            for c in Category::ALL {
                let n = sizes.and_then(|s| s.counts.get(&c)).copied().unwrap_or(0);
                if n == 0 {
                    continue;
                }
                let mb = sizes.and_then(|s| s.bytes.get(&c)).copied().unwrap_or(0) as f64 / MB;
                let mut take = chosen.contains(&c);
                if ui
                    .checkbox(&mut take, format!("{} ({n}, {mb:.0} MB)", c.label()))
                    .changed()
                {
                    if take {
                        chosen.insert(c);
                    } else {
                        chosen.remove(&c);
                    }
                }
            }
        });
        if let Some(s) = sizes {
            if !s.turned_off.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{} mods don't run on {} and come switched off",
                        s.turned_off.len(),
                        f.game_version
                    ))
                    .small()
                    .color(p.muted),
                );
            }
            if !s.other_loader.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{} mods in this shared folder are for another loader and stay behind",
                        s.other_loader.len()
                    ))
                    .small()
                    .color(p.muted),
                );
            }
        }
        for n in &f.notes {
            ui.label(RichText::new(n).small().color(p.muted));
        }
    });
}

fn hud_row(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    c: &FoundClient,
    state: &mut MigrateUi,
    targets: &[(String, String)],
) {
    let key = c.key();
    let mut on = state.picked.contains(&key);
    ui.horizontal(|ui| {
        let label = format!("{} · {}", c.launcher.name(), c.profile);
        if ui
            .checkbox(&mut on, RichText::new(label).color(p.text))
            .changed()
        {
            if on {
                state.picked.insert(key.clone());
            } else {
                state.picked.remove(&key);
            }
        }
        let widgets_on = c.settings.widgets_on().unwrap_or(0);
        ui.label(
            RichText::new(format!("{widgets_on} widgets on"))
                .small()
                .color(p.muted),
        );
    });
    if !on {
        return;
    }
    let Some(choice) = state.huds.get_mut(&key) else {
        return;
    };
    ui.indent(key.clone(), |ui| {
        let current = targets
            .iter()
            .find(|(id, _)| *id == choice.target)
            .map_or("Vanilla", |(_, n)| n.as_str());
        ui.horizontal(|ui| {
            ui.label("Apply to");
            widgets::modal_combo(("hud_target", &key))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (id, name) in targets {
                        ui.selectable_value(&mut choice.target, id.clone(), name);
                    }
                });
        });
        if !c.unsure.is_empty() {
            ui.label(
                RichText::new(format!(
                    "{} doesn't save whether these are on; tick the ones you use:",
                    c.launcher.name()
                ))
                .small()
                .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for u in &c.unsure {
                    let mut ticked = choice.on.contains(&u.widget);
                    if ui.checkbox(&mut ticked, &u.label).changed() {
                        if ticked {
                            choice.on.insert(u.widget.clone());
                        } else {
                            choice.on.remove(&u.widget);
                        }
                    }
                }
            });
        }
        for n in &c.notes {
            ui.label(RichText::new(n).small().color(p.muted));
        }
    });
}

fn chosen_bytes(f: &Found, sizes: Option<&arctic_core::migrate::Sizes>, state: &MigrateUi) -> u64 {
    let (Some(s), Some(chosen)) = (sizes, state.categories.get(&f.key())) else {
        return 0;
    };
    chosen
        .iter()
        .map(|c| s.bytes.get(c).copied().unwrap_or(0))
        .sum()
}

/// Categories worth taking by default: the ones with something in them, except screenshots
/// (often large; opt in).
fn default_categories(s: &arctic_core::migrate::Sizes) -> HashSet<Category> {
    Category::ALL
        .into_iter()
        .filter(|c| *c != Category::Screenshots && s.counts.get(c).copied().unwrap_or(0) > 0)
        .collect()
}

/// Returns true when Cancel was clicked.
fn migrate_progress(ui: &mut egui::Ui, p: &crate::theme::Palette, state: &MigrateUi) -> bool {
    let cancelling = state.cancel.load(std::sync::atomic::Ordering::Relaxed);
    ui.label(
        RichText::new(format!("{} of {} done", state.outcomes.len(), state.total)).color(p.text),
    );
    if let Some((label, s)) = &state.current {
        ui.label(RichText::new(label).small().color(p.muted));
        ui.horizontal(|ui| {
            ui.spinner();
            if s.bytes_total > 0 {
                let f = s.bytes_done as f32 / s.bytes_total as f32;
                ui.add(
                    egui::ProgressBar::new(f)
                        .desired_width(460.0)
                        .text(s.stage.clone()),
                );
            } else {
                ui.label(RichText::new(&s.stage).color(p.muted));
            }
        });
    }
    ui.add_space(8.0);
    let mut cancel = false;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if cancelling {
            ui.label(RichText::new("Stopping after this file…").color(p.muted));
        } else if widgets::button(ui, p, None, "Cancel", false)
            .on_hover_text("What was already copied stays; a half-copied instance is removed.")
            .clicked()
        {
            cancel = true;
        }
    });
    cancel
}

/// Returns (close, the failed outcome to try again).
fn migrate_done(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    state: &MigrateUi,
) -> (bool, Option<usize>) {
    let mut retry = None;
    egui::ScrollArea::vertical()
        .max_height(LIST_HEIGHT)
        .show(ui, |ui| {
            for (i, o) in state.outcomes.iter().enumerate() {
                let color = if o.ok { p.text } else { p.error };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&o.title).strong().color(color));
                    if o.retry.is_some()
                        && widgets::button(ui, p, None, "Try again", false).clicked()
                    {
                        retry = Some(i);
                    }
                });
                for l in &o.lines {
                    ui.label(RichText::new(l).small().color(p.muted));
                }
                ui.add_space(6.0);
            }
        });
    ui.add_space(8.0);
    let mut close = false;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if widgets::button(ui, p, None, "Done", true).clicked() {
            close = true;
        }
    });
    (close, retry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_categories_take_what_is_there_but_not_screenshots() {
        let mut s = arctic_core::migrate::Sizes::default();
        s.counts.insert(Category::Mods, 12);
        s.counts.insert(Category::Worlds, 0);
        s.counts.insert(Category::Screenshots, 300);
        let chosen = default_categories(&s);
        assert!(chosen.contains(&Category::Mods));
        assert!(!chosen.contains(&Category::Worlds), "nothing there");
        assert!(!chosen.contains(&Category::Screenshots), "opt in");
    }
}
