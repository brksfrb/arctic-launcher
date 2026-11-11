//! Indexed capture of actual native presented frames. A replay controller must
//! separately establish its world reconstruction and terrain readiness.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Edition;
#[cfg(windows)]
use crate::error::IoContext;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncoderKind {
    #[default]
    Ffmpeg,
    ArcticBuiltin,
}
impl EncoderKind {
    pub fn is_ffmpeg(&self) -> bool {
        *self == Self::Ffmpeg
    }
    /// Requested indexed-encoding configuration, not a repeatability receipt.
    pub fn indexed_profile(&self) -> &'static str {
        match self {
            Self::ArcticBuiltin => "mf_hardware_disabled_h264_v1",
            Self::Ffmpeg => "libx264_threads1_bitexact_v1",
        }
    }
}

/// Read-only request-contract check on an already correlated capture reply.
/// Capability/selected-profile declarations do not prove a running codec's
/// identity, successful encoding, native world readiness or repeatability.
pub(super) fn require_indexed_encoding(
    status: &Value,
    encoder: EncoderKind,
    selected: bool,
) -> Result<()> {
    let kind = match encoder {
        EncoderKind::Ffmpeg => "ffmpeg",
        EncoderKind::ArcticBuiltin => "arctic_builtin",
    };
    let capability = &status["indexedEncoding"];
    if capability["schema"].as_u64() != Some(1)
        || capability["frameMetadataSchema"].as_u64() != Some(2)
        || capability["profiles"][kind].as_str() != Some(encoder.indexed_profile())
    {
        return Err(Error::Other(
            "native capture bundle does not advertise the required indexed encoding profile".into(),
        ));
    }
    if selected && status["encodingProfile"].as_str() != Some(encoder.indexed_profile()) {
        return Err(Error::Other(
            "native capture selected a different or unknown indexed encoding profile".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedOptions {
    pub ffmpeg: PathBuf,
    #[serde(default, skip_serializing_if = "EncoderKind::is_ffmpeg")]
    pub encoder: EncoderKind,
    pub output: PathBuf,
    pub fps: u32,
    pub frames: u32,
    #[serde(default = "clean_view")]
    pub clean_view: bool,
    #[serde(default = "settle_frames")]
    pub settle_frames: u32,
}
fn clean_view() -> bool {
    true
}
fn settle_frames() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ControlIdentity {
    pub sequence: u64,
    pub request_id: String,
}

/// Require the clock operation and either its scoped camera consumer or an
/// expected presented view. These are not terrain/packet application fences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ControlFence {
    pub clock: ControlIdentity,
    pub camera: CameraFence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks_per_second: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_dispatch: Option<PacketIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cosmetic_clock_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<AppearanceIdentity>,
    /// Reject observed subchunk insertion work during the native world pass.
    /// This is one work-queue condition, not complete terrain readiness.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_subchunk_idle: bool,
    /// Require the active session's pending chunk maps and I/O task group idle.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_network_idle: bool,
    /// Require idle renderer groups, pending maps and observed shared-chunk builds.
    /// This does not certify chunk geometry, lighting or GPU completion.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_renderer_idle: bool,
    /// Require this draw's frame argument, primary view and terrain queue headers.
    /// This is not a resource-payload or GPU execution receipt.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_frame_input: bool,
    /// Require the camera pre-render time output from this producer frame,
    /// bound to the same acknowledged held-clock command and world sample.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_camera_time: bool,
    #[serde(default = "require_real_timer")]
    pub real_timer: bool,
}
fn require_real_timer() -> bool {
    true
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceIdentity {
    pub manifest_hash: String,
    pub time_ms: u32,
}
impl AppearanceIdentity {
    fn valid(&self) -> bool {
        self.time_ms <= 86_400_000
            && self.manifest_hash.len() == 64
            && self
                .manifest_hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
    fn matches(&self, value: &Value) -> bool {
        value["manifestHash"].as_str() == Some(self.manifest_hash.as_str())
            && value["timeMs"].as_u64() == Some(u64::from(self.time_ms))
            && value["renderReady"].as_bool() == Some(true)
    }
}

/// Existing scoped commands retain their JSON identity. Protocol cameras use
/// an expected native presented view, with no private quaternion conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CameraFence {
    Controlled(ControlIdentity),
    Presented(PresentedCamera),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentedCamera {
    pub session: String,
    pub pose: [f64; 6],
}
impl PresentedCamera {
    pub fn new(session: &str, pose: super::offline::CameraPose) -> Result<Self> {
        let source = [pose.x, pose.y, pose.z, pose.yaw, pose.pitch, pose.fov];
        if !token(session)
            || source.iter().any(|v| !v.is_finite())
            || source[..3].iter().any(|v| v.abs() > 30_000_000.0)
            || pose.yaw.abs() > 360_000.0
            || pose.pitch.abs() > 90.0
            || !(1.0..=179.0).contains(&pose.fov)
        {
            return Err(Error::Other(
                "invalid protocol camera target/session".into(),
            ));
        }
        let mut pose = source;
        // Match the camera packet's normalized yaw and float32 wire fields.
        pose[3] = (pose[3] + 180.0).rem_euclid(360.0) - 180.0;
        pose = pose.map(|v| f64::from(v as f32));
        if pose[3] == 180.0 {
            pose[3] = -180.0;
        }
        Ok(Self {
            session: session.into(),
            pose,
        })
    }
    fn valid(&self) -> bool {
        token(&self.session)
            && self.pose.iter().all(|v| v.is_finite())
            && self.pose[..3].iter().all(|v| v.abs() <= 30_000_000.0)
            && self.pose[3].abs() <= 360_000.0
            && self.pose[4].abs() <= 90.0
            && (1.0..=179.0).contains(&self.pose[5])
    }
    fn matches(&self, world: &Value) -> bool {
        let Some(actual) = world["camera"].as_array().filter(|v| v.len() == 6) else {
            return false;
        };
        world["session"].as_str() == Some(self.session.as_str())
            && actual
                .iter()
                .zip(self.pose)
                .enumerate()
                .all(|(i, (actual, expected))| {
                    actual.as_f64().filter(|v| v.is_finite()).is_some_and(|v| {
                        if i < 3 {
                            // Native float fields use their shortest JSON spelling.
                            // Recover the observed f32 before comparing with the wire
                            // target; decimal error grows beyond .001 far from origin.
                            (v as f32) == (expected as f32)
                        } else {
                            let delta = if i == 3 || i == 4 {
                                (v - expected + 180.0).rem_euclid(360.0) - 180.0
                            } else {
                                v - expected
                            };
                            delta.abs() <= 0.001
                        }
                    })
                })
    }
}
impl CameraFence {
    fn valid(&self) -> bool {
        match self {
            Self::Controlled(id) => {
                (1..=1_000_000_000).contains(&id.sequence) && token(&id.request_id)
            }
            Self::Presented(camera) => camera.valid(),
        }
    }
    fn matches(&self, world: &Value) -> bool {
        match self {
            Self::Presented(camera) => camera.matches(world),
            Self::Controlled(id) => {
                let camera = &world["controls"]["camera"];
                camera["sequence"].as_u64() == Some(id.sequence)
                    && camera["requestId"].as_str() == Some(id.request_id.as_str())
                    && camera["matrixConsumed"].as_bool() == Some(true)
                    && camera["restored"].as_bool() == Some(true)
                    && camera["playerUnchanged"].as_bool() == Some(true)
                    && camera["producerFrame"]
                        .as_u64()
                        .is_some_and(|f| f <= u64::from(u32::MAX))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PacketIdentity {
    pub session: String,
    pub sequence: u64,
    pub request_id: String,
    pub marker: String,
    pub generation: u64,
    pub replay_sequence: u64,
    pub time_ms: u32,
    #[serde(default)]
    pub time_remainder: u32,
    #[serde(default = "unit_denominator")]
    pub time_denominator: u32,
    #[serde(default = "default_packet_stage")]
    pub stage: String,
}
fn default_packet_stage() -> String {
    "client_tick_completed".into()
}
fn unit_denominator() -> u32 {
    1
}
impl PacketIdentity {
    fn fraction_matches(&self, value: &Value, remainder: &str, denominator: &str) -> bool {
        value[remainder]
            .as_u64()
            .or_else(|| value.get(remainder).is_none().then_some(0))
            == Some(u64::from(self.time_remainder))
            && value[denominator]
                .as_u64()
                .or_else(|| value.get(denominator).is_none().then_some(1))
                == Some(u64::from(self.time_denominator))
    }
    fn valid(&self) -> bool {
        token(&self.session)
            && matches!(
                self.stage.as_str(),
                "dispatch_returned" | "client_tick_completed"
            )
            && [self.sequence, self.generation, self.replay_sequence]
                .iter()
                .all(|s| (1..=1_000_000_000).contains(s))
            && token(&self.request_id)
            && self.time_ms <= 86_400_000
            && self.marker.len() == 16
            && self.marker.starts_with("37c1")
            && (1..=1000).contains(&self.time_denominator)
            && self.time_remainder < self.time_denominator
            && (self.time_ms < 86_400_000 || self.time_remainder == 0)
            && self
                .marker
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
    fn matches(&self, world: &Value) -> bool {
        let value = &world["controls"]["packetDispatch"];
        value["sequence"].as_u64() == Some(self.sequence)
            && value["requestId"].as_str() == Some(self.request_id.as_str())
            && value["session"].as_str() == Some(self.session.as_str())
            && world["session"] == value["session"]
            && value["marker"].as_str() == Some(self.marker.as_str())
            && value["generation"].as_u64() == Some(self.generation)
            && value["replaySequence"].as_u64() == Some(self.replay_sequence)
            && value["timeMs"].as_u64() == Some(u64::from(self.time_ms))
            && value["stage"].as_str() == Some(self.stage.as_str())
            && self.fraction_matches(value, "timeRemainder", "timeDenominator")
            && value["dispatcherReturned"].as_bool() == Some(true)
            && value["primaryClientThread"].as_bool() == Some(true)
            && value["observationSequence"].as_u64().is_some_and(|s| s > 0)
            && if self.stage == "dispatch_returned" {
                value["completion"].as_str() == Some("dispatch")
                    && value["postClientTickCompleted"].as_bool() == Some(false)
                    && value["dispatcherTick"]
                        .as_u64()
                        .zip(world["ticks"].as_u64())
                        .is_some_and(|(a, b)| a == b)
            } else {
                value["completion"].as_str() == Some("client_tick")
                    && value["postClientTickCompleted"].as_bool() == Some(true)
                    && value["dispatcherTick"]
                        .as_u64()
                        .zip(value["clientTick"].as_u64())
                        .zip(world["ticks"].as_u64())
                        .is_some_and(|((a, b), tick)| b > a && tick >= b)
            }
    }
}

impl ControlFence {
    /// Build expected identities from acknowledged operations. This does not
    /// establish that a frame has arrived with those native receipts.
    pub fn from_receipts(clock: &Value, camera: &Value, real_timer: bool) -> Result<Self> {
        let identity = |v: &Value| -> Result<ControlIdentity> {
            let sequence = v["sequence"]
                .as_u64()
                .filter(|s| (1..=1_000_000_000).contains(s))
                .ok_or_else(|| Error::Other("control receipt has no valid sequence".into()))?;
            let request_id = v["requestId"]
                .as_str()
                .filter(|id| token(id))
                .ok_or_else(|| {
                    Error::Other("control receipt has no valid request identity".into())
                })?;
            Ok(ControlIdentity {
                sequence,
                request_id: request_id.into(),
            })
        };
        if !clock["session"].as_str().is_some_and(token)
            || clock["session"] != camera["session"]
            || !clock["error"].is_null()
            || !camera["error"].is_null()
            || clock["frozen"].as_bool() != Some(true)
            || clock["leaseActive"].as_bool() != Some(true)
            || clock["heldTick"].as_u64().is_none()
            || real_timer && clock["realTimerControlled"].as_bool() != Some(true)
            || camera["op"].as_str() != Some("set")
            || camera["active"].as_bool() != Some(true)
            || camera["restored"].as_bool() != Some(true)
            || camera["playerUnchanged"].as_bool() != Some(true)
        {
            return Err(Error::Other("capture needs completed held-clock and restored camera receipts from the same live session".into()));
        }
        Ok(Self {
            clock: identity(clock)?,
            camera: CameraFence::Controlled(identity(camera)?),
            ticks_per_second: None,
            packet_dispatch: None,
            cosmetic_clock_ms: None,
            appearance: None,
            require_subchunk_idle: false,
            require_network_idle: false,
            require_renderer_idle: false,
            require_frame_input: false,
            require_camera_time: false,
            real_timer,
        })
    }
    /// Protocol cameras are accepted only when their expected view reaches an
    /// actual presented world layer at the acknowledged held-clock sample.
    pub fn from_presented(
        clock: &Value,
        pose: super::offline::CameraPose,
        real_timer: bool,
    ) -> Result<Self> {
        let session = clock["session"]
            .as_str()
            .filter(|s| token(s))
            .ok_or_else(|| {
                Error::Other("presented camera needs a valid native clock session".into())
            })?;
        let sequence = clock["sequence"]
            .as_u64()
            .filter(|s| (1..=1_000_000_000).contains(s))
            .ok_or_else(|| Error::Other("presented camera needs a valid clock sequence".into()))?;
        let request_id = clock["requestId"]
            .as_str()
            .filter(|s| token(s))
            .ok_or_else(|| {
                Error::Other("presented camera needs a valid clock request identity".into())
            })?;
        if !clock["error"].is_null()
            || clock["frozen"].as_bool() != Some(true)
            || clock["leaseActive"].as_bool() != Some(true)
            || clock["heldTick"].as_u64().is_none()
            || real_timer && clock["realTimerControlled"].as_bool() != Some(true)
        {
            return Err(Error::Other(
                "presented camera needs an acknowledged held-clock sample".into(),
            ));
        }
        Ok(Self {
            clock: ControlIdentity {
                sequence,
                request_id: request_id.into(),
            },
            camera: CameraFence::Presented(PresentedCamera::new(session, pose)?),
            ticks_per_second: None,
            packet_dispatch: None,
            cosmetic_clock_ms: None,
            appearance: None,
            require_subchunk_idle: false,
            require_network_idle: false,
            require_renderer_idle: false,
            require_frame_input: false,
            require_camera_time: false,
            real_timer,
        })
    }
    /// Require the marker's declared native stage. Dispatch-only stages match
    /// that same held tick; post-tick stages require their completed later tick.
    /// Terrain/application state remains unproven.
    pub fn with_packet(mut self, receipt: &Value) -> Result<Self> {
        let mut subset = serde_json::json!({"session":receipt["session"],"sequence":receipt["sequence"],"requestId":receipt["requestId"],
            "marker":receipt["marker"],"generation":receipt["generation"],"replaySequence":receipt["replaySequence"],"timeMs":receipt["timeMs"],"stage":receipt["stage"]});
        for key in ["timeRemainder", "timeDenominator"] {
            if let Some(value) = receipt.get(key) {
                subset[key] = value.clone();
            }
        }
        let packet: PacketIdentity = serde_json::from_value(subset)?;
        let completed = if packet.stage == "dispatch_returned" {
            receipt["completion"].as_str() == Some("dispatch")
                && receipt["postClientTickCompleted"].as_bool() == Some(false)
                && receipt["dispatcherTick"].as_u64().is_some()
        } else {
            receipt["completion"].as_str() == Some("client_tick")
                && receipt["postClientTickCompleted"].as_bool() == Some(true)
                && receipt["dispatcherTick"]
                    .as_u64()
                    .zip(receipt["clientTick"].as_u64())
                    .is_some_and(|(a, b)| b > a)
        };
        if !packet.valid()
            || !completed
            || !token(receipt["session"].as_str().unwrap_or(""))
            || receipt["dispatcherReturned"].as_bool() != Some(true)
            || receipt["primaryClientThread"].as_bool() != Some(true)
            || !receipt["observationSequence"]
                .as_u64()
                .is_some_and(|s| s > 0)
            || !receipt["error"].is_null()
        {
            return Err(Error::Other(
                "packet receipt has not completed its declared primary-thread dispatch/tick stage"
                    .into(),
            ));
        }
        self.packet_dispatch = Some(packet);
        Ok(self)
    }
    fn matches_world(&self, world: &Value) -> bool {
        let clock = &world["controls"]["clock"];
        let matches = |id: &ControlIdentity, value: &Value| {
            value["sequence"].as_u64() == Some(id.sequence)
                && value["requestId"].as_str() == Some(id.request_id.as_str())
        };
        matches(&self.clock, clock)
            && self.camera.matches(world)
            && self.ticks_per_second.is_none_or(|rate| {
                clock["ticksPerSecond"]
                    .as_f64()
                    .is_some_and(|actual| (actual as f32).to_bits() == rate.to_bits())
            })
            && self
                .packet_dispatch
                .as_ref()
                .is_none_or(|packet| packet.matches(world))
            && self.cosmetic_clock_ms.is_none_or(|time| {
                world["controls"]["cosmeticClockMs"].as_u64() == Some(u64::from(time))
                    && self.packet_dispatch.as_ref().is_some_and(|packet| {
                        packet.fraction_matches(
                            &world["controls"],
                            "cosmeticClockRemainder",
                            "cosmeticClockDenominator",
                        )
                    })
            })
            && self
                .appearance
                .as_ref()
                .is_none_or(|appearance| appearance.matches(&world["controls"]["appearance"]))
            && (!self.require_subchunk_idle || subchunk_idle(world))
            && (!self.require_network_idle || network_idle(world))
            && (!self.require_renderer_idle || renderer_idle(world))
            && (!self.require_frame_input || frame_input(world))
            && (!self.require_camera_time || camera_time(world))
            && clock["freezeSequence"].as_u64().is_some_and(|s| s > 0)
            && clock["ticks"].as_u64().is_some()
            && clock["ticks"] == world["ticks"]
            && clock["partial"]
                .as_f64()
                .zip(world["partial"].as_f64())
                .is_some_and(|(a, b)| (a as f32).to_bits() == (b as f32).to_bits())
            && (!self.real_timer || clock["realTimerControlled"].as_bool() == Some(true))
    }
    /// Require the cape/idle phase actually used by the Java world layer.
    /// A fresh dispatch marker must carry this same recorded timestamp.
    pub fn with_cosmetic_clock(mut self, time_ms: u32) -> Result<Self> {
        self.cosmetic_clock_ms = Some(time_ms);
        if !self.cosmetic_clock_valid() {
            return Err(Error::Other(
                "replay cosmetic clock needs a matching dispatch-only marker and both held timers"
                    .into(),
            ));
        }
        Ok(self)
    }
    fn cosmetic_clock_valid(&self) -> bool {
        self.cosmetic_clock_ms.is_none_or(|time| {
            time <= 86_400_000
                && self.real_timer
                && self.packet_dispatch.as_ref().is_some_and(|packet| {
                    packet.stage == "dispatch_returned" && packet.time_ms == time
                })
        })
    }
    pub fn with_appearance(mut self, manifest_hash: &str, time_ms: u32) -> Result<Self> {
        let appearance = AppearanceIdentity {
            manifest_hash: manifest_hash.into(),
            time_ms,
        };
        if !appearance.valid() || self.cosmetic_clock_ms != Some(time_ms) {
            return Err(Error::Other(
                "appearance capture needs a valid manifest hash and matching replay cosmetic clock"
                    .into(),
            ));
        }
        self.appearance = Some(appearance);
        Ok(self)
    }
}

/// Older manifests omit this optional fence. New requests must retain and
/// satisfy it during receipt writing/comparison as well as live admission.
pub(super) fn validate_camera_time_frame(frame: &Value) -> Result<()> {
    match frame["requiredControls"].get("requireCameraTime") {
        None | Some(Value::Bool(false)) => Ok(()),
        Some(Value::Bool(true)) => {
            let fence: ControlFence = serde_json::from_value(frame["requiredControls"].clone())?;
            if !fence.matches_world(&frame["world"]) {
                return Err(Error::Other(
                    "captured camera timing differs from its required frame/clock fence".into(),
                ));
            }
            Ok(())
        }
        _ => Err(Error::Other(
            "invalid captured camera timing requirement".into(),
        )),
    }
}
fn camera_time(world: &Value) -> bool {
    let clock = &world["controls"]["clock"];
    let time = &world["controls"]["cameraTime"];
    let copy = &time["clock"];
    let words = |key: &str| -> Option<[f32; 2]> {
        let values = time[key].as_array().filter(|values| values.len() == 2)?;
        let a = values[0].as_f64()? as f32;
        let b = values[1].as_f64()? as f32;
        (a.is_finite()
            && b.is_finite()
            && (0. ..=1000.).contains(&a)
            && (0. ..=1_000_000_000.).contains(&b))
        .then_some([a, b])
    };
    let Some((expected, observed)) = words("expected").zip(words("observed")) else {
        return false;
    };
    time["schema"].as_u64() == Some(1)
        && time["session"].as_str() == world["session"].as_str()
        && world["session"].as_str().is_some()
        && time["worldSequence"]
            .as_u64()
            .is_some_and(|sequence| sequence > 0)
        && time["worldSequence"] == world["sequence"]
        && time["producerFrame"]
            .as_u64()
            .is_some_and(|frame| u32::try_from(frame).is_ok())
        && time["outputMatched"].as_bool() == Some(true)
        && expected.map(f32::to_bits) == observed.map(f32::to_bits)
        && time["secondsScaled"]
            .as_f64()
            .is_some_and(|seconds| (seconds as f32).to_bits() == expected[1].to_bits())
        && time["freezeSequence"]
            .as_u64()
            .is_some_and(|generation| generation > 0)
        && time["freezeSequence"] == clock["freezeSequence"]
        && [
            "sequence",
            "requestId",
            "freezeSequence",
            "ticks",
            "realTimerControlled",
        ]
        .iter()
        .all(|key| !copy[*key].is_null() && copy[*key] == clock[*key])
        && copy["realTimerControlled"].as_bool().is_some()
        && copy["partial"]
            .as_f64()
            .is_some_and(|value| (0. ..1.).contains(&value))
        && copy["ticksPerSecond"]
            .as_f64()
            .is_some_and(|value| (1. ..=1000.).contains(&value))
        && ["partial", "ticksPerSecond"].iter().all(|key| {
            copy[*key]
                .as_f64()
                .zip(clock[*key].as_f64())
                .is_some_and(|(a, b)| {
                    a.is_finite() && b.is_finite() && (a as f32).to_bits() == (b as f32).to_bits()
                })
        })
}
fn world_work(world: &Value) -> Option<&Value> {
    let work = &world["controls"]["worldWork"];
    (work["schema"].as_u64() == Some(1)
        && work["session"] == world["session"]
        && work["ticks"].as_u64().is_some()
        && work["ticks"] == world["ticks"]
        && work["worldSequence"]
            .as_u64()
            .is_some_and(|sequence| sequence > 0)
        && work["worldSequence"] == world["sequence"]
        && work["matchingEndpoints"].as_bool() == Some(true))
    .then_some(work)
}
fn task_activity(value: &Value) -> bool {
    value["taskActivityObserved"].as_bool() == Some(true)
        && ["taskActivitySequence", "taskActivityGeneration"]
            .iter()
            .all(|key| value[*key].as_u64().is_some_and(|value| value > 0))
}
fn subchunk_idle(world: &Value) -> bool {
    let Some(work) = world_work(world) else {
        return false;
    };
    let insertion = &work["subchunkInsertion"];
    task_activity(work)
        && insertion["observedIdle"].as_bool() == Some(true)
        && [
            "pendingChanges",
            "queuedTasks",
            "inFlightTasks",
            "taskGroupState",
        ]
        .iter()
        .all(|name| insertion[*name].as_u64() == Some(0))
        && insertion["leaving"].as_bool() == Some(false)
}
fn network_idle(world: &Value) -> bool {
    let Some(work) = world_work(world) else {
        return false;
    };
    let network = &work["networkChunkIO"];
    let insertion = &network["legacyInsertion"];
    task_activity(network)
        && network["observedIdle"].as_bool() == Some(true)
        && [
            "pendingChunks",
            "pausedCallbacks",
            "inFlightTasks",
            "taskGroupState",
        ]
        .iter()
        .all(|name| network[*name].as_u64() == Some(0))
        && insertion["observed"].as_bool() == Some(true)
        && insertion["observedIdle"].as_bool() == Some(true)
        && ["outstandingChunks", "readyChunks"]
            .iter()
            .all(|name| insertion[*name].as_u64() == Some(0))
        && insertion["issuedChunks"]
            .as_u64()
            .zip(insertion["consumedChunks"].as_u64())
            .is_some_and(|(issued, consumed)| issued == consumed)
}
fn renderer_idle(world: &Value) -> bool {
    let Some(work) = world_work(world) else {
        return false;
    };
    let renderer = &work["rendererTasks"];
    let changes = &renderer["chunkChanges"];
    renderer["observed"].as_bool() == Some(true)
        && renderer["observedIdle"].as_bool() == Some(true)
        && ["background", "mainThread"].iter().all(|name| {
            let group = &renderer[*name];
            task_activity(group)
                && group["observedIdle"].as_bool() == Some(true)
                && group["inFlightTasks"].as_u64() == Some(0)
                && group["taskGroupState"].as_u64() == Some(0)
        })
        && changes["observed"].as_bool() == Some(true)
        && changes["observedIdle"].as_bool() == Some(true)
        && changes["dimensionId"]
            .as_i64()
            .is_some_and(|value| i32::try_from(value).is_ok())
        && changes["coordinatorsOwned"]
            .as_u64()
            .is_some_and(|count| (1..=8).contains(&count))
        && [
            "visibilityDirtyChunks",
            "actorNotificationChunks",
            "blockEntityNotificationChunks",
        ]
        .iter()
        .all(|name| changes[*name].as_u64() == Some(0))
        && shared_chunk_idle(&changes["sharedChunkWork"])
}
fn shared_chunk_idle(value: &Value) -> bool {
    let [
        Some(keys),
        Some(live),
        Some(expired),
        Some(empty),
        Some(dirty),
        Some(ready),
    ] = [
        "chunkKeys",
        "liveChunks",
        "expiredWeakChunks",
        "emptyWeakChunks",
        "visibilityDirtyChunks",
        "visibilityReadyChunks",
    ]
    .map(|key| value[key].as_u64())
    else {
        return false;
    };
    value["observed"].as_bool() == Some(true)
        && value["observedIdle"].as_bool() == Some(true)
        && keys <= 32_768
        && live
            .checked_add(expired)
            .and_then(|count| count.checked_add(empty))
            == Some(keys)
        && dirty.checked_add(ready) == Some(live)
        && value["buildingOrSortingChunks"].as_u64() == Some(0)
        && value["visibilityBuildingChunks"].as_u64() == Some(0)
}
fn frame_input(world: &Value) -> bool {
    let Some(work) = world_work(world) else {
        return false;
    };
    let input = &work["frameInput"];
    let [
        Some(views),
        Some(index),
        Some(shadows),
        Some(other),
        Some(chunks),
        Some(vertices),
    ] = [
        "viewCount",
        "primaryViewIndex",
        "shadowViews",
        "otherClientViews",
        "chunkQueueCount",
        "maximumChunkVertexCount",
    ]
    .map(|key| input[key].as_u64())
    else {
        return false;
    };
    let Some(view_camera) = input["viewCamera"]
        .as_array()
        .filter(|values| values.len() == 6)
    else {
        return false;
    };
    let Some(renderer_camera) = input["rendererCamera"]
        .as_array()
        .filter(|values| values.len() == 6)
    else {
        return false;
    };
    let Some(layers) = input["layerQueueCounts"]
        .as_array()
        .filter(|values| values.len() == 51)
    else {
        return false;
    };
    let Some(total) = layers.iter().try_fold(0u64, |sum, value| {
        value
            .as_u64()
            .filter(|count| *count <= 1_000_000)
            .and_then(|count| sum.checked_add(count))
    }) else {
        return false;
    };
    [
        "observed",
        "worldArgumentObserved",
        "primaryViewMatched",
        "queueHeadersObserved",
        "queuePayloadObserved",
    ]
    .iter()
    .all(|name| input[*name].as_bool() == Some(true))
        && input["primaryViewCount"].as_u64() == Some(1)
        && (1..=16).contains(&views)
        && index < views
        && shadows
            .checked_add(other)
            .and_then(|count| count.checked_add(1))
            == Some(views)
        && chunks <= 1_000_000
        && vertices <= u64::from(u32::MAX)
        && input["layerEntryCount"].as_u64() == Some(total)
        && frame_payload(input, chunks, total, layers)
        && view_camera
            .iter()
            .zip(renderer_camera)
            .all(|(view, renderer)| {
                view.as_f64().zip(renderer.as_f64()).is_some_and(|(a, b)| {
                    a.is_finite() && b.is_finite() && a.abs() <= 32_000_000. && a == b
                })
            })
}

fn frame_payload(input: &Value, chunks: u64, total: u64, layers: &[Value]) -> bool {
    let payload = &input["chunkPayload"];
    if chunks > 32_768
        || total > 262_144
        || payload["chunkEntries"].as_u64() != Some(chunks)
        || payload["layerEntries"].as_u64() != Some(total)
        || payload["layerQueueCounts"].as_array().map(Vec::as_slice) != Some(layers)
        || ![
            "observed",
            "rangeArithmeticValid",
            "meshIndexOwnersObserved",
            "directAvailabilityObserved",
            "ordinaryAvailabilityObserved",
        ]
        .iter()
        .all(|key| payload[*key].as_bool() == Some(true))
    {
        return false;
    }
    let variants = |key: &str, length: usize, sum: u64| -> Option<Vec<u64>> {
        let values = payload[key]
            .as_array()
            .filter(|values| values.len() == length)?;
        let counts = values
            .iter()
            .map(Value::as_u64)
            .collect::<Option<Vec<_>>>()?;
        if counts
            .iter()
            .try_fold(0u64, |total, count| total.checked_add(*count))
            != Some(sum)
        {
            return None;
        }
        Some(counts)
    };
    let Some(indices) = variants("indexVariantCounts", 3, chunks) else {
        return false;
    };
    let Some(meshes) = variants("meshVariantCounts", 3, chunks) else {
        return false;
    };
    let Some(direct_indices) = variants("directIndexAvailabilityCounts", 3, indices[2]) else {
        return false;
    };
    let Some(direct_vertices) = variants("directVertexAvailabilityCounts", 3, meshes[2]) else {
        return false;
    };
    let Some(ordinary_indices) = variants("ordinaryIndexAvailabilityCounts", 3, indices[1]) else {
        return false;
    };
    let Some(ordinary_meshes) = variants("ordinaryMeshAvailabilityCounts", 3, meshes[1]) else {
        return false;
    };
    if variants("lodCounts", 2, total).is_none() {
        return false;
    }
    let [Some(direct_resources), Some(split)] =
        ["uniqueDirectResources", "splitStreamChunkEntries"].map(|key| payload[key].as_u64())
    else {
        return false;
    };
    let present_direct = (indices[2] - direct_indices[0]) + (meshes[2] - direct_vertices[0]);
    let ordinary_mesh_count = meshes[1] - ordinary_meshes[0];
    let ordinary_index_count = indices[1] - ordinary_indices[0];
    let present_ordinary = ordinary_mesh_count + ordinary_index_count;
    let [
        Some(ordinary_resources),
        Some(client_refs),
        Some(client_owners),
        Some(cpu_positions),
        Some(cpu_indices),
        Some(attributes),
    ] = [
        "uniqueOrdinaryResources",
        "clientResourceReferences",
        "uniqueClientResourceOwners",
        "ordinaryCpuPositionFallbackEntries",
        "ordinaryCpuIndexFallbackEntries",
        "ordinaryAttributeBufferEntries",
    ]
    .map(|key| payload[key].as_u64())
    else {
        return false;
    };
    let [
        Some(references),
        Some(owners),
        Some(positions),
        Some(referenced),
        Some(unreferenced),
        Some(masked),
        Some(fallback),
        Some(potential),
    ] = [
        "sharedResourceReferences",
        "uniqueSharedOwners",
        "uniqueChunkPositions",
        "referencedChunks",
        "unreferencedChunks",
        "visibleMaskEntries",
        "fallbackToUnsortedEntries",
        "potentialDrawEntries",
    ]
    .map(|key| payload[key].as_u64())
    else {
        return false;
    };
    references <= (chunks - indices[0]) + (chunks - meshes[0])
        && owners <= references
        && present_direct + present_ordinary <= references
        && direct_resources <= present_direct
        && (present_direct == 0) == (direct_resources == 0)
        && ordinary_resources <= present_ordinary
        && (present_ordinary == 0) == (ordinary_resources == 0)
        && client_refs <= ordinary_mesh_count * 3 + ordinary_index_count
        && client_owners <= client_refs
        && (client_refs == 0) == (client_owners == 0)
        && ordinary_indices[2] <= client_refs
        && cpu_positions <= ordinary_mesh_count
        && cpu_indices <= ordinary_mesh_count
        && attributes <= ordinary_mesh_count
        && split <= meshes[2] - direct_vertices[0]
        && (references == 0) == (owners == 0)
        && positions <= chunks
        && (chunks == 0) == (positions == 0)
        && referenced.checked_add(unreferenced) == Some(chunks)
        && referenced <= total
        && (total == 0) == (referenced == 0)
        && potential <= masked
        && masked <= total
        && fallback <= total
        && (potential == 0 || meshes[0] < chunks)
        && payload["missingPotentialMeshEntries"].as_u64() == Some(0)
        && payload["missingPotentialMaterialEntries"].as_u64() == Some(0)
        && payload["unavailablePotentialDirectMeshEntries"].as_u64() == Some(0)
        && payload["unavailablePotentialDirectIndexEntries"].as_u64() == Some(0)
        && payload["unavailablePotentialOrdinaryMeshEntries"].as_u64() == Some(0)
        && payload["unavailablePotentialOrdinaryIndexEntries"].as_u64() == Some(0)
        && frame_materials(payload, total)
}

fn frame_materials(payload: &Value, total: u64) -> bool {
    if ![
        "materialReferencesObserved",
        "materialSharedOwnersObserved",
        "renderMaterialPointersObserved",
    ]
    .iter()
    .all(|key| payload[*key].as_bool() == Some(true))
    {
        return false;
    }
    let [
        Some(nulls),
        Some(entries),
        Some(empty),
        Some(live),
        Some(owned_null),
        Some(references),
        Some(unique),
        Some(infos),
        Some(owners),
    ] = [
        "nullMaterialEntries",
        "materialReferenceEntries",
        "emptyMaterialReferenceEntries",
        "liveMaterialReferenceEntries",
        "ownedNullMaterialReferenceEntries",
        "materialSharedReferences",
        "uniqueMaterialReferences",
        "uniqueMaterialInfos",
        "uniqueMaterialOwners",
    ]
    .map(|key| payload[key].as_u64())
    else {
        return false;
    };
    let [Some(render), Some(unresolved), Some(unique_render)] = [
        "renderMaterialReferenceEntries",
        "nullRenderMaterialReferenceEntries",
        "uniqueRenderMaterialPointers",
    ]
    .map(|key| payload[key].as_u64()) else {
        return false;
    };
    nulls.checked_add(entries) == Some(total)
        && empty.checked_add(live) == Some(entries)
        && owned_null <= empty
        && live.checked_add(owned_null) == Some(references)
        && unique <= 16_384
        && unique <= entries
        && (entries == 0) == (unique == 0)
        && infos <= live
        && infos <= unique
        && (live == 0) == (infos == 0)
        && owners <= references
        && owners <= unique
        && (references == 0) == (owners == 0)
        && render.checked_add(unresolved) == Some(live)
        && unique_render <= render
        && unique_render <= infos
        && (render == 0) == (unique_render == 0)
        && payload["missingPotentialMaterialInfoEntries"].as_u64() == Some(0)
        && payload["missingPotentialRenderMaterialEntries"].as_u64() == Some(0)
}

/// Exact expected world-layer identity. A burst captures fresh presentations
/// of this same native sample; it does not advance ticks or replay packets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedSample {
    pub min_sequence: u64,
    pub ticks: u64,
    pub partial: f32,
    pub clock_ms: u64,
    pub camera: [f64; 6],
    #[serde(default = "one_frame")]
    pub frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<ControlFence>,
}
fn one_frame() -> u32 {
    1
}

impl IndexedSample {
    /// Require a recent observation of the expected controlled world, then
    /// capture a later world sequence at that same tick/fraction/camera. Waiting
    /// for native control propagation is the caller's responsibility.
    pub fn from_observation(status: &Value, controls: ControlFence) -> Result<Self> {
        let world = &status["observedWorld"]["world"];
        if !status["observedAgeMs"]
            .as_u64()
            .is_some_and(|age| age < 500)
            || !controls.matches_world(world)
            || world["schema"].as_u64() != Some(1)
            || !world["session"].as_str().is_some_and(token)
            || status["observedWorld"]["gpu"]["sequence"]
                .as_u64()
                .is_none()
        {
            return Err(Error::Other("the requested native clock/camera sample has not reached a recent presented world layer".into()));
        }
        let field = |name: &str| {
            world[name]
                .as_u64()
                .ok_or_else(|| Error::Other(format!("native world observation is missing {name}")))
        };
        let min_sequence = field("sequence")?
            .checked_add(1)
            .filter(|s| *s > 1)
            .ok_or_else(|| Error::Other("native world sequence limit reached".into()))?;
        let camera: [f64; 6] = serde_json::from_value(world["camera"].clone())?;
        let partial = world["partial"]
            .as_f64()
            .map(|p| p as f32)
            .filter(|p| p.is_finite() && (0.0..1.0).contains(p))
            .ok_or_else(|| Error::Other("invalid native world fraction".into()))?;
        if field("ticks")? > 1_000_000_000_000
            || camera
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 32_000_000.0)
            || !(1.0..=179.0).contains(&camera[5])
        {
            return Err(Error::Other(
                "invalid native world tick/camera observation".into(),
            ));
        }
        Ok(Self {
            min_sequence,
            ticks: field("ticks")?,
            partial,
            clock_ms: field("clockMs")?,
            camera,
            frames: 1,
            controls: Some(controls),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
// One short-lived control message at a time; boxing the sample buys nothing.
#[allow(clippy::large_enum_variant)]
pub enum CaptureControl {
    /// Own the slot across replay teardown/reconnect without opening an encoder.
    Reserve {
        lease_ms: u32,
    },
    Renew {
        lease_ms: u32,
    },
    Activate {
        session: String,
        #[serde(flatten)]
        options: IndexedOptions,
    },
    Start {
        session: String,
        #[serde(flatten)]
        options: IndexedOptions,
    },
    Sample {
        index: u32,
        #[serde(flatten)]
        sample: IndexedSample,
    },
    Stop {},
    /// Host-side observation of a recent tagged presentation, without readback.
    Observe {},
    /// Host-side wait for successful encoder exit, not a native admission.
    Finish {
        frames: u32,
    },
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// Control an already-loaded native adapter. Never launches Minecraft or
/// selects an account. Start/stop wait for admission; sample waits for matching
/// presented pixels to enter the encoder queue; finish waits for encoder exit.
/// Capture IDs scope later commands, and each publication gets a unique receipt
/// ID. The caller retains responsibility for clock/camera leases and cleanup.
#[cfg(windows)]
pub fn control(
    edition: Edition,
    bundle: &Path,
    capture_id: &str,
    mut command: CaptureControl,
) -> Result<Value> {
    use serde_json::json;
    use std::time::{Duration, Instant};
    if !token(capture_id) {
        return Err(Error::Other("invalid native capture identity".into()));
    }
    match &mut command {
        CaptureControl::Reserve { lease_ms } | CaptureControl::Renew { lease_ms }
            if !(5000..=120000).contains(lease_ms) =>
        {
            return Err(Error::Other(
                "capture reservation lease must be 5–120 seconds".into(),
            ));
        }
        CaptureControl::Start { session, options }
        | CaptureControl::Activate { session, options } => {
            if !token(session)
                || !(1..=60).contains(&options.fps)
                || options.frames == 0
                || options.frames > options.fps * 7200 + 1
                || !(1..=8).contains(&options.settle_frames)
            {
                return Err(Error::Other("indexed capture needs a live session, FPS 1–60, at least one frame over a replay span up to two hours, and 1–8 settling frames".into()));
            }
            options.ffmpeg = options.ffmpeg.canonicalize().at(&options.ffmpeg)?;
            options.output = std::path::absolute(&options.output).at(&options.output)?;
            if !options.ffmpeg.is_file()
                || options.output.exists()
                || options.output.with_extension("frames.jsonl").exists()
                || !options.output.parent().is_some_and(Path::is_dir)
                || options
                    .output
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("mp4"))
            {
                return Err(Error::Other("indexed capture needs an encoder executable and a new .mp4/manifest in an existing directory".into()));
            }
        }
        CaptureControl::Sample { index, sample } => {
            if sample
                .controls
                .as_ref()
                .and_then(|f| f.ticks_per_second)
                .is_some_and(|rate| !rate.is_finite() || !(1.0..=1000.0).contains(&rate))
            {
                return Err(Error::Other(
                    "capture fence has an invalid native tick rate".into(),
                ));
            }
            if let Some(fence) = &sample.controls
                && (!(1..=1_000_000_000).contains(&fence.clock.sequence)
                    || !token(&fence.clock.request_id)
                    || !fence.camera.valid())
            {
                return Err(Error::Other(
                    "capture fence needs a valid clock identity and camera requirement".into(),
                ));
            }
            if sample
                .controls
                .as_ref()
                .and_then(|f| f.packet_dispatch.as_ref())
                .is_some_and(|p| !p.valid())
            {
                return Err(Error::Other(
                    "capture fence has an invalid packet dispatch identity".into(),
                ));
            }
            if sample
                .controls
                .as_ref()
                .is_some_and(|f| !f.cosmetic_clock_valid())
            {
                return Err(Error::Other(
                    "capture fence has an invalid replay cosmetic clock".into(),
                ));
            }
            if sample.controls.as_ref().is_some_and(|f| {
                f.appearance
                    .as_ref()
                    .is_some_and(|a| !a.valid() || f.cosmetic_clock_ms != Some(a.time_ms))
            }) {
                return Err(Error::Other(
                    "capture fence has an invalid recorded appearance identity".into(),
                ));
            }
            if !(1..=120).contains(&sample.frames)
                || index.checked_add(sample.frames).is_none()
                || sample.min_sequence == 0
                || sample.ticks > 1_000_000_000_000
                || !sample.partial.is_finite()
                || !(0.0..1.0).contains(&sample.partial)
                || sample
                    .camera
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 32_000_000.0)
                || !(1.0..=179.0).contains(&sample.camera[5])
            {
                return Err(Error::Other("indexed sample needs a fresh world sequence, valid tick/fraction/camera and 1–120 presentations".into()));
            }
        }
        CaptureControl::Finish { frames } if !(1..=432_000).contains(frames) => {
            return Err(Error::Other("invalid indexed capture frame count".into()));
        }
        _ => {}
    }
    let endpoint = Endpoint::attach(edition, bundle)?;
    if matches!(&command, CaptureControl::Stop {})
        && let Some(status) = endpoint.read()?
        && status["captureId"].as_str() == Some(capture_id)
        && matches!(
            status["state"].as_str(),
            Some("complete" | "error" | "cancelled")
        )
    {
        return Ok(status);
    }
    if let CaptureControl::Finish { frames } = &command {
        return endpoint.finish(capture_id, *frames);
    }
    if matches!(&command, CaptureControl::Observe {}) {
        return endpoint.observe(capture_id);
    }
    let mut value = serde_json::to_value(&command)?;
    if matches!(
        &command,
        CaptureControl::Start { .. } | CaptureControl::Activate { .. }
    ) {
        value["mode"] = json!("indexed");
    }
    let published = endpoint.publish(capture_id, value)?;
    let started = Instant::now();
    let result = (|| -> Result<Value> {
        while started.elapsed() < Duration::from_secs(20) {
            if let Some(status) = endpoint.read()? {
                let rejected = &status["requestRejected"];
                if rejected["captureId"].as_str() == Some(capture_id)
                    && rejected["requestId"].as_str() == Some(published.request_id.as_str())
                {
                    return Err(Error::Other(
                        rejected["error"]
                            .as_str()
                            .unwrap_or("capture command rejected")
                            .into(),
                    ));
                }
                if status["captureId"].as_str() == Some(capture_id) {
                    let receipt = &status["commandReceipt"];
                    let accepted = receipt["requestId"].as_str()
                        == Some(published.request_id.as_str())
                        && receipt["captureId"].as_str() == Some(capture_id)
                        && receipt["accepted"].as_bool() == Some(true);
                    // Stopping an incomplete indexed clip intentionally leaves
                    // an error outcome for that clip. Admission still means
                    // this owned stop succeeded; it is not a completed video.
                    if accepted && matches!(&command, CaptureControl::Stop {}) {
                        return Ok(status);
                    }
                    if status["state"].as_str() == Some("error")
                        && (accepted
                            || !matches!(
                                &command,
                                CaptureControl::Start { .. } | CaptureControl::Activate { .. }
                            ))
                    {
                        return Err(Error::Other(
                            status["message"]
                                .as_str()
                                .unwrap_or("native capture failed")
                                .into(),
                        ));
                    }
                    if accepted {
                        match &command {
                            CaptureControl::Sample { index, sample } => {
                                let end = u64::from(*index) + u64::from(sample.frames);
                                let frame = &status["receipt"];
                                if status["frames"].as_u64().is_some_and(|n| n >= end)
                                    && frame["index"].as_u64() == Some(end - 1)
                                    && frame["captureId"].as_str() == Some(capture_id)
                                    && frame["requestId"].as_str()
                                        == Some(published.request_id.as_str())
                                {
                                    return Ok(status);
                                }
                            }
                            _ => return Ok(status),
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(Error::Other(format!(
            "native capture did not acknowledge this request; inspect {}",
            endpoint.report.display()
        )))
    })();
    if result.is_err() {
        published.withdraw();
    }
    result
}

#[cfg(not(windows))]
pub fn control(_: Edition, _: &Path, _: &str, _: CaptureControl) -> Result<Value> {
    Err(Error::Other(
        "native indexed capture requires Windows".into(),
    ))
}

#[cfg(windows)]
struct Endpoint {
    bundle: PathBuf,
    request: PathBuf,
    report: PathBuf,
}
#[cfg(windows)]
impl Endpoint {
    fn attach(edition: Edition, bundle: &Path) -> Result<Self> {
        let bundle = super::client::validate_bundle(bundle)?;
        let installed =
            super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
        let pid = super::package::process_id(edition.package_family(), &installed.path)
            .ok_or_else(|| {
                Error::Other(
                    "Bedrock must already be running with the native adapter loaded".into(),
                )
            })?;
        let marker = bundle.join(format!("arctic-bedrock-{pid}.ready"));
        if !std::fs::read_to_string(&marker).is_ok_and(|v| v.trim() == "native-preview") {
            return Err(Error::Other(
                "this native bundle is not loaded into the running game".into(),
            ));
        }
        Ok(Self {
            request: bundle.join(format!("arctic-bedrock-{pid}.capture-request.json")),
            report: bundle.join(format!("arctic-bedrock-{pid}.capture.json")),
            bundle,
        })
    }
    fn publish(&self, capture_id: &str, mut command: Value) -> Result<Published> {
        use serde_json::json;
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let request_id = format!(
            "frame-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        command["captureId"] = json!(capture_id);
        command["requestId"] = json!(request_id);
        let bytes = serde_json::to_vec(&command)?;
        if bytes.len() > 16 * 1024 {
            return Err(Error::Other("native capture command exceeds 16 KiB".into()));
        }
        let temporary = self.bundle.join(format!(".{request_id}.tmp"));
        let result = (|| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .at(&temporary)?;
            file.write_all(&bytes).at(&temporary)?;
            drop(file);
            std::fs::hard_link(&temporary, &self.request).at(&self.request)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&temporary);
        result?;
        Ok(Published {
            request: self.request.clone(),
            request_id,
            bytes,
        })
    }
    fn read(&self) -> Result<Option<Value>> {
        use std::io::Read;
        let mut bytes = Vec::new();
        let file = match std::fs::File::open(&self.report) {
            Ok(file) => file,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(Error::io(&self.report, error)),
        };
        file.take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .at(&self.report)?;
        if bytes.len() > 256 * 1024 {
            return Err(Error::Other("native capture report exceeds 256 KiB".into()));
        }
        Ok(serde_json::from_slice(&bytes).ok())
    }
    fn finish(&self, capture_id: &str, frames: u32) -> Result<Value> {
        use std::time::{Duration, Instant};
        let started = Instant::now();
        // Native encoding has a 60-second idle watchdog. Let its final codec
        // result and file flush arrive before the host declares completion lost.
        while started.elapsed() < Duration::from_secs(90) {
            if let Some(status) = self.read()?
                && status["captureId"].as_str() == Some(capture_id)
            {
                match status["state"].as_str() {
                    Some("complete") => {
                        let output = status["output"].as_str().map(Path::new);
                        if status["mode"].as_str() != Some("indexed")
                            || status["frames"].as_u64() != Some(u64::from(frames))
                            || !output.is_some_and(|p| {
                                p.is_absolute()
                                    && std::fs::metadata(p)
                                        .is_ok_and(|m| m.is_file() && m.len() > 0)
                                    && std::fs::metadata(p.with_extension("frames.jsonl"))
                                        .is_ok_and(|m| m.is_file() && m.len() > 0)
                            })
                        {
                            return Err(Error::Other("indexed encoder completed without the expected frames/output/manifest".into()));
                        }
                        return Ok(status);
                    }
                    Some("error") => {
                        return Err(Error::Other(
                            status["message"]
                                .as_str()
                                .unwrap_or("indexed encoder failed")
                                .into(),
                        ));
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(Error::Other(
            "native indexed encoder did not finish within 90 seconds".into(),
        ))
    }
    fn observe(&self, capture_id: &str) -> Result<Value> {
        use std::time::{Duration, Instant};
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) {
            if let Some(status) = self.read()?
                && status["captureId"].as_str() == Some(capture_id)
            {
                if matches!(
                    status["state"].as_str(),
                    Some("complete" | "error" | "finishing")
                ) {
                    return Err(Error::Other(
                        "indexed capture is no longer accepting frame samples".into(),
                    ));
                }
                let recent = std::fs::metadata(&self.report)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age < Duration::from_secs(2));
                if recent
                    && status["observedAgeMs"]
                        .as_u64()
                        .is_some_and(|age| age < 500)
                    && status["observedWorld"]["world"]["sequence"]
                        .as_u64()
                        .is_some()
                    && status["observedWorld"]["gpu"]["sequence"]
                        .as_u64()
                        .is_some()
                {
                    return Ok(status);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(Error::Other(
            "no recent tagged native presentation arrived for this capture".into(),
        ))
    }
}
#[cfg(windows)]
struct Published {
    request: PathBuf,
    request_id: String,
    bytes: Vec<u8>,
}
#[cfg(windows)]
impl Published {
    fn withdraw(&self) {
        if std::fs::read(&self.request).is_ok_and(|data| data == self.bytes) {
            let _ = std::fs::remove_file(&self.request);
        }
    }
}
