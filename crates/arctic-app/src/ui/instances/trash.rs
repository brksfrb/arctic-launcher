//! Removed instances: listed from `instances/.trash`, to bring back or delete for good.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use arctic_core::instances::{self, Trashed};
use eframe::egui::{self, Id, Modal, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::toasts::Kind;
use crate::widgets;

/// What the trash dialog holds while it's open.
#[derive(Default)]
pub struct TrashUi {
    pub open: bool,
    pub list: Vec<Trashed>,
    /// Asked "delete for good?" for this one.
    pub confirm: Option<PathBuf>,
}

impl ArcticApp {
    /// How many removed instances there are (the button's count; read when the grid shows).
    pub(super) fn trash_count(&self) -> usize {
        instances::trash_len(&self.dirs)
    }

    pub(super) fn open_trash(&mut self) {
        self.inst.trash = TrashUi {
            open: true,
            list: instances::trashed(&self.dirs),
            confirm: None,
        };
    }

    pub(super) fn trash_dialog(&mut self, ctx: &egui::Context) {
        if !self.inst.trash.open {
            return;
        }
        let p = self.palette();
        let mut restore: Option<PathBuf> = None;
        let mut delete: Option<PathBuf> = None;
        let mut close = false;
        let list = self.inst.trash.list.clone();
        let confirm = self.inst.trash.confirm.clone();
        Modal::new(Id::new("instances_trash"))
            .frame(super::detail::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(
                    RichText::new("Trash")
                        .font(crate::fonts::heading(22.0))
                        .color(p.text),
                );
                ui.label(
                    RichText::new("Removed instances, with their worlds and mods. Bring one back, or delete it for good.")
                        .color(p.muted),
                );
                ui.add_space(12.0);
                if list.is_empty() {
                    ui.label(RichText::new("Nothing here.").color(p.muted));
                }
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for t in &list {
                        widgets::row(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&t.name).color(p.text));
                                ui.label(RichText::new(removed_ago(t.removed)).small().color(p.muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if confirm.as_ref() == Some(&t.dir) {
                                    if widgets::button(ui, p, Some(Icon::Trash), "Yes, delete", true).clicked() {
                                        delete = Some(t.dir.clone());
                                    }
                                    ui.label(RichText::new("Worlds included. Sure?").color(p.error));
                                } else {
                                    if widgets::icon_button(ui, p, Icon::Trash, "Delete for good").clicked() {
                                        self.inst.trash.confirm = Some(t.dir.clone());
                                    }
                                    if widgets::button(ui, p, None, "Restore", false).clicked() {
                                        restore = Some(t.dir.clone());
                                    }
                                }
                            });
                        });
                        ui.separator();
                    }
                });
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if widgets::button(ui, p, None, "Close", false).clicked() {
                        close = true;
                    }
                });
            });
        if let Some(dir) = restore {
            match instances::restore(&self.dirs, &dir) {
                Ok(instance) => {
                    self.reload_instances();
                    self.toasts
                        .push(Kind::Success, "Instance restored", instance.name);
                }
                Err(e) => self
                    .toasts
                    .push(Kind::Error, "Could not restore it", e.to_string()),
            }
            self.inst.trash.list = instances::trashed(&self.dirs);
        }
        if let Some(dir) = delete {
            if let Err(e) = instances::delete_trashed(&self.dirs, &dir) {
                self.toasts
                    .push(Kind::Error, "Could not delete it", e.to_string());
            }
            self.inst.trash.confirm = None;
            self.inst.trash.list = instances::trashed(&self.dirs);
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.inst.trash = TrashUi::default();
        }
    }
}

/// "Removed 3 days ago" (from the Unix seconds in its folder's name).
fn removed_ago(removed: u64) -> String {
    if removed == 0 {
        return "Removed".into();
    }
    let when = SystemTime::UNIX_EPOCH + Duration::from_secs(removed);
    format!("Removed {}", super::worlds::ago(Some(when)))
}
