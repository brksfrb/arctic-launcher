//! Instance and world selection, with compatibility and batch feedback.

use std::sync::mpsc::{Receiver, TryRecvError};

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, ProjectType, datapack};
use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::content_tasks::{
    ContentChoice, ContentProgress, ContentReport, ContentRequest, ContentTarget, ContentVersions,
};
use crate::tasks::Outcome;
use crate::toasts::Kind;
use crate::{t, tf, widgets};

#[derive(Default)]
pub(crate) struct ContentUi {
    picker: Option<Picker>,
    pub(super) progress: Option<ContentProgress>,
    report: Option<ContentReport>,
}

impl ContentUi {
    pub(super) fn close_picker(&mut self) -> bool {
        self.picker.take().is_some()
    }
}

struct Picker {
    request: ContentRequest,
    receiver: Receiver<ContentVersions>,
    versions: Option<ContentVersions>,
    instance: String,
    worlds: Option<Outcome<Vec<String>>>,
    world: Option<String>,
}

#[derive(Default)]
struct Compatibility {
    fitting: usize,
    fallback: usize,
    unavailable: usize,
}

impl Compatibility {
    fn of(choices: &[Outcome<ContentChoice>]) -> Self {
        let mut fit = Self::default();
        for choice in choices {
            match choice {
                Ok(choice) if choice.other_game => fit.fallback += 1,
                Ok(_) => fit.fitting += 1,
                Err(_) => fit.unavailable += 1,
            }
        }
        fit
    }

    fn mixed(&self) -> bool {
        self.fitting > 0 && self.unavailable > 0
    }

    fn can_install(&self) -> bool {
        self.fitting + self.fallback > 0
    }
}

impl ArcticApp {
    pub(super) fn pick_content_target(
        &mut self,
        ctx: &egui::Context,
        request: ContentRequest,
        instance: &Instance,
    ) {
        if self.inst.content.progress.is_some() {
            return;
        }
        let receiver = request.fetch_versions(ctx);
        self.inst.content.picker = Some(Picker {
            request,
            receiver,
            versions: None,
            instance: instance.id.clone(),
            worlds: None,
            world: None,
        });
    }

    pub(super) fn pick_single_content(
        &mut self,
        ctx: &egui::Context,
        kind: ProjectType,
        hit: &ProjectHit,
        version: Option<String>,
        instance: &Instance,
    ) {
        self.pick_content_target(
            ctx,
            ContentRequest {
                kind,
                hits: vec![hit.clone()],
                version,
                bulk: false,
            },
            instance,
        );
    }

    pub(super) fn content_selection_bar(
        &mut self,
        ui: &mut egui::Ui,
        kind: ProjectType,
        instance: &Instance,
    ) {
        self.content_status(ui);
        let count = self.inst.search.selected.len();
        if count == 0 {
            return;
        }
        let p = self.palette();
        ui.horizontal_wrapped(|ui| {
            ui.label(tf!("instances.hub.selected_count", count = count));
            if ui
                .add_enabled_ui(self.inst.content.progress.is_none(), |ui| {
                    widgets::button(ui, p, None, t!("instances.hub.install_selected"), true)
                })
                .inner
                .clicked()
            {
                let hits = self.inst.search.selected.clone();
                self.pick_content_target(
                    ui.ctx(),
                    ContentRequest {
                        kind,
                        hits,
                        version: None,
                        bulk: true,
                    },
                    instance,
                );
            }
            if widgets::button(ui, p, None, t!("instances.hub.clear_selection"), false).clicked() {
                self.inst.search.selected.clear();
            }
        });
        ui.add_space(6.0);
    }

    fn content_status(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if let Some(progress) = &self.inst.content.progress {
            let fraction = (progress.current.saturating_sub(1) as f32 + progress.share)
                / progress.total.max(1) as f32;
            ui.add(egui::ProgressBar::new(fraction).animate(true).text(tf!(
                "instances.hub.installing_batch",
                current = progress.current,
                total = progress.total,
                title = progress.title
            )));
            ui.add_space(6.0);
        }
        if let Some(report) = &self.inst.content.report {
            let failed = report.failed.len();
            let mut close = false;
            crate::theme::card(p).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(tf!(
                        "instances.hub.install_summary",
                        installed = report.succeeded,
                        failed = failed
                    ));
                    close = ui.button(t!("instances.close")).clicked();
                });
                for note in &report.notes {
                    ui.label(note);
                }
                if failed > 0 {
                    egui::ScrollArea::vertical()
                        .id_salt("content_failures")
                        .max_height(115.0)
                        .show(ui, |ui| {
                            for (name, reason) in &report.failed {
                                ui.colored_label(p.error, format!("{name} — {reason}"));
                            }
                        });
                }
            });
            if close {
                self.inst.content.report = None;
            }
            ui.add_space(6.0);
        }
    }

    pub(super) fn content_install_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.inst.content.picker.take() else {
            return;
        };
        let Some(current) = self.instance_by_id(&picker.instance).cloned() else {
            return;
        };
        if picker.versions.is_none() {
            match picker.receiver.try_recv() {
                Ok(versions) => picker.versions = Some(versions),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    picker.versions = Some(
                        picker
                            .request
                            .hits
                            .iter()
                            .map(|_| {
                                Err(t!("instances.project_request_stopped_please_retry").into())
                            })
                            .collect(),
                    );
                }
            }
        }

        let p = self.palette();
        let kind = picker.request.kind;
        let game = self.pack_game(&current);
        let choices = picker
            .versions
            .as_ref()
            .map(|v| picker.request.choices(v, &current, &game));
        let fit = choices.as_ref().map(|c| Compatibility::of(c));
        let mut next = None;
        let mut close = false;
        let mut install = false;
        let modal = egui::Modal::new(egui::Id::new("content_install_target"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(440.0_f32.min((ctx.content_rect().width() - 50.0).max(160.0)));
                let title = if kind == ProjectType::Modpack {
                    t!("instances.hub.install_selected")
                } else {
                    t!("instances.hub.choose_instance")
                };
                ui.heading(title);
                if picker.request.bulk {
                    ui.label(tf!(
                        "instances.hub.selected_count",
                        count = picker.request.hits.len()
                    ));
                } else if let Some(hit) = picker.request.hits.first() {
                    ui.label(&hit.title);
                }
                if kind == ProjectType::Modpack {
                    ui.label(t!("instances.hub.modpacks_create_instances"));
                } else {
                    next = self.content_target_option(ui, &picker, &current);
                }
                if kind == ProjectType::DataPack {
                    self.content_world_option(ui, &mut picker, &current);
                }

                ui.add_space(8.0);
                if let (Some(fit), Some(choices)) = (&fit, &choices) {
                    if fit.fallback > 0 {
                        ui.colored_label(
                            p.warn,
                            tf!(
                                "instances.hub.other_game_warning",
                                versions = choices
                                    .iter()
                                    .find_map(|choice| {
                                        let choice = choice.as_ref().ok()?;
                                        choice.other_game.then(|| {
                                            super::preview::short_game_list(
                                                &choice.version.game_versions,
                                            )
                                        })
                                    })
                                    .unwrap_or_default(),
                                game = game
                            ),
                        );
                    }
                    if fit.mixed() {
                        ui.colored_label(
                            p.warn,
                            tf!("instances.hub.partial_install", count = fit.unavailable),
                        );
                    } else if !fit.can_install() {
                        if let Some(reason) =
                            choices.iter().find_map(|choice| choice.as_ref().err())
                        {
                            ui.colored_label(p.error, reason);
                        }
                        if ui.button(t!("instances.retry")).clicked() {
                            picker.receiver = picker.request.fetch_versions(ctx);
                            picker.versions = None;
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(t!("tasks.finding_a_version"));
                    });
                }
                let wants_world = kind == ProjectType::DataPack;
                let enabled = next.is_none()
                    && picker.versions.is_some()
                    && fit.as_ref().is_some_and(|f| f.can_install())
                    && (!wants_world || picker.world.is_some());
                let label = match &fit {
                    Some(fit) if fit.fallback > 0 => t!("instances.hub.install_anyway"),
                    Some(fit) if fit.mixed() => t!("instances.hub.install_compatible"),
                    _ if picker.request.bulk => t!("instances.hub.install_selected"),
                    _ => t!("instances.install"),
                };
                ui.horizontal_wrapped(|ui| {
                    install = ui
                        .add_enabled_ui(enabled, |ui| widgets::button(ui, p, None, label, true))
                        .inner
                        .clicked();
                    close = widgets::button(ui, p, None, t!("instances.cancel"), false).clicked();
                });
            });
        if let Some(id) = next {
            picker.instance = id;
            picker.worlds = None;
            picker.world = None;
        }
        if install && let Some(choices) = choices {
            let total = picker.request.hits.len();
            let title = picker.request.hits[0].title.clone();
            if picker.request.bulk {
                self.inst.search.selected.clear();
                self.inst.content.report = None;
            }
            self.inst.content.progress = Some(ContentProgress {
                current: 1,
                total,
                title,
                share: 0.0,
            });
            self.tasks.install_content(
                picker.request,
                choices,
                ContentTarget {
                    instance: current,
                    game,
                    world: picker.world,
                },
            );
        } else if !close && !modal.should_close() {
            self.inst.content.picker = Some(picker);
        }
    }

    fn content_target_option(
        &self,
        ui: &mut egui::Ui,
        picker: &Picker,
        instance: &Instance,
    ) -> Option<String> {
        let p = self.palette();
        let label = |target: &Instance| {
            let prefix = format!("{} · {}", target.name, Self::instance_subtitle(target));
            let Some(versions) = &picker.versions else {
                return RichText::new(prefix);
            };
            let choices = picker
                .request
                .choices(versions, target, &self.pack_game(target));
            let fit = Compatibility::of(&choices);
            if fit.unavailable > 0 || fit.fallback > 0 {
                let flag = if fit.mixed() {
                    t!("instances.hub.partially_compatible")
                } else if fit.fallback > 0 {
                    t!("instances.hub.other_game")
                } else {
                    t!("instances.incompatible")
                };
                RichText::new(format!("{prefix} · {flag}")).color(if fit.can_install() {
                    p.warn
                } else {
                    p.error
                })
            } else {
                RichText::new(prefix)
            }
        };
        let mut next = None;
        egui::ComboBox::from_id_salt("install_target_instance")
            .selected_text(label(instance))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for target in std::iter::once(&self.instance).chain(&self.custom_instances) {
                    if ui
                        .selectable_label(target.id == instance.id, label(target))
                        .clicked()
                        && target.id != instance.id
                    {
                        next = Some(target.id.clone());
                    }
                }
            });
        next
    }

    fn content_world_option(&self, ui: &mut egui::Ui, picker: &mut Picker, instance: &Instance) {
        let p = self.palette();
        let worlds = picker.worlds.get_or_insert_with(|| {
            datapack::worlds(&instance.game_dir(&self.dirs)).map_err(|e| e.to_string())
        });
        match worlds {
            Ok(worlds) if worlds.is_empty() => {
                ui.label(t!("instances.hub.no_worlds"));
            }
            Ok(worlds) => {
                ui.label(t!("instances.hub.choose_world"));
                egui::ComboBox::from_id_salt("data_pack_world")
                    .selected_text(
                        picker
                            .world
                            .as_deref()
                            .unwrap_or(t!("instances.hub.choose_world")),
                    )
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for world in worlds {
                            if ui
                                .selectable_label(
                                    picker.world.as_ref() == Some(world),
                                    world.as_str(),
                                )
                                .clicked()
                            {
                                picker.world = Some(world.clone());
                            }
                        }
                    });
            }
            Err(e) => {
                ui.colored_label(p.error, e);
            }
        }
    }

    pub(super) fn on_content_installed(&mut self, report: ContentReport) {
        self.inst.content.progress = None;
        if report.kind == ProjectType::Modpack && report.succeeded > 0 {
            self.reload_instances();
        } else {
            if report.shaders_switched_on {
                self.update_instance(&report.target, |instance| instance.shaders = true);
            }
            if matches!(report.kind, ProjectType::Mod | ProjectType::Shader) {
                self.refresh_mods(&report.target);
            }
            if matches!(report.kind, ProjectType::ResourcePack | ProjectType::Shader)
                && self
                    .inst
                    .packs
                    .showing
                    .as_ref()
                    .is_some_and(|(id, _, _)| id == &report.target)
                && let Some(instance) = self.instance_by_id(&report.target).cloned()
            {
                self.refresh_packs(&instance);
            }
        }

        if report.bulk {
            self.toasts.push(
                if report.failed.is_empty() {
                    Kind::Success
                } else {
                    Kind::Error
                },
                tf!(
                    "instances.hub.install_summary",
                    installed = report.succeeded,
                    failed = report.failed.len()
                ),
                "",
            );
            self.inst.content.report = Some(report);
        } else if let Some((title, error)) = report.failed.into_iter().next() {
            self.toasts.push(
                Kind::Error,
                tf!("instances.hub.install_failed_for", title = title),
                error,
            );
        } else {
            self.toasts.push(
                Kind::Success,
                t!("instances.installed"),
                report.notes.join(" "),
            );
        }
    }
}

/// Checkboxes retain selections across searches and pagination in the current browser.
pub(super) fn select_content(ui: &mut egui::Ui, selected: &mut Vec<ProjectHit>, hit: &ProjectHit) {
    let position = selected
        .iter()
        .position(|item| item.project_id == hit.project_id);
    let mut checked = position.is_some();
    if ui
        .checkbox(&mut checked, "")
        .on_hover_text(t!("instances.hub.select_content"))
        .changed()
    {
        if let Some(index) = position {
            selected.remove(index);
        } else {
            selected.push(hit.clone());
        }
    }
}
