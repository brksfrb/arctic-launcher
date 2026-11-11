//! `arctic bedrock …`: Minecraft: Bedrock Edition on Windows.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus};
use arctic_core::bedrock::versions::{self, Catalog};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Folders, Player};
use arctic_core::{Error, Result};
use serde_json::{Value, json};

use super::Ctx;
use crate::cli::{
    BedrockArgs, BedrockCommand, BedrockPacksCommand, BedrockServersCommand, BedrockWorldsCommand,
};

/// Versions listed without `--all`.
const NEWEST: usize = 10;

pub fn run(ctx: &Ctx, args: &BedrockArgs) -> Result<i32> {
    let edition = if args.preview {
        Edition::Preview
    } else {
        Edition::Release
    };
    let account = args.account.as_deref();
    match &args.command {
        BedrockCommand::CompareExports { first, second } => {
            let comparison =
                bedrock::video_audit::compare(first, second, &AtomicBool::new(false), |_, _| {})?;
            let matches = comparison.matches();
            ctx.out.emit(json!({"event":"exports_compared","comparison":&comparison}),|| {
                if matches {"Saved export pixels, timing and observed input rows match. Native-world and full determinism remain unverified.".into()}
                else {
                    let frame=comparison.first_difference.as_ref().map_or(String::new(),|difference|format!(" at frame {} ({})",difference.index,difference.fields.join(", ")));
                    let intent=if comparison.intent_differences.is_empty(){String::new()}else{format!("; input intent differs in {}",comparison.intent_differences.join(", "))};
                    format!("Exports differ{frame}{intent}.")
                }
            });
            Ok(if matches { 0 } else { 1 })
        }
        BedrockCommand::NativeAppearance {
            bundle,
            session,
            sequence,
            control,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(control)
                .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut bytes))
                .map_err(|e| Error::io(control, e))?;
            if bytes.len() > 16 * 1024 {
                return Err(Error::Other("appearance operation exceeds 16 KiB".into()));
            }
            let command: bedrock::appearance_control::AppearanceControl =
                serde_json::from_slice(&bytes)?;
            let receipt =
                bedrock::appearance_control::control(edition, bundle, session, *sequence, command)?;
            ctx.out.emit(json!({"event":"native_appearance_applied","receipt":receipt,"native_world_verified":false,"deterministic_video_verified":false}),
                ||"Appearance archive/renderer state acknowledged; native presentation remains unverified.".into());
            Ok(0)
        }
        BedrockCommand::NativePacket {
            bundle,
            request,
            wait,
            wait_dispatch,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(request)
                .and_then(|f| f.take(4097).read_to_end(&mut bytes))
                .map_err(|e| Error::io(request, e))?;
            if bytes.len() > 4096 {
                return Err(Error::Other("native packet arm exceeds 4 KiB".into()));
            }
            let value: Value = serde_json::from_slice(&bytes)?;
            if *wait && *wait_dispatch {
                return Err(Error::Other("choose --wait or --wait-dispatch".into()));
            }
            if (*wait || *wait_dispatch)
                && (!value["requestId"].is_string() || !value["marker"].is_string())
            {
                return Err(Error::Other(
                    "marker waits need the populated requestId/marker returned by the arm command"
                        .into(),
                ));
            }
            let request: bedrock::packet::PacketArm = serde_json::from_value(value)?;
            let receipt = if *wait {
                bedrock::packet::wait(edition, bundle, &request)?
            } else if *wait_dispatch {
                bedrock::packet::wait_dispatch(edition, bundle, &request)?
            } else {
                bedrock::packet::arm(edition, bundle, &request)?
            };
            ctx.out.emit(json!({"event":if *wait {"native_packet_tick_completed"} else if *wait_dispatch {"native_packet_dispatch_returned"} else {"native_packet_armed"},
                "request":request,"replay_command":request.replay_command(),"receipt":receipt,"native_world_verified":false}),
                || if *wait { "Native marker dispatch and a later client tick completed; terrain readiness is separate.".into() }
                else if *wait_dispatch { "Native marker dispatcher returned; permit the planned client tick before waiting for tick completion.".into() }
                else { "Native marker armed; send the returned replay step and permit a client tick.".into() });
            Ok(0)
        }
        BedrockCommand::NativeCapture {
            bundle,
            capture_id,
            control,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(control)
                .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut bytes))
                .map_err(|e| Error::io(control, e))?;
            if bytes.len() > 16 * 1024 {
                return Err(Error::Other(
                    "native capture operation exceeds 16 KiB".into(),
                ));
            }
            let command: bedrock::capture::CaptureControl = serde_json::from_slice(&bytes)?;
            let event = match &command {
                bedrock::capture::CaptureControl::Reserve { .. } => "indexed_capture_reserved",
                bedrock::capture::CaptureControl::Renew { .. } => {
                    "indexed_capture_reservation_renewed"
                }
                bedrock::capture::CaptureControl::Start { .. } => "indexed_capture_started",
                bedrock::capture::CaptureControl::Activate { .. } => "indexed_capture_started",
                bedrock::capture::CaptureControl::Sample { .. } => "indexed_frame_queued",
                bedrock::capture::CaptureControl::Stop {} => "indexed_capture_stopping",
                bedrock::capture::CaptureControl::Observe {} => "indexed_world_observed",
                bedrock::capture::CaptureControl::Finish { .. } => "indexed_video_saved",
            };
            let status = bedrock::capture::control(edition, bundle, capture_id, command)?;
            ctx.out.emit(
                json!({"event":event,"status":status,"deterministic_video_verified":false}),
                || match event {
                    "indexed_frame_queued" => {
                        "Matching native presented frame queued for encoding.".into()
                    }
                    "indexed_video_saved" => {
                        "Indexed native video and frame manifest saved.".into()
                    }
                    "indexed_capture_stopping" => "Owned indexed capture is stopping.".into(),
                    "indexed_world_observed" => {
                        "Recent native presented-world identity observed.".into()
                    }
                    _ => "Indexed native capture is waiting for explicit samples.".into(),
                },
            );
            Ok(0)
        }
        BedrockCommand::NativeClock {
            bundle,
            session,
            sequence,
            control,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(control)
                .and_then(|file| file.take(4097).read_to_end(&mut bytes))
                .map_err(|e| Error::io(control, e))?;
            if bytes.len() > 4096 {
                return Err(Error::Other("native clock operation exceeds 4 KiB".into()));
            }
            let control: bedrock::client::NativeClockControl = serde_json::from_slice(&bytes)?;
            let receipt =
                bedrock::client::clock_control(edition, bundle, session, *sequence, control)?;
            ctx.out.emit(json!({"event":"native_clock_applied","receipt":receipt}),
                || if control.op==bedrock::client::NativeClockOperation::Step {
                    "Native clock step completed its primary world tick.".into()
                } else if control.op==bedrock::client::NativeClockOperation::Rate {
                    "Native timer multiplier applied and restored; playback fidelity remains unverified.".into()
                } else {"Native clock operation acknowledged.".into()});
            Ok(0)
        }
        BedrockCommand::NativeCamera {
            bundle,
            session,
            sequence,
            target,
            release,
        } => {
            use std::io::Read;
            // Explicitly reject invalid combinations too for non-clap callers.
            if target.is_some() == *release {
                return Err(Error::Other(
                    "choose a native camera target or --release".into(),
                ));
            }
            let target = target
                .as_ref()
                .map(|path| -> Result<bedrock::client::NativeCameraTarget> {
                    let mut bytes = Vec::new();
                    std::fs::File::open(path)
                        .and_then(|file| file.take(4097).read_to_end(&mut bytes))
                        .map_err(|e| Error::io(path, e))?;
                    if bytes.len() > 4096 {
                        return Err(Error::Other("native camera target exceeds 4 KiB".into()));
                    }
                    Ok(serde_json::from_slice(&bytes)?)
                })
                .transpose()?;
            let receipt =
                bedrock::client::camera_control(edition, bundle, session, *sequence, target)?;
            ctx.out.emit(
                json!({"event":"native_camera_applied","receipt":receipt}),
                || {
                    format!(
                        "Native camera {} acknowledged by its render consumer.",
                        if *release { "release" } else { "target" }
                    )
                },
            );
            Ok(0)
        }
        BedrockCommand::Capture {
            bundle,
            ffmpeg,
            output,
            seconds,
            fps,
        } => {
            ctx.out.info(
                "Recording presented frames; keep the game rendering until the clip finishes…",
            );
            let file =
                bedrock::client::record_video(edition, bundle, ffmpeg, output, *seconds, *fps)?;
            ctx.out.emit(
                json!({"event":"video_saved","file":file,"fps":fps,"seconds":seconds}),
                || format!("Video saved: {}", file.display()),
            );
            Ok(0)
        }
        BedrockCommand::Status => status(ctx, edition),
        BedrockCommand::Play { native_client } => {
            match native_client {
                Some(bundle) => bedrock::client::launch(edition, bundle)?,
                None => bedrock::launch(edition)?,
            }
            ctx.out.emit(
                json!({"event": "launched", "edition": edition.id(), "native_client": native_client.as_ref().map(|_| "preview")}),
                || if native_client.is_some() {
                    format!("Arctic's native client preview is rendering in {}. Right Shift opens its menu. Engine bindings, including in-world skins/capes/cosmetics, are pending.", edition.name())
                } else { format!("Starting Minecraft {}.", edition.name()) },
            );
            Ok(0)
        }
        BedrockCommand::Offline {
            address,
            name,
            skin,
            cape,
            slim,
            record_to,
            listen_port,
            version,
            play,
        } => {
            let mut options = bedrock::offline::OfflineOptions::new(
                address,
                name,
                *listen_port,
                version.clone(),
            )?;
            options.appearance =
                bedrock::appearance::Appearance::read(skin.as_deref(), cape.as_deref(), *slim)?;
            options.record_to = record_to
                .as_ref()
                .map(std::path::absolute)
                .transpose()
                .map_err(|e| Error::Other(e.to_string()))?;
            ctx.out.info("Preparing the unsigned Bedrock connector (first use installs its protocol helper)…");
            let mut session = bedrock::offline::start(ctx.root.root(), &options)?;
            ctx.out.emit(json!({"event":"offline_ready", "address":session.address, "username":session.username,
                "version":session.version, "xbox_auth":false}), ||
                format!("Offline connector ready: join {} in Bedrock as {}. The target must accept unsigned players. Keep this command open; Ctrl+C stops it.", session.address, session.username));
            if *play {
                bedrock::launch(edition)?;
            }
            watch_session(ctx, &mut session, edition)
        }
        BedrockCommand::Replay {
            file,
            listen_port,
            nethernet,
            speed,
            start_ms,
        } => {
            let file = std::fs::canonicalize(file)
                .map_err(|e| Error::Other(format!("couldn't open replay: {e}")))?;
            let transport = if *nethernet {
                bedrock::offline::ReplayTransport::Nethernet
            } else {
                bedrock::offline::ReplayTransport::Raknet
            };
            let options = bedrock::offline::PlaybackOptions {
                file,
                listen_port: listen_port.unwrap_or_else(|| transport.default_port()),
                speed: *speed,
                start_ms: *start_ms,
                transport,
            };
            let mut session = bedrock::offline::playback(ctx.root.root(), &options)?;
            ctx.out.emit(json!({"event":"replay_ready","address":session.address,"username":session.username,"version":session.version,"xbox_auth":false}),
                ||format!("Replay ready: join {} in Bedrock. Playback uses no Xbox authentication. Keep this command open.",session.address));
            watch_session(ctx, &mut session, edition)
        }
        BedrockCommand::Versions { all } => {
            let catalog = versions::catalog(&ctx.root.cache())?;
            list_versions(ctx, &catalog, edition, *all);
            Ok(0)
        }
        BedrockCommand::Install { version } => install(ctx, edition, version),
        BedrockCommand::Worlds(cmd) => run_worlds(ctx, edition, account, cmd),
        BedrockCommand::Packs(cmd) => run_packs(ctx, edition, cmd),
        BedrockCommand::Servers(cmd) => run_servers(ctx, edition, account, cmd),
    }
}

fn watch_session(
    ctx: &Ctx,
    session: &mut bedrock::offline::OfflineConnector,
    edition: Edition,
) -> Result<i32> {
    use std::io::BufRead;
    use std::sync::{Arc, atomic::Ordering};
    let (sender, input) = std::sync::mpsc::channel();
    let export_cancel = Arc::new(AtomicBool::new(false));
    let input_cancel = Arc::clone(&export_cancel);
    std::thread::spawn(move || {
        for line in std::io::stdin()
            .lock()
            .lines()
            .map_while(std::result::Result::ok)
        {
            if matches!(
                line.trim().to_ascii_lowercase().as_str(),
                "stop" | "cancel export"
            ) {
                input_cancel.store(true, Ordering::Relaxed);
            }
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let emit = |event: bedrock::offline::OfflineEvent| {
        ctx.out.emit(json!({"event":event.event,"message":event.message,"username":event.username,"xbox_auth":event.xbox_auth,
            "position_ms":event.position_ms,"duration_ms":event.duration_ms,"paused":event.paused,"speed":event.speed,
            "generation":event.generation,"sequence":event.sequence,"connection_required":event.connection_required,
            "packet_ready":event.packet_ready,"packets_queued":event.packets_queued,
            "marker_timestamp":event.marker_timestamp,"marker_queued":event.marker_queued,
            "camera_time":event.camera_time,"camera_queued":event.camera_queued,
            "camera_pose":event.camera_pose,
            "request_id":event.request_id,"operation":event.operation,"result":event.result,"state":event.state,
            "bootstrap_time_ms":event.bootstrap_time_ms,"wait_for_client_init":event.wait_for_client_init,
            "client_initialized":event.client_initialized,"viewers":event.viewers,"initialized_viewers":event.initialized_viewers,
            "viewer_id":event.viewer_id,"player_uuid":event.player_uuid,
            "viewer_identities":event.viewer_identities.as_ref().map(|viewers|viewers.iter().map(|v|
                json!({"viewer_id":v.viewer_id,"player_uuid":v.player_uuid,"username":v.username,
                    "generation":v.generation,"client_initialized":v.client_initialized})).collect::<Vec<_>>()),
            "native_world_verified":event.native_world_verified,"pending_step":event.pending_step}),
            || if event.event=="replay_state" {
                format!("{} / {} ms · {}× · {}",event.position_ms.unwrap_or_default(),event.duration_ms.unwrap_or_default(),
                    event.speed.unwrap_or(1.0),if event.paused.unwrap_or_default(){"paused"}else{"playing"})
            } else {event.message.clone().unwrap_or_else(||event.event.clone())});
    };
    let mut show_state = false;
    let mut paired_recording: Option<bedrock::recording::PairLease> = None;
    let mut playback_clock: Option<bedrock::playback_clock::PlaybackClock> = None;
    while session.running() {
        while let Some(event) = session.try_event() {
            if event.event == "replay_state" {
                if !ctx.out.json && !show_state {
                    continue;
                }
                show_state = false;
            }
            emit(event);
        }
        for _ in 0..32 {
            let line = match input.try_recv() {
                Ok(line) => line,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let clock_failed=release_playback_clock(ctx,&mut playback_clock).is_err_and(|error| {
                        ctx.out.emit(json!({"event":"native_playback_clock_failed","message":error.to_string()}),||format!("Playback clock cleanup: {error}"));true
                    });
                    let failed = if let Err(error) =
                        finish_native_recording(ctx, session, edition, &mut paired_recording)
                    {
                        ctx.out.emit(json!({"event":"native_recording_failed","message":error.to_string(),"native_world_verified":false}),||format!("Paired recording: {error}"));
                        true
                    } else {
                        false
                    };
                    for event in session.stop()? {
                        emit(event);
                    }
                    return Ok(i32::from(failed || clock_failed));
                }
            };
            if line.trim().eq_ignore_ascii_case("cancel export") {
                export_cancel.store(false, Ordering::Relaxed);
                ctx.out.info("Export cancellation requested.");
                continue;
            }
            if line.trim().eq_ignore_ascii_case("stop") {
                let clock_failed=release_playback_clock(ctx,&mut playback_clock).is_err_and(|error| {
                    ctx.out.emit(json!({"event":"native_playback_clock_failed","message":error.to_string()}),||format!("Playback clock cleanup: {error}"));true
                });
                let failed = if let Err(error) =
                    finish_native_recording(ctx, session, edition, &mut paired_recording)
                {
                    ctx.out.emit(json!({"event":"native_recording_failed","message":error.to_string(),"native_world_verified":false}),||format!("Paired recording: {error}"));
                    true
                } else {
                    false
                };
                for event in session.stop()? {
                    emit(event);
                }
                return Ok(i32::from(failed || clock_failed));
            }
            let command = match line.trim().to_ascii_lowercase().as_str() {
                "pause" => json!({"op":"pause","paused":true}),
                "resume" => json!({"op":"pause","paused":false}),
                "state" => json!({"op":"state"}),
                "camera clear" => json!({"op":"camera_clear"}),
                _ => match serde_json::from_str::<Value>(&line) {
                    Ok(command) => command,
                    Err(_) => {
                        ctx.out.info("Use pause, resume, state, camera clear, cancel export, stop, or JSON replay controls. native_clock_attach accepts a loaded bundle; native_clock_release detaches it.");
                        continue;
                    }
                },
            };
            if command["op"].as_str() == Some("native_clock_attach") {
                let result = (|| -> Result<Value> {
                    if playback_clock.is_some() {
                        return Err(Error::Other(
                            "this playback loop already owns a native clock coordinator".into(),
                        ));
                    }
                    if command.as_object().is_none_or(|object| {
                        object.len() != 2
                            || object
                                .keys()
                                .any(|key| !matches!(key.as_str(), "op" | "bundle"))
                    }) {
                        return Err(Error::Other(
                            "native_clock_attach accepts only op and bundle".into(),
                        ));
                    }
                    let bundle: PathBuf = serde_json::from_value(command["bundle"].clone())?;
                    let clock =
                        bedrock::playback_clock::PlaybackClock::attach(session, edition, &bundle)?;
                    let status = clock.status();
                    playback_clock = Some(clock);
                    Ok(status)
                })();
                match result {
                    Ok(status)=>ctx.out.emit(json!({"event":"native_playback_clock_attached","status":status}),
                        ||"Replay viewer correlated; owned native rate coordination enabled (unverified).".into()),
                    Err(error)=>ctx.out.emit(json!({"event":"native_playback_clock_failed","message":error.to_string()}),
                        ||format!("Playback clock: {error}. The replay may remain paused.")),
                }
                show_state = true;
                continue;
            }
            if command["op"].as_str() == Some("native_clock_release") {
                let result = if command.as_object().is_none_or(|object| object.len() != 1) {
                    Err(Error::Other(
                        "native_clock_release accepts no extra fields".into(),
                    ))
                } else {
                    release_playback_clock(ctx, &mut playback_clock)
                };
                if let Err(error) = result {
                    ctx.out.emit(
                        json!({"event":"native_playback_clock_failed","message":error.to_string()}),
                        || format!("Playback clock release: {error}"),
                    );
                }
                continue;
            }
            if matches!(command["op"].as_str(), Some("pause" | "speed"))
                && let Some(clock) = playback_clock.as_mut()
            {
                match clock.control(session,&command) {
                    Ok(reply)=>ctx.out.emit(json!({"event":"native_playback_clock_updated","status":clock.status(),"state":reply.state}),
                        ||"Replay timing and owned native rate acknowledged; fidelity remains unverified.".into()),
                    Err(error)=>{
                        ctx.out.emit(json!({"event":"native_playback_clock_failed","message":error.to_string()}),||format!("Playback clock: {error}"));
                        playback_clock=None;
                    }
                }
                show_state = true;
                continue;
            }
            if matches!(
                command["op"].as_str(),
                Some("reset" | "seek" | "step" | "export")
            ) && let Err(error) = release_playback_clock(ctx, &mut playback_clock)
            {
                ctx.out.emit(
                    json!({"event":"native_playback_clock_failed","message":error.to_string()}),
                    || format!("Playback clock handoff: {error}"),
                );
                continue;
            }
            if command["op"].as_str() == Some("recording_state")
                || command["op"].as_str() == Some("state") && !session.is_playback()
            {
                let result = if command.as_object().is_none_or(|o| o.len() != 1) {
                    Err(Error::Other(
                        "recording state accepts no extra fields".into(),
                    ))
                } else {
                    session.request_recording_control(
                        &json!({"op":"recording_state"}),
                        std::time::Duration::from_secs(10),
                    )
                };
                match result {
                    Ok(reply) => ctx.out.emit(
                        json!({"event":"recording_state","state":reply.state}),
                        || {
                            format!(
                                "{} live recording(s), initialized: {}",
                                reply.state["recordings"], reply.state["initialized"]
                            )
                        },
                    ),
                    Err(error) => ctx.out.emit(
                        json!({"event":"control_error","message":error.to_string()}),
                        || error.to_string(),
                    ),
                }
                continue;
            }
            if command["op"].as_str() == Some("native_record_begin") {
                let result = (|| -> Result<()> {
                    if paired_recording.is_some() {
                        return Err(Error::Other(
                            "this CLI already owns a paired recording".into(),
                        ));
                    }
                    if command
                        .as_object()
                        .is_none_or(|o| o.keys().any(|k| !matches!(k.as_str(), "op" | "bundle")))
                    {
                        return Err(Error::Other(
                            "native_record_begin accepts only op and bundle".into(),
                        ));
                    }
                    let bundle: PathBuf = serde_json::from_value(command["bundle"].clone())?;
                    bedrock::recording::begin(session, edition, &bundle, &mut paired_recording)
                })();
                match result {
                    Ok(())=> {
                        let lease=paired_recording.as_ref().expect("acknowledged paired recording lease");
                        ctx.out.emit(json!({"event":"native_recording_started","lease":lease,"alignment_verified":false,"native_world_verified":false}),
                            ||format!("Appearance recording paired with {}",lease.source.display()));
                    }
                    Err(error)=>ctx.out.emit(json!({"event":"native_recording_failed","message":error.to_string(),"retainedLease":paired_recording.as_ref(),"native_world_verified":false}),||format!("Paired recording: {error}")),
                }
                continue;
            }
            if command["op"].as_str() == Some("native_record_finish") {
                let result = if command.as_object().is_none_or(|o| o.len() != 1)
                    || paired_recording.is_none()
                {
                    Err(Error::Other(
                        "native_record_finish needs this CLI's active pair and no extra fields"
                            .into(),
                    ))
                } else {
                    finish_native_recording(ctx, session, edition, &mut paired_recording)
                };
                if let Err(error) = result {
                    ctx.out.emit(json!({"event":"native_recording_failed","message":error.to_string(),"native_world_verified":false}),||format!("Paired recording: {error}"));
                }
                continue;
            }
            if command["op"].as_str() == Some("native_record_cancel") {
                let result = (|| -> Result<()> {
                    if command.as_object().is_none_or(|o| o.len() != 1) {
                        return Err(Error::Other(
                            "native_record_cancel accepts no extra fields".into(),
                        ));
                    }
                    let lease = paired_recording.as_ref().ok_or_else(|| {
                        Error::Other("this CLI has no owned appearance take".into())
                    })?;
                    bedrock::recording::cancel(edition, lease)?;
                    paired_recording = None;
                    Ok(())
                })();
                match result {
                    Ok(())=>ctx.out.emit(json!({"event":"native_recording_cancelled"}),||"Owned appearance capture released; packet recording continues".into()),
                    Err(error)=>ctx.out.emit(json!({"event":"native_recording_failed","message":error.to_string(),"retainedLease":paired_recording.as_ref(),"native_world_verified":false}),||format!("Paired recording cleanup: {error}")),
                }
                continue;
            }
            if command["op"].as_str() == Some("native_export") {
                let options = (|| -> Result<bedrock::export_job::ExportOptions> {
                    if command.as_object().is_none_or(|o| {
                        o.keys()
                            .any(|key| !matches!(key.as_str(), "op" | "options"))
                    }) {
                        return Err(Error::Other(
                            "native_export accepts only op and options".into(),
                        ));
                    }
                    Ok(serde_json::from_value(command["options"].clone())?)
                })();
                let result=options.and_then(|options|{
                    release_playback_clock(ctx,&mut playback_clock)?;
                    bedrock::export_job::run(session,edition,options,&export_cancel,|p| {
                    ctx.out.emit(json!({"event":"native_export_progress","stage":p.stage,"frames_queued":p.frames_queued,"frames_total":p.frames_total}),
                        ||format!("Indexed export: {} · {}/{} frames queued",p.stage,p.frames_queued,p.frames_total));
                    })
                });
                export_cancel.store(false, Ordering::Relaxed);
                match result {
                    Ok(outcome)=>ctx.out.emit(json!({"event":"native_export_saved","outcome":outcome}),
                        ||format!("Indexed video saved: {}",outcome.output.display())),
                    Err(error)=>ctx.out.emit(json!({"event":"native_export_failed","message":error.to_string(),"native_world_verified":false,"deterministic_video_verified":false}),
                        ||format!("Indexed export failed: {error}")),
                }
                show_state = true;
                continue;
            }
            session.control(&command)?;
            show_state = true;
        }
        if let Some(clock) = playback_clock.as_mut()
            && let Err(error) = clock.service(session)
        {
            ctx.out.emit(
                json!({"event":"native_playback_clock_failed","message":error.to_string()}),
                || format!("Playback clock coordination stopped: {error}"),
            );
            playback_clock = None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if let Some(lease) = paired_recording.as_ref() {
        let _ = bedrock::recording::cancel(edition, lease);
    }
    for event in session.stop()? {
        emit(event);
    }
    Err(Error::Other("the local Bedrock session stopped".into()))
}

fn release_playback_clock(
    ctx: &Ctx,
    clock: &mut Option<bedrock::playback_clock::PlaybackClock>,
) -> Result<()> {
    let Some(owned) = clock.as_mut() else {
        return Ok(());
    };
    owned.release()?;
    *clock = None;
    ctx.out
        .emit(json!({"event":"native_playback_clock_released"}), || {
            "Owned native playback clock released.".into()
        });
    Ok(())
}

fn finish_native_recording(
    ctx: &Ctx,
    session: &mut bedrock::offline::OfflineConnector,
    edition: Edition,
    pair: &mut Option<bedrock::recording::PairLease>,
) -> Result<()> {
    let Some(lease) = pair.as_ref() else {
        return Ok(());
    };
    ctx.out.info("Finishing packet and appearance recordings…");
    let outcome = bedrock::recording::finish(session, edition, lease)?;
    ctx.out.emit(
        json!({"event":"native_recording_saved","outcome":outcome}),
        || format!("Paired recording saved: {}", outcome.source.display()),
    );
    *pair = None;
    Ok(())
}

fn status(ctx: &Ctx, edition: Edition) -> Result<i32> {
    let install = bedrock::find(edition);
    let folders = bedrock::folders(edition);
    let running = install.is_some() && bedrock::running(edition);
    let accounts: Vec<Value> = folders
        .players
        .iter()
        .map(|p| json!({"id": p.id, "path": p.com_mojang}))
        .collect();
    ctx.out.emit(
        json!({
            "edition": edition.id(),
            "supported": bedrock::supported(),
            "installed": install.is_some(),
            "version": install.as_ref().map(|i| &i.version),
            "path": install.as_ref().map(|i| &i.path),
            "gdk": install.as_ref().map(|i| i.gdk),
            "running": running,
            "data": folders.root,
            "packs": folders.packs.as_ref().map(|p| &p.com_mojang),
            "accounts": accounts,
        }),
        || {
            if !bedrock::supported() {
                return "Bedrock Edition is only played on Windows.".into();
            }
            let mut text = match &install {
                Some(i) => format!(
                    "Minecraft {} {}{}{}\n  {}",
                    edition.name(),
                    versions::display(&i.version, edition),
                    if i.gdk { "" } else { " (old UWP build)" },
                    if running { ", running" } else { "" },
                    i.path.display()
                ),
                None => format!(
                    "Minecraft {} isn't installed (get it from the Microsoft Store).",
                    edition.name()
                ),
            };
            if let Some(root) = &folders.root {
                text.push_str(&format!("\nData: {}", root.display()));
            }
            for p in &folders.players {
                text.push_str(&format!(
                    "\n  {:<22} {}  (--account {})",
                    p.label(),
                    p.com_mojang.display(),
                    p.id
                ));
            }
            text
        },
    );
    Ok(0)
}

fn list_versions(ctx: &Ctx, catalog: &Catalog, edition: Edition, all: bool) {
    let installed = bedrock::find(edition).map(|i| i.version);
    let list = catalog.of(edition);
    let shown = if all {
        list.len()
    } else {
        NEWEST.min(list.len())
    };
    for v in &list[..shown] {
        let current = installed.as_deref() == Some(v.id.as_str());
        ctx.out.emit(
            json!({"id": v.id, "name": v.display(), "edition": edition.id(), "installed": current}),
            || {
                format!(
                    "{:<12} {:<14}{}",
                    v.display(),
                    v.id,
                    if current { "  installed" } else { "" }
                )
            },
        );
    }
    if shown < list.len() {
        ctx.out
            .info(&format!("…and {} older (--all).", list.len() - shown));
    }
}

fn install(ctx: &Ctx, edition: Edition, wanted: &str) -> Result<i32> {
    let catalog = versions::catalog(&ctx.root.cache())?;
    let version = if wanted.eq_ignore_ascii_case("latest") {
        catalog.latest(edition)
    } else {
        catalog.find(edition, wanted)
    }
    .cloned()
    .ok_or_else(|| {
        Error::Other(format!(
            "no {} version '{wanted}' (see `arctic bedrock versions{}`)",
            edition.name(),
            if edition == Edition::Preview {
                " --preview"
            } else {
                ""
            }
        ))
    })?;
    if bedrock::find(edition).is_some_and(|i| i.version == version.id) {
        ctx.out.emit(
            json!({"event": "already_installed", "version": version.id}),
            || {
                format!(
                    "{} {} is installed already.",
                    edition.name(),
                    version.display()
                )
            },
        );
        return Ok(0);
    }
    let cancel = AtomicBool::new(false);
    let result = versions::install(&version, &cancel, &|p| {
        // Windows only tells how far along it is, not bytes.
        let stage = format!("{}  {:>3}%", p.stage, p.done);
        ctx.out
            .progress(arctic_core::ProgressInfo { stage: &stage, ..p });
    });
    ctx.out.end_progress();
    result?;
    ctx.out.emit(
        json!({"event": "installed", "edition": edition.id(), "version": version.id}),
        || format!("Installed {} {}.", edition.name(), version.display()),
    );
    Ok(0)
}

/// The account folder asked for, or the one played last.
fn player<'a>(folders: &'a Folders, account: Option<&str>, edition: Edition) -> Result<&'a Player> {
    if let Some(id) = account
        && !folders.players.iter().any(|p| p.id == id)
    {
        return Err(Error::Other(format!(
            "no account folder '{id}' (see `arctic bedrock status`)"
        )));
    }
    folders.player(account).ok_or_else(|| no_data(edition))
}

fn no_data(edition: Edition) -> Error {
    Error::Other(format!(
        "Minecraft {} has no data folders yet: start it once first",
        edition.name()
    ))
}

fn run_worlds(
    ctx: &Ctx,
    edition: Edition,
    account: Option<&str>,
    command: &BedrockWorldsCommand,
) -> Result<i32> {
    let folders = bedrock::folders(edition);
    let player = player(&folders, account, edition)?;
    match command {
        BedrockWorldsCommand::List => {
            for w in worlds::list(player) {
                ctx.out.emit(
                    json!({"id": w.id, "name": w.name, "size": w.size, "path": w.path}),
                    || format!("{:<32} {:>9}  {}", w.name, megabytes(w.size), w.id),
                );
            }
        }
        BedrockWorldsCommand::Import { path } => {
            let name = worlds::import(path, player)?;
            ctx.out
                .emit(json!({"event": "imported", "name": name}), || {
                    format!("Imported \"{name}\".")
                });
        }
        BedrockWorldsCommand::Export { world, to } => {
            let found = find_world(player, world)?;
            let to = to.clone().unwrap_or_else(|| {
                std::path::PathBuf::from(format!(
                    "{}.mcworld",
                    found.name.replace(['/', '\\', ':'], "_")
                ))
            });
            worlds::export(&found, &to)?;
            ctx.out.emit(
                json!({"event": "exported", "id": found.id, "file": to}),
                || format!("Exported \"{}\" to {}", found.name, to.display()),
            );
        }
        BedrockWorldsCommand::Backup { world } => {
            let found = find_world(player, world)?;
            let file = worlds::backup(&found, &ctx.root.root().join("bedrock").join("backups"))?;
            ctx.out.emit(
                json!({"event": "backed_up", "id": found.id, "file": file}),
                || format!("Backed up \"{}\" to {}", found.name, file.display()),
            );
        }
        BedrockWorldsCommand::Remove { world } => {
            let found = find_world(player, world)?;
            worlds::trash(&found, player)?;
            ctx.out
                .emit(json!({"event": "removed", "id": found.id}), || {
                    format!("Moved \"{}\" to com.mojang\\arctic-trash", found.name)
                });
        }
    }
    Ok(0)
}

fn find_world(player: &Player, query: &str) -> Result<World> {
    let list = worlds::list(player);
    list.iter()
        .find(|w| w.id == query)
        .or_else(|| list.iter().find(|w| w.name.eq_ignore_ascii_case(query)))
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "no world '{query}' (see `arctic bedrock worlds list`)"
            ))
        })
}

fn run_packs(ctx: &Ctx, edition: Edition, command: &BedrockPacksCommand) -> Result<i32> {
    let folders = bedrock::folders(edition);
    let shared = folders.packs.as_ref().ok_or_else(|| no_data(edition))?;
    match command {
        BedrockPacksCommand::List => {
            for p in packs::list(shared) {
                ctx.out.emit(pack_json(&p), || {
                    format!(
                        "{:<14} {:<32} {:<9} {}",
                        p.kind.name(),
                        p.name,
                        p.version,
                        if p.development { "development" } else { "" }
                    )
                });
            }
        }
        BedrockPacksCommand::Import { path } => {
            let names = packs::import(path, shared)?;
            ctx.out
                .emit(json!({"event": "imported", "packs": names}), || {
                    format!("Imported {}.", names.join(", "))
                });
        }
        BedrockPacksCommand::Remove { pack } => {
            let list = packs::list(shared);
            let query = pack.to_ascii_lowercase();
            let found = list
                .iter()
                .find(|p| p.uuid == query)
                .or_else(|| {
                    list.iter().find(|p| {
                        p.path
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(pack))
                    })
                })
                .or_else(|| list.iter().find(|p| p.name.eq_ignore_ascii_case(pack)))
                .ok_or_else(|| {
                    Error::Other(format!(
                        "no pack '{pack}' (see `arctic bedrock packs list`)"
                    ))
                })?;
            packs::trash(found, shared)?;
            ctx.out
                .emit(json!({"event": "removed", "uuid": found.uuid}), || {
                    format!("Moved \"{}\" to com.mojang\\arctic-trash", found.name)
                });
        }
    }
    Ok(0)
}

fn pack_json(p: &Pack) -> Value {
    json!({
        "kind": format!("{:?}", p.kind).to_lowercase(),
        "name": p.name,
        "description": p.description,
        "version": p.version,
        "uuid": p.uuid,
        "development": p.development,
        "path": p.path,
    })
}

fn run_servers(
    ctx: &Ctx,
    edition: Edition,
    account: Option<&str>,
    command: &BedrockServersCommand,
) -> Result<i32> {
    if let BedrockServersCommand::Ping { address } = command {
        let result = servers::ping(address);
        ctx.out.emit(
            json!({"address": address, "status": status_json(&result)}),
            || {
                let mut text = format!("{address}  {}", summary(&result));
                if let Ok(s) = &result {
                    if !s.status.motd.is_empty() {
                        text.push_str(&format!("\n{}", s.status.motd));
                    }
                    if !s.level.is_empty() {
                        text.push_str(&format!("\n{}", s.level));
                    }
                }
                text
            },
        );
        return Ok(i32::from(result.is_err()));
    }
    let folders = bedrock::folders(edition);
    let player = player(&folders, account, edition)?;
    match command {
        BedrockServersCommand::List { no_ping } => {
            let list = servers::list(player);
            if list.is_empty() {
                ctx.out
                    .info("No servers yet (add one with `arctic bedrock servers add`).");
                return Ok(0);
            }
            let addresses: Vec<String> = list.iter().map(|s| s.address()).collect();
            let results: std::sync::Mutex<Vec<Option<Result<BedrockStatus>>>> =
                std::sync::Mutex::new(addresses.iter().map(|_| None).collect());
            if !no_ping {
                servers::ping_each(&addresses, |i, r| {
                    if let Ok(mut all) = results.lock() {
                        all[i] = Some(r);
                    }
                });
            }
            let results = results.into_inner().unwrap_or_default();
            for (i, server) in list.iter().enumerate() {
                let status = results.get(i).and_then(Option::as_ref);
                let mut row = json!({"name": server.name, "address": addresses[i]});
                if let Some(r) = status {
                    row["status"] = status_json(r);
                }
                ctx.out.emit(row, || {
                    let state = status.map_or_else(String::new, |r| format!("  {}", summary(r)));
                    format!("{:<24} {:<28}{state}", server.name, addresses[i])
                });
            }
        }
        BedrockServersCommand::Add { address, name } => {
            let added = servers::add(player, name.as_deref().unwrap_or(""), address)?;
            ctx.out.emit(
                json!({"event": if added { "added" } else { "already_listed" }, "address": address}),
                || match added {
                    true => format!("Added {address} to the game's server list."),
                    false => format!("{address} is already on the list."),
                },
            );
        }
        BedrockServersCommand::Remove { address } => {
            if !servers::remove(player, address)? {
                return Err(Error::Other(format!("{address} isn't on the list")));
            }
            ctx.out
                .emit(json!({"event": "removed", "address": address}), || {
                    format!("Removed {address}.")
                });
        }
        BedrockServersCommand::Ping { .. } => unreachable!(),
    }
    Ok(0)
}

fn status_json(result: &Result<BedrockStatus>) -> Value {
    match result {
        Ok(s) => json!({
            "online": s.status.online,
            "max": s.status.max,
            "version": s.status.version,
            "protocol": s.status.protocol,
            "motd": s.status.motd,
            "level": s.level,
            "game_mode": s.game_mode,
            "ping_ms": s.status.ping_ms,
        }),
        Err(e) => json!({"error": e.to_string()}),
    }
}

fn summary(result: &Result<BedrockStatus>) -> String {
    match result {
        Ok(s) => format!(
            "{}/{} online  {} ms  {}",
            s.status.online, s.status.max, s.status.ping_ms, s.status.version
        ),
        Err(e) => format!("offline ({e})"),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
