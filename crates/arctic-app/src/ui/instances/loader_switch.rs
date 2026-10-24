//! Changing an instance's mod loader: another build of the same loader (nothing else changes), or
//! another loader (Fabric, Quilt, NeoForge, Forge) on the same Minecraft version, with the mods looked
//! up on Modrinth first so you see which ones exist there.

use std::collections::hash_map::Entry;

use arctic_core::instances::{Instance, Loader};
use arctic_core::loaders::LoaderKind;
use arctic_core::mods::retarget::ModCheck;
use eframe::egui::{self, RichText};

use super::Load;
use crate::app::ArcticApp;
use crate::tasks::Retargeted;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

/// Mods listed before the rest are folded away.
const SHOWN_ROWS: usize = 12;

/// The open instance's loader change in progress.
pub struct LoaderUi {
    pub instance: String,
    /// The loader picked to move to.
    pub picked: Option<LoaderKind>,
    /// What the mods become under `.0` (asked for with "Check mods").
    pub check: Option<(LoaderKind, Load<Vec<ModCheck>>)>,
    pub applying: bool,
    pub show_all: bool,
}

impl ArcticApp {
    /// The "Mod loader" card of a custom instance that has a loader.
    pub(super) fn loader_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let (Some(kind), Some(game)) = (instance.loader.kind(), instance.version.clone()) else {
            return;
        };
        if instance.is_default() {
            return;
        }
        // Which loaders have a build for this version, and the builds of the current one.
        for k in LoaderKind::ALL {
            if let Entry::Vacant(slot) = self.inst.loader_games.entry(k) {
                slot.insert(Load::Loading);
                self.tasks.loader_games(k);
            }
        }
        if let Entry::Vacant(slot) = self.inst.loader_versions.entry((kind, game.clone())) {
            slot.insert(Load::Loading);
            self.tasks.loader_versions(kind, game.clone());
        }
        let p = self.palette();
        let mut state = match self.inst.loader_switch.take() {
            Some(s) if s.instance == instance.id => s,
            _ => LoaderUi {
                instance: instance.id.clone(),
                picked: None,
                check: None,
                applying: false,
                show_all: false,
            },
        };
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Mod loader")
                    .size(17.0)
                    .strong()
                    .color(p.text),
            );
            ui.label(
                RichText::new(format!(
                    "{} {} on Minecraft {game}.",
                    kind.label(),
                    instance.loader.version().unwrap_or_default()
                ))
                .color(p.muted),
            );
            ui.add_space(6.0);
            self.loader_build_row(ui, instance, kind, &game);
            ui.add_space(8.0);
            self.loader_kind_row(ui, &mut state, instance, kind, &game);
            self.loader_preview(ui, &mut state, instance, &game);
        });
        self.inst.loader_switch = Some(state);
    }

    /// Another build of the same loader: only the instance's loader version changes.
    fn loader_build_row(
        &mut self,
        ui: &mut egui::Ui,
        instance: &Instance,
        kind: LoaderKind,
        game: &str,
    ) {
        let p = self.palette();
        let current = instance.loader.version().unwrap_or_default().to_owned();
        let builds = match self.inst.loader_versions.get(&(kind, game.to_owned())) {
            Some(Load::Ready(list)) if !list.is_empty() => list.clone(),
            Some(Load::Failed(e)) => {
                ui.label(RichText::new(format!("Couldn't list the builds: {e}")).color(p.error));
                return;
            }
            Some(Load::Ready(_)) => return,
            _ => {
                ui.label(RichText::new("Loading builds…").color(p.muted));
                return;
            }
        };
        let id = ("loader_build", &instance.id);
        let mut choice = ui
            .data(|d| d.get_temp::<String>(egui::Id::new(id)))
            .unwrap_or_else(|| current.clone());
        widgets::row(ui, |ui| {
            ui.label(format!("{} build", kind.label()));
            egui::ComboBox::from_id_salt(id)
                .width(220.0)
                .height(320.0)
                .selected_text(&choice)
                .show_ui(ui, |ui| {
                    for v in &builds {
                        let label = if v.stable {
                            format!("{}  (stable)", v.id)
                        } else {
                            v.id.clone()
                        };
                        ui.selectable_value(&mut choice, v.id.clone(), label);
                    }
                });
            if choice != current && widgets::button(ui, p, None, "Use this build", true).clicked() {
                let version = choice.clone();
                self.update_instance(&instance.id, |i| {
                    i.loader = Loader::new(Some(kind), version);
                });
                self.toasts.push(
                    Kind::Success,
                    format!("Now on {} {choice}", kind.label()),
                    "",
                );
            }
        });
        ui.data_mut(|d| d.insert_temp(egui::Id::new(id), choice));
    }

    /// Another loader: pick it, check the mods, switch.
    fn loader_kind_row(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut LoaderUi,
        instance: &Instance,
        kind: LoaderKind,
        game: &str,
    ) {
        let p = self.palette();
        let others: Vec<LoaderKind> = LoaderKind::ALL
            .into_iter()
            .filter(|k| *k != kind)
            .filter(|k| {
                matches!(
                    self.inst.loader_games.get(k),
                    Some(Load::Ready(games)) if games.contains(game)
                )
            })
            .collect();
        if others.is_empty() {
            let waiting = LoaderKind::ALL
                .into_iter()
                .any(|k| matches!(self.inst.loader_games.get(&k), Some(Load::Loading)));
            let text = if waiting {
                "Looking for other loaders…".to_owned()
            } else {
                format!("No other loader has a build for Minecraft {game}.")
            };
            ui.label(RichText::new(text).color(p.muted));
            return;
        }
        ui.label(
            RichText::new(
                "Switch to another loader. Mods are made for one loader, so each is looked up again; the ones without a version there are turned off, not deleted.",
            )
            .color(p.muted),
        );
        ui.add_space(4.0);
        let busy = state.applying || matches!(state.check, Some((_, Load::Loading)));
        widgets::row(ui, |ui| {
            for k in &others {
                let on = state.picked == Some(*k);
                if ui.selectable_label(on, k.label()).clicked() && !busy {
                    state.picked = if on { None } else { Some(*k) };
                    state.check = None;
                }
            }
            if let Some(k) = state.picked
                && !busy
                && state.check.is_none()
                && widgets::button(ui, p, None, "Check mods", true).clicked()
            {
                state.check = Some((k, Load::Loading));
                self.tasks.loader_check(instance.clone(), k);
            }
        });
    }

    /// The looked-up mods and the button that switches.
    fn loader_preview(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut LoaderUi,
        instance: &Instance,
        game: &str,
    ) {
        let p = self.palette();
        let Some((kind, load)) = &state.check else {
            return;
        };
        if state.picked != Some(*kind) {
            return;
        }
        let kind = *kind;
        ui.add_space(8.0);
        let checks = match load {
            Load::Loading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Looking up the mods on Modrinth…").color(p.muted));
                });
                return;
            }
            Load::Failed(e) => {
                ui.label(RichText::new(format!("Couldn't check the mods: {e}")).color(p.error));
                return;
            }
            Load::Ready(checks) => checks.clone(),
        };
        let known = checks.iter().filter(|c| c.project_id.is_some()).count();
        let ready = checks.iter().filter(|c| c.available.is_some()).count();
        let missing = checks.iter().filter(|c| c.missing()).count();
        ui.label(
            RichText::new(format!(
                "{ready} of {known} mods have a {} version for {game}.",
                kind.label()
            ))
            .strong()
            .color(if missing == 0 { p.text } else { p.warn }),
        );
        if missing > 0 {
            ui.label(
                RichText::new(format!(
                    "{missing} will be turned off (not deleted): they don't have a {} version yet.",
                    kind.label()
                ))
                .color(p.muted),
            );
        }
        ui.add_space(4.0);
        let shown = if state.show_all {
            checks.len()
        } else {
            SHOWN_ROWS.min(checks.len())
        };
        for c in &checks[..shown] {
            ui.horizontal_wrapped(|ui| {
                let name = if c.dependency {
                    format!("{} (needed by another mod)", c.title)
                } else {
                    c.title.clone()
                };
                ui.label(RichText::new(name).color(p.text));
                let (text, color) = match (&c.project_id, &c.available) {
                    (None, _) => ("not on Modrinth, kept as it is".to_owned(), p.muted),
                    (Some(_), Some(v)) => (format!("→ {v}"), p.accent),
                    (Some(_), None) => ("no version yet, turned off".to_owned(), p.warn),
                };
                ui.label(RichText::new(text).small().color(color));
            });
        }
        if checks.len() > shown && ui.link(format!("Show all {} mods", checks.len())).clicked() {
            state.show_all = true;
        }
        ui.add_space(8.0);
        if state.applying {
            ui.horizontal(|ui| {
                ui.spinner();
                let stage = self
                    .inst
                    .install_progress
                    .as_ref()
                    .map(|s| s.stage.clone())
                    .unwrap_or_else(|| "Switching".to_owned());
                ui.label(RichText::new(stage).color(p.muted));
            });
            return;
        }
        widgets::row(ui, |ui| {
            if widgets::button(ui, p, None, &format!("Switch to {}", kind.label()), true).clicked()
            {
                state.applying = true;
                self.tasks
                    .loader_switch(instance.clone(), kind, checks.clone());
            }
            if widgets::button(ui, p, None, "Cancel", false).clicked() {
                state.check = None;
                state.picked = None;
            }
        });
    }

    /// A check finished.
    pub(super) fn on_loader_checked(
        &mut self,
        instance: String,
        loader: LoaderKind,
        result: Result<Vec<ModCheck>, String>,
    ) {
        if let Some(state) = self.inst.loader_switch.as_mut()
            && state.instance == instance
        {
            state.check = Some((
                loader,
                match result {
                    Ok(checks) => Load::Ready(checks),
                    Err(e) => Load::Failed(e),
                },
            ));
        }
    }

    /// The switch finished.
    pub(super) fn on_loader_switched(
        &mut self,
        instance: String,
        loader: LoaderKind,
        result: Result<Retargeted, String>,
    ) {
        self.inst.install_progress = None;
        if let Some(state) = self.inst.loader_switch.as_mut()
            && state.instance == instance
        {
            state.applying = false;
            state.check = None;
            state.picked = None;
        }
        match result {
            Ok(done) => {
                let build = done.loader_version.clone();
                self.update_instance(&instance, |i| {
                    i.loader = Loader::new(Some(loader), build);
                });
                let mut detail = format!("{} mods updated", done.updated);
                if !done.disabled.is_empty() {
                    detail.push_str(&format!(", turned off: {}", done.disabled.join(", ")));
                }
                if !done.failed.is_empty() {
                    detail.push_str(&format!(", couldn't download: {}", done.failed.join(", ")));
                }
                self.toasts
                    .push(Kind::Success, format!("Now on {}", loader.label()), detail);
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not change the loader", e),
        }
        if self.inst.open.as_deref() == Some(instance.as_str()) {
            self.refresh_mods(&instance);
        }
    }
}
