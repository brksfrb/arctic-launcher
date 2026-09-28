//! Evens out network timing: packets are held briefly and played in
//! order, one per 20 ms tick. A missing packet is played as "lost" (the
//! decoder conceals it); a long silence resets the buffer.

use std::collections::BTreeMap;

/// Frames held before playing starts (60 ms).
const TARGET: usize = 3;
/// More than this waiting: skip ahead (the speaker's clock runs fast, or
/// packets bunched up).
const MAX_HELD: usize = 12;
/// This many ticks with nothing: the speaker stopped.
const IDLE_TICKS: u32 = 10;

#[derive(Default)]
pub struct Jitter {
    held: BTreeMap<u16, Vec<u8>>,
    /// Sequence number to play next, once started.
    next: Option<u16>,
    idle: u32,
}

/// What to play this tick.
#[derive(Debug, PartialEq, Eq)]
pub enum Play {
    /// Nothing (not speaking, or still filling up).
    Silence,
    Frame(Vec<u8>),
    /// The packet for this tick didn't come: conceal it.
    Lost,
}

impl Jitter {
    pub fn push(&mut self, seq: u16, opus: Vec<u8>) {
        if let Some(next) = self.next {
            // Too late to play.
            if behind(seq, next) {
                return;
            }
        }
        self.held.insert(seq, opus);
        while self.held.len() > MAX_HELD {
            let first = *self.held.keys().next().unwrap_or(&seq);
            self.held.remove(&first);
            self.next = None;
        }
    }

    /// One 20 ms tick.
    pub fn pop(&mut self) -> Play {
        let Some(next) = self.next else {
            if self.held.len() < TARGET {
                return Play::Silence;
            }
            let first = self.first();
            self.next = Some(first);
            return self.pop();
        };
        match self.held.remove(&next) {
            Some(frame) => {
                self.idle = 0;
                self.next = Some(next.wrapping_add(1));
                Play::Frame(frame)
            }
            None if self.held.is_empty() => {
                self.idle += 1;
                if self.idle >= IDLE_TICKS {
                    self.next = None;
                    self.idle = 0;
                    return Play::Silence;
                }
                self.next = Some(next.wrapping_add(1));
                Play::Lost
            }
            None => {
                self.next = Some(next.wrapping_add(1));
                Play::Lost
            }
        }
    }

    /// The oldest held sequence number (wrap-aware).
    fn first(&self) -> u16 {
        let keys: Vec<u16> = self.held.keys().copied().collect();
        // Around the wrap, 65535 comes before 0.
        let wraps =
            keys.first().is_some_and(|f| *f < 1024) && keys.last().is_some_and(|l| *l > 64_512);
        if wraps {
            *keys.iter().find(|k| **k > 32_768).unwrap_or(&keys[0])
        } else {
            keys[0]
        }
    }
}

/// `seq` comes before `next` (within half the sequence space).
fn behind(seq: u16, next: u16) -> bool {
    let d = next.wrapping_sub(seq);
    d != 0 && d < 32_768
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(n: u8) -> Vec<u8> {
        vec![n]
    }

    #[test]
    fn waits_then_plays_in_order_and_conceals_gaps() {
        let mut j = Jitter::default();
        j.push(11, frame(11));
        j.push(10, frame(10));
        assert_eq!(j.pop(), Play::Silence);
        j.push(13, frame(13));
        assert_eq!(j.pop(), Play::Frame(frame(10)));
        assert_eq!(j.pop(), Play::Frame(frame(11)));
        assert_eq!(j.pop(), Play::Lost);
        assert_eq!(j.pop(), Play::Frame(frame(13)));
        // Late packets are dropped.
        j.push(12, frame(12));
        j.push(14, frame(14));
        assert_eq!(j.pop(), Play::Frame(frame(14)));
        // Silence for a while resets.
        for _ in 0..IDLE_TICKS {
            j.pop();
        }
        assert_eq!(j.pop(), Play::Silence);
    }

    #[test]
    fn handles_the_wrap() {
        let mut j = Jitter::default();
        for seq in [65_534u16, 65_535, 0] {
            j.push(seq, vec![seq as u8]);
        }
        assert_eq!(j.pop(), Play::Frame(vec![65_534u16 as u8]));
        assert_eq!(j.pop(), Play::Frame(vec![65_535u16 as u8]));
        assert_eq!(j.pop(), Play::Frame(vec![0]));
        assert!(behind(65_535, 0));
        assert!(!behind(1, 0));
    }
}
