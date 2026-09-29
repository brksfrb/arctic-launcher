//! `arctic screenshots …`, `arctic replays …` and `arctic logs`.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use arctic_core::{Error, Result, replays, screenshots};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::{LogLevelArg, LogsArgs, ReplaysCommand, ScreenshotsCommand};

/// How often `logs --follow` looks for new lines.
const FOLLOW_POLL: Duration = Duration::from_millis(500);

pub fn screenshots(ctx: &Ctx, command: &ScreenshotsCommand) -> Result<i32> {
    match command {
        ScreenshotsCommand::List { instance, limit } => {
            let only = match instance {
                Some(q) => Some(arctic_core::instances::find(&ctx.dirs, q)?.id),
                None => None,
            };
            let shots = screenshots::list(&ctx.dirs)
                .into_iter()
                .filter(|s| only.as_ref().is_none_or(|id| &s.instance_id == id))
                .take(limit.unwrap_or(usize::MAX));
            for s in shots {
                let name = file_name(&s.path);
                ctx.out.emit(
                    json!({
                        "file": name,
                        "path": s.path,
                        "instance": s.instance_id,
                        "taken": unix(s.taken),
                        "size": s.size,
                    }),
                    || format!("{name:<28} {:<20} {}", s.instance_name, s.path.display()),
                );
            }
        }
        ScreenshotsCommand::Remove { file } => {
            let shot = screenshots::list(&ctx.dirs)
                .into_iter()
                .find(|s| file_name(&s.path) == *file || s.path == Path::new(file))
                .ok_or_else(|| {
                    Error::Other(format!(
                        "no screenshot '{file}' (see `arctic screenshots list`)"
                    ))
                })?;
            screenshots::trash(&shot)?;
            ctx.out.emit(json!({"event": "removed", "file": file}), || {
                format!("Moved {file} to .trash")
            });
        }
    }
    Ok(0)
}

pub fn replays(ctx: &Ctx, command: &ReplaysCommand) -> Result<i32> {
    match command {
        ReplaysCommand::List { instance } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            for r in replays::list(&inst.game_dir(&ctx.dirs)) {
                let minutes = r.duration.as_secs() / 60;
                let seconds = r.duration.as_secs() % 60;
                ctx.out.emit(
                    json!({
                        "name": r.name,
                        "path": r.path,
                        "server": r.server,
                        "recorded": r.recorded.map(unix),
                        "duration_ms": r.duration.as_millis() as u64,
                        "version": r.mc_version,
                        "clips": r.clips,
                        "size": r.size,
                    }),
                    || {
                        format!(
                            "{:<36} {minutes:>3}:{seconds:02}  {:<8} {}",
                            r.name, r.mc_version, r.server
                        )
                    },
                );
            }
            ctx.out.info(&format!(
                "Watch one with: arctic launch --instance {} --replay FILE",
                inst.id
            ));
        }
        ReplaysCommand::Remove { replay, instance } => {
            let inst = instance_or_default(ctx, instance.as_deref())?;
            let found = replays::list(&inst.game_dir(&ctx.dirs))
                .into_iter()
                .find(|r| r.name == *replay || r.path == Path::new(replay))
                .ok_or_else(|| {
                    Error::Other(format!(
                        "no replay '{replay}' in {} (see `arctic replays list`)",
                        inst.name
                    ))
                })?;
            replays::trash(&found)?;
            ctx.out
                .emit(json!({"event": "removed", "name": found.name}), || {
                    format!("Moved {} to .trash", found.name)
                });
        }
    }
    Ok(0)
}

pub fn logs(ctx: &Ctx, args: &LogsArgs) -> Result<i32> {
    let path = log_path(ctx, args)?;
    if args.path {
        ctx.out
            .emit(json!({"path": path}), || path.display().to_string());
        return Ok(0);
    }
    let bytes = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().filter(|l| wanted(l, args.level)).collect();
    let skip = args.lines.map_or(0, |n| lines.len().saturating_sub(n));
    for line in &lines[skip..] {
        print_line(ctx, line);
    }
    if args.follow {
        follow(ctx, &path, bytes.len() as u64, args.level)?;
    }
    Ok(0)
}

/// The game log the launcher keeps (or the game's own `latest.log`), or
/// the launcher's log.
fn log_path(ctx: &Ctx, args: &LogsArgs) -> Result<PathBuf> {
    if args.launcher {
        return Ok(ctx.root.launcher_logs().join("launcher.log"));
    }
    let inst = instance_or_default(ctx, args.instance.as_deref())?;
    let kept = ctx.dirs.logs().join(format!("game-{}.log", inst.id));
    let latest = inst.game_dir(&ctx.dirs).join("logs").join("latest.log");
    let newest = [kept, latest]
        .into_iter()
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(when, _)| *when)
        .map(|(_, p)| p);
    newest.ok_or_else(|| Error::Other(format!("{} has no game log yet", inst.name)))
}

/// Minecraft's lines look like `[time] [thread/LEVEL]: message`.
fn wanted(line: &str, level: Option<LogLevelArg>) -> bool {
    match level {
        None => true,
        Some(LogLevelArg::Error) => {
            line.contains("/ERROR]") || line.contains("/FATAL]") || line.contains("[ERROR]")
        }
        Some(LogLevelArg::Warn) => {
            wanted(line, Some(LogLevelArg::Error))
                || line.contains("/WARN]")
                || line.contains("[WARN]")
        }
    }
}

fn print_line(ctx: &Ctx, line: &str) {
    ctx.out.emit(json!({"line": line}), || line.to_owned());
}

/// Print lines appended after `from` until interrupted.
fn follow(ctx: &Ctx, path: &Path, mut from: u64, level: Option<LogLevelArg>) -> Result<()> {
    let mut partial = String::new();
    loop {
        std::thread::sleep(FOLLOW_POLL);
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if len < from {
            // A new game started a new log.
            from = 0;
        }
        if len == from {
            continue;
        }
        let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
        file.seek(SeekFrom::Start(from))
            .map_err(|e| Error::io(path, e))?;
        let mut chunk = Vec::new();
        file.read_to_end(&mut chunk)
            .map_err(|e| Error::io(path, e))?;
        from += chunk.len() as u64;
        partial.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = partial.find('\n') {
            let line: String = partial.drain(..=end).collect();
            let line = line.trim_end_matches(['\r', '\n']);
            if wanted(line, level) {
                print_line(ctx, line);
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_log_levels() {
        let info = "[12:00:00] [Render thread/INFO]: hello";
        let warn = "[12:00:00] [Render thread/WARN]: careful";
        let error = "[12:00:00] [Render thread/ERROR]: broke";
        assert!(wanted(info, None));
        assert!(!wanted(info, Some(LogLevelArg::Warn)));
        assert!(wanted(warn, Some(LogLevelArg::Warn)));
        assert!(!wanted(warn, Some(LogLevelArg::Error)));
        assert!(wanted(error, Some(LogLevelArg::Warn)));
        assert!(wanted(error, Some(LogLevelArg::Error)));
    }
}
