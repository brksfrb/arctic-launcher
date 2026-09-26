//! Slide-in notifications in the top-right corner.

use eframe::egui::{
    self, Align2, Area, CornerRadius, Frame, Id, Order, RichText, Sense, Stroke, vec2,
};

use crate::art::icons::{self, Icon};
use crate::motion::eased;
use crate::theme::Palette;
use crate::widgets;

const WIDTH: f32 = 320.0;
const SLIDE: f32 = 0.25;
const FADE: f32 = 0.3;
const MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Success,
    Info,
    Error,
}

impl Kind {
    fn lifetime(self) -> f64 {
        match self {
            Kind::Error => 9.0,
            _ => 4.5,
        }
    }
}

/// Optional button on a toast; returned to the app when clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastAction {
    /// Open the Logs tab on this game run.
    ShowLogs(crate::tasks::LaunchId),
}

impl ToastAction {
    fn label(&self) -> &'static str {
        match self {
            ToastAction::ShowLogs(_) => "View logs",
        }
    }
}

struct Toast {
    id: u64,
    kind: Kind,
    title: String,
    body: String,
    action: Option<ToastAction>,
    created: Option<f64>,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    pub fn push(&mut self, kind: Kind, title: impl Into<String>, body: impl Into<String>) {
        self.push_with_action(kind, title, body, None);
    }

    pub fn push_with_action(
        &mut self,
        kind: Kind,
        title: impl Into<String>,
        body: impl Into<String>,
        action: Option<ToastAction>,
    ) {
        self.next_id += 1;
        self.items.push(Toast {
            id: self.next_id,
            kind,
            title: title.into(),
            body: body.into(),
            action,
            created: None,
        });
    }

    /// Draw all toasts; returns the action the user clicked, if any.
    pub fn show(&mut self, ctx: &egui::Context, p: &Palette) -> Option<ToastAction> {
        let now = ctx.input(|i| i.time);
        self.items
            .retain(|t| t.created.is_none_or(|c| now - c < t.kind.lifetime()));
        let mut y = MARGIN;
        let mut dismissed = None;
        let mut clicked = None;
        for toast in &mut self.items {
            let created = *toast.created.get_or_insert(now);
            let age = (now - created) as f32;
            let remaining = toast.kind.lifetime() as f32 - age;
            let enter = eased(now, created, SLIDE);
            let alpha = enter.min((remaining / FADE).clamp(0.0, 1.0));
            let offset = vec2(-MARGIN + (1.0 - enter) * 60.0, y);
            let response = Area::new(Id::new(("toast", toast.id)))
                .order(Order::Foreground)
                .anchor(Align2::RIGHT_TOP, offset)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.multiply_opacity(alpha);
                    match toast_body(ui, p, toast) {
                        Outcome::Dismissed => dismissed = Some(toast.id),
                        Outcome::Action(a) => {
                            clicked = Some(a);
                            dismissed = Some(toast.id);
                        }
                        Outcome::None => {}
                    }
                })
                .response;
            y += response.rect.height() + 8.0;
            ctx.request_repaint();
        }
        if let Some(id) = dismissed {
            self.items.retain(|t| t.id != id);
        }
        clicked
    }
}

enum Outcome {
    None,
    Dismissed,
    Action(ToastAction),
}

fn toast_body(ui: &mut egui::Ui, p: &Palette, toast: &Toast) -> Outcome {
    let (icon, color) = match toast.kind {
        Kind::Success => (Icon::Check, p.accent),
        Kind::Info => (Icon::Info, p.accent),
        Kind::Error => (Icon::Close, p.error),
    };
    let mut outcome = Outcome::None;
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(12.0)
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: p.shadow,
        })
        .show(ui, |ui| {
            ui.set_width(WIDTH);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), 11.0, color.gamma_multiply(0.18));
                icons::draw(ui.painter(), icon, rect.shrink(6.0), color);
                ui.vertical(|ui| {
                    ui.set_width(WIDTH - 70.0);
                    ui.label(RichText::new(&toast.title).strong().color(p.text));
                    if !toast.body.is_empty() {
                        ui.label(RichText::new(&toast.body).small().color(p.muted));
                    }
                    if let Some(action) = &toast.action
                        && ui.link(action.label()).clicked()
                    {
                        outcome = Outcome::Action(action.clone());
                    }
                });
                if widgets::icon_button(ui, p, Icon::Close, "Dismiss").clicked() {
                    outcome = Outcome::Dismissed;
                }
            });
        });
    outcome
}
