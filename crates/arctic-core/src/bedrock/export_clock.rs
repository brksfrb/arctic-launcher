//! Frame-indexed export timing over the acknowledged native clock API.
//! Packet application, terrain readiness and presented pixels are separate.
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Edition,
    capture::{self, CaptureControl, ControlFence, IndexedSample},
    native::{self as client, NativeCameraTarget, NativeClockControl, NativeClockOperation},
};
use crate::{Error, Result};

/// The timeline starts at a completed native tick with zero interpolation.
/// Keep the integer frame index as the source of time, including at 30/60 FPS.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timeline {
    pub fps: u32,
    pub ticks_per_second: u32,
    pub base_tick: u64,
    pub replay_start_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTime {
    pub index: u32,
    pub held_tick: u64,
    pub partial: f32,
    /// Floor of the exact rational replay timestamp, for the packet helper.
    pub replay_time_ms: u32,
    /// Sub-millisecond remainder over `time_denominator`.
    pub time_remainder: u32,
    pub time_denominator: u32,
}

impl Timeline {
    pub fn validate(&self) -> Result<()> {
        if !(1..=60).contains(&self.fps)
            || !(1..=1000).contains(&self.ticks_per_second)
            || self.base_tick > 1_000_000_000_000
            || self.replay_start_ms > 86_400_000
        {
            return Err(Error::Other("export clock needs FPS 1–60, integral TPS 1–1000 and bounded native/replay origins".into()));
        }
        Ok(())
    }
    pub fn frame(&self, index: u32) -> Result<FrameTime> {
        self.validate()?;
        if index > self.fps * 7200 {
            return Err(Error::Other("export clock exceeds two hours".into()));
        }
        let ticks = u64::from(index) * u64::from(self.ticks_per_second);
        let held_tick = self
            .base_tick
            .checked_add(ticks / u64::from(self.fps))
            .filter(|tick| *tick <= 1_000_000_000_000)
            .ok_or_else(|| Error::Other("native export tick limit reached".into()))?;
        let (replay_time_ms, time_remainder) =
            self.replay_time(u64::from(index) * 1000, self.fps)?;
        Ok(FrameTime {
            index,
            held_tick,
            partial: (ticks % u64::from(self.fps)) as f32 / self.fps as f32,
            replay_time_ms,
            time_remainder,
            time_denominator: self.fps,
        })
    }
    fn replay_time(&self, numerator: u64, denominator: u32) -> Result<(u32, u32)> {
        let time = u64::from(self.replay_start_ms) + numerator / u64::from(denominator);
        if time > 86_400_000
            || time == 86_400_000 && !numerator.is_multiple_of(u64::from(denominator))
        {
            return Err(Error::Other(
                "export frame exceeds the replay time limit".into(),
            ));
        }
        Ok((time as u32, (numerator % u64::from(denominator)) as u32))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedPhase {
    BeforeTick,
    AfterTick,
    BeforeFrame,
}

/// The controller calls the replay feeder before every real tick, including
/// ticks between output frames at low FPS, after each acknowledged tick, and
/// again for fractional-frame data. The feeder owns its transport/application
/// barriers; AfterTick must not queue the same recorded packets a second time.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPoint {
    pub phase: FeedPhase,
    pub frame: u32,
    pub held_tick: u64,
    pub replay_time_ms: u32,
    pub time_remainder: u32,
    pub time_denominator: u32,
}

/// Own this controller on the export worker, not the UI/render thread. A failed
/// feed or acknowledgement poisons it: retrying a partially advanced frame
/// could feed packets or tick the simulation twice. Start from a new freeze.
pub struct NativeExportClock {
    timeline: Timeline,
    session: String,
    next_sequence: u64,
    next_frame: u32,
    held_tick: u64,
    freeze_sequence: u64,
    last: Value,
    failed: bool,
    captured_frame: Option<u32>,
    released: Option<Value>,
    camera_owner: Option<Value>,
    camera_released: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameClockReceipt {
    pub time: FrameTime,
    pub clock: Value,
    pub native_world_verified: bool,
    pub deterministic_video_verified: bool,
}

/// Capture must already be started with this identity/session and output FPS.
/// Targets are native engine quaternions, as required by `camera_control`.
pub struct FrameCapture<'a> {
    pub edition: Edition,
    pub bundle: &'a Path,
    pub capture_id: &'a str,
    pub index: u32,
    pub camera_sequence: u64,
    pub camera: NativeCameraTarget,
}

/// Capture the frame already returned by advance_frame after its feeder has
/// queued the public camera path. Pose comes from that step's camera receipt.
pub struct ProtocolFrameCapture<'a> {
    pub edition: Edition,
    pub bundle: &'a Path,
    pub capture_id: &'a str,
    pub frame: FrameClockReceipt,
    pub pose: super::offline::CameraPose,
    pub packet: Option<Value>,
    pub appearance_manifest: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameCaptureReceipt {
    pub frame: FrameClockReceipt,
    pub camera: Value,
    pub capture: Value,
}

impl NativeExportClock {
    /// Attach to a successful explicit freeze of both native timers at alpha 0.
    /// This constructor performs no process interaction or clock mutation.
    pub fn from_freeze(timeline: Timeline, freeze: &Value, next_sequence: u64) -> Result<Self> {
        timeline.validate()?;
        let sequence = freeze["sequence"]
            .as_u64()
            .filter(|s| (1..=1_000_000_000).contains(s))
            .ok_or_else(|| Error::Other("export clock needs a completed freeze sequence".into()))?;
        let session = freeze["session"]
            .as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
            .ok_or_else(|| Error::Other("export freeze has no valid native session".into()))?;
        if freeze["schema"].as_u64() != Some(1)
            || freeze["op"].as_str() != Some("freeze")
            || freeze["freezeSequence"].as_u64() != Some(sequence)
            || next_sequence <= sequence
            || next_sequence > 1_000_000_000
            || !held(freeze, session, &timeline, timeline.base_tick, 0.0)
        {
            return Err(Error::Other(
                "export must begin from the same acknowledged zero-fraction freeze of both timers"
                    .into(),
            ));
        }
        Ok(Self {
            held_tick: timeline.base_tick,
            timeline,
            session: session.into(),
            next_sequence,
            next_frame: 0,
            freeze_sequence: sequence,
            last: freeze.clone(),
            failed: false,
            captured_frame: None,
            released: None,
            camera_owner: None,
            camera_released: None,
        })
    }
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }
    pub fn last_receipt(&self) -> &Value {
        &self.last
    }
    pub fn next_frame(&self) -> u32 {
        self.next_frame
    }

    /// Tick only as many times as this frame's rational timestamp requires.
    /// This does not add a tick to satisfy a packet marker on a fractional
    /// frame; that marker's later-tick receipt must remain pending instead.
    pub fn advance_frame<F>(
        &mut self,
        edition: Edition,
        bundle: &Path,
        index: u32,
        mut feed: F,
    ) -> Result<FrameClockReceipt>
    where
        F: FnMut(FeedPoint) -> Result<()>,
    {
        if self.failed {
            return Err(Error::Other(
                "export clock failed; a new acknowledged freeze is required".into(),
            ));
        }
        if index != self.next_frame {
            return Err(Error::Other(
                "export frames must be requested exactly once in increasing order".into(),
            ));
        }
        let time = self.timeline.frame(index)?;
        let result = (|| -> Result<FrameClockReceipt> {
            while self.held_tick < time.held_tick {
                let next_tick = self.held_tick + 1;
                let offset = next_tick - self.timeline.base_tick;
                let (replay_time_ms, time_remainder) = self
                    .timeline
                    .replay_time(offset * 1000, self.timeline.ticks_per_second)?;
                feed(FeedPoint {
                    phase: FeedPhase::BeforeTick,
                    frame: index,
                    held_tick: next_tick,
                    replay_time_ms,
                    time_remainder,
                    time_denominator: self.timeline.ticks_per_second,
                })?;
                self.apply(edition, bundle, NativeClockOperation::Step, next_tick, 0.0)?;
                feed(FeedPoint {
                    phase: FeedPhase::AfterTick,
                    frame: index,
                    held_tick: next_tick,
                    replay_time_ms,
                    time_remainder,
                    time_denominator: self.timeline.ticks_per_second,
                })?;
            }
            feed(FeedPoint {
                phase: FeedPhase::BeforeFrame,
                frame: index,
                held_tick: time.held_tick,
                replay_time_ms: time.replay_time_ms,
                time_remainder: time.time_remainder,
                time_denominator: time.time_denominator,
            })?;
            self.apply(
                edition,
                bundle,
                NativeClockOperation::Sample,
                time.held_tick,
                time.partial,
            )?;
            Ok(FrameClockReceipt {
                time,
                clock: self.last.clone(),
                native_world_verified: false,
                deterministic_video_verified: false,
            })
        })();
        if result.is_ok() {
            self.next_frame += 1;
        } else {
            self.failed = true;
        }
        result
    }

    /// Connect exact native timing, restored camera consumption and a later
    /// matching presented frame to the indexed encoder. No screenshot, launch,
    /// navigation or guessed camera basis is used. This still depends on the
    /// feeder's packet/world/terrain barriers and does not certify determinism.
    pub fn capture_frame<F>(
        &mut self,
        request: FrameCapture<'_>,
        feed: F,
    ) -> Result<FrameCaptureReceipt>
    where
        F: FnMut(FeedPoint) -> Result<()>,
    {
        // Keep one sequence for guarded cleanup before advancing any ticks.
        if !(1..1_000_000_000).contains(&request.camera_sequence) {
            self.failed = true;
            return Err(Error::Other(
                "direct export camera sequence must reserve its final release command".into(),
            ));
        }
        let mut frame = self.advance_frame(request.edition, request.bundle, request.index, feed)?;
        let result = (|| -> Result<FrameCaptureReceipt> {
            let camera = if let Some(owner) = self.camera_owner.as_ref() {
                client::camera_control_owned(
                    request.edition,
                    request.bundle,
                    &self.session,
                    request.camera_sequence,
                    Some(request.camera),
                    owner,
                )?
            } else {
                client::camera_control_from_idle(
                    request.edition,
                    request.bundle,
                    &self.session,
                    request.camera_sequence,
                    request.camera,
                )?
            };
            // Retain ownership before any later clock/fence/capture failure,
            // so cleanup can release this exact acknowledged override.
            self.camera_owner = Some(camera.clone());
            // Camera consumption can take time. Renew before waiting for the
            // presented identity, leaving a full lease for bounded readback.
            self.apply(
                request.edition,
                request.bundle,
                NativeClockOperation::Renew,
                frame.time.held_tick,
                frame.time.partial,
            )?;
            frame.clock = self.last.clone();
            let fence = ControlFence::from_receipts(&frame.clock, &camera, true)?;
            let capture = self.capture_matching(
                request.edition,
                request.bundle,
                request.capture_id,
                &frame,
                fence,
            )?;
            Ok(FrameCaptureReceipt {
                frame,
                camera,
                capture,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Use the engine's public camera instruction route. This does not patch
    /// any camera component, synthesize a quaternion or advance another tick.
    /// The actual presented view, held clock and fresh GPU tag must all match.
    pub fn capture_protocol_frame(
        &mut self,
        request: ProtocolFrameCapture<'_>,
    ) -> Result<FrameCaptureReceipt> {
        if self.camera_owner.is_some() {
            self.failed = true;
            return Err(Error::Other(
                "release the direct native camera before starting a protocol camera export".into(),
            ));
        }
        let index = request.frame.time.index;
        if self.failed
            || Some(self.next_frame) != index.checked_add(1)
            || self.timeline.frame(index)? != request.frame.time
            || self.held_tick != request.frame.time.held_tick
            || self.last["sequence"] != request.frame.clock["sequence"]
            || self.last["requestId"] != request.frame.clock["requestId"]
            || self
                .captured_frame
                .is_some_and(|captured| captured >= index)
        {
            return Err(Error::Other(
                "protocol capture needs the latest unconsumed native export frame receipt".into(),
            ));
        }
        request.pose.validate()?;
        let mut frame = request.frame;
        frame.native_world_verified = false;
        frame.deterministic_video_verified = false;
        let result = (|| -> Result<FrameCaptureReceipt> {
            self.apply(
                request.edition,
                request.bundle,
                NativeClockOperation::Renew,
                frame.time.held_tick,
                frame.time.partial,
            )?;
            frame.clock = self.last.clone();
            let mut fence = ControlFence::from_presented(&frame.clock, request.pose, true)?;
            if let Some(packet) = request.packet.as_ref() {
                fence = fence
                    .with_packet(packet)?
                    .with_cosmetic_clock(frame.time.replay_time_ms)?;
                if !fence.packet_dispatch.as_ref().is_some_and(|packet| {
                    packet.time_remainder == frame.time.time_remainder
                        && packet.time_denominator == frame.time.time_denominator
                }) {
                    return Err(Error::Other(
                        "native frame marker differs from the exact export timestamp".into(),
                    ));
                }
            }
            if let Some(hash) = request.appearance_manifest {
                fence = fence.with_appearance(hash, frame.time.replay_time_ms)?;
            }
            let capture = self.capture_matching(
                request.edition,
                request.bundle,
                request.capture_id,
                &frame,
                fence,
            )?;
            let camera = serde_json::json!({"mode":"protocol","requestedPose":request.pose,
                "presentedPose":capture["receipt"]["world"]["camera"],"nativeWorldVerified":false,"terrainReady":false});
            Ok(FrameCaptureReceipt {
                frame,
                camera,
                capture,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn capture_matching(
        &mut self,
        edition: Edition,
        bundle: &Path,
        capture_id: &str,
        frame: &FrameClockReceipt,
        mut fence: ControlFence,
    ) -> Result<Value> {
        use std::time::{Duration, Instant};
        let index = frame.time.index;
        if self
            .captured_frame
            .is_some_and(|captured| captured >= index)
        {
            return Err(Error::Other(
                "native export frame was already captured".into(),
            ));
        }
        fence.ticks_per_second = Some(self.timeline.ticks_per_second as f32);
        fence.require_subchunk_idle = true;
        fence.require_network_idle = true;
        fence.require_renderer_idle = true;
        fence.require_frame_input = true;
        fence.require_camera_time = true;
        let started = Instant::now();
        let sample = loop {
            let observed =
                capture::control(edition, bundle, capture_id, CaptureControl::Observe {})?;
            if observed["cameraTimeFenceImplemented"].as_bool() != Some(true) {
                return Err(Error::Other("indexed export needs a native bundle with producer-frame camera timing receipts".into()));
            }
            if observed["mode"].as_str() != Some("indexed")
                || observed["fps"].as_u64() != Some(u64::from(self.timeline.fps))
            {
                return Err(Error::Other(
                    "indexed encoder mode/FPS differs from the export clock".into(),
                ));
            }
            if observed["observedWorld"]["world"]["session"].as_str() != Some(self.session.as_str())
            {
                return Err(Error::Other(
                    "indexed export observation belongs to a different native session".into(),
                ));
            }
            match IndexedSample::from_observation(&observed,fence.clone()) {
                Ok(sample) if sample.ticks==frame.time.held_tick && sample.partial.to_bits()==frame.time.partial.to_bits() => break sample,
                _ if started.elapsed()>=Duration::from_secs(5) => return Err(Error::Other("native clock, camera, cosmetic phase, frame inputs or network/subchunk/renderer work did not reach the export frame".into())),
                _ => std::thread::sleep(Duration::from_millis(25)),
            }
        };
        let result = capture::control(
            edition,
            bundle,
            capture_id,
            CaptureControl::Sample { index, sample },
        )?;
        self.captured_frame = Some(index);
        Ok(result)
    }

    fn sequence(&mut self) -> Result<u64> {
        if self.next_sequence > 1_000_000_000 {
            return Err(Error::Other(
                "native export clock sequence limit reached".into(),
            ));
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        Ok(sequence)
    }
    fn apply(
        &mut self,
        edition: Edition,
        bundle: &Path,
        op: NativeClockOperation,
        tick: u64,
        partial: f32,
    ) -> Result<()> {
        let sequence = self.sequence()?;
        let receipt = client::clock_control_owned(
            edition,
            bundle,
            &self.session,
            sequence,
            NativeClockControl {
                op,
                partial,
                real_timer: true,
                lease_ms: 30000,
                rate: None,
            },
            &self.last,
        )?;
        if receipt["sequence"].as_u64() != Some(sequence)
            || receipt["freezeSequence"].as_u64() != Some(self.freeze_sequence)
            || !held(&receipt, &self.session, &self.timeline, tick, partial)
            || op == NativeClockOperation::Step
                && (receipt["worldTickComplete"].as_bool() != Some(true)
                    || receipt["completedTick"].as_u64() != Some(tick))
        {
            return Err(Error::Other(
                "native export clock changed session, tick, rate, fraction or freeze ownership"
                    .into(),
            ));
        }
        self.held_tick = tick;
        self.last = receipt;
        Ok(())
    }
    /// Explicit cleanup, also available after a failed frame. No Drop handler
    /// starts processes or blocks an arbitrary thread on a native consumer.
    pub fn resume(&mut self, edition: Edition, bundle: &Path) -> Result<Value> {
        self.failed = true;
        let camera_result = (|| -> Result<()> {
            if self.camera_released.is_none()
                && let Some(owner) = self.camera_owner.as_ref()
            {
                let sequence = owner["sequence"]
                    .as_u64()
                    .and_then(|s| s.checked_add(1))
                    .filter(|s| *s <= 1_000_000_000)
                    .ok_or_else(|| {
                        Error::Other("owned export camera has no release sequence remaining".into())
                    })?;
                self.camera_released = Some(client::camera_control_owned(
                    edition,
                    bundle,
                    &self.session,
                    sequence,
                    None,
                    owner,
                )?);
            }
            Ok(())
        })();
        // Release the clock even when camera cleanup is rejected. The camera
        // command cannot force-clear a replacement owned by another caller.
        let clock_result = (|| -> Result<Value> {
            if let Some(released) = self.released.as_ref() {
                return Ok(released.clone());
            }
            let sequence = self.sequence()?;
            let released =
                client::resume_owned_clock(edition, bundle, &self.session, sequence, &self.last)?;
            self.released = Some(released.clone());
            Ok(released)
        })();
        match (camera_result, clock_result) {
            (Ok(()), Ok(mut released)) => {
                if let Some(camera) = self.camera_released.as_ref() {
                    released["cameraRelease"] = camera.clone();
                }
                Ok(released)
            }
            (Err(camera), Err(clock)) => Err(Error::Other(format!(
                "export camera cleanup failed: {camera}; clock cleanup failed: {clock}"
            ))),
            (Err(camera), Ok(_)) => Err(Error::Other(format!(
                "export clock released; camera cleanup failed: {camera}"
            ))),
            (Ok(()), Err(clock)) => Err(clock),
        }
    }
}

fn held(receipt: &Value, session: &str, timeline: &Timeline, tick: u64, partial: f32) -> bool {
    receipt["session"].as_str() == Some(session)
        && receipt["error"].is_null()
        && receipt["frozen"].as_bool() == Some(true)
        && receipt["leaseActive"].as_bool() == Some(true)
        && receipt["realTimerControlled"].as_bool() == Some(true)
        && receipt["heldTick"].as_u64() == Some(tick)
        && ["timer", "realTimer"].iter().all(|key| {
            let timer = &receipt[*key];
            timer["alpha"]
                .as_f64()
                .is_some_and(|v| (v as f32).to_bits() == partial.to_bits())
                && timer["ticksPerSecond"].as_f64() == Some(f64::from(timeline.ticks_per_second))
        })
}
