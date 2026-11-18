//! Best-effort Windows native crash evidence. Never infer a culprit from loaded DLLs.

#[cfg(any(windows, test))]
mod wer;

#[cfg(windows)]
use std::time::{Duration, Instant, SystemTime};

pub(crate) fn is_native(code: i32) -> bool {
    matches!(
        code as u32,
        0xC0000005 | 0xC0000409 | 0xC0000374 | 0xC000001D
    )
}

#[cfg(any(windows, test))]
#[derive(Debug, PartialEq, Eq)]
struct Fault {
    module: String,
    version: String,
    exception: String,
    offset: String,
}

#[cfg(any(windows, test))]
fn java_exe(name: &str) -> bool {
    ["javaw.exe", "java.exe", "Arctic Client.exe"]
        .iter()
        .any(|exe| exe.eq_ignore_ascii_case(name))
}

#[cfg(any(windows, test))]
fn process_id(value: &str) -> Option<u32> {
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => value.parse().ok(),
    }
}

#[cfg(any(windows, test))]
fn parse(text: &str, pid: u32) -> Option<Fault> {
    // /l:en-US makes the rendered Application Error fields independent of OS language.
    for event in text.split("Event[") {
        let field = |name: &str| -> Option<&str> {
            event
                .lines()
                .find_map(|line| line.trim().strip_prefix(name))
                .map(str::trim)
        };
        let Some(process) = field("Faulting process id:") else {
            continue;
        };
        if process_id(process) != Some(pid) {
            continue;
        }
        let app = field("Faulting application name:")?
            .split(',')
            .next()?
            .trim();
        if !java_exe(app) {
            continue;
        }
        let module = field("Faulting module name:")?;
        let (module, rest) = module.split_once(", version:")?;
        let version = rest.split(',').next()?.trim();
        // Keep file names only: Windows event paths contain user names.
        let module = module.rsplit(['\\', '/']).next()?.trim();
        return Some(Fault {
            module: module.into(),
            version: version.into(),
            exception: field("Exception code:")?.into(),
            offset: field("Fault offset:")?.into(),
        });
    }
    None
}

// Only attribute a DLL to a mod when exactly one enabled jar actually bundles it.
#[cfg(any(windows, test))]
fn bundled_owner(mods: &std::path::Path, module: &str) -> Option<String> {
    let mut owners = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    for entry in std::fs::read_dir(mods).ok()?.flatten() {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jar") {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let Ok(zip) = zip::ZipArchive::new(file) else {
            continue;
        };
        if zip.file_names().any(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|n| n.eq_ignore_ascii_case(module))
        }) {
            let name = crate::crash_conflicts::read_jar(&path)
                .map(|m| m.id)
                .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            owners.push(name.replace(['\n', '\r', ';'], "_"));
        }
    }
    (owners.len() == 1).then(|| owners.remove(0))
}

#[cfg(windows)]
fn query_event(pid: u32, begin: SystemTime, end: SystemTime, deadline: Instant) -> Option<Fault> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    let begin: chrono::DateTime<chrono::Utc> = begin.into();
    let end: chrono::DateTime<chrono::Utc> = end.into();
    // Older Windows event schemas have unnamed EventData fields. Match PID and
    // executable in the rendered parser rather than relying on that schema.
    let filter = format!(
        "/q:*[System[Provider[@Name='Application Error'] and EventID=1000 and TimeCreated[@SystemTime>='{}' and @SystemTime<='{}']]]",
        begin.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    );
    let mut child = Command::new("wevtutil.exe")
        .args([
            "qe",
            "Application",
            &filter,
            "/c:5",
            "/rd:true",
            "/f:text",
            "/l:en-US",
            "/uni:false",
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Drain concurrently so a full pipe cannot stall the subprocess. Bound retained output.
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        let _ = stdout.take(64 * 1024).read_to_end(&mut bytes);
        bytes
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    while !reader.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !reader.is_finished() {
        return None;
    }
    let bytes = reader.join().ok()?;
    parse(&String::from_utf8_lossy(&bytes), pid)
}

#[cfg(windows)]
pub(crate) fn collect(
    pid: u32,
    exe: &std::path::Path,
    start: SystemTime,
    exit: SystemTime,
    mods: &std::path::Path,
) -> String {
    let now = Instant::now();
    let budget = Duration::from_secs(10);
    let deadline = now + budget;
    let begin = start.max(exit - Duration::from_secs(15));
    // Absolute offsets keep retries spaced without accumulating sleeps after a slow query.
    let mut fault = None;
    for ms in [0, 500, 1500, 3000, 6000, 8000] {
        let at = now + Duration::from_millis(ms);
        if ms > 0 && at < Instant::now() {
            continue;
        }
        std::thread::sleep(at.saturating_duration_since(Instant::now()));
        if Instant::now() >= deadline {
            break;
        }
        let attempt = deadline.min(Instant::now() + Duration::from_secs(3));
        if let Some(found) = query_event(pid, begin, SystemTime::now(), attempt) {
            fault = Some((found, ""));
            break;
        }
    }
    let fault = fault.or_else(|| {
        wer::collect(exe.to_str()?, pid, start, exit).map(|found| (found, "; source=WER"))
    });
    let Some((fault, source)) = fault else {
        return "[Arctic diagnostics] native fault: no matching Application Error 1000 or WER report captured; cause unknown".into();
    };
    let owner = bundled_owner(mods, &fault.module)
        .map(|name| format!("; bundled mod={name}"))
        .unwrap_or_default();
    format!(
        "[Arctic diagnostics] native fault: module={}; version={}; exception={}; offset={}{source}{owner}",
        fault.module, fault.version, fault.exception, fault.offset
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_dll_owner_requires_a_unique_enabled_jar() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let jar = |name: &str| {
            let mut zip =
                zip::ZipWriter::new(std::fs::File::create(dir.path().join(name)).unwrap());
            zip.start_file(
                "native/windows/mod-native.dll",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"dll").unwrap();
            zip.finish().unwrap();
        };
        jar("voice.jar");
        assert_eq!(
            bundled_owner(dir.path(), "mod-native.dll").as_deref(),
            Some("voice.jar")
        );
        assert!(bundled_owner(dir.path(), "other.dll").is_none());
        jar("other.jar");
        assert!(bundled_owner(dir.path(), "mod-native.dll").is_none());
    }
    #[test]
    fn application_error_1000() {
        let text = "Event[0]:\n  Event ID: 1000\n  Description:\nFaulting application name: Arctic Client.exe, version: 21.0.1.0, time stamp: 0x123\nFaulting module name: C:\\Users\\Alice\\nvoglv64.dll, version: 32.0.15.8266, time stamp: 0x456\nException code: 0xc0000409\nFault offset: 0x0000000000123456\nFaulting process id: 0x1234\nFaulting application path: C:\\Users\\Alice\\Arctic Client.exe";
        let f = parse(text, 0x1234).unwrap();
        assert_eq!(
            f,
            Fault {
                module: "nvoglv64.dll".into(),
                version: "32.0.15.8266".into(),
                exception: "0xc0000409".into(),
                offset: "0x0000000000123456".into()
            }
        );
        assert!(parse(text, 99).is_none());
        let other = text.replace("0x1234", "0x9999");
        assert_eq!(parse(&format!("{other}\n{text}"), 0x1234), Some(f));
        assert!(parse(&text.replace("Arctic Client.exe", "other.exe"), 0x1234).is_none());
        assert!(parse("unavailable", 0x1234).is_none());
    }
}
