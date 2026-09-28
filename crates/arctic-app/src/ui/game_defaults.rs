//! Settings → "Game settings for new instances": FOV, render distance, GUI
//! scale, volumes, autojump… (or everything, copied from an instance you
//! set up), written into each new instance on its first start. Shareable
//! as a code.

use arctic_core::game_defaults::{self, COMMON, Defaults, Kind};
use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme::Palette;
use crate::toasts::Kind as Toast;
use crate::widgets;

#[derive(Default)]
pub struct DefaultsUi {
    loaded: Option<Defaults>,
    /// Instance picked for copying from / applying to.
    instance: Option<String>,
    show_all: bool,
}

impl ArcticApp {
    pub(crate) fn game_defaults_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        if self.game_defaults.loaded.is_none() {
            self.game_defaults.loaded = Some(Defaults::load(&self.dirs).unwrap_or_default());
        }
        let mut d = self.game_defaults.loaded.clone().unwrap_or_default();
        let before = d.clone();
        ui.label(
            RichText::new(
                "Every new instance starts with these. Pick the ones you always change; \
                 the rest stay at Minecraft's defaults.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        egui::Grid::new("game_defaults")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for c in COMMON {
                    common_row(ui, p, &mut d, c);
                    ui.end_row();
                }
            });
        let extra: Vec<(String, String)> = d
            .values
            .iter()
            .filter(|(k, _)| !COMMON.iter().any(|c| c.key == k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !extra.is_empty() {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Plus {} more (keys, video, chat…)", extra.len()))
                        .color(p.text),
                );
                let label = if self.game_defaults.show_all {
                    "Hide"
                } else {
                    "Show"
                };
                if ui.small_button(label).clicked() {
                    self.game_defaults.show_all = !self.game_defaults.show_all;
                }
            });
            if self.game_defaults.show_all {
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .id_salt("defaults_extra")
                    .show(ui, |ui| {
                        for (k, v) in &extra {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(k).monospace().small().color(p.muted));
                                ui.label(RichText::new(v).monospace().small().color(p.text));
                                if widgets::icon_button(
                                    ui,
                                    p,
                                    Icon::Close,
                                    "Leave at the game's default",
                                )
                                .clicked()
                                {
                                    d.values.remove(k);
                                }
                            });
                        }
                    });
            }
        }
        ui.add_space(8.0);
        self.defaults_actions(ui, p, &mut d);
        if d != before {
            match d.save(&self.dirs) {
                Ok(()) => self.game_defaults.loaded = Some(d),
                Err(e) => {
                    self.toasts
                        .push(Toast::Error, "Couldn't save the defaults", e.to_string())
                }
            }
        }
    }

    fn defaults_actions(&mut self, ui: &mut egui::Ui, p: &Palette, d: &mut Defaults) {
        let mut all = vec![(self.instance.id.clone(), self.instance.name.clone())];
        all.extend(
            self.custom_instances
                .iter()
                .map(|i| (i.id.clone(), i.name.clone())),
        );
        let picked = self
            .game_defaults
            .instance
            .clone()
            .filter(|id| all.iter().any(|(i, _)| i == id))
            .unwrap_or_else(|| all[0].0.clone());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Instance").color(p.muted));
            let name = all.iter().find(|(i, _)| *i == picked).map_or("", |(_, n)| n.as_str());
            egui::ComboBox::from_id_salt("defaults_instance")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (id, name) in &all {
                        let mut sel = picked.clone();
                        if ui.selectable_value(&mut sel, id.clone(), name).clicked() {
                            self.game_defaults.instance = Some(sel);
                        }
                    }
                });
            let instance = self.instance_by_id(&picked).cloned();
            if let Some(instance) = instance {
                let game_dir = instance.game_dir(&self.dirs);
                if widgets::button(ui, p, Some(Icon::Import), "Copy its settings", false)
                    .on_hover_text("Everything you set in that instance becomes the default")
                    .clicked()
                {
                    match Defaults::capture(&game_dir) {
                        Ok(c) => {
                            let n = c.values.len();
                            *d = c;
                            self.toasts.push(Toast::Success, format!("Copied {n} settings"), format!("From {}", instance.name));
                        }
                        Err(_) => self.toasts.push(Toast::Info, "Nothing to copy yet", "Play that instance once first."),
                    }
                }
                if widgets::button(ui, p, None, "Apply to it", false)
                    .on_hover_text("Puts these settings into that instance now (the rest of its settings stay)")
                    .clicked()
                {
                    if self.runs.instance_active(&instance.id) {
                        self.toasts.push(Toast::Info, "Close that game first", "It saves its own settings when it quits.");
                    } else {
                        match d.apply_to(&game_dir, instance.version.as_deref()) {
                            Ok(n) => self.toasts.push(Toast::Success, format!("Applied {n} settings"), format!("To {}", instance.name)),
                            Err(e) => self.toasts.push(Toast::Error, "Couldn't apply them", e.to_string()),
                        }
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            if !d.is_empty() && widgets::button(ui, p, Some(Icon::Share), "Share", false).clicked()
            {
                self.open_share_defaults();
            }
            if widgets::button(ui, p, Some(Icon::Import), "Use a code", false).clicked() {
                self.open_share_import();
            }
            if !d.is_empty()
                && widgets::button(ui, p, None, "Clear", false)
                    .on_hover_text("New instances start with Minecraft's own defaults")
                    .clicked()
            {
                d.values.clear();
            }
        });
    }

    /// An imported code replaced the defaults on disk.
    pub(crate) fn reload_game_defaults(&mut self) {
        self.game_defaults.loaded = None;
    }
}

/// One common setting: whether it's set, and its value.
fn common_row(ui: &mut egui::Ui, p: &Palette, d: &mut Defaults, c: &game_defaults::Common) {
    let current = d.values.get(c.key).cloned();
    let mut on = current.is_some();
    if ui.checkbox(&mut on, c.label).changed() {
        if on {
            d.values.insert(c.key.to_owned(), c.game_default.to_owned());
        } else {
            d.values.remove(c.key);
        }
    }
    let Some(value) = current.filter(|_| on) else {
        ui.label(
            RichText::new(format!("game default ({})", c.kind.show(c.game_default)))
                .small()
                .color(p.muted),
        );
        ui.label("");
        return;
    };
    let num = value.parse::<f64>().unwrap_or(0.0);
    let new = match c.kind {
        Kind::Toggle => {
            let mut b = value == "true";
            let label = if b { "On" } else { "Off" };
            ui.checkbox(&mut b, label);
            b.to_string()
        }
        Kind::Whole { min, max } => {
            let mut n = num.round() as i64;
            ui.add(egui::Slider::new(&mut n, min..=max));
            n.to_string()
        }
        Kind::Percent => {
            let mut n = (num * 100.0).round() as i64;
            ui.add(egui::Slider::new(&mut n, 0..=100).suffix("%"));
            game_defaults::format_float(n as f64 / 100.0)
        }
        Kind::Fov => {
            let mut deg = game_defaults::fov_degrees(num);
            ui.add(egui::Slider::new(&mut deg, 30..=110).suffix("°"));
            if deg == game_defaults::fov_degrees(num) {
                value.clone()
            } else {
                game_defaults::fov_stored(deg)
            }
        }
        Kind::GuiScale => {
            let mut n = num.round() as i64;
            ui.add(egui::Slider::new(&mut n, 0..=6).custom_formatter(|v, _| {
                if v == 0.0 {
                    "Auto".into()
                } else {
                    format!("{v}")
                }
            }));
            n.to_string()
        }
    };
    ui.label("");
    if new != value {
        d.values.insert(c.key.to_owned(), new);
    }
}
