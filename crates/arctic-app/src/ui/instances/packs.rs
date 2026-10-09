//! Resource packs and shaders for an instance: what's installed (switch on
//! or off, delete) and a Modrinth browser to add more.

use std::collections::HashSet;

use arctic_core::instances::Instance;
use arctic_core::mods::packs::{self, Pack, PackKind};
use arctic_core::mods::{ProjectHit, SearchPage, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::browse::{compact, sort_combo};
use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

const PAGE_SIZE: usize = 20;
const SEARCH_DELAY: f64 = 0.35;
const MB: f64 = 1024.0 * 1024.0;

pub struct PacksUi {
    kind: PackKind,
    text: String,
    sort: SortBy,
    request: u64,
    results: Option<Result<SearchPage, String>>,
    /// (instance id, kind) the results and list are for.
    showing: Option<(String, PackKind)>,
    installed: Vec<Pack>,
    installing: HashSet<String>,
    typed_at: Option<f64>,
}

impl Default for PacksUi {
    fn default() -> Self {
        Self {
            kind: PackKind::Resource,
            text: String::new(),
            sort: SortBy::Downloads,
            request: 0,
            results: None,
            showing: None,
            installed: Vec::new(),
            installing: HashSet::new(),
            typed_at: None,
        }
    }
}

impl ArcticApp {
    /// The instance's Minecraft version (Vanilla follows the Play tab).
    fn pack_game(&self, instance: &Instance) -> String {
        instance
            .version
            .clone()
            .or_else(|| self.settings.last_version.clone())
            .unwrap_or_default()
    }

    pub(super) fn packs_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let key = (instance.id.clone(), self.inst.packs.kind);
        if self.inst.packs.showing.as_ref() != Some(&key) {
            self.inst.packs.showing = Some(key);
            self.refresh_packs(instance);
            self.run_rp_search(instance);
        }
        ui.horizontal(|ui| {
            for kind in [PackKind::Resource, PackKind::Shader] {
                if ui
                    .selectable_label(self.inst.packs.kind == kind, kind.label())
                    .clicked()
                {
                    self.inst.packs.kind = kind;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Folder, "Open the folder").clicked() {
                    let dir = instance
                        .game_dir(&self.dirs)
                        .join(self.inst.packs.kind.folder());
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that_detached(&dir);
                }
            });
        });
        ui.add_space(6.0);
        self.installed_packs(ui, instance);
        ui.add_space(12.0);
        self.rp_search_bar(ui, instance);
        ui.add_space(8.0);
        self.rp_results(ui, instance);
    }

    fn refresh_packs(&mut self, instance: &Instance) {
        self.inst.packs.installed =
            packs::list(self.inst.packs.kind, &instance.game_dir(&self.dirs));
    }

    fn installed_packs(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let kind = self.inst.packs.kind;
        let game_dir = instance.game_dir(&self.dirs);
        let list = self.inst.packs.installed.clone();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let hint = match kind {
                PackKind::Resource => {
                    "Switched-on packs stack; the last one switched on is on top."
                }
                PackKind::Shader => {
                    "One shader at a time. Shaders run through Iris (added for you)."
                }
            };
            ui.label(RichText::new(hint).small().color(p.muted));
            if list.is_empty() {
                ui.label(RichText::new("None yet: find some below.").color(p.muted));
                return;
            }
            for pack in &list {
                crate::widgets::row(ui, |ui| {
                    let size = if pack.size > 0 {
                        format!("{:.1} MB", pack.size as f64 / MB)
                    } else {
                        "folder".into()
                    };
                    ui.label(RichText::new(&pack.file_name).color(p.text));
                    ui.label(RichText::new(size).small().color(p.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, p, Icon::Trash, "Delete").clicked() {
                            match packs::remove(kind, &game_dir, &pack.file_name) {
                                Ok(()) => self.refresh_packs(instance),
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    "Couldn't delete it",
                                    e.to_string(),
                                ),
                            }
                        }
                        let label = if pack.active { "On" } else { "Off" };
                        if widgets::button(ui, p, None, label, pack.active).clicked() {
                            match packs::set_active(kind, &game_dir, &pack.file_name, !pack.active)
                            {
                                Ok(()) => self.refresh_packs(instance),
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    "Couldn't switch it",
                                    e.to_string(),
                                ),
                            }
                        }
                    });
                });
            }
        });
    }

    fn rp_search_bar(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let mut search = false;
        crate::widgets::row(ui, |ui| {
            let hint = match self.inst.packs.kind {
                PackKind::Resource => "Search resource packs on Modrinth",
                PackKind::Shader => "Search shaders on Modrinth",
            };
            let field = ui.add(
                widgets::text_field(&mut self.inst.packs.text)
                    .hint_text(hint)
                    .desired_width(360.0),
            );
            let now = ui.input(|i| i.time);
            if field.changed() {
                self.inst.packs.typed_at = Some(now);
            }
            search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(at) = self.inst.packs.typed_at {
                if now - at >= SEARCH_DELAY {
                    search = true;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(
                            SEARCH_DELAY - (now - at),
                        ));
                }
            }
            let before = self.inst.packs.sort;
            sort_combo(ui, "pack_sort", &mut self.inst.packs.sort);
            search |= self.inst.packs.sort != before;
            search |= widgets::button(ui, p, None, "Search", true).clicked();
        });
        if search {
            self.inst.packs.typed_at = None;
            self.run_rp_search(instance);
        }
    }

    fn run_rp_search(&mut self, instance: &Instance) {
        let game = self.pack_game(instance);
        let packs = &mut self.inst.packs;
        packs.request += 1;
        packs.results = None;
        let query = SearchQuery {
            text: packs.text.trim().to_owned(),
            game_version: game,
            loader: None,
            sort: packs.sort,
            offset: 0,
            limit: PAGE_SIZE,
            project_type: packs.kind.project_type(),
        };
        self.tasks.pack_search(packs.request, query);
    }

    fn rp_results(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let hits: Vec<ProjectHit> = match &self.inst.packs.results {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Searching Modrinth…").color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                ui.label(RichText::new(format!("Search failed: {e}")).color(p.error));
                return;
            }
            Some(Ok(page)) if page.hits.is_empty() => {
                ui.label(RichText::new("Nothing found.").color(p.muted));
                return;
            }
            Some(Ok(page)) => page.hits.clone(),
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for hit in &hits {
                crate::widgets::row(ui, |ui| {
                    mod_icon(ui, self, hit.icon_url.as_deref());
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 130.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&hit.title).strong().color(p.text));
                            ui.label(
                                RichText::new(format!("by {}", hit.author))
                                    .small()
                                    .color(p.muted),
                            );
                        });
                        ui.label(RichText::new(&hit.description).color(p.muted));
                        ui.label(
                            RichText::new(format!("{} downloads", compact(hit.downloads)))
                                .small()
                                .color(p.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.inst.packs.installing.contains(&hit.project_id) {
                            ui.spinner();
                        } else if widgets::button(ui, p, Some(Icon::Plus), "Install", true)
                            .clicked()
                        {
                            self.inst.packs.installing.insert(hit.project_id.clone());
                            self.tasks.pack_install(
                                instance.clone(),
                                self.inst.packs.kind,
                                hit.project_id.clone(),
                                self.pack_game(instance),
                            );
                        }
                    });
                });
                ui.separator();
            }
        });
    }

    pub(crate) fn on_pack_search(&mut self, request: u64, result: Result<SearchPage, String>) {
        if request == self.inst.packs.request {
            self.inst.packs.results = Some(result);
        }
    }

    pub(crate) fn on_pack_installed(
        &mut self,
        instance_id: String,
        project: String,
        result: Result<crate::pack_tasks::PackDone, String>,
    ) {
        self.inst.packs.installing.remove(&project);
        match result {
            Ok(done) => {
                if done.shaders_switched_on {
                    self.update_instance(&instance_id, |i| i.shaders = true);
                }
                self.toasts
                    .push(Kind::Success, format!("Installed {}", done.file), done.note);
            }
            Err(e) => self.toasts.push(Kind::Error, "Couldn't install it", e),
        }
        if let Some(instance) = self.instance_by_id(&instance_id).cloned() {
            self.refresh_packs(&instance);
        }
    }
}
