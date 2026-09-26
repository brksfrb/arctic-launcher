//! First-run setup: look, account, memory and what to play first. Shown
//! once per profile, on top of the normal window so theme changes preview
//! live behind it.

use arctic_core::settings::ThemeMode;
use arctic_core::system;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Modal, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, vec2,
};

use super::dialogs::option_tile;
use crate::app::{AddAccount, ArcticApp, Tab};
use crate::art::icons::Icon;
use crate::art::{lerp_color, snowflake};
use crate::motion::eased;
use crate::theme::{self, Palette};
use crate::widgets;

const WIDTH: f32 = 500.0;
/// Seconds a step takes to slide in.
const STEP_TRANSITION: f32 = 0.22;
const STEP_SLIDE: f32 = 18.0;
const THEMES: [(ThemeMode, &str); 3] = [
    (ThemeMode::Default, "Aurora"),
    (ThemeMode::Dark, "Dark"),
    (ThemeMode::Light, "Light"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Look,
    Account,
    Memory,
    Client,
    Start,
}

impl Step {
    const ALL: [Step; 6] = [
        Step::Welcome,
        Step::Look,
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
    Vanilla,
    Modded,
    Later,
}

pub struct Onboarding {
    step: Step,
    changed_at: f64,
    total_mb: Option<u64>,
    first: FirstPlay,
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
        let total_mb = system::total_memory_mb();
        self.settings.max_memory_mb = system::recommended_memory_mb(total_mb);
        self.onboarding = Some(Onboarding {
            step: Step::Welcome,
            changed_at: 0.0,
            total_mb,
            first: FirstPlay::Vanilla,
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
                    ui.multiply_opacity(t);
                    ui.add_space((1.0 - t) * STEP_SLIDE);
                    ui.set_min_height(300.0);
                    match step {
                        Step::Welcome => welcome(ui, p, now),
                        Step::Look => self.look_step(ui),
                        Step::Account => self.account_step(ui),
                        Step::Memory => self.memory_step(ui),
                        Step::Client => self.client_step(ui),
                        Step::Start => self.start_step(ui),
                    }
                });
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if step == Step::Welcome {
                        if ui.link("Skip setup").clicked() {
                            finish = true;
                        }
                    } else if ui.button("Back").clicked() {
                        go = Some(-1);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, icon) = match step {
                            Step::Welcome => ("Get started", Icon::Play),
                            Step::Start => ("Start playing", Icon::Check),
                            Step::Account if self.accounts.active().is_none() => {
                                ("Skip for now", Icon::Play)
                            }
                            _ => ("Next", Icon::Play),
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
            "Pick a look",
            "You can change this any time in Settings.",
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for (mode, name) in THEMES {
                if theme_tile(ui, p, mode, name, self.settings.theme == mode) {
                    self.settings.theme = mode;
                }
            }
        });
        ui.add_space(12.0);
        ui.checkbox(&mut self.settings.animations, "Animated background");
        ui.checkbox(&mut self.settings.intro, "Intro animation on start");
    }

    fn account_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            "Add your account",
            "Sign in with the Microsoft account that owns Minecraft.",
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
                    RichText::new(format!("Signed in as {name}"))
                        .size(16.0)
                        .color(p.text),
                );
            });
            ui.add_space(8.0);
            if ui.link("Add another account").clicked() {
                self.add_account = AddAccount::Choose;
            }
            return;
        }
        let note = if self.msa_configured {
            "Opens your browser, or gives you a code to enter on any device."
        } else {
            "Microsoft sign-in isn't available in this build."
        };
        if option_tile(
            ui,
            p,
            Icon::User,
            "Add account",
            note,
            self.msa_configured || cfg!(feature = "offline-accounts"),
            false,
        ) {
            self.add_account = AddAccount::Choose;
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new("Your sign-in stays on this PC. Arctic never sees your password.")
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
            "Memory for Minecraft",
            "How much RAM the game may use. The suggestion suits most setups.",
        );
        ui.add_space(14.0);
        let recommended = system::recommended_memory_mb(total);
        let max = total.map_or(16 * 1024, |t| (t as u32).saturating_sub(1024).max(2048));
        ui.spacing_mut().slider_width = WIDTH - 90.0;
        // The dialog is the same color as the default slider track.
        ui.visuals_mut().widgets.inactive.bg_fill = p.surface_hover;
        ui.add(
            egui::Slider::new(&mut self.settings.max_memory_mb, 1024..=max)
                .trailing_fill(true)
                .step_by(512.0)
                .custom_formatter(|v, _| format!("{:.1} GB", v / 1024.0)),
        );
        ui.add_space(6.0);
        let detail = match total {
            Some(t) => format!(
                "Recommended: {:.1} GB (this PC has {:.0} GB)",
                recommended as f32 / 1024.0,
                t as f32 / 1024.0
            ),
            None => format!("Recommended: {:.1} GB", recommended as f32 / 1024.0),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(detail).color(p.muted));
            if self.settings.max_memory_mb != recommended && ui.link("Use it").clicked() {
                self.settings.max_memory_mb = recommended;
            }
        });
        if total.is_some_and(|t| u64::from(self.settings.max_memory_mb) > t / 2) {
            ui.label(
                RichText::new("That's over half your RAM; your PC may slow down while playing.")
                    .color(p.warn),
            );
        }
    }

    fn client_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(
            ui,
            p,
            "Your in-game style",
            "The Arctic Client restyles Minecraft's menus. Press Right Shift in game for HUD widgets and capes.",
        );
        ui.add_space(12.0);
        super::client_style::picker(ui, p, &mut self.settings, (WIDTH - 24.0) / 3.0);
    }

    fn start_step(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        heading(ui, p, "What do you want to play?", "");
        ui.add_space(10.0);
        let Some(first) = self.onboarding.as_ref().map(|o| o.first) else {
            return;
        };
        let choices = [
            (
                FirstPlay::Vanilla,
                Icon::Play,
                "Vanilla",
                "The latest Minecraft release, with the Arctic Client.",
            ),
            (
                FirstPlay::Modded,
                Icon::Layers,
                "Modded",
                "A Fabric instance with one-click mods from Modrinth.",
            ),
            (
                FirstPlay::Later,
                Icon::Gear,
                "I'll decide later",
                "Look around first.",
            ),
        ];
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
            "Show what you're playing on Discord",
        );
    }

    fn finish_onboarding(&mut self, now: f64) {
        let first = self.onboarding.take().map_or(FirstPlay::Later, |o| o.first);
        self.settings.onboarded = true;
        self.persist_settings();
        match first {
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

fn welcome(ui: &mut egui::Ui, p: &Palette, now: f64) {
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (rect, _) = ui.allocate_exact_size(vec2(120.0, 120.0), Sense::hover());
        let glow = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 28);
        ui.painter().circle_filled(rect.center(), 58.0, glow);
        snowflake(
            ui.painter(),
            rect.center(),
            46.0,
            (now * 0.15) as f32,
            p.accent,
        );
        ui.ctx().request_repaint();
        ui.add_space(14.0);
        ui.label(
            RichText::new("Welcome to Arctic")
                .size(28.0)
                .strong()
                .color(p.text),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("A fast, lightweight Minecraft launcher.\nLet's get you set up. It takes about a minute.")
                .color(p.muted),
        );
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
    let size = vec2((WIDTH - 24.0) / 3.0, 118.0);
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
