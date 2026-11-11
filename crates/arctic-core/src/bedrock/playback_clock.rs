//! Worker-owned normal replay clock coordination. A protocol marker correlates
//! one helper viewer with the native dispatcher; it does not prove simulation,
//! terrain readiness, camera presentation or deterministic video fidelity.
use super::{
    Edition,
    client::{self, NativeClockControl, NativeClockOperation},
    export_clock::{FeedPhase, FeedPoint},
    export_feed::{ReplayQueue, ViewerIdentity},
    offline::{OfflineConnector, ReplayControlReply, ReplayStep},
    packet::{self, PacketArm},
};
use crate::{Error, Result, error::IoContext};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const LEASE_MS: u32 = 5000;
const RENEW_AFTER: Duration = Duration::from_millis(1000);
fn fail(message: &str) -> Error {
    Error::Other(message.into())
}
fn report(path: &Path) -> Result<Value> {
    let file = std::fs::File::open(path).at(path)?;
    if !file
        .metadata()
        .at(path)?
        .modified()
        .at(path)?
        .elapsed()
        .is_ok_and(|age| age < Duration::from_secs(2))
    {
        return Err(fail("native playback clock report is stale"));
    }
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes).at(path)?;
    if bytes.len() > 65536 {
        return Err(fail("native playback clock report exceeds 64 KiB"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
struct Native {
    edition: Edition,
    bundle: PathBuf,
    pid: u32,
    session: String,
}
impl Native {
    #[cfg(windows)]
    fn attach(edition: Edition, bundle: &Path) -> Result<Self> {
        let bundle = client::validate_bundle(bundle)?;
        let installed = super::find(edition).ok_or_else(|| fail("Bedrock isn't installed"))?;
        let pid = super::package::process_id(edition.package_family(), &installed.path)
            .ok_or_else(|| fail("playback clock needs the already-running native client"))?;
        if !std::fs::read_to_string(bundle.join(format!("arctic-bedrock-{pid}.ready")))
            .is_ok_and(|value| value.trim() == "native-preview")
        {
            return Err(fail("this native bundle is not loaded"));
        }
        let clock = report(&bundle.join(format!("arctic-bedrock-{pid}.clock.json")))?;
        let prefix = format!("native_{pid}_");
        let session = clock["session"]
            .as_str()
            .filter(|session| {
                session.len() <= 64
                    && session
                        .strip_prefix(prefix.as_str())
                        .and_then(|epoch| epoch.parse::<u64>().ok())
                        .is_some_and(|epoch| epoch > 0)
            })
            .ok_or_else(|| fail("native playback clock has no current world session"))?
            .to_owned();
        let native = Self {
            edition,
            bundle,
            pid,
            session,
        };
        native.clock()?;
        Ok(native)
    }
    #[cfg(not(windows))]
    fn attach(_: Edition, _: &Path) -> Result<Self> {
        Err(fail("native playback clock requires Windows"))
    }
    fn clock(&self) -> Result<Value> {
        let value = report(
            &self
                .bundle
                .join(format!("arctic-bedrock-{}.clock.json", self.pid)),
        )?;
        if value["session"].as_str() != Some(self.session.as_str())
            || value["bindingReady"].as_bool() != Some(true)
            || value["installed"].as_bool() != Some(true)
            || value["controlEnabled"].as_bool() != Some(true)
            || value["rateRestorationFailed"].as_bool() != Some(false)
        {
            return Err(fail(
                "native playback clock binding changed or restoration failed",
            ));
        }
        Ok(value)
    }
    fn idle(value: &Value) -> bool {
        value["frozen"].as_bool() == Some(false)
            && value["queued"].is_null()
            && value["pendingWorldTick"].is_null()
            && value["waitingForRealTimer"].as_bool() == Some(false)
            && value["ratePendingSim"].as_bool() == Some(false)
            && value["ratePendingReal"].as_bool() == Some(false)
    }
    fn sequence(value: &Value) -> Result<u64> {
        value["sequence"]
            .as_u64()
            .filter(|sequence| *sequence <= 1_000_000_000)
            .ok_or_else(|| fail("native playback clock sequence is unavailable or exhausted"))
    }
}

/// Own on a worker together with the existing OfflineConnector. All helper
/// operations and service calls are serialized by its mutable borrow. Drop
/// performs best-effort owned release; the native lease also expires on its
/// own. Do not drop or call this blocking controller on the UI/render thread.
pub struct PlaybackClock {
    native: Native,
    viewer: ViewerIdentity,
    helper_sequence: u64,
    receipt: Option<Value>,
    rate: f32,
    renewed: Instant,
    failed: bool,
}
impl PlaybackClock {
    /// Bind one initialized replay viewer via a unique dispatch-only marker.
    /// Temporarily pauses/catches up the helper, without launching anything.
    /// On failure it can remain paused; no uncorrelated world is resumed.
    pub fn attach(helper: &mut OfflineConnector, edition: Edition, bundle: &Path) -> Result<Self> {
        if !helper.is_playback() {
            return Err(fail("native playback clock needs a replay connection"));
        }
        let native = Native::attach(edition, bundle)?;
        let before = native.clock()?;
        if !Native::idle(&before) || !before["playbackRate"].is_null() {
            return Err(fail("another operation already owns the native clock"));
        }
        let sequence = Native::sequence(&before)?;
        if sequence >= 999_999_999 {
            return Err(fail(
                "native playback clock has no remaining application/release sequence budget",
            ));
        }
        let initial = helper.request_control(&json!({"op":"state"}), Duration::from_secs(5))?;
        let initial_viewer = viewer(&initial.state)?;
        let (paused, speed) = timing(&initial.state)?;
        let packets = report(
            &native
                .bundle
                .join(format!("arctic-bedrock-{}.packet.json", native.pid)),
        )?;
        if packets["session"].as_str() != Some(native.session.as_str())
            || packets["contextLive"].as_bool() != Some(true)
            || packets["installed"].as_bool() != Some(true)
            || !packets["pending"].is_null()
        {
            return Err(fail(
                "native playback marker has no idle current-world binding",
            ));
        }
        let marker_sequence = Native::sequence(&packets)?
            .checked_add(1)
            .filter(|sequence| *sequence <= 1_000_000_000)
            .ok_or_else(|| fail("native replay marker sequence is exhausted"))?;
        let mut queue = ReplayQueue::attach(helper, Duration::from_secs(5))?;
        if *queue.identity() != initial_viewer {
            return Err(fail("replay viewer changed while binding its native clock"));
        }
        let time = queue.position_ms();
        let arm = PacketArm::new(
            &native.session,
            marker_sequence,
            &ReplayStep {
                generation: initial_viewer.generation,
                sequence: queue.next_sequence(),
                time_ms: time,
            },
        )?
        .dispatch_only();
        packet::arm(edition, &native.bundle, &arm)?;
        queue.queue_marked(
            FeedPoint {
                phase: FeedPhase::BeforeFrame,
                frame: 0,
                held_tick: 0,
                replay_time_ms: time,
                time_remainder: 0,
                time_denominator: 1,
            },
            &arm,
            Duration::from_secs(10),
        )?;
        packet::wait_dispatch(edition, &native.bundle, &arm)?;
        queue.confirm(Duration::from_secs(5))?;
        let helper_sequence = queue.next_sequence() - 1;
        drop(queue);
        let current = native.clock()?;
        if Native::sequence(&current)? != sequence
            || !Native::idle(&current)
            || !current["playbackRate"].is_null()
        {
            return Err(fail("native clock changed during replay correlation"));
        }
        let mut owned = Self {
            native,
            viewer: initial_viewer,
            helper_sequence,
            receipt: None,
            rate: 1.,
            renewed: Instant::now(),
            failed: false,
        };
        owned.apply(if paused { 0. } else { speed }, sequence)?;
        let reply = helper.request_control(
            &json!({"op":"pause","paused":paused}),
            Duration::from_secs(5),
        )?;
        owned.same_helper(&reply.state)?;
        if timing(&reply.state)? != (paused, speed)
            || reply.state["speed"].as_f64() != initial.state["speed"].as_f64()
        {
            return Err(fail(
                "replay timing changed while acquiring its native clock",
            ));
        }
        Ok(owned)
    }
    fn same_helper(&self, state: &Value) -> Result<()> {
        if viewer(state)? != self.viewer
            || state["lastStepSequence"].as_u64() != Some(self.helper_sequence)
        {
            return Err(fail(
                "native playback clock lost its original replay viewer/step",
            ));
        }
        Ok(())
    }
    fn owned(&self) -> Result<Value> {
        let current = self.native.clock()?;
        let receipt = self
            .receipt
            .as_ref()
            .ok_or_else(|| fail("no native playback rate is owned"))?;
        if Native::sequence(&current)?
            != receipt["sequence"]
                .as_u64()
                .ok_or_else(|| fail("owned clock receipt has no sequence"))?
            || current["last"]["requestId"] != receipt["requestId"]
            || !Native::idle(&current)
            || current["last"]["rateApplied"].as_bool() != Some(true)
            || !current["playbackRate"]
                .as_f64()
                .is_some_and(|rate| (rate as f32).to_bits() == self.rate.to_bits())
            || !current["leaseRemainingMs"]
                .as_u64()
                .is_some_and(|left| left > 0)
        {
            return Err(fail(
                "native playback rate changed, expired or belongs to another controller",
            ));
        }
        Ok(current)
    }
    fn apply(&mut self, rate: f32, sequence: u64) -> Result<Value> {
        if sequence >= 999_999_999 {
            return Err(fail(
                "native playback clock must reserve a sequence for release",
            ));
        }
        let receipt = if let Some(owner) = self.receipt.as_ref() {
            client::clock_control_owned(
                self.native.edition,
                &self.native.bundle,
                &self.native.session,
                sequence + 1,
                NativeClockControl {
                    op: NativeClockOperation::Rate,
                    partial: 0.,
                    real_timer: true,
                    lease_ms: LEASE_MS,
                    rate: Some(rate),
                },
                owner,
            )?
        } else {
            let idle = self.native.clock()?;
            if Native::sequence(&idle)? != sequence {
                return Err(fail("native clock changed before playback acquisition"));
            }
            client::clock_control_from_idle(
                self.native.edition,
                &self.native.bundle,
                &self.native.session,
                sequence + 1,
                NativeClockControl {
                    op: NativeClockOperation::Rate,
                    partial: 0.,
                    real_timer: true,
                    lease_ms: LEASE_MS,
                    rate: Some(rate),
                },
                &idle,
            )?
        };
        self.rate = rate;
        self.renewed = Instant::now();
        self.receipt = Some(receipt.clone());
        Ok(receipt)
    }
    /// Forward a speed/pause change only after checking both owned endpoints.
    /// Native application is acknowledged before returning. A failure poisons
    /// this coordinator; elapsed-time alignment still requires live validation.
    pub fn control(
        &mut self,
        helper: &mut OfflineConnector,
        command: &Value,
    ) -> Result<ReplayControlReply> {
        if self.failed {
            return Err(fail(
                "native playback clock failed; release it before reacquiring",
            ));
        }
        let result = (|| {
            let op = command["op"]
                .as_str()
                .ok_or_else(|| fail("playback clock command has no operation"))?;
            let valid = match op {
                "pause" => {
                    command
                        .as_object()
                        .is_some_and(|object| object.len() == 2 && object.contains_key("paused"))
                        && command["paused"].is_boolean()
                }
                "speed" => {
                    command
                        .as_object()
                        .is_some_and(|object| object.len() == 2 && object.contains_key("speed"))
                        && command["speed"]
                            .as_f64()
                            .is_some_and(|speed| speed.is_finite() && (0.1..=16.).contains(&speed))
                }
                _ => false,
            };
            if !valid {
                return Err(fail(
                    "native playback clock accepts only pause/speed controls",
                ));
            }
            let before = helper.request_control(&json!({"op":"state"}), Duration::from_secs(5))?;
            self.same_helper(&before.state)?;
            let current = self.owned()?;
            let reply = helper.request_control(command, Duration::from_secs(5))?;
            self.same_helper(&reply.state)?;
            let expected_pause = if op == "pause" {
                command["paused"].as_bool()
            } else {
                before.state["paused"].as_bool()
            };
            let expected_speed = if op == "speed" {
                command["speed"].as_f64()
            } else {
                before.state["speed"].as_f64()
            };
            if reply.state["paused"].as_bool() != expected_pause
                || reply.state["speed"].as_f64() != expected_speed
            {
                return Err(fail(
                    "replay helper did not accept the requested timing change",
                ));
            }
            let (paused, speed) = timing(&reply.state)?;
            self.apply(if paused { 0. } else { speed }, Native::sequence(&current)?)?;
            Ok(reply)
        })();
        if result.is_err() {
            self.failed = true;
            let _ = self.release();
        }
        result
    }
    /// Call regularly on the owner worker. Fresh correlated helper state drives
    /// renewal; cached UI events cannot keep a clock lease alive indefinitely.
    pub fn service(&mut self, helper: &mut OfflineConnector) -> Result<Option<Value>> {
        if self.failed {
            return Err(fail("native playback clock is no longer coordinated"));
        }
        if self.renewed.elapsed() < RENEW_AFTER {
            return Ok(None);
        }
        let result = (|| {
            let mut reply =
                helper.request_control(&json!({"op":"state"}), Duration::from_secs(5))?;
            self.same_helper(&reply.state)?;
            let current = self.owned()?;
            let (mut paused, speed) = timing(&reply.state)?;
            // Match Java's end-of-replay pause for this one bound viewer.
            if !paused
                && reply.state["packetReady"].as_bool() == Some(true)
                && reply.state["positionMs"]
                    .as_u64()
                    .zip(reply.state["durationMs"].as_u64())
                    .is_some_and(|(at, end)| at >= end)
            {
                reply = helper.request_control(
                    &json!({"op":"pause","paused":true}),
                    Duration::from_secs(5),
                )?;
                self.same_helper(&reply.state)?;
                paused = timing(&reply.state)?.0;
                if !paused {
                    return Err(fail(
                        "replay helper did not acknowledge its end-of-replay pause",
                    ));
                }
            }
            self.apply(if paused { 0. } else { speed }, Native::sequence(&current)?)
                .map(Some)
        })();
        if result.is_err() {
            self.failed = true;
            let _ = self.release();
        }
        result
    }
    /// Release only this controller's acknowledged request/sequence. It never
    /// resumes a replacement world, export freeze or another rate controller.
    pub fn release(&mut self) -> Result<()> {
        self.failed = true;
        let Some(receipt) = self.receipt.as_ref() else {
            return Ok(());
        };
        let current = self.native.clock()?;
        if Native::sequence(&current)? == receipt["sequence"].as_u64().unwrap_or(u64::MAX)
            && Native::idle(&current)
            && current["playbackRate"].is_null()
        {
            self.receipt = None;
            return Ok(());
        }
        let current = self.owned()?;
        client::resume_owned_clock(
            self.native.edition,
            &self.native.bundle,
            &self.native.session,
            Native::sequence(&current)? + 1,
            receipt,
        )?;
        self.receipt = None;
        Ok(())
    }
    pub fn status(&self) -> Value {
        json!({"nativeSession":self.native.session,"viewerId":self.viewer.viewer_id,"generation":self.viewer.generation,
            "rate":self.rate,"active":self.receipt.is_some()&&!self.failed,
            "nativePlaybackSynchronizedVerified":false,"deterministicVideoVerified":false})
    }
}
impl Drop for PlaybackClock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
fn viewer(state: &Value) -> Result<ViewerIdentity> {
    if state["event"].as_str() != Some("replay_state")
        || state["viewers"].as_u64() != Some(1)
        || state["initializedViewers"].as_u64() != Some(1)
        || state["clientInitialized"].as_bool() != Some(true)
        || state["connectionRequired"].as_bool() != Some(false)
        || !state["pendingStep"].is_null()
    {
        return Err(fail(
            "native playback clock requires one initialized replay viewer without a pending step",
        ));
    }
    let generation = state["generation"]
        .as_u64()
        .filter(|generation| (1..=1_000_000_000).contains(generation))
        .ok_or_else(|| fail("replay has no valid generation"))?;
    let identities = state["viewerIdentities"]
        .as_array()
        .filter(|identities| identities.len() == 1)
        .ok_or_else(|| fail("replay has no unique viewer identity"))?;
    let current = &identities[0];
    if current["generation"].as_u64() != Some(generation)
        || current["clientInitialized"].as_bool() != Some(true)
    {
        return Err(fail("replay viewer is not initialized in this generation"));
    }
    let id = |key: &str| {
        current[key]
            .as_str()
            .filter(|id| uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == *id))
            .map(str::to_owned)
            .ok_or_else(|| fail("replay viewer has an invalid UUID"))
    };
    Ok(ViewerIdentity {
        generation,
        viewer_id: id("viewerId")?,
        player_uuid: id("playerUuid")?,
    })
}
fn timing(state: &Value) -> Result<(bool, f32)> {
    let paused = state["paused"]
        .as_bool()
        .ok_or_else(|| fail("replay pause state is unknown"))?;
    let speed = state["speed"]
        .as_f64()
        .filter(|speed| speed.is_finite() && (0.1..=16.).contains(speed))
        .ok_or_else(|| fail("replay speed is outside 0.1..16"))?;
    Ok((paused, speed as f32))
}
