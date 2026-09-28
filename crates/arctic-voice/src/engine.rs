//! The running voice chat: the microphone is read in 20 ms frames (voice
//! activation or push-to-talk), encoded and handed to the network; packets
//! from others go through a jitter buffer per speaker, are decoded, placed
//! by position and mixed to the speakers or headphones.
//!
//! Audio devices run at their own rate and channel count; everything is
//! converted to and from 48 kHz here with a simple linear resampler (fine
//! for speech).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// A device's name as the system shows it.
fn name_of(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|n| n.name().to_owned())
}

use crate::codec::{VoiceDecoder, VoiceEncoder};
use crate::jitter::{Jitter, Play};
use crate::packet::Packet;
use crate::spatial::{self, Place};
use crate::vad::{Vad, level_db};
use crate::{FRAME, SAMPLE_RATE};

/// A speaker, identified by the caller (e.g. a network peer).
pub type PeerId = u64;

/// How the microphone opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// While the push-to-talk key is held ([`Engine::set_talking`]).
    PushToTalk,
    /// When the microphone is louder than this (dBFS, like -45).
    Voice { threshold_db: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Device names; `None` = the system default.
    pub input: Option<String>,
    pub output: Option<String>,
    pub mode: Mode,
    /// Microphone and speaker gain (1 = unchanged).
    pub mic_gain: f32,
    pub volume: f32,
    /// Blocks beyond which voices are silent.
    pub range: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            mode: Mode::Voice {
                threshold_db: -45.0,
            },
            mic_gain: 1.0,
            volume: 1.0,
            range: spatial::DEFAULT_RANGE,
        }
    }
}

/// Sends an encoded packet to everyone in range.
pub type Outgoing = Box<dyn Fn(Vec<u8>) + Send>;

struct Speaker {
    jitter: Jitter,
    decoder: VoiceDecoder,
    place: Option<Place>,
    /// Set by the player (0 = muted).
    volume: f32,
    /// Last tick with sound, for "who's speaking".
    heard: Option<Instant>,
}

struct Shared {
    config: Mutex<Config>,
    talking: AtomicBool,
    /// Mic level for the settings meter (dBFS × 100, as bits).
    mic_level: AtomicU32,
    sending: AtomicBool,
    listener: Mutex<Option<Place>>,
    speakers: Mutex<HashMap<PeerId, Speaker>>,
    captured: Mutex<VecDeque<f32>>,
    playback: Mutex<VecDeque<f32>>,
    stop: AtomicBool,
}

/// A running voice chat; dropping it stops everything.
pub struct Engine {
    shared: Arc<Shared>,
    /// `None` when the microphone couldn't be opened: listening only.
    _input: Option<cpal::Stream>,
    mic_error: Option<String>,
    _output: cpal::Stream,
    worker: Option<std::thread::JoinHandle<()>>,
}

/// Names of the microphones and outputs, for the settings.
pub fn devices() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let inputs = host
        .input_devices()
        .map(|l| l.filter_map(|d| name_of(&d)).collect())
        .unwrap_or_default();
    let outputs = host
        .output_devices()
        .map(|l| l.filter_map(|d| name_of(&d)).collect())
        .unwrap_or_default();
    (inputs, outputs)
}

impl Engine {
    pub fn start(config: Config, send: Outgoing) -> Result<Self, String> {
        let host = cpal::default_host();
        let output = pick(&host, config.output.as_deref(), false)?;
        let input_name = config.input.clone();
        let shared = Arc::new(Shared {
            config: Mutex::new(config),
            talking: AtomicBool::new(false),
            mic_level: AtomicU32::new((-100.0f32).to_bits()),
            sending: AtomicBool::new(false),
            listener: Mutex::new(None),
            speakers: Mutex::new(HashMap::new()),
            captured: Mutex::new(VecDeque::new()),
            playback: Mutex::new(VecDeque::new()),
            stop: AtomicBool::new(false),
        });
        let output_stream = open_output(&output, shared.clone())?;
        output_stream.play().map_err(|e| e.to_string())?;
        // Without a microphone (blocked, unplugged) you can still listen.
        let (input_stream, mic_error) = match pick(&host, input_name.as_deref(), true)
            .and_then(|input| open_input(&input, shared.clone()))
            .and_then(|stream| stream.play().map(|()| stream).map_err(|e| e.to_string()))
        {
            Ok(stream) => (Some(stream), None),
            Err(e) => {
                log::warn!("voice: listening only, {e}");
                (None, Some(e))
            }
        };
        let worker = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("arctic-voice".into())
                .spawn(move || run(shared, send))
                .map_err(|e| e.to_string())?
        };
        Ok(Self {
            shared,
            _input: input_stream,
            mic_error,
            _output: output_stream,
            worker: Some(worker),
        })
    }

    pub fn set_config(&self, config: Config) {
        *lock(&self.shared.config) = config;
    }

    /// Push-to-talk key held or released.
    pub fn set_talking(&self, on: bool) {
        self.shared.talking.store(on, Ordering::Relaxed);
    }

    /// Where you are and which way you look (from the game).
    pub fn set_listener(&self, place: Option<Place>) {
        *lock(&self.shared.listener) = place;
    }

    /// Where a speaker stands (`None` = not in your world: silent).
    pub fn set_place(&self, peer: PeerId, place: Option<Place>) {
        if let Some(s) = lock(&self.shared.speakers).get_mut(&peer) {
            s.place = place;
        }
    }

    /// Mute (0) or turn a speaker up or down.
    pub fn set_volume(&self, peer: PeerId, volume: f32) {
        if let Some(s) = lock(&self.shared.speakers).get_mut(&peer) {
            s.volume = volume.clamp(0.0, 2.0);
        }
    }

    /// A packet from a speaker.
    pub fn receive(&self, peer: PeerId, bytes: &[u8]) {
        let Some(packet) = Packet::decode(bytes) else {
            return;
        };
        let mut speakers = lock(&self.shared.speakers);
        let speaker = match speakers.entry(peer) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => match VoiceDecoder::new() {
                Ok(decoder) => e.insert(Speaker {
                    jitter: Jitter::default(),
                    decoder,
                    place: None,
                    volume: 1.0,
                    heard: None,
                }),
                Err(_) => return,
            },
        };
        speaker.jitter.push(packet.seq, packet.opus);
    }

    /// Someone left: forget them.
    pub fn remove(&self, peer: PeerId) {
        lock(&self.shared.speakers).remove(&peer);
    }

    /// Speakers heard in the last half second.
    pub fn speaking(&self) -> Vec<PeerId> {
        let now = Instant::now();
        lock(&self.shared.speakers)
            .iter()
            .filter(|(_, s)| {
                s.heard
                    .is_some_and(|t| now - t < Duration::from_millis(500))
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// You're being sent right now (voice detected or key held).
    pub fn sending(&self) -> bool {
        self.shared.sending.load(Ordering::Relaxed)
    }

    /// Why the microphone isn't used (you can only listen), if it isn't.
    pub fn mic_error(&self) -> Option<&str> {
        self.mic_error.as_deref()
    }

    /// Microphone loudness (dBFS), for a level meter.
    pub fn mic_level(&self) -> f32 {
        f32::from_bits(self.shared.mic_level.load(Ordering::Relaxed))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn pick(host: &cpal::Host, name: Option<&str>, input: bool) -> Result<cpal::Device, String> {
    let found = name.and_then(|n| {
        if input {
            host.input_devices()
                .ok()?
                .find(|d| name_of(d).as_deref() == Some(n))
        } else {
            host.output_devices()
                .ok()?
                .find(|d| name_of(d).as_deref() == Some(n))
        }
    });
    let default = if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    };
    found.or(default).ok_or_else(|| {
        if input {
            "no microphone found".to_owned()
        } else {
            "no speakers or headphones found".to_owned()
        }
    })
}

/// Longest audio kept waiting either way (avoids growing delay).
const MAX_QUEUED: usize = SAMPLE_RATE as usize / 2;

fn open_input(device: &cpal::Device, shared: Arc<Shared>) -> Result<cpal::Stream, String> {
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    let rate = config.sample_rate();
    let mut resampler = Resampler::new(rate, SAMPLE_RATE);
    let stream = device
        .build_input_stream(
            config.into(),
            move |data: &[f32], _| {
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|c| c.iter().sum::<f32>() / channels as f32)
                    .collect();
                let out = resampler.process(&mono);
                let mut q = lock(&shared.captured);
                q.extend(out);
                while q.len() > MAX_QUEUED {
                    q.pop_front();
                }
            },
            |e| log::warn!("microphone: {e}"),
            None,
        )
        .map_err(|e| format!("couldn't open the microphone: {e}"))?;
    Ok(stream)
}

fn open_output(device: &cpal::Device, shared: Arc<Shared>) -> Result<cpal::Stream, String> {
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    let rate = config.sample_rate();
    let mut resampler = StereoResampler::new(SAMPLE_RATE, rate);
    let mut pending: VecDeque<(f32, f32)> = VecDeque::new();
    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let frames = data.len() / channels;
                while pending.len() < frames {
                    let mut q = lock(&shared.playback);
                    if q.len() < 2 {
                        break;
                    }
                    let mut chunk = Vec::with_capacity(q.len() / 2);
                    while q.len() >= 2 {
                        let l = q.pop_front().unwrap_or(0.0);
                        let r = q.pop_front().unwrap_or(0.0);
                        chunk.push((l, r));
                    }
                    drop(q);
                    pending.extend(resampler.process(&chunk));
                }
                for frame in data.chunks_mut(channels) {
                    let (l, r) = pending.pop_front().unwrap_or((0.0, 0.0));
                    match frame.len() {
                        1 => frame[0] = (l + r) * 0.5,
                        _ => {
                            frame[0] = l;
                            frame[1] = r;
                            for extra in frame.iter_mut().skip(2) {
                                *extra = 0.0;
                            }
                        }
                    }
                }
            },
            |e| log::warn!("speakers: {e}"),
            None,
        )
        .map_err(|e| format!("couldn't open the speakers: {e}"))?;
    Ok(stream)
}

/// Every 20 ms: send our frame, mix everyone else's.
fn run(shared: Arc<Shared>, send: Outgoing) {
    let Ok(mut encoder) = VoiceEncoder::new() else {
        log::warn!("voice: Opus encoder failed to start");
        return;
    };
    let mut vad = Vad::new(-45.0);
    let mut seq: u16 = 0;
    let tick = Duration::from_millis(20);
    let mut next = Instant::now();
    while !shared.stop.load(Ordering::Relaxed) {
        let config = lock(&shared.config).clone();
        // Our voice.
        let frame: Option<Vec<f32>> = {
            let mut q = lock(&shared.captured);
            (q.len() >= FRAME).then(|| q.drain(..FRAME).map(|s| s * config.mic_gain).collect())
        };
        if let Some(frame) = frame {
            shared
                .mic_level
                .store(level_db(&frame).to_bits(), Ordering::Relaxed);
            let open = match config.mode {
                Mode::PushToTalk => shared.talking.load(Ordering::Relaxed),
                Mode::Voice { threshold_db } => {
                    vad.threshold_db = threshold_db;
                    vad.speaking(&frame)
                }
            };
            shared.sending.store(open, Ordering::Relaxed);
            if open && let Ok(opus) = encoder.encode(&frame) {
                send(Packet { seq, opus }.encode());
                seq = seq.wrapping_add(1);
            }
        }
        // Everyone else, placed around us.
        let listener = *lock(&shared.listener);
        let mut mix = vec![0.0f32; FRAME * 2];
        {
            let mut speakers = lock(&shared.speakers);
            for s in speakers.values_mut() {
                let samples = match s.jitter.pop() {
                    Play::Silence => continue,
                    Play::Frame(opus) => s.decoder.decode(Some(&opus)),
                    Play::Lost => s.decoder.decode(None),
                };
                let Ok(samples) = samples else { continue };
                s.heard = Some(Instant::now());
                let (l, r) = match (listener, s.place) {
                    (Some(me), Some(them)) => spatial::gains(me, them, config.range),
                    // Not placed (menus, other world): heard plainly if nothing says otherwise.
                    _ => (0.0, 0.0),
                };
                let (l, r) = (l * s.volume * config.volume, r * s.volume * config.volume);
                for (i, v) in samples.iter().enumerate() {
                    mix[i * 2] += v * l;
                    mix[i * 2 + 1] += v * r;
                }
            }
        }
        {
            let mut q = lock(&shared.playback);
            q.extend(mix.iter().map(|v| v.clamp(-1.0, 1.0)));
            while q.len() > MAX_QUEUED * 2 {
                q.pop_front();
            }
        }
        next += tick;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}

/// Linear resampling of mono audio, carrying position across calls.
pub(crate) struct Resampler {
    step: f64,
    pos: f64,
    last: f32,
}

impl Resampler {
    pub(crate) fn new(from: u32, to: u32) -> Self {
        Self {
            step: from as f64 / to as f64,
            pos: 0.0,
            last: 0.0,
        }
    }

    pub(crate) fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if (self.step - 1.0).abs() < 1e-9 {
            return input.to_vec();
        }
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 1);
        // Sample -1 is the last one of the previous call.
        let at = |i: isize| {
            if i < 0 {
                self.last
            } else {
                input.get(i as usize).copied().unwrap_or(0.0)
            }
        };
        while self.pos < input.len() as f64 - 1.0 {
            let i = self.pos.floor() as isize;
            let t = (self.pos - i as f64) as f32;
            out.push(at(i) * (1.0 - t) + at(i + 1) * t);
            self.pos += self.step;
        }
        self.pos -= input.len() as f64;
        if let Some(l) = input.last() {
            self.last = *l;
        }
        out
    }
}

/// The same for stereo pairs.
struct StereoResampler {
    left: Resampler,
    right: Resampler,
}

impl StereoResampler {
    fn new(from: u32, to: u32) -> Self {
        Self {
            left: Resampler::new(from, to),
            right: Resampler::new(from, to),
        }
    }

    fn process(&mut self, input: &[(f32, f32)]) -> Vec<(f32, f32)> {
        let l: Vec<f32> = input.iter().map(|p| p.0).collect();
        let r: Vec<f32> = input.iter().map(|p| p.1).collect();
        self.left
            .process(&l)
            .into_iter()
            .zip(self.right.process(&r))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_length_ratio() {
        let mut r = Resampler::new(44_100, 48_000);
        let mut total = 0;
        for _ in 0..100 {
            total += r.process(&vec![0.5; 441]).len();
        }
        // 1 second of 44.1 kHz → about 48000 samples.
        assert!((total as i64 - 48_000).abs() < 50, "{total}");
        let mut same = Resampler::new(48_000, 48_000);
        assert_eq!(same.process(&[0.1, 0.2]), vec![0.1, 0.2]);
    }
}
