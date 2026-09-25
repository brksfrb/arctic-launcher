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
pub fn recommended_memory_mb(total_mb: Option<u64>) -> u32 {
    let Some(total) = total_mb else {
        return 4 * 1024;
    };
    let quarter = u32::try_from(total / 4).unwrap_or(u32::MAX);
    let clamped = quarter.clamp(FLOOR_RECOMMENDED_MB, MAX_RECOMMENDED_MB);
    (clamped / 512 * 512).max(MIN_MEMORY_MB)
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    pub fn total_memory_bytes() -> Option<u64> {
        // SAFETY: MEMORYSTATUSEX is plain data; dwLength must be set before the call.
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: `status` is a valid, initialized MEMORYSTATUSEX.
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        (ok != 0).then_some(status.ullTotalPhys)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn total_memory_bytes() -> Option<u64> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        super::parse_meminfo(&text)
    }
}

/// `MemTotal:  16318376 kB` → bytes.
#[cfg_attr(windows, allow(dead_code))]
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
    fn total_memory_is_readable_here() {
        assert!(total_memory_mb().is_some_and(|mb| mb > 256));
    }
}
