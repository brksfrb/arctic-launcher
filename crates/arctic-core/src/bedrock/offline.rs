//! Unsigned Bedrock sessions for servers that explicitly accept offline players.
//! A private protocol helper terminates the local connection and creates its own
//! unsigned upstream identity. No Xbox/Microsoft credentials are requested.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::IoContext;
use crate::{Error, Result};

#[path = "camera_curve.rs"]
mod camera_curve;
pub use camera_curve::CameraCurve;

/// Java ReplayViewer's ordered playback steps. Preserve the existing Bedrock
/// continuous 0.1x input as well as the shared faster/slower controls.
pub const REPLAY_SPEEDS: [f64; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
pub const MIN_REPLAY_SPEED: f64 = 0.1;
pub const MAX_REPLAY_SPEED: f64 = 16.0;

const SCRIPT: &str = include_str!("../../../../mod/versions/bedrock/offline/connector.cjs");
const REPLAY: &str = include_str!("../../../../mod/versions/bedrock/offline/replay.cjs");
const REPLAY_PACKS: &str =
    include_str!("../../../../mod/versions/bedrock/offline/replay-packs.cjs");
const REPLAY_SELF: &str = include_str!("../../../../mod/versions/bedrock/offline/replay-self.cjs");
const RECORDING_CONTROL: &str =
    include_str!("../../../../mod/versions/bedrock/offline/recording-control.cjs");
const CAMERA: &str = include_str!("../../../../mod/versions/bedrock/offline/camera.cjs");
const REPLAY_NETHERNET: &str =
    include_str!("../../../../mod/versions/bedrock/offline/replay-nethernet.cjs");
const LOOPBACK_RTC: &str =
    include_str!("../../../../mod/versions/bedrock/offline/loopback-rtc.cjs");
const PACKAGE: &str = include_str!("../../../../mod/versions/bedrock/offline/package.json");
const LOCK: &str = include_str!("../../../../mod/versions/bedrock/offline/package-lock.json");
const PREFIX: &str = "ARCTIC_BEDROCK_OFFLINE ";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// None discovers the protocol from the target server's ping.
    pub version: Option<String>,
    pub listen_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<super::appearance::Appearance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_to: Option<PathBuf>,
}

impl OfflineOptions {
    pub fn new(
        address: &str,
        username: &str,
        listen_port: u16,
        version: Option<String>,
    ) -> Result<Self> {
        let (host, port) = arctic_ping::parse_bedrock_address(address).ok_or_else(|| {
            Error::Other("enter a Bedrock server address (host or host:port)".into())
        })?;
        let username = username.trim();
        if !(3..=16).contains(&username.len())
            || !username
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(Error::Other(
                "use an offline name with 3–16 letters, digits or underscores".into(),
            ));
        }
        if listen_port == 0 || port == 0 {
            return Err(Error::Other("ports must be between 1 and 65535".into()));
        }
        if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") && port == listen_port {
            return Err(Error::Other(
                "the offline connector cannot forward to itself".into(),
            ));
        }
        Ok(Self {
            host,
            port,
            username: username.into(),
            version,
            listen_port,
            appearance: None,
            record_to: None,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEvent {
    pub event: String,
    pub request_id: Option<String>,
    pub operation: Option<String>,
    pub result: Option<serde_json::Value>,
    pub state: Option<serde_json::Value>,
    pub address: Option<String>,
    pub username: Option<String>,
    pub version: Option<String>,
    pub message: Option<String>,
    pub xbox_auth: Option<bool>,
    pub position_ms: Option<u32>,
    pub duration_ms: Option<u32>,
    pub paused: Option<bool>,
    pub speed: Option<f64>,
    pub generation: Option<u64>,
    pub sequence: Option<u64>,
    pub connection_required: Option<bool>,
    pub packet_ready: Option<bool>,
    pub bootstrap_time_ms: Option<u32>,
    pub wait_for_client_init: Option<bool>,
    pub client_initialized: Option<bool>,
    pub viewers: Option<u32>,
    pub initialized_viewers: Option<u32>,
    pub viewer_id: Option<String>,
    pub player_uuid: Option<String>,
    pub viewer_identities: Option<Vec<ReplayViewerIdentity>>,
    pub packets_queued: Option<bool>,
    pub camera_time: Option<CameraSampleTime>,
    pub camera_queued: Option<bool>,
    pub camera_pose: Option<CameraPose>,
    pub marker_timestamp: Option<String>,
    pub marker_queued: Option<bool>,
    pub native_world_verified: Option<bool>,
    pub pending_step: Option<ReplayStep>,
    pub resource_packs: Option<ReplayResourcePacks>,
    pub automatic_reconnect_available: Option<bool>,
    pub world_reset: Option<ReplayWorldReset>,
}

/// Connection/bootstrap status only, never native terrain or render proof.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayWorldReset {
    pub generation: u64,
    pub time_ms: u32,
    pub stage: String,
    pub automatic_reconnect: bool,
    pub old_viewer_id: Option<String>,
    pub new_viewer_id: Option<String>,
    pub message: Option<String>,
}

/// Archives present in the recording; client loading/rendering is separate.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResourcePacks {
    pub offered: u32,
    pub embedded: u32,
    pub offered_bytes: u64,
    pub embedded_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayViewerIdentity {
    pub viewer_id: String,
    pub player_uuid: String,
    pub username: String,
    pub generation: u64,
    pub client_initialized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayStep {
    pub generation: u64,
    pub sequence: u64,
    pub time_ms: u32,
}

/// Exact camera sampling time; packet timestamps still use time_ms's floor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraSampleTime {
    pub time_ms: u32,
    pub remainder: u32,
    pub denominator: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayControlReply {
    pub request_id: String,
    pub operation: String,
    pub result: serde_json::Value,
    /// Fresh helper state copied after this operation completed, not a cached
    /// periodic replay_state event. Queue progress is not native world proof.
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
    #[serde(default = "default_camera_fov")]
    pub fov: f64,
}

fn default_camera_fov() -> f64 {
    70.0
}
fn default_camera_ease() -> String {
    "linear".into()
}

impl Default for CameraPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 80.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        }
    }
}
impl CameraPose {
    pub fn validate(&self) -> Result<()> {
        self.validate_evaluated()?;
        if self.yaw.abs() > 360_000.0 {
            return Err(Error::Other(
                "invalid camera coordinates, angles or FOV".into(),
            ));
        }
        Ok(())
    }
    /// Java's nearest-turn track can accumulate yaw beyond the saved-key
    /// limit. Preserve that evaluated angle through the spline; only wire
    /// conversion normalizes it. Other geometry bounds still apply.
    pub(super) fn validate_evaluated(&self) -> Result<()> {
        if ![self.x, self.y, self.z, self.yaw, self.pitch, self.fov]
            .iter()
            .all(|v| v.is_finite())
            || [self.x, self.y, self.z]
                .iter()
                .any(|v| v.abs() > 30_000_000.0)
            || self.pitch.abs() > 90.0
            || !(1.0..=179.0).contains(&self.fov)
        {
            return Err(Error::Other(
                "invalid camera coordinates, angles or FOV".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraKeyframe {
    pub time_ms: u32,
    pub pose: CameraPose,
    #[serde(default = "default_camera_ease")]
    pub ease: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraPath {
    pub format: u32,
    pub keyframes: Vec<CameraKeyframe>,
}

impl CameraPath {
    /// Binary fingerprint shared with camera.cjs: accepted key order, easing
    /// codes and exact float64 pose bits, independent of JSON object ordering.
    pub fn sha256(&self) -> Result<String> {
        use sha2::{Digest, Sha256};
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"arctic-bedrock-camera-path-v1\0");
        hash.update(self.format.to_le_bytes());
        hash.update((self.keyframes.len() as u32).to_le_bytes());
        for frame in &self.keyframes {
            hash.update(frame.time_ms.to_le_bytes());
            hash.update([match frame.ease.as_str() {
                "linear" => 0,
                "smooth" => 1,
                "hold" => 2,
                "catmull_rom" => 3,
                _ => unreachable!(),
            }]);
            let p = frame.pose;
            for value in [p.x, p.y, p.z, p.yaw, p.pitch, p.fov] {
                hash.update(value.to_le_bytes());
            }
        }
        Ok(hex::encode(hash.finalize()))
    }
    /// Read a Bedrock path or the camera keys from Java's extracted arctic.json.
    /// This imports camera edits only; Java replay packets remain incompatible.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Other("camera path exceeds 1 MiB".into()));
        }
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        let path: Self = if value.get("format").is_some() {
            serde_json::from_value(value)?
        } else {
            #[derive(Deserialize)]
            struct JavaExtras {
                version: u32,
                keyframes: Vec<JavaKeyframe>,
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct JavaKeyframe {
                time: u32,
                x: f64,
                y: f64,
                z: f64,
                yaw: f32,
                pitch: f32,
                fov: f32,
            }
            let java: JavaExtras = serde_json::from_value(value)?;
            if java.version != 1 || java.keyframes.is_empty() || java.keyframes.len() > 4096 {
                return Err(Error::Other(
                    "Java camera import needs Arctic extras version 1 and 1–4096 keyframes".into(),
                ));
            }
            // Java sorts keys stably and keeps the most recent edit at each
            // paused timestamp. Inserting in source order preserves that rule.
            let mut keys = std::collections::BTreeMap::new();
            for key in java.keyframes {
                keys.insert(
                    key.time,
                    CameraKeyframe {
                        time_ms: key.time,
                        pose: CameraPose {
                            x: key.x,
                            y: key.y,
                            z: key.z,
                            yaw: key.yaw as f64,
                            pitch: key.pitch as f64,
                            fov: key.fov as f64,
                        },
                        ease: "catmull_rom".into(),
                    },
                );
            }
            Self {
                format: 1,
                keyframes: keys.into_values().collect(),
            }
        };
        path.validate()?;
        Ok(path)
    }

    pub fn validate(&self) -> Result<()> {
        let mut previous = None;
        if self.format != 1 || self.keyframes.is_empty() || self.keyframes.len() > 4096 {
            return Err(Error::Other(
                "camera path needs format 1 and 1–4096 keyframes".into(),
            ));
        }
        for frame in &self.keyframes {
            let p = frame.pose;
            if frame.time_ms > 86_400_000
                || previous.is_some_and(|time| frame.time_ms <= time)
                || !matches!(
                    frame.ease.as_str(),
                    "linear" | "smooth" | "hold" | "catmull_rom"
                )
                || ![p.x, p.y, p.z, p.yaw, p.pitch, p.fov]
                    .iter()
                    .all(|v| v.is_finite())
                || [p.x, p.y, p.z].iter().any(|v| v.abs() > 30_000_000.0)
                || p.yaw.abs() > 360_000.0
                || p.pitch.abs() > 90.0
                || !(1.0..=179.0).contains(&p.fov)
            {
                return Err(Error::Other(
                    "invalid camera keyframe time, interpolation or pose".into(),
                ));
            }
            previous = Some(frame.time_ms);
        }
        Ok(())
    }
}

/// Owns the helper process. Dropping it disconnects only this offline session.
#[derive(Debug)]
pub struct OfflineConnector {
    child: Child,
    events: Receiver<OfflineEvent>,
    latest_state: Arc<Mutex<Option<OfflineEvent>>>,
    deferred: Mutex<VecDeque<OfflineEvent>>,
    playback: bool,
    pub address: String,
    pub username: String,
    pub version: String,
}

impl OfflineConnector {
    pub fn is_playback(&self) -> bool {
        self.playback
    }
    pub fn try_event(&self) -> Option<OfflineEvent> {
        self.deferred
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
            .or_else(|| self.events.try_recv().ok())
            .or_else(|| {
                self.latest_state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
            })
    }

    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    /// Send a bounded JSON replay control over the private owned pipe. Command
    /// validation and acknowledgements belong to the playback helper.
    pub fn control(&mut self, command: &serde_json::Value) -> Result<()> {
        let mut data = serde_json::to_vec(command)?;
        if data.len() > 1024 * 1024 - 1 {
            return Err(Error::Other("replay command exceeds 1 MiB".into()));
        }
        data.push(b'\n');
        self.child
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Other("local Bedrock session is stopped".into()))?
            .write_all(&data)
            .map_err(|e| Error::Other(format!("couldn't send replay control: {e}")))
    }
    /// Worker-only, correlated control of this already-running helper. Unlike
    /// periodic replay_state events, its reply cannot be coalesced or confused
    /// with a state published before the command. Other lifecycle events remain
    /// available through try_event; this never starts a helper or game.
    pub fn request_control(
        &mut self,
        command: &serde_json::Value,
        timeout: Duration,
    ) -> Result<ReplayControlReply> {
        if !self.playback {
            return Err(Error::Other(
                "correlated replay controls require a playback session".into(),
            ));
        }
        self.request_owned_control(command, timeout, "replay_control")
    }
    pub fn request_recording_control(
        &mut self,
        command: &serde_json::Value,
        timeout: Duration,
    ) -> Result<ReplayControlReply> {
        if self.playback {
            return Err(Error::Other(
                "recording control requires the existing live relay".into(),
            ));
        }
        self.request_owned_control(command, timeout, "recording_control")
    }
    fn request_owned_control(
        &mut self,
        command: &serde_json::Value,
        timeout: Duration,
        reply_event: &str,
    ) -> Result<ReplayControlReply> {
        if !(Duration::from_millis(500)..=Duration::from_secs(30)).contains(&timeout) {
            return Err(Error::Other(
                "replay control wait must be 500–30000 ms".into(),
            ));
        }
        let mut command = command.clone();
        let object = command
            .as_object_mut()
            .ok_or_else(|| Error::Other("replay control must be an object".into()))?;
        if object.contains_key("requestId") {
            return Err(Error::Other(
                "correlated replay control owns requestId".into(),
            ));
        }
        let operation = object
            .get("op")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| Error::Other("replay control has no operation".into()))?;
        let request_id = format!("replay-{}", uuid::Uuid::new_v4().simple());
        object.insert("requestId".into(), serde_json::json!(request_id));
        self.control(&command)?;
        let started = Instant::now();
        while started.elapsed() < timeout {
            let event = match self.events.recv_timeout(
                timeout
                    .saturating_sub(started.elapsed())
                    .min(Duration::from_millis(100)),
            ) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !self.running() {
                        return Err(Error::Other(
                            "replay helper stopped while waiting for a control reply".into(),
                        ));
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Other(
                        "replay helper output closed before its control reply".into(),
                    ));
                }
            };
            if event.request_id.as_deref() == Some(request_id.as_str()) {
                if event.event == "control_error" {
                    return Err(Error::Other(
                        event
                            .message
                            .unwrap_or_else(|| "replay control failed".into()),
                    ));
                }
                if event.event != reply_event
                    || event.operation.as_deref() != Some(operation.as_str())
                {
                    return Err(Error::Other(
                        "replay control reply has a different operation".into(),
                    ));
                }
                return Ok(ReplayControlReply {
                    request_id,
                    operation,
                    result: event
                        .result
                        .filter(|v| v.is_object())
                        .ok_or_else(|| Error::Other("replay control reply has no result".into()))?,
                    state: event
                        .state
                        .filter(|v| v.is_object())
                        .ok_or_else(|| Error::Other("replay control reply has no state".into()))?,
                });
            }
            // Consume the legacy duplicate of our own step. Older timed-out
            // correlated replies cannot satisfy this request and need no UI copy.
            if matches!(event.event.as_str(), "replay_control" | "recording_control")
                || event.event == "control_error" && event.request_id.is_some()
                || event.event == "replay_step"
                    && operation == "step"
                    && event.generation == command["generation"].as_u64()
                    && event.sequence == command["sequence"].as_u64()
            {
                continue;
            }
            let error = (event.event == "error").then(|| {
                event
                    .message
                    .clone()
                    .unwrap_or_else(|| "replay helper failed".into())
            });
            let mut deferred = self.deferred.lock().unwrap_or_else(|e| e.into_inner());
            if deferred.len() >= 1024 {
                return Err(Error::Other(
                    "replay worker must drain pending lifecycle events".into(),
                ));
            }
            deferred.push_back(event);
            if let Some(error) = error {
                return Err(Error::Other(error));
            }
        }
        Err(Error::Other(
            "replay helper did not return the correlated control reply before timeout".into(),
        ))
    }
    /// Finish a recording before releasing the local session, then collect its
    /// final save/error events. Call from a worker when used by a GUI.
    pub fn stop(&mut self) -> Result<Vec<OfflineEvent>> {
        self.shutdown()?;
        let mut events: Vec<_> = self
            .deferred
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        while let Ok(event) = self.events.recv_timeout(Duration::from_millis(100)) {
            events.push(event);
        }
        Ok(events)
    }
    fn shutdown(&mut self) -> Result<()> {
        self.child.stdin.take();
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(Error::Other(
                            "local Bedrock helper exited with an error".into(),
                        ))
                    };
                }
                Err(e) => {
                    return Err(Error::Other(format!(
                        "couldn't stop the local Bedrock helper: {e}"
                    )));
                }
                Ok(None) => {}
            }
            if Instant::now() >= until {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(Error::Other("Bedrock helper did not finish within 3 seconds; its recording may be incomplete".into()));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for OfflineConnector {
    fn drop(&mut self) {
        // Closing stdin lets the helper clean up its RakNet sockets and peers.
        let _ = self.shutdown();
    }
}

fn command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

/// Prepare the pinned protocol helper once, then start a loopback-only relay.
/// Requires Node.js 24+ with npm; the first start installs its pinned dependencies.
pub fn start(data_dir: &Path, options: &OfflineOptions) -> Result<OfflineConnector> {
    if options.record_to.as_ref().is_some_and(|p| !p.is_absolute()) {
        return Err(Error::Other(
            "replay output must be an absolute path".into(),
        ));
    }
    start_helper(
        data_dir,
        &serde_json::to_vec(options)?,
        false,
        &options.username,
        options.listen_port,
        "127.0.0.1",
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackOptions {
    pub file: PathBuf,
    pub listen_port: u16,
    pub speed: f64,
    pub start_ms: u32,
    pub transport: ReplayTransport,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayTransport {
    #[default]
    Raknet,
    Nethernet,
}

impl ReplayTransport {
    pub const fn host(self) -> &'static str {
        match self {
            Self::Raknet => "127.0.0.1",
            Self::Nethernet => "127.0.0.2",
        }
    }
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Raknet => 19143,
            Self::Nethernet => 7551,
        }
    }
}

/// Validate the local Bedrock recording before downloading/spawning a helper.
pub fn replay_metadata(file: &Path) -> Result<serde_json::Value> {
    let mut reader = std::fs::File::open(file).at(file)?;
    replay_metadata_file(&mut reader, file)
}

/// Validate the actual held replay descriptor instead of reopening its path.
pub(super) fn replay_metadata_file(
    reader: &mut std::fs::File,
    file: &Path,
) -> Result<serde_json::Value> {
    use std::io::{Read, Seek, SeekFrom};
    reader.seek(SeekFrom::Start(0)).at(file)?;
    let size = reader.metadata().at(file)?.len();
    if !(22..=2 * 1024 * 1024 * 1024).contains(&size) {
        return Err(Error::Other("invalid Bedrock replay size".into()));
    }
    let mut header = [0u8; 13];
    reader.read_exact(&mut header).at(file)?;
    if &header[..9] != b"ARCTICBR\x01" {
        return Err(Error::Other("not an Arctic Bedrock replay (.abr)".into()));
    }
    let length = u32::from_be_bytes(header[9..13].try_into().unwrap()) as usize;
    if length > 4096 || 13 + length as u64 + 9 > size {
        return Err(Error::Other("invalid Bedrock replay metadata size".into()));
    }
    let mut json = vec![0; length];
    reader.read_exact(&mut json).at(file)?;
    let metadata: serde_json::Value = serde_json::from_slice(&json)?;
    let name = metadata["username"].as_str().unwrap_or_default();
    let version = metadata["version"].as_str().unwrap_or_default();
    let parts: Vec<_> = version.split('.').collect();
    if metadata["format"] != 1
        || metadata["edition"] != "bedrock"
        || !(3..=16).contains(&name.len())
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        || !(3..=4).contains(&parts.len())
        || version.len() > 32
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(Error::Other("invalid Bedrock replay metadata".into()));
    }
    reader.seek(SeekFrom::End(-9)).at(file)?;
    let mut footer = [0; 9];
    reader.read_exact(&mut footer).at(file)?;
    if footer[4..] != [0; 5] || u32::from_be_bytes(footer[..4].try_into().unwrap()) > 86_400_000 {
        return Err(Error::Other("Bedrock replay was not finalized".into()));
    }
    Ok(metadata)
}

pub fn playback(data_dir: &Path, options: &PlaybackOptions) -> Result<OfflineConnector> {
    if options.listen_port == 0
        || !options.speed.is_finite()
        || !(MIN_REPLAY_SPEED..=MAX_REPLAY_SPEED).contains(&options.speed)
        || options.start_ms > 86_400_000
        || !options.file.is_absolute()
    {
        return Err(Error::Other(
            "replay needs an absolute file, nonzero port, speed 0.1–16 and a start within 24 hours"
                .into(),
        ));
    }
    let metadata = replay_metadata(&options.file)?;
    start_helper(
        data_dir,
        &serde_json::to_vec(options)?,
        true,
        metadata["username"].as_str().unwrap_or_default(),
        options.listen_port,
        options.transport.host(),
    )
}

fn start_helper(
    data_dir: &Path,
    config_bytes: &[u8],
    replay: bool,
    username: &str,
    listen_port: u16,
    listen_host: &str,
) -> Result<OfflineConnector> {
    let node = node_runtime()?;
    // A dependency update must not replace modules used by a live older session.
    let mut digest = Sha256::new();
    for source in [
        LOCK,
        SCRIPT,
        REPLAY,
        REPLAY_PACKS,
        REPLAY_SELF,
        RECORDING_CONTROL,
        CAMERA,
        REPLAY_NETHERNET,
        LOOPBACK_RTC,
    ] {
        digest.update(source.as_bytes());
    }
    let fingerprint = hex::encode(digest.finalize());
    let helper = data_dir
        .join("bedrock/offline-connector")
        .join(&fingerprint[..16]);
    prepare_helper(&node, &helper)?;
    // Each launch has its own file so two profiles cannot overwrite each other's
    // destination while their helper processes are starting.
    let config = helper.join(format!("session-{}.json", uuid::Uuid::new_v4().simple()));
    std::fs::write(&config, config_bytes).at(&config)?;
    let spawn = command(&node)
        .arg(helper.join("connector.cjs"))
        .arg(if replay {
            "--replay-config"
        } else {
            "--config"
        })
        .arg(&config)
        .current_dir(&helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(e) => {
            let _ = std::fs::remove_file(&config);
            return Err(Error::io(&helper, e));
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Other("offline connector has no output pipe".into()))?;
    let (sender, events) = mpsc::channel();
    // The launcher may be on another tab for hours. Coalesce clock updates
    // without dropping recording/export/error events or buffering every tick.
    let latest_state = Arc::new(Mutex::new(None));
    let state_writer = Arc::clone(&latest_state);
    std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let Some(payload) = line.strip_prefix(PREFIX) else {
                continue;
            };
            if let Ok(event) = serde_json::from_str::<OfflineEvent>(payload) {
                if event.event == "replay_state" {
                    *state_writer.lock().unwrap_or_else(|e| e.into_inner()) = Some(event);
                } else {
                    // A queued readiness snapshot must not be delivered after
                    // a connection/world transition from the same pipe.
                    if matches!(
                        event.event.as_str(),
                        "replay_joined" | "replay_reset" | "replay_disconnected" | "error"
                    ) {
                        let mut state = state_writer.lock().unwrap_or_else(|e| e.into_inner());
                        // A retiring transfer peer can close after the next
                        // generation initialized; preserve that newer snapshot.
                        if event.generation.is_none_or(|generation| {
                            state
                                .as_ref()
                                .and_then(|s: &OfflineEvent| s.generation)
                                .is_none_or(|current| generation >= current)
                        }) {
                            state.take();
                        }
                    }
                    if sender.send(event).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut session = OfflineConnector {
        child,
        events,
        latest_state,
        deferred: Mutex::new(VecDeque::new()),
        playback: replay,
        address: String::new(),
        username: username.into(),
        version: String::new(),
    };
    let started = Instant::now();
    let result = loop {
        match session.events.recv_timeout(Duration::from_millis(100)) {
            Ok(event) if event.event == "ready" => {
                if event.xbox_auth != Some(false) {
                    break Err(Error::Other(
                        "offline connector did not confirm unsigned login mode".into(),
                    ));
                }
                let expected = format!("{listen_host}:{listen_port}");
                if event.address.as_deref() != Some(&expected)
                    || event.username.as_deref() != Some(username)
                {
                    break Err(Error::Other(
                        "offline connector returned an unexpected local endpoint or identity"
                            .into(),
                    ));
                }
                session.address = expected;
                session.version = event.version.unwrap_or_default();
                break Ok(());
            }
            Ok(event) if event.event == "error" => {
                break Err(Error::Other(
                    event
                        .message
                        .unwrap_or_else(|| "offline connector failed".into()),
                ));
            }
            _ => {}
        }
        if !session.running() {
            break Err(Error::Other(
                "offline connector exited before it was ready".into(),
            ));
        }
        if started.elapsed() >= Duration::from_secs(30) {
            break Err(Error::Other(
                "offline connector did not become ready within 30 seconds".into(),
            ));
        }
    };
    let _ = std::fs::remove_file(&config);
    result?;
    Ok(session)
}

fn node_runtime() -> Result<PathBuf> {
    let output = command("node")
        .args([
            "-p",
            "JSON.stringify({exe:process.execPath,major:+process.versions.node.split('.')[0]})",
        ])
        .output()
        .map_err(|_| {
            Error::Other(
                "offline Bedrock connections need Node.js 24 or newer with npm installed".into(),
            )
        })?;
    let info: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| Error::Other("couldn't inspect the Node.js runtime".into()))?;
    if !output.status.success() || info["major"].as_u64().unwrap_or(0) < 24 {
        return Err(Error::Other(
            "offline Bedrock connections need Node.js 24 or newer".into(),
        ));
    }
    info["exe"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| Error::Other("couldn't locate the Node.js executable".into()))
}

fn prepare_helper(node: &Path, helper: &Path) -> Result<()> {
    std::fs::create_dir_all(helper).at(helper)?;
    let lock = helper.join("package-lock.json");
    let package = helper.join("package.json");
    let installed = helper.join(".arctic-installed-lock.json");
    let install = std::fs::read_to_string(&installed).ok().as_deref() != Some(LOCK)
        || !helper
            .join("node_modules/bedrock-protocol/package.json")
            .is_file();
    std::fs::write(helper.join("connector.cjs"), SCRIPT).at(helper)?;
    std::fs::write(helper.join("replay.cjs"), REPLAY).at(helper)?;
    std::fs::write(helper.join("replay-packs.cjs"), REPLAY_PACKS).at(helper)?;
    std::fs::write(helper.join("replay-self.cjs"), REPLAY_SELF).at(helper)?;
    std::fs::write(helper.join("recording-control.cjs"), RECORDING_CONTROL).at(helper)?;
    std::fs::write(helper.join("camera.cjs"), CAMERA).at(helper)?;
    std::fs::write(helper.join("replay-nethernet.cjs"), REPLAY_NETHERNET).at(helper)?;
    std::fs::write(helper.join("loopback-rtc.cjs"), LOOPBACK_RTC).at(helper)?;
    std::fs::write(&package, PACKAGE).at(&package)?;
    std::fs::write(&lock, LOCK).at(&lock)?;
    if install {
        let mut npm = if cfg!(windows) {
            let cli = node
                .parent()
                .unwrap_or(Path::new("."))
                .join("node_modules/npm/bin/npm-cli.js");
            if !cli.is_file() {
                return Err(Error::Other(
                    "install Node.js with npm to prepare offline Bedrock connections".into(),
                ));
            }
            let mut cmd = command(node);
            cmd.arg(cli);
            cmd
        } else {
            command("npm")
        };
        let output = npm
            .args([
                "ci",
                "--ignore-scripts",
                "--omit=optional",
                "--no-audit",
                "--no-fund",
            ])
            .current_dir(helper)
            .output()
            .at(helper)?;
        if !output.status.success() {
            return Err(Error::Other(format!(
                "couldn't install the offline Bedrock protocol helper: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        std::fs::write(&installed, LOCK).at(&installed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_events_preserve_generation_and_step_correlations_without_world_claims() {
        let event: OfflineEvent = serde_json::from_value(serde_json::json!({
            "event":"replay_step","generation":2,"sequence":7,"positionMs":1500,
            "packetsQueued":true,"nativeWorldVerified":false,"xboxAuth":false
        }))
        .unwrap();
        assert_eq!(event.generation, Some(2));
        assert_eq!(event.sequence, Some(7));
        assert_eq!(event.position_ms, Some(1500));
        assert_eq!(event.packets_queued, Some(true));
        assert_eq!(event.native_world_verified, Some(false));
        let state: OfflineEvent = serde_json::from_value(serde_json::json!({
            "event":"replay_state","generation":2,"connectionRequired":true,"packetReady":false,
            "pendingStep":{"generation":2,"sequence":8,"timeMs":2000}
        }))
        .unwrap();
        assert_eq!(state.connection_required, Some(true));
        assert_eq!(state.packet_ready, Some(false));
        let pending = state.pending_step.unwrap();
        assert_eq!(
            (pending.generation, pending.sequence, pending.time_ms),
            (2, 8, 2000)
        );
    }

    #[test]
    fn camera_paths_validate_before_saving_and_use_the_helper_schema() {
        let mut path = CameraPath {
            format: 1,
            keyframes: vec![CameraKeyframe {
                time_ms: 0,
                pose: CameraPose::default(),
                ease: "smooth".into(),
            }],
        };
        path.validate().unwrap();
        let json = serde_json::to_value(&path).unwrap();
        assert_eq!(json["keyframes"][0]["timeMs"], 0);
        assert_eq!(json["keyframes"][0]["pose"]["fov"], 70.0);
        let implicit: CameraPath =
            serde_json::from_value(serde_json::json!({"format":1,"keyframes":[
                {"timeMs":0,"pose":{"x":0,"y":80,"z":0,"yaw":0,"pitch":0}}
            ]}))
            .unwrap();
        implicit.validate().unwrap();
        assert_eq!(implicit.keyframes[0].ease, "linear");
        assert_eq!(implicit.keyframes[0].pose.fov, 70.0);
        path.keyframes.push(path.keyframes[0].clone());
        assert!(path.validate().is_err());
        path.keyframes[1].time_ms = 100;
        path.validate().unwrap();
        path.keyframes[1].pose.fov = f64::NAN;
        assert!(path.validate().is_err());
        assert!(
            serde_json::from_value::<CameraPath>(
                serde_json::json!({"format":1,"keyframes":[],"token":"no"})
            )
            .is_err()
        );
    }

    #[test]
    fn offline_options_require_a_usable_name_address_and_local_port() {
        assert!(OfflineOptions::new("play.example.net", "Alex", 19142, None).is_ok());
        assert!(OfflineOptions::new("play.example.net", "../name", 19142, None).is_err());
        assert!(OfflineOptions::new("127.0.0.1:19142", "Alex", 19142, None).is_err());
        assert!(OfflineOptions::new("play.example.net", "Alex", 0, None).is_err());
        let json = serde_json::to_value(
            OfflineOptions::new("play.example.net", "Guest_1", 19142, None).unwrap(),
        )
        .unwrap();
        assert_eq!(json["listenPort"], 19142);
        assert!(json.get("accessToken").is_none());
    }
    #[test]
    fn validates_replay_metadata_before_starting_node() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("world.abr");
        let json=serde_json::to_vec(&serde_json::json!({"format":1,"edition":"bedrock","username":"ArcticTest","version":"1.26.51"})).unwrap();
        let mut data = b"ARCTICBR\x01".to_vec();
        data.extend_from_slice(&(json.len() as u32).to_be_bytes());
        data.extend_from_slice(&json);
        data.extend_from_slice(&[0; 9]);
        std::fs::write(&file, &data).unwrap();
        assert_eq!(replay_metadata(&file).unwrap()["username"], "ArcticTest");
        data[0] = 0;
        std::fs::write(&file, &data).unwrap();
        assert!(replay_metadata(&file).is_err());
        data[0] = b'A';
        data[9..13].copy_from_slice(&4097u32.to_be_bytes());
        std::fs::write(&file, &data).unwrap();
        assert!(replay_metadata(&file).is_err());
        assert!(
            playback(
                dir.path(),
                &PlaybackOptions {
                    file,
                    listen_port: 19143,
                    speed: f64::NAN,
                    start_ms: 0,
                    transport: ReplayTransport::Raknet,
                }
            )
            .is_err()
        );
    }
}
