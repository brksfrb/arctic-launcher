//! Look only at APPCRASH signatures. LoadedModule entries are inventory.
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::{Fault, java_exe, process_id};

const MAX_BYTES: u64 = 256 * 1024;
const SCAN_TIME: Duration = Duration::from_secs(1);

fn decode(bytes: &[u8]) -> Option<String> {
    let (bytes, little) = if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        (rest, true)
    } else if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        (rest, false)
    } else if bytes.get(1) == Some(&0) {
        (bytes, true)
    } else {
        return std::str::from_utf8(bytes)
            .ok()
            .map(|s| s.trim_start_matches('\u{feff}').to_owned());
    };
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    let units: Vec<_> = pairs
        .iter()
        .map(|&p| {
            if little {
                u16::from_le_bytes(p)
            } else {
                u16::from_be_bytes(p)
            }
        })
        .collect();
    String::from_utf16(&units).ok()
}

struct Report {
    fault: Fault,
    has_pid: bool,
    event_time: Option<SystemTime>,
}

fn basename(value: &str) -> &str {
    value.rsplit(['\\', '/']).next().unwrap_or("").trim()
}

fn parse(bytes: &[u8], exe: &str, pid: u32) -> Option<Report> {
    let text = decode(bytes)?;
    let fields: HashMap<_, _> = text
        .lines()
        .filter_map(|line| {
            let (k, v) = line.split_once('=')?;
            // LoadedModule and other unrelated fields may contain user paths.
            (!k.starts_with("LoadedModule[")).then_some((k.trim(), v.trim()))
        })
        .collect();
    if !java_exe(basename(exe)) || !fields.get("EventType")?.eq_ignore_ascii_case("APPCRASH") {
        return None;
    }

    let mut sig = HashMap::new();
    for (&key, &name) in &fields {
        if key.starts_with("Sig[")
            && let Some(prefix) = key.strip_suffix("].Name")
            && let Some(&value) = fields.get(format!("{prefix}].Value").as_str())
        {
            sig.insert(name, value);
        }
    }
    let app_path = fields.get("AppPath");
    let app_name = sig.get("Application Name");
    app_path.or(app_name)?;
    if [app_path, app_name]
        .into_iter()
        .flatten()
        .any(|s| !basename(s).eq_ignore_ascii_case(basename(exe)))
    {
        return None;
    }

    let mut has_pid = false;
    for (&k, &v) in &fields {
        let id = if ["ProcessId", "AppProcessId", "FaultingProcessId", "Pid"]
            .iter()
            .any(|name| k.eq_ignore_ascii_case(name))
        {
            process_id(v)?
        } else if k == "AppSessionGuid" {
            // WER encodes the process ID in the first DWORD of the session GUID.
            let guid = uuid::Uuid::parse_str(v).ok()?;
            if guid.is_nil() {
                continue;
            }
            guid.as_fields().0
        } else {
            continue;
        };
        if id != pid {
            return None;
        }
        has_pid = true;
    }
    // With no PID, distinguish different Java installations when both paths are known.
    if !has_pid
        && exe.contains(['\\', '/'])
        && let Some(path) = app_path
    {
        let normal = |s: &str| s.trim_start_matches(r"\\?\").replace('/', "\\");
        if !normal(path).eq_ignore_ascii_case(&normal(exe)) {
            return None;
        }
    }
    let event_time = if let Some(ticks) = fields.get("EventTime") {
        // Decimal FILETIME, in 100 ns ticks since 1601.
        let ticks = ticks
            .parse::<u64>()
            .ok()?
            .checked_sub(116_444_736_000_000_000)?;
        Some(SystemTime::UNIX_EPOCH.checked_add(Duration::from_micros(ticks / 10))?)
    } else {
        None
    };
    let module = basename(sig.get("Fault Module Name")?);
    if module.is_empty() || module.contains(';') || module.chars().any(char::is_control) {
        return None;
    }
    let hex = |name| {
        sig.get(name)
            .and_then(|s| {
                let digits = s
                    .strip_prefix("0x")
                    .or_else(|| s.strip_prefix("0X"))
                    .unwrap_or(s);
                u64::from_str_radix(digits, 16)
                    .ok()
                    .map(|_| format!("0x{digits}"))
            })
            .unwrap_or_else(|| "unknown".into())
    };
    let version = sig
        .get("Fault Module Version")
        .copied()
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .unwrap_or("unknown");
    Some(Report {
        fault: Fault {
            module: module.into(),
            version: version.into(),
            exception: hex("Exception Code"),
            offset: hex("Exception Offset"),
        },
        has_pid,
        event_time,
    })
}

fn read_report(
    path: &Path,
    exe: &str,
    pid: u32,
    start: SystemTime,
    exit: SystemTime,
) -> Option<Report> {
    let meta = path.symlink_metadata().ok()?;
    let created = meta.created().ok()?;
    let latest = SystemTime::now();
    if !meta.is_file() || meta.len() > MAX_BYTES || created < start || created > latest {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_BYTES {
        return None;
    }
    let report = parse(&bytes, exe, pid)?;
    // Without a PID, a report from earlier in a long-running session could be another Java.
    let earliest = if report.has_pid {
        start
    } else {
        start.max(exit - Duration::from_secs(15))
    };
    let time = report.event_time.unwrap_or(created);
    (earliest..=latest).contains(&time).then_some(report)
}

fn scan(
    roots: impl IntoIterator<Item = PathBuf>,
    exe: &str,
    pid: u32,
    start: SystemTime,
    exit: SystemTime,
    deadline: Instant,
) -> Option<Fault> {
    let mut candidate = None;
    let mut ambiguous = false;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if Instant::now() >= deadline {
                return None;
            }
            if !entry
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("appcrash_")
                || !entry.metadata().is_ok_and(|m| {
                    m.is_dir() && !m.is_symlink() && m.created().is_ok_and(|t| t >= start)
                })
            {
                continue;
            }
            if let Some(report) =
                read_report(&entry.path().join("Report.wer"), exe, pid, start, exit)
            {
                if report.has_pid {
                    return Some(report.fault);
                }
                if candidate.as_ref().is_some_and(|f| f != &report.fault) {
                    ambiguous = true;
                }
                candidate = Some(report.fault);
            }
        }
    }
    if ambiguous { None } else { candidate }
}

#[cfg(windows)]
pub(super) fn collect(exe: &str, pid: u32, start: SystemTime, exit: SystemTime) -> Option<Fault> {
    let exe = exe.to_owned();
    let deadline = Instant::now() + SCAN_TIME;
    let (tx, rx) = std::sync::mpsc::channel();
    // Even a stalled filesystem read must not hold up the crash report.
    std::thread::Builder::new()
        .spawn(move || {
            let system =
                std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
            let roots = std::iter::once(system)
                .chain(std::env::var_os("LOCALAPPDATA"))
                .flat_map(|base| {
                    let wer = PathBuf::from(base).join(r"Microsoft\Windows\WER");
                    [wer.join("ReportArchive"), wer.join("ReportQueue")]
                });
            let _ = tx.send(scan(roots, &exe, pid, start, exit, deadline));
        })
        .ok()?;
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()?
}

#[cfg(test)]
mod tests {
    use super::*;

    const WER: &str = r"Version=1
EventType=APPCRASH
AppSessionGuid=00001234-0001-000a-4f65-de7743e5da01
Sig[0].Name=Application Name
Sig[0].Value=Arctic Client.exe
Sig[3].Name=Fault Module Name
Sig[4].Name=Fault Module Version
Sig[4].Value=27.20.100.8681
Sig[3].Value=C:\Users\Renée\ig9icd64.dll
Sig[6].Name=Exception Code
Sig[6].Value=c0000005
Sig[7].Name=Exception Offset
Sig[7].Value=00001234
LoadedModule[0]=C:\Windows\jvm.dll
AppPath=C:\Users\Renée\Arctic Client.exe
";
    const EXE: &str = "Arctic Client.exe";
    const PID: u32 = 0x1234;

    #[test]
    fn reads_named_appcrash_signatures_in_utf16_and_utf8() {
        let text = WER.replace('\n', "\r\n");
        let utf16: Vec<_> = std::iter::once(0xfeff)
            .chain(text.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        for bytes in [&utf16[..], text.as_bytes()] {
            let report = parse(bytes, EXE, PID).unwrap();
            assert!(report.has_pid);
            assert_eq!(
                report.fault,
                Fault {
                    module: "ig9icd64.dll".into(),
                    version: "27.20.100.8681".into(),
                    exception: "0xc0000005".into(),
                    offset: "0x00001234".into(),
                }
            );
        }
    }

    #[test]
    fn rejects_other_processes_and_inventory_only_reports() {
        assert!(parse(WER.as_bytes(), EXE, 123).is_none());
        for extra in [
            "ProcessId=123",
            "AppPath=C:\\javaw.exe",
            "EventType=AppHangB1",
        ] {
            assert!(parse(format!("{WER}{extra}").as_bytes(), EXE, PID).is_none());
        }
        let no_module = WER.replace("Fault Module Name", "Other Field");
        assert!(parse(no_module.as_bytes(), EXE, PID).is_none());
        let java = WER.replace(EXE, "javaw.exe");
        assert!(parse(java.as_bytes(), "JAVAW.EXE", PID).is_some());

        let pidless = java.replace("AppSessionGuid=", "Unused=");
        assert!(parse(pidless.as_bytes(), r"C:\Other\javaw.exe", PID).is_none());
        assert!(parse(pidless.as_bytes(), r"C:\Users\Renée\javaw.exe", PID).is_some());
    }

    #[test]
    fn scan_requires_a_recent_report_and_prefers_pid_over_ambiguous_matches() {
        let dir = tempfile::tempdir().unwrap();
        let start = SystemTime::now() - Duration::from_secs(1);
        let exit = SystemTime::now();
        let scan_dir = |since, exit| {
            scan(
                [dir.path().to_owned()],
                EXE,
                PID,
                since,
                exit,
                Instant::now() + SCAN_TIME,
            )
        };
        let write = |folder: &str, text: &str| {
            let path = dir.path().join(folder);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("Report.wer"), text).unwrap();
        };
        let pidless = WER
            .lines()
            .filter(|s| !s.starts_with("AppSessionGuid="))
            .collect::<Vec<_>>()
            .join("\n");
        write(
            "AppCrash_Arctic Client.exe_one",
            &format!("{pidless}\nEventTime=116444736000000000"),
        );
        assert!(scan_dir(start, exit).is_none());
        write(
            "AppCrash_Arctic Client.exe_one",
            &format!("{pidless}\n{}", "x".repeat(MAX_BYTES as usize)),
        );
        assert!(scan_dir(start, exit).is_none());
        write("AppCrash_Arctic Client.exe_one", &pidless);
        assert_eq!(scan_dir(start, exit).unwrap().module, "ig9icd64.dll");
        assert!(scan_dir(exit + Duration::from_secs(2), exit + Duration::from_secs(3)).is_none());
        assert!(scan_dir(start, exit + Duration::from_secs(30)).is_none());
        write(
            "AppCrash_Arctic Client.exe_two",
            &pidless.replace("ig9icd64.dll", "other.dll"),
        );
        assert!(scan_dir(start, exit).is_none());
        write(
            "AppCrash_Arctic Client.exe_three",
            &format!("{pidless}\nProcessId=0x1234\n"),
        );
        assert_eq!(scan_dir(start, exit).unwrap().module, "ig9icd64.dll");
    }
}
