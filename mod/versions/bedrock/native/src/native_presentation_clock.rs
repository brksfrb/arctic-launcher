//! Public QPC sampling in the inspected game's presentation clock domain.
//! Reads its initialized epoch; never calls or initializes a private routine.
use crate::world_hooks::{PresentationClockBinding, read};
use serde_json::{Value, json};
use std::sync::OnceLock;

static BINDING: OnceLock<Option<PresentationClockBinding>> = OnceLock::new();
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceCounter(counter: *mut i64) -> i32;
    fn QueryPerformanceFrequency(frequency: *mut i64) -> i32;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Domain {
    epoch_ns: i64,
    frequency: i64,
    guard: i32,
}
impl Domain {
    pub(crate) fn value(self) -> Value {
        json!({"schema":1,"source":"qpc_game_epoch","profile":crate::native_scene_layout::PRODUCT_VERSION,
            "epochNanoseconds":self.epoch_ns,"frequency":self.frequency})
    }
}
pub(crate) struct Sample {
    domain: Domain,
    nanoseconds: i64,
}
impl Sample {
    pub(crate) fn domain(&self) -> Domain {
        self.domain
    }
    pub(crate) fn seconds(&self) -> f64 {
        self.nanoseconds as f64 / 1_000_000_000.
    }
    pub(crate) fn value(&self) -> Value {
        json!({"schema":1,"domain":self.domain.value(),"nanoseconds":self.nanoseconds,
            "seconds":self.seconds()})
    }
}

fn storage(binding: PresentationClockBinding, base: usize) -> Option<(i32, i64)> {
    let mut guard = [0u8; 4];
    let mut epoch = [0u8; 8];
    read(base.checked_add(binding.guard_rva)?, &mut guard).ok()?;
    read(base.checked_add(binding.epoch_rva)?, &mut epoch).ok()?;
    let state = i32::from_le_bytes(guard);
    let epoch = i64::from_le_bytes(epoch);
    // MSVC's local-static states 0/-1 mean uninitialized/initializing. Do not
    // cause initialization from a packet callback or adopt a half-written epoch.
    (state != 0 && state != -1 && epoch >= 0).then_some((state, epoch))
}
pub(crate) fn domain() -> Option<Domain> {
    let base = crate::world_hooks::image_base()?;
    let binding = (*BINDING.get_or_init(|| crate::world_hooks::presentation_clock_binding().ok()))?;
    let before = storage(binding, base)?;
    let mut frequency = 0i64;
    // SAFETY: a public Win32 API writes one owned, valid i64 output.
    if unsafe { QueryPerformanceFrequency(&mut frequency) } == 0 || frequency <= 0 {
        return None;
    }
    if storage(binding, base)? != before {
        return None;
    }
    Some(Domain {
        epoch_ns: before.1,
        frequency,
        guard: before.0,
    })
}

fn counter_ns(counter: i64, frequency: i64) -> Option<i64> {
    if counter < 0 || frequency <= 0 {
        return None;
    }
    if frequency == 10_000_000 {
        return counter.checked_mul(100);
    }
    // Same positive-integer quotient/remainder conversion as the inspected
    // 24 MHz and generic branches, without invoking the game's clock helper.
    let whole = (counter / frequency).checked_mul(1_000_000_000)?;
    let fraction = (counter % frequency).checked_mul(1_000_000_000)? / frequency;
    whole.checked_add(fraction)
}
pub(crate) fn sample() -> Option<Sample> {
    let before = domain()?;
    let mut counter = 0i64;
    // SAFETY: a public Win32 API writes one owned, valid i64 output.
    if unsafe { QueryPerformanceCounter(&mut counter) } == 0 {
        return None;
    }
    let nanoseconds = counter_ns(counter, before.frequency)?.checked_sub(before.epoch_ns)?;
    if !(0..=1_000_000_000_000_000_000).contains(&nanoseconds) || domain()? != before {
        return None;
    }
    Some(Sample {
        domain: before,
        nanoseconds,
    })
}
