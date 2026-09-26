//! The community skin gallery inside the Skins tab: browse, preview, add
//! to your library or wear, report; and share your own skins.

use std::collections::HashMap;

use arctic_core::cosmetics::{GalleryItem, GallerySort};
use eframe::egui::{self, RichText, vec2};

use super::Selection;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::gallery_tasks::GalleryResult;
use crate::tasks::Event;
use crate::toasts::Kind;
use crate::widgets;

const SEARCH_DELAY: f64 = 0.35;

#[derive(Default)]
pub struct GalleryUi {
    pub query: String,
    pub sort: GallerySort,
    request: u64,
    pub loading: bool,
    pub page: Option<Result<Vec<GalleryItem>, String>>,
    /// Texture hash → PNG for the listed skins.
    pub pngs: HashMap<String, Vec<u8>>,
    typed_at: Option<f64>,
    pub busy: bool,
}

impl ArcticApp {
    pub(super) fn gallery_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if self.skins.gallery.page.is_none() && !self.skins.gallery.loading {
            self.run_gallery_search();
        }
        let mut search = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                widgets::text_field(&mut self.skins.gallery.query)
                    .hint_text("Search skins or creators")
                    .desired_width(240.0),
            );
            let now = ui.input(|i| i.time);
            if field.changed() {
                self.skins.gallery.typed_at = Some(now);
            }
            if let Some(at) = self.skins.gallery.typed_at {
                if now - at >= SEARCH_DELAY {
                    search = true;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(
                            SEARCH_DELAY - (now - at),
                        ));
                }
            }
            let before = self.skins.gallery.sort;
            ui.selectable_value(
                &mut self.skins.gallery.sort,
                GallerySort::Popular,
                "Popular",
            );
            ui.selectable_value(&mut self.skins.gallery.sort, GallerySort::New, "New");
            search |= self.skins.gallery.sort != before;
            if self.skins.gallery.loading {
                ui.spinner();
            }
        });
        if search {
            self.skins.gallery.typed_at = None;
            self.run_gallery_search();
        }
        ui.add_space(8.0);
        let items = match &self.skins.gallery.page {
            None => return,
            Some(Err(e)) => {
                ui.label(RichText::new(e).color(p.muted));
                return;
            }
            Some(Ok(items)) if items.is_empty() => {
                ui.label(
                    RichText::new(
                        "Nothing here yet. Share a skin from your library to start the gallery.",
                    )
                    .color(p.muted),
                );
                return;
            }
            Some(Ok(items)) => items.clone(),
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for item in &items {
                let selected = self.skins.selection == Selection::Gallery(item.clone());
                if self.skin_tile(
                    ui,
                    &format!("gal:{}", item.texture),
                    &item.name,
                    Some(item.variant()),
                    selected,
                ) {
                    self.skins.selection = Selection::Gallery(item.clone());
                }
            }
        });
    }

    fn run_gallery_search(&mut self) {
        let g = &mut self.skins.gallery;
        g.request += 1;
        g.loading = true;
        self.tasks
            .gallery_search(g.request, g.sort, g.query.trim().to_owned());
    }

    /// Actions under the preview for a gallery skin.
    pub(super) fn gallery_actions(&mut self, ui: &mut egui::Ui, item: &GalleryItem) {
        let p = self.palette();
        ui.label(RichText::new(&item.name).size(18.0).strong().color(p.text));
        ui.label(
            RichText::new(format!(
                "by {} · used {} times",
                item.author, item.downloads
            ))
            .color(p.muted),
        );
        ui.add_space(4.0);
        let busy = self.skins.gallery.busy;
        ui.horizontal(|ui| {
            let can_wear = self.arctic_state().is_some() && !busy;
            let wear = ui
                .add_enabled_ui(can_wear, |ui| {
                    widgets::button(ui, p, Some(Icon::Check), "Wear", true)
                })
                .inner;
            if wear.clicked() {
                self.skins.gallery.busy = true;
                self.tasks.gallery_take(item.clone(), true);
            }
            let add = ui
                .add_enabled_ui(!busy, |ui| {
                    widgets::button(ui, p, Some(Icon::Plus), "Add to library", false)
                })
                .inner;
            if add.clicked() {
                self.skins.gallery.busy = true;
                self.tasks.gallery_take(item.clone(), false);
            }
            if busy {
                ui.spinner();
            }
        });
        if let Some(account) = self.accounts.active().cloned()
            && ui.link("Report this skin").clicked()
        {
            self.tasks.gallery_report(account, item.id.clone());
        }
    }

    /// "Share to gallery" for a library skin.
    pub(super) fn share_button(
        &mut self,
        ui: &mut egui::Ui,
        entry: &arctic_core::skins::SkinEntry,
    ) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        let enabled = self.arctic_state().is_some() && !self.skins.gallery.busy;
        let share = ui
            .add_enabled(enabled, egui::Button::new("Share to gallery"))
            .on_hover_text("Everyone can find and use it in the Arctic gallery");
        if share.clicked()
            && let Ok(png) = arctic_core::skins::Library::read_png(&self.skins_dir(), &entry.id)
        {
            self.skins.gallery.busy = true;
            self.tasks
                .gallery_share(account, png, entry.variant, entry.name.clone());
        }
    }

    pub(super) fn on_gallery_event(&mut self, event: Event) {
        let g = &mut self.skins.gallery;
        match event {
            Event::Gallery(request, result) if request == g.request => {
                g.loading = false;
                match result {
                    Ok(GalleryResult { page, textures }) => {
                        g.pngs.extend(textures);
                        g.page = Some(Ok(page.items));
                    }
                    Err(e) => g.page = Some(Err(e)),
                }
            }
            Event::GalleryTaken(result) => {
                g.busy = false;
                match result {
                    Ok((name, png, variant, wear)) => {
                        self.add_skin(&name, &png, Some(variant));
                        if wear
                            && let Selection::Library(id) = self.skins.selection.clone()
                            && let Some(entry) = self
                                .skins
                                .library
                                .skins
                                .iter()
                                .find(|s| s.id == id)
                                .cloned()
                        {
                            self.wear_skin(&entry);
                        }
                    }
                    Err(e) => self.toasts.push(Kind::Error, "Could not get skin", e),
                }
            }
            Event::GalleryDone(result) => {
                g.busy = false;
                match result {
                    Ok(message) => {
                        self.toasts.push(Kind::Success, message, "");
                        g.page = None;
                    }
                    Err(e) => self.toasts.push(Kind::Error, "Gallery", e),
                }
            }
            _ => {}
        }
    }
}
