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
use crate::{t, tf};
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

/// The open picture's file and what its header says: its size, or why it can't be shown.
type OpenCheck = (PathBuf, Result<(u32, u32), String>);

/// Whether the open screenshot can be drawn yet.
enum Load {
    Ready,
    Pending,
    Failed(String),
}

/// A picture that still hasn't loaded after this long is given up on (an error instead of waiting
/// for ever).
const LOAD_GIVE_UP_SECS: f64 = 20.0;

#[derive(Default)]
pub struct ScreenshotsUi {
    /// When the picture was clicked open (egui time).
    opened_at: f64,
    shots: Option<Vec<Shot>>,
    thumbs: HashMap<PathBuf, Thumb>,
    /// Instance id to show, or every instance.
    filter: Option<String>,
    open: Option<Shot>,
    /// Whether the open picture can be drawn at all, worked out once per opening.
    open_check: Option<OpenCheck>,
    loading: bool,
    dev_opened: bool,
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
        crate::widgets::row(ui, |ui| {
            ui.vertical(|ui| {
                widgets::page_header(
                    ui,
                    p,
                    t!("settings.screenshots"),
                    t!("settings.from_every_instance_newest_first"),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if widgets::button(ui, p, None, t!("settings.refresh"), false).clicked() {
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
                ui.label(RichText::new(t!("settings.looking_for_screenshots")).color(p.muted));
            });
            // An open preview (or its dim backdrop) must never outlive the page drawing it.
            self.screenshot_preview(ui.ctx());
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
                RichText::new(t!("settings.no_screenshots_yet_press_f2_in_game_to_take"))
                    .color(p.muted),
            );
            self.screenshot_preview(ui.ctx());
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for shot in shown {
                self.thumbnail(ui, shot);
            }
            self.devshot_open_viewer(ui);
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
            .map_or(t!("settings.all_instances").to_owned(), |(_, n)| n.clone());
        egui::ComboBox::from_id_salt("shot_filter")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.shots.filter, None, t!("settings.all_instances"));
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
            self.shots.opened_at = ui.input(|i| i.time);
            self.shots.open_check = None;
        }
    }

    /// Dev builds, `ARCTIC_DEVSHOT_FRAMES`: opens the first shot's viewer the way a click does, once
    /// its thumbnail is drawn, and starts recording every frame.
    fn devshot_open_viewer(&mut self, ui: &egui::Ui) {
        if !crate::devshot::enabled() || !self.devshot.wants_frames() || self.shots.dev_opened {
            return;
        }
        let Some(shot) = self.shots.shots.as_ref().and_then(|l| l.first()).cloned() else {
            return;
        };
        let Some(Thumb::Ready(_)) = self.shots.thumbs.get(&shot.path) else {
            return;
        };
        let now = ui.input(|i| i.time);
        if now < 3.0 {
            return;
        }
        self.shots.dev_opened = true;
        self.shots.open = Some(shot);
        self.shots.opened_at = now;
        self.shots.open_check = None;
        self.devshot.start_recording();
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

    /// Whether the open screenshot's picture is ready, still loading, or can't be shown (and
    /// why). Checked up front for what the GPU can't take (egui panics on a texture bigger than
    /// the card's limit) and for files that aren't pictures at all.
    fn preview_load(
        &mut self,
        ctx: &egui::Context,
        path: &std::path::Path,
        uri: &str,
        room: egui::Vec2,
    ) -> Load {
        if self
            .shots
            .open_check
            .as_ref()
            .is_none_or(|(p, _)| p != path)
        {
            let limit = ctx.input(|i| i.max_texture_side) as u32;
            let verdict = match image::image_dimensions(path) {
                Ok((w, h)) if w > limit || h > limit => Err(tf!(
                    "settings.it_is_bigger_than_this_graphics_card_can",
                    w = w,
                    h = h,
                    limit = limit
                )),
                Ok(size) => Ok(size),
                Err(e) => Err(tf!("settings.the_file_cant_be_read_as_a_picture", e = e)),
            };
            self.shots.open_check = Some((path.to_path_buf(), verdict));
        }
        if let Some((_, Err(why))) = &self.shots.open_check {
            return Load::Failed(why.clone());
        }
        match egui::Image::new(uri.to_owned()).load_for_size(ctx, room) {
            Ok(egui::load::TexturePoll::Ready { .. }) => Load::Ready,
            Ok(egui::load::TexturePoll::Pending { .. }) => Load::Pending,
            Err(e) => Load::Failed(e.to_string()),
        }
    }

    /// How big the picture is drawn in `room`: its own shape (from the file's header, so it is the
    /// same before and after the picture has loaded), as large as fits. The dialog is sized from
    /// this, never from `room` itself: an unloaded picture must not leave a window-sized empty frame.
    fn picture_size(&self, room: egui::Vec2) -> egui::Vec2 {
        let aspect = match &self.shots.open_check {
            Some((_, Ok((w, h)))) if *w > 0 && *h > 0 => *w as f32 / *h as f32,
            _ => 16.0 / 9.0,
        };
        fit_aspect(room, aspect)
    }

    fn screenshot_preview(&mut self, ctx: &egui::Context) {
        let Some(shot) = self.shots.open.clone() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let uri = crate::widgets::file_uri(&shot.path);
        let room = ctx.content_rect().size() * vec2(0.9, 0.82) - vec2(0.0, 60.0);
        let mut load = self.preview_load(ctx, &shot.path, &uri, room);
        let picture = self.picture_size(room);
        let waited = ctx.input(|i| i.time) - self.shots.opened_at;
        if matches!(load, Load::Pending) && waited > LOAD_GIVE_UP_SECS {
            load = Load::Failed(t!("settings.it_is_taking_too_long_to_load").to_owned());
        }
        self.devshot.frame_note(&format!(
            "  viewer load={value}", // i18n-ignore: diagnostics
            value = match &load {
                Load::Ready => "ready",
                Load::Pending => "pending",
                Load::Failed(_) => "failed",
            }
        ));
        if matches!(load, Load::Pending) {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        // The dialog is drawn from the very first frame, in its final size and place: the picture's
        // shape comes from the file's header, so nothing waits for (or is re-measured after) the
        // decode. Until the picture has loaded its thumbnail fills the same rectangle.
        let placeholder = match self.shots.thumbs.get(&shot.path) {
            Some(Thumb::Ready(uri)) => Some(uri.clone()),
            _ => None,
        };
        let area = Modal::default_area(Id::new("shot_preview")).fade_in(false);
        let modal = Modal::new(Id::new("shot_preview"))
            .area(area)
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                // As large as the window allows (room left for the bar below),
                // keeping the shot's shape.
                let image_width = if let Load::Failed(why) = &load {
                    // Say what went wrong, in a box about the size of a small picture.
                    let size = vec2(room.x.min(520.0), room.y.min(260.0));
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(8), p.surface);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(16.0)), |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space((size.y - 90.0).max(0.0) / 2.0);
                            ui.label(
                                RichText::new(t!("settings.couldnt_show_this_screenshot")).strong(),
                            );
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(why.as_str()).color(p.muted)));
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(t!("settings.open_below_may_still_work"))
                                    .color(p.muted),
                            );
                        });
                    });
                    size.x
                } else {
                    let view_id = Id::new(("shot_view", &shot.path));
                    let (mut zoom, mut center): (f32, egui::Vec2) = ui
                        .data(|d| d.get_temp(view_id))
                        .filter(|(z, c): &(f32, egui::Vec2)| {
                            z.is_finite() && *z >= 1.0 && c.x.is_finite() && c.y.is_finite()
                        })
                        .unwrap_or((1.0, vec2(0.5, 0.5)));
                    let half = 0.5 / zoom;
                    // The rectangle is reserved at its final size whether or not the picture is
                    // here yet.
                    let (rect, image) = ui.allocate_exact_size(picture, Sense::click_and_drag());
                    let image = ui.interact(rect, image.id, Sense::click_and_drag());
                    if matches!(load, Load::Ready) {
                        egui::Image::new(uri.clone())
                            .uv(egui::Rect::from_min_max(
                                (center - vec2(half, half)).to_pos2(),
                                (center + vec2(half, half)).to_pos2(),
                            ))
                            .corner_radius(CornerRadius::same(8))
                            .paint_at(ui, rect);
                    } else {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(8), p.surface);
                        if let Some(thumb) = &placeholder {
                            egui::Image::new(thumb.clone())
                                .corner_radius(CornerRadius::same(8))
                                .paint_at(ui, rect);
                        }
                        ui.place(
                            egui::Rect::from_center_size(rect.center(), vec2(24.0, 24.0)),
                            egui::Spinner::new(),
                        );
                    }
                    zoom_and_pan(ui, &image, &mut zoom, &mut center);
                    ui.data_mut(|d| d.insert_temp(view_id, (zoom, center)));
                    rect.width()
                };
                ui.set_max_width(image_width.max(480.0));
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
                crate::widgets::row(ui, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, p, None, t!("settings.close"), false).clicked() {
                            close = true;
                        }
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Trash,
                            t!("settings.delete_moves_it_to_trash"),
                        )
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
                                    t!("settings.couldnt_delete_it"),
                                    e.to_string(),
                                ),
                            }
                        }
                        if widgets::icon_button(ui, p, Icon::Folder, t!("settings.show_in_folder"))
                            .clicked()
                            && let Some(dir) = shot.path.parent()
                        {
                            let _ = open::that_detached(dir);
                        }
                        if widgets::button(
                            ui,
                            p,
                            Some(Icon::Copy),
                            t!("settings.copy_image"),
                            false,
                        )
                        .clicked()
                        {
                            self.tasks.screenshot_copy(shot.path.clone());
                        }
                        if widgets::button(ui, p, Some(Icon::External), t!("settings.open"), true)
                            .clicked()
                        {
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
            Ok(()) => self
                .toasts
                .push(Kind::Success, t!("settings.copied_the_screenshot"), ""),
            Err(e) => self
                .toasts
                .push(Kind::Error, t!("settings.couldnt_copy_it"), e),
        }
    }
}

/// The biggest rectangle of shape `aspect` (width over height) that fits in `room`.
fn fit_aspect(room: egui::Vec2, aspect: f32) -> egui::Vec2 {
    let room = vec2(room.x.max(1.0), room.y.max(1.0));
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    if room.x / room.y > aspect {
        vec2(room.y * aspect, room.y)
    } else {
        vec2(room.x, room.x / aspect)
    }
}

/// Most the picture can be magnified.
const MAX_ZOOM: f32 = 12.0;

/// Scroll over the picture to zoom into the spot under the pointer, drag to
/// move around, double-click to see all of it again. `center` is the middle
/// of what's shown, as a fraction of the whole picture.
fn zoom_and_pan(ui: &egui::Ui, image: &egui::Response, zoom: &mut f32, center: &mut egui::Vec2) {
    // A picture with no size yet (a zero-sized window) has nothing to zoom or drag: dividing by
    // it would leave NaN in the view, and the picture would never draw again.
    if image.rect.width() < 1.0 || image.rect.height() < 1.0 {
        return;
    }
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
        0..60 => t!("settings.just_now").into(),
        60..3600 => tf!("settings.min_ago", value = secs / 60),
        3600..86_400 => tf!("common.h_ago", n = secs / 3600),
        86_400..172_800 => t!("common.yesterday").into(),
        _ => tf!("settings.days_ago", _400 = secs / 86_400),
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
        // As tall as the buttons beside it, or it sits a few pixels off their line.
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = widgets::BUTTON_HEIGHT;
            widgets::modal_combo("send_shot")
                .selected_text(t!("settings.send_to"))
                .show_ui(ui, |ui| {
                    for (id, name) in &friends {
                        if ui.selectable_label(false, name).clicked() {
                            picked = Some((id.clone(), name.clone()));
                        }
                    }
                });
        });
        if let Some((id, name)) = picked
            && let Some(account) = self.accounts.active().cloned()
        {
            self.tasks.chat_send_image(account, id, path.to_path_buf());
            self.toasts
                .push(Kind::Info, tf!("settings.sending_to", name = name), "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_is_fitted_to_its_own_shape() {
        let room = vec2(1000.0, 500.0);
        assert_eq!(fit_aspect(room, 2.0), vec2(1000.0, 500.0));
        assert_eq!(fit_aspect(room, 1.0), vec2(500.0, 500.0));
        assert_eq!(fit_aspect(room, 4.0), vec2(1000.0, 250.0));
        // Nonsense shapes and rooms never give a zero or NaN size.
        let odd = fit_aspect(vec2(-5.0, 0.0), f32::NAN);
        assert!(odd.x.is_finite() && odd.y.is_finite() && odd.x > 0.0 && odd.y > 0.0);
    }
}
