//! Correlate replay markers with a native main-thread dispatch and later tick.
//! These receipts do not establish packet application order or terrain readiness.
use super::Edition;
#[cfg(windows)]
use crate::error::IoContext;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PacketArm {
    #[serde(default = "schema")]
    pub schema: u32,
    pub session: String,
    pub sequence: u64,
    #[serde(default = "request_id")]
    pub request_id: String,
    #[serde(default = "marker")]
    pub marker: String,
    pub generation: u64,
    pub replay_sequence: u64,
    pub time_ms: u32,
    #[serde(default)]
    pub time_remainder: u32,
    #[serde(default = "unit_denominator")]
    pub time_denominator: u32,
    #[serde(default)]
    pub completion: Completion,
    #[serde(default = "timeout")]
    pub timeout_ms: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    Dispatch,
    #[default]
    ClientTick,
}
impl Completion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::ClientTick => "client_tick",
        }
    }
}
fn schema() -> u32 {
    1
}
fn unit_denominator() -> u32 {
    1
}
fn timeout() -> u32 {
    15000
}
fn request_id() -> String {
    format!(
        "packet-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    )
}
fn marker() -> String {
    format!("37c1{}", &uuid::Uuid::new_v4().simple().to_string()[..12])
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
impl PacketArm {
    pub fn new(session: &str, sequence: u64, step: &super::offline::ReplayStep) -> Result<Self> {
        let request = Self {
            schema: 1,
            session: session.into(),
            sequence,
            request_id: request_id(),
            marker: marker(),
            generation: step.generation,
            replay_sequence: step.sequence,
            time_ms: step.time_ms,
            time_remainder: 0,
            time_denominator: 1,
            completion: Completion::ClientTick,
            timeout_ms: timeout(),
        };
        request.validate()?;
        Ok(request)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.schema != 1
            || !token(&self.session)
            || !token(&self.request_id)
            || ![self.sequence, self.generation, self.replay_sequence]
                .iter()
                .all(|s| (1..=1_000_000_000).contains(s))
            || self.time_ms > 86_400_000
            || !(1..=1000).contains(&self.time_denominator)
            || self.time_remainder >= self.time_denominator
            || self.time_ms == 86_400_000 && self.time_remainder != 0
            || !(500..=30000).contains(&self.timeout_ms)
            || self.marker.len() != 16
            || !self.marker.starts_with("37c1")
            || !self
                .marker
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(Error::Other(
                "invalid native packet-marker session, sequence, identity, replay step or timeout"
                    .into(),
            ));
        }
        Ok(())
    }
    /// Send only after `arm` succeeds. Queue completion remains separate from
    /// the native receipt returned by `wait`.
    pub fn replay_command(&self) -> Value {
        serde_json::json!({"op":"step","generation":self.generation,"sequence":self.replay_sequence,
            "timeMs":self.time_ms,"marker":self.marker})
    }
    pub fn dispatch_only(mut self) -> Self {
        self.completion = Completion::Dispatch;
        self
    }
    /// Preserve the frame fraction on the native marker, while packet selection
    /// still uses time_ms's floor. Camera and cosmetic samples share this value.
    pub fn with_fraction(mut self, remainder: u32, denominator: u32) -> Result<Self> {
        self.time_remainder = remainder;
        self.time_denominator = denominator;
        self.validate()?;
        Ok(self)
    }
}

/// Admit a marker against the already-loaded client's current world session.
/// Does not send a packet, step a clock, or launch the game.
#[cfg(windows)]
pub fn arm(edition: Edition, bundle: &Path, request: &PacketArm) -> Result<Value> {
    use std::io::Write;
    request.validate()?;
    let endpoint = Endpoint::attach(edition, bundle)?;
    let bytes = serde_json::to_vec(request)?;
    let temporary = endpoint.bundle.join(format!(".{}.tmp", request.request_id));
    let published = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .at(&temporary)?;
        file.write_all(&bytes).at(&temporary)?;
        drop(file);
        std::fs::hard_link(&temporary, &endpoint.request).at(&endpoint.request)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    published?;
    let result = endpoint.wait(request, Stage::Armed);
    if result.is_err() && std::fs::read(&endpoint.request).is_ok_and(|v| v == bytes) {
        let _ = std::fs::remove_file(&endpoint.request);
    }
    result
}

/// Require the marker's native dispatcher return and a later completed client
/// tick in the same world session. The caller must permit that tick; a frozen
/// clock is not advanced or resumed by this wait.
#[cfg(windows)]
pub fn wait(edition: Edition, bundle: &Path, request: &PacketArm) -> Result<Value> {
    request.validate()?;
    if request.completion != Completion::ClientTick {
        return Err(Error::Other(
            "a dispatch-only marker has no post-tick completion".into(),
        ));
    }
    Endpoint::attach(edition, bundle)?.wait(request, Stage::ClientTick)
}
/// Require the primary-thread dispatcher return without advancing a frozen
/// clock. A controller can now permit its already-planned tick, then call wait.
#[cfg(windows)]
pub fn wait_dispatch(edition: Edition, bundle: &Path, request: &PacketArm) -> Result<Value> {
    request.validate()?;
    Endpoint::attach(edition, bundle)?.wait(request, Stage::Dispatch)
}
#[cfg(not(windows))]
pub fn arm(_: Edition, _: &Path, _: &PacketArm) -> Result<Value> {
    Err(Error::Other("native packet markers require Windows".into()))
}
#[cfg(not(windows))]
pub fn wait(_: Edition, _: &Path, _: &PacketArm) -> Result<Value> {
    Err(Error::Other("native packet markers require Windows".into()))
}
#[cfg(not(windows))]
pub fn wait_dispatch(_: Edition, _: &Path, _: &PacketArm) -> Result<Value> {
    Err(Error::Other("native packet markers require Windows".into()))
}

#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Armed,
    Dispatch,
    ClientTick,
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
        let bundle = super::native::validate_bundle(bundle)?;
        let installed =
            super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
        let pid = super::package::process_id(&installed.path, edition.package_family())
            .ok_or_else(|| {
                Error::Other(
                    "Bedrock must already be running with the native adapter loaded".into(),
                )
            })?;
        let ready = bundle.join(format!("arctic-bedrock-{pid}.ready"));
        if !std::fs::read_to_string(&ready).is_ok_and(|v| v.trim() == "native-preview") {
            return Err(Error::Other(
                "this native bundle is not loaded into the running game".into(),
            ));
        }
        Ok(Self {
            request: bundle.join(format!("arctic-bedrock-{pid}.packet-request.json")),
            report: bundle.join(format!("arctic-bedrock-{pid}.packet.json")),
            bundle,
        })
    }
    fn wait(&self, request: &PacketArm, stage: Stage) -> Result<Value> {
        use std::time::{Duration, Instant};
        let started = Instant::now();
        let limit = if stage == Stage::Armed {
            3000
        } else {
            u64::from(request.timeout_ms) + 2000
        };
        while started.elapsed() < Duration::from_millis(limit) {
            if let Some(status) = self.read() {
                for key in ["requestRejected", "last"] {
                    let value = &status[key];
                    if value["session"].as_str() != Some(request.session.as_str())
                        || value["sequence"].as_u64() != Some(request.sequence)
                        || value["requestId"].as_str() != Some(request.request_id.as_str())
                        || value["marker"].as_str() != Some(request.marker.as_str())
                    {
                        continue;
                    }
                    if let Some(error) = value["error"].as_str() {
                        return Err(Error::Other(error.into()));
                    }
                    if key != "last"
                        || status["session"].as_str() != Some(request.session.as_str())
                        || status["contextLive"].as_bool() != Some(true)
                        || value["completion"].as_str() != Some(request.completion.as_str())
                        || value["generation"].as_u64() != Some(request.generation)
                        || value["replaySequence"].as_u64() != Some(request.replay_sequence)
                        || value["timeMs"].as_u64() != Some(u64::from(request.time_ms))
                        || value["timeRemainder"]
                            .as_u64()
                            .or_else(|| value.get("timeRemainder").is_none().then_some(0))
                            != Some(u64::from(request.time_remainder))
                        || value["timeDenominator"]
                            .as_u64()
                            .or_else(|| value.get("timeDenominator").is_none().then_some(1))
                            != Some(u64::from(request.time_denominator))
                    {
                        continue;
                    }
                    if stage == Stage::Armed
                        && matches!(
                            value["stage"].as_str(),
                            Some("armed" | "dispatch_returned" | "client_tick_completed")
                        )
                    {
                        return Ok(value.clone());
                    }
                    let dispatched = matches!(
                        value["stage"].as_str(),
                        Some("dispatch_returned" | "client_tick_completed")
                    ) && value["dispatcherReturned"].as_bool() == Some(true)
                        && value["primaryClientThread"].as_bool() == Some(true)
                        && value["observationSequence"].as_u64().is_some_and(|s| s > 0)
                        && value["dispatcherTick"].as_u64().is_some();
                    if stage == Stage::Dispatch && dispatched {
                        return Ok(value.clone());
                    }
                    if stage == Stage::ClientTick
                        && dispatched
                        && value["stage"].as_str() == Some("client_tick_completed")
                        && value["postClientTickCompleted"].as_bool() == Some(true)
                        && value["dispatcherTick"]
                            .as_u64()
                            .zip(value["clientTick"].as_u64())
                            .is_some_and(|(a, b)| b > a)
                    {
                        return Ok(value.clone());
                    }
                }
                if status["session"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s != request.session)
                {
                    return Err(Error::Other(
                        "native packet-marker world session changed".into(),
                    ));
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(Error::Other(format!(
            "native packet marker did not reach its required stage; inspect {}",
            self.report.display()
        )))
    }
    fn read(&self) -> Option<Value> {
        use std::{io::Read, time::Duration};
        let file = std::fs::File::open(&self.report).ok()?;
        // Inspect the same opened file that is read, across native atomic renames.
        // Freshness is evidence of the control worker, not terrain readiness.
        if file.metadata().ok()?.modified().ok()?.elapsed().ok()? >= Duration::from_secs(2) {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1).read_to_end(&mut bytes).ok()?;
        (bytes.len() <= 64 * 1024)
            .then(|| serde_json::from_slice(&bytes).ok())
            .flatten()
    }
}
