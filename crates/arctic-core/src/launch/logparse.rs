//! Turn raw game output into readable log lines.
//!
//! Modern versions print log4j XML events (because of the official logging
//! config); older ones print plain text. XML events are collapsed into
//! `[thread/LEVEL] message` lines, with stack traces kept as extra lines.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub level: Level,
    pub text: String,
}

/// Largest XML event we buffer before giving up and flushing it raw.
const MAX_EVENT_BYTES: usize = 256 * 1024;

/// Stateful per-stream parser (XML events can span many lines).
#[derive(Debug, Default)]
pub struct LogParser {
    event: Option<String>,
}

impl LogParser {
    /// Feed one raw line; returns zero or more finished log lines.
    pub fn feed(&mut self, raw: &str) -> Vec<LogLine> {
        let line = raw.trim_end_matches(['\r', '\n']);
        if let Some(buf) = &mut self.event {
            buf.push('\n');
            buf.push_str(line);
            if line.contains("</log4j:Event>") {
                let xml = self.event.take().unwrap_or_default();
                return parse_event(&xml);
            }
            // A never-closed event must not swallow the rest of the stream.
            if buf.len() > MAX_EVENT_BYTES {
                return self.finish();
            }
            return Vec::new();
        }
        if line.trim_start().starts_with("<log4j:Event") {
            if line.contains("</log4j:Event>") {
                return parse_event(line);
            }
            self.event = Some(line.to_owned());
            return Vec::new();
        }
        if line.trim().is_empty() {
            return Vec::new();
        }
        vec![LogLine {
            level: guess_level(line),
            text: line.to_owned(),
        }]
    }
}

impl LogParser {
    /// Flush a half-received event (stream ended or event too large) as
    /// plain lines, so e.g. a crash message cut off mid-write still shows.
    pub fn finish(&mut self) -> Vec<LogLine> {
        let Some(buf) = self.event.take() else {
            return Vec::new();
        };
        buf.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| LogLine {
                level: guess_level(l),
                text: l.to_owned(),
            })
            .collect()
    }
}

fn parse_event(xml: &str) -> Vec<LogLine> {
    let level = match attr(xml, "level").unwrap_or("INFO") {
        "WARN" => Level::Warn,
        "ERROR" | "FATAL" => Level::Error,
        "DEBUG" | "TRACE" => Level::Debug,
        _ => Level::Info,
    };
    let thread = attr(xml, "thread").unwrap_or("main");
    let message = cdata_in(xml, "log4j:Message").unwrap_or_default();
    let mut lines = vec![LogLine {
        level,
        text: format!("[{thread}/{}] {message}", level_name(level)),
    }];
    if let Some(trace) = cdata_in(xml, "log4j:Throwable") {
        lines.extend(
            trace
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| LogLine {
                    level,
                    text: format!("    {}", l.trim_end()),
                }),
        );
    }
    lines
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Debug => "DEBUG",
        Level::Info => "INFO",
        Level::Warn => "WARN",
        Level::Error => "ERROR",
    }
}

fn attr<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = xml.find(&key)? + key.len();
    let end = xml[start..].find('"')? + start;
    Some(&xml[start..end])
}

fn cdata_in(xml: &str, tag: &str) -> Option<String> {
    let open = xml.find(&format!("<{tag}>"))?;
    let start = xml[open..].find("<![CDATA[")? + open + "<![CDATA[".len();
    let end = xml[start..].find("]]>")? + start;
    Some(xml[start..end].to_owned())
}

/// Best-effort level for plain-text lines like `[12:00:00] [main/WARN]: …`.
fn guess_level(line: &str) -> Level {
    const ERROR_TAGS: [&str; 4] = ["/ERROR]", "[ERROR]", "[SEVERE]", "/FATAL]"];
    const WARN_TAGS: [&str; 3] = ["/WARN]", "[WARN]", "[WARNING]"];
    const DEBUG_TAGS: [&str; 2] = ["/DEBUG]", "[DEBUG]"];
    let upper = line.to_ascii_uppercase();
    let has = |tags: &[&str]| tags.iter().any(|t| upper.contains(t));
    let stack_trace =
        line.starts_with("	at ") || line.starts_with("Caused by:") || upper.contains("EXCEPTION");
    if has(&ERROR_TAGS) || stack_trace {
        Level::Error
    } else if has(&WARN_TAGS) {
        Level::Warn
    } else if has(&DEBUG_TAGS) {
        Level::Debug
    } else {
        Level::Info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_multiline_xml_event() {
        let mut p = LogParser::default();
        let input = [
            r#"<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1" level="INFO" thread="Render thread">"#,
            r#"  <log4j:Message><![CDATA[Setting user: Steve]]></log4j:Message>"#,
            r#"</log4j:Event>"#,
        ];
        let out: Vec<LogLine> = input.iter().flat_map(|l| p.feed(l)).collect();
        assert_eq!(
            out,
            [LogLine {
                level: Level::Info,
                text: "[Render thread/INFO] Setting user: Steve".into()
            }]
        );
    }

    #[test]
    fn keeps_stack_traces_with_event_level() {
        let mut p = LogParser::default();
        let input = [
            r#"<log4j:Event logger="x" timestamp="1" level="ERROR" thread="main">"#,
            r#"  <log4j:Message><![CDATA[Boom]]></log4j:Message>"#,
            r#"  <log4j:Throwable><![CDATA[java.lang.RuntimeException: bad"#,
            "\tat a.b.C.run(C.java:1)",
            r#"]]></log4j:Throwable>"#,
            r#"</log4j:Event>"#,
        ];
        let out: Vec<LogLine> = input.iter().flat_map(|l| p.feed(l)).collect();
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|l| l.level == Level::Error));
        assert!(out[2].text.contains("at a.b.C.run"));
    }

    #[test]
    fn unterminated_event_is_flushed_on_finish() {
        let mut p = LogParser::default();
        assert!(
            p.feed(r#"<log4j:Event level="ERROR" thread="main">"#)
                .is_empty()
        );
        assert!(p.feed("  <log4j:Message><![CDATA[Crash!").is_empty());
        let out = p.finish();
        assert_eq!(out.len(), 2);
        assert!(out[1].text.contains("Crash!"));
        assert!(p.finish().is_empty());
    }

    #[test]
    fn oversized_event_is_flushed_early() {
        let mut p = LogParser::default();
        p.feed(r#"<log4j:Event level="INFO" thread="main">"#);
        let big = "x".repeat(MAX_EVENT_BYTES + 1);
        assert!(!p.feed(&big).is_empty());
        assert_eq!(p.feed("after")[0].text, "after");
    }

    #[test]
    fn plain_lines_get_levels() {
        let mut p = LogParser::default();
        assert_eq!(
            p.feed("[12:00:00] [main/WARN]: careful")[0].level,
            Level::Warn
        );
        assert_eq!(
            p.feed("2013-01-01 [CLIENT] [SEVERE] oops")[0].level,
            Level::Error
        );
        assert_eq!(p.feed("LWJGL Version: 2.9.4")[0].level, Level::Info);
        assert!(p.feed("   ").is_empty());
    }
}
