//! Ordered packet-queue feeding for a frame-indexed native export worker.
//! A helper queue receipt never establishes native application/terrain readiness.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Serialize;
use serde_json::{Value, json};

use super::{
    Edition,
    export_clock::{FeedPhase, FeedPoint},
    offline::{
        CameraCurve, CameraPath, CameraSampleTime, OfflineConnector, ReplayControlReply, ReplayStep,
    },
    packet::{self, Completion, PacketArm},
};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerIdentity {
    pub generation: u64,
    pub viewer_id: String,
    pub player_uuid: String,
}

/// Holds the existing connection exclusively while a worker feeds packets.
/// Returning/dropping this borrow does not disconnect the helper or game.
pub struct ReplayQueue<'a> {
    session: &'a mut OfflineConnector,
    identity: ViewerIdentity,
    next_sequence: u64,
    last_time_ms: u32,
    last_point: Option<FeedPoint>,
    last_camera_pose: Option<super::offline::CameraPose>,
    camera_identity: Option<(u64, String)>,
    camera_curve: Option<CameraCurve>,
    failed: bool,
}

impl<'a> ReplayQueue<'a> {
    /// Pause and require a freshly correlated snapshot of one initialized,
    /// caught-up viewer. If playback stopped between milliseconds, queue
    /// forward to the next integer origin instead of seeking packets backward.
    /// No helper, game or account session is created here.
    pub fn attach(session: &'a mut OfflineConnector, timeout: Duration) -> Result<Self> {
        let reply = session.request_control(&json!({"op":"pause","paused":true}), timeout)?;
        let identity = viewer(&reply.state)?;
        let sequence = reply.state["lastStepSequence"]
            .as_u64()
            .filter(|s| *s < 1_000_000_000)
            .ok_or_else(|| {
                Error::Other("replay helper has no available export step sequence".into())
            })?;
        let last_time_ms = position(&reply.state)?;
        let targets = reply.state["viewerTargets"]
            .as_array()
            .filter(|targets| targets.len() == 1)
            .ok_or_else(|| {
                Error::Other("replay export needs the viewer's current target times".into())
            })?;
        let target = &targets[0];
        if target["viewerId"].as_str() != Some(identity.viewer_id.as_str()) {
            return Err(Error::Other(
                "replay target times belong to a different viewer".into(),
            ));
        }
        let time = |key: &str| -> Result<f64> {
            target[key]
                .as_f64()
                .filter(|time| time.is_finite() && (0.0..=86_400_000.0).contains(time))
                .ok_or_else(|| Error::Other("replay viewer has invalid target times".into()))
        };
        let packet_time = time("packetTimeMs")?;
        let camera_time = time("cameraTimeMs")?;
        if packet_time.floor() != f64::from(last_time_ms) {
            return Err(Error::Other(
                "replay viewer has not reached its paused queue target".into(),
            ));
        }
        let origin = packet_time.max(camera_time).ceil() as u32;
        let mut queue = Self {
            session,
            identity,
            next_sequence: sequence + 1,
            last_time_ms,
            last_point: None,
            last_camera_pose: None,
            camera_identity: None,
            camera_curve: None,
            failed: false,
        };
        if origin > last_time_ms {
            queue.queue(
                FeedPoint {
                    phase: FeedPhase::BeforeFrame,
                    frame: 0,
                    held_tick: 0,
                    replay_time_ms: origin,
                    time_remainder: 0,
                    time_denominator: 1,
                },
                timeout,
            )?;
        }
        Ok(queue)
    }
    pub fn identity(&self) -> &ViewerIdentity {
        &self.identity
    }
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    pub fn position_ms(&self) -> u32 {
        self.last_time_ms
    }
    pub(crate) fn camera_identity(&self) -> Option<&(u64, String)> {
        self.camera_identity.as_ref()
    }
    /// Requested float32 wire pose from the last acknowledged editor step;
    /// still distinct from an actual native presented camera observation.
    pub fn last_camera_pose(&self) -> Option<super::offline::CameraPose> {
        if self.failed {
            None
        } else {
            self.last_camera_pose
        }
    }

    /// Configure the same editor path used by normal playback and .abr export.
    /// Camera instructions are queued during subsequent exact steps, allowing
    /// the native engine to supply its own camera quaternion/basis.
    pub fn camera_path(
        &mut self,
        path: &CameraPath,
        timeout: Duration,
    ) -> Result<ReplayControlReply> {
        self.live()?;
        let curve = path.curve()?;
        let expected = path.sha256()?;
        self.confirm(timeout)?;
        let result = (|| -> Result<ReplayControlReply> {
            let reply = self
                .session
                .request_control(&json!({"op":"camera_path","path":path}), timeout)?;
            if viewer(&reply.state)? != self.identity
                || reply.state["camera"].as_str() != Some("path")
                || reply.state["cameraPathSha256"].as_str() != Some(expected.as_str())
                || !reply.state["cameraRevision"]
                    .as_u64()
                    .is_some_and(|revision| (1..=999_999_999).contains(&revision))
            {
                return Err(Error::Other(
                    "replay helper did not accept the exact editor camera path".into(),
                ));
            }
            Ok(reply)
        })();
        if result.is_err() {
            self.failed = true;
        } else if let Ok(reply) = &result {
            self.last_camera_pose = None;
            self.camera_identity =
                Some((reply.state["cameraRevision"].as_u64().unwrap(), expected));
            self.camera_curve = Some(curve);
        }
        result
    }

    /// Queue through the timeline point and wait for that exact helper step.
    /// Millisecond packet timestamps use the floor; the rational remainder is
    /// retained for native interpolation and the editor camera sampler, never
    /// rounded forward when choosing which recorded packets to send.
    /// The caller still needs a native application/terrain barrier afterwards.
    pub fn queue(&mut self, point: FeedPoint, timeout: Duration) -> Result<ReplayControlReply> {
        self.queue_inner(point, None, timeout)
    }
    /// The native arm must already be admitted. Tick markers belong before a
    /// planned tick; dispatch-only markers belong before an output frame.
    pub fn queue_marked(
        &mut self,
        point: FeedPoint,
        arm: &PacketArm,
        timeout: Duration,
    ) -> Result<ReplayControlReply> {
        arm.validate()?;
        let phase_matches = matches!(
            (point.phase, arm.completion),
            (FeedPhase::BeforeTick, Completion::ClientTick)
                | (FeedPhase::BeforeFrame, Completion::Dispatch)
        );
        if !phase_matches
            || arm.generation != self.identity.generation
            || arm.replay_sequence != self.next_sequence
            || arm.time_ms != point.replay_time_ms
            || arm.time_remainder != point.time_remainder
            || arm.time_denominator != point.time_denominator
        {
            return Err(Error::Other(
                "native marker differs from the planned pre-tick replay step".into(),
            ));
        }
        self.queue_inner(point, Some(arm), timeout)
    }
    fn queue_inner(
        &mut self,
        point: FeedPoint,
        arm: Option<&PacketArm>,
        timeout: Duration,
    ) -> Result<ReplayControlReply> {
        self.live()?;
        if point.phase == FeedPhase::AfterTick
            || point.replay_time_ms > 86_400_000
            || point.replay_time_ms < self.last_time_ms
            || point.time_denominator == 0
            || point.time_denominator > 1000
            || point.time_remainder >= point.time_denominator
            || point.replay_time_ms == 86_400_000 && point.time_remainder != 0
            || point.held_tick > 1_000_000_000_000
            || self.last_point.is_some_and(|last| {
                point.frame < last.frame
                    || point.held_tick < last.held_tick
                    || point.replay_time_ms == last.replay_time_ms
                        && u64::from(point.time_remainder) * u64::from(last.time_denominator)
                            < u64::from(last.time_remainder) * u64::from(point.time_denominator)
            })
        {
            return Err(Error::Other(
                "replay export feed must move forward within the same rational timeline".into(),
            ));
        }
        // Calculate from the owned, accepted path before queuing native work.
        // A matching path fingerprint alone does not prove helper sampling.
        let expected_camera = self
            .camera_curve
            .as_ref()
            .map(|curve| {
                curve
                    .sample_for_view_at(CameraSampleTime {
                        time_ms: point.replay_time_ms,
                        remainder: point.time_remainder,
                        denominator: point.time_denominator,
                    })
                    .and_then(|pose| pose.wire_pose())
            })
            .transpose()?;
        if self.next_sequence > 1_000_000_000 {
            return Err(Error::Other("replay export step limit reached".into()));
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let mut queued_camera = None;
        let result = (|| -> Result<ReplayControlReply> {
            let mut command = json!({"op":"step","generation":self.identity.generation,
                "sequence":sequence,"timeMs":point.replay_time_ms,
                "cameraTime":{"remainder":point.time_remainder,"denominator":point.time_denominator}});
            if let Some(arm) = arm {
                command["marker"] = json!(arm.marker);
            }
            let reply = self.session.request_control(&command, timeout)?;
            self.same_viewer(&reply.state)?;
            if reply.result["event"].as_str() != Some("replay_step")
                || reply.result["generation"].as_u64() != Some(self.identity.generation)
                || reply.result["sequence"].as_u64() != Some(sequence)
                || reply.result["positionMs"].as_u64() != Some(u64::from(point.replay_time_ms))
                || reply.result["packetsQueued"].as_bool() != Some(true)
                || reply.result["nativeWorldVerified"].as_bool() != Some(false)
                || reply.result["cameraTime"]["timeMs"].as_u64()
                    != Some(u64::from(point.replay_time_ms))
                || reply.result["cameraTime"]["remainder"].as_u64()
                    != Some(u64::from(point.time_remainder))
                || reply.result["cameraTime"]["denominator"].as_u64()
                    != Some(u64::from(point.time_denominator))
                || !matches!(
                    reply.state["camera"].as_str(),
                    Some("path" | "fixed" | "recorded")
                )
                || reply.result["cameraQueued"].as_bool()
                    != Some(matches!(
                        reply.state["camera"].as_str(),
                        Some("path" | "fixed")
                    ))
                || reply.state["lastStepSequence"].as_u64() != Some(sequence)
                || position(&reply.state)? != point.replay_time_ms
                || arm.is_some_and(|arm| {
                    reply.result["markerTimestamp"].as_str() != Some(arm.marker.as_str())
                        || reply.result["markerQueued"].as_bool() != Some(true)
                        || reply.result["viewerId"].as_str()
                            != Some(self.identity.viewer_id.as_str())
                })
            {
                return Err(Error::Other(
                    "replay helper did not acknowledge the exact export queue position".into(),
                ));
            }
            if reply.result["cameraQueued"].as_bool() == Some(true) {
                let pose: super::offline::CameraPose =
                    serde_json::from_value(reply.result["cameraPose"].clone())?;
                pose.validate()?;
                let values = |p: super::offline::CameraPose| [p.x, p.y, p.z, p.yaw, p.pitch, p.fov];
                // JSON has no negative zero identity. All other wire values
                // are exact float32 values promoted to double on both sides.
                if expected_camera.is_some_and(|expected| values(expected) != values(pose)) {
                    return Err(Error::Other("replay helper camera differs from the accepted path at the exact export time".into()));
                }
                queued_camera = Some(pose);
            }
            Ok(reply)
        })();
        if result.is_ok() {
            self.last_time_ms = point.replay_time_ms;
            self.last_point = Some(point);
            self.last_camera_pose = queued_camera;
        } else {
            self.failed = true;
        }
        result
    }
    /// Recheck the viewer without queuing another copy of the recorded data.
    pub fn confirm(&mut self, timeout: Duration) -> Result<ReplayControlReply> {
        self.live()?;
        let result = (|| -> Result<ReplayControlReply> {
            let reply = self
                .session
                .request_control(&json!({"op":"state"}), timeout)?;
            self.same_viewer(&reply.state)?;
            if position(&reply.state)? != self.last_time_ms
                || reply.state["lastStepSequence"].as_u64() != Some(self.next_sequence - 1)
            {
                return Err(Error::Other(
                    "replay export queue changed outside its feeder".into(),
                ));
            }
            Ok(reply)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    /// Hash the replay's actual open descriptor while this viewer is paused.
    pub fn source_identity(&mut self, timeout: Duration) -> Result<ReplayControlReply> {
        self.live()?;
        let result = (|| -> Result<ReplayControlReply> {
            let reply = self
                .session
                .request_control(&json!({"op":"recording_source"}), timeout)?;
            self.same_viewer(&reply.state)?;
            if position(&reply.state)? != self.last_time_ms
                || reply.state["lastStepSequence"].as_u64() != Some(self.next_sequence - 1)
            {
                return Err(Error::Other(
                    "replay queue changed while hashing its source".into(),
                ));
            }
            Ok(reply)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    /// Cleanup is available even after a failed feed. Verify the same viewer
    /// and last owned sequence before canceling a partial step or clearing its
    /// protocol camera. This never resets/reconnects another world.
    pub fn release_camera(&mut self, timeout: Duration) -> Result<ReplayControlReply> {
        self.failed = true;
        self.last_camera_pose = None;
        release_owned_camera(
            self.session,
            &self.identity,
            self.next_sequence - 1,
            self.camera_identity.as_ref(),
            timeout,
        )
    }
    fn live(&self) -> Result<()> {
        if self.failed {
            return Err(Error::Other(
                "replay export feeder failed; reacquire the current initialized viewer".into(),
            ));
        }
        Ok(())
    }
    fn same_viewer(&self, state: &Value) -> Result<()> {
        if viewer(state)? != self.identity {
            return Err(Error::Other(
                "replay export connection/player/generation changed".into(),
            ));
        }
        if self
            .camera_identity
            .as_ref()
            .is_some_and(|(revision, hash)| {
                state["camera"].as_str() != Some("path")
                    || state["cameraRevision"].as_u64() != Some(*revision)
                    || state["cameraPathSha256"].as_str() != Some(hash.as_str())
            })
        {
            return Err(Error::Other(
                "replay editor camera path changed during indexed export".into(),
            ));
        }
        Ok(())
    }
}

struct PendingMarker {
    arm: PacketArm,
    point: FeedPoint,
    observation: u64,
}

/// Dispatch/tick coordination for NativeExportClock's feed callback. This
/// wraps an existing replay connection and already-loaded native adapter.
/// It does not turn marker dispatch into asynchronous chunk/terrain proof.
pub struct NativeMarkerFeed<'a> {
    queue: ReplayQueue<'a>,
    edition: Edition,
    bundle: PathBuf,
    session: String,
    next_marker_sequence: u64,
    pending: Option<PendingMarker>,
    last_tick_receipt: Option<Value>,
    last_frame_dispatch: Option<Value>,
    failed: bool,
}
impl<'a> NativeMarkerFeed<'a> {
    pub fn source_identity(&mut self, timeout: Duration) -> Result<ReplayControlReply> {
        if self.failed || self.pending.is_some() {
            return Err(Error::Other(
                "cannot confirm source with a failed or pending native replay step".into(),
            ));
        }
        self.queue.source_identity(timeout)
    }
    /// Source metadata only; construction neither inspects nor starts a game.
    pub fn new(
        queue: ReplayQueue<'a>,
        edition: Edition,
        bundle: &Path,
        session: &str,
        next_marker_sequence: u64,
    ) -> Result<Self> {
        if session.is_empty()
            || session.len() > 64
            || !session
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || !(1..=1_000_000_000).contains(&next_marker_sequence)
        {
            return Err(Error::Other(
                "native marker feeder needs a native session and increasing marker sequence".into(),
            ));
        }
        Ok(Self {
            queue,
            edition,
            bundle: bundle.into(),
            session: session.into(),
            next_marker_sequence,
            pending: None,
            last_tick_receipt: None,
            last_frame_dispatch: None,
            failed: false,
        })
    }
    /// Covers only the last planned tick's marked step, not packets fed at a
    /// later fractional output timestamp. No terrain/application flag is set.
    pub fn last_tick_receipt(&self) -> Option<&Value> {
        self.last_tick_receipt.as_ref()
    }
    pub fn last_camera_pose(&self) -> Option<super::offline::CameraPose> {
        if self.failed {
            None
        } else {
            self.queue.last_camera_pose()
        }
    }
    pub fn last_frame_dispatch(&self) -> Option<&Value> {
        if self.failed {
            None
        } else {
            self.last_frame_dispatch.as_ref()
        }
    }
    pub fn last_sequence(&self) -> u64 {
        self.queue.next_sequence - 1
    }
    pub fn release_camera(&mut self, timeout: Duration) -> Result<ReplayControlReply> {
        self.failed = true;
        self.queue.release_camera(timeout)
    }
    pub fn feed(&mut self, point: FeedPoint) -> Result<()> {
        if self.failed {
            return Err(Error::Other(
                "native marker feeder failed; reset/reacquire the replay world".into(),
            ));
        }
        let result = self.feed_inner(point);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn feed_inner(&mut self, point: FeedPoint) -> Result<()> {
        let timeout = Duration::from_secs(10);
        match point.phase {
            FeedPhase::BeforeTick => {
                self.last_frame_dispatch = None;
                if self.pending.is_some() || self.next_marker_sequence > 1_000_000_000 {
                    return Err(Error::Other(
                        "native replay marker is pending or its sequence is exhausted".into(),
                    ));
                }
                let sequence = self.next_marker_sequence;
                self.next_marker_sequence += 1;
                let arm = PacketArm::new(
                    &self.session,
                    sequence,
                    &ReplayStep {
                        generation: self.queue.identity.generation,
                        sequence: self.queue.next_sequence,
                        time_ms: point.replay_time_ms,
                    },
                )?
                .with_fraction(point.time_remainder, point.time_denominator)?;
                packet::arm(self.edition, &self.bundle, &arm)?;
                self.queue.queue_marked(point, &arm, timeout)?;
                let dispatched = packet::wait_dispatch(self.edition, &self.bundle, &arm)?;
                if dispatched["stage"].as_str() != Some("dispatch_returned")
                    || dispatched["dispatcherTick"].as_u64() != point.held_tick.checked_sub(1)
                {
                    return Err(Error::Other(
                        "replay marker returned outside the held pre-tick sample".into(),
                    ));
                }
                let observation = dispatched["observationSequence"].as_u64().ok_or_else(|| {
                    Error::Other("native replay marker has no dispatcher observation".into())
                })?;
                self.pending = Some(PendingMarker {
                    arm,
                    point,
                    observation,
                });
            }
            FeedPhase::AfterTick => {
                let pending = self.pending.take().ok_or_else(|| {
                    Error::Other("native replay tick has no pre-dispatched marker".into())
                })?;
                if pending.point.frame != point.frame
                    || pending.point.held_tick != point.held_tick
                    || pending.point.replay_time_ms != point.replay_time_ms
                    || pending.point.time_remainder != point.time_remainder
                    || pending.point.time_denominator != point.time_denominator
                {
                    return Err(Error::Other(
                        "native replay tick differs from its marked feed point".into(),
                    ));
                }
                let receipt = packet::wait(self.edition, &self.bundle, &pending.arm)?;
                if receipt["clientTick"].as_u64() != Some(point.held_tick)
                    || receipt["dispatcherTick"].as_u64() != point.held_tick.checked_sub(1)
                    || receipt["observationSequence"].as_u64() != Some(pending.observation)
                {
                    return Err(Error::Other(
                        "native marker did not complete on the exact planned tick".into(),
                    ));
                }
                self.queue.confirm(timeout)?;
                self.last_tick_receipt = Some(receipt);
            }
            FeedPhase::BeforeFrame => {
                if self.pending.is_some() {
                    return Err(Error::Other(
                        "native replay marker must finish its planned tick before a frame".into(),
                    ));
                }
                if self.next_marker_sequence > 1_000_000_000 {
                    return Err(Error::Other(
                        "native replay marker sequence is exhausted".into(),
                    ));
                }
                let sequence = self.next_marker_sequence;
                self.next_marker_sequence += 1;
                let arm = PacketArm::new(
                    &self.session,
                    sequence,
                    &ReplayStep {
                        generation: self.queue.identity.generation,
                        sequence: self.queue.next_sequence,
                        time_ms: point.replay_time_ms,
                    },
                )?
                .with_fraction(point.time_remainder, point.time_denominator)?
                .dispatch_only();
                packet::arm(self.edition, &self.bundle, &arm)?;
                self.queue.queue_marked(point, &arm, timeout)?;
                let receipt = packet::wait_dispatch(self.edition, &self.bundle, &arm)?;
                if receipt["stage"].as_str() != Some("dispatch_returned")
                    || receipt["dispatcherTick"].as_u64() != Some(point.held_tick)
                    || receipt["postClientTickCompleted"].as_bool() != Some(false)
                {
                    return Err(Error::Other(
                        "frame marker did not return at the held native tick".into(),
                    ));
                }
                self.last_frame_dispatch = Some(receipt);
            }
        }
        Ok(())
    }
}

pub(crate) fn release_owned_camera(
    session: &mut OfflineConnector,
    identity: &ViewerIdentity,
    last_sequence: u64,
    camera: Option<&(u64, String)>,
    timeout: Duration,
) -> Result<ReplayControlReply> {
    let (revision, hash) = camera.ok_or_else(|| {
        Error::Other(
            "camera identity was not confirmed; refusing to clear an unowned replay camera".into(),
        )
    })?;
    let state = session.request_control(&json!({"op":"state"}), timeout)?;
    if viewer_state(&state.state, false)? != *identity
        || state.state["lastStepSequence"].as_u64() != Some(last_sequence)
    {
        return Err(Error::Other(
            "cannot release an export camera owned by a different replay step/viewer".into(),
        ));
    }
    if state.state["camera"].as_str() != Some("path")
        || state.state["cameraRevision"].as_u64() != Some(*revision)
        || state.state["cameraPathSha256"].as_str() != Some(hash.as_str())
    {
        return Err(Error::Other(
            "replay camera changed before cleanup; refusing to clear another path".into(),
        ));
    }
    session.request_control(
        &json!({"op":"cancel_step","generation":identity.generation,"sequence":last_sequence}),
        timeout,
    )?;
    let reply = session.request_control(&json!({"op":"camera_clear"}), timeout)?;
    if viewer_state(&reply.state, false)? != *identity
        || reply.state["camera"].as_str() != Some("recorded")
        || reply.state["cameraRevision"].as_u64() != revision.checked_add(1)
        || !reply.state["cameraPathSha256"].is_null()
    {
        return Err(Error::Other(
            "replay camera release did not retain its original viewer".into(),
        ));
    }
    Ok(reply)
}

fn position(state: &Value) -> Result<u32> {
    state["positionMs"]
        .as_u64()
        .filter(|v| *v <= 86_400_000)
        .map(|v| v as u32)
        .ok_or_else(|| Error::Other("replay helper has no valid queue position".into()))
}
fn viewer(state: &Value) -> Result<ViewerIdentity> {
    viewer_state(state, true)
}
fn viewer_state(state: &Value, require_ready: bool) -> Result<ViewerIdentity> {
    if state["event"].as_str() != Some("replay_state")
        || state["paused"].as_bool() != Some(true)
        || state["clientInitialized"].as_bool() != Some(true)
        || state["connectionRequired"].as_bool() != Some(false)
        || state["viewers"].as_u64() != Some(1)
        || state["initializedViewers"].as_u64() != Some(1)
        || require_ready
            && (state["packetReady"].as_bool() != Some(true)
                || !state.get("pendingStep").is_some_and(Value::is_null))
    {
        return Err(Error::Other(
            "native export needs one paused, initialized and caught-up replay viewer".into(),
        ));
    }
    let generation = state["generation"]
        .as_u64()
        .filter(|g| (1..=1_000_000_000).contains(g))
        .ok_or_else(|| Error::Other("replay export has no valid world generation".into()))?;
    let viewers = state["viewerIdentities"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| Error::Other("replay export needs exactly one viewer identity".into()))?;
    let current = &viewers[0];
    if current["generation"].as_u64() != Some(generation)
        || current["clientInitialized"].as_bool() != Some(true)
    {
        return Err(Error::Other(
            "replay viewer identity is not initialized in the current generation".into(),
        ));
    }
    let id = |key: &str| -> Result<String> {
        current[key]
            .as_str()
            .filter(|id| id.len() == 36 && uuid::Uuid::parse_str(id).is_ok())
            .map(str::to_owned)
            .ok_or_else(|| Error::Other(format!("replay export needs a valid {key}")))
    };
    Ok(ViewerIdentity {
        generation,
        viewer_id: id("viewerId")?,
        player_uuid: id("playerUuid")?,
    })
}
