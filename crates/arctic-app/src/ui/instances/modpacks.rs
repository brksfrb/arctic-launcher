//! The discovery page's frame (back button, title, `.mrpack` import) and the modpack rows.
//! Searching and filtering live in `hub.rs`.

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, ProjectType};
use eframe::egui::{self, RichText};

use super::Load;
use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};

impl ArcticApp {
    pub(super) fn modpacks_page(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if widgets::button(
            ui,
            p,
            Some(Icon::ChevronLeft),
            t!("instances.all_instances"),
            false,
        )
        .clicked()
        {
            self.inst.modpacks_open = false;
            return;
        }
        ui.add_space(8.0);
        crate::widgets::row(ui, |ui| {
            ui.vertical(|ui| {
                widgets::page_header(
                    ui,
                    p,
                    t!("instances.hub.title"),
                    t!("instances.hub.subtitle"),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let busy = self.inst.pack_installing.is_some();
                let import = ui
                    .add_enabled_ui(!busy, |ui| {
                        widgets::button(
                            ui,
                            p,
                            Some(Icon::Folder),
                            t!("instances.import_mrpack"),
                            false,
                        )
                    })
                    .inner;
                if import.clicked() {
                    self.inst.pack_installing = Some(String::new());
                    self.tasks.import_modpack_file();
                }
            });
        });
        ui.add_space(6.0);
        self.hub_body(ui);
        self.project_preview_in_hub(ui.ctx());
        self.pack_preview_dialog(ui.ctx());
    }

    pub(super) fn pack_install_progress(&mut self, ui: &mut egui::Ui) {
        if self.inst.pack_installing.is_none() {
            return;
        }
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.spinner();
            match &self.inst.install_progress {
                Some(progress) if progress.bytes_total > 0 => {
                    let f = progress.bytes_done as f32 / progress.bytes_total as f32;
                    ui.add(
                        egui::ProgressBar::new(f)
                            .desired_width(320.0)
                            .text(progress.stage.clone()),
                    );
                }
                Some(progress) => {
                    ui.label(RichText::new(&progress.stage).color(p.muted));
                }
                None => {
                    ui.label(RichText::new(t!("instances.installing_modpack")).color(p.muted));
                }
            }
        });
        ui.add_space(8.0);
    }

    pub(super) fn pack_row(&mut self, ui: &mut egui::Ui, hit: &ProjectHit) {
        let p = self.palette();
        let target = self.instance.clone();
        let kind = ProjectType::Modpack;
        let installed = self.hub_installed(ui.ctx(), kind, &target);
        let action = self.hub_action(kind, &target, &hit.project_id, &installed);
        let mut open = false;
        ui.horizontal(|ui| {
            super::content_install::select_content(
                ui,
                p,
                &mut self.inst.search.selected,
                hit,
                action.selectable(),
            );
            open |= ui
                .scope(|ui| mod_icon(ui, self, hit.icon_url.as_deref()))
                .response
                .interact(egui::Sense::CLICK)
                .on_hover_text(t!("instances.preview_project"))
                .clicked();
            ui.vertical(|ui| {
                let title = ui
                    .add(
                        egui::Label::new(RichText::new(&hit.title).strong().color(p.text))
                            .wrap()
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(t!("instances.preview_project"));
                widgets::focus_ring(ui, &title);
                open |= title.clicked();
                ui.label(
                    RichText::new(tf!("common.by_author", author = hit.author))
                        .small()
                        .color(p.muted),
                );
                ui.label(RichText::new(&hit.description).color(p.muted));
                ui.label(
                    RichText::new(super::hub::stats_line(hit))
                        .small()
                        .color(p.muted),
                );
                ui.horizontal_wrapped(|ui| {
                    self.hub_install_button(ui, kind, &target, hit, action);
                    if widgets::button(ui, p, None, t!("instances.view_mods"), false).clicked() {
                        self.open_pack_preview(&hit.project_id, &hit.title, None);
                    }
                    let expanded =
                        self.inst.pack_choosing.as_deref() == Some(hit.project_id.as_str());
                    let label = if expanded {
                        t!("instances.hide_versions")
                    } else {
                        t!("instances.versions")
                    };
                    if widgets::button(ui, p, None, label, false).clicked() {
                        if expanded {
                            self.inst.pack_choosing = None;
                        } else {
                            self.inst.pack_choosing = Some(hit.project_id.clone());
                            if !self.inst.pack_versions.contains_key(&hit.project_id) {
                                self.inst
                                    .pack_versions
                                    .insert(hit.project_id.clone(), Load::Loading);
                                self.tasks.pack_versions(hit.project_id.clone());
                            }
                        }
                    }
                });
            });
        });
        if open {
            self.open_discover_project(ui.ctx(), &target, hit, kind, None);
        }
        if self.inst.pack_choosing.as_deref() == Some(hit.project_id.as_str()) {
            self.pack_version_list(ui, &hit.project_id, &hit.title);
        }
    }

    /// The versions of one modpack, each with its Minecraft version and loader and an Install button.
    fn pack_version_list(&mut self, ui: &mut egui::Ui, project_id: &str, title: &str) {
        let p = self.palette();
        let versions = match self.inst.pack_versions.get(project_id) {
            None | Some(Load::Loading) => {
                ui.spinner();
                return;
            }
            Some(Load::Failed(e)) => {
                ui.label(
                    RichText::new(tf!("instances.could_not_load_the_versions", e = e))
                        .color(p.error),
                );
                return;
            }
            Some(Load::Ready(list)) => list.clone(),
        };
        if versions.is_empty() {
            ui.label(
                RichText::new(t!("instances.this_modpack_has_no_installable_versions"))
                    .color(p.muted),
            );
            return;
        }
        let busy = self.inst.pack_installing.is_some();
        let mut chosen = None;
        let mut viewed = None;
        egui::ScrollArea::vertical()
            .id_salt("pack_versions")
            .max_height(240.0)
            .show(ui, |ui| {
                for v in &versions {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&v.label).strong().color(p.text));
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {}",
                                v.game_versions.join(", "),
                                v.loaders.join(", "),
                                v.version_type
                            ))
                            .small()
                            .color(p.muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let install = ui
                                .add_enabled_ui(!busy, |ui| {
                                    widgets::button(
                                        ui,
                                        p,
                                        Some(Icon::Plus),
                                        t!("instances.install"),
                                        false,
                                    )
                                })
                                .inner;
                            if install.clicked() {
                                chosen = Some(v.id.clone());
                            }
                            if widgets::button(ui, p, None, t!("instances.view_mods"), false)
                                .clicked()
                            {
                                viewed = Some(v.id.clone());
                            }
                        });
                    });
                }
            });
        if let Some(version_id) = viewed {
            self.open_pack_preview(project_id, title, Some(version_id));
        }
        if let Some(version_id) = chosen {
            self.inst.pack_installing = Some(project_id.to_owned());
            self.inst.install_progress = None;
            self.inst.pack_choosing = None;
            self.tasks
                .install_modpack(project_id.to_owned(), Some(version_id));
        }
    }

    /// A modpack install (or import) finished.
    pub(crate) fn on_modpack_installed(&mut self, result: Result<Option<Instance>, String>) {
        self.inst.pack_installing = None;
        self.inst.install_progress = None;
        match result {
            Ok(Some(instance)) => {
                self.reload_instances();
                self.toasts.push(
                    Kind::Success,
                    tf!("instances.installed_4", name = instance.name),
                    "Press Play when you're ready.",
                );
                self.inst.modpacks_open = false;
                self.open_instance(&instance.id);
            }
            Ok(None) => {}
            Err(e) => self.toasts.push(
                Kind::Error,
                t!("instances.could_not_install_modpack"),
                crate::i18n::core_error(&e),
            ),
        }
    }
}
