//! A hint of which PC this is, so an offline name whose key was lost (Windows reinstalled) can be given back to
//! the PC it was used on.
//!
//! Only a salted hash of the PC's firmware identity ever leaves it: the system UUID from the firmware
//! (SMBIOS), or, when that is a placeholder, the mainboard serial. Never the processor id (every CPU of one
//! model shares it, so it would let strangers claim each other's names) and never the OS's machine id (it
//! doesn't survive a reinstall and isn't hardware). When neither firmware value is usable there is no hint
//! and nothing is sent. The server stores a hash of the hash and only compares it. Neither the raw ids nor
//! the hash are ever logged.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

static HINT: OnceLock<Option<String>> = OnceLock::new();

/// Work the hint out in the background now (asking Windows takes a moment), so sign-in doesn't wait for it.
pub fn warm() {
    let _ = std::thread::Builder::new()
        .name("device-hint".into())
        .spawn(|| {
            let _ = hint();
        });
}

/// The hint (64 hex characters), or None when this PC tells nothing usable.
///
/// If `warm()` is still asking the system, this waits for that (the `OnceLock` runs the work once and
/// blocks other callers until it is done). That wait is bounded: the lookup never takes longer than
/// `LOOKUP_TIMEOUT`, and it never calls back into `hint()`, so it can't deadlock.
pub fn hint() -> Option<String> {
    HINT.get_or_init(|| {
        let hint = raw().map(|id| digest(&id));
        if hint.is_none() {
            log::debug!("device hint unavailable");
        }
        hint
    })
    .clone()
}

/// The longest the system is given to answer.
#[cfg(windows)]
const LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

fn digest(raw: &str) -> String {
    hex::encode(Sha256::digest(format!("arctic-device-v2|{raw}").as_bytes()))
}

/// Values firmware makers leave in when they filled nothing in (compared lowercase).
const PLACEHOLDERS: &[&str] = &[
    "none",
    "null",
    "unknown",
    "n/a",
    "na",
    "not available",
    "not applicable",
    "not specified",
    "not set",
    "to be filled by o.e.m.",
    "to be filled by oem",
    "o.e.m.",
    "oem",
    "default string",
    "system serial number",
    "system product name",
    "system manufacturer",
    "base board serial number",
    "baseboard serial number",
    "board serial number",
    "serial number",
    "chassis serial number",
    "default",
    "123456789",
    "1234567890",
    "0123456789",
    "123456789012345678",
    "03000200-0400-0500-0006-000700080009",
    "00020003-0004-0005-0006-000700080009",
    "12345678-1234-1234-1234-123456789012",
    "12345678-1234-5678-1234-567812345678",
    "11111111-1111-1111-1111-111111111111",
];

/// True when a firmware value says nothing about this PC.
fn placeholder(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if v.len() < 6 || PLACEHOLDERS.contains(&v.as_str()) {
        return true;
    }
    if v.contains("to be filled") || v.contains("default string") || v.contains("o.e.m") {
        return true;
    }
    // "0000...", "FFFF...", "xxxx..." and the like: one character repeated (dashes aside).
    let mut chars = v.chars().filter(|c| *c != '-');
    match chars.next() {
        Some(first) => chars.all(|c| c == first),
        None => true,
    }
}

/// The system UUID if usable, else the board serial, else None. Tagged, so a serial can never be taken for a
/// UUID and the hash stays the same each time.
fn pick(uuid: &str, board_serial: &str) -> Option<String> {
    let (uuid, serial) = (uuid.trim(), board_serial.trim());
    if !placeholder(uuid) {
        Some(format!("uuid:{}", uuid.to_ascii_lowercase()))
    } else if !placeholder(serial) {
        Some(format!("board:{}", serial.to_ascii_lowercase()))
    } else {
        None
    }
}

#[cfg(windows)]
fn raw() -> Option<String> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "$ErrorActionPreference='Stop'; \
                  'UUID=' + (Get-CimInstance Win32_ComputerSystemProduct).UUID; \
                  'BOARD=' + (Get-CimInstance Win32_BaseBoard).SerialNumber";
    // The system's own PowerShell, not whichever one a search path finds first.
    let system = std::env::var_os("SystemRoot").map(|root| {
        std::path::PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
    });
    let program = system
        .filter(|p| p.is_file())
        .unwrap_or_else(|| "powershell".into());
    let mut child = Command::new(program)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on its own thread, so a hung PowerShell can be killed without blocking on the pipe.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        // Capped: the answer is two short lines.
        let _ = (&mut stdout).take(64 * 1024).read_to_end(&mut bytes);
        bytes
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if start.elapsed() >= LOOKUP_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    let text = String::from_utf8_lossy(&reader.join().ok()?).into_owned();
    parse_windows(&text)
}

/// Reads the `UUID=` and `BOARD=` lines of the PowerShell answer; garbage in any other line is ignored.
#[cfg(any(windows, test))]
fn parse_windows(text: &str) -> Option<String> {
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(name))
            .unwrap_or("")
    };
    pick(field("UUID="), field("BOARD="))
}

#[cfg(target_os = "macos")]
fn raw() -> Option<String> {
    let out = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text.lines().find(|l| l.contains("IOPlatformUUID"))?;
    pick(line.split('"').nth(3)?, "")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn raw() -> Option<String> {
    // The firmware's own ids survive a reinstall (the UUID is often readable by root only). There is
    // deliberately no fallback to /etc/machine-id: it is made at install time and isn't hardware.
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    pick(
        &read("/sys/class/dmi/id/product_uuid"),
        &read("/sys/class/dmi/id/board_serial"),
    )
}

#[cfg(not(any(windows, unix)))]
fn raw() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_ids_tell_nothing() {
        for p in [
            "",
            "   ",
            "To be filled by O.E.M.",
            "Default string",
            "System Serial Number",
            "123456789",
            "Not Available",
            "None",
            "O.E.M.",
            "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF",
            "00000000-0000-0000-0000-000000000000",
            "03000200-0400-0500-0006-000700080009",
            "xxxxxxxx",
            "abc",
        ] {
            assert!(placeholder(p), "{p:?} should be a placeholder");
        }
        assert!(!placeholder("AB12CD34-1111-2222-3333-444455556666"));
        assert!(!placeholder("PF2XK9Q7"));
    }

    #[test]
    fn the_uuid_wins_then_the_board_serial_then_nothing() {
        let uuid = "AB12CD34-1111-2222-3333-444455556666";
        assert_eq!(
            pick(uuid, "SN123456").as_deref(),
            Some("uuid:ab12cd34-1111-2222-3333-444455556666")
        );
        assert_eq!(
            pick("FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF", " PF2XK9Q7 ").as_deref(),
            Some("board:pf2xk9q7")
        );
        assert_eq!(pick("", "Default string"), None);
        assert_eq!(pick("00000000-0000-0000-0000-000000000000", ""), None);
    }

    #[test]
    fn a_serial_is_never_taken_for_the_uuid() {
        assert_ne!(pick("PF2XK9Q7", ""), pick("", "PF2XK9Q7"));
    }

    #[test]
    fn windows_answers_parse_and_garbage_gives_none() {
        let good = "UUID=AB12CD34-1111-2222-3333-444455556666\r\nBOARD=SN123456\r\n";
        assert_eq!(
            parse_windows(good).as_deref(),
            Some("uuid:ab12cd34-1111-2222-3333-444455556666")
        );
        assert_eq!(
            parse_windows("UUID=To be filled by O.E.M.\nBOARD=PF2XK9Q7\n").as_deref(),
            Some("board:pf2xk9q7")
        );
        assert_eq!(parse_windows(""), None);
        assert_eq!(parse_windows("\u{fffd}\u{fffd} not what we asked"), None);
        assert_eq!(parse_windows("UUID=\nBOARD=\n"), None);
    }

    #[test]
    fn the_hash_is_stable_salted_and_64_hex() {
        let id = pick("AB12CD34-1111-2222-3333-444455556666", "").unwrap();
        assert_eq!(digest(&id), digest(&id));
        assert_eq!(digest(&id).len(), 64);
        assert!(
            digest(&id)
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        );
        assert_ne!(digest(&id), digest("other"));
        assert_ne!(digest(&id), hex::encode(Sha256::digest(id.as_bytes())));
    }

    #[test]
    fn hint_never_hangs_or_panics_here() {
        warm();
        let first = hint();
        assert_eq!(first, hint());
        if let Some(h) = first {
            assert_eq!(h.len(), 64);
        }
    }
}
