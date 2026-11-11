//! Settings tab. Every change is saved automatically.

use std::path::PathBuf;

use arctic_core::settings::{CrashReports, GameStartAction, MIN_MEMORY_MB, Settings, ThemeMode};
use arctic_core::update::UpdateChannel;
use eframe::egui::{self, CornerRadius, RichText, Sense, Stroke, StrokeKind, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme::{self, Palette};
use crate::widgets;
use crate::{t, tf};

const MAX_MEMORY_SLIDER_MB: u32 = 32 * 1024;
const MEMORY_STEP_MB: f64 = 512.0;
const MAX_RESOLUTION: u32 = 7680;
const RESOLUTIONS: [(u32, u32); 4] = [(854, 480), (1280, 720), (1600, 900), (1920, 1080)];
const THEMES: [(ThemeMode, &str); 4] = [
    (ThemeMode::Default, "settings.theme.aurora"),
    (ThemeMode::Glass, "settings.theme.glass"),
    (ThemeMode::Dark, "settings.theme.dark"),
    (ThemeMode::Light, "settings.theme.light"),
];

/// The language picker: "System (its name)" and every language by its own name. A pick
/// applies at once.
fn language_row(ui: &mut egui::Ui, language: &mut String) {
    use crate::i18n::{self, LANGUAGES, SYSTEM};
    widgets::field_row(ui, |ui| {
        ui.label(t!("common.language"));
        let system_label = tf!(
            "common.language.system",
            name = i18n::name_of(i18n::system_language())
        );
        let shown = if language.as_str() == SYSTEM {
            system_label.clone()
        } else {
            i18n::name_of(language).to_owned()
        };
        let before = language.clone();
        // Every language is listed in its own script, so its font has to be there before the
        // list is drawn (not only once it's picked).
        for entry in LANGUAGES {
            crate::fonts::ensure_for(ui.ctx(), entry.name);
        }
        // As tall as the row, so the label beside it sits on the same center line.
        ui.spacing_mut().interact_size.y = widgets::FIELD_HEIGHT;
        egui::ComboBox::from_id_salt("language")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(language.as_str() == SYSTEM, system_label)
                    .clicked()
                {
                    *language = SYSTEM.to_owned();
                }
                for entry in LANGUAGES {
                    if ui
                        .selectable_label(language.as_str() == entry.code, entry.name)
                        .clicked()
                    {
                        *language = entry.code.to_owned();
                    }
                }
            });
        if *language != before {
            i18n::set_language(language);
            i18n::ensure_fonts(ui.ctx());
        }
    });
}

impl ArcticApp {
    pub(crate) fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            t!("settings.settings"),
            t!("settings.changes_are_saved_automatically"),
        );
        let mut performance = self.instance.performance;
        let mut choose_picture = false;
        let s = &mut self.settings;

        section(ui, p, t!("settings.appearance"), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (mode, name) in THEMES {
                    if theme_tile(ui, theme::palette(mode), t!(name), s.theme == mode) {
                        s.theme = mode;
                    }
                }
            });
            ui.add_space(6.0);
            crate::zoom::size_row(ui, &mut s.interface_size);
            language_row(ui, &mut s.language);
            widgets::field_row(ui, |ui| {
                use arctic_core::settings::Backdrop;
                ui.label(t!("settings.background"));
                ui.selectable_value(
                    &mut s.backdrop,
                    Backdrop::Scenery,
                    t!("settings.arctic_scenery"),
                );
                ui.selectable_value(
                    &mut s.backdrop,
                    Backdrop::Picture,
                    t!("settings.your_screenshot"),
                )
                .on_hover_text(t!("settings.your_newest_screenshot_or_the_versions_title"));
                if ui
                    .selectable_label(s.backdrop == Backdrop::Custom, t!("settings.your_picture"))
                    .on_hover_text(match &s.custom_backdrop {
                        Some(path) => {
                            tf!("settings.choose_another_picture_now", path = path.display())
                        }
                        None => t!("settings.choose_a_picture_of_your_own").to_owned(),
                    })
                    .clicked()
                {
                    match &s.custom_backdrop {
                        Some(_) if s.backdrop != Backdrop::Custom => s.backdrop = Backdrop::Custom,
                        _ => choose_picture = true,
                    }
                }
            });
            ui.checkbox(&mut s.animations, t!("settings.animated_background"))
                .on_hover_text(t!("settings.aurora_snowfall_and_shooting_stars_pauses"));
            ui.checkbox(
                &mut s.reduce_cape_motion,
                t!("settings.freeze_animated_capes_and_cosmetics"),
            )
            .on_hover_text(t!("settings.animated_capes_and_sculpted_cosmetics_stand"));
            ui.checkbox(&mut s.intro, t!("settings.intro_animation_on_start"));
            ui.checkbox(
                &mut s.start_maximized,
                t!("settings.start_the_launcher_maximized"),
            );
            if cfg!(windows) {
                ui.checkbox(
                    &mut s.tray,
                    t!("settings.keep_arctic_in_the_system_tray_when_closed"),
                )
                .on_hover_text(t!("settings.reopens_instantly_quit_from_the_tray_icon"));
            }
            ui.checkbox(
                &mut s.exit_games_with_launcher,
                t!("settings.close_minecraft_when_arctic_quits"),
            )
            .on_hover_text(t!("settings.off_games_keep_running_after_the_launcher"));
            ui.checkbox(&mut s.show_sponsor, t!("settings.show_supporter_mentions"))
                .on_hover_text(t!("settings.supported_by_flash_hosting_in_the_sidebar"));
            ui.checkbox(&mut s.quick_tips, t!("settings.show_quick_tips"))
                .on_hover_text(t!("settings.a_tip_about_one_of_arctics_features_in_the"));
        });

        section(ui, p, "Arctic Client", |ui| {
            ui.label(
                RichText::new(t!("settings.how_minecrafts_menus_look_in_game_press"))
                    .small()
                    .color(p.muted),
            );
            ui.add_space(4.0);
            super::client_style::picker(ui, p, s, super::client_style::SETTINGS_TILE_WIDTH);
            ui.add_space(6.0);
            super::client_style::fancy_toggle(ui, s);
            ui.checkbox(
                &mut performance,
                t!("settings.boost_fps_with_performance_mods"),
            )
            .on_hover_text(t!("settings.vanilla_adds_sodium_lithium_ferritecore"));
        });

        section(ui, p, t!("settings.launching"), |ui| {
            ui.checkbox(&mut s.gentle_launch, t!("settings.gentle_launch"))
                .on_hover_text(t!("settings.downloads_a_few_files_at_a_time_and_starts"));
        });

        section(ui, p, t!("settings.memory"), |ui| {
            memory_slider(
                ui,
                t!("settings.maximum"),
                &mut s.max_memory_mb,
                MIN_MEMORY_MB,
                MAX_MEMORY_SLIDER_MB,
            );
            let max = s.max_memory_mb;
            memory_slider(
                ui,
                t!("settings.minimum"),
                &mut s.min_memory_mb,
                MIN_MEMORY_MB,
                max,
            );
            ui.label(
                RichText::new(t!("settings.4_gb_suits_vanilla_giving_java_more_than_it"))
                    .small()
                    .color(p.muted),
            );
        });

        section(ui, p, t!("settings.versions"), |ui| {
            ui.checkbox(&mut s.show_snapshots, t!("settings.show_snapshots"))
                .on_hover_text(t!("settings.weekly_development_builds_they_can_break"));
            ui.checkbox(
                &mut s.show_old_versions,
                t!("settings.show_old_alpha_and_beta_versions"),
            );
        });

        section(ui, p, t!("settings.game_window"), |ui| {
            widgets::field_row(ui, |ui| {
                ui.label(t!("settings.resolution"));
                ui.add(egui::DragValue::new(&mut s.window_width).range(320..=MAX_RESOLUTION));
                ui.label("×");
                ui.add(egui::DragValue::new(&mut s.window_height).range(240..=MAX_RESOLUTION));
                egui::ComboBox::from_id_salt("res_presets")
                    .selected_text(t!("settings.presets"))
                    .show_ui(ui, |ui| {
                        for (w, h) in RESOLUTIONS {
                            if ui.selectable_label(false, format!("{w} × {h}")).clicked() {
                                (s.window_width, s.window_height) = (w, h);
                            }
                        }
                    });
            });
            ui.checkbox(&mut s.fullscreen, t!("settings.start_in_fullscreen"));
            ui.add_enabled(
                !s.fullscreen,
                egui::Checkbox::new(
                    &mut s.game_maximized,
                    t!("settings.start_the_game_maximized"),
                ),
            )
            .on_hover_text(t!("settings.the_game_window_fills_the_screen_with_the"));
            // macOS already runs OpenGL games on the gaming card.
            if !cfg!(target_os = "macos") {
                ui.checkbox(
                    &mut s.high_performance_gpu,
                    t!("settings.use_the_gaming_graphics_card"),
                )
                .on_hover_text(t!("settings.laptops_with_two_graphics_cards_minecraft"));
            }
            widgets::field_row(ui, |ui| {
                ui.label(t!("settings.when_the_game_starts"));
                ui.selectable_value(
                    &mut s.on_game_start,
                    GameStartAction::KeepOpen,
                    t!("settings.keep_launcher_open"),
                );
                ui.selectable_value(
                    &mut s.on_game_start,
                    GameStartAction::Minimize,
                    t!("settings.minimize_launcher"),
                );
            });
            ui.checkbox(
                &mut s.discord_presence,
                t!("settings.show_what_youre_playing_on_discord"),
            );
            widgets::field_row(ui, |ui| {
                ui.label(t!("settings.crash_reports"))
                    .on_hover_text(t!("settings.when_the_game_crashes_arctic_can_send_its"));
                ui.selectable_value(
                    &mut s.crash_reports,
                    CrashReports::Ask,
                    t!("settings.ask_me"),
                );
                ui.selectable_value(
                    &mut s.crash_reports,
                    CrashReports::Always,
                    t!("settings.always_send"),
                );
                ui.selectable_value(
                    &mut s.crash_reports,
                    CrashReports::Never,
                    t!("settings.never"),
                );
            });
        });

        section(ui, p, "Java", |ui| {
            ui.label(
                RichText::new(t!("settings.arctic_downloads_the_right_java_for_each"))
                    .small()
                    .color(p.muted),
            );
            widgets::field_row(ui, |ui| {
                ui.label(t!("settings.extra_jvm_arguments"));
                ui.add(widgets::text_field(&mut s.extra_jvm_args).desired_width(320.0));
            });
            java_override(ui, p, s);
        });

        section(ui, p, t!("settings.updates"), |ui| {
            if arctic_core::update::managed_by_flatpak() {
                ui.label(t!("settings.updates_are_managed_by_your_linux_software"));
                ui.checkbox(
                    &mut s.show_release_notes,
                    t!("settings.show_whats_new_after_updating"),
                );
                return;
            }
            widgets::field_row(ui, |ui| {
                ui.label(t!("settings.channel"));
                ui.selectable_value(
                    &mut s.update_channel,
                    UpdateChannel::Stable,
                    t!("settings.stable"),
                );
                ui.selectable_value(
                    &mut s.update_channel,
                    UpdateChannel::Beta,
                    t!("settings.beta"),
                );
            });
            ui.checkbox(
                &mut s.check_updates_on_start,
                t!("settings.check_for_updates_on_start"),
            )
            .on_hover_text(t!("settings.a_few_seconds_after_the_launcher_opens_so_it"));
            ui.checkbox(
                &mut s.auto_install_updates,
                t!("settings.install_updates_automatically"),
            )
            .on_hover_text(t!("settings.new_versions_download_by_themselves_and_are"));
            ui.checkbox(
                &mut s.restart_hidden_for_updates,
                t!("settings.restart_by_itself_when_waiting_in_the_tray"),
            )
            .on_hover_text(t!("settings.off_an_installed_update_is_used_the_next"));
            ui.checkbox(
                &mut s.show_release_notes,
                t!("settings.show_whats_new_after_updating"),
            )
            .on_hover_text(t!("settings.after_an_update_a_popup_lists_whats_new_and"));
        });

        section(ui, p, t!("settings.network"), |ui| {
            self.network_section(ui, p)
        });

        section(ui, p, t!("settings.voice_chat"), |ui| {
            self.voice_section(ui, p)
        });

        section(ui, p, t!("settings.shared_between_instances"), |ui| {
            ui.label(
                RichText::new(t!("settings.kept_the_same_in_every_instance_of_this"))
                    .small()
                    .color(p.muted),
            );
            ui.add_space(4.0);
            let shared = &mut self.settings.shared;
            ui.checkbox(&mut shared.servers, t!("settings.server_list"));
            ui.checkbox(
                &mut shared.client,
                t!("settings.arctic_client_settings_hud_crosshair_keys"),
            );
            ui.checkbox(
                &mut shared.game,
                t!("settings.minecraft_settings_keys_video_sound_fov"),
            );
        });

        section(
            ui,
            p,
            t!("settings.game_settings_for_new_instances"),
            |ui| self.game_defaults_section(ui, p),
        );

        section(ui, p, t!("settings.storage"), |ui| {
            self.storage_section(ui, p)
        });

        section(ui, p, t!("settings.share_back_up"), |ui| {
            ui.label(
                RichText::new(t!("settings.your_settings_instances_as_mod_lists_and_hud"))
                    .small()
                    .color(p.muted),
            );
            ui.add_space(4.0);
            crate::widgets::row(ui, |ui| {
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Share),
                    t!("settings.export_profile"),
                    false,
                )
                .clicked()
                {
                    self.open_share(None);
                }
                if widgets::button(ui, p, Some(Icon::Import), t!("settings.import"), false)
                    .clicked()
                {
                    self.open_share_import();
                }
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Layers),
                    t!("settings.from_another_launcher"),
                    false,
                )
                .clicked()
                {
                    self.open_migrate();
                }
            });
        });

        crate::widgets::row(ui, |ui| {
            if widgets::button(ui, p, None, t!("settings.run_setup_again"), false)
                .on_hover_text(t!("settings.the_first_run_walkthrough_starting_from_your"))
                .clicked()
            {
                self.start_onboarding();
            }
            if widgets::button(ui, p, None, t!("settings.desktop_shortcut"), false)
                .on_hover_text(t!("settings.puts_an_arctic_launcher_shortcut_on_your"))
                .clicked()
            {
                match crate::desktop_shortcut::create() {
                    Ok(path) => self.toasts.push(
                        crate::toasts::Kind::Success,
                        t!("settings.desktop_shortcut_made"),
                        path.display().to_string(),
                    ),
                    Err(e) => self.toasts.push(
                        crate::toasts::Kind::Error,
                        t!("settings.couldnt_make_the_shortcut"),
                        e,
                    ),
                }
            }
            if widgets::button(ui, p, None, t!("settings.restore_defaults"), false).clicked() {
                self.settings = Settings {
                    last_version: self.settings.last_version.clone(),
                    ..Settings::default()
                };
            }
        });
        if choose_picture {
            self.tasks.pick_backdrop();
        }
        if performance != self.instance.performance {
            let id = self.instance.id.clone();
            if performance {
                self.update_instance(&id, |i| i.performance = true);
            } else {
                self.performance_off_confirm = Some(id);
            }
        }
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
        // From where the tile's rounded corners end, or the mountains poke out of them.
        egui::pos2(preview.left() + 10.0, base),
        egui::pos2(preview.left() + 38.0, base - 30.0),
        egui::pos2(preview.left() + 64.0, base - 12.0),
        egui::pos2(preview.left() + 86.0, base - 26.0),
        egui::pos2(preview.right() - 10.0, base),
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
    if tp.glass {
        // A frosted pane over the mountains.
        let pane = egui::Rect::from_min_size(preview.min + vec2(14.0, 16.0), vec2(60.0, 32.0));
        painter.rect_filled(
            pane,
            CornerRadius::same(6),
            egui::Color32::from_white_alpha(46),
        );
        painter.rect_stroke(
            pane,
            CornerRadius::same(6),
            Stroke::new(1.0, egui::Color32::from_white_alpha(90)),
            egui::StrokeKind::Inside,
        );
    }
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
        .checkbox(&mut enabled, t!("settings.use_a_custom_java_executable"))
        .changed()
    {
        s.java_override = enabled.then(PathBuf::new);
    }
    if let Some(path) = &mut s.java_override {
        let mut text = path.display().to_string();
        widgets::field_row(ui, |ui| {
            ui.label(t!("settings.path_to_javaw_exe"));
            if ui
                .add(widgets::text_field(&mut text).desired_width(320.0))
                .changed()
            {
                *path = PathBuf::from(text.trim());
            }
        });
        if !path.as_os_str().is_empty() && !path.is_file() {
            ui.label(
                RichText::new(t!("settings.file_not_found_the_managed_runtime_will_be"))
                    .color(p.warn),
            );
        }
    }
}
