//! Modpack browser: search Modrinth modpacks and install one as a new
//! instance, or import a `.mrpack` file.

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

const PAGE_SIZE: usize = 20;
const SEARCH_DELAY: f64 = 0.35;
/// Marks the shared search state as belonging to this page.
const SEARCH_OWNER: &str = "\0modpacks";

impl ArcticApp {
    pub(super) fn modpacks_page(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if widgets::button(ui, p, Some(Icon::ChevronLeft), "All instances", false).clicked() {
            self.inst.modpacks_open = false;
            return;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                widgets::page_header(
                    ui,
                    p,
                    "Modpacks",
                    "Install a whole pack in one click. Each one becomes its own instance.",
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let busy = self.inst.pack_installing.is_some();
                let import = ui
                    .add_enabled_ui(!busy, |ui| {
                        widgets::button(ui, p, Some(Icon::Folder), "Import .mrpack…", false)
                    })
                    .inner;
                if import.clicked() {
                    self.inst.pack_installing = Some(String::new());
                    self.tasks.import_modpack_file();
                }
            });
        });
        if self.inst.search.instance.as_deref() != Some(SEARCH_OWNER) {
            self.inst.search.instance = Some(SEARCH_OWNER.to_owned());
            self.inst.search.text.clear();
            self.inst.search.sort = SortBy::Downloads;
            self.run_pack_search();
        }
        self.pack_search_bar(ui);
        ui.add_space(8.0);
        self.pack_install_progress(ui);
        self.pack_results(ui);
    }

    fn pack_search_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let mut search = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                widgets::text_field(&mut self.inst.search.text)
                    .hint_text("Search modpacks on Modrinth")
                    .desired_width(360.0),
            );
            let now = ui.input(|i| i.time);
            if field.changed() {
                self.inst.search.typed_at = Some(now);
            }
            search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(at) = self.inst.search.typed_at {
                if now - at >= SEARCH_DELAY {
                    search = true;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(
                            SEARCH_DELAY - (now - at),
                        ));
                }
            }
            let before = self.inst.search.sort;
            super::browse::sort_combo(ui, "pack_sort", &mut self.inst.search.sort);
            search |= self.inst.search.sort != before;
            search |= widgets::button(ui, p, None, "Search", true).clicked();
        });
        if search {
            self.inst.search.typed_at = None;
            self.run_pack_search();
        }
    }

    fn run_pack_search(&mut self) {
        let search = &mut self.inst.search;
        search.request += 1;
        search.loading = true;
        search.results = None;
        let query = SearchQuery {
            text: search.text.trim().to_owned(),
            sort: search.sort,
            limit: PAGE_SIZE,
            project_type: arctic_core::mods::ProjectType::Modpack,
            ..SearchQuery::default()
        };
        self.tasks.mod_search(search.request, query);
    }

    fn pack_install_progress(&mut self, ui: &mut egui::Ui) {
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
                    ui.label(RichText::new("Installing modpack…").color(p.muted));
                }
            }
        });
        ui.add_space(8.0);
    }

    fn pack_results(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let hits = match &self.inst.search.results {
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
                ui.label(RichText::new("No modpacks found.").color(p.muted));
                return;
            }
            Some(Ok(page)) => page.hits.clone(),
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for hit in &hits {
                self.pack_row(ui, hit);
                ui.separator();
            }
        });
    }

    fn pack_row(&mut self, ui: &mut egui::Ui, hit: &ProjectHit) {
        let p = self.palette();
        ui.horizontal(|ui| {
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
                    RichText::new(format!(
                        "{} downloads",
                        super::browse::compact(hit.downloads)
                    ))
                    .small()
                    .color(p.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = self.inst.pack_installing.is_some();
                if self.inst.pack_installing.as_deref() == Some(hit.project_id.as_str()) {
                    ui.spinner();
                } else {
                    let install = ui
                        .add_enabled_ui(!busy, |ui| {
                            widgets::button(ui, p, Some(Icon::Plus), "Install", true)
                        })
                        .inner;
                    if install.clicked() {
                        self.inst.pack_installing = Some(hit.project_id.clone());
                        self.inst.install_progress = None;
                        self.tasks.install_modpack(hit.project_id.clone());
                    }
                }
            });
        });
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
                    format!("Installed {}", instance.name),
                    "Press Play when you're ready.",
                );
                self.inst.modpacks_open = false;
                self.open_instance(&instance.id);
            }
            Ok(None) => {}
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not install modpack", e),
        }
    }
}
