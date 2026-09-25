//! Modrinth mod browser for an instance.

use std::collections::HashSet;

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme;
use crate::widgets;

const PAGE_SIZE: usize = 20;
/// Seconds after the last keystroke before searching.
const SEARCH_DELAY: f64 = 0.35;
const SORTS: [(SortBy, &str); 4] = [
    (SortBy::Relevance, "Relevance"),
    (SortBy::Downloads, "Most downloads"),
    (SortBy::Updated, "Recently updated"),
    (SortBy::Newest, "Newest"),
];

impl ArcticApp {
    pub(super) fn browse_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        // First visit for this instance: show popular mods right away.
        if self.inst.search.instance.as_deref() != Some(instance.id.as_str()) {
            self.inst.search.instance = Some(instance.id.clone());
            self.inst.search.text.clear();
            self.inst.search.sort = SortBy::Downloads;
            self.run_mod_search(instance, 0);
        }
        let mut search = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                widgets::text_field(&mut self.inst.search.text)
                    .hint_text("Search mods on Modrinth")
                    .desired_width(360.0),
            );
            let now = ui.input(|i| i.time);
            if field.changed() {
                self.inst.search.typed_at = Some(now);
            }
            search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // Search shortly after typing stops.
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
            egui::ComboBox::from_id_salt("mod_sort")
                .selected_text(SORTS.iter().find(|s| s.0 == before).map_or("", |s| s.1))
                .show_ui(ui, |ui| {
                    for (sort, name) in SORTS {
                        ui.selectable_value(&mut self.inst.search.sort, sort, name);
                    }
                });
            search |= self.inst.search.sort != before;
            search |= widgets::button(ui, p, None, "Search", true).clicked();
        });
        if search {
            self.inst.search.typed_at = None;
            self.run_mod_search(instance, 0);
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Showing {} mods for Minecraft {}",
                instance.loader.label(),
                instance.version.as_deref().unwrap_or("?")
            ))
            .small()
            .color(p.muted),
        );
        ui.add_space(8.0);
        self.search_results(ui, instance);
    }

    fn run_mod_search(&mut self, instance: &Instance, offset: usize) {
        let search = &mut self.inst.search;
        search.request += 1;
        search.loading = true;
        if offset == 0 {
            search.results = None;
        }
        let query = SearchQuery {
            text: search.text.trim().to_owned(),
            game_version: instance.version.clone().unwrap_or_default(),
            loader: instance.loader.kind(),
            sort: search.sort,
            offset,
            limit: PAGE_SIZE,
        };
        self.tasks.mod_search(search.request, query);
    }

    fn search_results(&mut self, ui: &mut egui::Ui, instance: &Instance) {
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
                ui.label(RichText::new("No mods found.").color(p.muted));
                return;
            }
            Some(Ok(page)) => page.hits.clone(),
        };
        let installed: HashSet<String> = match &self.inst.mod_files {
            Some((id, files)) if *id == instance.id => files
                .iter()
                .filter_map(|f| f.tracked.as_ref().map(|m| m.project_id.clone()))
                .collect(),
            _ => HashSet::new(),
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for hit in &hits {
                self.hit_row(ui, instance, hit, installed.contains(&hit.project_id));
                ui.separator();
            }
        });
        if let Some(progress) = &self.inst.install_progress
            && progress.bytes_total > 0
        {
            let f = progress.bytes_done as f32 / progress.bytes_total as f32;
            ui.add(
                egui::ProgressBar::new(f)
                    .desired_width(320.0)
                    .text(progress.stage.clone()),
            );
        }
    }

    fn hit_row(
        &mut self,
        ui: &mut egui::Ui,
        instance: &Instance,
        hit: &ProjectHit,
        installed: bool,
    ) {
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
                    RichText::new(format!("{} downloads", compact(hit.downloads)))
                        .small()
                        .color(p.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = self.inst.installing.contains(&hit.project_id);
                if installed {
                    ui.label(RichText::new("Installed").color(p.accent));
                } else if busy {
                    ui.spinner();
                } else if widgets::button(ui, p, Some(Icon::Plus), "Install", true).clicked() {
                    self.inst.installing.insert(hit.project_id.clone());
                    self.tasks
                        .mod_install(instance.clone(), hit.project_id.clone());
                }
            });
        });
    }
}

/// 1234567 → "1.2M".
fn compact(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}K", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

#[cfg(test)]
mod tests {
    use super::compact;

    #[test]
    fn compact_numbers() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(12_300), "12.3K");
        assert_eq!(compact(4_500_000), "4.5M");
    }
}
