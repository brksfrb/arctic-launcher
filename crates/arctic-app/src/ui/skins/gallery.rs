//! The community skin gallery inside the Cosmetics tab: browse, wear with a
//! click (it's added to your library too), add or report from the
//! right-click menu; and share your own skins.

use std::collections::HashMap;

use arctic_core::cosmetics::{GalleryItem, GallerySort};
use eframe::egui::{self, RichText, vec2};

use crate::app::ArcticApp;
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
        let can_wear = self.arctic_state().is_some() && !self.skins.gallery.busy;
        let trying = match &self.skins.trying.skin {
            Some(Some((key, _))) => Some(key.clone()),
            _ => None,
        };
        let mut wear: Option<GalleryItem> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for item in &items {
                let key = format!("gal:{}", item.texture);
                let worn = trying.as_deref() == Some(key.as_str());
                let tile = self
                    .skin_tile(ui, &key, &item.name, Some(item.variant()), worn)
                    .on_hover_text(format!(
                        "by {} · used {} times",
                        item.author, item.downloads
                    ));
                if tile.clicked() && can_wear && !worn {
                    wear = Some(item.clone());
                }
                tile.context_menu(|ui| self.gallery_menu(ui, item));
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Click a skin to wear it (it's saved to your library). Right-click for more.",
            )
            .small()
            .color(p.muted),
        );
        if let Some(item) = wear {
            // Shown at once; worn for real once it's in the library.
            let key = format!("gal:{}", item.texture);
            self.skins.trying.skin = Some(Some((key, item.variant())));
            self.skins.gallery.busy = true;
            self.tasks.gallery_take(item, true);
        }
    }

    fn run_gallery_search(&mut self) {
        let g = &mut self.skins.gallery;
        g.request += 1;
        g.loading = true;
        self.tasks
            .gallery_search(g.request, g.sort, g.query.trim().to_owned());
    }

    /// A gallery skin's right-click menu.
    fn gallery_menu(&mut self, ui: &mut egui::Ui, item: &GalleryItem) {
        let busy = self.skins.gallery.busy;
        if ui
            .add_enabled(!busy, egui::Button::new("Add to library"))
            .clicked()
        {
            self.skins.gallery.busy = true;
            self.tasks.gallery_take(item.clone(), false);
            ui.close();
        }
        if let Some(account) = self.accounts.active().cloned()
            && ui.button("Report this skin").clicked()
        {
            self.tasks.gallery_report(account, item.id.clone());
            ui.close();
        }
    }

    /// "Share to gallery" for a library skin; true when clicked.
    pub(super) fn share_button(
        &mut self,
        ui: &mut egui::Ui,
        entry: &arctic_core::skins::SkinEntry,
    ) -> bool {
        let Some(account) = self.accounts.active().cloned() else {
            return false;
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
            return true;
        }
        false
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
                        let entry = self.add_skin(&name, &png, Some(variant));
                        if wear && let Some(entry) = entry {
                            self.wear_skin(&entry);
                        }
                    }
                    Err(e) => {
                        self.skins.trying.skin = None;
                        self.toasts.push(Kind::Error, "Could not get skin", e);
                    }
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
