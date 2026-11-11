//! Indexed replay video from an existing helper and already-loaded native client.
//! Native packet application/terrain and repeatable simulation remain unverified.
use std::{path::PathBuf, sync::atomic::AtomicBool};

use serde::{Deserialize, Serialize};
#[cfg(windows)]
use serde_json::Value;

use super::{
    Edition,
    offline::{CameraPath, OfflineConnector},
};
use crate::{Error, Result, error::IoContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportOptions {
    pub bundle: PathBuf,
    pub ffmpeg: PathBuf,
    #[serde(
        default,
        skip_serializing_if = "super::capture::EncoderKind::is_ffmpeg"
    )]
    pub encoder: super::capture::EncoderKind,
    pub output: PathBuf,
    pub fps: u32,
    pub frames: u32,
    pub camera_path: CameraPath,
    /// Rebuild at this recorded timestamp before indexed capture. None keeps
    /// the current-world workflow. This never launches a client or helper.
    #[serde(default)]
    pub start_ms: Option<u32>,
    /// Explicit inclusive range. Frame-count exports keep this absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<super::export_range::ExportRange>,
    #[serde(default)]
    pub appearance_snapshot: Option<PathBuf>,
    #[serde(default = "settle_frames")]
    pub settle_frames: u32,
}
fn settle_frames() -> u32 {
    3
}
impl ExportOptions {
    pub fn set_range(&mut self, range: super::export_range::ExportRange) -> Result<()> {
        let frames = range.frames(self.fps)?;
        self.start_ms = Some(range.from_ms);
        self.frames = frames;
        self.range = Some(range);
        Ok(())
    }
    /// File/metadata validation only; does not start an encoder, helper or game.
    pub fn validate(&mut self) -> Result<()> {
        self.camera_path.validate()?;
        if !(1..=60).contains(&self.fps)
            || self.frames == 0
            || self.frames > self.fps * 7200 + 1
            || !(1..=8).contains(&self.settle_frames)
            || self.start_ms.is_some_and(|time| time > 86_400_000)
        {
            return Err(Error::Other("indexed replay export needs FPS 1–60, at least one frame over a replay span up to two hours, and 1–8 settling frames".into()));
        }
        if let Some(range) = self.range
            && (self.start_ms != Some(range.from_ms)
                || !range.matches(self.fps, self.frames, range.from_ms)?)
        {
            return Err(Error::Other(
                "selected export range disagrees with its start/frame count".into(),
            ));
        }
        self.bundle = super::client::validate_bundle(&self.bundle)?;
        self.ffmpeg = self.ffmpeg.canonicalize().at(&self.ffmpeg)?;
        self.output = std::path::absolute(&self.output).at(&self.output)?;
        if let Some(snapshot) = self.appearance_snapshot.as_mut() {
            *snapshot = snapshot.canonicalize().at(&*snapshot)?;
            if !snapshot.is_file()
                || !snapshot
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".arctic-looks.zip"))
            {
                return Err(Error::Other(
                    "replay appearance needs an existing .arctic-looks.zip snapshot".into(),
                ));
            }
        }
        if !self.ffmpeg.is_file()
            || self.output.exists()
            || self.output.with_extension("frames.jsonl").exists()
            || self.output.with_extension("decoded.jsonl").exists()
            || self.output.with_extension("export.json").exists()
            || !self.output.parent().is_some_and(|p| p.is_dir())
            || self
                .output
                .extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("mp4"))
        {
            return Err(Error::Other("export needs an encoder executable and new MP4, frame manifest, decoded audit and export identity paths in an existing folder".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub stage: &'static str,
    pub frames_queued: u32,
    pub frames_total: u32,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldRebuildReceipt {
    pub generation: u64,
    pub old_viewer_id: String,
    pub new_viewer_id: String,
    pub player_uuid: String,
    pub native_pid: u32,
    pub previous_native_session: String,
    pub native_session: String,
    pub target_ms: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    pub capture_id: String,
    pub output: PathBuf,
    pub manifest: PathBuf,
    pub frames: u32,
    pub fps: u32,
    pub encoder: super::capture::EncoderKind,
    pub replay_origin_ms: u32,
    pub native_session: String,
    pub appearance_snapshot: Option<PathBuf>,
    pub appearance_manifest_hash: Option<String>,
    pub world_rebuild: Option<WorldRebuildReceipt>,
    pub decoded_video_audit: Option<super::video_audit::VideoAudit>,
    pub export_receipt: Option<PathBuf>,
    pub native_world_verified: bool,
    pub deterministic_video_verified: bool,
}

/// Run on a worker that exclusively owns the existing playback connection.
/// A requested start timestamp rebuilds the existing offline viewer through a
/// local transfer. No helper/client launch or sign-in is performed.
#[cfg(windows)]
pub fn run<F>(
    session: &mut OfflineConnector,
    edition: Edition,
    mut options: ExportOptions,
    cancel: &AtomicBool,
    mut progress: F,
) -> Result<ExportOutcome>
where
    F: FnMut(ExportProgress),
{
    use super::{
        capture::{self, CaptureControl, IndexedOptions},
        client::{self, NativeClockControl, NativeClockOperation},
        export_clock::{NativeExportClock, ProtocolFrameCapture, Timeline},
        export_feed::{self, NativeMarkerFeed, ReplayQueue},
    };
    use std::time::Duration;
    options.validate()?;
    canceled(cancel)?;
    let mut binding = binding(
        edition,
        &options.bundle,
        options.appearance_snapshot.is_some() || options.start_ms.is_some(),
    )?;
    let capture_id = format!(
        "export-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let timeout = Duration::from_secs(10);
    let mut clock: Option<NativeExportClock> = None;
    let mut freeze: Option<Value> = None;
    let mut clock_released = false;
    let mut capture_requested = false;
    let mut camera_attempted = false;
    let mut camera_identity = None;
    let mut identity = None;
    let mut last_sequence = 0;
    let mut appearance: Option<AppearanceLease> = None;
    let mut reset: Option<(u64, String)> = None;
    let mut world_rebuild = None;
    let mut export_intent = None;
    progress(ExportProgress {
        stage: "preparing",
        frames_queued: 0,
        frames_total: options.frames,
    });
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<ExportOutcome> {
        // Reserve capture before changing playback/camera/clock, so another
        // active recorder is rejected without modifying the connected replay.
        canceled(cancel)?;
        let reservation=capture::control(edition,&options.bundle,&capture_id,CaptureControl::Reserve {lease_ms:120000})?;
        capture_requested=true;
        capture::require_indexed_encoding(&reservation,options.encoder,false)?;
        if let Some(start)=options.start_ms {
            progress(ExportProgress {stage:"rebuilding_world",frames_queued:0,frames_total:options.frames});
            let (fresh,receipt)=rebuild(session,edition,&options,&capture_id,&binding,start,cancel,&mut reset)?;
            binding=fresh;world_rebuild=Some(receipt);
        }
        let indexed=IndexedOptions {ffmpeg:options.ffmpeg.clone(),encoder:options.encoder,output:options.output.clone(),fps:options.fps,frames:options.frames,
            clean_view:true,settle_frames:options.settle_frames};
        let mut queue=ReplayQueue::attach(session,timeout)?;
        identity=Some(queue.identity().clone());last_sequence=queue.next_sequence()-1;
        let replay_origin_ms=queue.position_ms();
        if let Some(range)=options.range
            && !range.matches(options.fps,options.frames,replay_origin_ms)? {
                return Err(Error::Other("rebuilt replay did not reach the selected export range start".into()));
            }
        let state=queue.confirm(timeout)?;
        let duration=state.state["durationMs"].as_u64().filter(|v|*v<=86_400_000)
            .ok_or_else(||Error::Other("replay export has no valid duration".into()))?;
        if options.range.is_some_and(|range|u64::from(range.to_ms)>duration)
            ||u64::from(replay_origin_ms)*u64::from(options.fps)+u64::from(options.frames-1)*1000>duration*u64::from(options.fps) {
            return Err(Error::Other("export frame range exceeds the remaining replay".into()));
        }
        // Source hashing can take time. Keep only the reservation while
        // preparing; the encoder and held native clock are not active yet.
        progress(ExportProgress {stage:"binding_source",frames_queued:0,frames_total:options.frames});
        let source_reply=queue.source_identity(Duration::from_secs(30))?;
        let source=super::export_identity::SourceLease::bind(&source_reply.result,cancel)?;
        let mut companion_identity=None;
        if options.appearance_snapshot.is_none()&&super::recording::has_companion(&state.state)? {
            let linked=super::recording::automatic_appearance(&source_reply.result)?
                .ok_or_else(||Error::Other("appearance association disappeared while preparing export".into()))?;
            options.appearance_snapshot=Some(linked.path);companion_identity=Some((linked.manifest_hash,linked.recording_id));
            let current=self::binding(edition,&options.bundle,true)?;
            if current.session!=binding.session||current.clock_sequence!=binding.clock_sequence||current.marker_sequence!=binding.marker_sequence||current.used_markers!=binding.used_markers {
                return Err(Error::Other("native controls changed while selecting the appearance companion".into()));
            }
            binding.appearance_sequence=current.appearance_sequence;
        }
        if let Some(snapshot)=options.appearance_snapshot.as_ref() {
            use super::appearance_control::{self,AppearanceControl};
            let sequence=binding.appearance_sequence.ok_or_else(||Error::Other("native appearance control is unavailable".into()))?;
            let state=appearance_control::control(edition,&options.bundle,&binding.session,sequence,AppearanceControl::State)?;
            if state["state"]["loaded"].as_bool()!=Some(false) {
                return Err(Error::Other("export cannot replace an existing appearance mount; clear it or export without selecting another snapshot".into()));
            }
            let owner=format!("export-{}",uuid::Uuid::new_v4().simple());
            appearance=Some(AppearanceLease {sequence:sequence+1,owner:owner.clone(),manifest:None});
            let mounted=appearance_control::control(edition,&options.bundle,&binding.session,sequence+1,
                AppearanceControl::Load {path:snapshot.clone(),owner:Some(owner)})?;
            let hash=mounted["state"]["appearance"]["manifestHash"].as_str().ok_or_else(||Error::Other("appearance mount has no manifest identity".into()))?.to_owned();
            if let Some((expected,id))=companion_identity.as_ref()
                && (&hash!=expected||mounted["state"]["appearance"]["recordingId"].as_str()!=Some(id.as_str())) {
                return Err(Error::Other("appearance companion changed between validation and native mount".into()));
            }
            if let Some(appearance)=appearance.as_mut() {appearance.manifest=Some(hash);}
            if !mounted["state"]["appearance"]["sourceTimeMs"].as_u64().is_some_and(|time|time<=u64::from(replay_origin_ms)) {
                return Err(Error::Other("selected appearance snapshot is later than the export origin".into()));
            }
            let last_numerator=u64::from(replay_origin_ms)*u64::from(options.fps)+u64::from(options.frames-1)*1000;
            if !mounted["state"]["appearance"]["endTimeMs"].as_u64().is_some_and(|time|
                time<=86_400_000&&last_numerator<=time*u64::from(options.fps)) {
                return Err(Error::Other("selected appearance history ends before the export frame range".into()));
            }
        }
        canceled(cancel)?;camera_attempted=true;
        queue.camera_path(&options.camera_path,timeout)?;
        camera_identity=queue.camera_identity().cloned();
        let admitted=capture::control(edition,&options.bundle,&capture_id,CaptureControl::Activate {session:binding.session.clone(),options:indexed})?;
        capture::require_indexed_encoding(&admitted,options.encoder,true)?;
        let receipt=client::clock_control_from_idle(edition,&options.bundle,&binding.session,binding.clock_sequence,
            NativeClockControl {op:NativeClockOperation::Freeze,partial:0.0,real_timer:true,lease_ms:30000,rate:None},&binding.clock_owner)?;
        freeze=Some(receipt.clone());
        let base_tick=receipt["heldTick"].as_u64().ok_or_else(||Error::Other("native freeze has no completed tick".into()))?;
        let rate=receipt["timer"]["ticksPerSecond"].as_f64().filter(|v|v.is_finite() && (1.0..=1000.0).contains(v) && v.fract()==0.0)
            .ok_or_else(||Error::Other("native export needs an observed integral tick rate".into()))? as u32;
        let intent=super::export_identity::ExportIntent {schema:4,source:source.source.clone(),camera_path:options.camera_path.clone(),
            camera_path_sha256:options.camera_path.sha256()?,replay_origin_ms,frames:options.frames,fps:options.fps,
            native_ticks_per_second:Some(rate),settle_frames:options.settle_frames,encoder:options.encoder,
            encoding_profile:Some(options.encoder.indexed_profile().into()),
            selected_range:options.range,
            appearance_manifest_hash:appearance.as_ref().and_then(|lease|lease.manifest.clone())};
        intent.validate()?;
        let ticks=u64::from(options.frames-1)*u64::from(rate)/u64::from(options.fps);
        let markers=ticks+u64::from(options.frames);
        if binding.clock_sequence+ticks+2*u64::from(options.frames)+2>1_000_000_000
            || binding.marker_sequence+markers-1>1_000_000_000 || binding.used_markers+markers>1_000_000
        {return Err(Error::Other("export exceeds the remaining native command/marker budget; restart the client process or shorten the range".into()));}
        clock=Some(NativeExportClock::from_freeze(Timeline {fps:options.fps,ticks_per_second:rate,base_tick,replay_start_ms:replay_origin_ms},
            &receipt,binding.clock_sequence+1)?);
        let mut feed=NativeMarkerFeed::new(queue,edition,&options.bundle,&binding.session,binding.marker_sequence)?;
        let controller=clock.as_mut().ok_or_else(||Error::Other("native export clock was not created".into()))?;
        let mut final_replay_ms=replay_origin_ms;
        for index in 0..options.frames {
            canceled(cancel)?;
            let frame=controller.advance_frame(edition,&options.bundle,index,|point| {
                canceled(cancel)?;
                let result=feed.feed(point);last_sequence=feed.last_sequence();result
            })?;
            let pose=feed.last_camera_pose().ok_or_else(||Error::Other("editor camera pose was not queued for this export frame".into()))?;
            let packet=feed.last_frame_dispatch().cloned().ok_or_else(||Error::Other("export frame has no native dispatcher receipt".into()))?;
            final_replay_ms=frame.time.replay_time_ms;
            canceled(cancel)?;
            controller.capture_protocol_frame(ProtocolFrameCapture {edition,bundle:&options.bundle,capture_id:&capture_id,frame,pose,packet:Some(packet),
                appearance_manifest:appearance.as_ref().and_then(|a|a.manifest.as_deref())})?;
            progress(ExportProgress {stage:"capturing",frames_queued:index+1,frames_total:options.frames});
        }
        source.confirm(&feed.source_identity(timeout)?.result)?;
        // All pixels are queued. Release simulation before waiting on the
        // encoder, so its completion cannot outlive the held-clock lease.
        if let Some(appearance)=appearance.as_mut() {
            appearance.sequence+=1;
            super::appearance_control::control(edition,&options.bundle,&binding.session,appearance.sequence,
                super::appearance_control::AppearanceControl::Time {time_ms:final_replay_ms,owner:Some(appearance.owner.clone())})?;
        }
        controller.resume(edition,&options.bundle)?;clock_released=true;
        canceled(cancel)?;
        progress(ExportProgress {stage:"encoding",frames_queued:options.frames,frames_total:options.frames});
        let encoded=capture::control(edition,&options.bundle,&capture_id,CaptureControl::Finish {frames:options.frames})?;
        capture::require_indexed_encoding(&encoded,options.encoder,true)?;
        canceled(cancel)?;
        source.unchanged()?;export_intent=Some(intent);
        Ok(ExportOutcome {capture_id:capture_id.clone(),output:options.output.clone(),manifest:options.output.with_extension("frames.jsonl"),
            frames:options.frames,fps:options.fps,encoder:options.encoder,replay_origin_ms,native_session:binding.session.clone(),
            appearance_snapshot:options.appearance_snapshot.clone(),appearance_manifest_hash:appearance.as_ref().and_then(|a|a.manifest.clone()),
            world_rebuild:world_rebuild.clone(),
            decoded_video_audit:None,export_receipt:None,
            native_world_verified:false,deterministic_video_verified:false})
    })).unwrap_or_else(|_|Err(Error::Other("export worker panicked; releasing its owned controls".into())));
    // The closure's feeder borrow has ended, including on failure. Cleanup
    // checks original viewer/sequence and capture ownership before mutation.
    let mut cleanup = Vec::new();
    if result.is_err()
        && let Some((generation, old_viewer_id)) = reset.as_ref()
        && let Err(error)=session.request_control(&serde_json::json!({"op":"cancel_reset","generation":generation,"oldViewerId":old_viewer_id}),timeout) {
            cleanup.push(format!("reset cleanup: {error}"));
        }
    if result.is_err()
        && capture_requested
        && let Err(error) = capture::control(
            edition,
            &options.bundle,
            &capture_id,
            CaptureControl::Stop {},
        )
    {
        cleanup.push(format!("capture cleanup: {error}"));
    }
    if camera_attempted
        && let Some(identity) = identity.as_ref()
        && let Err(error) = export_feed::release_owned_camera(
            session,
            identity,
            last_sequence,
            camera_identity.as_ref(),
            timeout,
        )
    {
        cleanup.push(format!("camera cleanup: {error}"));
    }
    if let Some(appearance) = appearance.as_ref()
        && let Err(error) = appearance.release(edition, &options.bundle, &binding.session)
    {
        cleanup.push(format!("appearance cleanup: {error}"));
    }
    if !clock_released {
        let resumed = if let Some(clock) = clock.as_mut() {
            clock.resume(edition, &options.bundle).map(|_| ())
        } else if let Some(freeze) = freeze.as_ref() {
            client::resume_owned_clock(
                edition,
                &options.bundle,
                &binding.session,
                binding.clock_sequence + 1,
                freeze,
            )
            .map(|_| ())
        } else {
            Ok(())
        };
        if let Err(error) = resumed {
            cleanup.push(format!("clock cleanup: {error}"));
        }
    }
    match result {
        Ok(mut outcome) if cleanup.is_empty() => {
            // All native/session controls are released before this read-only
            // pass. It must decode the artifact, not trust pipe frame counts.
            let audit = super::video_audit::run(
                super::video_audit::Request {
                    executable: &options.ffmpeg,
                    encoder: options.encoder,
                    video: &outcome.output,
                    capture_id: &outcome.capture_id,
                    frames: options.frames,
                    fps: options.fps,
                },
                cancel,
                |stage, count| {
                    progress(ExportProgress {
                        stage,
                        frames_queued: count,
                        frames_total: options.frames,
                    })
                },
            );
            match audit {
                Ok(audit) => {
                    let intent = export_intent.take().ok_or_else(|| {
                        Error::Other("export completed without its bound source intent".into())
                    })?;
                    progress(ExportProgress {
                        stage: "binding_export",
                        frames_queued: options.frames,
                        frames_total: options.frames,
                    });
                    let receipt=super::export_identity::write_receipt(&outcome.output,&outcome.capture_id,intent,&audit,cancel)
                        .map_err(|error|Error::Other(format!("Video audited at {}, but export identity binding failed: {error}; artifacts are retained",outcome.output.display())))?;
                    outcome.export_receipt = Some(receipt);
                    outcome.decoded_video_audit = Some(audit);
                    progress(ExportProgress {
                        stage: "complete",
                        frames_queued: options.frames,
                        frames_total: options.frames,
                    });
                    Ok(outcome)
                }
                Err(error) => Err(Error::Other(format!(
                    "Video encoded at {}, but its decoded-frame audit failed: {error}; video and partial audits are retained",
                    outcome.output.display()
                ))),
            }
        }
        Ok(outcome) => Err(Error::Other(format!(
            "Video encoded at {}, but {}",
            outcome.output.display(),
            cleanup.join("; ")
        ))),
        Err(error) => Err(Error::Other(format!(
            "Replay export failed: {error}{}; output, if created, remains at {}",
            if cleanup.is_empty() {
                String::new()
            } else {
                format!("; {}", cleanup.join("; "))
            },
            options.output.display()
        ))),
    }
}

#[cfg(not(windows))]
pub fn run<F>(
    _: &mut OfflineConnector,
    _: Edition,
    _: ExportOptions,
    _: &AtomicBool,
    _: F,
) -> Result<ExportOutcome>
where
    F: FnMut(ExportProgress),
{
    Err(Error::Other("native replay export requires Windows".into()))
}

#[cfg(windows)]
fn canceled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Other("export canceled".into()));
    }
    Ok(())
}
#[cfg(windows)]
struct Binding {
    pid: u32,
    session: String,
    clock_sequence: u64,
    marker_sequence: u64,
    used_markers: u64,
    appearance_sequence: Option<u64>,
    clock_owner: Value,
}

/// Rebuild only the viewer already associated with this native world. The
/// capture reservation is independent of world epochs; no encoder/clock lease
/// is opened until the replacement world and its idle bindings are observed.
#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn rebuild(
    session: &mut OfflineConnector,
    edition: Edition,
    options: &ExportOptions,
    capture_id: &str,
    previous: &Binding,
    start_ms: u32,
    cancel: &AtomicBool,
    owned: &mut Option<(u64, String)>,
) -> Result<(Binding, WorldRebuildReceipt)> {
    use super::{
        appearance_control::{self, AppearanceControl},
        capture::{self, CaptureControl},
    };
    use serde_json::json;
    use std::time::{Duration, Instant};
    let timeout = Duration::from_secs(10);
    let before = session
        .request_control(&json!({"op":"state"}), timeout)?
        .state;
    if before["automaticReconnectAvailable"].as_bool() != Some(true)
        || before["clientInitialized"].as_bool() != Some(true)
        || before["packetReady"].as_bool() != Some(true)
        || before["worldReset"]["stage"]
            .as_str()
            .is_some_and(|s| matches!(s, "initializing" | "catching_up"))
    {
        return Err(Error::Other("world rebuild needs one initialized local RakNet replay viewer with automatic reconnect available".into()));
    }
    let generation = before["generation"]
        .as_u64()
        .and_then(|g| g.checked_add(1))
        .filter(|g| *g <= 1_000_000_000)
        .ok_or_else(|| {
            Error::Other("replay world generation is unavailable or exhausted".into())
        })?;
    let viewers = before["viewerIdentities"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| {
            Error::Other("world rebuild requires exactly one existing replay viewer".into())
        })?;
    let old = &viewers[0];
    let canonical = |value: &Value| {
        value
            .as_str()
            .filter(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == *s))
            .map(str::to_owned)
    };
    let old_id = canonical(&old["viewerId"])
        .ok_or_else(|| Error::Other("replay viewer has no canonical identity".into()))?;
    let player = canonical(&old["playerUuid"])
        .ok_or_else(|| Error::Other("replay viewer has no canonical player UUID".into()))?;
    if old["generation"].as_u64() != Some(generation - 1)
        || old["clientInitialized"].as_bool() != Some(true)
    {
        return Err(Error::Other(
            "replay viewer changed before world rebuild".into(),
        ));
    }
    let bounded = |key: &str| {
        before[key]
            .as_u64()
            .filter(|n| *n <= 86_400_000)
            .map(|n| n as u32)
            .ok_or_else(|| Error::Other(format!("replay rebuild has no valid {key}")))
    };
    let duration = bounded("durationMs")?;
    let target = start_ms.max(bounded("bootstrapTimeMs")?);
    if options
        .range
        .is_some_and(|range| range.from_ms != target || range.to_ms > duration)
    {
        return Err(Error::Other(
            "selected export range starts before bootstrap or ends after the recording".into(),
        ));
    }
    if u64::from(target) * u64::from(options.fps) + u64::from(options.frames - 1) * 1000
        > u64::from(duration) * u64::from(options.fps)
        || options
            .camera_path
            .keyframes
            .last()
            .is_some_and(|key| key.time_ms > duration)
    {
        return Err(Error::Other(
            "requested export range/camera path exceeds the recording".into(),
        ));
    }
    let native = appearance_control::control(
        edition,
        &options.bundle,
        &previous.session,
        previous
            .appearance_sequence
            .ok_or_else(|| Error::Other("world rebuild needs native appearance identity".into()))?,
        AppearanceControl::State,
    )?;
    let available = |value: &Value, expected: &str| -> Result<()> {
        let state = &value["state"];
        if state["session"].as_str() != Some(expected)
            || state["localPlayer"].as_str() != Some(player.as_str())
            || state["worldCurrent"].as_bool() != Some(true)
            || state["loaded"].as_bool() != Some(false)
            || matches!(
                state["history"]["stage"].as_str(),
                Some("recording" | "finishing")
            )
        {
            return Err(Error::Other("native world does not match this replay viewer, or another appearance owner is active".into()));
        }
        Ok(())
    };
    available(&native, &previous.session)?;
    // Preserve cancellation authority before sending an operation that could
    // commit despite a lost/late response from the helper.
    *owned = Some((generation, old_id.clone()));
    let reply = session.request_control(
        &json!({"op":"reset","timeMs":start_ms,"reconnect":true}),
        timeout,
    )?;
    if reply.state["generation"].as_u64() != Some(generation)
        || reply.state["worldReset"]["oldViewerId"].as_str() != Some(old_id.as_str())
        || reply.state["worldReset"]["timeMs"].as_u64() != Some(u64::from(target))
    {
        return Err(Error::Other(
            "replay reset acknowledgement changed owner/generation/target".into(),
        ));
    }
    let started = Instant::now();
    let mut renewed = Instant::now();
    let mut last = "waiting for the replacement replay world".to_owned();
    while started.elapsed() < Duration::from_secs(90) {
        canceled(cancel)?;
        if renewed.elapsed() >= Duration::from_secs(10) {
            capture::control(
                edition,
                &options.bundle,
                capture_id,
                CaptureControl::Renew { lease_ms: 120000 },
            )?;
            renewed = Instant::now();
        }
        let state = session
            .request_control(&json!({"op":"state"}), timeout)?
            .state;
        let reset = &state["worldReset"];
        if state["generation"].as_u64() != Some(generation)
            || reset["generation"].as_u64() != Some(generation)
            || reset["oldViewerId"].as_str() != Some(old_id.as_str())
            || reset["timeMs"].as_u64() != Some(u64::from(target))
        {
            return Err(Error::Other(
                "another replay reset superseded this export".into(),
            ));
        }
        if reset["stage"].as_str() == Some("failed") {
            return Err(Error::Other(
                reset["message"]
                    .as_str()
                    .unwrap_or("replay reconnect failed")
                    .into(),
            ));
        }
        if reset["stage"].as_str() == Some("ready")
            && state["clientInitialized"].as_bool() == Some(true)
            && state["packetReady"].as_bool() == Some(true)
            && state["paused"].as_bool() == Some(true)
            && state["positionMs"].as_u64() == Some(u64::from(target))
        {
            let viewers = state["viewerIdentities"]
                .as_array()
                .filter(|v| v.len() == 1)
                .ok_or_else(|| Error::Other("fresh replay world lost its single viewer".into()))?;
            let viewer = &viewers[0];
            let new_id = canonical(&viewer["viewerId"])
                .filter(|id| id != &old_id)
                .ok_or_else(|| {
                    Error::Other("replay reset retained the old viewer identity".into())
                })?;
            if viewer["playerUuid"].as_str() != Some(player.as_str())
                || viewer["generation"].as_u64() != Some(generation)
                || viewer["clientInitialized"].as_bool() != Some(true)
                || reset["newViewerId"].as_str() != Some(new_id.as_str())
            {
                return Err(Error::Other(
                    "fresh replay identity does not match the reset acknowledgement".into(),
                ));
            }
            match binding(edition, &options.bundle, true) {
                Ok(mut fresh) => {
                    if fresh.pid != previous.pid {
                        return Err(Error::Other(
                            "the native client process changed during replay reset".into(),
                        ));
                    }
                    if fresh.session != previous.session {
                        let sequence = fresh.appearance_sequence.ok_or_else(|| {
                            Error::Other(
                                "fresh native world has no appearance identity consumer".into(),
                            )
                        })?;
                        let native = appearance_control::control(
                            edition,
                            &options.bundle,
                            &fresh.session,
                            sequence,
                            AppearanceControl::State,
                        )?;
                        if native["state"]["worldCurrent"].as_bool() != Some(true) {
                            last = "waiting for the fresh native world's current snapshot".into();
                            std::thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                        available(&native, &fresh.session)?;
                        fresh.appearance_sequence = Some(sequence + 1);
                        capture::control(
                            edition,
                            &options.bundle,
                            capture_id,
                            CaptureControl::Renew { lease_ms: 120000 },
                        )?;
                        let receipt = WorldRebuildReceipt {
                            generation,
                            old_viewer_id: old_id.clone(),
                            new_viewer_id: new_id,
                            player_uuid: player.clone(),
                            native_pid: fresh.pid,
                            previous_native_session: previous.session.clone(),
                            native_session: fresh.session.clone(),
                            target_ms: target,
                        };
                        *owned = None;
                        return Ok((fresh, receipt));
                    }
                    last = "replay reconnected but the native world epoch has not changed".into();
                }
                Err(error) => last = error.to_string(),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(Error::Other(format!(
        "fresh native replay world was not established: {last}"
    )))
}
#[cfg(windows)]
struct AppearanceLease {
    sequence: u64,
    owner: String,
    manifest: Option<String>,
}
#[cfg(windows)]
impl AppearanceLease {
    fn release(&self, edition: Edition, bundle: &std::path::Path, session: &str) -> Result<()> {
        use super::appearance_control::{self, AppearanceControl};
        let state = match appearance_control::control(
            edition,
            bundle,
            session,
            self.sequence + 1,
            AppearanceControl::State,
        ) {
            Ok(state) => state,
            Err(status) => {
                // An out-of-range timeline sample can make state inspection
                // fail. Java still checks the owner before clearing, so the
                // lease can release itself without trusting a broken status.
                return appearance_control::control(
                    edition,
                    bundle,
                    session,
                    self.sequence + 2,
                    AppearanceControl::Clear {
                        owner: Some(self.owner.clone()),
                    },
                )
                .map(|_| ())
                .map_err(|error| {
                    Error::Other(format!("appearance state: {status}; owned clear: {error}"))
                });
            }
        };
        if state["state"]["loaded"].as_bool() == Some(false) {
            return Ok(());
        }
        if state["state"]["appearance"]["owner"].as_str() != Some(self.owner.as_str()) {
            return Err(Error::Other(
                "appearance mount has a different owner; it was left unchanged".into(),
            ));
        }
        appearance_control::control(
            edition,
            bundle,
            session,
            self.sequence + 2,
            AppearanceControl::Clear {
                owner: Some(self.owner.clone()),
            },
        )?;
        Ok(())
    }
}
#[cfg(windows)]
fn binding(edition: Edition, bundle: &std::path::Path, with_appearance: bool) -> Result<Binding> {
    let installed =
        super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
    let pid =
        super::package::process_id(edition.package_family(), &installed.path).ok_or_else(|| {
            Error::Other(
                "native export needs the already-running client and connected replay".into(),
            )
        })?;
    let ready = bundle.join(format!("arctic-bedrock-{pid}.ready"));
    if !std::fs::read_to_string(&ready).is_ok_and(|v| v.trim() == "native-preview") {
        return Err(Error::Other(
            "this bundle is not loaded into the running native client".into(),
        ));
    }
    let clock = report(&bundle.join(format!("arctic-bedrock-{pid}.clock.json")))?;
    let packet = report(&bundle.join(format!("arctic-bedrock-{pid}.packet.json")))?;
    let prefix = format!("native_{pid}_");
    let session = clock["session"]
        .as_str()
        .filter(|s| {
            s.len() <= 64
                && s.strip_prefix(prefix.as_str())
                    .and_then(|epoch| epoch.parse::<u64>().ok())
                    .is_some_and(|epoch| epoch > 0)
        })
        .ok_or_else(|| Error::Other("native export has no current world session".into()))?;
    if clock["installed"].as_bool() != Some(true)
        || clock["controlEnabled"].as_bool() != Some(true)
        || clock["bindingReady"].as_bool() != Some(true)
        || clock["frozen"].as_bool() != Some(false)
        || !clock["queued"].is_null()
        || !clock["pendingWorldTick"].is_null()
        || !clock["playbackRate"].is_null()
        || clock["rateRestorationFailed"].as_bool() == Some(true)
        || clock["waitingForRealTimer"].as_bool() != Some(false)
        || packet["installed"].as_bool() != Some(true)
        || packet["contextLive"].as_bool() != Some(true)
        || packet["session"].as_str() != Some(session)
        || !packet["pending"].is_null()
    {
        return Err(Error::Other(
            "native export needs fresh idle clock/packet bindings in the same world".into(),
        ));
    }
    let next = |status: &Value| {
        status["sequence"]
            .as_u64()
            .and_then(|s| s.checked_add(1))
            .filter(|s| *s < 1_000_000_000)
            .ok_or_else(|| {
                Error::Other("native export command sequences are unavailable or exhausted".into())
            })
    };
    let used_markers = packet["usedMarkers"]
        .as_u64()
        .filter(|n| *n <= 1_000_000)
        .ok_or_else(|| Error::Other("native export marker usage is unavailable".into()))?;
    let appearance_sequence = if with_appearance {
        let appearance = report(&bundle.join(format!("arctic-bedrock-{pid}.appearance.json")))?;
        let sequence = next(&appearance)?;
        if appearance["contextLive"].as_bool() != Some(true)
            || appearance["session"].as_str() != Some(session)
            || !appearance["queued"].is_null()
            || !appearance["executing"].is_null()
            || sequence > 999_999_995
        {
            return Err(Error::Other(
                "replay appearance export needs a current idle appearance consumer".into(),
            ));
        }
        Some(sequence)
    } else {
        None
    };
    Ok(Binding {
        pid,
        session: session.into(),
        clock_sequence: next(&clock)?,
        marker_sequence: next(&packet)?,
        used_markers,
        appearance_sequence,
        clock_owner: clock,
    })
}
#[cfg(windows)]
fn report(path: &std::path::Path) -> Result<Value> {
    use std::{io::Read, time::Duration};
    let file = std::fs::File::open(path).at(path)?;
    if !file
        .metadata()
        .at(path)?
        .modified()
        .at(path)?
        .elapsed()
        .is_ok_and(|age| age < Duration::from_secs(2))
    {
        return Err(Error::Other(format!(
            "native export report is not current: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1).read_to_end(&mut bytes).at(path)?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::Other("native export report exceeds 64 KiB".into()));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
