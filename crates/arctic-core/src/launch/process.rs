//! Running the game: spawn, tee output to the log file, detect when the
//! window is up, and report the exit, all on background threads so the
//! launcher UI never waits on the game.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::LaunchPlan;
use super::logparse::{LogLine, LogParser};
use super::priority;
use crate::Result;
use crate::error::IoContext;

/// How often the watcher checks for exit or a kill request.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);

/// Log lines printed right around the moment the game window opens:
/// modern versions (1.13+), LWJGL 2 era, and a late fallback.
const WINDOW_MARKERS: [&str; 3] = ["Backend library:", "LWJGL Version:", "Sound engine started"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    /// The game window is (about to be) visible.
    WindowReady,
    /// The process ended. `code` is `None` if it was killed by a signal.
    Exited { code: Option<i32> },
    /// Parsed output lines (one raw line may expand to several).
    Output(Vec<LogLine>),
}

/// Handle to a running game. Dropping it does not stop the game.
#[derive(Debug, Clone)]
pub struct GameHandle {
    pid: u32,
    kill: Arc<AtomicBool>,
}

impl GameHandle {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// End the game now, without waiting for the watcher (the launcher is
    /// quitting and its threads go with it).
    pub fn terminate(&self) {
        terminate(self.pid);
    }

    /// Ask the watcher to terminate the game (Force close).
    pub fn kill(&self) {
        self.kill.store(true, Ordering::Relaxed);
    }
}

/// Game logs kept per instance and copy: the last run's and the runs before it.
const KEPT_GAME_LOGS: usize = 5;

/// `game-vanilla.log` becomes `game-vanilla.1.log`, that one `game-vanilla.2.log`, and so on; the
/// oldest goes. A new run starts a fresh file without losing the run before it.
fn rotate_game_log(file: &Path) {
    let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let numbered = |n: usize| file.with_file_name(format!("{stem}.{n}.log"));
    let _ = std::fs::remove_file(numbered(KEPT_GAME_LOGS - 1));
    for n in (1..KEPT_GAME_LOGS - 1).rev() {
        let _ = std::fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = std::fs::rename(file, numbered(1));
}

/// Start the game. Output is written to `plan.log_file`; `on_event` is
/// called from background threads.
pub fn spawn(
    plan: &LaunchPlan,
    on_event: impl Fn(GameEvent) + Send + Sync + 'static,
) -> Result<GameHandle> {
    log::info!("launching: {}", plan.redacted_command());
    rotate_game_log(&plan.log_file);
    let log = Arc::new(Mutex::new(File::create(&plan.log_file).at(&plan.log_file)?));
    let (args, argfile) = command_args(plan)?;
    let start = |exe: &Path| {
        let mut cmd = Command::new(exe);
        cmd.args(&args)
            .envs(crate::gpu_preference::env(plan.high_performance_gpu))
            .current_dir(game_cwd(&plan.game_dir))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        priority::boost(&mut cmd);
        cmd.spawn()
    };
    let mut exe = game_exe(plan);
    let started = std::time::Instant::now();
    let mut child = match start(&exe) {
        Ok(child) => child,
        // The branded copy would not start: run the runtime's own Java.
        Err(e) if exe != plan.java => {
            log::warn!(
                "{} did not start ({e}); using {}",
                exe.display(),
                plan.java.display()
            );
            exe = plan.java.clone();
            crate::gpu_preference::apply(&exe, plan.high_performance_gpu);
            match start(&plan.java) {
                Ok(child) => child,
                Err(e) => {
                    if let Some(file) = &argfile {
                        let _ = std::fs::remove_file(file);
                    }
                    return Err(crate::Error::io(&plan.java, e));
                }
            }
        }
        Err(e) => {
            // The argument file holds the access token: not left behind when the game did not start.
            if let Some(file) = &argfile {
                let _ = std::fs::remove_file(file);
            }
            return Err(crate::Error::io(&plan.java, e));
        }
    };
    let launch_context = format!(
        "[Arctic diagnostics] runtime: agent={}; executable={}; high_performance_gpu={}",
        plan.args
            .iter()
            .any(|a| a.starts_with("-javaagent:") && a.contains("arctic")),
        if exe == plan.java {
            "runtime java"
        } else {
            "branded Arctic Client"
        },
        plan.high_performance_gpu
    );
    let launch_context = format!(
        "{launch_context}; {}",
        crate::gpu_preference::diagnostics(&exe)
    );
    writeln!(log.lock().unwrap(), "{launch_context}").ok();
    on_event(GameEvent::Output(vec![LogLine {
        level: super::logparse::Level::Info,
        text: launch_context,
    }]));
    if plan.maximize_window {
        super::game_window::maximize_when_open(child.id());
    }

    let on_event = Arc::new(on_event);
    let window_seen = Arc::new(AtomicBool::new(false));
    let ready_seen = Arc::new(AtomicBool::new(false));
    priority::settle_when_ready(child.id(), ready_seen.clone(), window_seen.clone());
    let secrets: Arc<[String]> = plan.log_secrets().into();
    let tee = |stream: Box<dyn Read + Send>| {
        let (log, seen, on_event) = (log.clone(), window_seen.clone(), on_event.clone());
        let (ready, secrets) = (ready_seen.clone(), secrets.clone());
        std::thread::spawn(move || {
            let seen = Seen {
                window: &seen,
                ready: &ready,
            };
            tee_output(stream, &log, seen, &secrets, &*on_event)
        })
    };
    let out_reader = child.stdout.take().map(|out| tee(Box::new(out)));
    let err_reader = child.stderr.take().map(|err| tee(Box::new(err)));

    let kill = Arc::new(AtomicBool::new(false));
    let handle = GameHandle {
        pid: child.id(),
        kill: kill.clone(),
    };
    let game_dir = plan.game_dir.clone();
    let log_path = plan.log_file.clone();
    std::thread::spawn(move || {
        let code = loop {
            if kill.swap(false, Ordering::Relaxed) {
                let message = "[Arctic diagnostics] launcher force-close requested";
                writeln!(log.lock().unwrap(), "{message}").ok();
                on_event(GameEvent::Output(vec![LogLine {
                    level: super::logparse::Level::Info,
                    text: message.into(),
                }]));
                let _ = child.kill();
            }
            match child.try_wait() {
                Ok(Some(status)) => break exit_code(&status),
                Ok(None) => std::thread::sleep(WATCH_INTERVAL),
                Err(e) => {
                    log::error!("lost track of the game process: {e}");
                    break None;
                }
            }
        };
        for reader in [out_reader, err_reader].into_iter().flatten() {
            let _ = reader.join();
        }
        if let Some(c) = code {
            let summary = format!(
                "[Arctic diagnostics] process ended after {} ms; exit code {}",
                started.elapsed().as_millis(),
                crate::crash::exit_description(c)
            );
            writeln!(log.lock().unwrap(), "{summary}").ok();
            on_event(GameEvent::Output(vec![LogLine {
                level: super::logparse::Level::Info,
                text: summary,
            }]));
        }
        if code.is_some_and(|c| c != 0) && started.elapsed() < Duration::from_secs(5) {
            let probe = runtime_probe(&exe);
            writeln!(log.lock().unwrap(), "{probe}").ok();
            on_event(GameEvent::Output(vec![LogLine {
                level: super::logparse::Level::Info,
                text: probe,
            }]));
        }
        if code.is_some_and(|c| c != 0) {
            super::startup_cache::drop_pack(&game_dir);
        }
        if let Some(file) = argfile {
            // It holds the access token: gone with the game.
            let _ = std::fs::remove_file(file);
        }
        // Keep startup output and diagnostic markers even after the UI's ring buffer rolls over.
        if let Ok(file) = File::open(&log_path) {
            let mut evidence = String::new();
            let mut initial = 0;
            for line in BufReader::new(file)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let keep = line.contains("[Arctic diagnostics]")
                    || line.contains("pack metadata")
                    || line.contains("Missing metadata in pack")
                    || line.contains("MalformedJsonException") && line.contains("$.pack");
                if initial < 2048 || keep {
                    initial += line.len();
                    if evidence.len() < 64 * 1024 {
                        evidence.push_str(&line);
                        evidence.push('\n');
                    }
                }
            }
            let _ = std::fs::write(game_dir.join("arctic-launch-diagnostics.txt"), &evidence);
            on_event(GameEvent::Output(vec![LogLine {
                level: super::logparse::Level::Info,
                text: evidence,
            }]));
        }
        on_event(GameEvent::Exited { code });
    });
    Ok(handle)
}

/// Bounded and invisible: same executable, no game or account arguments.
fn runtime_probe(exe: &Path) -> String {
    let mut cmd = Command::new(exe);
    cmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let result = (|| -> std::io::Result<String> {
        let mut probe = cmd.spawn()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if probe.try_wait()?.is_some() {
                let out = probe.wait_with_output()?;
                let output = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                return Ok(format!(
                    "status={}; {}",
                    out.status,
                    crate::crash::scrubbed_tail(&output, 2048).replace('\n', " | ")
                ));
            }
            if std::time::Instant::now() >= deadline {
                let _ = probe.kill();
                let _ = probe.wait();
                return Ok("timed out after 3 s".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })();
    format!(
        "[Arctic diagnostics] same-runtime java -version: {}",
        result.unwrap_or_else(|e| format!("failed: {e}"))
    )
}

/// The game's exit code; a game the system stopped with a signal (Linux, macOS) counts as 128 plus the
/// signal, like a shell reports it: 137 is the system killing it for running out of memory.
fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    if let Some(code) = status.code() {
        return Some(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            log::warn!(
                "the game was stopped by signal {signal}{}",
                if signal == 9 {
                    " (the system killed it, usually because it ran out of memory)"
                } else {
                    ""
                }
            );
            return Some(128 + signal);
        }
    }
    None
}

/// Start the game fully detached from this process: output goes straight
/// to `plan.log_file` (no pipes), so the game keeps running and logging
/// after a short-lived caller (e.g. the CLI) exits. Returns the pid.
pub fn spawn_detached(plan: &LaunchPlan) -> Result<u32> {
    log::info!("launching (detached): {}", plan.redacted_command());
    rotate_game_log(&plan.log_file);
    let log = File::create(&plan.log_file).at(&plan.log_file)?;
    let log_err = log.try_clone().at(&plan.log_file)?;
    let (args, argfile) = command_args(plan)?;
    let start = |exe: &Path| {
        Command::new(exe)
            .args(&args)
            .envs(crate::gpu_preference::env(plan.high_performance_gpu))
            .current_dir(game_cwd(&plan.game_dir))
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log_err.try_clone()?)
            .spawn()
    };
    let exe = game_exe(plan);
    let spawned = start(&exe).or_else(|e| {
        if exe == plan.java {
            return Err(e);
        }
        // The branded copy would not start: run the runtime's own Java.
        log::warn!(
            "{} did not start ({e}); using {}",
            exe.display(),
            plan.java.display()
        );
        start(&plan.java)
    });
    if let Some(file) = &argfile {
        // Java reads it while it starts. It holds the access token, so it does not stay (the caller may
        // exit before the game does, so nothing waits for the game itself).
        if spawned.is_ok() {
            std::thread::sleep(ARGFILE_READ_TIME);
        }
        let _ = std::fs::remove_file(file);
    }
    let child = spawned.at(&plan.java)?;
    if plan.maximize_window {
        super::game_window::maximize_when_open(child.id());
    }
    Ok(child.id())
}

/// End process `pid` (and, on Windows, what it started).
pub fn terminate(pid: u32) {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        c
    } else {
        let mut c = Command::new("kill");
        c.arg(pid.to_string());
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up.
        cmd.creation_flags(0x0800_0000);
    }
    if let Err(e) = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        log::warn!("couldn't end the game ({pid}): {e}");
    }
}

/// What the game's process is called on Windows and macOS.
const CLIENT_PROCESS: &str = if cfg!(windows) {
    "Arctic Client.exe"
} else {
    "arctic-client"
};

/// Discord marks any `javaw.exe` (macOS: `java`) running Minecraft as
/// "Playing Minecraft", which buries the launcher's own presence. A copy of
/// the Java launcher under the client's name, next to the original (it finds
/// the rest of Java from its own folder), keeps that from happening and
/// shows "Arctic Client" in Task Manager. Falls back to `java` on any error
/// (a read-only Java folder, say).
/// The executable the game runs as, set to the graphics card the player wants.
/// CreateProcess refuses a command line over 32 767 characters; stay below it with room for the program.
const MAX_COMMAND_CHARS: usize = 30_000;
/// How long a detached start gives Java to read its argument file before it is deleted.
const ARGFILE_READ_TIME: Duration = Duration::from_secs(5);

/// The arguments to start Java with, and the `@argfile` that holds them when the command line would be too
/// long for Windows (a Forge or NeoForge classpath of 100+ libraries under a long data folder). Java 9+ reads
/// `@file`; Java 8 can't, and gets a message about moving the data folder instead of an obscure spawn error.
fn command_args(plan: &LaunchPlan) -> Result<(Vec<String>, Option<PathBuf>)> {
    let length = plan
        .args
        .iter()
        .map(|a| a.chars().count() + 3)
        .sum::<usize>()
        + plan.java.as_os_str().len();
    if !cfg!(windows) || length <= MAX_COMMAND_CHARS {
        return Ok((plan.args.clone(), None));
    }
    if !java_reads_argfiles(&plan.java) {
        return Err(crate::Error::Other(format!(
            "The game's command line is {length} characters, more than Windows allows (32 767), and \
             this Java can't read it from a file. Move the data folder to a shorter path, like \
             C:\\ArcticData (Settings > Storage)."
        )));
    }
    let file = plan.log_file.with_extension("args");
    std::fs::write(&file, argfile_text(&plan.args)).at(&file)?;
    log::info!(
        "command line of {length} characters: arguments passed in {}",
        file.display()
    );
    Ok((vec![format!("@{}", file.display())], Some(file)))
}

/// One quoted argument per line; inside quotes a backslash starts an escape, so paths need theirs doubled.
fn argfile_text(args: &[String]) -> String {
    let mut text = String::new();
    for arg in args {
        text.push('"');
        for c in arg.chars() {
            match c {
                '\\' => text.push_str("\\\\"),
                '"' => text.push_str("\\\""),
                '\n' => text.push_str("\\n"),
                '\r' => text.push_str("\\r"),
                c => text.push(c),
            }
        }
        text.push_str("\"\n");
    }
    text
}

/// Whether the Java at `java` (`<runtime>/bin/java`) is version 9 or newer, from the runtime's `release` file.
fn java_reads_argfiles(java: &Path) -> bool {
    java.parent()
        .and_then(Path::parent)
        .and_then(|runtime| std::fs::read_to_string(runtime.join("release")).ok())
        .and_then(|release| java_major(&release))
        .is_some_and(|major| major >= 9)
}

/// The major version in a runtime's `release` file (`JAVA_VERSION="1.8.0_402"` is 8, `"17.0.9"` is 17).
fn java_major(release: &str) -> Option<u32> {
    let version = release
        .lines()
        .find_map(|l| l.strip_prefix("JAVA_VERSION="))?
        .trim()
        .trim_matches('"');
    let mut parts = version.split(['.', '-', '+', '_']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// The folder the game process starts in. Minecraft gets its folders as absolute arguments, but Java
/// itself fails at startup ("Could not determine current working directory") when it starts inside a
/// Flatpak document-portal mount (`/run/user/<uid>/doc/...`, where a data folder picked in the file
/// chooser lives), because the mount can't answer `getcwd`. There it starts in the home folder instead.
fn game_cwd(game_dir: &Path) -> PathBuf {
    if cfg!(target_os = "linux") && is_portal_path(game_dir) {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if let Some(home) = home.filter(|h| h.is_dir()) {
            log::warn!(
                "{} is a document-portal folder; starting the game in {} so Java can read its working directory",
                game_dir.display(),
                home.display()
            );
            return home;
        }
    }
    game_dir.to_path_buf()
}

fn is_portal_path(path: &Path) -> bool {
    let mut parts = path.components().skip(1).map(|c| c.as_os_str());
    parts.next().is_some_and(|p| p == "run")
        && parts.next().is_some_and(|p| p == "user")
        && parts.next().is_some()
        && parts.next().is_some_and(|p| p == "doc")
}

fn game_exe(plan: &LaunchPlan) -> PathBuf {
    let exe = branded_java(&plan.java);
    crate::gpu_preference::apply(&exe, plan.high_performance_gpu);
    exe
}

fn branded_java(java: &Path) -> PathBuf {
    let is_java = java
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| matches!(n, "javaw.exe" | "java"));
    if !is_java || !(cfg!(windows) || cfg!(target_os = "macos")) {
        return java.to_path_buf();
    }
    let copy = java.with_file_name(CLIENT_PROCESS);
    let same = |a: &Path, b: &Path| match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() <= b.modified().ok(),
        _ => false,
    };
    if !same(java, &copy)
        && let Err(e) = std::fs::copy(java, &copy)
    {
        log::info!("running the game as {}: {e}", java.display());
        return java.to_path_buf();
    }
    in_named_folder(&copy)
}

/// NVIDIA's recorder files clips under the name of the folder that holds
/// `bin`, which is the runtime's (`java-runtime-delta`). The game is started
/// from a copy of the runtime named after the client instead,
/// `runtimes/.named/<runtime>/Arctic Client`, made of hard links (no extra
/// space). Java works out its home from that path, so nothing else changes.
/// (A junction isn't enough: recorders see the path it leads to.)
#[cfg(windows)]
fn in_named_folder(exe: &Path) -> PathBuf {
    const NAME: &str = "Arctic Client";
    let named = (|| {
        let bin = exe.parent()?;
        let runtime = bin.parent()?;
        if bin.file_name()? != "bin" || runtime.file_name()? == NAME {
            return None;
        }
        let named = runtime
            .parent()?
            .join(".named")
            .join(runtime.file_name()?)
            .join(NAME);
        // Earlier versions made a junction here: remove the link itself, never what it leads to.
        if junction::exists(&named).unwrap_or(false) {
            std::fs::remove_dir(&named)
                .inspect_err(|e| log::info!("old junction for the game's name: {e}"))
                .ok()?;
        }
        remove_system_dlls(&named);
        mirror_tree(runtime, &named)
            .inspect_err(|e| log::info!("named copy of the Java runtime: {e}"))
            .ok()?;
        Some(named.join("bin").join(exe.file_name()?))
    })();
    named.unwrap_or_else(|| exe.to_path_buf())
}

/// Files with the names of Windows' own C runtime (`ucrtbase.dll`, `api-ms-win-*.dll`). Mojang's
/// runtimes bundle them in `bin`; the named copy leaves them out (Windows 10/11 have them in
/// System32), so the launcher never writes a file under a system DLL's name.
#[cfg(any(windows, test))]
fn is_system_dll(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name == "ucrtbase.dll" || (name.starts_with("api-ms-win-") && name.ends_with(".dll"))
}

#[cfg(any(windows, test))]
fn is_link(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    if junction::exists(path).unwrap_or(false) {
        return true;
    }
    false
}

/// Delete the system-named DLLs an older version left in the named copy's `bin` (hard links or
/// copies; removing a link only removes that name, never the runtime's file). Only inside a
/// `.named` folder, and never through a symlink or junction. Failures (a file in use) are logged.
#[cfg(any(windows, test))]
fn remove_system_dlls(named: &Path) {
    let under_named = named
        .ancestors()
        .skip(1)
        .any(|a| a.file_name().is_some_and(|n| n == ".named"));
    let bin = named.join("bin");
    if !under_named || is_link(named) || is_link(&bin) || !bin.is_dir() {
        return;
    }
    // Every ancestor up to `.named` must be a real folder too.
    for a in named.ancestors().skip(1) {
        if a.file_name().is_some_and(|n| n == ".named") {
            break;
        }
        if is_link(a) {
            return;
        }
    }
    let Ok(entries) = std::fs::read_dir(&bin) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        if is_file && is_system_dll(&entry.file_name()) {
            let _ = std::fs::remove_file(&path)
                .inspect_err(|e| log::info!("old {} in the named copy: {e}", path.display()));
        }
    }
}

/// Make `dst` hold every file of `src` as a hard link (a plain copy where a link can't be
/// made). Files already linked are left alone, so this is quick once done; a file the runtime
/// replaced is linked again.
#[cfg(any(windows, test))]
fn mirror_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let (from, to) = (entry.path(), dst.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_dir() {
            mirror_tree(&from, &to)?;
            continue;
        }
        if !kind.is_file() || same_file(&from, &to) {
            continue;
        }
        if dst.file_name().is_some_and(|n| n == "bin") && is_system_dll(&entry.file_name()) {
            continue;
        }
        let _ = std::fs::remove_file(&to);
        if std::fs::hard_link(&from, &to).is_err() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Already the same file (a hard link of it), or a copy as large and as new.
#[cfg(any(windows, test))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() == b.modified().ok(),
        _ => false,
    }
}

#[cfg(not(windows))]
fn in_named_folder(exe: &Path) -> PathBuf {
    exe.to_path_buf()
}

/// What the output readers of both streams have seen so far.
#[derive(Clone, Copy)]
struct Seen<'a> {
    window: &'a AtomicBool,
    /// The Arctic Client said the game is ready.
    ready: &'a AtomicBool,
}

/// Copy a child stream into the log line by line, firing `WindowReady` the
/// first time a marker shows up (on either stream). `secrets` never reach
/// the log or the launcher (1.8 prints its session token, for one).
fn tee_output(
    stream: Box<dyn Read + Send>,
    log: &Mutex<File>,
    seen: Seen,
    secrets: &[String],
    on_event: &(dyn Fn(GameEvent) + Send + Sync),
) {
    let mut reader = BufReader::new(stream);
    let mut parser = LogParser::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => {
                let rest = parser.finish();
                if !rest.is_empty() {
                    on_event(GameEvent::Output(rest));
                }
                return;
            }
            Ok(_) => {}
        }
        redact(&mut line, secrets);
        if let Ok(mut file) = log.lock() {
            let _ = file.write_all(&line);
        }
        // `swap` makes the check-and-set atomic: stdout and stderr are read
        // by separate threads and must not both fire.
        if is_window_marker(&line) && !seen.window.swap(true, Ordering::Relaxed) {
            on_event(GameEvent::WindowReady);
        }
        if String::from_utf8_lossy(&line).contains(priority::READY_MARKER) {
            seen.ready.store(true, Ordering::Relaxed);
        }
        let parsed = parser.feed(&String::from_utf8_lossy(&line));
        if !parsed.is_empty() {
            on_event(GameEvent::Output(parsed));
        }
    }
}

/// Replace every secret in `line` with `<redacted>`.
fn redact(line: &mut Vec<u8>, secrets: &[String]) {
    for secret in secrets.iter().map(String::as_bytes) {
        while let Some(at) = line.windows(secret.len()).position(|w| w == secret) {
            line.splice(at..at + secret.len(), b"<redacted>".iter().copied());
        }
    }
}

fn is_window_marker(line: &[u8]) -> bool {
    let text = String::from_utf8_lossy(line);
    WINDOW_MARKERS.iter().any(|m| text.contains(m))
}

#[cfg(test)]
mod tests {
    #[test]
    fn portal_folders_are_recognized() {
        assert!(is_portal_path(Path::new(
            "/run/user/1000/doc/AbC/Minecraft/Arctic Launcher"
        )));
        assert!(!is_portal_path(Path::new("/run/user/1000/gvfs/x")));
        assert!(!is_portal_path(Path::new("/home/u/.var/app/doc/x")));
    }

    #[test]
    fn argument_files_quote_and_escape() {
        let args = vec![r"-Dpath=C:\natives".to_owned(), "say \"hi\"".to_owned()];
        assert_eq!(
            argfile_text(&args),
            "\"-Dpath=C:\\\\natives\"\n\"say \\\"hi\\\"\"\n"
        );
    }

    /// A path with spaces and backslashes, written the way a Windows launch does and read back
    /// with OpenJDK's `@argfile` rules. Inside quotes `\` escapes the next character, so each
    /// path separator is doubled, and a newline ends a token even inside quotes. Unquoted, the
    /// space in `Forge 1.8.9` would split that path.
    #[test]
    fn argument_file_round_trips_a_windows_path_with_spaces() {
        let natives = r"C:\Users\me\AppData\Local\ArcticLauncher\profiles\default\instances\Forge 1.8.9\natives";
        let classpath = format!(
            r"C:\Users\me\AppData\Local\ArcticLauncher\shared\libraries\net\minecraft\launchwrapper\1.12\launchwrapper-1.12.jar;{natives}\client.jar"
        );
        let args = vec![
            format!("-Djava.library.path={natives}"),
            r"-javaagent:C:\Program Files\Arctic Launcher\agent.jar".to_owned(),
            "-cp".into(),
            classpath,
            "net.minecraft.launchwrapper.Launch".into(),
        ];
        let parsed = java_argfile_tokens(&argfile_text(&args));
        assert_eq!(parsed, args);
        assert!(!parsed.iter().any(|a| a == "Forge"));
        let cp = parsed.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(parsed[cp + 2], "net.minecraft.launchwrapper.Launch");
    }

    /// `args.c` `nextToken` cases: doubled backslashes inside quotes, and `#` comments.
    #[test]
    fn java_argfile_parser_matches_openjdk() {
        assert_eq!(
            java_argfile_tokens("-version -cp \"c:\\\\java libs\\\\one.jar\" \n"),
            ["-version", "-cp", "c:\\java libs\\one.jar"]
        );
        assert_eq!(
            java_argfile_tokens(
                "line one #comment\n'line #2' #rest are comment\r\n#comment on line 3\nline 4 #comment to eof"
            ),
            ["line", "one", "line #2", "line", "4"]
        );
        assert_eq!(
            java_argfile_tokens("escaped chars testing \"\\a\\b\\c\\f\\n\\r\\t\\v\\9\""),
            ["escaped", "chars", "testing", "abc\u{000c}\n\r\tv9"]
        );
    }

    #[test]
    fn java_versions_come_from_the_release_file() {
        assert_eq!(java_major("JAVA_VERSION=\"1.8.0_402\"\n"), Some(8));
        assert_eq!(
            java_major("IMPLEMENTOR=\"x\"\nJAVA_VERSION=\"17.0.9\"\n"),
            Some(17)
        );
        assert_eq!(java_major("JAVA_VERSION=\"21\"\n"), Some(21));
        assert_eq!(java_major("nothing here"), None);
    }

    #[test]
    fn named_runtime_copy_links_every_file_and_updates_changed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("java-runtime-delta");
        std::fs::create_dir_all(runtime.join("bin")).unwrap();
        std::fs::create_dir_all(runtime.join("lib/security")).unwrap();
        std::fs::write(runtime.join("bin/javaw.exe"), b"java").unwrap();
        std::fs::write(runtime.join("lib/security/cacerts"), b"certs").unwrap();
        let named = dir.path().join(".named/java-runtime-delta/Arctic Client");
        mirror_tree(&runtime, &named).unwrap();
        assert_eq!(std::fs::read(named.join("bin/javaw.exe")).unwrap(), b"java");
        assert_eq!(
            std::fs::read(named.join("lib/security/cacerts")).unwrap(),
            b"certs"
        );
        // A runtime update replaces a file: the copy follows.
        std::fs::remove_file(runtime.join("lib/security/cacerts")).unwrap();
        std::fs::write(runtime.join("lib/security/cacerts"), b"newer certs").unwrap();
        mirror_tree(&runtime, &named).unwrap();
        assert_eq!(
            std::fs::read(named.join("lib/security/cacerts")).unwrap(),
            b"newer certs"
        );
    }

    use super::*;

    #[test]
    fn system_dlls_are_not_mirrored_and_old_ones_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("java-runtime-delta");
        std::fs::create_dir_all(runtime.join("bin")).unwrap();
        for f in [
            "javaw.exe",
            "UCRTBASE.dll",
            "api-ms-win-core-file-l1-1-0.dll",
            "jli.dll",
        ] {
            std::fs::write(runtime.join("bin").join(f), b"x").unwrap();
        }
        std::fs::create_dir_all(runtime.join("lib")).unwrap();
        std::fs::write(runtime.join("lib/api-ms-win-x.dll"), b"x").unwrap();
        let named = dir.path().join(".named/java-runtime-delta/Arctic Client");
        // An older version had linked everything.
        mirror_tree(&runtime, &named).unwrap();
        std::fs::write(named.join("bin/ucrtbase.dll"), b"old").unwrap();
        std::fs::write(named.join("bin/API-MS-WIN-core-x.DLL"), b"old").unwrap();
        remove_system_dlls(&named);
        mirror_tree(&runtime, &named).unwrap();
        let has = |f: &str| named.join(f).exists();
        assert!(has("bin/javaw.exe") && has("bin/jli.dll"));
        assert!(!has("bin/ucrtbase.dll") && !has("bin/UCRTBASE.dll"));
        assert!(!has("bin/api-ms-win-core-file-l1-1-0.dll") && !has("bin/API-MS-WIN-core-x.DLL"));
        // Only bin is filtered.
        assert!(has("lib/api-ms-win-x.dll"));
        // The runtime itself is untouched.
        assert!(runtime.join("bin/UCRTBASE.dll").exists());
        assert!(runtime.join("bin/api-ms-win-core-file-l1-1-0.dll").exists());
    }

    #[test]
    fn cleanup_stays_inside_the_named_folder() {
        let dir = tempfile::tempdir().unwrap();
        // Not under .named: nothing is deleted.
        let plain = dir.path().join("Arctic Client");
        std::fs::create_dir_all(plain.join("bin")).unwrap();
        std::fs::write(plain.join("bin/ucrtbase.dll"), b"x").unwrap();
        remove_system_dlls(&plain);
        assert!(plain.join("bin/ucrtbase.dll").exists());
        // A hard link is removed by name only.
        let runtime = dir.path().join("rt");
        std::fs::create_dir_all(runtime.join("bin")).unwrap();
        std::fs::write(runtime.join("bin/ucrtbase.dll"), b"x").unwrap();
        let named = dir.path().join(".named/rt/Arctic Client");
        std::fs::create_dir_all(named.join("bin")).unwrap();
        std::fs::hard_link(
            runtime.join("bin/ucrtbase.dll"),
            named.join("bin/ucrtbase.dll"),
        )
        .unwrap();
        remove_system_dlls(&named);
        assert!(!named.join("bin/ucrtbase.dll").exists());
        assert!(runtime.join("bin/ucrtbase.dll").exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_does_not_follow_links() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("rt");
        std::fs::create_dir_all(runtime.join("bin")).unwrap();
        std::fs::write(runtime.join("bin/ucrtbase.dll"), b"x").unwrap();
        // The named folder is a link to the real runtime (like the old junction).
        let named = dir.path().join(".named/rt/Arctic Client");
        std::fs::create_dir_all(named.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&runtime, &named).unwrap();
        remove_system_dlls(&named);
        assert!(runtime.join("bin/ucrtbase.dll").exists());
        // Same for a linked bin.
        let named2 = dir.path().join(".named/rt2/Arctic Client");
        std::fs::create_dir_all(&named2).unwrap();
        std::os::unix::fs::symlink(runtime.join("bin"), named2.join("bin")).unwrap();
        remove_system_dlls(&named2);
        assert!(runtime.join("bin/ucrtbase.dll").exists());
    }

    /// Run by the Windows CI job against a real Mojang runtime (`ARCTIC_RUNTIME_DIR`, with
    /// `ARCTIC_HELLO_DIR` holding a compiled `Hello` class): the named copy has no bundled
    /// `ucrtbase.dll`/`api-ms-win-*.dll` and still starts Java.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs a downloaded Mojang runtime"]
    fn named_runtime_without_system_dlls_starts_java() {
        let runtime = PathBuf::from(std::env::var_os("ARCTIC_RUNTIME_DIR").unwrap());
        let hello = PathBuf::from(std::env::var_os("ARCTIC_HELLO_DIR").unwrap());
        let bundled = |dir: &Path| {
            std::fs::read_dir(dir.join("bin"))
                .unwrap()
                .flatten()
                .filter(|e| is_system_dll(&e.file_name()))
                .count()
        };
        assert!(bundled(&runtime) > 0, "the runtime should ship the DLLs");
        let java = runtime.join("bin").join("java.exe");
        // Run twice: the second pass is the "already mirrored" launch.
        for _ in 0..2 {
            let named = in_named_folder(&java);
            assert_ne!(named, java, "the named copy was not made");
            assert!(named.starts_with(runtime.parent().unwrap().join(".named")));
            assert_eq!(bundled(named.parent().unwrap().parent().unwrap()), 0);
            let version = Command::new(&named).arg("-version").output().unwrap();
            assert!(version.status.success(), "{version:?}");
            println!("{}", String::from_utf8_lossy(&version.stderr));
            let run = Command::new(&named)
                .arg("-cp")
                .arg(&hello)
                .arg("Hello")
                .output()
                .unwrap();
            assert!(run.status.success(), "{run:?}");
            assert_eq!(
                String::from_utf8_lossy(&run.stdout).trim(),
                "hello from java"
            );
        }
    }

    #[test]
    fn game_logs_keep_the_runs_before() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("game-vanilla.log");
        let read = |name: &str| std::fs::read_to_string(dir.path().join(name)).ok();
        for run in 1..=7 {
            rotate_game_log(&log);
            std::fs::write(&log, format!("run {run}")).unwrap();
        }
        assert_eq!(read("game-vanilla.log").as_deref(), Some("run 7"));
        assert_eq!(read("game-vanilla.1.log").as_deref(), Some("run 6"));
        assert_eq!(read("game-vanilla.4.log").as_deref(), Some("run 3"));
        assert_eq!(read("game-vanilla.5.log"), None);
    }

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn the_game_runs_as_arctic_client() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir
            .path()
            .join(if cfg!(windows) { "javaw.exe" } else { "java" });
        std::fs::write(&java, b"java v1").unwrap();
        let first = branded_java(&java);
        assert!(first.ends_with(CLIENT_PROCESS), "{}", first.display());
        assert!(dir.path().join(CLIENT_PROCESS).is_file());
        assert_eq!(std::fs::read(&first).unwrap(), b"java v1");
        // A Java update is copied again.
        std::fs::write(&java, b"java v2 (longer)").unwrap();
        assert_eq!(
            std::fs::read(branded_java(&java)).unwrap(),
            b"java v2 (longer)"
        );
        // Started from a folder named after the client (for recorders).
        let runtime = dir.path().join("java-runtime-delta");
        let bin = runtime.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(java.file_name().unwrap()), b"java").unwrap();
        let named = branded_java(&bin.join(java.file_name().unwrap()));
        if cfg!(windows) {
            assert!(
                named.ends_with(format!("Arctic Client/bin/{CLIENT_PROCESS}")),
                "{}",
                named.display()
            );
            assert!(named.starts_with(dir.path().join(".named").join("java-runtime-delta")));
            assert_eq!(std::fs::read(&named).unwrap(), b"java");
            // A second launch reuses the same junction.
            assert_eq!(branded_java(&bin.join(java.file_name().unwrap())), named);
        }
        // Anything that isn't the Java launcher is left alone.
        let other = dir.path().join("custom-java.exe");
        assert_eq!(branded_java(&other), other);
    }

    #[test]
    fn detects_window_markers_in_plain_and_xml_logs() {
        assert!(is_window_marker(
            b"[12:00:01] [Render thread/INFO]: Backend library: LWJGL version 3.3.3"
        ));
        assert!(is_window_marker(
            b"<log4j:Message><![CDATA[Backend library: LWJGL version 3.4.3+4]]></log4j:Message>"
        ));
        assert!(is_window_marker(
            b"2013-04-01 [CLIENT] [INFO] LWJGL Version: 2.9.0"
        ));
        assert!(!is_window_marker(b"Setting user: Steve"));
    }

    #[test]
    fn tee_writes_log_and_fires_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let log = Mutex::new(File::create(&path).unwrap());
        let (window, ready_seen) = (AtomicBool::new(false), AtomicBool::new(false));
        let seen = Seen {
            window: &window,
            ready: &ready_seen,
        };
        let fired = Mutex::new(Vec::new());
        let input =
            b"a\nLWJGL Version: 2\nLWJGL Version: 2\n[Arctic startup] ready 9000 ms\nz".to_vec();
        tee_output(
            Box::new(std::io::Cursor::new(input.clone())),
            &log,
            seen,
            &[],
            &|e| fired.lock().unwrap().push(e),
        );
        drop(log);
        assert_eq!(std::fs::read(&path).unwrap(), input);
        let fired = fired.into_inner().unwrap();
        let ready = fired
            .iter()
            .filter(|e| **e == GameEvent::WindowReady)
            .count();
        let lines: usize = fired
            .iter()
            .map(|e| match e {
                GameEvent::Output(l) => l.len(),
                _ => 0,
            })
            .sum();
        assert_eq!((ready, lines), (1, 5));
        assert!(ready_seen.load(Ordering::Relaxed));
    }

    #[test]
    fn session_tokens_stay_out_of_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let log = Mutex::new(File::create(&path).unwrap());
        let (window, ready) = (AtomicBool::new(false), AtomicBool::new(false));
        let seen = Seen {
            window: &window,
            ready: &ready,
        };
        let input = b"[Client thread/INFO] (Session ID is token:eyJ.SECRET:15ca)
ok SECRET SECRET
"
        .to_vec();
        let shown = Mutex::new(Vec::new());
        tee_output(
            Box::new(std::io::Cursor::new(input)),
            &log,
            seen,
            &["eyJ.SECRET".to_owned(), "SECRET".to_owned()],
            &|e| {
                if let GameEvent::Output(lines) = e {
                    shown
                        .lock()
                        .unwrap()
                        .extend(lines.into_iter().map(|l| l.text));
                }
            },
        );
        drop(log);
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("SECRET"), "{written}");
        assert!(written.contains("token:<redacted>:15ca"));
        assert!(
            shown
                .into_inner()
                .unwrap()
                .iter()
                .all(|l| !l.contains("SECRET"))
        );
    }

    /// OpenJDK's `@argfile` tokenizer (`src/java.base/share/native/libjli/args.c`, `nextToken`).
    /// Inside quotes `\` escapes the next character (`n`, `r`, `t`, `f` become those controls;
    /// anything else, including `\`, is kept as itself). A newline ends a token even inside a
    /// quote. Outside quotes `\` is just a character, and `#` starts a comment.
    fn java_argfile_tokens(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut parser = ArgParser {
            chars: &chars,
            i: 0,
            state: ArgState::FindNext,
            quote: '"',
            parts: Vec::new(),
        };
        let mut tokens = Vec::new();
        while let Some(token) = parser.next_token() {
            tokens.push(token);
        }
        if matches!(parser.state, ArgState::InToken | ArgState::InQuote) && !parser.parts.is_empty()
        {
            tokens.push(parser.parts.concat());
        }
        tokens
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ArgState {
        FindNext,
        InComment,
        InQuote,
        InEscape,
        SkipLeadWs,
        InToken,
    }

    struct ArgParser<'a> {
        chars: &'a [char],
        i: usize,
        state: ArgState,
        quote: char,
        parts: Vec<String>,
    }

    impl ArgParser<'_> {
        fn next_token(&mut self) -> Option<String> {
            let n = self.chars.len();
            let mut anchor = self.i;
            while self.i < n {
                let ch = self.chars[self.i];
                if self.state == ArgState::FindNext || self.state == ArgState::SkipLeadWs {
                    if matches!(ch, ' ' | '\n' | '\r' | '\t' | '\u{000c}') {
                        self.i += 1;
                        continue;
                    }
                    self.state = if self.state == ArgState::FindNext {
                        ArgState::InToken
                    } else {
                        ArgState::InQuote
                    };
                    anchor = self.i;
                } else if self.state == ArgState::InEscape {
                    if ch == '\n' || ch == '\r' {
                        self.state = ArgState::SkipLeadWs;
                    } else {
                        let escaped = match ch {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            'f' => '\u{000c}',
                            other => other,
                        };
                        self.parts.push(escaped.to_string());
                        self.state = ArgState::InQuote;
                    }
                    anchor = self.i + 1;
                    self.i += 1;
                    continue;
                } else if self.state == ArgState::InComment {
                    if ch != '\n' && ch != '\r' {
                        self.i += 1;
                        continue;
                    }
                    anchor = self.i + 1;
                    self.state = ArgState::FindNext;
                    self.i += 1;
                    continue;
                }

                match ch {
                    ' ' | '\t' | '\u{000c}' if self.state == ArgState::InQuote => self.i += 1,
                    ' ' | '\t' | '\u{000c}' | '\n' | '\r' => {
                        let token = self.finish(anchor, self.i);
                        self.i += 1;
                        self.state = ArgState::FindNext;
                        return Some(token);
                    }
                    '#' if self.state != ArgState::InQuote => {
                        self.state = ArgState::InComment;
                        anchor = self.i + 1;
                        self.i += 1;
                    }
                    '\\' if self.state != ArgState::InQuote => self.i += 1,
                    '\\' => {
                        self.push_span(anchor, self.i);
                        self.state = ArgState::InEscape;
                        anchor = self.i + 1;
                        self.i += 1;
                    }
                    '\'' | '"' if self.state == ArgState::InQuote && self.quote != ch => {
                        self.i += 1;
                    }
                    '\'' | '"' => {
                        self.push_span(anchor, self.i);
                        anchor = self.i + 1;
                        if self.state == ArgState::InToken {
                            self.quote = ch;
                            self.state = ArgState::InQuote;
                        } else {
                            self.state = ArgState::InToken;
                        }
                        self.i += 1;
                    }
                    _ => self.i += 1,
                }
            }
            if matches!(self.state, ArgState::InToken | ArgState::InQuote) {
                self.push_span(anchor, n);
            }
            None
        }

        fn push_span(&mut self, from: usize, to: usize) {
            if from < to {
                self.parts.push(self.chars[from..to].iter().collect());
            }
        }

        fn finish(&mut self, anchor: usize, end: usize) -> String {
            self.push_span(anchor, end);
            std::mem::take(&mut self.parts).concat()
        }
    }
}
