//! Mods that won't start together: checked when Play is pressed (and shown on the Mods page),
//! with one click to fix each.

use arctic_core::instances::Instance;
use arctic_core::launch::QuickPlay;
use arctic_core::migrate::{Clash, Fix};
use arctic_core::mods;
use eframe::egui::{self, Id, Modal, RichText};

use super::super::dialogs::dialog_frame;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

const DIALOG_WIDTH: f32 = 520.0;
const LIST_HEIGHT: f32 = 400.0;

/// A Play that clashes stopped, to start once they're fixed (or anyway).
pub struct PendingStart {
    instance_id: String,
    another_copy: bool,
    quick_play: Option<QuickPlay>,
}

#[derive(Default)]
pub struct ClashState {
    /// Last check: (instance id, mods folder fingerprint, what was found).
    checked: Option<(String, u64, Vec<Clash>)>,
    pub pending: Option<PendingStart>,
}

impl ArcticApp {
    /// What clashes in this instance's mods folder (checked again only when the folder changed).
    pub(crate) fn mod_clashes(&mut self, instance: &Instance) -> Vec<Clash> {
        let Some(loader) = instance.loader.kind() else {
            return Vec::new();
        };
        let Some(game) = instance.version.as_deref() else {
            return Vec::new();
        };
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        let stamp = super::mods_page::folder_stamp(&mods_dir);
        if let Some((id, s, found)) = &self.inst.clashes.checked
            && *id == instance.id
            && *s == stamp
        {
            return found.clone();
        }
        let found = arctic_core::migrate::clashes(&mods_dir, loader, game);
        if !found.is_empty() {
            log::info!("{} mod clashes in {}", found.len(), instance.id);
        }
        self.inst.clashes.checked = Some((instance.id.clone(), stamp, found.clone()));
        found
    }

    /// Hold a start back when its mods clash (the dialog then offers fixes); true if held.
    pub(crate) fn held_by_clashes(
        &mut self,
        instance: &Instance,
        another_copy: bool,
        quick_play: &Option<QuickPlay>,
    ) -> bool {
        if self.mod_clashes(instance).is_empty() {
            return false;
        }
        self.inst.clashes.pending = Some(PendingStart {
            instance_id: instance.id.clone(),
            another_copy,
            quick_play: quick_play.clone(),
        });
        true
    }

    pub(crate) fn mod_clash_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.inst.clashes.pending else {
            return;
        };
        let Some(instance) = self.instance_by_id(&pending.instance_id).cloned() else {
            self.inst.clashes.pending = None;
            return;
        };
        let clashes = self.mod_clashes(&instance);
        // Fixes change the folder; look again soon.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let p = self.palette();
        let mut fix = None;
        let (mut play, mut cancel) = (false, false);
        let modal = Modal::new(Id::new("mod_clashes"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(DIALOG_WIDTH);
                let title = if clashes.is_empty() {
                    "All fixed"
                } else {
                    "These mods won't start together"
                };
                ui.label(
                    RichText::new(title)
                        .font(crate::fonts::heading(22.0))
                        .color(p.text),
                );
                ui.add_space(6.0);
                let about = if clashes.is_empty() {
                    format!("{} is ready to play.", instance.name)
                } else {
                    format!(
                        "The game would stop while loading {}. Fix them here, or play anyway.",
                        instance.name
                    )
                };
                ui.label(RichText::new(about).color(p.muted));
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .max_height(LIST_HEIGHT)
                    .auto_shrink([false, true])
                    .show(ui, |ui| fix = self.clash_rows(ui, p, &clashes));
                ui.add_space(12.0);
                widgets::row(ui, |ui| {
                    if clashes.is_empty() {
                        play = widgets::button(ui, p, Some(Icon::Play), "Play", true).clicked();
                    } else {
                        play = widgets::button(ui, p, None, "Play anyway", false).clicked();
                    }
                    cancel = widgets::button(ui, p, None, "Cancel", false).clicked();
                });
            });
        if let Some(fix) = fix {
            self.apply_clash_fix(&instance, &fix);
        }
        if play {
            let Some(pending) = self.inst.clashes.pending.take() else {
                return;
            };
            self.launch_unchecked(pending.another_copy, pending.quick_play);
        } else if cancel || modal.should_close() {
            self.inst.clashes.pending = None;
        }
    }

    /// The Mods page's warning: the same list, above the mods.
    pub(super) fn clash_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let clashes = self.mod_clashes(instance);
        if clashes.is_empty() {
            return;
        }
        let p = self.palette();
        let mut fix = None;
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let title = match clashes.len() {
                1 => "1 problem will stop the game".to_owned(),
                n => format!("{n} problems will stop the game"),
            };
            ui.label(RichText::new(title).strong().color(p.warn));
            ui.add_space(6.0);
            fix = self.clash_rows(ui, p, &clashes);
        });
        ui.add_space(8.0);
        if let Some(fix) = fix {
            self.apply_clash_fix(instance, &fix);
        }
    }

    /// Each problem and its fix buttons; returns the fix clicked.
    fn clash_rows(&self, ui: &mut egui::Ui, p: &Palette, clashes: &[Clash]) -> Option<Fix> {
        let mut clicked = None;
        for clash in clashes {
            // Inset, so the buttons (the dialog's own color) stand out.
            egui::Frame::new()
                .fill(p.bg)
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&clash.problem).color(p.text));
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for fix in &clash.fixes {
                            let busy = matches!(fix, Fix::Get { project, .. } if self.inst.installing.contains(project));
                            let label = if busy { "Getting it…".to_owned() } else { fix.label() };
                            let primary = matches!(fix, Fix::Get { .. });
                            if ui.add_enabled_ui(!busy, |ui| widgets::button(ui, p, None, &label, primary)).inner.clicked() {
                                clicked = Some(fix.clone());
                            }
                        }
                    });
                });
            ui.add_space(6.0);
        }
        clicked
    }

    fn apply_clash_fix(&mut self, instance: &Instance, fix: &Fix) {
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        match fix {
            Fix::TurnOff { file, .. } => {
                if let Err(e) = mods::set_enabled(&mods_dir, file, false) {
                    self.toasts
                        .push(Kind::Error, "Could not turn the mod off", e.to_string());
                }
            }
            Fix::Get {
                project, replacing, ..
            } => {
                self.inst.installing.insert(project.clone());
                self.tasks
                    .mod_install(instance.clone(), project.clone(), replacing.clone());
            }
        }
        if self.inst.open.as_deref() == Some(instance.id.as_str()) {
            self.refresh_mods(&instance.id);
        }
    }
}
