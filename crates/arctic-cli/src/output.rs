//! Human vs JSON output, and a single-line progress display.

use std::io::{IsTerminal, Write};
use std::sync::Mutex;
use std::time::Instant;

use arctic_core::ProgressInfo;
use serde_json::{Value, json};

const MB: f64 = 1024.0 * 1024.0;

pub struct Out {
    pub json: bool,
    /// Start of the current progress display, for speed.
    started: Mutex<Option<Instant>>,
}

impl Out {
    pub fn new(json: bool) -> Self {
        Self {
            json,
            started: Mutex::new(None),
        }
    }

    /// A result record: JSON object in `--json` mode, `human` text otherwise.
    pub fn emit(&self, value: Value, human: impl FnOnce() -> String) {
        if self.json {
            println!("{value}");
        } else {
            println!("{}", human());
        }
    }

    /// Something worth knowing that isn't the result (stderr for humans).
    pub fn info(&self, message: &str) {
        self.note("info", message);
    }

    /// Something that went only partly right.
    pub fn warn(&self, message: &str) {
        self.note("warning", message);
    }

    fn note(&self, event: &str, message: &str) {
        if self.json {
            println!("{}", json!({ "event": event, "message": message }));
        } else if event == "warning" {
            eprintln!("warning: {message}");
        } else {
            eprintln!("{message}");
        }
    }

    /// Progress goes to stderr (humans) or as `{"event":"progress"}` lines.
    pub fn progress(&self, p: ProgressInfo) {
        if self.json {
            println!(
                "{}",
                json!({
                    "event": "progress",
                    "stage": p.stage,
                    "files_done": p.done,
                    "files_total": p.total,
                    "bytes_done": p.bytes_done,
                    "bytes_total": p.bytes_total,
                })
            );
            return;
        }
        if !std::io::stderr().is_terminal() {
            return;
        }
        let started = {
            let mut slot = self.started.lock().unwrap_or_else(|e| e.into_inner());
            *slot.get_or_insert_with(Instant::now)
        };
        let line = if p.bytes_total > 0 {
            let secs = started.elapsed().as_secs_f64().max(0.1);
            let pct = p.bytes_done as f64 / p.bytes_total as f64 * 100.0;
            format!(
                "{}  {pct:5.1}%  {:.1}/{:.1} MB  {:.1} MB/s",
                p.stage,
                p.bytes_done as f64 / MB,
                p.bytes_total as f64 / MB,
                p.bytes_done as f64 / MB / secs
            )
        } else {
            format!("{}…", p.stage)
        };
        let mut err = std::io::stderr();
        let _ = write!(err, "\r\x1b[2K{line}");
        let _ = err.flush();
    }

    /// Clear the progress line before printing results.
    pub fn end_progress(&self) {
        if !self.json && std::io::stderr().is_terminal() {
            eprint!("\r\x1b[2K");
        }
    }
}

/// Parse `4G`, `4096M`, `4096` (MiB) into MiB.
pub fn parse_memory(text: &str) -> Option<u32> {
    let t = text.trim().to_ascii_uppercase();
    let (num, mult) = if let Some(n) = t.strip_suffix('G').or_else(|| t.strip_suffix("GB")) {
        (n, 1024.0)
    } else if let Some(n) = t.strip_suffix('M').or_else(|| t.strip_suffix("MB")) {
        (n, 1.0)
    } else {
        (t.as_str(), 1.0)
    };
    let value: f64 = num.trim().parse().ok()?;
    (value > 0.0).then(|| (value * mult).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sizes() {
        assert_eq!(parse_memory("4G"), Some(4096));
        assert_eq!(parse_memory("1.5g"), Some(1536));
        assert_eq!(parse_memory("6144M"), Some(6144));
        assert_eq!(parse_memory("2048"), Some(2048));
        assert_eq!(parse_memory("lots"), None);
        assert_eq!(parse_memory("0"), None);
    }
}
