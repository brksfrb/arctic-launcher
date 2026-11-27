//! Image cards for Discover's resource packs and shaders.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::sync::Arc;

use arctic_core::instances::Instance;
use arctic_core::mods::{ProjectHit, packs::PackKind};
use eframe::egui::{self, RichText, Sense, vec2};

use super::Load;
use super::hub::{project_tags, stats_counts};
use super::mods_page::{ICON, mod_icon, visible_rows_with_height};
use crate::app::ArcticApp;
use crate::tasks::Tasks;
use crate::{t, tf, theme, widgets};

const MIN_CARD_WIDTH: f32 = 250.0;
const CARD_SPACING: f32 = 6.0;

#[derive(Default)]
pub(super) struct HubBanners {
    /// A missing value also covers a pending lookup or a project with no gallery.
    sources: HashMap<String, Option<String>>,
    images: HashMap<String, Load<BannerImage>>,
}

struct BannerImage {
    ctx: egui::Context,
    uri: String,
}

impl Drop for BannerImage {
    fn drop(&mut self) {
        self.ctx.forget_image(&self.uri);
    }
}

impl HubBanners {
    fn source(&mut self, tasks: &Tasks, hit: &ProjectHit) -> Option<&str> {
        let url = self
            .sources
            .entry(hit.project_id.clone())
            .or_insert_with(|| {
                if hit.featured_gallery.is_none() {
                    tasks.hub_banner(hit.project_id.clone());
                }
                hit.featured_gallery.clone()
            });
        if hit.featured_gallery.is_some() && *url != hit.featured_gallery {
            url.clone_from(&hit.featured_gallery);
        }
        url.as_deref()
    }

    pub(super) fn source_result(&mut self, project: String, url: Option<String>) {
        if let Some(cached @ None) = self.sources.get_mut(&project) {
            *cached = url;
        }
    }

    pub(super) fn image_result(
        &mut self,
        ctx: &egui::Context,
        url: &str,
        result: &Result<Arc<[u8]>, String>,
    ) {
        if let Some(image @ Load::Loading) = self.images.get_mut(url) {
            *image = match result {
                Ok(bytes) => {
                    let uri = super::icon_texture_uri(url).replacen("modicon/", "hub-banner/", 1);
                    ctx.include_bytes(uri.clone(), bytes.clone());
                    Load::Ready(BannerImage {
                        ctx: ctx.clone(),
                        uri,
                    })
                }
                Err(e) => Load::Failed(e.clone()),
            };
        }
    }

    fn paint(&mut self, ui: &egui::Ui, tasks: &Tasks, hit: &ProjectHit, rect: egui::Rect) {
        let Some(url) = self.source(tasks, hit).map(str::to_owned) else {
            return;
        };
        match self.images.entry(url) {
            Entry::Vacant(entry) => {
                tasks.mod_gallery_image(entry.key().clone());
                entry.insert(Load::Loading);
            }
            Entry::Occupied(mut entry) => {
                if let Load::Ready(image) = entry.get() {
                    match egui::Image::new(image.uri.as_str()).load_for_size(ui.ctx(), rect.size())
                    {
                        Ok(egui::load::TexturePoll::Ready { texture }) => {
                            let scale = rect.size() / texture.size;
                            let uv = scale / scale.max_elem();
                            egui::Image::new(texture)
                                .uv(egui::Rect::from_center_size(egui::pos2(0.5, 0.5), uv))
                                .corner_radius(8)
                                .paint_at(ui, rect);
                        }
                        Ok(egui::load::TexturePoll::Pending { .. }) => {}
                        Err(e) => {
                            entry.insert(Load::Failed(e.to_string()));
                        }
                    }
                }
            }
        }
    }
}

impl ArcticApp {
    pub(super) fn hub_card_grid(
        &mut self,
        ui: &mut egui::Ui,
        kind: PackKind,
        target: &Instance,
        hits: &[ProjectHit],
        installed: &HashSet<String>,
    ) {
        let p = self.palette();
        let game = self.pack_game(target);
        let shader_note = (kind == PackKind::Shader)
            .then(|| self.shader_note(target))
            .flatten();
        let gap = ui.spacing().item_spacing.x;
        let width = ui.available_width();
        let columns = (((width + gap) / (MIN_CARD_WIDTH + gap)) as usize).max(1);
        let card_width = (width + gap) / columns as f32 - gap;
        let frame = theme::card(p).inner_margin(10.0);
        let margin = frame.total_margin().sum();
        let content_width = (card_width - margin.x).max(1.0);
        let banner_height = content_width * 9.0 / 16.0;
        let body = ui.text_style_height(&egui::TextStyle::Body);
        let small = ui.text_style_height(&egui::TextStyle::Small);
        let header = ICON.max(body + small + CARD_SPACING);
        let mut text_rows = 3;
        let mut sections = 6;
        if hits.iter().any(|hit| hit.is_incompatible(&game)) {
            text_rows += 1;
            sections += 1;
        }
        if shader_note.is_some() {
            text_rows += 2;
            sections += 1;
        }
        let content_height = banner_height
            + header
            + 2.0 * body
            + text_rows as f32 * small
            + widgets::BUTTON_HEIGHT
            + (sections - 1) as f32 * CARD_SPACING;
        let card_height = content_height + margin.y;
        let row_height = card_height + ui.spacing().item_spacing.y;
        let mut visible = HashSet::new();
        visible_rows_with_height(ui, hits.len().div_ceil(columns), row_height, |ui, row| {
            let first = row * columns;
            let end = (first + columns).min(hits.len());
            for (col, hit) in hits[first..end].iter().enumerate() {
                let rect = egui::Rect::from_min_size(
                    ui.max_rect().min + vec2(col as f32 * (card_width + gap), 0.0),
                    vec2(card_width, card_height),
                );
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let mut card = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(&hit.project_id)
                        .max_rect(rect),
                );
                let open = frame
                    .show(&mut card, |ui| {
                        self.hub_image_card(
                            ui,
                            target,
                            kind,
                            hit,
                            shader_note.as_deref(),
                            installed.contains(&hit.project_id),
                        )
                    })
                    .inner;
                if let Some(Some(url)) = self.inst.hub.banners.sources.get(&hit.project_id) {
                    visible.insert(url.clone());
                }
                if open {
                    self.open_discover_project(
                        ui.ctx(),
                        target,
                        hit,
                        kind.project_type(),
                        Some(kind),
                    );
                }
            }
        });
        // Release decoded images and textures as their cards scroll away. A later visit uses disk.
        self.inst
            .hub
            .banners
            .images
            .retain(|url, state| !matches!(state, Load::Ready(_)) || visible.contains(url));
    }

    fn hub_image_card(
        &mut self,
        ui: &mut egui::Ui,
        target: &Instance,
        kind: PackKind,
        hit: &ProjectHit,
        shader_note: Option<&str>,
        installed: bool,
    ) -> bool {
        let p = self.palette();
        let game = self.pack_game(target);
        let width = ui.available_width();
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = CARD_SPACING;
        let (banner, response) =
            ui.allocate_exact_size(vec2(width, width * 9.0 / 16.0), Sense::click());
        let mut open = preview_click(response);
        if ui.is_rect_visible(banner) {
            ui.painter().rect_filled(banner, 8, p.surface_hover);
            self.inst.hub.banners.paint(ui, &self.tasks, hit, banner);
        }
        open |= self.hub_card_heading(ui, hit);
        ui.scope(|ui| {
            ui.set_min_height(2.0 * ui.text_style_height(&egui::TextStyle::Body));
            short_label(ui, &hit.description, egui::TextStyle::Body, p.muted, 2);
        });
        let tags = project_tags(hit);
        if !tags.is_empty() {
            short_label(ui, &tags, egui::TextStyle::Small, p.muted, 1);
        }
        short_label(ui, &stats_counts(hit), egui::TextStyle::Small, p.muted, 2);
        if hit.is_incompatible(&game) {
            short_label(
                ui,
                t!("instances.incompatible"),
                egui::TextStyle::Small,
                p.error,
                1,
            )
            .on_hover_text(tf!(
                "instances.no_file_is_tagged_for_minecraft_you_can",
                game = game
            ));
        }
        if let Some(note) = shader_note {
            short_label(ui, note, egui::TextStyle::Small, p.accent, 2);
        }
        ui.add_space((ui.available_height() - widgets::BUTTON_HEIGHT).max(0.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_min_height(widgets::BUTTON_HEIGHT);
            self.hub_install_button(ui, kind.project_type(), target, hit, installed);
        });
        open
    }

    fn hub_card_heading(&mut self, ui: &mut egui::Ui, hit: &ProjectHit) -> bool {
        let p = self.palette();
        ui.horizontal(|ui| {
            let icon = ui.scope(|ui| mod_icon(ui, self, hit.icon_url.as_deref()));
            let icon_clicked = preview_click(icon.response.interact(Sense::click()));
            // Keep the title clear of the select checkbox pinned to the card's right edge.
            let reserved = ui.spacing().icon_width + 2.0 * ui.spacing().item_spacing.x;
            let title_width = (ui.available_width() - reserved).max(0.0);
            let title_clicked = ui
                .scope(|ui| {
                    ui.set_max_width(title_width);
                    ui.vertical(|ui| {
                        let title =
                            egui::Label::new(RichText::new(&hit.title).strong().color(p.text))
                                .truncate()
                                .sense(Sense::click());
                        let clicked = preview_click(ui.add(title));
                        short_label(
                            ui,
                            &tf!("common.by_author", author = hit.author),
                            egui::TextStyle::Small,
                            p.muted,
                            1,
                        );
                        clicked
                    })
                    .inner
                })
                .inner;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                super::content_install::select_content(ui, &mut self.inst.search.selected, hit);
            });
            icon_clicked || title_clicked
        })
        .inner
    }
}

fn preview_click(response: egui::Response) -> bool {
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(t!("instances.preview_project"))
        .clicked()
}

fn short_label(
    ui: &mut egui::Ui,
    text: &str,
    style: egui::TextStyle,
    color: egui::Color32,
    rows: usize,
) -> egui::Response {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        style.resolve(ui.style()),
        color,
        ui.available_width(),
    );
    job.wrap.max_rows = rows;
    ui.add(egui::Label::new(job).wrap())
}
