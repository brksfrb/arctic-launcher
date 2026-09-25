//! Small animation, timing and formatting helpers shared by the UI.

use std::collections::VecDeque;

use eframe::egui::emath::easing;

/// Seconds a tab takes to fade/slide in.
pub const TAB_TRANSITION: f32 = 0.16;
/// Window used to smooth the download speed.
const RATE_WINDOW_SECS: f64 = 2.5;

/// Eased progress in `[0, 1]` of an animation that started at `start`.
pub fn eased(now: f64, start: f64, duration: f32) -> f32 {
    easing::cubic_out((((now - start) as f32) / duration).clamp(0.0, 1.0))
}

/// Throughput estimate from recent (time, bytes) samples.
#[derive(Debug, Default)]
pub struct RateMeter {
    samples: VecDeque<(f64, u64)>,
}

impl RateMeter {
    pub fn push(&mut self, now: f64, bytes: u64) {
        // A retry can move the byte count backwards; restart the window.
        if self.samples.back().is_some_and(|&(_, b)| bytes < b) {
            self.samples.clear();
        }
        self.samples.push_back((now, bytes));
        while self
            .samples
            .front()
            .is_some_and(|&(t, _)| now - t > RATE_WINDOW_SECS)
        {
            self.samples.pop_front();
        }
    }

    /// Bytes per second, if there is enough data.
    pub fn bytes_per_sec(&self) -> Option<f64> {
        let (&(t0, b0), &(t1, b1)) = (self.samples.front()?, self.samples.back()?);
        let dt = t1 - t0;
        (dt > 0.25).then(|| (b1 - b0) as f64 / dt)
    }

    /// Seconds left for `remaining` bytes at the current rate.
    pub fn eta_secs(&self, remaining: u64) -> Option<f64> {
        let rate = self.bytes_per_sec().filter(|r| *r > 1.0)?;
        Some(remaining as f64 / rate)
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= 1024.0 * MB {
        format!("{:.2} GB", b / (1024.0 * MB))
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}

pub fn format_duration(secs: f64) -> String {
    let s = secs.round().max(0.0) as u64;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, (s % 3600) / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_meter_measures_window() {
        let mut m = RateMeter::default();
        assert!(m.bytes_per_sec().is_none());
        m.push(0.0, 0);
        m.push(1.0, 1000);
        assert_eq!(m.bytes_per_sec(), Some(1000.0));
        assert_eq!(m.eta_secs(5000), Some(5.0));
        m.push(10.0, 2000); // old samples fall out of the window
        assert!(m.bytes_per_sec().is_none());
    }

    #[test]
    fn rate_meter_resets_on_rewind() {
        let mut m = RateMeter::default();
        m.push(0.0, 500);
        m.push(1.0, 100);
        assert!(m.bytes_per_sec().is_none());
    }

    #[test]
    fn formatting() {
        assert_eq!(format_bytes(512 * 1024), "512 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_duration(42.4), "42s");
        assert_eq!(format_duration(125.0), "2m 05s");
        assert_eq!(format_duration(3700.0), "1h 01m");
    }

    #[test]
    fn easing_is_clamped() {
        assert_eq!(eased(0.0, 0.0, 1.0), 0.0);
        assert_eq!(eased(5.0, 0.0, 1.0), 1.0);
    }
}
