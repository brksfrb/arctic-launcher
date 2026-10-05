//! Screenshots tab: every instance's screenshots as a grid of thumbnails
//! (made in the background, cached), and a big preview with actions.

use std::collections::HashMap;
use std::path::PathBuf;

use arctic_core::screenshots::{self, Shot};
use eframe::egui::{self, CornerRadius, Id, Modal, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::toasts::Kind;
use crate::widgets;

const THUMB_W: f32 = 240.0;
const THUMB_H: f32 = 135.0;
/// Thumbnails asked for at once (the rest wait their turn).
const MAX_LOADING: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Thumb {
    Loading,
    /// Registered with egui under this URI.
    Ready(String),
    Failed,
}

#[derive(Default)]
pub struct ScreenshotsUi {
    shots: Option<Vec<Shot>>,
    thumbs: HashMap<PathBuf, Thumb>,
    /// Instance id to show, or every instance.
    filter: Option<String>,
    open: Option<Shot>,
    loading: bool,
    /// When the list was last asked for (egui time), to look again now and then.
    asked: f64,
}

/// While the tab is open, new screenshots show up within this many seconds.
const RESCAN_EVERY: f64 = 3.0;

impl ArcticApp {
    pub(crate) fn screenshots_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let now = ui.input(|i| i.time);
        // Shots taken in game appear on their own, without pressing Refresh.
        if !self.shots.loading
            && (self.shots.shots.is_none() || now - self.shots.asked >= RESCAN_EVERY)
        {
            self.shots.loading = true;
            self.shots.asked = now;
            self.tasks.screenshot_list();
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(RESCAN_EVERY));
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                widgets::page_header(ui, p, "Screenshots", "From every instance, newest first.");
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if widgets::button(ui, p, None, "Refresh", false).clicked() {
                    self.shots.loading = true;
                    self.shots.asked = now;
                    self.tasks.screenshot_list();
                }
                self.screenshot_filter(ui);
            });
        });
        let Some(shots) = self.shots.shots.clone() else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Looking for screenshots…").color(p.muted));
            });
            return;
        };
        let shown: Vec<&Shot> = shots
            .iter()
            .filter(|s| {
                self.shots
                    .filter
                    .as_ref()
                    .is_none_or(|f| *f == s.instance_id)
            })
            .collect();
        if shown.is_empty() {
            ui.label(
                RichText::new("No screenshots yet. Press F2 in game to take one.").color(p.muted),
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for shot in shown {
                self.thumbnail(ui, shot);
            }
        });
        self.screenshot_preview(ui.ctx());
    }

    fn screenshot_filter(&mut self, ui: &mut egui::Ui) {
        let mut names: Vec<(String, String)> =
            vec![(self.instance.id.clone(), self.instance.name.clone())];
        names.extend(
            self.custom_instances
                .iter()
                .map(|i| (i.id.clone(), i.name.clone())),
        );
        let current = self
            .shots
            .filter
            .as_ref()
            .and_then(|id| names.iter().find(|(i, _)| i == id))
            .map_or("All instances".to_owned(), |(_, n)| n.clone());
        egui::ComboBox::from_id_salt("shot_filter")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.shots.filter, None, "All instances");
                for (id, name) in names {
                    ui.selectable_value(&mut self.shots.filter, Some(id), name);
                }
            });
    }

    fn thumbnail(&mut self, ui: &mut egui::Ui, shot: &Shot) {
        let response = self.shot_thumb(ui, shot, vec2(THUMB_W, THUMB_H));
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("{} · {}", shot.instance_name, when(shot.taken)));
        if response.clicked() {
            self.shots.open = Some(shot.clone());
        }
    }

    /// A screenshot's thumbnail at `size` (loaded in the background).
    pub(crate) fn shot_thumb(
        &mut self,
        ui: &mut egui::Ui,
        shot: &Shot,
        size: egui::Vec2,
    ) -> egui::Response {
        let p = self.palette();
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let visible = ui.is_rect_visible(rect);
        let state = self.shots.thumbs.get(&shot.path).cloned();
        let loading = self
            .shots
            .thumbs
            .values()
            .filter(|t| **t == Thumb::Loading)
            .count();
        if visible && state.is_none() && loading < MAX_LOADING {
            self.shots.thumbs.insert(shot.path.clone(), Thumb::Loading);
            self.tasks.screenshot_thumb(shot.path.clone());
        }
        let radius = CornerRadius::same(8);
        match state {
            Some(Thumb::Ready(uri)) if visible => {
                egui::Image::new(uri)
                    .corner_radius(radius)
                    .fit_to_exact_size(rect.size())
                    .paint_at(ui, rect);
            }
            _ => {
                ui.painter().rect_filled(rect, radius, p.surface_hover);
            }
        }
        if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                radius,
                egui::Stroke::new(2.0, p.accent),
                egui::StrokeKind::Inside,
            );
        }
        response
    }

    /// The newest screenshots (listing them first if needed).
    pub(crate) fn recent_shots(&mut self, n: usize) -> Option<Vec<Shot>> {
        if self.shots.shots.is_none() && !self.shots.loading {
            self.shots.loading = true;
            self.tasks.screenshot_list();
        }
        self.shots
            .shots
            .as_ref()
            .map(|l| l.iter().take(n).cloned().collect())
    }

    fn screenshot_preview(&mut self, ctx: &egui::Context) {
        let Some(shot) = self.shots.open.clone() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let uri = crate::widgets::file_uri(&shot.path);
        let modal = Modal::new(Id::new("shot_preview"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                // As large as the window allows (room left for the bar below),
                // keeping the shot's shape.
                let room = ctx.content_rect().size() * vec2(0.9, 0.82) - vec2(0.0, 60.0);
                let view_id = Id::new(("shot_view", &shot.path));
                let (mut zoom, mut center): (f32, egui::Vec2) = ui
                    .data(|d| d.get_temp(view_id))
                    .unwrap_or((1.0, vec2(0.5, 0.5)));
                let half = 0.5 / zoom;
                let image = ui.add(
                    egui::Image::new(uri.clone())
                        .fit_to_exact_size(room)
                        .uv(egui::Rect::from_min_max(
                            (center - vec2(half, half)).to_pos2(),
                            (center + vec2(half, half)).to_pos2(),
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .sense(Sense::click_and_drag()),
                );
                zoom_and_pan(ui, &image, &mut zoom, &mut center);
                ui.data_mut(|d| d.insert_temp(view_id, (zoom, center)));
                ui.set_max_width(image.rect.width().max(480.0));
                ui.add_space(8.0);
                let name = shot
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{name} · {} · {}",
                            shot.instance_name,
                            when(shot.taken)
                        ))
                        .color(p.muted),
                    )
                    .truncate(),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, p, None, "Close", false).clicked() {
                            close = true;
                        }
                        if widgets::icon_button(ui, p, Icon::Trash, "Delete (moves it to .trash)")
                            .clicked()
                        {
                            match screenshots::trash(&shot) {
                                Ok(()) => {
                                    if let Some(list) = &mut self.shots.shots {
                                        list.retain(|s| s.path != shot.path);
                                    }
                                    close = true;
                                }
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    "Couldn't delete it",
                                    e.to_string(),
                                ),
                            }
                        }
                        if widgets::icon_button(ui, p, Icon::Folder, "Show in folder").clicked()
                            && let Some(dir) = shot.path.parent()
                        {
                            let _ = open::that_detached(dir);
                        }
                        if widgets::button(ui, p, Some(Icon::Copy), "Copy image", false).clicked() {
                            self.tasks.screenshot_copy(shot.path.clone());
                        }
                        if widgets::button(ui, p, Some(Icon::External), "Open", true).clicked() {
                            let _ = open::that_detached(&shot.path);
                        }
                        self.send_shot_menu(ui, &shot.path);
                    });
                });
            });
        if close || modal.should_close() {
            self.shots.open = None;
        }
    }

    pub(crate) fn on_screenshot_list(&mut self, shots: Vec<Shot>) {
        self.shots.loading = false;
        // Debug and developer builds: ARCTIC_DEVSHOT_PAGE=shot opens the newest (screenshots).
        if crate::devshot::enabled()
            && std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() == Ok("shot")
            && self.shots.shots.is_none()
        {
            self.shots.open = shots.first().cloned();
        }
        self.shots.shots = Some(shots);
    }

    pub(crate) fn on_screenshot_thumb(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        png: Result<Vec<u8>, String>,
    ) {
        let state = match png {
            Ok(bytes) => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                path.hash(&mut h);
                let uri = format!("bytes://shot/{:016x}.png", h.finish());
                ctx.include_bytes(uri.clone(), bytes);
                Thumb::Ready(uri)
            }
            Err(_) => Thumb::Failed,
        };
        self.shots.thumbs.insert(path, state);
    }

    pub(crate) fn on_screenshot_copied(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.toasts.push(Kind::Success, "Copied the screenshot", ""),
            Err(e) => self.toasts.push(Kind::Error, "Couldn't copy it", e),
        }
    }
}

/// Most the picture can be magnified.
const MAX_ZOOM: f32 = 12.0;

/// Scroll over the picture to zoom into the spot under the pointer, drag to
/// move around, double-click to see all of it again. `center` is the middle
/// of what's shown, as a fraction of the whole picture.
fn zoom_and_pan(ui: &egui::Ui, image: &egui::Response, zoom: &mut f32, center: &mut egui::Vec2) {
    if image.double_clicked() {
        *zoom = 1.0;
        *center = vec2(0.5, 0.5);
        return;
    }
    let scroll = if image.hovered() {
        ui.input(|i| i.smooth_scroll_delta.y)
    } else {
        0.0
    };
    if scroll != 0.0
        && let Some(pointer) = image.hover_pos()
    {
        // The spot under the pointer stays where it is.
        let rel = (pointer - image.rect.min) / image.rect.size() - vec2(0.5, 0.5);
        let under = *center + rel / *zoom;
        *zoom = (*zoom * (scroll * 0.004).exp()).clamp(1.0, MAX_ZOOM);
        *center = under - rel / *zoom;
    }
    if image.dragged() && *zoom > 1.0 {
        *center -= image.drag_delta() / image.rect.size() / *zoom;
    }
    let half = 0.5 / *zoom;
    *center = center.clamp(vec2(half, half), vec2(1.0 - half, 1.0 - half));
}

/// "5 min ago", "yesterday", or a date.
fn when(t: std::time::SystemTime) -> String {
    let secs = t.elapsed().map_or(0, |d| d.as_secs());
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

impl ArcticApp {
    /// "Send to…": the screenshot to a friend's chat.
    fn send_shot_menu(&mut self, ui: &mut egui::Ui, path: &std::path::Path) {
        let friends: Vec<(String, String)> = self
            .friends_overview()
            .map(|o| {
                o.friends
                    .iter()
                    .map(|f| (f.id.clone(), f.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if friends.is_empty() {
            return;
        }
        let mut picked = None;
        egui::ComboBox::from_id_salt("send_shot")
            .selected_text("Send to…")
            .show_ui(ui, |ui| {
                for (id, name) in &friends {
                    if ui.selectable_label(false, name).clicked() {
                        picked = Some((id.clone(), name.clone()));
                    }
                }
            });
        if let Some((id, name)) = picked
            && let Some(account) = self.accounts.active().cloned()
        {
            self.tasks.chat_send_image(account, id, path.to_path_buf());
            self.toasts
                .push(Kind::Info, format!("Sending to {name}…"), "");
        }
    }
}
