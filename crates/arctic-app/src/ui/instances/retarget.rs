//! Moving an instance to another Minecraft version: pick it, see which mods have a version there
//! (and which get turned off), then switch.

use arctic_core::instances::{Instance, Loader};
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

/// The open instance's version change in progress.
pub struct RetargetUi {
    pub instance: String,
    /// The version picked in the list.
    pub picked: Option<String>,
    /// What the mods become on `.0` (asked for with "Check mods").
    pub check: Option<(String, Load<Vec<ModCheck>>)>,
    pub applying: bool,
    pub show_all: bool,
}

impl ArcticApp {
    /// The "Minecraft version" card of a custom instance.
    pub(super) fn version_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        if instance.is_default() {
            return;
        }
        let p = self.palette();
        let kind = instance.loader.kind();
        if let Some(kind) = kind
            && !self.inst.loader_games.contains_key(&kind)
        {
            self.inst.loader_games.insert(kind, Load::Loading);
            self.tasks.loader_games(kind);
        }
        let current = instance.version.clone().unwrap_or_default();
        let state = match self.inst.retarget.take() {
            Some(s) if s.instance == instance.id => s,
            _ => RetargetUi {
                instance: instance.id.clone(),
                picked: None,
                check: None,
                applying: false,
                show_all: false,
            },
        };
        let mut state = state;
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Minecraft version").size(17.0).strong().color(p.text));
            ui.label(
                RichText::new(if kind.is_some() {
                    "Move this instance to another version. Its mods are looked up on Modrinth first, so you see which ones exist there."
                } else {
                    "Move this instance to another version. Worlds made in a newer version can't be opened in an older one."
                })
                .color(p.muted),
            );
            ui.add_space(6.0);
            let Some(games) = self.create_game_versions(kind) else {
                ui.label(RichText::new("Loading versions…").color(p.muted));
                return;
            };
            let picked = state.picked.clone().unwrap_or_else(|| current.clone());
            let mut choice = picked.clone();
            widgets::row(ui, |ui| {
                ui.label("Version");
                egui::ComboBox::from_id_salt(("retarget", &instance.id))
                    .width(200.0)
                    .height(320.0)
                    .selected_text(&choice)
                    .show_ui(ui, |ui| {
                        for g in &games {
                            ui.selectable_value(&mut choice, g.clone(), g);
                        }
                    });
                let busy = state.applying
                    || matches!(state.check, Some((_, Load::Loading)));
                if choice != current && !busy {
                    let label = if kind.is_some() { "Check mods" } else { "Change version" };
                    if widgets::button(ui, p, None, label, true).clicked() {
                        match kind {
                            Some(_) => {
                                state.check = Some((choice.clone(), Load::Loading));
                                self.tasks.retarget_check(instance.clone(), choice.clone());
                            }
                            None => {
                                let id = instance.id.clone();
                                let game = choice.clone();
                                self.update_instance(&id, |i| i.version = Some(game.clone()));
                                self.toasts.push(Kind::Success, format!("Now on {game}"), "");
                            }
                        }
                    }
                }
            });
            if choice != picked {
                state.check = None;
            }
            state.picked = Some(choice.clone());
            self.retarget_preview(ui, &mut state, instance, &choice);
        });
        self.inst.retarget = Some(state);
    }

    /// The looked-up mods and the button that switches.
    fn retarget_preview(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut RetargetUi,
        instance: &Instance,
        version: &str,
    ) {
        let p = self.palette();
        let Some((checked, load)) = &state.check else {
            return;
        };
        if checked != version {
            return;
        }
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
                "{ready} of {known} mods have a version for {version}."
            ))
            .strong()
            .color(if missing == 0 { p.text } else { p.warn }),
        );
        if missing > 0 {
            ui.label(
                RichText::new(format!(
                    "{missing} will be turned off (not deleted): they don't have a version for {version} yet."
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
            if widgets::button(ui, p, None, &format!("Change to {version}"), true).clicked() {
                state.applying = true;
                self.tasks
                    .retarget_apply(instance.clone(), version.to_owned(), checks.clone());
            }
            if widgets::button(ui, p, None, "Cancel", false).clicked() {
                state.check = None;
                state.picked = None;
            }
        });
    }

    /// A check finished.
    pub(super) fn on_retarget_checked(
        &mut self,
        instance: String,
        version: String,
        result: Result<Vec<ModCheck>, String>,
    ) {
        if let Some(state) = self.inst.retarget.as_mut()
            && state.instance == instance
        {
            state.check = Some((
                version,
                match result {
                    Ok(checks) => Load::Ready(checks),
                    Err(e) => Load::Failed(e),
                },
            ));
        }
    }

    /// The switch finished.
    pub(super) fn on_retargeted(
        &mut self,
        instance: String,
        version: String,
        result: Result<Retargeted, String>,
    ) {
        self.inst.install_progress = None;
        if let Some(state) = self.inst.retarget.as_mut()
            && state.instance == instance
        {
            state.applying = false;
            state.check = None;
            state.picked = None;
        }
        match result {
            Ok(done) => {
                let kind = self.instance_by_id(&instance).and_then(|i| i.loader.kind());
                let game = version.clone();
                self.update_instance(&instance, |i| {
                    i.version = Some(game);
                    i.loader = Loader::new(kind, done.loader_version.clone());
                });
                let mut detail = format!("{} mods updated", done.updated);
                if !done.disabled.is_empty() {
                    detail.push_str(&format!(", turned off: {}", done.disabled.join(", ")));
                }
                if !done.failed.is_empty() {
                    detail.push_str(&format!(", couldn't download: {}", done.failed.join(", ")));
                }
                self.toasts
                    .push(Kind::Success, format!("Now on {version}"), detail);
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not change the version", e),
        }
        if self.inst.open.as_deref() == Some(instance.as_str()) {
            self.refresh_mods(&instance);
        }
    }
}
