//! Per-IP request limit: a fixed window counter, pruned as it goes.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Limiter {
    window: Duration,
    max: u32,
    counts: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl Limiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            window,
            max,
            counts: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, ip: IpAddr) -> bool {
        let mut counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if counts.len() > 50_000 {
            counts.retain(|_, (start, _)| now.duration_since(*start) < self.window);
        }
        let entry = counts.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit() {
        let l = Limiter::new(2, Duration::from_secs(60));
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert!(l.allow(ip));
        assert!(l.allow(ip));
        assert!(!l.allow(ip));
        assert!(l.allow("5.6.7.8".parse().unwrap()));
    }
}
