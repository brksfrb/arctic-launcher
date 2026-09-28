//! Proximity voice chat for Arctic: the microphone is captured in 20 ms
//! frames, compressed with Opus and sent to nearby Arctic players; their
//! voices come back through a jitter buffer and are mixed in stereo by
//! where they stand relative to you (louder when close, from the left or
//! right as they are). The network and the game's positions are supplied
//! by the caller; this crate only does audio.

pub mod codec;
pub mod engine;
pub mod jitter;
pub mod packet;
pub mod spatial;
pub mod svc;
pub mod vad;

/// Opus runs at 48 kHz; voice is mono.
pub const SAMPLE_RATE: u32 = 48_000;
/// 20 ms frames.
pub const FRAME: usize = 960;
