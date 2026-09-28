//! Settings → "Voice chat": proximity voice with other Arctic players on
//! the same server. Off until turned on.

use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::theme::Palette;

#[derive(Default)]
pub struct VoiceSettingsUi {
    /// Microphones and outputs (listed once, refreshed on demand).
    devices: Option<(Vec<String>, Vec<String>)>,
}

impl ArcticApp {
    pub(crate) fn voice_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        ui.label(
            RichText::new(
                "Talk with other Arctic players near you on the same server: voices get quieter \
                 with distance and come from where the player stands. Audio goes directly between \
                 players; the Arctic server only tells launchers who else is on that server.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        let v = &mut self.settings.voice;
        ui.checkbox(&mut v.enabled, "Voice chat on");
        if !v.enabled {
            return;
        }
        ui.label(RichText::new(self.voice.status()).small().color(p.accent));
        ui.add_space(4.0);
        let v = &mut self.settings.voice;
        crate::widgets::field_row(ui, |ui| {
            ui.label("Talk");
            ui.selectable_value(&mut v.push_to_talk, false, "When I speak");
            ui.selectable_value(&mut v.push_to_talk, true, "Push to talk");
        });
        if v.push_to_talk {
            ui.label(
                RichText::new("Hold V in game to talk (change it in the Arctic menu → Voice).")
                    .small()
                    .color(p.muted),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label("Sensitivity");
                ui.add(
                    egui::Slider::new(&mut v.threshold_db, -70.0..=-20.0)
                        .custom_formatter(|db, _| format!("{db:.0} dB")),
                )
                .on_hover_text("Lower picks up quieter speech (and more background noise).");
            });
            if let Some(level) = self.voice.mic_level() {
                let t = ((level + 70.0) / 50.0).clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(t)
                        .desired_width(220.0)
                        .text("microphone"),
                );
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let v = &mut self.settings.voice;
        ui.horizontal(|ui| {
            ui.label("Volume");
            let mut pct = (v.volume * 100.0).round() as i32;
            if ui
                .add(egui::Slider::new(&mut pct, 0..=200).suffix("%"))
                .changed()
            {
                v.volume = pct as f32 / 100.0;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Microphone");
            let mut pct = (v.mic_gain * 100.0).round() as i32;
            if ui
                .add(egui::Slider::new(&mut pct, 0..=200).suffix("%"))
                .changed()
            {
                v.mic_gain = pct as f32 / 100.0;
            }
        });
        let (inputs, outputs) = self
            .voice_ui
            .devices
            .get_or_insert_with(crate::voice::devices)
            .clone();
        let v = &mut self.settings.voice;
        device_combo(
            ui,
            "voice_input",
            "Microphone device",
            &mut v.input,
            &inputs,
        );
        device_combo(ui, "voice_output", "Output device", &mut v.output, &outputs);
        if ui.small_button("Refresh devices").clicked() {
            self.voice_ui.devices = None;
        }
        let v = &mut self.settings.voice;
        ui.checkbox(&mut v.friends_only, "Only friends")
            .on_hover_text("Hear and be heard only by your friends.");
        ui.add_enabled(
            !v.friends_only,
            egui::Checkbox::new(
                &mut v.simple_voice_chat,
                "Also talk with Simple Voice Chat players",
            ),
        )
        .on_hover_text(
            "On servers that run the Simple Voice Chat plugin, connect to it too, so players \
             using that mod hear you and you hear them. Not with \"Only friends\".",
        );
        if !v.muted.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} player(s) muted", v.muted.len())).color(p.muted),
                );
                if ui.small_button("Unmute all").clicked() {
                    v.muted.clear();
                }
            });
        }
    }
}

fn device_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut Option<String>,
    names: &[String],
) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(value.clone().unwrap_or_else(|| "System default".into()))
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(value, None, "System default");
                for n in names {
                    ui.selectable_value(value, Some(n.clone()), n);
                }
            });
    });
}
