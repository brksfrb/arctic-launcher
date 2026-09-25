//! Logs tab: live game output and the launcher's own log, with level
//! filters, search, copy and auto-scroll.
//!
//! The filtered view is cached as line indices and only rebuilt when new
//! lines arrive or the filter changes; rendering reads just the visible rows.

use arctic_core::launch::logparse::{Level, LogLine};
use eframe::egui::{self, CornerRadius, RichText, Stroke, TextStyle};

use crate::app::{ArcticApp, LogSource};
use crate::art::icons::Icon;
use crate::logbook;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

const LEVELS: [(Level, &str); 3] = [
    (Level::Debug, "All"),
    (Level::Warn, "Warnings"),
    (Level::Error, "Errors"),
];

/// What a cached filtered view was computed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogViewKey {
    source: LogSource,
    min_level: Level,
    query: String,
    revision: u64,
}

impl ArcticApp {
    pub(crate) fn logs_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Logs",
            "Live output from Minecraft and the launcher.",
        );
        // Toolbar first: it may change the source, filter or clear the log,
        // and the view below must be built from the state it leaves behind.
        let copy_requested = self.logs_toolbar(ui);
        ui.add_space(8.0);
        if let Some((total, lines)) = logbook::snapshot_if_changed(self.launcher_log_seen) {
            self.launcher_log = lines;
            self.launcher_log_seen = total;
        }
        let source = self.log_source;
        let key = LogViewKey {
            source,
            min_level: self.log_min_level,
            query: self.log_search.trim().to_lowercase(),
            revision: match source {
                LogSource::Game => self.game_log_rev,
                LogSource::Launcher => self.launcher_log_seen,
            },
        };
        if self.log_cache.as_ref().map(|(k, _)| k) != Some(&key) {
            let indices = self.filter_log(&key);
            self.log_cache = Some((key, indices));
        }
        let indices = self
            .log_cache
            .as_ref()
            .map(|(_, i)| i.clone())
            .unwrap_or_default();
        let line = |i: usize| match source {
            LogSource::Game => self.game_log.get(i),
            LogSource::Launcher => self.launcher_log.get(i),
        };
        if copy_requested {
            let text: Vec<&str> = indices
                .iter()
                .filter_map(|&i| line(i))
                .map(|l| l.text.as_str())
                .collect();
            ui.ctx().copy_text(text.join(
                "
",
            ));
            let count = text.len();
            self.toasts
                .push(Kind::Info, format!("Copied {count} lines"), "");
        }
        let line = |i: usize| match source {
            LogSource::Game => self.game_log.get(i),
            LogSource::Launcher => self.launcher_log.get(i),
        };
        console(ui, p, &indices, &line, self.log_follow, source);
    }

    fn filter_log(&self, key: &LogViewKey) -> Vec<usize> {
        let keep = |l: &LogLine| {
            l.level >= key.min_level
                && (key.query.is_empty() || l.text.to_lowercase().contains(&key.query))
        };
        match key.source {
            LogSource::Game => self
                .game_log
                .iter()
                .enumerate()
                .filter(|(_, l)| keep(l))
                .map(|(i, _)| i)
                .collect(),
            LogSource::Launcher => self
                .launcher_log
                .iter()
                .enumerate()
                .filter(|(_, l)| keep(l))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    /// Returns true when "Copy" was clicked.
    fn logs_toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let p = self.palette();
        let mut copy = false;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.log_source, LogSource::Game, "Minecraft");
            ui.selectable_value(&mut self.log_source, LogSource::Launcher, "Launcher");
            ui.separator();
            for (level, name) in LEVELS {
                ui.selectable_value(&mut self.log_min_level, level, name);
            }
            ui.separator();
            ui.add(
                widgets::text_field(&mut self.log_search)
                    .hint_text("Filter…")
                    .desired_width(180.0),
            );
            ui.checkbox(&mut self.log_follow, "Auto-scroll");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.log_source == LogSource::Game
                    && widgets::icon_button(ui, p, Icon::Trash, "Clear").clicked()
                {
                    self.game_log.clear();
                    self.game_log_rev += 1;
                }
                let file = match self.log_source {
                    LogSource::Game => self
                        .dirs
                        .logs()
                        .join(format!("game-{}.log", self.instance.id)),
                    LogSource::Launcher => self.dirs.launcher_logs().join("launcher.log"),
                };
                if file.is_file()
                    && widgets::icon_button(ui, p, Icon::Document, "Open log file").clicked()
                {
                    let _ = open::that_detached(&file);
                }
                if widgets::icon_button(ui, p, Icon::Copy, "Copy shown lines").clicked() {
                    copy = true;
                }
            });
        });
        copy
    }
}

fn console<'a>(
    ui: &mut egui::Ui,
    p: &Palette,
    indices: &[usize],
    line: &dyn Fn(usize) -> Option<&'a LogLine>,
    follow: bool,
    source: LogSource,
) {
    let fill = if p.dark {
        p.bg.gamma_multiply(0.85)
    } else {
        egui::Color32::from_white_alpha(230)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let height = ui.available_height().max(240.0);
            if indices.is_empty() {
                ui.set_min_height(height);
                ui.centered_and_justified(|ui| {
                    let msg = match source {
                        LogSource::Game => "No output yet. Launch the game to see its log here.",
                        LogSource::Launcher => "Nothing logged yet.",
                    };
                    ui.label(RichText::new(msg).color(p.muted));
                });
                return;
            }
            let row_height = ui.text_style_height(&TextStyle::Monospace);
            egui::ScrollArea::both()
                .auto_shrink(false)
                .max_height(height)
                .stick_to_bottom(follow)
                .show_rows(ui, row_height, indices.len(), |ui, range| {
                    for l in indices[range].iter().filter_map(|&i| line(i)) {
                        let color = match l.level {
                            Level::Error => p.error,
                            Level::Warn => p.warn,
                            Level::Debug => p.muted,
                            Level::Info => p.text,
                        };
                        ui.add(
                            egui::Label::new(RichText::new(&l.text).monospace().color(color))
                                .extend(),
                        );
                    }
                });
        });
}
