use std::collections::VecDeque;

use crate::crash::Diagnosis;

pub const DLL_NOT_FOUND: i32 = 0xC000_0135_u32 as i32;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExitDiagnostics {
    pub output: ChildOutput,
    dll: Option<DllFailure>,
}

impl ExitDiagnostics {
    pub(crate) fn dll_report(&self) -> Option<&str> {
        self.dll.as_ref().map(|dll| dll.report.as_str())
    }

    pub fn dll_diagnosis(&self) -> Diagnosis {
        let detail = match self.dll.as_ref().and_then(|dll| dll.cause) {
            Some(DllCause::NamedCopy) => {
                "The Arctic copy of Java may be damaged. Share this crash report with Arctic support so they can check the runtime files."
            }
            Some(DllCause::Windows) => {
                "Java could not load a library it needs. Your Windows version may be too old or missing the Microsoft Visual C++ runtime. Update Windows and install or repair the Microsoft Visual C++ Redistributable."
            }
            _ => {
                "Windows could not find a library the game needs. An old Windows version, a missing Microsoft Visual C++ runtime, or damaged Java files can cause this. The crash report has the results of the checks Arctic could run."
            }
        };
        Diagnosis::new("Windows couldn't load a required DLL", detail, Vec::new())
    }

    #[cfg(windows)]
    pub(super) fn check_dlls(
        &mut self,
        source: &std::path::Path,
        exe: &std::path::Path,
        command: impl Fn(&std::path::Path) -> std::process::Command,
    ) {
        self.dll = Some(DllFailure::collect(source, exe, command));
    }
}

/// Bounded, redacted child output, including lines the UI parser doesn't display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChildOutput {
    count: usize,
    head: String,
    tail: VecDeque<String>,
    tail_bytes: usize,
}

impl ChildOutput {
    const HALF_BUDGET: usize = 4 * 1024;
    const LINE_LIMIT: usize = 2 * 1024;

    pub(super) fn push(&mut self, stream: &str, raw: &str) {
        self.count += 1;
        let raw = raw.trim_end_matches(['\r', '\n']);
        let line = if raw.len() > Self::LINE_LIMIT {
            let end = raw.floor_char_boundary(Self::LINE_LIMIT);
            format!("[{stream}] {} [line truncated]\n", &raw[..end])
        } else {
            format!("[{stream}] {raw}\n")
        };
        if self.tail.is_empty() && self.head.len() + line.len() <= Self::HALF_BUDGET {
            self.head.push_str(&line);
            return;
        }
        self.tail_bytes += line.len();
        self.tail.push_back(line);
        while self.tail_bytes > Self::HALF_BUDGET {
            self.tail_bytes -= self.tail.pop_front().unwrap().len();
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn report(&self) -> String {
        let mut text = self.head.clone();
        let kept = self.head.lines().count() + self.tail.len();
        if kept < self.count {
            text.push_str(&format!("[{} lines omitted]\n", self.count - kept));
        }
        text.extend(self.tail.iter().map(String::as_str));
        text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DllFailure {
    report: String,
    cause: Option<DllCause>,
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DllCause {
    NamedCopy,
    Windows,
}

impl DllCause {
    #[cfg(any(windows, test))]
    fn from_checks(
        unexpected_missing: bool,
        source_status: Option<i32>,
        branded_status: Option<i32>,
        system_missing: bool,
    ) -> Option<Self> {
        if unexpected_missing || (source_status == Some(0) && branded_status == Some(DLL_NOT_FOUND))
        {
            Some(Self::NamedCopy)
        } else if source_status == Some(DLL_NOT_FOUND)
            || (source_status != Some(0) && system_missing)
        {
            Some(Self::Windows)
        } else {
            None
        }
    }
}

#[cfg(windows)]
impl DllFailure {
    fn collect(
        source: &std::path::Path,
        exe: &std::path::Path,
        command: impl Fn(&std::path::Path) -> std::process::Command,
    ) -> Self {
        use crate::system::windows::{self, UCRT, VCRUNTIME};
        use std::ffi::OsStr;
        use std::io;
        use std::path::Path;

        let (mut bin_report, mut unexpected_missing) = (String::new(), false);
        match source.parent().zip(exe.parent()) {
            Some((source_bin, named_bin)) if source_bin != named_bin => {
                match super::named_runtime::missing_bin_files(source_bin, named_bin) {
                    Ok(missing) if missing.is_empty() => {
                        bin_report.push_str("Named bin contains every file in source bin.\n");
                    }
                    Ok(missing) => {
                        let uses_system_ucrt = windows::can_use_system_ucrt();
                        bin_report
                            .push_str("Files missing from named bin (relative to source bin)\n");
                        for path in missing {
                            let expected = uses_system_ucrt
                                && path.parent() == Some(Path::new(""))
                                && path
                                    .file_name()
                                    .is_some_and(super::named_runtime::is_system_dll);
                            unexpected_missing |= !expected;
                            let reason = if expected {
                                " (omitted in favor of the Windows UCRT)"
                            } else {
                                ""
                            };
                            bin_report.push_str(&format!("  {}{reason}\n", path.display()));
                        }
                    }
                    Err(error) => {
                        bin_report.push_str(&format!("Named bin comparison failed: {error}\n"))
                    }
                }
            }
            _ => bin_report.push_str("Named bin comparison: game used the source bin directory.\n"),
        }

        let mut report = String::new();
        let system_dir = windows::system_directory();
        let check = |name| match &system_dir {
            Ok(dir) => windows::has_dll(dir, name),
            Err(error) => Err(io::Error::other(error.to_string())),
        };
        let mut system_missing = false;
        for name in [UCRT, VCRUNTIME] {
            let state = match check(name) {
                Ok(true) => "present".into(),
                Ok(false) => {
                    system_missing = true;
                    "missing".into()
                }
                Err(error) => format!("check failed: {error}"),
            };
            report.push_str(&format!("Windows system directory {name}: {state}\n"));
        }

        let source_name = source
            .file_name()
            .unwrap_or(OsStr::new("Java"))
            .to_string_lossy();
        let source_status = probe_version(
            &mut report,
            &format!("Source {source_name}"),
            command(source),
        );
        // A game-specific native library can fail even when its Java executable starts.
        let branded_status = if source_status == Some(0) && source != exe && !unexpected_missing {
            probe_version(&mut report, "Branded Arctic Client.exe", command(exe))
        } else {
            None
        };
        report.push_str(&bin_report);
        let cause = DllCause::from_checks(
            unexpected_missing,
            source_status,
            branded_status,
            system_missing,
        );
        Self { report, cause }
    }
}

#[cfg(windows)]
fn probe_version(report: &mut String, name: &str, probe: std::process::Command) -> Option<i32> {
    let (code, description) = match super::process::runtime_probe_command(probe) {
        Ok(Some((status, output))) => {
            let code = status.code();
            let status_text =
                code.map_or_else(|| status.to_string(), crate::crash::exit_description);
            (
                code,
                format!(
                    "exit code {status_text}\n{}",
                    crate::crash::scrubbed_tail(&output, 2048)
                ),
            )
        }
        Ok(None) => (None, "timed out after 3 s".into()),
        Err(error) => (None, format!("could not run: {error}")),
    };
    report.push_str(&format!("{name} -version: {description}\n"));
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dll_advice_distinguishes_the_two_javas_from_a_system_failure() {
        use DllCause::{NamedCopy, Windows};
        let check = DllCause::from_checks;
        assert_eq!(check(true, None, None, true), Some(NamedCopy));
        assert_eq!(
            check(false, Some(0), Some(DLL_NOT_FOUND), false),
            Some(NamedCopy)
        );
        assert_eq!(
            check(false, Some(DLL_NOT_FOUND), None, false),
            Some(Windows)
        );
        assert_eq!(check(false, None, None, true), Some(Windows));
        assert_eq!(check(false, Some(0), Some(0), true), None);
        assert_eq!(check(false, None, None, false), None);
    }

    #[test]
    fn preserves_short_output_and_bounds_verbose_output() {
        let mut child = ChildOutput::default();
        child.push("stdout", "starting\r\n");
        child.push("stderr", "\n");
        child.push("stderr", "missing library");
        assert_eq!(child.count(), 3);
        assert_eq!(
            child.report(),
            "[stdout] starting\n[stderr] \n[stderr] missing library\n"
        );
        for _ in 0..100 {
            child.push("stdout", &"界".repeat(3000));
        }
        child.push("stderr", "last line");
        let report = child.report();
        assert_eq!(child.count(), 104);
        assert!(report.starts_with("[stdout] starting\n"));
        assert!(report.contains(" lines omitted]"));
        assert!(report.ends_with("[stderr] last line\n"));
        assert!(report.len() < 2 * ChildOutput::HALF_BUDGET + 64);
    }
}
