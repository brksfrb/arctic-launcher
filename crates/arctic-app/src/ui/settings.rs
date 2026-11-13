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

const MAX_MEMORY_SLIDER_MB: u32 = 32 * 1024;
const MEMORY_STEP_MB: f64 = 512.0;
const MAX_RESOLUTION: u32 = 7680;
const RESOLUTIONS: [(u32, u32); 4] = [(854, 480), (1280, 720), (1600, 900), (1920, 1080)];
const THEMES: [(ThemeMode, &str); 4] = [
    (ThemeMode::Default, "Aurora"),
    (ThemeMode::Glass, "Glass"),
    (ThemeMode::Dark, "Dark"),
    (ThemeMode::Light, "Light"),
];

/// The groups of the Settings page: one is shown at a time (picked with the chips at the top).
const GROUPS: [&str; 5] = ["General", "Game", "Network & voice", "Data", "Experiments"];

/// Versions the Rust client can run: 1.8.9 is Arctic's own rewrite, 26.3 is Horizon's.
const RUST_CLIENT_VERSIONS: [&str; 2] = ["1.8.9", "26.3"];

/// Whether a Rust client is actually here yet. One line to flip when it is.
const RUST_CLIENT_READY: bool = false;

thread_local! {
    /// A pending section or control to reveal.
    static SCROLL_TO: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
    /// The group being shown (the UI runs on one thread).
    static GROUP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Select the containing tab and reveal the requested section on the next frame.
pub(super) fn open_section(title: &'static str) {
    GROUP.with(|g| g.set(group_of(title)));
    SCROLL_TO.with(|s| s.set(Some(title)));
}

/// Which group a section belongs to.
fn group_of(title: &str) -> usize {
    match title {
        "Freeze animated capes and cosmetics"
        | "Appearance"
        | "Arctic Client"
        | "Launching"
        | "Updates" => 0,
        "Memory" | "Versions" | "Game window" | "Java" => 1,
        "Network" | "Voice chat" => 2,
        "Experiments" => 4,
        _ => 3,
    }
}

impl ArcticApp {
    pub(crate) fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(ui, p, "Settings", "Changes are saved automatically.");
        let mut group = GROUP.with(|g| g.get());
        ui.horizontal(|ui| {
            for (i, name) in GROUPS.iter().enumerate() {
                if ui.selectable_label(group == i, *name).clicked() {
                    group = i;
                }
            }
        });
        GROUP.with(|g| g.set(group));
        ui.add_space(8.0);
        let mut performance = self.instance.performance;
        let mut choose_picture = false;
        let mut restart_update = false;
        let update_installed = matches!(self.update, crate::app::UpdateState::Installed { .. });
        let software_gl = self.software_gl;
        // Read from the set the app already keeps: `installed_versions` walks the disk, and this
        // runs on every painted frame.
        let rust_client_version = RUST_CLIENT_VERSIONS
            .into_iter()
            .find(|v| self.installed.contains(*v));
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
            crate::zoom::size_row(ui, &mut s.interface_size);
            widgets::field_row(ui, |ui| {
                use arctic_core::settings::Backdrop;
                ui.label("Background");
                ui.selectable_value(&mut s.backdrop, Backdrop::Scenery, "Arctic scenery");
                ui.selectable_value(&mut s.backdrop, Backdrop::Picture, "Your screenshot")
                    .on_hover_text("Your newest screenshot, or the version's title screen");
                if ui
                    .selectable_label(s.backdrop == Backdrop::Custom, "Your picture…")
                    .on_hover_text(match &s.custom_backdrop {
                        Some(path) => format!("Choose another picture. Now: {}", path.display()),
                        None => "Choose a picture of your own".to_owned(),
                    })
                    .clicked()
                {
                    match &s.custom_backdrop {
                        Some(_) if s.backdrop != Backdrop::Custom => s.backdrop = Backdrop::Custom,
                        _ => choose_picture = true,
                    }
                }
            });
            ui.checkbox(&mut s.animations, "Animated background")
                .on_hover_text("Aurora, snowfall and shooting stars. Pauses while you play.");
            if software_gl {
                ui.label(
                    RichText::new(
                        "Off for now: your graphics driver draws in software (no GPU), where this slows the whole PC down. The glass blur is off too.",
                    )
                    .small()
                    .color(p.warn),
                );
            }
            let freeze = ui.checkbox(&mut s.reduce_cape_motion, "Freeze animated capes and cosmetics")
                .on_hover_text("Animated capes and sculpted cosmetics stand still here. Only your own view changes; what you and others wear stays as chosen.");
            if SCROLL_TO.with(|s| s.get()) == Some("Freeze animated capes and cosmetics") {
                freeze.scroll_to_me(Some(egui::Align::Center));
                SCROLL_TO.with(|s| s.set(None));
            }
            ui.checkbox(&mut s.intro, "Intro animation on start");
            ui.checkbox(&mut s.start_maximized, "Start the launcher maximized");
            if cfg!(windows) {
                ui.checkbox(&mut s.tray, "Keep Arctic in the system tray when closed")
                    .on_hover_text("Reopens instantly. Quit from the tray icon.");
            }
            ui.checkbox(
                &mut s.exit_games_with_launcher,
                "Close Minecraft when Arctic quits",
            )
            .on_hover_text("Off: games keep running after the launcher closes.");
            ui.checkbox(
                &mut s.confirm_join_launch,
                "Ask before launching Minecraft to join a server",
            )
            .on_hover_text("Join on a server asks first when no game is running.");
            ui.checkbox(&mut s.show_sponsor, "Show supporter mentions")
                .on_hover_text(
                    "\"Supported by Flash Hosting\" in the sidebar, About and Play together.",
                );
            ui.checkbox(&mut s.quick_tips, "Show quick tips")
                .on_hover_text("A tip about one of Arctic's features in the sidebar, a new one every 5 minutes.");
        });

        section(ui, p, "Arctic Client", |ui| {
            ui.label(
                RichText::new(
                    "How Minecraft's menus look in game. Press Right Shift in game for HUD widgets and capes.",
                )
                .small()
                .color(p.muted),
            );
            ui.add_space(4.0);
            ui.checkbox(&mut s.play_with_client, "Play with Arctic Client")
                .on_hover_text(
                    "On: games start with the Arctic Client (HUD, zoom, cosmetics, replays and more). Off: no game does, whatever its own switch says; the mod is taken out of the instance's mods folder.",
                );
            ui.add_space(4.0);
            ui.add_enabled_ui(s.play_with_client, |ui| {
                super::client_style::picker(ui, p, s, super::client_style::SETTINGS_TILE_WIDTH);
                ui.add_space(6.0);
                super::client_style::fancy_toggle(ui, s);
            });
            ui.checkbox(&mut performance, "Boost FPS with performance mods")
                .on_hover_text(
                    "Vanilla: adds Sodium, Lithium, FerriteCore, ImmediatelyFast, EntityCulling, Particle Core, BadOptimizations, ModernFix and Polarium. Other instances have their own switch.",
                );
        });

        section(ui, p, "Launching", |ui| {
            ui.checkbox(&mut s.gentle_launch, "Gentle launch")
                .on_hover_text(
                    "Downloads a few files at a time and starts the game at low priority, so a \
                     computer that freezes under load stays usable. Starting takes longer.",
                );
        });

        section(ui, p, "Experiments", |ui| {
            ui.label(
                RichText::new("Unfinished work, off by default. Expect rough edges.")
                    .small()
                    .color(p.muted),
            );
            ui.add_space(6.0);

            let usable = RUST_CLIENT_READY && rust_client_version.is_some();
            ui.add_enabled_ui(usable, |ui| {
                ui.checkbox(&mut s.rust_client, "Use rust rewrite for client");
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Runs Minecraft 1.8.9 rebuilt in Rust rather than starting the Java game. \
                     Everything that plugs into Java — Fabric, Forge, OptiFine, mixins — is left \
                     behind, because there is no JVM for it to plug into.",
                )
                .small()
                .color(p.muted),
            );

            // Say which of the two reasons it is, rather than leaving a dead switch unexplained.
            let why = if !RUST_CLIENT_READY {
                Some(
                    "Greyed out because the Rust client isn't finished yet. The switch is here so \
                     you can see it coming; it will turn on by itself when the client can run."
                        .to_owned(),
                )
            } else {
                rust_client_version.is_none().then(|| {
                    format!(
                        "Greyed out because neither {} is installed. The Rust client reads the \
                         game's textures and sounds from your own copy, so install one of those \
                         versions first — downloading it is enough, you never have to start it.",
                        RUST_CLIENT_VERSIONS.join(" nor ")
                    )
                })
            };
            if let Some(why) = why {
                ui.add_space(4.0);
                ui.label(RichText::new(why).small().color(p.warn));
            }

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "The idea and the groundwork are Horizon's (playhorizon.dev), who rebuilt \
                     Minecraft Java Edition 26.3 in Rust. Arctic's client targets 1.8.9 and builds \
                     on their MIT-licensed work.",
                )
                .small()
                .color(p.muted),
            );
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

        section(ui, p, "Versions", |ui| {
            ui.checkbox(&mut s.show_snapshots, "Show snapshots")
                .on_hover_text("Weekly development builds. They can break worlds; back up first.");
            ui.checkbox(&mut s.show_old_versions, "Show old alpha and beta versions");
        });

        section(ui, p, "Game window", |ui| {
            widgets::field_row(ui, |ui| {
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
            ui.add_enabled(
                !s.fullscreen,
                egui::Checkbox::new(&mut s.game_maximized, "Start the game maximized"),
            )
            .on_hover_text(
                "The game window fills the screen, with the taskbar and title bar still there.",
            );
            // macOS already runs OpenGL games on the gaming card.
            if !cfg!(target_os = "macos") {
                ui.checkbox(&mut s.high_performance_gpu, "Use the gaming graphics card")
                    .on_hover_text(
                        "Laptops with two graphics cards: Minecraft runs on the fast one instead of the built-in one. A choice you made yourself (Windows' graphics settings, or PRIME variables on Linux) is kept.",
                    );
            }
            widgets::field_row(ui, |ui| {
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
            ui.checkbox(
                &mut s.discord_presence,
                "Show what you're playing on Discord",
            );
            let presence_on = s.discord_presence;
            ui.add_enabled_ui(presence_on, |ui| {
                ui.indent("discord_show_server", |ui| {
                    ui.checkbox(&mut s.discord_show_server, "Show server in Discord status")
                        .on_hover_text(crate::discord::SHOW_SERVER_TIP)
                        .on_disabled_hover_text(
                            "Turn on \"Show what you're playing on Discord\" first.",
                        );
                });
            });
            widgets::field_row(ui, |ui| {
                ui.label("Crash reports")
                    .on_hover_text("When the game crashes, Arctic can send its log so the crash can be fixed. Your name, folders and sign-in details are taken out first.");
                ui.selectable_value(&mut s.crash_reports, CrashReports::Ask, "Ask me");
                ui.selectable_value(&mut s.crash_reports, CrashReports::Always, "Always send");
                ui.selectable_value(&mut s.crash_reports, CrashReports::Never, "Never");
            });
            widgets::field_row(ui, |ui| {
                ui.label("Contact for crashes")
                    .on_hover_text("Optional. Sent with a crash report so we can write back to you: a Discord name or an e-mail address. Leave it empty to stay anonymous.");
                ui.add(
                    egui::TextEdit::singleline(&mut s.crash_contact)
                        .hint_text("Discord name or e-mail (optional)")
                        .char_limit(80)
                        .desired_width(240.0),
                );
            });
        });

        section(ui, p, "Java", |ui| {
            ui.label(
                RichText::new("Arctic downloads the right Java for each version automatically.")
                    .small()
                    .color(p.muted),
            );
            widgets::field_row(ui, |ui| {
                ui.label("Extra JVM arguments");
                ui.add(widgets::text_field(&mut s.extra_jvm_args).desired_width(320.0));
            });
            java_override(ui, p, s);
        });

        section(ui, p, "Updates", |ui| {
            if arctic_core::update::managed_by_flatpak() {
                ui.label("Updates are managed by your Linux software manager.");
                ui.checkbox(&mut s.show_release_notes, "Show what's new after updating");
                return;
            }
            widgets::field_row(ui, |ui| {
                ui.label("Channel");
                ui.selectable_value(&mut s.update_channel, UpdateChannel::Stable, "Stable");
                ui.selectable_value(&mut s.update_channel, UpdateChannel::Beta, "Beta");
            });
            ui.checkbox(&mut s.check_updates_on_start, "Check for updates on start")
                .on_hover_text("A few seconds after the launcher opens, so it starts fast");
            ui.checkbox(&mut s.auto_install_updates, "Install updates automatically")
                .on_hover_text(
                    "New versions download by themselves and are used the next time you open the launcher.",
                );
            ui.checkbox(
                &mut s.restart_hidden_for_updates,
                "Restart by itself when waiting in the tray",
            )
            .on_hover_text(
                "On by default: restart into an installed update while hidden in the tray with no game running. Off: use the update next time you open the launcher, or choose Restart now here.",
            );
            ui.checkbox(&mut s.show_release_notes, "Show what's new after updating")
                .on_hover_text("After an update, a popup lists what's new and what's fixed");
            if update_installed {
                restart_update =
                    widgets::button(ui, p, None, "Update installed - Restart now", false).clicked();
            }
        });

        if restart_update {
            self.restart_after_update(ui.ctx());
        }

        section(ui, p, "Network", |ui| self.network_section(ui, p));

        section(ui, p, "Voice chat", |ui| self.voice_section(ui, p));

        section(ui, p, "Shared between instances", |ui| {
            ui.label(
                RichText::new(
                    "Kept the same in every instance of this profile: whatever you changed last is used everywhere, from the next start.",
                )
                .small()
                .color(p.muted),
            );
            ui.add_space(4.0);
            let shared = &mut self.settings.shared;
            ui.checkbox(&mut shared.servers, "Server list");
            ui.checkbox(
                &mut shared.client,
                "Arctic Client settings (HUD, crosshair, keys, waypoints)",
            );
            ui.checkbox(
                &mut shared.game,
                "Minecraft settings (keys, video, sound, FOV)",
            );
        });

        section(ui, p, "Game settings for new instances", |ui| {
            self.game_defaults_section(ui, p)
        });

        section(ui, p, "Storage", |ui| self.storage_section(ui, p));

        section(ui, p, "Share & back up", |ui| {
            ui.label(
                RichText::new(
                    "Your settings, instances (as mod lists) and HUD setups, as a code, text or file. Accounts and passwords are never included.",
                )
                .small()
                .color(p.muted),
            );
            ui.add_space(4.0);
            crate::widgets::row(ui, |ui| {
                if widgets::button(ui, p, Some(Icon::Share), "Export profile", false).clicked() {
                    self.open_share(None);
                }
                if widgets::button(ui, p, Some(Icon::Import), "Import", false).clicked() {
                    self.open_share_import();
                }
                if widgets::button(ui, p, Some(Icon::Layers), "From another launcher", false)
                    .clicked()
                {
                    self.open_migrate();
                }
            });
        });

        crate::widgets::row(ui, |ui| {
            if widgets::button(ui, p, None, "Run setup again", false)
                .on_hover_text("The first-run walkthrough, starting from your current choices")
                .clicked()
            {
                self.start_onboarding();
            }
            if widgets::button(ui, p, None, "Desktop shortcut", false)
                .on_hover_text("Puts an Arctic Launcher shortcut on your desktop")
                .clicked()
            {
                match crate::desktop_shortcut::create() {
                    Ok(path) => self.toasts.push(
                        crate::toasts::Kind::Success,
                        "Desktop shortcut made",
                        path.display().to_string(),
                    ),
                    Err(e) => self.toasts.push(
                        crate::toasts::Kind::Error,
                        "Couldn't make the shortcut",
                        e,
                    ),
                }
            }
            if widgets::button(ui, p, None, "Restore defaults", false).clicked() {
                self.settings = Settings {
                    last_version: self.settings.last_version.clone(),
                    version_view: self.settings.version_view,
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
    if group_of(title) != GROUP.with(|g| g.get()) {
        return;
    }
    let response = theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(17.0).strong().color(p.text));
        ui.add_space(6.0);
        body(ui);
    });
    if SCROLL_TO.with(|s| s.get()) == Some(title) {
        ui.scroll_to_rect(response.response.rect, Some(egui::Align::Min));
        SCROLL_TO.with(|s| s.set(None));
    }
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
        .checkbox(&mut enabled, "Use a custom Java executable")
        .changed()
    {
        s.java_override = enabled.then(PathBuf::new);
    }
    if let Some(path) = &mut s.java_override {
        let mut text = path.display().to_string();
        widgets::field_row(ui, |ui| {
            ui.label("Path to javaw.exe");
            if ui
                .add(widgets::text_field(&mut text).desired_width(320.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_tip_settings_select_the_containing_group_and_request_scroll() {
        for (section, group) in [
            ("Storage", 3),
            ("Appearance", 0),
            ("Arctic Client", 0),
            ("Voice chat", 2),
            ("Freeze animated capes and cosmetics", 0),
        ] {
            GROUP.with(|g| g.set(1));
            open_section(section);
            assert_eq!(GROUP.with(|g| g.get()), group, "{section}");
            assert_eq!(SCROLL_TO.with(|s| s.get()), Some(section));
        }
        SCROLL_TO.with(|s| s.set(None));
    }
}
