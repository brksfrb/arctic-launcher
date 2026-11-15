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
use crate::{t, tf};
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
    /// How Java was started (flags, classpath files), for the crash report.
    pub java: Option<String>,
    /// The player requested a stop; the resulting exit is not a crash.
    killed: bool,
}

impl Run {
    pub fn is_active(&self) -> bool {
        !matches!(self.state, RunState::Ended)
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

    /// End the game and remember the player's intent before handling its exit.
    pub fn force_close(&mut self) {
        if let Some(game) = self.game() {
            game.kill();
            self.killed = true;
        }
    }

    /// Windows kills exit with 1, Unix kills with 137. Both mean "was stopped"
    /// when requested by the launcher, including requests seen only by the watcher.
    fn reported_exit_code(&mut self, code: Option<i32>, killed: bool) -> Option<i32> {
        self.killed |= killed;
        if self.killed { None } else { code }
    }

    pub fn push_log(&mut self, lines: impl IntoIterator<Item = LogLine>) {
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
    fn crash_report(&self, title: &str, code: Option<i32>) -> crash::Report {
        let mut lines: Vec<&str> = self.log.iter().map(|l| l.text.as_str()).collect();
        let mixin =
            fresh_diagnostic_sidecar(&self.game_dir, self.started, "arctic-mixin-failure.txt");
        if let Some(context) = &mixin {
            lines.push(context);
        }
        let hang = fresh_hang_dump(&self.game_dir, self.started);
        if let Some(dump) = &hang {
            lines.push(dump);
        }
        // Short freezes the in-game watchdog recorded this run (the same report, consent and scrubbing).
        let freezes = fresh_freeze_files(&self.game_dir, self.started);
        // The game's own log file often says more than what it printed (a mod that quits quietly writes
        // only there), and Java writes a file of its own when it dies.
        let own_log = latest_log_tail(&self.game_dir, self.started);
        if !own_log.is_empty() {
            lines.push("---- logs/latest.log ----");
            lines.extend(own_log.iter().map(String::as_str));
        }
        // Keep both bounded snapshots and the fatal hang summary within the report's 8 KB log tail.
        // Newest last, so its client stacks have priority if the report's budget changes.
        for freeze in freezes.iter().rev() {
            lines.push(freeze);
        }
        let hang_at_exit = freezes.first().and_then(|text| freeze_at_exit(text));
        if let Some(summary) = &hang_at_exit {
            lines.push(summary);
        }
        let written = newest_crash_report(&self.game_dir, self.started)
            .or_else(|| newest_hs_err(&self.game_dir, self.started));
        // What the launcher itself logged (a failed sign-in, a download, a launch step) often explains
        // a crash the game's own log doesn't.
        let launcher = crate::logbook::snapshot_if_changed(0)
            .map(|(_, lines)| lines)
            .unwrap_or_default();
        let launcher: Vec<&str> = launcher.iter().map(|l| l.text.as_str()).collect();
        // How it ended: a game that wrote nothing at all is otherwise a report with nothing in it.
        let java = self.java.as_deref().map(|j| match code {
            Some(c) => format!(
                "{j}\n(exit code {})\n{}",
                crash::exit_description(c),
                self.run_summary(written.is_some())
            ),
            None => format!("{j}\n{}", self.run_summary(written.is_some())),
        });
        crash::Report::build(
            &self.version,
            "",
            title,
            &lines,
            &launcher,
            written.as_deref(),
            java.as_deref(),
        )
    }

    /// How the run went, for a crash that left no error text: how long the game lived, how much it
    /// printed and whether it wrote a log of its own. Tells a game that died on start from one that
    /// died mid-play.
    fn run_summary(&self, has_report: bool) -> String {
        let lived = self.started.elapsed().map_or(0, |d| d.as_secs());
        let latest = self.game_dir.join("logs").join("latest.log");
        let own = match std::fs::metadata(&latest) {
            Err(_) => "none".to_owned(),
            Ok(m) if m.modified().is_ok_and(|t| t < self.started) => {
                "left over from an earlier run".to_owned()
            }
            Ok(m) => format!("{count} bytes", count = m.len()), // i18n-ignore
        };
        format!(
            "run: ended after {lived} s; {count} lines printed; logs/latest.log: {own}; crash report or hs_err: {value}", // i18n-ignore: diagnostics
            lived = lived,
            count = self.log.len(),
            own = own,
            value = if has_report { "yes" } else { "none" }
        )
    }

    /// Memory a big pack was started with, if it was too little for it: the warning to show.
    pub fn memory_warning(&self) -> Option<String> {
        let given = crash::max_memory_mb(self.java.as_deref()?)?;
        let mods = arctic_core::launch::mod_count(&self.game_dir.join("mods"));
        crash::memory_warning(given, mods, arctic_core::system::total_memory_mb())
    }

    /// Why the game crashed, from its log and the crash report it wrote.
    #[cfg(test)]
    fn diagnose(&self) -> Option<Diagnosis> {
        arctic_core::crash_conflicts::diagnose_in(&self.crash_text(), &self.game_dir.join("mods"))
    }

    fn exit_diagnosis(&self, code: Option<i32>, shutdown_watchdog: bool) -> Option<Diagnosis> {
        let code = code.filter(|c| *c != 0 && !shutdown_watchdog)?;
        arctic_core::crash_conflicts::diagnose_exit_in(
            &self.crash_text(),
            code,
            &self.game_dir.join("mods"),
        )
    }

    /// The end of the game's log and the crash report it wrote, as one text.
    fn crash_text(&self) -> String {
        let skip = self.log.len().saturating_sub(DIAGNOSE_LINES);
        let mut text: String = self
            .log
            .iter()
            .skip(skip)
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(report) = newest_crash_report(&self.game_dir, self.started)
            .or_else(|| newest_hs_err(&self.game_dir, self.started))
        {
            text.push('\n');
            text.push_str(&report);
        }
        let own_log = latest_log_tail(&self.game_dir, self.started);
        if !own_log.is_empty() {
            text.push('\n');
            text.push_str(&own_log.join("\n"));
        }
        if let Some(hang) = fresh_hang_dump(&self.game_dir, self.started) {
            text.push_str(&format!("\n{hang}"));
        }
        let freezes = fresh_freeze_files(&self.game_dir, self.started);
        for freeze in freezes.iter().rev() {
            text.push_str(&format!("\n{freeze}"));
        }
        if let Some(summary) = freezes.first().and_then(|f| freeze_at_exit(f)) {
            text.push_str(&format!("\n{summary}"));
        }
        if let Some((_, launcher)) = crate::logbook::snapshot_if_changed(0) {
            for line in launcher.iter().filter(|l| {
                l.text
                    .contains("is more than this computer can spare: using")
            }) {
                text.push_str(&format!("\n{}", line.text));
            }
        }
        text
    }
}

/// The sidecar survives long sessions and resource reloads that push the dump out of the UI log.
fn fresh_hang_dump(game_dir: &std::path::Path, since: SystemTime) -> Option<String> {
    fresh_diagnostic_sidecar(game_dir, since, "arctic-hang.txt")
}

fn fresh_diagnostic_sidecar(
    game_dir: &std::path::Path,
    since: SystemTime,
    name: &str,
) -> Option<String> {
    let path = game_dir.join(name);
    let meta = std::fs::metadata(&path).ok()?;
    if meta.modified().ok()? < since || meta.len() > 4 * 1024 * 1024 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// The newest short-freeze files this run wrote. Two 3 KB heads leave room for the hang-at-exit
/// summary in the report's 8 KB log tail; the full files remain in `logs/arctic-freezes/`.
fn fresh_freeze_files(game_dir: &std::path::Path, since: SystemTime) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(game_dir.join("logs").join("arctic-freezes")) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, std::path::PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("freeze-") && name.ends_with(".txt")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
                return None;
            }
            let modified = meta.modified().ok()?;
            (modified >= since).then(|| (modified, e.path()))
        })
        .collect();
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    files
        .into_iter()
        .take(2)
        .filter_map(|(_, path)| {
            let text = std::fs::read_to_string(&path).ok()?;
            let name = path.file_name()?.to_string_lossy().into_owned();
            let mut head: String = text.lines().take(120).collect::<Vec<_>>().join("\n");
            if head.len() > 3 * 1024 {
                let mut end = 3 * 1024;
                while !head.is_char_boundary(end) {
                    end -= 1;
                }
                head.truncate(end);
                // i18n-ignore: crash-report diagnostic, not UI text
                head.push_str("\n[snapshot truncated; full file saved locally]");
            }
            Some(format!("---- logs/arctic-freezes/{name} ----\n{head}"))
        })
        .collect()
}

/// An unfinished snapshot is a lower bound on the hang, not its final duration.
/// Only call this for the newest freeze: a later recovered freeze rules out an older one at exit.
fn freeze_at_exit(text: &str) -> Option<String> {
    let duration = text.lines().find_map(|line| {
        line.strip_prefix("duration: ")?
            .strip_suffix(" (still frozen when written)")
    })?;
    let top = text.lines().find_map(|line| {
        line.strip_prefix("Arctic: game froze for ")?
            .split_once(", mostly in ")
            .map(|(_, frame)| {
                frame
                    .split(" (Arctic code:")
                    .next()
                    .unwrap_or(frame)
                    .split(" [")
                    .next()
                    .unwrap_or(frame)
            })
    });
    Some(match top {
        Some(frame) => tf!(
            "tasks.hang_at_exit_client_hung_for_at_least_in",
            duration = duration,
            frame = frame
        ),
        None => tf!(
            "tasks.hang_at_exit_client_hung_for_at_least",
            duration = duration
        ),
    })
}

/// Lines of `logs/latest.log` that are looked at.
const LATEST_LOG_LINES: usize = 120;

/// Recent registry errors plus the end of this run's own log. Forge's detailed ID warnings
/// precede its crash report/mod inventory, often falling outside the ordinary 120-line tail.
fn latest_log_tail(game_dir: &std::path::Path, since: SystemTime) -> Vec<String> {
    let path = game_dir.join("logs").join("latest.log");
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .is_ok_and(|modified| modified >= since);
    if !fresh {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let skip = lines.len().saturating_sub(LATEST_LOG_LINES);
    let registry = crash::registry_failure_evidence(&lines[..skip].join("\n"));
    registry
        .lines()
        .chain(lines[skip..].iter().copied())
        .map(str::to_owned)
        .collect()
}

/// The file Java writes when it dies (`hs_err_pid<number>.log`, in the game's folder), if this run made one.
fn newest_hs_err(game_dir: &std::path::Path, since: SystemTime) -> Option<String> {
    let newest = std::fs::read_dir(game_dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("hs_err_pid") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            (modified >= since).then(|| (modified, e.path()))
        })
        .max_by_key(|(modified, _)| *modified)?;
    std::fs::read_to_string(newest.1)
        .ok()
        .map(|text| crash::trim_hs_err(&text))
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
                    stage: t!("tasks.starting").into(),
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
            java: None,
            killed: false,
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

    /// The newest running copy of an instance, of `version` when given.
    pub fn active_for(&self, instance_id: &str, version: Option<&str>) -> Option<&Run> {
        self.list.iter().rev().find(|r| {
            r.is_active() && r.instance_id == instance_id && version.is_none_or(|v| r.version == v)
        })
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
                // The same text as the toast, including every cause. This used to
                // reach the screen and never `logs/launcher.log`.
                log::error!("Launch failed: {e}");
                self.toasts.push(Kind::Error, t!("tasks.launch_failed"), e);
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
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.minimized_for_game = true;
                    self.trim_now(ctx);
                }
            }
            GameEvent::Output(lines) => {
                if let RunState::Starting { stage, .. } = &mut run.state {
                    *stage = lines.iter().fold(*stage, |s, l| s.advance(&l.text));
                }
                run.push_log(lines);
            }
            GameEvent::Exited { code, killed } => {
                let code = run.reported_exit_code(code, killed);
                run.state = RunState::Ended;
                self.bridge.forget_launch(id);
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
                let instance_id = run.instance_id.clone();
                // Time played: from the start to the exit, for games that really started.
                if let Ok(played) = run.started.elapsed() {
                    if let Err(e) = arctic_core::playtime::record(
                        &self.dirs,
                        &instance_id,
                        played.as_secs(),
                        arctic_core::auth::now_secs(),
                    ) {
                        log::warn!("play time: {e}");
                    }
                    // What the game itself recorded: stretches in servers, worlds and menus.
                    if let Err(e) = arctic_core::playtime::ingest_game_log(
                        &self.dirs,
                        &instance_id,
                        &run.game_dir,
                    ) {
                        log::warn!("play time from the game: {e}");
                    }
                    self.playtime = arctic_core::playtime::Stats::load(&self.dirs);
                }
                let diagnosis = run.exit_diagnosis(code, watchdog);
                if let Some(d) = &diagnosis {
                    run.push_log([LogLine {
                        level: Level::Error,
                        text: format!("──── Arctic: {} — {} ────", d.title, d.detail),
                    }]);
                    if d.repair {
                        arctic_core::launch::files::request_repair(&self.dirs);
                        // The loader failed before any mod ran: the next start also goes without the
                        // client's startup agent, the one thing Arctic adds to the loader's start.
                        arctic_core::launch::startup_cache::turn_off(&run.game_dir);
                    }
                }
                let crashed = matches!(code, Some(c) if c != 0 && !watchdog);
                let native_place = crashed
                    .then(|| arctic_core::stability::place(&run.crash_text()))
                    .flatten();
                if crashed {
                    run.push_log([LogLine {
                        level: Level::Info,
                        text: "Need help with this crash? Join our Discord: https://discord.arcticlauncher.com"
                            .into(),
                    }]);
                    let report =
                        run.crash_report(diagnosis.as_ref().map_or("", |d| d.title.as_str()), code);
                    self.offer_crash_report(id, report);
                }
                // Native crashes in several different places point at the computer, not a mod.
                if let Some(place) = native_place {
                    match arctic_core::stability::record(
                        &self.dirs,
                        arctic_core::auth::now_secs(),
                        &place,
                    ) {
                        Ok(Some(hint)) => self.toasts.push(
                            Kind::Info,
                            t!("tasks.this_may_be_a_problem_with_your_pc"),
                            hint,
                        ),
                        Ok(None) => {}
                        Err(e) => log::warn!("crash history: {e}"),
                    }
                }
                // Back to the launcher once the last game is closed.
                if self.minimized_for_game && !self.runs.any_game() {
                    self.minimized_for_game = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.report_exit(id, &title, &instance_id, code, watchdog, diagnosis);
            }
        }
    }

    /// Player-facing text for a diagnosis. Crash reports keep the English title from core
    /// so uploaded groups stay stable; the toast is translated.
    fn localized_diagnosis(d: &Diagnosis) -> (String, String) {
        match d.title.as_str() {
            "Minecraft ran out of memory and froze" => {
                let mut detail = t!("tasks.the_game_froze_with_memory_pressure").to_string();
                if d.detail.contains("Arctic capped the memory") {
                    detail.push(' ');
                    detail.push_str(t!("tasks.arctic_capped_the_memory"));
                }
                if let Some((_, hint)) = d.detail.split_once("\n\n") {
                    detail.push_str("\n\n");
                    detail.push_str(hint);
                }
                (
                    t!("tasks.minecraft_ran_out_of_memory_and_froze").into(),
                    detail,
                )
            }
            "The game froze and was closed" => {
                let detail = if d.detail == "The run contains an unrecovered watchdog freeze." {
                    t!("tasks.the_run_contains_an_unrecovered_watchdog_freeze").into()
                } else {
                    d.detail.clone()
                };
                (t!("tasks.the_game_froze_and_was_closed").into(), detail)
            }
            _ => (d.title.clone(), d.detail.clone()),
        }
    }

    fn report_exit(
        &mut self,
        id: LaunchId,
        title: &str,
        instance_id: &str,
        code: Option<i32>,
        watchdog: bool,
        diagnosis: Option<Diagnosis>,
    ) {
        match code {
            Some(0) => {}
            Some(_) if watchdog => self.toasts.push(
                Kind::Info,
                tf!("tasks.closed", title = title),
                t!("tasks.it_was_slow_to_shut_down_which_happens"),
            ),
            Some(c) => {
                // A crash the launcher can fix gets a "Fix it" button instead of the logs one.
                let fix = diagnosis.as_ref().and_then(|d| d.fix.clone());
                let (heading, body) = match diagnosis {
                    Some(d) => {
                        let (title2, body) = Self::localized_diagnosis(&d);
                        (tf!("tasks.crashed", title = title, title2 = title2), body)
                    }
                    None => (
                        tf!("tasks.closed_unexpectedly", title = title),
                        tf!("tasks.exit_code", c = c),
                    ),
                };
                let action = match fix {
                    Some(fix) => ToastAction::Fix {
                        instance: instance_id.to_owned(),
                        fix,
                    },
                    None => ToastAction::ShowLogs(id),
                };
                self.toasts
                    .push_with_action(Kind::Error, heading, body, Some(action));
            }
            None => self
                .toasts
                .push(Kind::Info, tf!("tasks.was_stopped", title = title), ""),
        }
    }

    /// What to do with a crash report, by the player's setting: nothing,
    /// send it, or ask with a button on a toast.
    fn offer_crash_report(&mut self, id: LaunchId, mut report: crash::Report) {
        report.contact = self
            .settings
            .crash_contact
            .trim()
            .chars()
            .take(80)
            .collect();
        match self.settings.crash_reports {
            CrashReports::Never => {}
            CrashReports::Always => self.tasks.send_crash_report(report),
            CrashReports::Ask => {
                if let Some(run) = self.runs.get_mut(id) {
                    run.crash_report = Some(report);
                }
                self.toasts.push_with_action(
                    Kind::Info,
                    t!("tasks.help_fix_this_crash"),
                    t!("tasks.send_the_log_to_arctic_your_name_folders_and"),
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

impl ArcticApp {
    /// The proxy covers Minecraft's login services for every game, but server
    /// connections only through the Arctic Client (Fabric/Quilt and Vanilla
    /// instances that have it on). Say so when a game that skips it starts.
    pub(crate) fn warn_if_proxy_misses_servers(
        &mut self,
        instance: &arctic_core::instances::Instance,
        version_id: &str,
    ) {
        if arctic_core::proxy::ProxySettings::load(&self.dirs)
            .active()
            .is_none()
        {
            return;
        }
        let forge = matches!(
            instance.loader.kind(),
            Some(
                arctic_core::loaders::LoaderKind::Forge
                    | arctic_core::loaders::LoaderKind::NeoForge
            )
        );
        let client = if forge {
            arctic_core::arctic_mod::supports_forge(version_id)
                && instance.loader.kind() == Some(arctic_core::loaders::LoaderKind::Forge)
        } else {
            arctic_core::arctic_mod::supports(version_id)
        };
        if instance.arctic_mod && client {
            return;
        }
        self.toasts.push(
            crate::toasts::Kind::Error,
            t!("tasks.the_proxy_wont_cover_this_games_servers"),
            t!("tasks.only_minecrafts_login_goes_through_it"),
        );
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
    use std::time::Duration;

    #[test]
    fn registry_warnings_before_latest_log_tail_reach_diagnosis_and_upload() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("logs")).unwrap();
        std::fs::create_dir_all(dir.path().join("crash-reports")).unwrap();
        let error = "[Render thread/WARN] [net.minecraftforge.registries.ForgeRegistry/REGISTRIES]: Registry minecraft:item: Object did not get ID it asked for. Name: example:widget Expected: 24 Got: 25";
        let log = format!(
            "{}\n{error}\n{}",
            "startup noise\n".repeat(200),
            "mod inventory line\n".repeat(500)
        );
        std::fs::write(dir.path().join("logs/latest.log"), log).unwrap();
        let written = include_str!("../../arctic-core/tests/fixtures/crash-314-registry-sync.log");
        std::fs::write(dir.path().join("crash-reports/crash.txt"), written).unwrap();
        let mut runs = Runs::default();
        let id = runs.start("pack", "1.20.1", "Pack".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.started = SystemTime::now() - Duration::from_secs(60);
        let tail = latest_log_tail(dir.path(), run.started);
        assert!(tail.iter().any(|line| line == error));
        assert_eq!(tail.len(), LATEST_LOG_LINES + 1);
        let diagnosis = run.exit_diagnosis(Some(-1), false).unwrap();
        assert_eq!(diagnosis.title, "Forge registry IDs did not sync");
        assert_eq!(diagnosis.mods, ["example"]);
        let report = run.crash_report(&diagnosis.title, Some(-1));
        assert!(report.log.contains("---- registry failure context ----"));
        assert!(
            report
                .log
                .contains("Name: example:widget Expected: 24 Got: 25")
        );
        assert!(report.log.contains("matched: java.lang.RuntimeException:"));
        assert!(report.log.len() < 48 * 1024);
        // An old latest.log must not contribute attribution or evidence to the next run.
        assert!(
            latest_log_tail(dir.path(), SystemTime::now() + Duration::from_secs(60)).is_empty()
        );
    }

    #[test]
    fn a_force_close_is_not_a_crash() {
        let mut runs = Runs::default();
        let id = runs.start("x", "1.0", "X".into(), PathBuf::new());
        let run = runs.get_mut(id).unwrap();
        run.force_close();
        assert!(!run.killed, "nothing to kill while still preparing");
        // Crash-like output must not override an intentional stop.
        run.push_log([LogLine {
            level: Level::Error,
            text: "java.lang.OutOfMemoryError: Java heap space".into(),
        }]);
        // The UI remembers its request even before the watcher sees it.
        run.killed = true;
        for code in [Some(1), Some(137), Some(0), None] {
            let reported = run.reported_exit_code(code, false);
            assert_eq!(reported, None, "a requested stop: {code:?}");
            assert!(run.exit_diagnosis(reported, false).is_none());
        }
    }

    #[test]
    fn a_watcher_force_close_is_not_a_crash() {
        let mut runs = Runs::default();
        let id = runs.start("x", "1.0", "X".into(), PathBuf::new());
        let run = runs.get_mut(id).unwrap();
        // A direct handle kill or launcher shutdown bypassed Run::force_close.
        let code = run.reported_exit_code(Some(1), true);
        assert_eq!(code, None);
        assert!(run.killed);
        assert!(run.exit_diagnosis(code, false).is_none());
    }

    #[test]
    fn a_nonzero_exit_without_force_close_is_still_a_crash() {
        let mut runs = Runs::default();
        let id = runs.start("x", "1.0", "X".into(), PathBuf::new());
        let run = runs.get_mut(id).unwrap();
        run.push_log([LogLine {
            level: Level::Error,
            text: "java.lang.OutOfMemoryError: Java heap space".into(),
        }]);
        for code in [1, 137] {
            let reported = run.reported_exit_code(Some(code), false);
            assert_eq!(reported, Some(code));
            assert!(run.exit_diagnosis(reported, false).is_some());
        }
        assert!(run.exit_diagnosis(Some(0), false).is_none());
        assert!(run.exit_diagnosis(Some(1), true).is_none());
    }

    #[test]
    fn mixin_sidecar_survives_log_eviction_and_ignores_previous_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arctic-mixin-failure.txt");
        let started = SystemTime::now() - Duration::from_secs(1);
        std::fs::write(&path, "[Arctic mixin] class=example.Item; role=preloader\n[Arctic mixin] tick()V\n[Arctic mixin mod] last_mod 1.2.3").unwrap();
        assert!(
            fresh_diagnostic_sidecar(dir.path(), started, "arctic-mixin-failure.txt").is_some()
        );
        assert!(
            fresh_diagnostic_sidecar(
                dir.path(),
                SystemTime::now() + Duration::from_secs(1),
                "arctic-mixin-failure.txt"
            )
            .is_none()
        );
        let mut runs = Runs::default();
        let id = runs.start("pack", "26.3", "Pack".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.started = started;
        let report = run.crash_report("Crash", Some(1));
        assert!(report.log.contains("class=example.Item; role=preloader"));
        assert!(report.log.contains("last_mod 1.2.3"));
        std::fs::File::create(&path)
            .unwrap()
            .set_len(5 * 1024 * 1024)
            .unwrap();
        assert!(
            fresh_diagnostic_sidecar(dir.path(), started, "arctic-mixin-failure.txt").is_none()
        );
    }

    #[test]
    fn this_runs_freeze_files_ride_along_with_the_crash_report() {
        let dir = tempfile::tempdir().unwrap();
        let freezes = dir.path().join("logs").join("arctic-freezes");
        std::fs::create_dir_all(&freezes).unwrap();
        let before = SystemTime::now() - std::time::Duration::from_secs(60);
        std::fs::write(
            freezes.join("freeze-20260101-000000.txt"),
            "Arctic: game froze for 5.1s",
        )
        .unwrap();
        std::fs::write(freezes.join("notes.txt"), "not a freeze file").unwrap();
        std::fs::write(freezes.join("freeze-partial.tmp"), "unfinished write").unwrap();
        std::fs::create_dir(freezes.join("freeze-directory.txt")).unwrap();
        let found = fresh_freeze_files(dir.path(), before);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("game froze for 5.1s"));
        // A freeze from an earlier run is not this run's.
        let after = SystemTime::now() + std::time::Duration::from_secs(60);
        assert!(fresh_freeze_files(dir.path(), after).is_empty());
    }

    const UNFINISHED_FREEZE: &str = "Arctic: game froze for 3.1s, mostly in org.lwjgl.vulkan.VK10.vkQueuePresentKHR(Native Method) [not in a world (menu/loading) - may be normal]\nstarted: 2026-11-13 12:00:00.000\nduration: 3.12s (still frozen when written)\nsamples: 3 client-thread stacks\n== Distinct client-thread stacks (count of samples) ==\n[3x] stack 1\n    at org.lwjgl.vulkan.VK10.vkQueuePresentKHR(Native Method)\n    at example.Game.render(Game.java:42)\n== All other threads (captured once, at the first sample) ==\nthread \"worker\" WAITING\n";

    #[test]
    fn report_301_keeps_the_unknown_exit_title_in_the_uploaded_report() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir(&mods).unwrap();
        for version in ["1.13.9+kotlin.2.3.10", "1.14.2+kotlin.2.4.21"] {
            let file = mods.join(format!("fabric-language-kotlin-{version}.jar"));
            let mut jar = zip::ZipWriter::new(std::fs::File::create(file).unwrap());
            jar.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            write!(jar, r#"{{"id":"fabric-language-kotlin","name":"Fabric Language Kotlin","version":"{version}"}}"#).unwrap();
            jar.finish().unwrap();
        }
        let mut runs = Runs::default();
        let id = runs.start("pvp-map", "1.21.11", "Pvp map".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.started = SystemTime::now() - Duration::from_secs(51);
        run.java = Some("Java java-runtime-delta -Xmx2048M".into());
        run.push_log(
            include_str!("../../arctic-core/tests/fixtures/crash-301-kotlin-hang.log")
                .lines()
                .map(|line| LogLine {
                    level: Level::Info,
                    text: line.into(),
                }),
        );
        let diagnosis = run.exit_diagnosis(Some(-805306369), false).unwrap();
        assert_eq!(
            diagnosis.title,
            "Game exited with code -805306369 (0xCFFFFFFF)"
        );
        assert!(diagnosis.detail.contains("possible mods-folder clash"));
        assert!(diagnosis.fix.is_none());
        let report = run.crash_report(&diagnosis.title, Some(-805306369));
        assert_eq!(
            report.title,
            "Game exited with code -805306369 (0xCFFFFFFF)"
        );
        assert!(!report.title.contains("installed twice"));
        assert!(!report.title.contains("resource-pack"));
        assert!(run.exit_diagnosis(Some(0), false).is_none());
        assert!(run.exit_diagnosis(Some(-805306369), true).is_none());

        run.push_log([LogLine {
            level: Level::Error,
            text: "Hang at exit: client hung for 12s; no recovery recorded\nheap: 2000 MB used of 2048 MB max at start".into(),
        }]);
        let diagnosis = run.exit_diagnosis(Some(-805306369), false).unwrap();
        assert_eq!(diagnosis.title, "Minecraft ran out of memory and froze");
        let (_, localized_detail) = ArcticApp::localized_diagnosis(&diagnosis);
        assert!(localized_detail.contains("possible mods-folder clash"));
    }

    #[test]
    fn logged_version_error_then_memory_hang_is_not_a_version_crash() {
        let dir = tempfile::tempdir().unwrap();
        let freezes = dir.path().join("logs/arctic-freezes");
        std::fs::create_dir_all(&freezes).unwrap();
        std::fs::write(
            freezes.join("freeze-now.txt"),
            format!("{UNFINISHED_FREEZE}\nheap: 3868 MB used of 4100 MB max at start\n"),
        )
        .unwrap();
        std::fs::write(dir.path().join("logs/latest.log"),
            "[main/ERROR] Optional integration failed\njava.lang.NoSuchMethodError: optional.mod()\n[Render thread/INFO] JEI finished").unwrap();
        let mut runs = Runs::default();
        let id = runs.start("pack", "1.21.1", "Pack".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.started = SystemTime::now() - Duration::from_secs(60);
        run.java = Some("Java 21 -Xmx4100M".into());
        let diagnosis = run.exit_diagnosis(Some(-805306369), false).unwrap();
        assert_eq!(diagnosis.title, "Minecraft ran out of memory and froze");
        let report = run.crash_report(&diagnosis.title, Some(-805306369));
        assert_eq!(report.title, diagnosis.title);
        assert!(report.log.contains("matched: heap: 3868 MB"));
        run.force_close();
        run.killed = true;
        let code = run.reported_exit_code(Some(-805306369), true);
        assert!(run.exit_diagnosis(code, false).is_none());
    }

    #[test]
    fn windows_kill_with_an_unfinished_freeze_reports_the_hang_at_exit() {
        let dir = tempfile::tempdir().unwrap();
        let freezes = dir.path().join("logs/arctic-freezes");
        std::fs::create_dir_all(&freezes).unwrap();
        std::fs::write(
            freezes.join("freeze-20261113-120000.txt"),
            UNFINISHED_FREEZE,
        )
        .unwrap();
        let mut runs = Runs::default();
        let id = runs.start("pack", "1.21.11", "Pack".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.started = SystemTime::now() - Duration::from_secs(60);
        run.java = Some("Java 21".into());
        let diagnosis = run.exit_diagnosis(Some(-805306369), false).unwrap();
        assert_eq!(diagnosis.title, "The game froze and was closed");
        assert!(diagnosis.detail.contains(
            "hung for at least 3.12s in org.lwjgl.vulkan.VK10.vkQueuePresentKHR(Native Method)"
        ));
        assert!(!diagnosis.detail.contains("may be normal"));
        assert!(!diagnosis.repair);
        let report = run.crash_report(&diagnosis.title, Some(-805306369));
        assert_eq!(report.title, diagnosis.title);
        assert!(
            report
                .log
                .contains("Hang at exit: client hung for at least 3.12s")
        );
        assert!(report.log.contains("still frozen when written"));
        assert!(report.log.contains("at example.Game.render(Game.java:42)"));
        assert!(report.log.contains("thread \"worker\" WAITING"));
        assert!(report.log.contains("0xCFFFFFFF"));
        assert!(run.exit_diagnosis(Some(0), false).is_none());
        assert!(run.exit_diagnosis(Some(-805306369), true).is_none());

        // Two large snapshots and a verbose on-disk log must retain the fatal stack and summary
        // within Report's byte limit, including when truncation lands inside a UTF-8 character.
        let large_snapshot = format!(
            "{UNFINISHED_FREEZE}{}",
            format!("extra snapshot detail: {}\n", "é".repeat(2000)).repeat(120)
        );
        let old = freezes.join("freeze-20261113-115930.txt");
        std::fs::write(
            &old,
            large_snapshot.replace(" (still frozen when written)", ""),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(30)),
            )
            .unwrap();
        std::fs::write(freezes.join("freeze-20261113-120000.txt"), large_snapshot).unwrap();
        std::fs::write(
            dir.path().join("logs/latest.log"),
            format!("verbose log line {}\n", "x".repeat(300)).repeat(3000),
        )
        .unwrap();
        let large_report = run.crash_report(&diagnosis.title, Some(-805306369));
        assert!(
            large_report
                .log
                .contains("Hang at exit: client hung for at least 3.12s")
        );
        assert!(large_report.log.contains("freeze-20261113-120000.txt"));
        assert!(
            large_report
                .log
                .contains("at example.Game.render(Game.java:42)")
        );
        assert!(large_report.log.contains("snapshot truncated"));

        // A specific error still takes priority over the freeze evidence.
        run.push_log([LogLine {
            level: Level::Error,
            text: "java.lang.OutOfMemoryError: Java heap space".into(),
        }]);
        assert_eq!(
            run.exit_diagnosis(Some(-805306369), false).unwrap().title,
            "Minecraft ran out of memory"
        );
        run.clear_log();

        // A finished snapshot alone does not prove an ongoing stall, but together with the
        // Windows hang termination code it still explains why this run ended.
        std::fs::write(
            freezes.join("freeze-20261113-120000.txt"),
            UNFINISHED_FREEZE.replace(" (still frozen when written)", ""),
        )
        .unwrap();
        assert_eq!(
            run.exit_diagnosis(Some(-805306369), false).unwrap().title,
            "The game froze and was closed"
        );
        assert!(run.exit_diagnosis(Some(1), false).is_none());
        assert!(
            !run.crash_report("", Some(-805306369))
                .log
                .contains("Hang at exit:")
        );

        // No snapshot from a previous run can diagnose this run.
        std::fs::write(
            freezes.join("freeze-20261113-120000.txt"),
            UNFINISHED_FREEZE,
        )
        .unwrap();
        run.started = SystemTime::now() + Duration::from_secs(60);
        assert_ne!(
            run.exit_diagnosis(Some(-805306369), false).unwrap().title,
            diagnosis.title
        );
        assert!(
            !run.crash_report("", Some(-805306369))
                .log
                .contains("still frozen when written")
        );
    }

    #[test]
    fn a_later_recovered_freeze_rules_out_an_older_unfinished_snapshot_at_exit() {
        let dir = tempfile::tempdir().unwrap();
        let freezes = dir.path().join("logs/arctic-freezes");
        std::fs::create_dir_all(&freezes).unwrap();
        let old = freezes.join("freeze-20261113-120000.txt");
        std::fs::write(&old, UNFINISHED_FREEZE).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(30)),
            )
            .unwrap();
        std::fs::write(
            freezes.join("freeze-20261113-120020.txt"),
            UNFINISHED_FREEZE.replace(" (still frozen when written)", ""),
        )
        .unwrap();
        let found = fresh_freeze_files(dir.path(), SystemTime::now() - Duration::from_secs(60));
        assert_eq!(found.len(), 2);
        assert!(freeze_at_exit(&found[0]).is_none());
        assert!(freeze_at_exit(&found[1]).is_some());
        assert!(
            freeze_at_exit("duration: 3.00s (still frozen when written)\n")
                .unwrap()
                .contains("hung for at least 3.00s;")
        );
    }

    #[test]
    fn a_small_heap_does_not_hide_a_missing_library_or_disable_repair() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        std::fs::create_dir(&mods).unwrap();
        for n in 0..80 {
            std::fs::write(mods.join(format!("mod-{n}.jar")), b"").unwrap();
        }
        let mut runs = Runs::default();
        let id = runs.start("pack", "1.21.4", "Pack".into(), dir.path().into());
        let run = runs.get_mut(id).unwrap();
        run.java = Some("Java java-runtime-delta, flags: -Xms512M -Xmx1918M".into());
        run.push_log([LogLine {
            level: Level::Error,
            text: "java.lang.NoClassDefFoundError: org/spongepowered/asm/mixin/throwables/MixinError\nCaused by: java.lang.ClassNotFoundException: org.spongepowered.asm.mixin.throwables.MixinError".into(),
        }]);
        assert!(run.memory_warning().is_some());
        let diagnosis = run.diagnose().unwrap();
        assert_eq!(diagnosis.title, "A game library couldn't be loaded");
        assert!(diagnosis.repair);
        run.clear_log();
        run.push_log([LogLine {
            level: Level::Error,
            text: "java.lang.OutOfMemoryError: Java heap space".into(),
        }]);
        assert_eq!(run.diagnose().unwrap().title, "Minecraft ran out of memory");
    }

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
