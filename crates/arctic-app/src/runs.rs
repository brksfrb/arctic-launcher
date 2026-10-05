//! Games started from the launcher. Several can run at once, one per
//! instance (two copies of one instance would fight over its worlds and
//! files); each keeps its own log, can be stopped on its own, and gets its
//! crash explained when it ends badly.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use arctic_core::crash::{self, Diagnosis};
use arctic_core::launch::logparse::{Level, LogLine};
use arctic_core::launch::startup::Stage;
use arctic_core::launch::{GameEvent, GameHandle};
use arctic_core::settings::{CrashReports, GameStartAction};
use eframe::egui;

use crate::app::{ArcticApp, LogSource};
use crate::motion::RateMeter;
use crate::tasks::{Event, LaunchId, ProgressSnapshot, Tasks};
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
        /// What the game's log says it's doing.
        stage: Stage,
        since: Instant,
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
    /// The Minecraft version it runs (Vanilla can run several at once).
    pub version: String,
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
    /// The crash, ready to send if the player agrees.
    crash_report: Option<crash::Report>,
    /// Loading in the background with its window hidden, for a Play that shows it at once.
    pub standby: bool,
    /// What this standby was made for (instance, version, account, settings, mods).
    pub standby_key: u64,
    /// Dropped while still preparing: end it as soon as it starts.
    pub standby_cancelled: bool,
}

impl Run {
    pub fn is_active(&self) -> bool {
        !matches!(self.state, RunState::Ended)
    }

    /// Running for the player (a standby isn't: nobody pressed Play yet).
    pub fn is_playing(&self) -> bool {
        self.is_active() && !self.standby
    }

    /// How long ago the game was started.
    pub fn started_ago(&self) -> Option<std::time::Duration> {
        self.started.elapsed().ok()
    }

    /// The game process, once it's started and until it exits.
    pub fn game(&self) -> Option<&GameHandle> {
        match &self.state {
            RunState::Starting { game, .. } | RunState::Running { game } => Some(game),
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

    /// The crash as a report, from the game log's end and the crash report it wrote.
    fn crash_report(&self, title: &str) -> crash::Report {
        let lines: Vec<&str> = self.log.iter().map(|l| l.text.as_str()).collect();
        let written = newest_crash_report(&self.game_dir, self.started);
        crash::Report::build(&self.version, "", title, &lines, written.as_deref())
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
    pub fn start(
        &mut self,
        instance_id: &str,
        version: &str,
        title: String,
        game_dir: PathBuf,
    ) -> LaunchId {
        self.next_id += 1;
        self.list
            .retain(|r| r.is_active() || r.instance_id != instance_id);
        self.list.push(Run {
            id: self.next_id,
            instance_id: instance_id.to_owned(),
            version: version.to_owned(),
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
            crash_report: None,
            standby: false,
            standby_key: 0,
            standby_cancelled: false,
        });
        self.next_id
    }

    pub fn get(&self, id: LaunchId) -> Option<&Run> {
        self.list.iter().find(|r| r.id == id)
    }

    pub fn get_mut(&mut self, id: LaunchId) -> Option<&mut Run> {
        self.list.iter_mut().find(|r| r.id == id)
    }

    /// The latest run of an instance (active or finished), not counting a standby.
    pub fn for_instance(&self, instance_id: &str) -> Option<&Run> {
        self.list
            .iter()
            .rev()
            .find(|r| r.instance_id == instance_id && !r.standby)
    }

    /// The game loading in the background, if there is one.
    pub fn standby(&self) -> Option<&Run> {
        self.list.iter().rev().find(|r| r.standby && r.is_active())
    }

    /// The newest running copy of an instance, of `version` when given.
    pub fn active_for(&self, instance_id: &str, version: Option<&str>) -> Option<&Run> {
        self.list.iter().rev().find(|r| {
            r.is_playing() && r.instance_id == instance_id && version.is_none_or(|v| r.version == v)
        })
    }

    pub fn active(&self) -> impl Iterator<Item = &Run> {
        self.list.iter().filter(|r| r.is_playing())
    }

    pub fn any_active(&self) -> bool {
        self.active().next().is_some()
    }

    /// A game process is up (not just preparing).
    pub fn any_game(&self) -> bool {
        self.list.iter().any(|r| r.game().is_some() && !r.standby)
    }

    pub fn instance_active(&self, instance_id: &str) -> bool {
        self.for_instance(instance_id).is_some_and(Run::is_playing)
    }

    /// The newest run, for "View logs" defaults.
    pub fn latest(&self) -> Option<&Run> {
        self.list.iter().rev().find(|r| !r.standby)
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
        if self.runs.get(id).is_none() {
            return;
        }
        match result {
            Ok(game) => {
                let Some(run) = self.runs.get_mut(id) else {
                    return;
                };
                run.state = RunState::Starting {
                    game,
                    stage: Stage::Java,
                    since: Instant::now(),
                };
                if run.standby
                    && let RunState::Starting { game, .. } = &run.state
                {
                    if run.standby_cancelled {
                        game.terminate();
                    } else {
                        game.hide_windows();
                    }
                }
                let early = std::mem::take(&mut run.early);
                // Replay events from a game that was faster than our bookkeeping.
                for event in early {
                    self.on_game_event(id, event, ctx);
                }
            }
            Err(e) => {
                if let Some(run) = self.runs.get_mut(id) {
                    run.state = RunState::Ended;
                }
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
                if let RunState::Starting { game, .. } =
                    std::mem::replace(&mut run.state, RunState::Ended)
                {
                    run.state = RunState::Running { game };
                }
                if self.settings.on_game_start == GameStartAction::Minimize
                    && !self.minimized_for_game
                    && !run.standby
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.minimized_for_game = true;
                }
            }
            GameEvent::Output(lines) => {
                if let RunState::Starting { stage, .. } = &mut run.state {
                    *stage = lines.iter().fold(*stage, |s, l| s.advance(&l.text));
                }
                run.push_log(lines);
            }
            GameEvent::Exited { code } => {
                run.state = RunState::Ended;
                if run.standby {
                    // A background copy nobody asked for yet: no crash toast, no log noise.
                    // One that dies right away more than once isn't tried again.
                    let early = run
                        .started
                        .elapsed()
                        .is_ok_and(|d| d < crate::standby::EARLY_EXIT);
                    if early && code != Some(0) && !run.standby_cancelled {
                        self.standby_fails += 1;
                    }
                    return;
                }
                // What the game changed goes to the profile's other instances.
                let (dirs, shared) = (self.dirs.clone(), self.settings.shared);
                std::thread::spawn(move || {
                    if let Err(e) = arctic_core::shared::sync(&dirs, shared, None) {
                        log::warn!("sharing between instances: {e}");
                    }
                });
                // The game threw its unfinished recording away itself unless it crashed or
                // was killed; either way nothing is writing it now (unless another copy of
                // the instance still runs in the same folder).
                let game_dir = run.game_dir.clone();
                if !self
                    .runs
                    .list
                    .iter()
                    .any(|other| other.id != id && other.is_active() && other.game_dir == game_dir)
                {
                    std::thread::spawn(move || {
                        arctic_core::replays::clear_temp(&game_dir);
                    });
                }
                let Some(run) = self.runs.get_mut(id) else {
                    return;
                };
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
                let crashed = matches!(code, Some(c) if c != 0 && !watchdog);
                if crashed {
                    let report =
                        run.crash_report(diagnosis.as_ref().map_or("", |d| d.title.as_str()));
                    self.offer_crash_report(id, report);
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

    /// What to do with a crash report, by the player's setting: nothing,
    /// send it, or ask with a button on a toast.
    fn offer_crash_report(&mut self, id: LaunchId, report: crash::Report) {
        match self.settings.crash_reports {
            CrashReports::Never => {}
            CrashReports::Always => self.tasks.send_crash_report(report),
            CrashReports::Ask => {
                if let Some(run) = self.runs.get_mut(id) {
                    run.crash_report = Some(report);
                }
                self.toasts.push_with_action(
                    Kind::Info,
                    "Help fix this crash?",
                    "Send the log to Arctic. Your name, folders and sign-in details are taken out first.",
                    Some(ToastAction::SendCrash(id)),
                );
            }
        }
    }

    /// The player said yes to sending this run's crash report.
    pub(crate) fn send_crash_report(&mut self, id: LaunchId) {
        if let Some(report) = self.runs.get_mut(id).and_then(|r| r.crash_report.take()) {
            self.tasks.send_crash_report(report);
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

impl Tasks {
    /// Send a crash report in the background; says so when it's done.
    pub fn send_crash_report(&self, report: crash::Report) {
        self.run(move |t| {
            let base = arctic_core::cosmetics::base_url();
            let result = arctic_core::cosmetics::send_crash_report(&base, &report)
                .map_err(|e| e.to_string());
            t.send(Event::CrashReportSent(result));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_run_per_instance_and_old_logs_make_way() {
        let mut runs = Runs::default();
        let a = runs.start("vanilla", "26.3", "Vanilla · 26.3".into(), PathBuf::new());
        let b = runs.start("pack", "1.21.6", "Pack · 1.21.6".into(), PathBuf::new());
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
        let a2 = runs.start("vanilla", "26.3", "Vanilla · 26.3".into(), PathBuf::new());
        assert!(
            runs.get(a).is_none(),
            "the old finished run of the same instance goes"
        );
        assert_eq!(runs.for_instance("vanilla").unwrap().id, a2);
        assert!(runs.get(b).is_some());
        assert_eq!(runs.latest().unwrap().id, a2);
    }

    #[test]
    fn vanilla_runs_tell_versions_apart() {
        let mut runs = Runs::default();
        let a = runs.start("vanilla", "26.3", "Vanilla · 26.3".into(), PathBuf::new());
        let b = runs.start("vanilla", "1.8.9", "Vanilla · 1.8.9".into(), PathBuf::new());
        assert_eq!(runs.active_for("vanilla", Some("26.3")).unwrap().id, a);
        assert_eq!(runs.active_for("vanilla", Some("1.8.9")).unwrap().id, b);
        assert!(runs.active_for("vanilla", Some("1.21.1")).is_none());
        assert_eq!(runs.active_for("vanilla", None).unwrap().id, b);
        runs.get_mut(b).unwrap().state = RunState::Ended;
        assert_eq!(
            runs.active_for("vanilla", None).unwrap().id,
            a,
            "an older copy still running counts"
        );
    }

    #[test]
    fn logs_are_capped() {
        let mut runs = Runs::default();
        let id = runs.start("x", "1.0", "X".into(), PathBuf::new());
        let run = runs.get_mut(id).unwrap();
        run.push_log((0..LOG_LINES + 10).map(|i| LogLine {
            level: Level::Info,
            text: i.to_string(),
        }));
        assert_eq!(run.log.len(), LOG_LINES);
        assert_eq!(run.log.front().unwrap().text, "10");
    }
}
