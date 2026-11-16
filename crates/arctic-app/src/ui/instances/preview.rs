//! Shared Modrinth preview. A modal leaves the browser and its scroll state mounted.
use std::{
    collections::HashMap,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};

use arctic_core::instances::Instance;
use arctic_core::mods::discovery::{ShaderRequirement, VersionInfo};
use arctic_core::mods::{self, ProjectDetail, ProjectHit, ProjectType, packs::PackKind};
use eframe::egui::{self, RichText};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_commonmark_backend::pulldown::parser_options;
use pulldown_cmark::{Event, Parser, Tag};

use super::{IconState, browse::compact, mods_page::mod_icon};
use crate::{app::ArcticApp, art::icons::Icon, widgets};
use crate::{t, tf};
type DetailResult = Result<ProjectDetail, String>;
type VersionsResult = Result<Vec<VersionInfo>, String>;
/// Versions listed on a project page; the rest are on Modrinth.
const VERSIONS_SHOWN: usize = 30;

pub struct Preview {
    instance_id: String,
    hit: ProjectHit,
    kind: Option<PackKind>,
    /// A modpack: installs as an instance of its own.
    modpack: bool,
    receiver: Receiver<DetailResult>,
    detail: Option<DetailResult>,
    versions_rx: Receiver<VersionsResult>,
    versions: Option<VersionsResult>,
    markdown: CommonMarkCache,
    enlarged: Option<String>,
    context: egui::Context,
    gallery: HashMap<String, IconState>,
    body: String,
    html: Arc<HashMap<String, String>>,
}

impl Drop for Preview {
    fn drop(&mut self) {
        for state in self.gallery.values() {
            if let IconState::Ready(uri) = state {
                self.context.forget_image(uri);
            }
        }
    }
}

impl Preview {
    pub(super) fn gallery_result(&mut self, url: String, result: Result<Vec<u8>, String>) {
        if !matches!(self.gallery.get(&url), Some(IconState::Loading)) {
            return;
        }
        let state = match result {
            Ok(bytes) => {
                let uri = super::icon_texture_uri(&url).replacen("modicon/", "gallery/", 1);
                self.context.include_bytes(uri.clone(), bytes);
                IconState::Ready(uri)
            }
            Err(_) => IconState::Failed,
        };
        self.gallery.insert(url, state);
    }

    fn for_instance(self, instance_id: &str) -> Option<Self> {
        (self.instance_id == instance_id).then_some(self)
    }
}

fn fetch_versions(id: String, ctx: egui::Context) -> Receiver<VersionsResult> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result = mods::discovery::project_version_list(&id).map_err(|e| e.to_string());
        let _ = send.send(result);
        ctx.request_repaint();
    });
    receive
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
    let mut images: Vec<std::ops::Range<usize>> = Vec::new();
    // Share the viewer's extension options so footnotes and definition lists
    // cannot hide active images or links from sanitization. The viewer below
    // leaves its optional math and heading-attribute parsing disabled.
    for (event, range) in Parser::new_ext(body, parser_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) if !safe_url(&dest_url) => {
                cache.add_link_hook(dest_url.to_string());
            }
            // Start events visit outer images first. Keep only disjoint ranges
            // so replacing a nested image cannot invalidate its parent's range.
            Event::Start(Tag::Image { .. })
                if images.last().is_none_or(|outer| range.start >= outer.end) =>
            {
                images.push(range);
            }
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

// Match the viewer's HTML-block aggregation, including blocks split into
// multiple parser events. Inline HTML is rendered as text by the viewer.
fn prepare_html(body: &str) -> HashMap<String, String> {
    let mut prepared = HashMap::new();
    let mut block = String::new();
    for event in Parser::new_ext(body, parser_options()) {
        match event {
            Event::Html(html) => block.push_str(&html),
            Event::End(pulldown_cmark::TagEnd::HtmlBlock) => {
                prepared.insert(block.clone(), html_text(&block));
                block.clear();
            }
            _ => {}
        }
    }
    prepared
}

impl ArcticApp {
    pub(super) fn open_project_preview(
        &mut self,
        ctx: &egui::Context,
        hit: ProjectHit,
        kind: Option<PackKind>,
    ) {
        let Some(instance_id) = self.inst.open.clone() else {
            return;
        };
        self.open_project_preview_in(ctx, instance_id, hit, kind, false);
    }

    /// The project page, installing into `instance_id` (`modpack`: as a new instance instead).
    pub(super) fn open_project_preview_in(
        &mut self,
        ctx: &egui::Context,
        instance_id: String,
        hit: ProjectHit,
        kind: Option<PackKind>,
        modpack: bool,
    ) {
        self.inst.preview = Some(Preview {
            instance_id,
            receiver: fetch(hit.project_id.clone(), ctx.clone()),
            versions_rx: fetch_versions(hit.project_id.clone(), ctx.clone()),
            versions: None,
            modpack,
            hit,
            kind,
            detail: None,
            markdown: CommonMarkCache::default(),
            enlarged: None,
            context: ctx.clone(),
            gallery: HashMap::new(),
            body: String::new(),
            html: Arc::default(),
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
                        follows: 0,
                        icon_url: None,
                        categories: vec![],
                        game_versions: vec![],
                    },
                    std::env::var("ARCTIC_DEVSHOT_KIND")
                        .ok()
                        .map(|_| PackKind::Shader),
                );
            }
        }
        // A preview belongs to the instance that opened it, even when navigation
        // leaves it in global UI state. Discard it before rendering install actions.
        let Some(mut preview) = self
            .inst
            .preview
            .take()
            .and_then(|preview| preview.for_instance(&instance.id))
        else {
            return;
        };
        if preview.detail.is_none() {
            match preview.receiver.try_recv() {
                Ok(result) => {
                    if let Ok(detail) = &result {
                        preview.body = prepare_description(&detail.body, &mut preview.markdown);
                        preview.html = Arc::new(prepare_html(&preview.body));
                    }
                    preview.detail = Some(result);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    preview.detail = Some(Err(
                        t!("instances.project_request_stopped_please_retry").into()
                    ))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if preview.versions.is_none() {
            match preview.versions_rx.try_recv() {
                Ok(result) => preview.versions = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    preview.versions = Some(Err(t!(
                        "instances.project_request_stopped_please_retry"
                    )
                    .into()))
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
                    close |= widgets::button(
                        ui,
                        p,
                        Some(Icon::ChevronLeft),
                        t!("instances.back_to_results"),
                        false,
                    )
                    .clicked();
                    ui.label(
                        RichText::new(t!("instances.project_preview_esc_to_close"))
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
                            RichText::new(tf!("common.by_author", author = preview.hit.author))
                                .color(p.muted),
                        );
                        ui.label(tf!(
                            "instances.downloads_2",
                            value = compact(detail.map_or(preview.hit.downloads, |d| d.downloads))
                        ));
                        ui.label(
                            RichText::new(tf!(
                                "instances.hub.follows",
                                compact =
                                    compact(detail.map_or(preview.hit.follows, |d| d.followers))
                            ))
                            .color(p.muted),
                        );
                        if preview.kind == Some(PackKind::Shader)
                            && let Some(note) = self.shader_note(instance)
                        {
                            ui.label(RichText::new(note).small().color(p.accent));
                        }
                        if preview.kind.is_some()
                            && preview.hit.is_incompatible(&self.pack_game(instance))
                        {
                            ui.label(RichText::new(t!("instances.incompatible")).color(p.error))
                                .on_hover_text(tf!(
                                    "instances.no_file_is_tagged_for_minecraft_you_can_2",
                                    pack_game = self.pack_game(instance)
                                ));
                        }
                    });
                    let busy = if preview.modpack {
                        self.inst.pack_installing.is_some()
                    } else {
                        match preview.kind {
                            Some(_) => self.inst.packs.installing.contains(&preview.hit.project_id),
                            None => self.inst.installing.contains(&preview.hit.project_id),
                        }
                    };
                    let vanilla_mods = !preview.modpack
                        && preview.kind.is_none()
                        && instance.loader.kind().is_none();
                    let installed = preview.kind.is_none()
                        && !preview.modpack
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
                        ui.label(RichText::new(t!("instances.installed")).color(p.accent));
                    } else if busy {
                        ui.spinner();
                        ui.label(t!("instances.installing"));
                    } else {
                        let button = ui
                            .add_enabled_ui(!vanilla_mods, |ui| {
                                widgets::button(
                                    ui,
                                    p,
                                    Some(Icon::Plus),
                                    t!("instances.install"),
                                    true,
                                )
                            })
                            .inner;
                        let clicked = if vanilla_mods {
                            button.on_disabled_hover_text(t!("instances.hub.vanilla_has_no_mods"));
                            false
                        } else {
                            button.clicked()
                        };
                        if clicked {
                            if preview.modpack {
                                self.inst.pack_installing = Some(preview.hit.project_id.clone());
                                self.inst.install_progress = None;
                                self.tasks
                                    .install_modpack(preview.hit.project_id.clone(), None);
                            } else if let Some(kind) = preview.kind {
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
                    }
                    if preview.modpack
                        && widgets::button(ui, p, None, t!("instances.view_mods"), false).clicked()
                    {
                        self.open_pack_preview(&preview.hit.project_id, &preview.hit.title, None);
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
                            ui.label(t!("instances.loading_project_from_modrinth"));
                        }
                        Some(Err(error)) => {
                            ui.colored_label(p.error, error);
                            if ui.button(t!("instances.retry")).clicked() {
                                preview.receiver =
                                    fetch(preview.hit.project_id.clone(), ctx.clone());
                                preview.detail = None;
                            }
                        }
                        Some(Ok(detail)) => {
                            ui.label(RichText::new(t!("instances.gallery")).strong());
                            if detail.gallery.is_empty() {
                                ui.label(
                                    RichText::new(t!(
                                        "instances.this_project_has_no_gallery_screenshots_yet"
                                    ))
                                    .color(p.muted),
                                );
                            }
                            for image in &detail.gallery {
                                let url = image.raw_url.as_deref().unwrap_or(&image.url);
                                if self
                                    .preview_image(ui, &mut preview.gallery, url, 360.0)
                                    .clicked()
                                {
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
                            ui.label(
                                RichText::new(t!("instances.supported_minecraft_versions"))
                                    .strong(),
                            );
                            ui.label(detail.game_versions.join(", "));
                            ui.label(RichText::new(t!("instances.loaders_platforms")).strong());
                            ui.label(detail.loaders.join(", "));
                            if !detail.categories.is_empty() {
                                ui.label(RichText::new(t!("instances.hub.categories")).strong());
                                ui.label(
                                    detail
                                        .categories
                                        .iter()
                                        .map(|c| mods::discovery::pretty_name(c))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                );
                            }
                            ui.separator();
                            self.project_version_list(ui, &preview, instance);
                            ui.separator();
                            ui.heading(t!("instances.about_this_project"));
                            let prepared_html = Arc::clone(&preview.html);
                            let render_html = move |ui: &mut egui::Ui, html: &str| {
                                ui.label(
                                    prepared_html
                                        .get(html)
                                        .map(String::as_str)
                                        .unwrap_or_default(),
                                );
                            };
                            CommonMarkViewer::new()
                                .render_html_fn(Some(&render_html))
                                .show(ui, &mut preview.markdown, &preview.body);
                        }
                    });
            });
        if let Some(url) = preview.enlarged.clone() {
            egui::Modal::new(egui::Id::new("gallery_enlarged"))
                .frame(super::dialog_frame(p))
                .show(ctx, |ui| {
                    ui.set_width((ctx.content_rect().width() - 90.0).max(280.0));
                    if ui.button(t!("instances.back_to_preview_esc")).clicked() {
                        preview.enlarged = None;
                    }
                    self.preview_image(
                        ui,
                        &mut preview.gallery,
                        &url,
                        (ctx.content_rect().height() - 150.0).max(200.0),
                    );
                });
        }
        if !close {
            self.inst.preview = Some(preview);
        }
    }

    /// The project's versions, newest first, those that fit the target instance marked.
    fn project_version_list(&mut self, ui: &mut egui::Ui, preview: &Preview, instance: &Instance) {
        let p = self.palette();
        ui.label(RichText::new(t!("instances.hub.versions")).strong());
        let versions = match &preview.versions {
            None => {
                ui.spinner();
                return;
            }
            Some(Err(e)) => {
                ui.colored_label(p.error, e);
                return;
            }
            Some(Ok(v)) if v.is_empty() => {
                ui.label(RichText::new(t!("instances.hub.no_versions")).color(p.muted));
                return;
            }
            Some(Ok(v)) => v,
        };
        let kind = match (preview.modpack, preview.kind) {
            (true, _) => ProjectType::Modpack,
            (_, Some(PackKind::Resource)) => ProjectType::ResourcePack,
            (_, Some(PackKind::Shader)) => ProjectType::Shader,
            _ => ProjectType::Mod,
        };
        let (loader, game) = if preview.modpack {
            (None, String::new())
        } else {
            (instance.loader.kind(), self.pack_game(instance))
        };
        let busy = self.inst.pack_installing.is_some();
        let mut chosen = None;
        for v in versions.iter().take(VERSIONS_SHOWN) {
            let fits = !preview.modpack && v.fits(kind, loader, &game);
            ui.horizontal(|ui| {
                if fits {
                    ui.label(RichText::new("✔").color(p.accent))
                        .on_hover_text(tf!("instances.hub.fits", name = instance.name));
                }
                ui.label(RichText::new(&v.number).strong().color(p.text));
                let mut parts = vec![
                    v.version_type.clone(),
                    v.game_versions
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                        + if v.game_versions.len() > 3 { "…" } else { "" },
                    v.loaders.join(", "),
                    v.date.chars().take(10).collect(),
                ];
                if kind == ProjectType::Shader {
                    match v.shader_requirement() {
                        Some(ShaderRequirement::Iris) => parts.push("Iris".into()),
                        Some(ShaderRequirement::OptiFineFormat) => parts.push("OptiFine".into()),
                        None => {}
                    }
                }
                ui.label(RichText::new(parts.join(" · ")).small().color(p.muted));
                if preview.modpack {
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
                    });
                }
            });
        }
        if versions.len() > VERSIONS_SHOWN {
            ui.label(
                RichText::new(tf!(
                    "instances.hub.more_versions",
                    n = versions.len() - VERSIONS_SHOWN
                ))
                .small()
                .color(p.muted),
            );
        }
        if let Some(version_id) = chosen {
            self.inst.pack_installing = Some(preview.hit.project_id.clone());
            self.inst.install_progress = None;
            self.tasks
                .install_modpack(preview.hit.project_id.clone(), Some(version_id));
        }
    }

    fn preview_image(
        &mut self,
        ui: &mut egui::Ui,
        gallery: &mut HashMap<String, IconState>,
        url: &str,
        height: f32,
    ) -> egui::Response {
        let size = egui::vec2(ui.available_width(), height);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        if !safe_url(url) {
            ui.put(rect, egui::Label::new("Image URL unavailable"));
            return response;
        }
        match gallery.get(url).cloned() {
            Some(IconState::Ready(uri)) => ui
                .put(
                    rect,
                    egui::Image::new(uri)
                        .max_size(size)
                        .corner_radius(8)
                        .sense(egui::Sense::click()),
                )
                .on_hover_text(t!("instances.click_to_enlarge")),
            Some(IconState::Failed) => {
                ui.put(rect, egui::Label::new("Screenshot unavailable"));
                response
            }
            state => {
                if state.is_none() {
                    gallery.insert(url.to_owned(), IconState::Loading);
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

    fn preview_fixture() -> Preview {
        let (_, receiver) = mpsc::channel();
        Preview {
            instance_id: "instance-a".into(),
            hit: ProjectHit {
                project_id: "project".into(),
                slug: "project".into(),
                title: "Project".into(),
                description: String::new(),
                author: String::new(),
                downloads: 0,
                follows: 0,
                icon_url: None,
                categories: vec![],
                game_versions: vec![],
            },
            kind: None,
            modpack: false,
            versions_rx: mpsc::channel().1,
            versions: None,
            receiver,
            detail: None,
            markdown: CommonMarkCache::default(),
            enlarged: None,
            context: egui::Context::default(),
            gallery: HashMap::new(),
            body: String::new(),
            html: Arc::default(),
        }
    }

    #[test]
    fn gallery_cache_is_released_on_drop_and_reopening_can_reload() {
        let mut preview = preview_fixture();
        let ctx = preview.context.clone();
        let url = "https://example.com/screenshot.png".to_owned();
        preview.gallery.insert(url.clone(), IconState::Loading);
        preview.gallery_result(url.clone(), Ok(vec![1, 2, 3]));
        let IconState::Ready(uri) = preview.gallery.get(&url).unwrap() else {
            panic!("image not ready")
        };
        let uri = uri.clone();
        assert!(ctx.try_load_bytes(&uri).is_ok());
        drop(preview);
        assert!(ctx.try_load_bytes(&uri).is_err());
        let mut reopened = preview_fixture();
        reopened.gallery_result(url.clone(), Ok(vec![4, 5, 6]));
        assert!(!reopened.gallery.contains_key(&url));
        assert!(ctx.try_load_bytes(&uri).is_err());
        reopened.gallery.insert(url.clone(), IconState::Loading);
        reopened.gallery_result(url, Ok(vec![4, 5, 6]));
        assert!(reopened.context.try_load_bytes(&uri).is_ok());
    }

    #[test]
    fn back_drops_preview_and_gallery_before_leaving_instance() {
        let mut preview = preview_fixture();
        let ctx = preview.context.clone();
        let url = "https://example.com/screenshot.png".to_owned();
        preview.gallery.insert(url.clone(), IconState::Loading);
        preview.gallery_result(url.clone(), Ok(vec![1, 2, 3]));
        let IconState::Ready(uri) = preview.gallery.get(&url).unwrap() else {
            panic!("image not ready")
        };
        let uri = uri.clone();
        let mut inst = super::super::InstancesUi {
            open: Some("instance-a".into()),
            preview: Some(preview),
            ..Default::default()
        };

        assert!(ctx.try_load_bytes(&uri).is_ok());
        assert!(inst.go_back());
        assert!(inst.preview.is_none());
        assert!(ctx.try_load_bytes(&uri).is_err());
        assert_eq!(inst.open.as_deref(), Some("instance-a"));
        assert!(inst.go_back());
        assert!(inst.open.is_none());
        inst.open = Some("instance-a".into());
        assert!(inst.preview.is_none());
    }

    #[test]
    fn preview_cannot_move_to_another_instance_after_navigation() {
        let preview = preview_fixture().for_instance("instance-a").unwrap();
        assert!(preview.for_instance("instance-b").is_none());
    }

    #[test]
    fn html_blocks_are_prepared_as_the_viewer_aggregates_them() {
        let html = "<div>\n<p>Hello</p>\n<p>World</p>\n</div>\n";
        let prepared = prepare_html(html);
        let text = prepared.get(html).unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn nested_images_replace_only_disjoint_outermost_ranges() {
        let mut cache = CommonMarkCache::default();
        let placeholder = " [Image — see project gallery] ";
        for image in [
            "![![x](https://example.com/verylongimagepath)](b)",
            "![![![x](https://example.com/verylongimagepath)](b)](c)",
            "![before ![x](https://example.com/verylongimagepath) after](b)",
        ] {
            let body = format!("é {image}![next](https://example.com/next) end");
            assert_eq!(
                prepare_description(&body, &mut cache),
                format!("é {placeholder}{placeholder} end"),
            );
        }
    }

    #[test]
    fn extension_content_cannot_bypass_image_and_link_restrictions() {
        for body in [
            "Reference[^note]\n\n[^note]: footnote\n\n    ![secret](file:///secret) [open](customprotocol:payload)",
            "Term\n: ![secret](file:///secret) [open](customprotocol:payload)",
            "| Content |\n| --- |\n| ![secret](file:///secret) [open](customprotocol:payload) |",
            "- [ ] ~~![secret](file:///secret) [open](customprotocol:payload)~~",
        ] {
            let mut cache = CommonMarkCache::default();
            let prepared = prepare_description(body, &mut cache);
            assert!(prepared.contains("[Image — see project gallery]"));
            assert!(!prepared.contains("file:///secret"));
            assert!(cache.get_link_hook("customprotocol:payload").is_some());
            for event in Parser::new_ext(&prepared, parser_options()) {
                match event {
                    Event::Start(Tag::Image { .. }) => panic!("image survived: {prepared}"),
                    Event::Start(Tag::Link { dest_url, .. }) if !safe_url(&dest_url) => {
                        assert!(cache.get_link_hook(&dest_url).is_some());
                    }
                    _ => {}
                }
            }
        }
    }

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
