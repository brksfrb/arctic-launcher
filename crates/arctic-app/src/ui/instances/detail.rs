//! One instance's page: header, sub-tabs, and the Settings page.

use arctic_core::instances::{self, Instance};
use arctic_core::settings::MIN_MEMORY_MB;
use eframe::egui::{self, CornerRadius, RichText, Stroke};

use super::InstancePage;
use crate::app::{ArcticApp, LaunchState, Tab};
use crate::art::icons::Icon;
use crate::theme::{self, Palette};
use crate::toasts::Kind;
use crate::widgets;

const MAX_MEMORY_MB: u32 = 32 * 1024;

pub(super) fn dialog_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(22.0)
}

impl ArcticApp {
    pub(super) fn instance_page(&mut self, ui: &mut egui::Ui, id: &str) {
        let p = self.palette();
        let Some(instance) = self.instance_by_id(id).cloned() else {
            return;
        };
        if widgets::button(ui, p, Some(Icon::ChevronLeft), "All instances", false).clicked() {
            self.inst.open = None;
            return;
        }
        ui.add_space(8.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let emblem = widgets::instance_emblem(ui, p, &instance.icon, 64.0);
                self.icon_picker(&emblem, id);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&instance.name)
                            .size(22.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(RichText::new(Self::instance_subtitle(&instance)).color(p.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let idle = matches!(self.launch, LaunchState::Idle);
                    let play = ui
                        .add_enabled_ui(idle, |ui| {
                            widgets::button(ui, p, Some(Icon::Play), "Play", true)
                        })
                        .inner;
                    if play.clicked() {
                        self.settings.last_instance =
                            (!instance.is_default()).then(|| id.to_owned());
                        let now = ui.input(|i| i.time);
                        self.set_tab(Tab::Play, now);
                        self.launch_selected();
                    }
                    if widgets::icon_button(ui, p, Icon::Folder, "Open instance folder").clicked() {
                        let dir = instance.game_dir(&self.dirs);
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = open::that_detached(&dir);
                    }
                });
            });
        });
        ui.add_space(12.0);
        let modded = instance.loader.kind().is_some();
        ui.horizontal(|ui| {
            if modded {
                ui.selectable_value(&mut self.inst.page, InstancePage::Mods, "Mods");
                ui.selectable_value(&mut self.inst.page, InstancePage::Browse, "Browse mods");
            }
            ui.selectable_value(&mut self.inst.page, InstancePage::Worlds, "Worlds");
            ui.selectable_value(&mut self.inst.page, InstancePage::Settings, "Settings");
        });
        ui.add_space(10.0);
        match (self.inst.page, modded) {
            (InstancePage::Mods, true) => self.mods_page(ui, &instance),
            (InstancePage::Browse, true) => self.browse_page(ui, &instance),
            (InstancePage::Worlds, _) => self.worlds_page(ui, &instance),
            _ => self.instance_settings(ui, &instance),
        }
    }

    fn instance_settings(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let id = instance.id.clone();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("General").size(17.0).strong().color(p.text));
            ui.add_space(6.0);
            if instance.is_default() {
                ui.label(
                    RichText::new(
                        "The default instance plays whichever release you pick on the Play tab.",
                    )
                    .color(p.muted),
                );
            } else {
                let mut name = self
                    .inst
                    .rename
                    .clone()
                    .unwrap_or_else(|| instance.name.clone());
                widgets::field_row(ui, |ui| {
                    ui.label("Name");
                    ui.add(
                        widgets::text_field(&mut name)
                            .char_limit(48)
                            .desired_width(280.0),
                    );
                    if name.trim() != instance.name
                        && !name.trim().is_empty()
                        && ui.button("Save").clicked()
                    {
                        self.update_instance(&id, |i| i.name = name.trim().to_owned());
                        self.inst.rename = None;
                        return;
                    }
                    self.inst.rename = Some(name);
                });
                ui.label(RichText::new(Self::instance_subtitle(instance)).color(p.muted));
            }
        });
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Memory").size(17.0).strong().color(p.text));
            let mut custom = instance.max_memory_mb.is_some();
            let mut mb = instance
                .max_memory_mb
                .unwrap_or(self.settings.max_memory_mb);
            let mut changed = ui
                .checkbox(&mut custom, "Use a custom amount for this instance")
                .changed();
            if custom {
                changed |= ui
                    .add(
                        egui::Slider::new(&mut mb, MIN_MEMORY_MB..=MAX_MEMORY_MB)
                            .step_by(512.0)
                            .custom_formatter(|v, _| format!("{:.1} GB", v / 1024.0)),
                    )
                    .changed();
            } else {
                ui.label(
                    RichText::new(format!(
                        "Using the launcher setting ({:.1} GB).",
                        self.settings.max_memory_mb as f32 / 1024.0
                    ))
                    .color(p.muted),
                );
            }
            if changed {
                self.update_instance(&id, |i| i.max_memory_mb = custom.then_some(mb));
            }
        });
        self.java_card(ui, instance);
        if instance.is_default() {
            return;
        }
        self.arctic_mod_card(ui, instance);
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Remove instance").size(17.0).strong().color(p.text));
            ui.label(RichText::new("The folder (worlds and mods included) is moved to instances/.trash, not deleted.").color(p.muted));
            ui.add_space(6.0);
            if !self.inst.confirm_delete {
                if widgets::button(ui, p, Some(Icon::Trash), "Remove", false).clicked() {
                    self.inst.confirm_delete = true;
                }
                return;
            }
            ui.horizontal(|ui| {
                if ui.button("Keep it").clicked() {
                    self.inst.confirm_delete = false;
                }
                if widgets::button(ui, p, Some(Icon::Trash), "Yes, remove", true).clicked() {
                    self.delete_instance(&id);
                }
            });
        });
    }

    /// Per-instance Java executable and JVM flags.
    fn java_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let id = instance.id.clone();
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Java").size(17.0).strong().color(p.text));
            let mut custom = instance.java_path.is_some();
            if ui
                .checkbox(&mut custom, "Use a specific Java for this instance")
                .changed()
            {
                let path = custom.then(|| self.settings.java_override.clone().unwrap_or_default());
                self.update_instance(&id, |i| i.java_path = path);
            }
            if let Some(path) = &instance.java_path {
                let mut text = path.display().to_string();
                let response = widgets::field_row(ui, |ui| {
                    ui.label("Path");
                    ui.add(
                        widgets::text_field(&mut text)
                            .hint_text("C:\\Program Files\\Java\\bin\\javaw.exe")
                            .desired_width(420.0),
                    )
                })
                .inner;
                if response.changed() {
                    self.update_instance(&id, |i| i.java_path = Some(text.trim().into()));
                }
                if !path.as_os_str().is_empty() && !path.is_file() {
                    ui.label(
                        RichText::new("That file doesn't exist; the managed Java will be used.")
                            .color(p.warn),
                    );
                }
            } else {
                ui.label(
                    RichText::new("Arctic downloads the right Java for each Minecraft version.")
                        .color(p.muted),
                );
            }
            ui.add_space(6.0);
            let mut args = instance.jvm_args.clone();
            let changed = widgets::field_row(ui, |ui| {
                ui.label("Extra JVM arguments");
                ui.add(
                    widgets::text_field(&mut args)
                        .hint_text("e.g. -XX:+UseZGC")
                        .desired_width(420.0),
                )
                .changed()
            })
            .inner;
            if changed {
                self.update_instance(&id, |i| i.jvm_args = args);
            }
        });
    }

    /// Toggle for Arctic's companion mod (Fabric and Quilt only).
    fn arctic_mod_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        use arctic_core::arctic_mod;
        use arctic_core::loaders::LoaderKind;
        let p = self.palette();
        let fabric_like = matches!(
            instance.loader.kind(),
            Some(LoaderKind::Fabric | LoaderKind::Quilt)
        );
        if !fabric_like {
            return;
        }
        let supported = instance
            .version
            .as_deref()
            .is_some_and(arctic_mod::supports);
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Arctic mod").size(17.0).strong().color(p.text));
            ui.label(
                RichText::new(
                    "Shows everyone's Arctic looks (skins and capes) and adds an Arctic menu in game.",
                )
                .color(p.muted),
            );
            if !supported {
                ui.label(RichText::new("Not available for this Minecraft version yet.").color(p.muted));
                return;
            }
            let mut on = instance.arctic_mod;
            if ui.checkbox(&mut on, "Install the Arctic mod").changed() {
                let id = instance.id.clone();
                self.update_instance(&id, |i| i.arctic_mod = on);
            }
        });
    }

    /// Apply `change` to an instance and save it.
    fn update_instance(&mut self, id: &str, change: impl FnOnce(&mut Instance)) {
        let dirs = self.dirs.clone();
        let Some(instance) = self.instance_mut(id) else {
            return;
        };
        change(instance);
        if let Err(e) = instance.save(&dirs) {
            self.toasts
                .push(Kind::Error, "Could not save instance", e.to_string());
        }
    }

    fn delete_instance(&mut self, id: &str) {
        match instances::remove(&self.dirs, id) {
            Ok(()) => {
                if self.settings.last_instance.as_deref() == Some(id) {
                    self.settings.last_instance = None;
                }
                self.inst.open = None;
                self.inst.confirm_delete = false;
                self.reload_instances();
                self.toasts
                    .push(Kind::Info, "Instance removed", "Moved to instances/.trash");
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not remove instance", e.to_string()),
        }
    }
}
