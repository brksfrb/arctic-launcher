//! Java VideoExport's inclusive replay range, independent of game/encoder IO.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRange {
    pub from_ms: u32,
    pub to_ms: u32,
}
impl ExportRange {
    /// VideoExport keeps at least one millisecond, even at a paused playhead.
    pub fn new(from_ms: u32, to_ms: u32) -> Result<Self> {
        let minimum = from_ms
            .checked_add(1)
            .ok_or_else(|| Error::Other("export range start overflows".into()))?;
        let range = Self {
            from_ms,
            to_ms: to_ms.max(minimum),
        };
        range.validate()?;
        Ok(range)
    }
    fn validate(self) -> Result<()> {
        if self.from_ms >= self.to_ms
            || self.to_ms > 86_400_000
            || self.to_ms - self.from_ms > 7_200_000
        {
            return Err(Error::Other(
                "export range needs ordered times within the replay and at most two hours".into(),
            ));
        }
        Ok(())
    }
    /// Port the actual Java endpoint predicate, including its multiplication
    /// by a precomputed double frameMillis. Rounding at an integral endpoint
    /// can differ from simply floor(duration*FPS/1000)+1.
    pub fn frames(self, fps: u32) -> Result<u32> {
        self.validate()?;
        if !(1..=60).contains(&fps) {
            return Err(Error::Other("export range needs FPS 1–60".into()));
        }
        let step = 1000.0 / f64::from(fps);
        let included =
            |index: u32| f64::from(self.from_ms) + f64::from(index) * step <= f64::from(self.to_ms);
        let mut last = ((u64::from(self.to_ms - self.from_ms) * u64::from(fps)) / 1000) as u32;
        while last > 0 && !included(last) {
            last -= 1;
        }
        while last < fps * 7200 && included(last + 1) {
            last += 1;
        }
        Ok(last + 1)
    }
    pub fn matches(self, fps: u32, frames: u32, origin_ms: u32) -> Result<bool> {
        Ok(self.from_ms == origin_ms && self.frames(fps)? == frames)
    }
}
