//! Facts about the machine the launcher runs on.

use crate::settings::MIN_MEMORY_MB;

/// Upper bound for the recommended game memory. More rarely helps and
/// makes garbage collection pauses longer.
const MAX_RECOMMENDED_MB: u32 = 8 * 1024;
/// Memory kept for the OS and other programs when recommending.
const FLOOR_RECOMMENDED_MB: u32 = 2 * 1024;

/// Installed physical memory in MiB, if it can be read.
pub fn total_memory_mb() -> Option<u64> {
    imp::total_memory_bytes().map(|b| b / (1024 * 1024))
}

/// A sensible `-Xmx` for this machine: a quarter of RAM, between 2 and
/// 8 GiB, rounded down to 512 MiB.
/// This computer's network name, if known (no lookups).
pub fn host_name() -> Option<String> {
    let from_env = ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok());
    from_env
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|n| n.trim().to_owned())
        .filter(|n| {
            !n.is_empty()
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        })
}

pub fn recommended_memory_mb(total_mb: Option<u64>) -> u32 {
    let Some(total) = total_mb else {
        return 4 * 1024;
    };
    let quarter = u32::try_from(total / 4).unwrap_or(u32::MAX);
    let clamped = quarter.clamp(FLOOR_RECOMMENDED_MB, MAX_RECOMMENDED_MB);
    (clamped / 512 * 512).max(MIN_MEMORY_MB)
}

/// "13:05:09" on this computer's clock for a Unix time in milliseconds.
pub fn local_clock(unix_ms: i64) -> Option<String> {
    let secs = imp::local_secs_of_day(unix_ms)?;
    Some(format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    ))
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

    /// Seconds since local midnight, with the time zone Windows has set.
    pub fn local_secs_of_day(unix_ms: i64) -> Option<i64> {
        // FILETIME counts 100 ns ticks from 1601.
        let ticks = u64::try_from(
            unix_ms
                .checked_mul(10_000)?
                .checked_add(116_444_736_000_000_000)?,
        )
        .ok()?;
        let file = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        // SAFETY: SYSTEMTIME is plain data, filled in by the calls below.
        let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
        let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
        // SAFETY: all pointers are to live, initialized values; a null zone means the current one.
        let ok = unsafe {
            FileTimeToSystemTime(&file, &mut utc) != 0
                && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0
        };
        ok.then(|| {
            i64::from(local.wHour) * 3600 + i64::from(local.wMinute) * 60 + i64::from(local.wSecond)
        })
    }

    pub fn total_memory_bytes() -> Option<u64> {
        // SAFETY: MEMORYSTATUSEX is plain data; dwLength must be set before the call.
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: `status` is a valid, initialized MEMORYSTATUSEX.
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        (ok != 0).then_some(status.ullTotalPhys)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    /// Seconds since local midnight. The zone's offset comes from `date`,
    /// asked once (a daylight-saving change mid-session is off by an hour).
    pub fn local_secs_of_day(unix_ms: i64) -> Option<i64> {
        static OFFSET: std::sync::OnceLock<Option<i64>> = std::sync::OnceLock::new();
        let offset = (*OFFSET.get_or_init(|| {
            let out = std::process::Command::new("date")
                .arg("+%z")
                .output()
                .ok()?;
            super::parse_utc_offset(String::from_utf8_lossy(&out.stdout).trim())
        }))?;
        Some((unix_ms / 1000 + offset).rem_euclid(86_400))
    }
    /// `sysctl -n hw.memsize`: the installed memory in bytes.
    pub fn total_memory_bytes() -> Option<u64> {
        let out = std::process::Command::new("/usr/sbin/sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    /// Seconds since local midnight. The zone's offset comes from `date`,
    /// asked once (a daylight-saving change mid-session is off by an hour).
    pub fn local_secs_of_day(unix_ms: i64) -> Option<i64> {
        static OFFSET: std::sync::OnceLock<Option<i64>> = std::sync::OnceLock::new();
        let offset = (*OFFSET.get_or_init(|| {
            let out = std::process::Command::new("date")
                .arg("+%z")
                .output()
                .ok()?;
            super::parse_utc_offset(String::from_utf8_lossy(&out.stdout).trim())
        }))?;
        Some((unix_ms / 1000 + offset).rem_euclid(86_400))
    }
    pub fn total_memory_bytes() -> Option<u64> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        super::parse_meminfo(&text)
    }
}

/// `+0300` / `-0530` → seconds east of UTC.
#[cfg_attr(windows, allow(dead_code))]
fn parse_utc_offset(text: &str) -> Option<i64> {
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let digits = text.get(1..5)?;
    let hours: i64 = digits.get(..2)?.parse().ok()?;
    let minutes: i64 = digits.get(2..)?.parse().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

/// `MemTotal:  16318376 kB` → bytes.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendation_is_a_quarter_within_bounds() {
        assert_eq!(recommended_memory_mb(Some(16 * 1024)), 4096);
        assert_eq!(recommended_memory_mb(Some(4 * 1024)), 2048);
        assert_eq!(recommended_memory_mb(Some(64 * 1024)), 8192);
        assert_eq!(recommended_memory_mb(Some(10_000)), 2048);
        assert_eq!(recommended_memory_mb(Some(13_000)), 3072);
        assert_eq!(recommended_memory_mb(None), 4096);
    }

    #[test]
    fn meminfo_parses() {
        let text = "MemTotal:       16318376 kB\nMemFree: 1 kB\n";
        assert_eq!(parse_meminfo(text), Some(16_318_376 * 1024));
        assert_eq!(parse_meminfo("nothing"), None);
    }

    #[test]
    fn utc_offsets_parse() {
        assert_eq!(parse_utc_offset("+0300"), Some(10_800));
        assert_eq!(parse_utc_offset("-0530"), Some(-19_800));
        assert_eq!(parse_utc_offset("UTC"), None);
    }

    #[test]
    fn local_clock_is_a_time_of_day() {
        let clock = local_clock(1_790_000_000_000).unwrap();
        assert_eq!(clock.len(), 8);
        assert_eq!(clock.as_bytes()[2], b':');
    }

    #[test]
    fn total_memory_is_readable_here() {
        assert!(total_memory_mb().is_some_and(|mb| mb > 256));
    }
}
