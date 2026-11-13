//! Bounded real-time or explicitly indexed capture of presented pixels. Encoding runs outside the
//! render thread; no desktop screenshots or synthetic world frames are used.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
const MAX_FRAME_METADATA: usize = 64 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub ffmpeg: PathBuf,
    #[serde(default)]
    pub encoder: EncoderKind,
    pub output: PathBuf,
    pub fps: u32,
    pub frames: u32,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub clean_view: bool,
    #[serde(default = "three_frames")]
    pub settle_frames: u32,
}
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncoderKind {
    #[default]
    Ffmpeg,
    ArcticBuiltin,
}
impl EncoderKind {
    fn indexed_profile(self) -> &'static str {
        match self {
            Self::ArcticBuiltin => "mf_hardware_disabled_h264_v1",
            Self::Ffmpeg => "libx264_threads1_bitexact_v1",
        }
    }
}
fn three_frames() -> u32 {
    3
}
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Realtime,
    Indexed,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FrameTag {
    schema: u32,
    session: String,
    sequence: u64,
    clock_ms: u64,
    ticks: u64,
    partial: f32,
    camera: [f64; 6],
    clean_view: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    controls: Option<Value>,
    #[serde(skip)]
    pub(crate) gpu: Option<PresentInfo>,
    #[serde(skip)]
    pub(crate) presented_constants: Option<Value>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresentInfo {
    pub api: &'static str,
    pub sequence: u64,
    pub buffer_index: Option<u32>,
    pub buffer_count: Option<u32>,
}
impl FrameTag {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        // Bounded world-work/material summaries plus camera/clock receipts
        // exceed the old pose-only 4 KiB envelope. Reports remain separately
        // bounded by the controller's existing 256 KiB admission limit.
        if text.len() > MAX_FRAME_METADATA {
            return None;
        }
        let value: Self = serde_json::from_str(text).ok()?;
        (value.schema == 1
            && valid_session(&value.session)
            && value.ticks <= 1_000_000_000_000
            && value.partial.is_finite()
            && (0. ..1.).contains(&value.partial)
            && value
                .camera
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 32_000_000.)
            && (1. ..=179.).contains(&value.camera[5]))
        .then_some(value)
    }
}
fn valid_session(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ControlIdentity {
    sequence: u64,
    request_id: String,
}
impl ControlIdentity {
    fn valid(&self) -> bool {
        (1..=1_000_000_000).contains(&self.sequence) && valid_session(&self.request_id)
    }
    fn matches(&self, value: &Value) -> bool {
        value["sequence"].as_u64() == Some(self.sequence)
            && value["requestId"].as_str() == Some(self.request_id.as_str())
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ControlFence {
    clock: ControlIdentity,
    camera: CameraFence,
    #[serde(default)]
    ticks_per_second: Option<f32>,
    #[serde(default)]
    packet_dispatch: Option<PacketIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cosmetic_clock_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    appearance: Option<AppearanceIdentity>,
    #[serde(default)]
    require_subchunk_idle: bool,
    #[serde(default)]
    require_network_idle: bool,
    #[serde(default)]
    require_renderer_idle: bool,
    #[serde(default)]
    require_frame_input: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    require_camera_time: bool,
    #[serde(default = "require_real_timer")]
    real_timer: bool,
}
fn require_real_timer() -> bool {
    true
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppearanceIdentity {
    manifest_hash: String,
    time_ms: u32,
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
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum CameraFence {
    Controlled(ControlIdentity),
    Presented(PresentedCamera),
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresentedCamera {
    session: String,
    pose: [f64; 6],
}
fn camera_value_matches(index: usize, actual: f64, expected: f64) -> bool {
    if !actual.is_finite() || !expected.is_finite() {
        return false;
    }
    if index < 3 {
        // live_frame serializes native f32 positions with shortest decimals;
        // recover those values instead of comparing decimal error in f64.
        // This also rejects a neighboring native position at small coordinates
        // that the previous fixed .001 tolerance could admit prematurely.
        (actual as f32) == (expected as f32)
    } else {
        let delta = if index == 3 || index == 4 {
            (actual - expected + 180.0).rem_euclid(360.0) - 180.0
        } else {
            actual - expected
        };
        delta.abs() <= 0.001
    }
}
impl PresentedCamera {
    fn valid(&self) -> bool {
        valid_session(&self.session)
            && self.pose.iter().all(|v| v.is_finite())
            && self.pose[..3].iter().all(|v| v.abs() <= 30_000_000.0)
            && self.pose[3].abs() <= 360_000.0
            && self.pose[4].abs() <= 90.0
            && (1.0..=179.0).contains(&self.pose[5])
    }
    fn matches(&self, tag: &FrameTag) -> bool {
        self.session == tag.session
            && tag
                .camera
                .iter()
                .zip(self.pose)
                .enumerate()
                .all(|(i, (actual, expected))| camera_value_matches(i, *actual, expected))
    }
}
impl CameraFence {
    fn valid(&self) -> bool {
        match self {
            Self::Controlled(id) => id.valid(),
            Self::Presented(camera) => camera.valid(),
        }
    }
    fn matches(&self, tag: &FrameTag) -> bool {
        match self {
            Self::Presented(camera) => camera.matches(tag),
            Self::Controlled(id) => {
                let Some(controls) = tag.controls.as_ref() else {
                    return false;
                };
                let camera = &controls["camera"];
                id.matches(camera)
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
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PacketIdentity {
    session: String,
    sequence: u64,
    request_id: String,
    marker: String,
    generation: u64,
    replay_sequence: u64,
    time_ms: u32,
    #[serde(default)]
    time_remainder: u32,
    #[serde(default = "unit_denominator")]
    time_denominator: u32,
    #[serde(default = "default_packet_stage")]
    stage: String,
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
        valid_session(&self.session)
            && matches!(
                self.stage.as_str(),
                "dispatch_returned" | "client_tick_completed"
            )
            && [self.sequence, self.generation, self.replay_sequence]
                .iter()
                .all(|s| (1..=1_000_000_000).contains(s))
            && valid_session(&self.request_id)
            && self.time_ms <= 86_400_000
            && (1..=1000).contains(&self.time_denominator)
            && self.time_remainder < self.time_denominator
            && (self.time_ms < 86_400_000 || self.time_remainder == 0)
            && self.marker.len() == 16
            && self.marker.starts_with("37c1")
            && self
                .marker
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
    fn matches(&self, tag: &FrameTag) -> bool {
        let Some(controls) = tag.controls.as_ref() else {
            return false;
        };
        let value = &controls["packetDispatch"];
        value["sequence"].as_u64() == Some(self.sequence)
            && value["requestId"].as_str() == Some(self.request_id.as_str())
            && tag.session == self.session
            && value["session"].as_str() == Some(self.session.as_str())
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
                    && value["dispatcherTick"].as_u64() == Some(tag.ticks)
            } else {
                value["completion"].as_str() == Some("client_tick")
                    && value["postClientTickCompleted"].as_bool() == Some(true)
                    && value["dispatcherTick"]
                        .as_u64()
                        .zip(value["clientTick"].as_u64())
                        .is_some_and(|(a, b)| b > a && tag.ticks >= b)
            }
    }
}
impl ControlFence {
    fn matches(&self, tag: &FrameTag) -> bool {
        let Some(controls) = &tag.controls else {
            return false;
        };
        let clock = &controls["clock"];
        self.clock.matches(clock)
            && self.camera.matches(tag)
            && self.ticks_per_second.is_none_or(|rate| {
                clock["ticksPerSecond"]
                    .as_f64()
                    .is_some_and(|actual| (actual as f32).to_bits() == rate.to_bits())
            })
            && self
                .packet_dispatch
                .as_ref()
                .is_none_or(|packet| packet.matches(tag))
            && self.cosmetic_clock_ms.is_none_or(|time| {
                controls["cosmeticClockMs"].as_u64() == Some(u64::from(time))
                    && self.packet_dispatch.as_ref().is_some_and(|packet| {
                        packet.fraction_matches(
                            controls,
                            "cosmeticClockRemainder",
                            "cosmeticClockDenominator",
                        )
                    })
            })
            && self
                .appearance
                .as_ref()
                .is_none_or(|appearance| appearance.matches(&controls["appearance"]))
            && (!self.require_subchunk_idle || subchunk_idle(tag))
            && (!self.require_network_idle || network_idle(tag))
            && (!self.require_renderer_idle || renderer_idle(tag))
            && (!self.require_frame_input || frame_input(tag))
            && (!self.require_camera_time || camera_time(tag))
            && clock["freezeSequence"].as_u64().is_some_and(|s| s > 0)
            && clock["ticks"].as_u64() == Some(tag.ticks)
            && clock["partial"]
                .as_f64()
                .is_some_and(|p| (p as f32).to_bits() == tag.partial.to_bits())
            && (!self.real_timer || clock["realTimerControlled"].as_bool() == Some(true))
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
}
fn camera_time(tag: &FrameTag) -> bool {
    let Some(controls) = &tag.controls else {
        return false;
    };
    let clock = &controls["clock"];
    let time = &controls["cameraTime"];
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
        && time["session"].as_str() == Some(tag.session.as_str())
        && time["worldSequence"].as_u64() == Some(tag.sequence)
        && tag.sequence > 0
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
fn world_work(tag: &FrameTag) -> Option<&Value> {
    let controls = tag.controls.as_ref()?;
    let work = &controls["worldWork"];
    (work["schema"].as_u64() == Some(1)
        && work["session"].as_str() == Some(tag.session.as_str())
        && work["ticks"].as_u64() == Some(tag.ticks)
        && work["worldSequence"].as_u64() == Some(tag.sequence)
        && tag.sequence > 0
        && work["matchingEndpoints"].as_bool() == Some(true))
    .then_some(work)
}
fn task_activity(value: &Value) -> bool {
    value["taskActivityObserved"].as_bool() == Some(true)
        && ["taskActivitySequence", "taskActivityGeneration"]
            .iter()
            .all(|key| value[*key].as_u64().is_some_and(|value| value > 0))
}
fn subchunk_idle(tag: &FrameTag) -> bool {
    let Some(work) = world_work(tag) else {
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
fn network_idle(tag: &FrameTag) -> bool {
    let Some(work) = world_work(tag) else {
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
fn renderer_idle(tag: &FrameTag) -> bool {
    let Some(work) = world_work(tag) else {
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
fn frame_input(tag: &FrameTag) -> bool {
    let Some(work) = world_work(tag) else {
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
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    index: u32,
    min_sequence: u64,
    ticks: u64,
    partial: f32,
    clock_ms: u64,
    camera: [f64; 6],
    #[serde(default = "one_frame")]
    frames: u32,
}
fn one_frame() -> u32 {
    1
}
impl Sample {
    fn matches(&self, tag: &FrameTag, options: &Options, last: Option<u64>) -> bool {
        options.session.as_deref() == Some(&tag.session)
            && tag.sequence >= self.min_sequence
            && last.is_none_or(|s| tag.sequence > s)
            && tag.ticks == self.ticks
            && tag.clock_ms == self.clock_ms
            && tag.partial.to_bits() == self.partial.to_bits()
            && (!options.clean_view || tag.clean_view)
            && self
                .camera
                .iter()
                .zip(tag.camera)
                .enumerate()
                .all(|(i, (a, b))| camera_value_matches(i, *a, b))
    }
}

impl Options {
    fn validate(&self) -> Result<(), String> {
        if !self.ffmpeg.is_absolute()
            || !self.ffmpeg.is_file()
            || !self.output.is_absolute()
            || self
                .output
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("mp4"))
            || !self.output.parent().is_some_and(Path::is_dir)
            || self.output.exists()
            || !(1..=60).contains(&self.fps)
            || self.frames == 0
            || self.frames > self.fps * 7200 + u32::from(self.mode == Mode::Indexed)
            || !(1..=8).contains(&self.settle_frames)
            || self.mode == Mode::Indexed && !self.session.as_deref().is_some_and(valid_session)
            || self.mode == Mode::Realtime && self.session.is_some()
            || self.mode == Mode::Indexed && self.output.with_extension("frames.jsonl").exists()
        {
            return Err("capture needs an absolute encoder executable, a new absolute .mp4, FPS 1–60 and 1 frame–2 hours".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Reserve {
        lease_ms: u32,
    },
    Renew {
        lease_ms: u32,
    },
    Activate {
        #[serde(flatten)]
        options: Options,
    },
    Start {
        #[serde(flatten)]
        options: Options,
    },
    Stop {},
    Sample {
        #[serde(flatten)]
        sample: Sample,
    },
}

struct Encoder {
    tx: SyncSender<FrameBatch>,
    result: Receiver<Result<u32, String>>,
}
struct FrameBatch {
    pixels: Arc<[u8]>,
    repeat: u32,
    metadata: Option<Value>,
}

struct Session {
    options: Options,
    encoder: Option<Encoder>,
    started: Instant,
    clock: Option<Instant>,
    dimensions: Option<(u32, u32)>,
    sent: u32,
    last: Option<Arc<[u8]>>,
    error: Option<String>,
    finishing: bool,
    pending: Option<(Sample, Instant)>,
    last_sequence: Option<u64>,
    receipt: Option<Value>,
    settling: Option<Settling>,
}
struct Settling {
    pixels: Vec<u8>,
    frames: u32,
    buffer_count: Option<u32>,
    buffer_mask: u32,
}
impl Session {
    fn settle(
        &mut self,
        pixels: Vec<u8>,
        gpu: Option<&PresentInfo>,
    ) -> Result<Option<(Vec<u8>, Value)>, String> {
        let coverage = match gpu.filter(|g| g.api == "dx12") {
            Some(g) => {
                let count = g
                    .buffer_count
                    .filter(|c| (1..=16).contains(c))
                    .ok_or("settled capture requires a valid DX12 buffer count")?;
                let index = g
                    .buffer_index
                    .filter(|i| *i < count)
                    .ok_or("settled capture requires a valid DX12 buffer index")?;
                Some((count, 1u32 << index))
            }
            None => None,
        };
        let count = coverage.map(|v| v.0);
        let mask = coverage.map_or(0, |v| v.1);
        if let Some(previous) = self
            .settling
            .as_mut()
            .filter(|s| s.buffer_count == count && s.pixels == pixels)
        {
            previous.frames = previous.frames.saturating_add(1);
            previous.buffer_mask |= mask;
            let all_buffers = count.is_none_or(|c| previous.buffer_mask == (1u32 << c) - 1);
            if previous.frames >= self.options.settle_frames && all_buffers {
                let proof = json!({"frames":previous.frames,"bufferCount":count,"bufferMask":previous.buffer_mask});
                self.settling = None;
                return Ok(Some((pixels, proof)));
            }
        } else {
            self.settling = Some(Settling {
                pixels,
                frames: 1,
                buffer_count: count,
                buffer_mask: mask,
            });
        }
        Ok(None)
    }
}

struct Manager {
    session: Option<Session>,
    reservation: Option<(Instant, Duration)>,
    status: Value,
    rejected: Option<Value>,
    capture_id: Option<String>,
    command_receipt: Option<Value>,
    observed: Option<(Instant, Value)>,
    sample_controls: Option<ControlFence>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            session: None,
            reservation: None,
            status: json!({"state":"idle","frames":0}),
            rejected: None,
            capture_id: None,
            command_receipt: None,
            observed: None,
            sample_controls: None,
        }
    }
}
static MANAGER: OnceLock<Mutex<Manager>> = OnceLock::new();
fn manager() -> &'static Mutex<Manager> {
    MANAGER.get_or_init(|| Mutex::new(Manager::default()))
}

pub fn command(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err("capture request exceeds 16 KiB or is empty".into());
    }
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let identity = json!({"captureId":value["captureId"],"requestId":value["requestId"],"output":value["output"],"op":value["op"]});
    let mut manager = manager().lock().map_err(|e| e.to_string())?;
    expire_reservation(&mut manager);
    let result = (|| -> Result<(), String> {
        let object = value
            .as_object_mut()
            .ok_or("capture command must be an object")?;
        let capture_id = object.remove("captureId");
        let request_id = object.remove("requestId");
        let controls = object
            .remove("controls")
            .map(serde_json::from_value::<ControlFence>)
            .transpose()
            .map_err(|e| e.to_string())?;
        let token = |v: Option<Value>| -> Result<Option<String>, String> {
            v.map(|v| {
                v.as_str()
                    .filter(|s| valid_session(s))
                    .map(str::to_owned)
                    .ok_or_else(|| "invalid capture/request identity".to_string())
            })
            .transpose()
        };
        let capture_id = token(capture_id)?;
        let request_id = token(request_id)?;
        if capture_id.is_some() != request_id.is_some() {
            return Err("scoped capture commands need captureId and requestId".into());
        }
        let command: Command = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let start = matches!(&command, Command::Start { .. } | Command::Activate { .. });
        let reserve = matches!(&command, Command::Reserve { .. });
        let sample = matches!(&command, Command::Sample { .. });
        if reserve && capture_id.is_none() {
            return Err("capture reservation requires scoped ownership".into());
        }
        if controls.as_ref().is_some_and(|f| {
            !f.clock.valid()
                || !f.camera.valid()
                || f.ticks_per_second
                    .is_some_and(|rate| !rate.is_finite() || !(1.0..=1000.0).contains(&rate))
                || f.packet_dispatch.as_ref().is_some_and(|p| !p.valid())
                || !f.cosmetic_clock_valid()
                || f.appearance
                    .as_ref()
                    .is_some_and(|a| !a.valid() || f.cosmetic_clock_ms != Some(a.time_ms))
        }) || controls.is_some() && (!sample || capture_id.is_none())
        {
            return Err("capture control fences need a scoped sample, valid clock identity and camera requirement".into());
        }
        if manager.reservation.is_some() && manager.capture_id != capture_id {
            return Err("capture slot is reserved by another owner".into());
        }
        if !start
            && !reserve
            && (manager.session.is_some() || capture_id.is_some())
            && manager.capture_id != capture_id
        {
            return Err("capture command belongs to another capture".into());
        }
        if !start
            && !reserve
            && capture_id.is_some()
            && manager.session.is_none()
            && manager.reservation.is_none()
        {
            return Err("owned capture is no longer running or reserved".into());
        }
        command_manager(&mut manager, command)?;
        if start || reserve {
            manager.capture_id = capture_id.clone();
        }
        if start || sample {
            manager.sample_controls = controls;
        }
        manager.command_receipt = Some(json!({"captureId":capture_id,"requestId":request_id,
            "op":identity["op"],"accepted":true}));
        Ok(())
    })();
    if let Err(error) = &result {
        let mut rejected = identity;
        rejected["error"] = json!(error);
        manager.rejected = Some(rejected);
    }
    result
}
fn command_manager(manager: &mut Manager, command: Command) -> Result<(), String> {
    if matches!(&command, Command::Activate { .. }) && manager.reservation.is_none() {
        return Err("indexed activation requires its unexpired capture reservation".into());
    }
    match command {
        Command::Reserve { lease_ms } => {
            if !(5000..=120000).contains(&lease_ms) {
                return Err("capture reservation lease must be 5–120 seconds".into());
            }
            if manager.session.is_some() || manager.reservation.is_some() {
                return Err("capture slot is already running, finishing or reserved".into());
            }
            manager.reservation =
                Some((Instant::now(), Duration::from_millis(u64::from(lease_ms))));
            manager.status = json!({"state":"reserved","frames":0});
            manager.observed = None;
            manager.sample_controls = None;
        }
        Command::Renew { lease_ms } => {
            if !(5000..=120000).contains(&lease_ms) || manager.reservation.is_none() {
                return Err(
                    "capture renewal needs a live reservation and a 5–120 second lease".into(),
                );
            }
            manager.reservation =
                Some((Instant::now(), Duration::from_millis(u64::from(lease_ms))));
        }
        Command::Start { options } | Command::Activate { options } => {
            options.validate()?;
            if manager.reservation.is_some() && options.mode != Mode::Indexed {
                return Err("reserved capture slot requires indexed mode".into());
            }
            if manager.session.is_some() {
                return Err("capture is already running or finishing".into());
            }
            manager.status =
                json!({"state":"waiting_for_frame","frames":0,"output":options.output});
            manager.rejected = None;
            manager.observed = None;
            manager.reservation = None;
            manager.session = Some(Session {
                options,
                encoder: None,
                started: Instant::now(),
                clock: None,
                dimensions: None,
                sent: 0,
                last: None,
                error: None,
                finishing: false,
                pending: None,
                last_sequence: None,
                receipt: None,
                settling: None,
            });
        }
        Command::Stop {} => {
            if manager.reservation.take().is_some() {
                manager.status = json!({"state":"cancelled","frames":0,"message":"owned capture reservation released"});
                manager.observed = None;
                manager.sample_controls = None;
            }
            if let Some(session) = &mut manager.session {
                if session.options.mode == Mode::Indexed && session.sent < session.options.frames {
                    session.error =
                        Some("indexed capture stopped before all samples were submitted".into());
                }
                session.finish();
            }
        }
        Command::Sample { sample } => {
            let session = manager
                .session
                .as_mut()
                .ok_or("no indexed capture is running")?;
            if session.options.mode != Mode::Indexed
                || session.finishing
                || session.pending.is_some()
                || sample.index != session.sent
                || sample.index >= session.options.frames
                || !(1..=120).contains(&sample.frames)
                || sample
                    .index
                    .checked_add(sample.frames)
                    .is_none_or(|end| end > session.options.frames)
                || sample.ticks > 1_000_000_000_000
                || !sample.partial.is_finite()
                || !(0. ..1.).contains(&sample.partial)
                || sample
                    .camera
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 32_000_000.)
                || !(1. ..=179.).contains(&sample.camera[5])
            {
                return Err("indexed capture needs the next frame index and a valid native sample, with no request pending".into());
            }
            session.pending = Some((sample, Instant::now()));
        }
    }
    manager.rejected = None;
    Ok(())
}

fn expire_reservation(manager: &mut Manager) {
    if manager
        .reservation
        .as_ref()
        .is_some_and(|(at, lease)| at.elapsed() >= *lease)
    {
        manager.reservation = None;
        manager.observed = None;
        manager.sample_controls = None;
        manager.status = json!({"state":"error","frames":0,"message":"capture reservation expired before indexed start"});
    }
}

impl Session {
    fn finish(&mut self) {
        self.finishing = true;
        if let Some(encoder) = &mut self.encoder {
            // Disconnect the producer while preserving its completion receiver.
            let (replacement, _) = mpsc::sync_channel(0);
            encoder.tx = replacement;
        }
    }
}

pub fn status() -> Value {
    manager()
        .lock()
        .map(|mut manager| {
            expire_reservation(&mut manager);
            let mut status = manager.status.clone();
            status["indexedEncoding"]=json!({"schema":1,"frameMetadataSchema":2,
                "profiles":{"ffmpeg":EncoderKind::Ffmpeg.indexed_profile(),"arctic_builtin":EncoderKind::ArcticBuiltin.indexed_profile()}});
            status["cameraTimeFenceImplemented"]=json!(true);
            status["frameMetadataMaximumBytes"]=json!(MAX_FRAME_METADATA);
            if let Some(session)=manager.session.as_ref() {
                status["encodingProfile"]=json!((session.options.mode==Mode::Indexed).then(||session.options.encoder.indexed_profile()));
            }
            status["captureId"] = json!(manager.capture_id);
            status["commandReceipt"] = json!(manager.command_receipt);
            status["reservationRemainingMs"]=json!(manager.reservation.as_ref().map(|(at,lease)|lease.saturating_sub(at.elapsed()).as_millis()));
            status["pendingControls"] = json!(manager.session.as_ref().filter(|s| s.pending.is_some())
                .and_then(|_| manager.sample_controls.as_ref()));
            status["observedWorld"] = manager.observed.as_ref().map(|(_, v)| v.clone()).unwrap_or(Value::Null);
            status["observedAgeMs"] = json!(manager.session.as_ref().and_then(|_| manager.observed.as_ref()
                .map(|(at, _)| at.elapsed().as_millis())));
            status["nativeWorldVerified"] = json!(false);
            status["deterministicVideoVerified"] = json!(false);
            if let Some(rejected) = &manager.rejected {
                status["requestOutput"] = rejected["output"].clone();
                status["requestError"] = rejected["error"].clone();
                status["requestRejected"] = rejected.clone();
            }
            status
        })
        .unwrap_or_else(|_| json!({"state":"error","message":"capture lock failed"}))
}

pub fn start_control(dir: PathBuf) {
    let pid = std::process::id();
    std::thread::spawn(move || {
        let request = dir.join(format!("arctic-bedrock-{pid}.capture-request.json"));
        let report = dir.join(format!("arctic-bedrock-{pid}.capture.json"));
        let mut previous = Value::Null;
        loop {
            crate::native_camera_control::service(&dir);
            crate::native_clock::service(&dir);
            crate::native_packet::service(&dir);
            crate::native_appearance::service(&dir);
            if let Ok(metadata) = request.metadata() {
                let mut identity = Value::Null;
                let result = if metadata.len() <= 16 * 1024 {
                    std::fs::read(&request)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| {
                            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                                identity = json!({"output":value["output"],"captureId":value["captureId"],
                                    "requestId":value["requestId"],"op":value["op"]});
                            }
                            command(&bytes)
                        })
                } else {
                    Err("capture request exceeds 16 KiB".into())
                };
                let _ = std::fs::remove_file(&request);
                if let Err(error) = result
                    && let Ok(mut manager) = manager().lock()
                {
                    if !identity.is_object() {
                        identity = json!({});
                    }
                    identity["error"] = json!(error);
                    manager.rejected = Some(identity);
                }
            }
            tick();
            let next = status();
            if previous != next {
                let temporary = report.with_extension("json.tmp");
                if std::fs::write(&temporary, next.to_string()).is_ok()
                    && std::fs::rename(&temporary, &report).is_ok()
                {
                    previous = next;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

fn tick() {
    let Ok(mut manager) = manager().lock() else {
        return;
    };
    tick_manager(&mut manager);
}

fn tick_manager(manager: &mut Manager) {
    expire_reservation(manager);
    let Some(session) = &mut manager.session else {
        return;
    };
    let mut done = None;
    if session
        .pending
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(15))
    {
        session.error = Some(
            "no matching settled native world/camera frame arrived for the requested sample".into(),
        );
        session.finish();
    }
    if let Some(encoder) = &session.encoder {
        match encoder.result.try_recv() {
            Ok(result) => done = Some(result),
            Err(mpsc::TryRecvError::Disconnected) => {
                done = Some(Err("video encoder exited without a result".into()))
            }
            _ => {}
        }
    } else if session.finishing {
        done = Some(Err(session
            .error
            .clone()
            .unwrap_or_else(|| "capture stopped before its first frame".into())));
    } else if session.started.elapsed()
        > Duration::from_secs(if session.options.mode == Mode::Indexed {
            60
        } else {
            10
        })
    {
        done = Some(Err("no presented frame arrived within 10 seconds".into()));
    }
    if let Some(result) = done {
        let failure = session
            .error
            .clone()
            .or_else(|| result.as_ref().err().cloned())
            .or_else(|| {
                result
                    .as_ref()
                    .ok()
                    .filter(|count| {
                        session.options.mode == Mode::Indexed && **count != session.options.frames
                    })
                    .map(|_| "indexed encoder completed with an unexpected frame count".to_owned())
            });
        manager.status = json!({"state":if failure.is_some(){"error"}else{"complete"},"message":failure,
            "frames":result.unwrap_or(session.sent),"output":session.options.output,"fps":session.options.fps,"mode":session.options.mode,
            "encoder":session.options.encoder,
            "encodingProfile":(session.options.mode==Mode::Indexed).then(||session.options.encoder.indexed_profile()),
            "receipt":session.receipt,"manifest":(session.options.mode==Mode::Indexed).then(||session.options.output.with_extension("frames.jsonl"))});
        manager.session = None;
    } else {
        manager.status = json!({"state":if session.finishing{"finishing"}else if session.encoder.is_some(){"capturing"}else{"waiting_for_frame"},
            "frames":session.sent,"output":session.options.output,"fps":session.options.fps,"mode":session.options.mode,
            "encoder":session.options.encoder,
            "encodingProfile":(session.options.mode==Mode::Indexed).then(||session.options.encoder.indexed_profile()),
            "receipt":session.receipt,"pendingIndex":session.pending.as_ref().map(|(s,_)|s.index),
            "settlingFrames":session.settling.as_ref().map_or(0,|s|s.frames),"settlingBufferMask":session.settling.as_ref().map_or(0,|s|s.buffer_mask)});
    }
}

pub fn frame_options() -> (bool, bool) {
    manager()
        .lock()
        .ok()
        .and_then(|m| {
            m.session
                .as_ref()
                .map(|s| (s.options.mode == Mode::Indexed, s.options.clean_view))
        })
        .unwrap_or((false, false))
}
pub fn needs_frame(tag: Option<&FrameTag>) -> bool {
    let Ok(mut manager) = manager().lock() else {
        return false;
    };
    if let Some(tag) = tag
        && manager.session.as_ref().is_some_and(|s| {
            !s.finishing
                && s.options.mode == Mode::Indexed
                && s.options.session.as_deref() == Some(tag.session.as_str())
        })
    {
        // A presented world-layer identity can be observed without issuing a
        // readback or encoding a frame. It is not terrain/application proof.
        manager.observed = Some((Instant::now(), json!({"world":tag,"gpu":tag.gpu})));
    }
    manager.session.as_ref().is_some_and(|session| {
        !session.finishing
            && (session.options.mode == Mode::Realtime
                || session.pending.as_ref().is_some_and(|(s, _)| {
                    tag.is_some_and(|t| s.matches(t, &session.options, session.last_sequence))
                        && manager
                            .sample_controls
                            .as_ref()
                            .is_none_or(|f| tag.is_some_and(|t| f.matches(t)))
                }))
            && (session.options.mode == Mode::Indexed
                || session.clock.is_none_or(|clock| {
                    (clock.elapsed().as_secs_f64() * session.options.fps as f64).floor() as u32
                        >= session.sent
                }))
    })
}

pub fn fail(error: String) {
    if let Ok(mut manager) = manager().lock()
        && let Some(session) = &mut manager.session
    {
        session.error = Some(error);
        session.finish();
    }
}

pub fn frame(width: u32, height: u32, rgba: Vec<u8>, tag: Option<FrameTag>) {
    let Ok(mut manager) = manager().lock() else {
        return;
    };
    frame_manager(&mut manager, width, height, rgba, tag);
}
fn frame_manager(
    manager: &mut Manager,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    tag: Option<FrameTag>,
) {
    let capture_id = manager.capture_id.clone();
    let request_id = manager
        .command_receipt
        .as_ref()
        .and_then(|v| v["requestId"].as_str())
        .map(str::to_owned);
    let controls = manager.sample_controls.clone();
    let Some(session) = &mut manager.session else {
        return;
    };
    if session.finishing {
        return;
    }
    if session.options.mode == Mode::Indexed
        && !session.pending.as_ref().is_some_and(|(s, _)| {
            tag.as_ref().is_some_and(|t| {
                s.matches(t, &session.options, session.last_sequence)
                    && controls.as_ref().is_none_or(|f| f.matches(t))
            })
        })
    {
        return;
    }
    let result = (|| -> Result<(), String> {
        if width < 2
            || height < 2
            || width > 3840
            || height > 2160
            || rgba.len() != width as usize * height as usize * 4
        {
            return Err("capture frame exceeds 3840×2160 or has invalid RGBA data".into());
        }
        if session
            .dimensions
            .is_some_and(|size| size != (width, height))
        {
            return Err("swap-chain size changed during capture; start a new clip".into());
        }
        session.dimensions = Some((width, height));
        let mut settled = None;
        let rgba = if session.options.mode == Mode::Indexed && session.options.settle_frames > 1 {
            // Examine each native world layer at most once. A camera seek can
            // reach its matrix before all native render preparation has settled.
            session.last_sequence = tag.as_ref().map(|t| t.sequence);
            match session.settle(rgba, tag.as_ref().and_then(|t| t.gpu.as_ref()))? {
                Some((pixels, proof)) => {
                    settled = Some(proof);
                    pixels
                }
                None => return Ok(()),
            }
        } else {
            rgba
        };
        if session.encoder.is_none() {
            session.encoder = Some(encode(&session.options, width, height)?);
            session.dimensions = Some((width, height));
            session.clock = Some(Instant::now());
        }
        let slot = if session.options.mode == Mode::Indexed {
            session.sent
        } else {
            ((session.clock.unwrap().elapsed().as_secs_f64() * session.options.fps as f64).floor()
                as u32)
                .min(session.options.frames - 1)
        };
        if slot.saturating_sub(session.sent) > session.options.fps * 2 {
            return Err("rendering stalled for more than two seconds during capture".into());
        }
        // Fill short real-time gaps with the last captured frame, preserving
        // wall-clock video duration without inventing simulation steps.
        let gap = slot.saturating_sub(session.sent);
        if gap > 0
            && let Some(last) = &session.last
        {
            session
                .encoder
                .as_ref()
                .unwrap()
                .tx
                .try_send(FrameBatch {
                    pixels: last.clone(),
                    repeat: gap,
                    metadata: None,
                })
                .map_err(|_| "video encoder cannot keep up with the capture rate")?;
            session.sent += gap;
        }
        let pixels: Arc<[u8]> = rgba.into();
        let receipt = tag
            .as_ref()
            .map(|tag| json!({"index":session.sent,"world":tag,"gpu":tag.gpu,"settled":settled,
                "captureId":capture_id,"requestId":request_id,
                "requiredControls":controls,
                "submittedShaderConstants":crate::native_gpu_constants::captured_frame(&tag.session,tag.sequence),
                "presentedShaderConstants":tag.presented_constants}));
        session
            .encoder
            .as_ref()
            .unwrap()
            .tx
            .try_send(FrameBatch {
                pixels: pixels.clone(),
                repeat: 1,
                metadata: if session.options.mode == Mode::Indexed {
                    receipt.clone()
                } else {
                    None
                },
            })
            .map_err(|_| "video encoder cannot keep up with the capture rate")?;
        session.sent += 1;
        if session.options.mode == Mode::Indexed {
            session.pending = session.pending.take().and_then(|(mut sample, _)| {
                if sample.frames <= 1 {
                    return None;
                }
                sample.index += 1;
                sample.frames -= 1;
                Some((sample, Instant::now()))
            });
            session.last_sequence = tag.map(|t| t.sequence);
            session.receipt = receipt;
        }
        session.last = Some(pixels);
        if session.sent == session.options.frames {
            session.finish();
        }
        Ok(())
    })();
    if let Err(error) = result {
        session.error = Some(error);
        session.finish();
    }
}

fn encode(options: &Options, width: u32, height: u32) -> Result<Encoder, String> {
    let output = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&options.output)
        .map_err(|e| e.to_string())?;
    // Keep the originally created file descriptor for final flush/size checks;
    // no path is reopened or replaced by the encoder worker.
    let encoded_output = output.try_clone().map_err(|e| e.to_string())?;
    let mut manifest = if options.mode == Mode::Indexed {
        Some(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(options.output.with_extension("frames.jsonl"))
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mut command = Process::new(&options.ffmpeg);
    match options.encoder {
        EncoderKind::ArcticBuiltin => {
            command
                .arg("--encode-video-stream")
                .arg(width.to_string())
                .arg(height.to_string())
                .arg(options.fps.to_string())
                .arg(options.frames.to_string())
                .arg(if options.mode == Mode::Indexed {
                    "indexed_mf_hardware_disabled_v1"
                } else {
                    "realtime"
                });
        }
        EncoderKind::Ffmpeg => {
            command.args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{width}x{height}"),
            "-framerate",
            &options.fps.to_string(),
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-threads",
            if options.mode==Mode::Indexed {"1"} else {"2"},
            "-vf",
            "pad=ceil(iw/2)*2:ceil(ih/2)*2:color=black,scale=in_range=full:out_range=limited:out_color_matrix=bt709:out_h_chr_pos=128:out_v_chr_pos=128",
            "-pix_fmt",
            "yuv420p",
            "-color_range",
            "tv",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-chroma_sample_location",
            "center",
        ]);
            if options.mode == Mode::Indexed {
                command.args(["-fflags", "+bitexact", "-flags:v", "+bitexact"]);
            }
            command.args([
                "-f",
                "mp4",
                "-movflags",
                "frag_keyframe+empty_moov",
                "pipe:1",
            ]);
        }
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::from(output))
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut input = child.stdin.take().ok_or("video encoder has no input")?;
    let stderr = child.stderr.take();
    let diagnostics = std::thread::spawn(move || {
        use std::io::Read;
        let mut tail = Vec::new();
        if let Some(mut stderr) = stderr {
            let mut chunk = [0u8; 1024];
            while let Ok(count) = stderr.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                tail.extend_from_slice(&chunk[..count]);
                if tail.len() > 8192 {
                    tail.drain(..tail.len() - 8192);
                }
            }
        }
        String::from_utf8_lossy(&tail).into_owned()
    });
    let child = std::sync::Arc::new(Mutex::new(child));
    let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let progress = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let started = Instant::now();
    let idle_ms = if options.mode == Mode::Indexed {
        60_000
    } else {
        10_000
    };
    let capacity = (64 * 1024 * 1024 / (width as usize * height as usize * 4)).clamp(1, 16);
    let (tx, rx) = mpsc::sync_channel::<FrameBatch>(capacity);
    let (done, result) = mpsc::channel();
    let encoder_kind = options.encoder;
    let mode = options.mode;
    let expected_frames = options.frames;
    let fps = options.fps;
    let watchdog_child = child.clone();
    let watchdog_active = active.clone();
    let watchdog_progress = progress.clone();
    std::thread::spawn(move || {
        use std::sync::atomic::Ordering;
        while watchdog_active.load(Ordering::Acquire) {
            if started.elapsed().as_millis() as u64
                > watchdog_progress.load(Ordering::Acquire) + idle_ms
            {
                if let Ok(mut child) = watchdog_child.lock() {
                    let _ = child.kill();
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    std::thread::spawn(move || {
        use std::sync::atomic::Ordering;
        let mut count = 0u32;
        let mut manifest_rows = 0u32;
        let mut failure = None;
        loop {
            match rx.recv_timeout(Duration::from_millis(idle_ms)) {
                Ok(batch) => {
                    if batch.repeat == 0
                        || batch.pixels.len() != width as usize * height as usize * 4
                        || count
                            .checked_add(batch.repeat)
                            .is_none_or(|end| end > expected_frames)
                        || mode == Mode::Indexed
                            && (batch.repeat != 1
                                || batch.metadata.as_ref().and_then(|m| m["index"].as_u64())
                                    != Some(u64::from(count)))
                        || mode != Mode::Indexed && batch.metadata.is_some()
                    {
                        failure = Some(
                            "encoder batch has invalid pixels, index, repetition or frame count"
                                .into(),
                        );
                        break;
                    }
                    for _ in 0..batch.repeat {
                        if let Err(error) = input.write_all(&batch.pixels) {
                            failure = Some(error.to_string());
                            break;
                        }
                        count += 1;
                        progress.store(started.elapsed().as_millis() as u64, Ordering::Release);
                    }
                    if failure.is_some() {
                        break;
                    }
                    let audit = (|| -> Result<(), String> {
                        if let Some(mut metadata) = batch.metadata {
                            let mut hash = crate::native_hash::Sha256::new()?;
                            hash.update(&batch.pixels)?;
                            metadata["rgbaSha256"] =
                                json!(crate::native_hash::hex(&hash.finish()?));
                            metadata["width"] = json!(width);
                            metadata["height"] = json!(height);
                            metadata["encoder"] = json!(encoder_kind);
                            metadata["encodedWidth"] = json!((width + 1) & !1);
                            metadata["encodedHeight"] = json!((height + 1) & !1);
                            let start = u64::from(count - 1) * 10_000_000 / u64::from(fps);
                            let end = u64::from(count) * 10_000_000 / u64::from(fps);
                            metadata["encoding"] = json!({"schema":2,"profile":encoder_kind.indexed_profile(),"encoderInputWritten":true,"decodedFrameVerified":false,
                                "sourcePixelFormat":"rgba8","sourceRange":"full","sourceAlpha":"discarded",
                                "requestedOutput":{"matrix":"bt709","primaries":"bt709","transfer":"bt709",
                                    "range":"limited","chromaSubsampling":"420","chromaLocation":"center","padding":"black_right_bottom"},
                                "time":{"numerator":count-1,"denominator":fps,"timescale":10_000_000,
                                    "start100ns":start,"duration100ns":end-start}});
                            let file = manifest.as_mut().ok_or("indexed frame has no manifest")?;
                            serde_json::to_writer(&mut *file, &metadata)
                                .map_err(|e| e.to_string())?;
                            file.write_all(b"\n").map_err(|e| e.to_string())?;
                            manifest_rows += 1;
                        }
                        Ok(())
                    })();
                    if let Err(error) = audit {
                        failure = Some(error);
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => {
                    failure = Some("capture encoder timed out waiting for the next frame".into());
                    break;
                }
            }
        }
        drop(input);
        if failure.is_none()
            && (count == 0
                || mode == Mode::Indexed && (count != expected_frames || manifest_rows != count))
        {
            failure =
                Some("encoder input ended without every indexed frame and manifest row".into());
        }
        if let Some(file) = manifest.as_ref()
            && let Err(error) = file.sync_all()
            && failure.is_none()
        {
            failure = Some(format!("syncing frame manifest: {error}"));
        }
        let status = loop {
            let result = child
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|mut child| child.try_wait().map_err(|e| e.to_string()));
            match result {
                Ok(Some(status)) => break Ok(status),
                Err(error) => break Err(error),
                _ => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        active.store(false, Ordering::Release);
        let diagnostics = diagnostics.join().unwrap_or_default();
        if failure.is_none() && status.as_ref().is_ok_and(|status| status.success()) {
            let artifact = encoded_output
                .sync_all()
                .and_then(|_| encoded_output.metadata());
            match artifact {
                Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {}
                Ok(_) => failure = Some("encoder completed without a nonempty video".into()),
                Err(error) => failure = Some(format!("syncing encoded video: {error}")),
            }
        }
        let outcome = match (failure, status) {
            (Some(error), status) => {
                Err(format!("{error}; encoder exit {status:?}: {diagnostics}"))
            }
            (_, Ok(status)) if status.success() => Ok(count),
            (_, status) => Err(format!("video encoder failed ({status:?}): {diagnostics}")),
        };
        // Completion hands the files to readers. Close our final write handles
        // before publishing it so a read-only audit can deny concurrent writes.
        drop(manifest);
        drop(encoded_output);
        let _ = done.send(outcome);
    });
    Ok(Encoder { tx, result })
}

#[unsafe(no_mangle)]
/// Queue capture control JSON.
///
/// # Safety
/// `json` must point to `length` readable bytes.
pub unsafe extern "C" fn arctic_bedrock_capture_v1(json: *const u8, length: usize) -> i32 {
    if json.is_null() || length == 0 || length > 16 * 1024 {
        return -1;
    }
    std::panic::catch_unwind(|| command(unsafe { std::slice::from_raw_parts(json, length) }))
        .map_or(-3, |result| if result.is_ok() { 0 } else { -2 })
}

#[unsafe(no_mangle)]
pub extern "C" fn arctic_bedrock_capture_status_v1() -> u64 {
    let status = status();
    let code = match status["state"].as_str().unwrap_or("") {
        "idle" => 0,
        "waiting_for_frame" => 1,
        "reserved" => 6,
        "cancelled" => 7,
        "capturing" => 2,
        "finishing" => 3,
        "complete" => 4,
        _ => 5,
    };
    (status["frames"].as_u64().unwrap_or(0) << 32) | code
}

#[cfg(test)]
mod tests {
    use super::*;
    fn indexed_options(output: PathBuf) -> Options {
        Options {
            settle_frames: 1,
            encoder: EncoderKind::Ffmpeg,
            ffmpeg: std::env::current_exe().unwrap(),
            output,
            fps: 24,
            frames: 3,
            mode: Mode::Indexed,
            session: Some("current".into()),
            clean_view: true,
        }
    }
    fn tag(sequence: u64) -> FrameTag {
        FrameTag {
            gpu: None,
            presented_constants: None,
            schema: 1,
            session: "current".into(),
            sequence,
            clock_ms: 125,
            ticks: 2,
            partial: 0.5,
            camera: [1., 2., 3., 15., 10., 60.],
            clean_view: true,
            controls: None,
        }
    }
    fn sample(index: u32) -> Sample {
        Sample {
            frames: 1,
            index,
            min_sequence: 7,
            ticks: 2,
            partial: 0.5,
            clock_ms: 125,
            camera: [1., 2., 3., 15., 10., 60.],
        }
    }
    #[test]
    fn indexed_frames_require_the_rendered_identity_and_never_fill_wall_time_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::default();
        command_manager(
            &mut manager,
            Command::Start {
                options: indexed_options(dir.path().join("sample.mp4")),
            },
        )
        .unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        let (_done, result) = mpsc::channel();
        let session = manager.session.as_mut().unwrap();
        session.encoder = Some(Encoder { tx, result });
        session.dimensions = Some((2, 2));
        session.clock = Some(Instant::now() - Duration::from_secs(90));
        command_manager(&mut manager, Command::Sample { sample: sample(0) }).unwrap();
        assert!(command_manager(&mut manager, Command::Sample { sample: sample(0) }).is_err());
        let mut foreign = tag(7);
        foreign.session = "expired".into();
        frame_manager(&mut manager, 2, 2, vec![9; 16], Some(foreign));
        let mut wrong_camera = tag(7);
        wrong_camera.camera[0] = 2.;
        frame_manager(&mut manager, 2, 2, vec![9; 16], Some(wrong_camera));
        frame_manager(&mut manager, 2, 2, vec![9; 16], None);
        assert_eq!(manager.session.as_ref().unwrap().sent, 0);
        assert!(rx.try_recv().is_err());
        frame_manager(&mut manager, 2, 2, vec![9; 16], Some(tag(7)));
        let batch = rx.try_recv().unwrap();
        assert_eq!(batch.repeat, 1);
        assert_eq!(&*batch.pixels, &[9; 16]);
        assert_eq!(batch.metadata.unwrap()["index"], 0);
        assert_eq!(manager.session.as_ref().unwrap().sent, 1);
        frame_manager(&mut manager, 2, 2, vec![8; 16], Some(tag(8)));
        assert!(rx.try_recv().is_err());
        assert!(command_manager(&mut manager, Command::Sample { sample: sample(0) }).is_err());
        command_manager(&mut manager, Command::Sample { sample: sample(1) }).unwrap();
        frame_manager(&mut manager, 2, 2, vec![8; 16], Some(tag(7)));
        assert!(rx.try_recv().is_err());
        frame_manager(&mut manager, 2, 2, vec![8; 16], Some(tag(8)));
        assert_eq!(rx.try_recv().unwrap().repeat, 1);
        assert_eq!(manager.session.as_ref().unwrap().sent, 2);
    }
    #[test]
    fn indexed_burst_is_bounded_and_each_frame_needs_a_new_matching_world() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::default();
        command_manager(
            &mut manager,
            Command::Start {
                options: indexed_options(dir.path().join("burst.mp4")),
            },
        )
        .unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        let (_done, result) = mpsc::channel();
        let session = manager.session.as_mut().unwrap();
        session.encoder = Some(Encoder { tx, result });
        session.dimensions = Some((2, 2));
        for count in [0, 4, 121] {
            let mut request = sample(0);
            request.frames = count;
            assert!(command_manager(&mut manager, Command::Sample { sample: request }).is_err());
            assert!(manager.session.as_ref().unwrap().pending.is_none());
        }
        let mut request = sample(0);
        request.frames = 3;
        command_manager(&mut manager, Command::Sample { sample: request }).unwrap();
        let mut first = tag(7);
        first.gpu = Some(PresentInfo {
            api: "dx12",
            sequence: 11,
            buffer_index: Some(2),
            buffer_count: Some(3),
        });
        frame_manager(&mut manager, 2, 2, vec![1; 16], Some(first));
        let metadata = rx.try_recv().unwrap().metadata.unwrap();
        assert_eq!(metadata["index"], 0);
        assert_eq!(metadata["gpu"]["bufferIndex"], 2);
        assert!(metadata["world"].get("gpu").is_none());
        frame_manager(&mut manager, 2, 2, vec![9; 16], Some(tag(7)));
        let mut foreign = tag(8);
        foreign.ticks += 1;
        frame_manager(&mut manager, 2, 2, vec![9; 16], Some(foreign));
        assert!(rx.try_recv().is_err());
        assert_eq!(
            manager
                .session
                .as_ref()
                .unwrap()
                .pending
                .as_ref()
                .unwrap()
                .0
                .index,
            1
        );
        for (sequence, index) in [(8, 1), (9, 2)] {
            frame_manager(
                &mut manager,
                2,
                2,
                vec![index as u8; 16],
                Some(tag(sequence)),
            );
            let batch = rx.try_recv().unwrap();
            assert_eq!(batch.repeat, 1);
            assert_eq!(batch.metadata.unwrap()["index"], index);
        }
        let session = manager.session.as_ref().unwrap();
        assert_eq!(session.sent, 3);
        assert!(session.finishing);
        assert!(session.pending.is_none());
    }
    #[test]
    fn settled_capture_rejects_transients_and_requires_every_dx12_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::default();
        let mut options = indexed_options(dir.path().join("settled.mp4"));
        options.settle_frames = 3;
        command_manager(&mut manager, Command::Start { options }).unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        let (_done, result) = mpsc::channel();
        let session = manager.session.as_mut().unwrap();
        session.encoder = Some(Encoder { tx, result });
        session.dimensions = Some((2, 2));
        command_manager(&mut manager, Command::Sample { sample: sample(0) }).unwrap();
        let frame = |sequence, buffer| {
            let mut t = tag(sequence);
            t.gpu = Some(PresentInfo {
                api: "dx12",
                sequence,
                buffer_index: Some(buffer),
                buffer_count: Some(3),
            });
            t
        };
        frame_manager(&mut manager, 2, 2, vec![1; 16], Some(frame(7, 0)));
        frame_manager(&mut manager, 2, 2, vec![1; 16], Some(frame(7, 1)));
        assert_eq!(
            manager
                .session
                .as_ref()
                .unwrap()
                .settling
                .as_ref()
                .unwrap()
                .frames,
            1
        );
        for (sequence, buffer) in [(8, 1), (9, 0), (10, 0)] {
            frame_manager(
                &mut manager,
                2,
                2,
                vec![2; 16],
                Some(frame(sequence, buffer)),
            );
            assert!(rx.try_recv().is_err());
        }
        let state = manager.session.as_ref().unwrap().settling.as_ref().unwrap();
        assert_eq!(state.frames, 3);
        assert_eq!(state.buffer_mask, 3);
        frame_manager(&mut manager, 2, 2, vec![2; 16], Some(frame(11, 2)));
        let batch = rx.try_recv().unwrap();
        assert_eq!(&*batch.pixels, &[2; 16]);
        assert_eq!(batch.repeat, 1);
        let metadata = batch.metadata.unwrap();
        assert_eq!(metadata["world"]["sequence"], 11);
        assert_eq!(
            metadata["settled"],
            json!({"frames":4,"bufferCount":3,"bufferMask":7})
        );
        assert!(manager.session.as_ref().unwrap().settling.is_none());
        assert_eq!(manager.session.as_ref().unwrap().sent, 1);
    }
    #[test]
    fn unsettled_native_images_time_out_without_starting_an_encoder() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::default();
        let mut options = indexed_options(dir.path().join("unstable.mp4"));
        options.settle_frames = 3;
        command_manager(&mut manager, Command::Start { options }).unwrap();
        command_manager(&mut manager, Command::Sample { sample: sample(0) }).unwrap();
        for (sequence, pixel) in [(7, 1), (8, 2), (9, 1), (10, 2)] {
            frame_manager(&mut manager, 2, 2, vec![pixel; 16], Some(tag(sequence)));
        }
        let state = manager.session.as_mut().unwrap();
        assert!(state.encoder.is_none());
        assert_eq!(state.sent, 0);
        state.pending.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(16);
        tick_manager(&mut manager);
        assert!(manager.session.is_none());
        assert_eq!(manager.status["state"], "error");
        assert!(
            manager.status["message"]
                .as_str()
                .unwrap()
                .contains("settled")
        );
        assert!(!dir.path().join("unstable.mp4").exists());
    }
    #[test]
    fn indexed_metadata_rejects_wrong_fraction_camera_clock_and_missing_clean_view() {
        let dir = tempfile::tempdir().unwrap();
        let options = indexed_options(dir.path().join("sample.mp4"));
        let request = sample(0);
        let correct = tag(7);
        assert!(request.matches(&correct, &options, None));
        for field in 0..4 {
            let mut wrong = tag(7);
            match field {
                0 => wrong.partial = 0.6,
                1 => wrong.clock_ms += 1,
                2 => wrong.clean_view = false,
                _ => wrong.ticks += 1,
            }
            assert!(!request.matches(&wrong, &options, None));
        }
        assert!(FrameTag::parse(&serde_json::to_string(&correct).unwrap()).is_some());
        let mut invalid = serde_json::to_value(correct).unwrap();
        invalid["partial"] = json!(1.);
        assert!(FrameTag::parse(&invalid.to_string()).is_none());
    }
    #[test]
    fn initial_capture_failure_keeps_its_cause_and_releases_the_session() {
        let dir = tempfile::tempdir().unwrap();
        for cause in [
            None,
            Some("encoder launch denied (os error 5)"),
            Some("GPU readback failed"),
        ] {
            let mut manager = Manager {
                session: Some(Session {
                    options: Options {
                        settle_frames: 3,
                        encoder: EncoderKind::Ffmpeg,
                        ffmpeg: std::env::current_exe().unwrap(),
                        output: dir.path().join("failed.mp4"),
                        fps: 30,
                        frames: 60,
                        mode: Mode::Realtime,
                        session: None,
                        clean_view: false,
                    },
                    encoder: None,
                    started: Instant::now(),
                    clock: None,
                    dimensions: None,
                    sent: 0,
                    last: None,
                    error: cause.map(str::to_owned),
                    finishing: true,
                    pending: None,
                    last_sequence: None,
                    receipt: None,
                    settling: None,
                }),
                ..Manager::default()
            };
            tick_manager(&mut manager);
            assert!(manager.session.is_none());
            assert_eq!(manager.status["state"], "error");
            assert_eq!(
                manager.status["message"],
                cause.unwrap_or("capture stopped before its first frame")
            );
            assert_eq!(manager.status["frames"], 0);
        }
    }
    #[test]
    fn capture_controls_validate_paths_limits_and_refuse_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut options = Options {
            settle_frames: 3,
            encoder: EncoderKind::Ffmpeg,
            ffmpeg: std::env::current_exe().unwrap(),
            output: dir.path().join("new.mp4"),
            fps: 30,
            frames: 60,
            mode: Mode::Realtime,
            session: None,
            clean_view: false,
        };
        options.validate().unwrap();
        let valid = json!({"op":"start","ffmpeg":options.ffmpeg,"output":options.output,"fps":30,"frames":60});
        assert!(serde_json::from_value::<Command>(valid.clone()).is_ok());
        let mut extra = valid;
        extra["accessToken"] = json!("never");
        assert!(serde_json::from_value::<Command>(extra).is_err());
        assert!(serde_json::from_value::<Command>(json!({"op":"stop","output":"no"})).is_err());
        options.fps = 0;
        assert!(options.validate().is_err());
        options.fps = 30;
        options.frames = 30 * 7200 + 1;
        assert!(options.validate().is_err());
        options.frames = 60;
        std::fs::write(&options.output, b"original").unwrap();
        assert!(options.validate().is_err());
        assert_eq!(std::fs::read(&options.output).unwrap(), b"original");
    }
}
