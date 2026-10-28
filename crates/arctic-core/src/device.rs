//! A hint of which PC this is, so an offline name whose key was lost (Windows reinstalled) can be given back to
//! the PC it was used on.
//!
//! Only a hash of the PC's hardware identity ever leaves it: the firmware's system UUID and the board and
//! processor serials on Windows, the platform UUID on a Mac, the firmware UUID on Linux. The server stores a
//! hash of that hash and only compares it. It says "this is one of the PCs this name was used on", never what
//! the PC is. A PC with new hardware gives a different hint.

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
pub fn hint() -> Option<String> {
    HINT.get_or_init(|| raw().map(|id| digest(&id))).clone()
}

fn digest(raw: &str) -> String {
    hex::encode(Sha256::digest(format!("arctic-device-v1|{raw}").as_bytes()))
}

/// Values firmware makers leave in when they filled nothing in.
fn placeholder(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    v.is_empty()
        || v.len() < 4
        || v.contains("to be filled")
        || v.contains("default string")
        || v.contains("not specified")
        || v.contains("not applicable")
        || v == "none"
        || v == "unknown"
        || v.chars().all(|c| c == '0' || c == 'f' || c == '-')
        || v == "03000200-0400-0500-0006-000700080009"
}

/// The usable parts, joined; None when there are none.
fn join(parts: &[String]) -> Option<String> {
    let good: Vec<&str> = parts
        .iter()
        .map(String::as_str)
        .filter(|p| !placeholder(p))
        .collect();
    (!good.is_empty()).then(|| good.join("|"))
}

#[cfg(windows)]
fn raw() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "(Get-CimInstance Win32_ComputerSystemProduct).UUID; \
                  (Get-CimInstance Win32_BaseBoard).SerialNumber; \
                  (Get-CimInstance Win32_Processor | Select-Object -First 1).ProcessorId";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let parts: Vec<String> = text.lines().map(|l| l.trim().to_owned()).collect();
    join(&parts)
}

#[cfg(target_os = "macos")]
fn raw() -> Option<String> {
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text.lines().find(|l| l.contains("IOPlatformUUID"))?;
    let value = line.split('"').nth(3)?.to_owned();
    join(&[value])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn raw() -> Option<String> {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default().trim().to_owned();
    // The firmware's own ids survive a reinstall (they may only be readable by root); the machine id does not.
    let firmware = join(&[
        read("/sys/class/dmi/id/product_uuid"),
        read("/sys/class/dmi/id/board_serial"),
    ]);
    firmware.or_else(|| join(&[read("/etc/machine-id")]))
}

#[cfg(not(any(windows, unix)))]
fn raw() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_placeholder_ids_tell_nothing() {
        assert_eq!(join(&[String::new(), "To be filled by O.E.M.".into()]), None);
        assert_eq!(join(&["FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF".into()]), None);
        assert_eq!(join(&["03000200-0400-0500-0006-000700080009".into()]), None);
    }

    #[test]
    fn real_ids_join_and_hash_the_same_each_time() {
        let id = join(&["AB12CD34-1111-2222-3333-444455556666".into(), "None".into(), "SN123456".into()]).unwrap();
        assert_eq!(id, "AB12CD34-1111-2222-3333-444455556666|SN123456");
        assert_eq!(digest(&id), digest(&id));
        assert_eq!(digest(&id).len(), 64);
        assert_ne!(digest(&id), digest("other"));
    }
}
