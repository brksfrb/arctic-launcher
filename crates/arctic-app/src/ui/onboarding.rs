//! First-run setup: how the launcher looks and behaves, account, memory,
//! how the game looks, and what to play first. Every default someone might
//! wonder about is a choice here. Shown once per profile, on top of the
//! normal window so theme changes preview live behind it.

use arctic_core::settings::{GameStartAction, ThemeMode};
use arctic_core::system;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Modal, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, vec2,
};

use super::dialogs::option_tile;
use crate::app::{AddAccount, ArcticApp, Tab};
use crate::art::icons::Icon;
use crate::art::{brand_mark, lerp_color};
use crate::motion::eased;
use crate::theme::{self, Palette};
use crate::widgets;
use crate::{t, tf};
const WIDTH: f32 = 500.0;
/// Seconds a step takes to slide in.
const STEP_TRANSITION: f32 = 0.22;
const STEP_SLIDE: f32 = 18.0;
/// Checkbox width, so hints line up with the label.
const HINT_INDENT: f32 = 24.0;
const THEMES: [(ThemeMode, &str); 4] = [
    (ThemeMode::Default, "play.theme.aurora"),
    (ThemeMode::Glass, "play.theme.glass"),
    (ThemeMode::Dark, "play.theme.dark"),
    (ThemeMode::Light, "play.theme.light"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Look,
    Behavior,
    Account,
    Memory,
    Client,
    Start,
}

impl Step {
    const ALL: [Step; 7] = [
        Step::Welcome,
        Step::Look,
        Step::Behavior,
        Step::Account,
        Step::Memory,
        Step::Client,
        Step::Start,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// What to do after setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstPlay {
    /// Bring instances over from other launchers.
    Move,
    Vanilla,
    Modded,
    Later,
}

pub struct Onboarding {
    step: Step,
    changed_at: f64,
    total_mb: Option<u64>,
    first: FirstPlay,
    /// Other launchers and clients found on this PC.
    others: Vec<&'static str>,
}

impl ArcticApp {
    /// Start setup for a profile that hasn't been through it.
    pub(crate) fn maybe_start_onboarding(&mut self, skip: bool) {
        self.onboarding = None;
        if self.settings.onboarded {
            return;
        }
        // Profiles from before onboarding existed are already set up.
        if skip || !self.accounts.accounts.is_empty() {
            self.settings.onboarded = true;
            return;
        }
        self.settings.max_memory_mb = system::recommended_memory_mb(system::total_memory_mb());
        self.start_onboarding();
    }

    /// Show setup from the start (first run, or "Run setup again" in
    /// Settings, which keeps the current choices as the starting point).
    pub(crate) fn start_onboarding(&mut self) {
        let total_mb = system::total_memory_mb();
        // Debug builds: `ARCTIC_DEVSHOT_ONBOARDING=<n>` opens step n (for screenshots).
        let step = std::env::var("ARCTIC_DEVSHOT_ONBOARDING")
            .ok()
            .filter(|_| crate::devshot::enabled())
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| Step::ALL.get(n).copied())
            .unwrap_or(Step::Welcome);
        let others = other_launchers();
        self.onboarding = Some(Onboarding {
            step,
            changed_at: 0.0,
            total_mb,
            first: if others.is_empty() {
                FirstPlay::Vanilla
            } else {
                FirstPlay::Move
            },
            others,
        });
    }

    pub(crate) fn onboarding_dialog(&mut self, ctx: &egui::Context) {
        let Some(step) = self.onboarding.as_ref().map(|o| o.step) else {
            return;
        };
        let p = self.palette();
        let now = ctx.input(|i| i.time);
        let t = self
            .onboarding
            .as_ref()
            .map_or(1.0, |o| eased(now, o.changed_at, STEP_TRANSITION));
        if t < 1.0 {
            ctx.request_repaint();
        }
        let mut go: Option<isize> = None;
        let mut finish = false;
        // Clicking outside or Escape doesn't dismiss setup; "Skip" does.
        Modal::new(Id::new("onboarding"))
            .frame(super::dialogs::dialog_frame(p).inner_margin(28.0))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                progress_dots(ui, p, step.index());
                ui.add_space(14.0);
                ui.scope(|ui| {
                    widgets::lift_controls(ui, p);
                    ui.multiply_opacity(t);
                    ui.add_space((1.0 - t) * STEP_SLIDE);
                    ui.set_min_height(300.0);
                    match step {
                        Step::Welcome => welcome(ui, p, now),
                        Step::Look => self.look_step(ui),
                        Step::Behavior => self.behavior_step(ui),
                        Step::Account => self.account_step(ui),
                        Step::Memory => self.memory_step(ui),
                        Step::Client => self.client_step(ui),
                        Step::Start => self.start_step(ui),
                    }
                });
                ui.add_space(18.0);
                crate::widgets::row(ui, |ui| {
                    if step == Step::Welcome {
                        if ui.link(t!("play.skip_setup")).clicked() {
                            finish = true;
                        }
                    } else if crate::widgets::button(ui, p, None, t!("play.back"), false).clicked()
                    {
                        go = Some(-1);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, icon) = match step {
                            Step::Welcome => (t!("play.get_started"), Icon::Play),
                            Step::Start => (t!("play.start_playing"), Icon::Check),
                            Step::Account if self.accounts.active().is_none() => {
                                (t!("play.skip_for_now"), Icon::Play)
                            }
                            _ => (t!("play.next"), Icon::Play),
                        };
                        if widgets::button(ui, p, Some(icon), label, true).clicked() {
                            if step == Step::Start {
                                finish = true;
                            } else {
                                go = Some(1);
                            }
                        }
                    });
                });
            });
        if finish {
            self.finish_onboarding(now);
        } else if let Some(delta) = go
            && let Some(o) = &mut self.onboarding
        {
            let next = step.index().saturating_add_signed(delta);
            if let Some(s) = Step::ALL.get(next) {
                o.step = *s;
                o.changed_at = now;
            }
        }
    }

    fn look_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            t!("play.pick_a_look"),
            t!("play.you_can_change_this_any_time_in_settings"),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for (mode, name) in THEMES {
                if theme_tile(ui, p, mode, t!(name), self.settings.theme == mode) {
                    self.settings.theme = mode;
                }
            }
        });
        ui.add_space(12.0);
        crate::zoom::size_row(ui, &mut self.settings.interface_size);
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.settings.animations,
            t!("play.animated_background"),
        )
        .on_hover_text(t!("play.aurora_snowfall_and_shooting_stars_pauses"));
        ui.checkbox(
            &mut self.settings.intro,
            t!("play.intro_animation_on_start"),
        );
    }

    fn behavior_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            t!("play.how_arctic_behaves"),
            t!("play.you_can_change_these_any_time_in_settings"),
        );
        ui.add_space(12.0);
        ui.label(RichText::new(t!("play.when_the_game_starts")).color(p.text));
        ui.add_space(4.0);
        let choices = [
            (
                GameStartAction::KeepOpen,
                Icon::Play,
                t!("play.keep_the_launcher_open"),
                t!("play.handy_for_logs_and_switching_instances"),
            ),
            (
                GameStartAction::Minimize,
                Icon::Layers,
                t!("play.minimize_it"),
                t!("play.out_of_the_way_while_you_play_it_comes_back"),
            ),
        ];
        for (action, icon, title, desc) in choices {
            if option_tile(
                ui,
                p,
                icon,
                title,
                desc,
                true,
                self.settings.on_game_start == action,
            ) {
                self.settings.on_game_start = action;
            }
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        if cfg!(windows) {
            ui.checkbox(
                &mut self.settings.tray,
                t!("play.keep_arctic_in_the_system_tray_when_closed"),
            )
            .on_hover_text(t!("play.reopens_instantly_quit_from_the_tray_icon"));
        }
        ui.checkbox(
            &mut self.settings.start_maximized,
            t!("play.start_the_launcher_maximized"),
        );
        ui.checkbox(
            &mut self.settings.check_updates_on_start,
            t!("play.check_for_launcher_updates_on_start"),
        );
    }

    fn account_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            t!("play.add_your_account"),
            t!("play.sign_in_with_the_microsoft_account_that_owns"),
        );
        ui.add_space(12.0);
        if let Some(account) = self.accounts.active() {
            let name = account.username.clone();
            ui.horizontal(|ui| {
                crate::art::icons::draw(
                    ui.painter(),
                    Icon::Check,
                    Rect::from_min_size(ui.cursor().min, vec2(20.0, 20.0)),
                    p.accent,
                );
                ui.add_space(26.0);
                ui.label(
                    RichText::new(tf!("play.signed_in_as", name = name))
                        .size(16.0)
                        .color(p.text),
                );
            });
            ui.add_space(8.0);
            if ui.link(t!("play.add_another_account")).clicked() {
                self.add_account = AddAccount::Choose;
            }
            return;
        }
        let note = if self.msa_configured {
            t!("play.opens_your_browser_or_gives_you_a_code_to")
        } else {
            t!("play.microsoft_sign_in_isnt_available_in_this_2")
        };
        if option_tile(
            ui,
            p,
            Icon::User,
            t!("play.add_account"),
            note,
            self.msa_configured || cfg!(feature = "offline-accounts"),
            false,
        ) {
            self.add_account = AddAccount::Choose;
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(t!("play.your_sign_in_stays_on_this_pc_encrypted"))
                .small()
                .color(p.muted),
        );
    }

    fn memory_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let total = self.onboarding.as_ref().and_then(|o| o.total_mb);
        heading(
            ui,
            p,
            t!("play.memory_and_storage"),
            t!("play.how_much_ram_the_game_may_use_the_suggestion"),
        );
        ui.add_space(14.0);
        let recommended = system::recommended_memory_mb(total);
        let max = total.map_or(16 * 1024, |t| (t as u32).saturating_sub(1024).max(2048));
        ui.spacing_mut().slider_width = WIDTH - 90.0;
        ui.add(
            egui::Slider::new(&mut self.settings.max_memory_mb, 1024..=max)
                .trailing_fill(true)
                .step_by(512.0)
                .custom_formatter(|v, _| format!("{:.1} GB", v / 1024.0)),
        );
        ui.add_space(6.0);
        let detail = match total {
            Some(t) => tf!(
                "play.recommended_gb_this_pc_has_gb",
                value = format!("{:.1}", recommended as f32 / 1024.0),
                value2 = format!("{:.0}", t as f32 / 1024.0)
            ),
            None => tf!(
                "play.recommended_gb",
                value = format!("{:.1}", recommended as f32 / 1024.0)
            ),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(detail).color(p.muted));
            if self.settings.max_memory_mb != recommended && ui.link(t!("play.use_it")).clicked() {
                self.settings.max_memory_mb = recommended;
            }
        });
        if total.is_some_and(|t| u64::from(self.settings.max_memory_mb) > t / 2) {
            ui.label(
                RichText::new(t!("play.thats_over_half_your_ram_your_pc_may_slow")).color(p.warn),
            );
        }
        self.storage_choice(ui, p);
    }

    fn client_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            t!("play.how_the_game_looks"),
            t!("play.the_arctic_client_restyles_minecrafts_menus"),
        );
        ui.add_space(10.0);
        let mut client = self.instance.arctic_mod;
        if ui
            .checkbox(&mut client, t!("play.play_with_the_arctic_client"))
            .changed()
        {
            let id = self.instance.id.clone();
            self.update_instance(&id, |i| i.arctic_mod = client);
        }
        if !client {
            ui.label(
                RichText::new(t!("play.pure_vanilla_no_cosmetics_or_capes_no"))
                    .small()
                    .color(p.warn),
            );
            ui.add_space(6.0);
        }
        ui.add_enabled_ui(client, |ui| {
            ui.add_space(6.0);
            super::client_style::picker(ui, p, &mut self.settings, (WIDTH - 24.0) / 3.0);
            ui.add_space(10.0);
            super::client_style::fancy_toggle(ui, &mut self.settings);
            hint(ui, p, t!("play.a_smooth_sharp_font_and_rounded_buttons"));
        });
        ui.add_space(6.0);
        let mut performance = self.instance.performance;
        if ui
            .checkbox(&mut performance, t!("play.boost_fps_with_performance_mods"))
            .changed()
        {
            let id = self.instance.id.clone();
            self.update_instance(&id, |i| i.performance = performance);
        }
        hint(ui, p, t!("play.adds_sodium_lithium_and_friends_to_vanilla"));
    }

    fn start_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(ui, p, t!("play.what_do_you_want_to_play"), "");
        ui.add_space(10.0);
        let Some((first, others)) = self
            .onboarding
            .as_ref()
            .map(|o| (o.first, o.others.clone()))
        else {
            return;
        };
        let found = match others.as_slice() {
            [one] => tf!(
                "play.found_copies_the_originals_stay_as_they_are",
                one = one
            ),
            [first, second] => tf!(
                "play.found_and_originals_stay_as_they_are",
                first = first,
                second = second
            ),
            [first, rest @ ..] => tf!(
                "play.found_and_more_originals_stay_as_they_are",
                first = first,
                count = rest.len()
            ),
            [] => String::new(),
        };
        let vanilla = if self.instance.arctic_mod {
            t!("play.the_latest_minecraft_release_with_the_arctic")
        } else {
            t!("play.the_latest_minecraft_release_pure_vanilla")
        };
        let mut choices = vec![];
        if !others.is_empty() {
            choices.push((
                FirstPlay::Move,
                Icon::Import,
                t!("play.bring_my_stuff_over"),
                found.as_str(),
            ));
        }
        choices.extend([
            (FirstPlay::Vanilla, Icon::Play, t!("play.vanilla"), vanilla),
            (
                FirstPlay::Modded,
                Icon::Layers,
                t!("play.modded"),
                t!("play.a_fabric_instance_with_one_click_mods_from"),
            ),
            (
                FirstPlay::Later,
                Icon::Gear,
                t!("play.ill_decide_later"),
                t!("play.look_around_first"),
            ),
        ]);
        for (choice, icon, title, desc) in choices {
            if option_tile(ui, p, icon, title, desc, true, first == choice)
                && let Some(o) = &mut self.onboarding
            {
                o.first = choice;
            }
            ui.add_space(6.0);
        }
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.settings.discord_presence,
            t!("play.show_what_youre_playing_on_discord"),
        );
    }

    fn finish_onboarding(&mut self, now: f64) {
        let first = self.onboarding.take().map_or(FirstPlay::Later, |o| o.first);
        self.settings.onboarded = true;
        self.persist_settings();
        match first {
            FirstPlay::Move => {
                self.set_tab(Tab::Instances, now);
                self.open_migrate();
            }
            FirstPlay::Vanilla => {
                self.settings.last_instance = None;
                self.set_tab(Tab::Play, now);
            }
            FirstPlay::Modded => {
                self.set_tab(Tab::Instances, now);
                self.open_create_dialog();
            }
            FirstPlay::Later => self.set_tab(Tab::Play, now),
        }
    }
}

/// Launchers and clients with something to bring over.
fn other_launchers() -> Vec<&'static str> {
    let scan = arctic_core::migrate::scan();
    let mut names: Vec<&'static str> = Vec::new();
    let found = scan
        .instances
        .iter()
        .map(|f| f.launcher)
        .chain(scan.clients.iter().map(|c| c.launcher));
    for launcher in found {
        if !names.contains(&launcher.name()) {
            names.push(launcher.name());
        }
    }
    names
}

fn welcome(ui: &mut egui::Ui, p: &Palette, now: f64) {
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (rect, _) = ui.allocate_exact_size(vec2(120.0, 120.0), Sense::hover());
        let glow = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 28);
        ui.painter().circle_filled(rect.center(), 58.0, glow);
        brand_mark(
            ui.ctx(),
            ui.painter(),
            rect.center(),
            50.0,
            (now * 0.15) as f32,
        );
        ui.ctx().request_repaint();
        ui.add_space(14.0);
        ui.label(
            RichText::new(t!("play.welcome_to_arctic"))
                .size(28.0)
                .strong()
                .color(p.text),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(t!("play.a_fast_lightweight_minecraft_launcher_lets")).color(p.muted),
        );
    });
}

/// A muted line under a switch, lined up with its label.
fn hint(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(HINT_INDENT);
        ui.label(RichText::new(text).small().color(p.muted));
    });
}

fn heading(ui: &mut egui::Ui, p: &Palette, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(24.0).strong().color(p.text));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).color(p.muted));
    }
}

fn progress_dots(ui: &mut egui::Ui, p: &Palette, current: usize) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    let n = Step::ALL.len();
    let gap = 6.0;
    let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
    for i in 0..n {
        let x = rect.left() + i as f32 * (w + gap);
        let bar = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), vec2(w, 4.0));
        let color = if i <= current {
            p.accent
        } else {
            p.surface_hover
        };
        ui.painter().rect_filled(bar, CornerRadius::same(2), color);
    }
}

/// Miniature of a theme's sky and mountains. Returns true when clicked.
fn theme_tile(ui: &mut egui::Ui, p: &Palette, mode: ThemeMode, name: &str, selected: bool) -> bool {
    let size = vec2(
        (WIDTH - 12.0 * (THEMES.len() as f32 - 1.0)) / THEMES.len() as f32,
        118.0,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let target = theme::palette(mode);
    let scene = &target.scene;
    let sky = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - 28.0));
    let painter = ui.painter().with_clip_rect(rect);
    let mut mesh = egui::Mesh::default();
    let top = scene.sky[0];
    let bottom = scene.sky[2];
    mesh.colored_vertex(sky.left_top(), top);
    mesh.colored_vertex(sky.right_top(), top);
    mesh.colored_vertex(sky.right_bottom(), bottom);
    mesh.colored_vertex(sky.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
    let base = sky.bottom();
    for (i, (ridge, _)) in scene.mountains.iter().enumerate() {
        let w = sky.width();
        let peak = sky.left() + w * (0.25 + 0.3 * i as f32);
        let h = sky.height() * (0.55 - 0.12 * i as f32);
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(peak - w * 0.42, base),
                Pos2::new(peak, base - h),
                Pos2::new(peak + w * 0.42, base),
            ],
            *ridge,
            Stroke::NONE,
        ));
    }
    let label = Rect::from_min_max(Pos2::new(rect.left(), sky.bottom()), rect.max);
    painter.rect_filled(
        label,
        CornerRadius::ZERO,
        lerp_color(p.surface, p.surface_hover, hover),
    );
    painter.text(
        label.center(),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(14.0),
        p.text,
    );
    let stroke = if selected {
        Stroke::new(2.0, p.accent)
    } else {
        Stroke::new(1.0, lerp_color(p.card_stroke, p.accent, hover))
    };
    painter.rect_stroke(rect, CornerRadius::ZERO, stroke, StrokeKind::Inside);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
