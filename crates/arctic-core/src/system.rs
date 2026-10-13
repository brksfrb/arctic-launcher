//! Facts about the machine the launcher runs on.

use crate::settings::MIN_MEMORY_MB;

/// Upper bound for the recommended game memory. More rarely helps and
/// makes garbage collection pauses longer.
const MAX_RECOMMENDED_MB: u32 = 8 * 1024;
/// Memory kept for the OS and other programs when recommending.
const FLOOR_RECOMMENDED_MB: u32 = 2 * 1024;

/// Memory the game is given at most, whatever the setting: what's installed minus room for the
/// system. A setting above what the computer has makes Java fail before the game opens
/// ("Native memory allocation failed").
/// Big modpacks need more than a quarter of the RAM to load a world (Cobbleverse, 300 mods, runs
/// out at 4 GB): (mods in the folder, at least this much memory). Only ever raises the setting.
const PACK_MEMORY: [(usize, u32); 3] = [(250, 8192), (150, 6144), (80, 5120)];

/// The memory for a game with `mods` mods when the player hasn't set one for its instance: the
/// global setting, or more for a big pack (then capped by what the computer can spare).
pub fn memory_for_mods(setting: u32, mods: usize) -> u32 {
    PACK_MEMORY
        .iter()
        .find(|(at_least, _)| mods >= *at_least)
        .map_or(setting, |(_, mb)| setting.max(*mb))
}

pub fn playable_memory_mb(asked: u32) -> u32 {
    match total_memory_mb() {
        Some(total) => asked.min(room_mb(total, commit_limit_mb())),
        None => asked,
    }
}

/// The most memory a game can be given on a computer with `total` MB installed and `commit` MB
/// that Windows (or Linux, with swap) can hand out in all: the installed memory less 2 GB for
/// the system, or, when the page file can back it, up to all of the installed memory. Small
/// computers then start big packs as other launchers do (slower, from the page file), and a
/// setting above the installed memory is still brought down.
fn room_mb(total: u64, commit: Option<u64>) -> u32 {
    let keep = u64::from(FLOOR_RECOMMENDED_MB);
    let by_memory = total.saturating_sub(keep);
    let by_commit = commit.map_or(0, |c| c.saturating_sub(keep).min(total));
    (by_memory.max(by_commit).min(u64::from(u32::MAX)) as u32).max(MIN_MEMORY_MB)
}

/// Memory plus page file (swap) in MiB, if it can be read.
fn commit_limit_mb() -> Option<u64> {
    imp::commit_limit_bytes().map(|b| b / (1024 * 1024))
}

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

    /// The commit limit: installed memory plus the page file.
    pub fn commit_limit_bytes() -> Option<u64> {
        // SAFETY: as above.
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: `status` is a valid, initialized MEMORYSTATUSEX.
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        (ok != 0).then_some(status.ullTotalPageFile)
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

    /// macOS grows its swap as needed: no fixed limit to go by.
    pub fn commit_limit_bytes() -> Option<u64> {
        None
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

    /// Memory plus swap.
    pub fn commit_limit_bytes() -> Option<u64> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let swap = super::meminfo_field(&text, "SwapTotal:")?;
        Some(super::parse_meminfo(&text)? + swap)
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
    meminfo_field(text, "MemTotal:")
}

/// A `/proc/meminfo` field (in kB there) → bytes.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn meminfo_field(text: &str, name: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with(name))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_computer_with_a_page_file_can_give_all_its_memory() {
        // 4 GB installed, 2 GB kept for the system without a page file to lean on.
        assert_eq!(room_mb(3966, None), 1918);
        assert_eq!(room_mb(3966, Some(3966)), 1918);
        // With a page file: up to all of the installed memory, not beyond it.
        assert_eq!(room_mb(3966, Some(10_000)), 3966);
        // A roomy computer is unchanged.
        assert_eq!(room_mb(16_384, Some(30_000)), 16_384);
        assert_eq!(room_mb(16_384, None), 14_336);
    }

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
    fn big_packs_get_more_memory_and_nothing_gets_less() {
        assert_eq!(memory_for_mods(4096, 12), 4096);
        assert_eq!(memory_for_mods(4096, 90), 5120);
        assert_eq!(memory_for_mods(4096, 160), 6144);
        assert_eq!(memory_for_mods(4096, 300), 8192);
        // A bigger setting stays.
        assert_eq!(memory_for_mods(12288, 300), 12288);
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
