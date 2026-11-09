//! Shared Modrinth preview. A modal leaves the browser and its scroll state mounted.
use std::sync::mpsc::{self, Receiver};

use arctic_core::instances::Instance;
use arctic_core::mods::{self, ProjectDetail, ProjectHit, packs::PackKind};
use eframe::egui::{self, RichText};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use pulldown_cmark::{Event, Parser, Tag};

use super::{IconState, browse::compact, mods_page::mod_icon};
use crate::{app::ArcticApp, art::icons::Icon, widgets};

type DetailResult = Result<ProjectDetail, String>;

pub struct Preview {
    hit: ProjectHit,
    kind: Option<PackKind>,
    receiver: Receiver<DetailResult>,
    detail: Option<DetailResult>,
    markdown: CommonMarkCache,
    enlarged: Option<String>,
}

fn fetch(id: String, ctx: egui::Context) -> Receiver<DetailResult> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result = mods::project_detail(&id).map_err(|e| e.to_string());
        let _ = send.send(result);
        ctx.request_repaint();
    });
    receive
}

/// No web view is used. HTML is converted to inert text; markdown links only
/// open HTTP(S). Inline images are represented by placeholders: gallery
/// images use our bounded, lazy, cached loader instead of arbitrary file URIs.
fn prepare_description(body: &str, cache: &mut CommonMarkCache) -> String {
    let mut images = Vec::new();
    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) if !safe_url(&dest_url) => {
                cache.add_link_hook(dest_url.to_string());
            }
            Event::Start(Tag::Image { .. }) => images.push(range),
            _ => {}
        }
    }
    let mut body = body.to_owned();
    for range in images.into_iter().rev() {
        body.replace_range(range, " [Image — see project gallery] ");
    }
    body
}

fn safe_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn html_text(html: &str) -> String {
    html2text::config::plain_no_decorate()
        .string_from_read(html.as_bytes(), 100)
        .unwrap_or_default()
}

impl ArcticApp {
    pub(super) fn open_project_preview(
        &mut self,
        ctx: &egui::Context,
        hit: ProjectHit,
        kind: Option<PackKind>,
    ) {
        self.inst.preview = Some(Preview {
            receiver: fetch(hit.project_id.clone(), ctx.clone()),
            hit,
            kind,
            detail: None,
            markdown: CommonMarkCache::default(),
            enlarged: None,
        });
    }

    pub(super) fn project_preview(&mut self, ctx: &egui::Context, instance: &Instance) {
        // Also makes real API previews reproducible with the existing devshot tool.
        if crate::devshot::enabled()
            && self.inst.preview.is_none()
            && let Ok(id) = std::env::var("ARCTIC_DEVSHOT_PROJECT")
        {
            // Consume once, without changing the process environment.
            let key = egui::Id::new("devshot_project_opened");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(key, true));
                self.open_project_preview(
                    ctx,
                    ProjectHit {
                        project_id: id.clone(),
                        slug: id.clone(),
                        title: id,
                        description: String::new(),
                        author: std::env::var("ARCTIC_DEVSHOT_AUTHOR").unwrap_or_default(),
                        downloads: 0,
                        icon_url: None,
                        categories: vec![],
                    },
                    std::env::var("ARCTIC_DEVSHOT_KIND")
                        .ok()
                        .map(|_| PackKind::Shader),
                );
            }
        }
        let Some(mut preview) = self.inst.preview.take() else {
            return;
        };
        if preview.detail.is_none() {
            match preview.receiver.try_recv() {
                Ok(result) => preview.detail = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    preview.detail = Some(Err("Project request stopped. Please retry.".into()))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let p = self.palette();
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut close = escape && preview.enlarged.is_none();
        if escape {
            preview.enlarged = None;
        }
        egui::Modal::new(egui::Id::new(("project_preview", &preview.hit.project_id)))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 100.0).clamp(280.0, 900.0));
                ui.horizontal(|ui| {
                    close |=
                        widgets::button(ui, p, Some(Icon::ChevronLeft), "Back to results", false)
                            .clicked();
                    ui.label(
                        RichText::new("Project preview · Esc to close")
                            .small()
                            .color(p.muted),
                    );
                });
                let detail = preview.detail.as_ref().and_then(|r| r.as_ref().ok());
                let title = detail.map_or(preview.hit.title.as_str(), |d| d.title.as_str());
                ui.horizontal(|ui| {
                    mod_icon(
                        ui,
                        self,
                        detail
                            .and_then(|d| d.icon_url.as_deref())
                            .or(preview.hit.icon_url.as_deref()),
                    );
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 120.0).max(100.0));
                        ui.heading(title);
                        ui.label(
                            RichText::new(format!("by {}", preview.hit.author)).color(p.muted),
                        );
                        ui.label(format!(
                            "{} downloads",
                            compact(detail.map_or(preview.hit.downloads, |d| d.downloads))
                        ));
                    });
                    let busy = match preview.kind {
                        Some(_) => self.inst.packs.installing.contains(&preview.hit.project_id),
                        None => self.inst.installing.contains(&preview.hit.project_id),
                    };
                    let installed = preview.kind.is_none()
                        && self
                            .inst
                            .mod_scans
                            .get(&instance.id)
                            .and_then(|s| s.data.as_ref())
                            .is_some_and(|data| {
                                data.files.iter().any(|f| {
                                    f.tracked
                                        .as_ref()
                                        .is_some_and(|m| m.project_id == preview.hit.project_id)
                                })
                            });
                    if installed {
                        ui.label(RichText::new("Installed").color(p.accent));
                    } else if busy {
                        ui.spinner();
                        ui.label("Installing…");
                    } else if widgets::button(ui, p, Some(Icon::Plus), "Install", true).clicked() {
                        if let Some(kind) = preview.kind {
                            self.inst
                                .packs
                                .installing
                                .insert(preview.hit.project_id.clone());
                            self.tasks.pack_install(
                                instance.clone(),
                                kind,
                                preview.hit.project_id.clone(),
                                self.pack_game(instance),
                            );
                        } else {
                            self.inst.installing.insert(preview.hit.project_id.clone());
                            self.tasks.mod_install(
                                instance.clone(),
                                preview.hit.project_id.clone(),
                                None,
                            );
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(("project_content", &preview.hit.project_id))
                    .max_height((ctx.content_rect().height() - 250.0).max(200.0))
                    .min_scrolled_height((ctx.content_rect().height() - 250.0).max(200.0))
                    .show(ui, |ui| match &preview.detail {
                        None => {
                            ui.spinner();
                            ui.label("Loading project from Modrinth…");
                        }
                        Some(Err(error)) => {
                            ui.colored_label(p.error, error);
                            if ui.button("Retry").clicked() {
                                preview.receiver =
                                    fetch(preview.hit.project_id.clone(), ctx.clone());
                                preview.detail = None;
                            }
                        }
                        Some(Ok(detail)) => {
                            ui.label(RichText::new("Gallery").strong());
                            if detail.gallery.is_empty() {
                                ui.label(
                                    RichText::new("This project has no gallery screenshots yet.")
                                        .color(p.muted),
                                );
                            }
                            for image in &detail.gallery {
                                let url = image.raw_url.as_deref().unwrap_or(&image.url);
                                if self.preview_image(ui, url, 360.0).clicked() {
                                    preview.enlarged = Some(url.to_owned());
                                }
                                if let Some(title) = &image.title {
                                    ui.label(RichText::new(title).strong());
                                }
                                if let Some(description) = &image.description {
                                    ui.label(description);
                                }
                                ui.add_space(8.0);
                            }
                            ui.separator();
                            ui.label(RichText::new("Supported Minecraft versions").strong());
                            ui.label(detail.game_versions.join(", "));
                            ui.label(RichText::new("Loaders / platforms").strong());
                            ui.label(detail.loaders.join(", "));
                            ui.separator();
                            ui.heading("About this project");
                            let body = prepare_description(&detail.body, &mut preview.markdown);
                            let render_html = |ui: &mut egui::Ui, html: &str| {
                                ui.label(html_text(html));
                            };
                            CommonMarkViewer::new()
                                .render_html_fn(Some(&render_html))
                                .show(ui, &mut preview.markdown, &body);
                        }
                    });
            });
        if let Some(url) = preview.enlarged.clone() {
            egui::Modal::new(egui::Id::new("gallery_enlarged"))
                .frame(super::dialog_frame(p))
                .show(ctx, |ui| {
                    ui.set_width((ctx.content_rect().width() - 90.0).max(280.0));
                    if ui.button("Back to preview · Esc").clicked() {
                        preview.enlarged = None;
                    }
                    self.preview_image(ui, &url, (ctx.content_rect().height() - 150.0).max(200.0));
                });
        }
        if !close {
            self.inst.preview = Some(preview);
        }
    }

    fn preview_image(&mut self, ui: &mut egui::Ui, url: &str, height: f32) -> egui::Response {
        let size = egui::vec2(ui.available_width(), height);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        if !safe_url(url) {
            ui.put(rect, egui::Label::new("Image URL unavailable"));
            return response;
        }
        match self.inst.icons.get(url).cloned() {
            Some(IconState::Ready(uri)) => ui
                .put(
                    rect,
                    egui::Image::new(uri)
                        .max_size(size)
                        .corner_radius(8)
                        .sense(egui::Sense::click()),
                )
                .on_hover_text("Click to enlarge"),
            Some(IconState::Failed) => {
                ui.put(rect, egui::Label::new("Screenshot unavailable"));
                response
            }
            state => {
                if state.is_none() {
                    self.inst.icons.insert(url.to_owned(), IconState::Loading);
                    self.tasks.mod_gallery_image(url.to_owned());
                }
                ui.put(rect, egui::Spinner::new());
                response
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_descriptions_cannot_open_files_or_run_scripts() {
        let mut cache = CommonMarkCache::default();
        let body = prepare_description(
            "# Hello\n[bad](javascript:alert) [local](file:///etc/passwd) ![secret](file:///secret)\n**bold**",
            &mut cache,
        );
        assert!(!body.contains("file:///secret"));
        assert!(body.contains("**bold**"));
        assert!(cache.get_link_hook("javascript:alert").is_some());
        assert!(cache.get_link_hook("file:///etc/passwd").is_some());
        let html = html_text("<h1>Hello</h1><script>alert('secret')</script><p>World</p>");
        assert!(html.contains("Hello"));
        assert!(html.contains("World"));
        assert!(!html.contains("secret"));
    }
}
