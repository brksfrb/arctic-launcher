//! Settings → "Voice chat": proximity voice with other Arctic players on
//! the same server. Off until turned on.

use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::theme::Palette;
use crate::{t, tf};
#[derive(Default)]
pub struct VoiceSettingsUi {
    /// Microphones and outputs (listed once, refreshed on demand).
    devices: Option<(Vec<String>, Vec<String>)>,
}

impl ArcticApp {
    pub(crate) fn voice_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        ui.label(
            RichText::new(t!("social.talk_with_other_arctic_players_near_you_on"))
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        let v = &mut self.settings.voice;
        ui.checkbox(&mut v.enabled, t!("social.voice_chat_on"));
        if !v.enabled {
            return;
        }
        ui.label(RichText::new(self.voice.status()).small().color(p.accent));
        ui.add_space(4.0);
        let v = &mut self.settings.voice;
        crate::widgets::field_row(ui, |ui| {
            ui.label(t!("social.talk"));
            ui.selectable_value(&mut v.push_to_talk, false, t!("social.when_i_speak"));
            ui.selectable_value(&mut v.push_to_talk, true, t!("social.push_to_talk"));
        });
        if v.push_to_talk {
            ui.label(
                RichText::new(t!("social.hold_v_in_game_to_talk_change_it_in_the"))
                    .small()
                    .color(p.muted),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label(t!("social.sensitivity"));
                ui.add(
                    egui::Slider::new(&mut v.threshold_db, -70.0..=-20.0)
                        .custom_formatter(|db, _| format!("{db:.0} dB")),
                )
                .on_hover_text(t!("social.lower_picks_up_quieter_speech_and_more"));
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
            ui.label(t!("social.volume"));
            let mut pct = (v.volume * 100.0).round() as i32;
            if ui
                .add(egui::Slider::new(&mut pct, 0..=200).suffix("%"))
                .changed()
            {
                v.volume = pct as f32 / 100.0;
            }
        });
        ui.horizontal(|ui| {
            ui.label(t!("social.microphone"));
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
            t!("social.microphone_device"),
            &mut v.input,
            &inputs,
        );
        device_combo(
            ui,
            "voice_output",
            t!("social.output_device"),
            &mut v.output,
            &outputs,
        );
        if ui.small_button(t!("social.refresh_devices")).clicked() {
            self.voice_ui.devices = None;
        }
        let v = &mut self.settings.voice;
        ui.checkbox(&mut v.friends_only, t!("social.only_friends"))
            .on_hover_text(t!("social.hear_and_be_heard_only_by_your_friends"));
        ui.add_enabled(
            !v.friends_only,
            egui::Checkbox::new(
                &mut v.simple_voice_chat,
                t!("social.also_talk_with_simple_voice_chat_players"),
            ),
        )
        .on_hover_text(t!("social.on_servers_that_run_the_simple_voice_chat"));
        if !v.muted.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tf!("social.player_s_muted", count = v.muted.len()))
                        .color(p.muted),
                );
                if ui.small_button(t!("social.unmute_all")).clicked() {
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
            .selected_text(
                value
                    .clone()
                    .unwrap_or_else(|| t!("social.system_default").into()),
            )
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(value, None, t!("social.system_default"));
                for n in names {
                    ui.selectable_value(value, Some(n.clone()), n);
                }
            });
    });
}
