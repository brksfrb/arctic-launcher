//! Games started from the launcher. Several can run at once, one per
//! instance (two copies of one instance would fight over its worlds and
//! files); each keeps its own log, can be stopped on its own, and gets its
//! crash explained when it ends badly.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::SystemTime;

use arctic_core::crash::{self, Diagnosis};
use arctic_core::launch::logparse::{Level, LogLine};
use arctic_core::launch::{GameEvent, GameHandle};
use arctic_core::settings::GameStartAction;
use eframe::egui;

use crate::app::{ArcticApp, LogSource};
use crate::motion::RateMeter;
use crate::tasks::{LaunchId, ProgressSnapshot};
use crate::toasts::{Kind, ToastAction};

/// Game log lines kept in memory per run (older ones are dropped).
const LOG_LINES: usize = 20_000;
/// Log lines searched for a crash cause (the end is where it is).
const DIAGNOSE_LINES: usize = 4000;

pub enum RunState {
    Preparing {
        progress: ProgressSnapshot,
        meter: RateMeter,
    },
    /// Process started, waiting for the game window.
    Starting {
        game: GameHandle,
    },
    Running {
        game: GameHandle,
    },
    /// Finished; its log stays readable until the instance runs again.
    Ended,
}

pub struct Run {
    pub id: LaunchId,
    pub instance_id: String,
    /// "Vanilla · 26.3", for lists and the Logs tab.
    pub title: String,
    pub state: RunState,
    pub log: VecDeque<LogLine>,
    /// Bumped whenever `log` changes (for the Logs tab cache).
    pub log_rev: u64,
    pub game_dir: PathBuf,
    started: SystemTime,
    /// Game events that overtook their `Launched` event.
    early: Vec<GameEvent>,
}

impl Run {
    pub fn is_active(&self) -> bool {
        !matches!(self.state, RunState::Ended)
    }

    /// The game process, once it's started and until it exits.
    pub fn game(&self) -> Option<&GameHandle> {
        match &self.state {
            RunState::Starting { game } | RunState::Running { game } => Some(game),
            _ => None,
        }
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
        self.log_rev += 1;
    }

    fn push_log(&mut self, lines: impl IntoIterator<Item = LogLine>) {
        self.log_rev += 1;
        self.log.extend(lines);
        let excess = self.log.len().saturating_sub(LOG_LINES);
        self.log.drain(..excess);
    }

    /// Minecraft's own shutdown watchdog fired: quitting took too long
    /// (typically network threads hanging without internet).
    fn shutdown_watchdog_fired(&self) -> bool {
        self.log
            .iter()
            .rev()
            .take(400)
            .any(|l| l.text.contains("Client shutdown from post-main"))
    }

    /// Why the game crashed, from its log and the crash report it wrote.
    fn diagnose(&self) -> Option<Diagnosis> {
        let skip = self.log.len().saturating_sub(DIAGNOSE_LINES);
        let mut text: String = self
            .log
            .iter()
            .skip(skip)
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(report) = newest_crash_report(&self.game_dir, self.started) {
            text.push('\n');
            text.push_str(&report);
        }
        crash::diagnose(&text)
    }
}

/// The crash report Minecraft wrote after `since`, if any.
fn newest_crash_report(game_dir: &std::path::Path, since: SystemTime) -> Option<String> {
    let entries = std::fs::read_dir(game_dir.join("crash-reports")).ok()?;
    let newest = entries
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            (modified >= since).then(|| (modified, e.path()))
        })
        .max_by_key(|(modified, _)| *modified)?;
    std::fs::read_to_string(newest.1).ok()
}

#[derive(Default)]
pub struct Runs {
    pub list: Vec<Run>,
    next_id: LaunchId,
}

impl Runs {
    /// Start tracking a launch; an older, finished run of the instance goes.
    pub fn start(&mut self, instance_id: &str, title: String, game_dir: PathBuf) -> LaunchId {
        self.next_id += 1;
        self.list
            .retain(|r| r.is_active() || r.instance_id != instance_id);
        self.list.push(Run {
            id: self.next_id,
            instance_id: instance_id.to_owned(),
            title,
            state: RunState::Preparing {
                progress: ProgressSnapshot {
                    stage: "Starting".into(),
                    ..ProgressSnapshot::default()
                },
                meter: RateMeter::default(),
            },
            log: VecDeque::new(),
            log_rev: 0,
            game_dir,
            started: SystemTime::now(),
            early: Vec::new(),
        });
        self.next_id
    }

    pub fn get(&self, id: LaunchId) -> Option<&Run> {
        self.list.iter().find(|r| r.id == id)
    }

    pub fn get_mut(&mut self, id: LaunchId) -> Option<&mut Run> {
        self.list.iter_mut().find(|r| r.id == id)
    }

    /// The latest run of an instance (active or finished).
    pub fn for_instance(&self, instance_id: &str) -> Option<&Run> {
        self.list
            .iter()
            .rev()
            .find(|r| r.instance_id == instance_id)
    }

    pub fn active(&self) -> impl Iterator<Item = &Run> {
        self.list.iter().filter(|r| r.is_active())
    }

    pub fn any_active(&self) -> bool {
        self.active().next().is_some()
    }

    /// A game process is up (not just preparing).
    pub fn any_game(&self) -> bool {
        self.list.iter().any(|r| r.game().is_some())
    }

    pub fn instance_active(&self, instance_id: &str) -> bool {
        self.for_instance(instance_id).is_some_and(Run::is_active)
    }

    /// The newest run, for "View logs" defaults.
    pub fn latest(&self) -> Option<&Run> {
        self.list.last()
    }

    /// Forget every run (profile switch; only allowed when none is active).
    pub fn clear(&mut self) {
        self.list.clear();
    }
}

impl ArcticApp {
    pub(crate) fn on_launch_progress(
        &mut self,
        id: LaunchId,
        snapshot: ProgressSnapshot,
        now: f64,
    ) {
        if let Some(Run {
            state: RunState::Preparing { progress, meter },
            ..
        }) = self.runs.get_mut(id)
        {
            meter.push(now, snapshot.bytes_done);
            *progress = snapshot;
        }
    }

    pub(crate) fn on_launched(
        &mut self,
        id: LaunchId,
        result: Result<GameHandle, String>,
        ctx: &egui::Context,
    ) {
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        match result {
            Ok(game) => {
                run.state = RunState::Starting { game };
                let early = std::mem::take(&mut run.early);
                // Replay events from a game that was faster than our bookkeeping.
                for event in early {
                    self.on_game_event(id, event, ctx);
                }
            }
            Err(e) => {
                run.state = RunState::Ended;
                self.toasts.push(Kind::Error, "Launch failed", e);
            }
        }
    }

    pub(crate) fn on_game_event(&mut self, id: LaunchId, event: GameEvent, ctx: &egui::Context) {
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        if matches!(run.state, RunState::Preparing { .. }) {
            run.early.push(event);
            return;
        }
        match event {
            GameEvent::WindowReady => {
                if let RunState::Starting { game } =
                    std::mem::replace(&mut run.state, RunState::Ended)
                {
                    run.state = RunState::Running { game };
                }
                if self.settings.on_game_start == GameStartAction::Minimize
                    && !self.minimized_for_game
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.minimized_for_game = true;
                }
            }
            GameEvent::Output(lines) => run.push_log(lines),
            GameEvent::Exited { code } => {
                run.state = RunState::Ended;
                let watchdog = run.shutdown_watchdog_fired();
                let title = run.title.clone();
                let diagnosis = matches!(code, Some(c) if c != 0 && !watchdog)
                    .then(|| run.diagnose())
                    .flatten();
                if let Some(d) = &diagnosis {
                    run.push_log([LogLine {
                        level: Level::Error,
                        text: format!("──── Arctic: {} — {} ────", d.title, d.detail),
                    }]);
                }
                // Back to the launcher once the last game is closed.
                if self.minimized_for_game && !self.runs.any_game() {
                    self.minimized_for_game = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.report_exit(id, &title, code, watchdog, diagnosis);
            }
        }
    }

    fn report_exit(
        &mut self,
        id: LaunchId,
        title: &str,
        code: Option<i32>,
        watchdog: bool,
        diagnosis: Option<Diagnosis>,
    ) {
        match code {
            Some(0) => {}
            Some(_) if watchdog => self.toasts.push(
                Kind::Info,
                format!("{title} closed"),
                "It was slow to shut down, which happens without internet.",
            ),
            Some(c) => {
                let (heading, body) = match diagnosis {
                    Some(d) => (format!("{title} crashed: {}", d.title), d.detail),
                    None => (
                        format!("{title} closed unexpectedly"),
                        format!("Exit code {c}."),
                    ),
                };
                self.toasts.push_with_action(
                    Kind::Error,
                    heading,
                    body,
                    Some(ToastAction::ShowLogs(id)),
                );
            }
            None => self
                .toasts
                .push(Kind::Info, format!("{title} was stopped"), ""),
        }
    }

    /// Note a line in a run's log (launch banners and the like).
    pub(crate) fn run_note(&mut self, id: LaunchId, text: String) {
        if let Some(run) = self.runs.get_mut(id) {
            run.push_log([LogLine {
                level: Level::Info,
                text,
            }]);
        }
    }

    /// Show a run's log in the Logs tab.
    pub(crate) fn show_run_log(&mut self, id: LaunchId, now: f64) {
        self.log_source = LogSource::Game(id);
        self.set_tab(crate::app::Tab::Logs, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_run_per_instance_and_old_logs_make_way() {
        let mut runs = Runs::default();
        let a = runs.start("vanilla", "Vanilla · 26.3".into(), PathBuf::new());
        let b = runs.start("pack", "Pack · 1.21.6".into(), PathBuf::new());
        assert!(runs.instance_active("vanilla") && runs.instance_active("pack"));
        assert_eq!(runs.active().count(), 2);
        assert!(!runs.any_game(), "still preparing");
        runs.get_mut(a).unwrap().state = RunState::Ended;
        assert!(!runs.instance_active("vanilla"));
        assert_eq!(
            runs.for_instance("vanilla").unwrap().id,
            a,
            "finished runs keep their log"
        );
        let a2 = runs.start("vanilla", "Vanilla · 26.3".into(), PathBuf::new());
        assert!(
            runs.get(a).is_none(),
            "the old finished run of the same instance goes"
        );
        assert_eq!(runs.for_instance("vanilla").unwrap().id, a2);
        assert!(runs.get(b).is_some());
        assert_eq!(runs.latest().unwrap().id, a2);
    }

    #[test]
    fn logs_are_capped() {
        let mut runs = Runs::default();
        let id = runs.start("x", "X".into(), PathBuf::new());
        let run = runs.get_mut(id).unwrap();
        run.push_log((0..LOG_LINES + 10).map(|i| LogLine {
            level: Level::Info,
            text: i.to_string(),
        }));
        assert_eq!(run.log.len(), LOG_LINES);
        assert_eq!(run.log.front().unwrap().text, "10");
    }
}
