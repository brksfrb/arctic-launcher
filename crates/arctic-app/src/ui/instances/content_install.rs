//! Instance and world selection, with compatibility and batch feedback.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, ProjectType, datapack};
use eframe::egui::{self, RichText};

use super::hub::SEARCH_OWNER;
use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::content_tasks::{
    ContentChoice, ContentProgress, ContentReport, ContentRequest, ContentTarget, ContentVersions,
};
use crate::tasks::Outcome;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::{t, tf, widgets};

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(crate) struct ContentUi {
    picker: Option<Picker>,
    running: Option<ContentProjects>,
    queue: VecDeque<PendingInstall>,
    progress: Option<ContentProgress>,
    report: Option<ContentReport>,
}

impl ContentUi {
    pub(super) fn installing(&self, target: &str, kind: ProjectType, project: &str) -> bool {
        let matches = |projects: &ContentProjects| {
            projects.kind == kind
                && (kind == ProjectType::Modpack || projects.target == target)
                && projects.ids.contains(project)
        };
        self.running.as_ref().is_some_and(matches)
            || self.queue.iter().any(|job| matches(&job.projects))
            || self.picker.as_ref().is_some_and(|picker| {
                picker.request.kind == kind
                    && (kind == ProjectType::Modpack || picker.instance == target)
                    && picker
                        .request
                        .hits
                        .iter()
                        .any(|hit| hit.project_id == project)
            })
    }

    fn batch_pending(&self) -> bool {
        self.running.as_ref().is_some_and(|job| job.bulk)
            || self.queue.iter().any(|job| job.projects.bulk)
            || self
                .picker
                .as_ref()
                .is_some_and(|picker| picker.request.bulk)
    }

    pub(super) fn set_progress(&mut self, job: u64, progress: ContentProgress) {
        if self
            .running
            .as_ref()
            .is_some_and(|running| running.job == job)
        {
            self.progress = Some(progress);
        }
    }

    pub(super) fn close_picker(&mut self) -> bool {
        self.picker.take().is_some()
    }
}

struct ContentProjects {
    job: u64,
    target: String,
    kind: ProjectType,
    ids: HashSet<String>,
    bulk: bool,
}

struct PendingInstall {
    projects: ContentProjects,
    request: ContentRequest,
    choices: Vec<Outcome<ContentChoice>>,
    target: ContentTarget,
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
        if self.inst.content.picker.is_some() || request.hits.is_empty() {
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
        let hits: Vec<_> = self
            .inst
            .search
            .selected
            .iter()
            .filter(|hit| !self.hub_content_busy(kind, instance, &hit.project_id))
            .cloned()
            .collect();
        crate::theme::card(p).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().interact_size.y = widgets::FIELD_HEIGHT;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(tf!("instances.hub.selected_count", count = count)).strong(),
                );
                if ui
                    .add_enabled_ui(
                        !self.inst.content.batch_pending() && !hits.is_empty(),
                        |ui| widgets::field_button(ui, p, t!("instances.hub.install_all"), true),
                    )
                    .inner
                    .clicked()
                {
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
                if widgets::field_button(ui, p, t!("instances.hub.clear_selection"), false)
                    .clicked()
                {
                    self.inst.search.selected.clear();
                }
            });
        });
        ui.add_space(6.0);
    }

    fn content_status(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if let Some(progress) = &self.inst.content.progress {
            let fraction = (progress.current.saturating_sub(1) as f32 + progress.share)
                / progress.total.max(1) as f32;
            ui.label(tf!(
                "instances.hub.installing_batch",
                current = progress.current,
                total = progress.total,
                title = progress.title
            ));
            ui.add(
                egui::ProgressBar::new(fraction)
                    .animate(true)
                    .show_percentage(),
            );
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
                Ok(versions) => {
                    for (hit, result) in picker.request.hits.iter().zip(&versions) {
                        if let Ok(versions) = result {
                            self.inst.hub.installed.remember_versions(
                                picker.request.kind,
                                &hit.project_id,
                                versions,
                            );
                        }
                    }
                    picker.versions = Some(versions);
                    ctx.request_repaint();
                }
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
        let choices = picker.versions.as_ref().map(|versions| {
            let installed = self.hub_installed(ctx, kind, &current);
            let mut choices = picker.request.choices(versions, &current, &game);
            for (hit, choice) in picker.request.hits.iter().zip(&mut choices) {
                if installed.contains(&hit.project_id) {
                    *choice = Err(t!("instances.installed").to_owned());
                } else if self.hub_content_busy(kind, &current, &hit.project_id) {
                    *choice = Err(t!("instances.installing").to_owned());
                }
            }
            choices
        });
        let fit = choices.as_ref().map(|c| Compatibility::of(c));
        let mut next = None;
        let mut close = false;
        let mut install = false;
        let modal = egui::Modal::new(egui::Id::new("content_install_target"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(440.0_f32.min((ctx.content_rect().width() - 50.0).max(160.0)));
                let title = if kind != ProjectType::Modpack {
                    t!("instances.hub.choose_instance")
                } else if picker.request.bulk {
                    t!("instances.hub.install_all")
                } else {
                    t!("instances.modpacks")
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
            let projects = ContentProjects {
                job: NEXT_JOB.fetch_add(1, Ordering::Relaxed),
                target: current.id.clone(),
                kind,
                ids: picker
                    .request
                    .hits
                    .iter()
                    .zip(&choices)
                    .filter(|(_, choice)| choice.is_ok())
                    .map(|(hit, _)| hit.project_id.clone())
                    .collect(),
                bulk: picker.request.bulk,
            };
            self.inst.content.queue.push_back(PendingInstall {
                projects,
                request: picker.request,
                choices,
                target: ContentTarget {
                    instance: current,
                    game,
                    world: picker.world,
                },
            });
            self.start_next_content_install();
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

    fn start_next_content_install(&mut self) {
        if self.inst.content.running.is_some() {
            return;
        }
        while let Some(job) = self.inst.content.queue.pop_front() {
            let target = &job.target.instance;
            if job.projects.kind != ProjectType::Modpack
                && !self.instance_by_id(&target.id).is_some_and(|current| {
                    current.loader == target.loader
                        && self.pack_game(current) == job.target.game
                        && current.game_dir(&self.dirs) == target.game_dir(&self.dirs)
                })
            {
                self.toasts.push(
                    Kind::Error,
                    tf!(
                        "instances.hub.install_failed_for",
                        title = job.request.hits[0].title
                    ),
                    t!("instances.hub.target_changed"),
                );
                continue;
            }
            self.inst.content.progress = Some(ContentProgress {
                current: 1,
                total: job.request.hits.len(),
                title: job.request.hits[0].title.clone(),
                share: 0.0,
            });
            if job.request.bulk {
                self.inst.content.report = None;
            }
            let id = job.projects.job;
            self.inst.content.running = Some(job.projects);
            self.tasks
                .install_content(id, job.request, job.choices, job.target);
            break;
        }
    }

    pub(super) fn on_content_item_installed(&mut self, job: u64, project: &str) {
        let Some(running) = self
            .inst
            .content
            .running
            .as_mut()
            .filter(|running| running.job == job)
        else {
            return;
        };
        running.ids.remove(project);
        let kind = running.kind;
        let target = running.target.clone();
        if kind == ProjectType::Mod {
            self.refresh_mods(&target);
        } else if kind != ProjectType::Modpack
            && let Some(instance) = self.instance_by_id(&target)
        {
            self.inst
                .hub
                .installed
                .refresh(kind, &instance.game_dir(&self.dirs));
        }
        let browser = self.inst.search.instance.as_deref();
        let discover = browser == Some(SEARCH_OWNER)
            && kind == self.inst.hub.section.project_type()
            && (kind == ProjectType::Modpack || self.inst.hub.target.as_deref() == Some(&target));
        if discover || browser == Some(&target) {
            prune_selection(&mut self.inst.search.selected, |id| id == project);
        }
    }

    pub(super) fn on_content_installed(&mut self, job: u64, report: ContentReport) {
        if self
            .inst
            .content
            .running
            .as_ref()
            .is_none_or(|running| running.job != job)
        {
            return;
        }
        self.inst.content.running = None;
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
            if matches!(
                report.kind,
                ProjectType::ResourcePack | ProjectType::Shader | ProjectType::DataPack
            ) && let Some(instance) = self.instance_by_id(&report.target)
            {
                let game_dir = instance.game_dir(&self.dirs);
                self.inst.hub.installed.refresh(report.kind, &game_dir);
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
            if !report.failed.is_empty() {
                self.toasts.push(
                    Kind::Error,
                    tf!(
                        "instances.hub.install_summary",
                        installed = report.succeeded,
                        failed = report.failed.len()
                    ),
                    "",
                );
            }
            self.inst.content.report = Some(report);
        } else if let Some((title, error)) = report.failed.into_iter().next() {
            self.toasts.push(
                Kind::Error,
                tf!("instances.hub.install_failed_for", title = title),
                error,
            );
        } else if report.kind == ProjectType::Modpack || !report.notes.is_empty() {
            self.toasts.push(
                Kind::Success,
                t!("instances.installed"),
                report.notes.join(" "),
            );
        }
        self.start_next_content_install();
    }
}

pub(super) fn prune_selection(selected: &mut Vec<ProjectHit>, installed: impl Fn(&str) -> bool) {
    selected.retain(|hit| !installed(&hit.project_id));
}

const SELECT_SIZE: f32 = 28.0;

/// Checkboxes retain selections across searches and pagination in the current browser.
pub(super) fn select_content(
    ui: &mut egui::Ui,
    p: &Palette,
    selected: &mut Vec<ProjectHit>,
    hit: &ProjectHit,
    selectable: bool,
) {
    let position = selected
        .iter()
        .position(|item| item.project_id == hit.project_id);
    if !selectable {
        ui.allocate_space(egui::Vec2::splat(SELECT_SIZE));
        return;
    }
    let (_, rect) = ui.allocate_space(egui::Vec2::splat(SELECT_SIZE));
    // The sticky bar can appear or disappear as this checkbox changes.
    let id = ui.make_persistent_id(("select_content", &hit.project_id));
    let response = ui.interact(rect, id, egui::Sense::click());
    let mut checked = position.is_some();
    if response.clicked() {
        response.request_focus();
        checked = !checked;
        if let Some(index) = position {
            selected.remove(index);
        } else {
            selected.push(hit.clone());
        }
    }

    let square = rect.shrink(4.0);
    let fill = if checked { p.accent } else { p.surface };
    let edge = if checked || response.hovered() {
        p.accent
    } else {
        p.muted
    };
    ui.painter().rect(
        square,
        4,
        fill,
        egui::Stroke::new(1.5, edge),
        egui::StrokeKind::Inside,
    );
    if checked {
        icons::draw(ui.painter(), Icon::Check, square.shrink(3.0), p.on_accent);
    }
    widgets::focus_ring(ui, &response);
    let label = t!("instances.hub.select_content");
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, checked, label)
    });
    response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(project: &str) -> ProjectHit {
        ProjectHit {
            project_id: project.into(),
            slug: project.into(),
            title: project.into(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            follows: 0,
            icon_url: None,
            featured_gallery: None,
            categories: Vec::new(),
            game_versions: Vec::new(),
        }
    }

    #[test]
    fn newly_installed_items_drop_out_of_selection_even_offscreen() {
        let mut selected = vec![hit("offscreen"), hit("still selected"), hit("installed")];
        let installed = HashSet::from(["offscreen", "installed"]);
        prune_selection(&mut selected, |id| installed.contains(id));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].project_id, "still selected");
    }

    #[test]
    fn running_install_is_scoped_to_its_projects_and_target() {
        let ui = ContentUi {
            running: Some(ContentProjects {
                job: 1,
                target: "fabric".into(),
                kind: ProjectType::Mod,
                ids: HashSet::from(["first".into(), "second".into()]),
                bulk: true,
            }),
            ..ContentUi::default()
        };
        assert!(ui.installing("fabric", ProjectType::Mod, "first"));
        assert!(ui.installing("fabric", ProjectType::Mod, "second"));
        assert!(!ui.installing("fabric", ProjectType::Mod, "unrelated"));
        assert!(!ui.installing("forge", ProjectType::Mod, "first"));
        assert!(!ui.installing("fabric", ProjectType::DataPack, "first"));
        assert!(ui.batch_pending());
    }
}
