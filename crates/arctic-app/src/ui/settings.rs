//! Settings tab. Every change is saved automatically.

use std::path::PathBuf;

use arctic_core::settings::{GameStartAction, MIN_MEMORY_MB, Settings, ThemeMode};
use arctic_core::update::UpdateChannel;
use eframe::egui::{self, CornerRadius, RichText, Sense, Stroke, StrokeKind, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme::{self, Palette};
use crate::widgets;

const MAX_MEMORY_SLIDER_MB: u32 = 32 * 1024;
const MEMORY_STEP_MB: f64 = 512.0;
const MAX_RESOLUTION: u32 = 7680;
const RESOLUTIONS: [(u32, u32); 4] = [(854, 480), (1280, 720), (1600, 900), (1920, 1080)];
const THEMES: [(ThemeMode, &str); 3] = [
    (ThemeMode::Default, "Aurora"),
    (ThemeMode::Dark, "Dark"),
    (ThemeMode::Light, "Light"),
];

impl ArcticApp {
    pub(crate) fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(ui, p, "Settings", "Changes are saved automatically.");
        let s = &mut self.settings;

        section(ui, p, "Appearance", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (mode, name) in THEMES {
                    if theme_tile(ui, theme::palette(mode), name, s.theme == mode) {
                        s.theme = mode;
                    }
                }
            });
            ui.add_space(6.0);
            ui.checkbox(&mut s.animations, "Animated background")
                .on_hover_text("Aurora, snowfall and shooting stars. Pauses while you play.");
            ui.checkbox(&mut s.intro, "Intro animation on start");
        });

        section(ui, p, "Memory", |ui| {
            memory_slider(
                ui,
                "Maximum",
                &mut s.max_memory_mb,
                MIN_MEMORY_MB,
                MAX_MEMORY_SLIDER_MB,
            );
            let max = s.max_memory_mb;
            memory_slider(ui, "Minimum", &mut s.min_memory_mb, MIN_MEMORY_MB, max);
            ui.label(
                RichText::new(
                    "4 GB suits vanilla. Giving Java more than it needs can cause stutter.",
                )
                .small()
                .color(p.muted),
            );
        });

        section(ui, p, "Game window", |ui| {
            ui.horizontal(|ui| {
                ui.label("Resolution");
                ui.add(egui::DragValue::new(&mut s.window_width).range(320..=MAX_RESOLUTION));
                ui.label("×");
                ui.add(egui::DragValue::new(&mut s.window_height).range(240..=MAX_RESOLUTION));
                egui::ComboBox::from_id_salt("res_presets")
                    .selected_text("Presets")
                    .show_ui(ui, |ui| {
                        for (w, h) in RESOLUTIONS {
                            if ui.selectable_label(false, format!("{w} × {h}")).clicked() {
                                (s.window_width, s.window_height) = (w, h);
                            }
                        }
                    });
            });
            ui.checkbox(&mut s.fullscreen, "Start in fullscreen");
            ui.horizontal(|ui| {
                ui.label("When the game starts");
                ui.selectable_value(
                    &mut s.on_game_start,
                    GameStartAction::KeepOpen,
                    "Keep launcher open",
                );
                ui.selectable_value(
                    &mut s.on_game_start,
                    GameStartAction::Minimize,
                    "Minimize launcher",
                );
            });
        });

        section(ui, p, "Java", |ui| {
            ui.label(
                RichText::new("Arctic downloads the right Java for each version automatically.")
                    .small()
                    .color(p.muted),
            );
            ui.horizontal(|ui| {
                ui.label("Extra JVM arguments");
                ui.add(egui::TextEdit::singleline(&mut s.extra_jvm_args).desired_width(320.0));
            });
            java_override(ui, p, s);
        });

        section(ui, p, "Updates", |ui| {
            ui.horizontal(|ui| {
                ui.label("Channel");
                ui.selectable_value(&mut s.update_channel, UpdateChannel::Stable, "Stable");
                ui.selectable_value(&mut s.update_channel, UpdateChannel::Beta, "Beta");
            });
            ui.checkbox(&mut s.check_updates_on_start, "Check for updates on start");
        });

        ui.horizontal(|ui| {
            if widgets::button(ui, p, Some(Icon::Folder), "Open data folder", false).clicked()
                && let Err(e) = open::that_detached(self.dirs.root())
            {
                self.toasts.push(
                    crate::toasts::Kind::Error,
                    "Could not open folder",
                    e.to_string(),
                );
            }
            if widgets::button(ui, p, None, "Restore defaults", false).clicked() {
                self.settings = Settings {
                    last_version: self.settings.last_version.clone(),
                    ..Settings::default()
                };
            }
        });
    }
}

fn section(ui: &mut egui::Ui, p: &Palette, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(17.0).strong().color(p.text));
        ui.add_space(6.0);
        body(ui);
    });
    ui.add_space(14.0);
}

fn memory_slider(ui: &mut egui::Ui, label: &str, value: &mut u32, min: u32, max: u32) {
    ui.horizontal(|ui| {
        ui.add_sized([80.0, 20.0], egui::Label::new(label));
        ui.add(
            egui::Slider::new(value, min..=max)
                .step_by(MEMORY_STEP_MB)
                .custom_formatter(|v, _| format!("{:.1} GB", v / 1024.0)),
        );
    });
}

/// Mini preview of a theme: its sky, a mountain and the accent. Returns
/// true when clicked.
fn theme_tile(ui: &mut egui::Ui, tp: &Palette, name: &str, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(112.0, 84.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let sel = ui.ctx().animate_bool(response.id.with("s"), selected);
    let painter = ui.painter();
    let preview = egui::Rect::from_min_size(rect.min, vec2(rect.width(), 58.0));
    let [top, _, horizon] = tp.scene.sky;
    painter.rect_filled(
        preview,
        CornerRadius::same(10),
        lerp_color(top, horizon, 0.35),
    );
    let base = preview.bottom();
    let peaks = [
        egui::pos2(preview.left() + 4.0, base),
        egui::pos2(preview.left() + 38.0, base - 30.0),
        egui::pos2(preview.left() + 64.0, base - 12.0),
        egui::pos2(preview.left() + 86.0, base - 26.0),
        egui::pos2(preview.right() - 4.0, base),
    ];
    painter.add(egui::Shape::convex_polygon(
        vec![peaks[0], peaks[1], peaks[2], peaks[4]],
        tp.scene.mountains[0].0,
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![peaks[2], peaks[3], peaks[4]],
        tp.scene.mountains[1].0,
        Stroke::NONE,
    ));
    painter.circle_filled(preview.right_top() + vec2(-16.0, 14.0), 6.0, tp.accent);
    let ring = lerp_color(tp.card_stroke, tp.accent, sel.max(hover * 0.5));
    painter.rect_stroke(
        preview,
        CornerRadius::same(10),
        Stroke::new(1.0 + 1.5 * sel, ring),
        StrokeKind::Outside,
    );
    let label_color = ui.visuals().text_color();
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.bottom() - 10.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.5),
        label_color,
    );
    if selected {
        let check = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 10.0),
            vec2(12.0, 12.0),
        );
        icons::draw(painter, Icon::Check, check, tp.accent);
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn java_override(ui: &mut egui::Ui, p: &Palette, s: &mut Settings) {
    let mut enabled = s.java_override.is_some();
    if ui
        .checkbox(&mut enabled, "Use a custom Java executable")
        .changed()
    {
        s.java_override = enabled.then(PathBuf::new);
    }
    if let Some(path) = &mut s.java_override {
        let mut text = path.display().to_string();
        ui.horizontal(|ui| {
            ui.label("Path to javaw.exe");
            if ui
                .add(egui::TextEdit::singleline(&mut text).desired_width(320.0))
                .changed()
            {
                *path = PathBuf::from(text.trim());
            }
        });
        if !path.as_os_str().is_empty() && !path.is_file() {
            ui.label(
                RichText::new("File not found; the managed runtime will be used.").color(p.warn),
            );
        }
    }
}
