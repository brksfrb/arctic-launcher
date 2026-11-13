// i18n-ignore-file: Bedrock stays in English for now.
//! Background jobs for the Bedrock tab: finding the game and its data,
//! the version list, installing a version, worlds, packs and servers.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus, Server};
use arctic_core::bedrock::versions::{self, Catalog, Version};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Folders, Install, Player};
use arctic_core::{ProgressInfo, Result};

use crate::tasks::{Event, Outcome, ProgressSnapshot, Tasks};
use crate::{t, tf};
/// What's on this computer for one edition.
#[derive(Debug, Clone, Default)]
pub struct BedrockScan {
    pub install: Option<Install>,
    pub running: bool,
    /// Gaming Services is installed (needed to install a version).
    pub gaming_services: bool,
    pub folders: Folders,
}
fn choose_video_encoder(
    dirs: &arctic_core::storage::DataDirs,
    ffmpeg_only: bool,
) -> Option<(PathBuf, bedrock::capture::EncoderKind)> {
    if !ffmpeg_only
        && crate::video_encoder::available()
        && let Ok(exe) = std::env::current_exe()
    {
        return Some((exe, bedrock::capture::EncoderKind::ArcticBuiltin));
    }
    arctic_core::ffmpeg::find(dirs)
        .or_else(|| {
            rfd::FileDialog::new()
                .set_title(t!("bedrock.choose_ffmpeg_executable"))
                .add_filter(t!("bedrock.executable"), &["exe"])
                .pick_file()
        })
        .map(|path| (path, bedrock::capture::EncoderKind::Ffmpeg))
}

pub struct ReplayVideoRequest {
    pub id: uuid::Uuid,
    pub edition: Edition,
    pub path: bedrock::offline::CameraPath,
    pub fps: u32,
    pub frames: u32,
    pub ffmpeg_only: bool,
    pub appearance_snapshot: Option<PathBuf>,
    pub start_ms: Option<u32>,
    pub range: Option<bedrock::export_range::ExportRange>,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Mutex<bedrock::export_job::ExportProgress>>,
    pub playback_clock: Option<PlaybackClockLease>,
    pub controls: Vec<serde_json::Value>,
}

/// Passive UI handoff. Native clock IO and the core controller's Drop run on
/// workers even when a window closes or an event is never consumed.
pub struct PlaybackClockLease {
    clock: Option<bedrock::playback_clock::PlaybackClock>,
}
impl PlaybackClockLease {
    pub fn active(&self) -> bool {
        self.clock
            .as_ref()
            .is_some_and(|clock| clock.status()["active"].as_bool() == Some(true))
    }
    pub fn status(&self) -> serde_json::Value {
        self.clock
            .as_ref()
            .map_or(serde_json::Value::Null, |clock| clock.status())
    }
    fn release_worker(&mut self) -> Result<()> {
        if let Some(clock) = self.clock.as_mut() {
            clock.release()?;
        }
        self.clock = None;
        Ok(())
    }
    fn drop_worker(&mut self) {
        drop(self.clock.take());
    }
}
impl Drop for PlaybackClockLease {
    fn drop(&mut self) {
        if let Some(clock) = self.clock.take() {
            std::thread::spawn(move || drop(clock));
        }
    }
}
pub enum PlaybackClockAction {
    Attach,
    Service,
    Release,
    Commands {
        commands: Vec<serde_json::Value>,
        release_first: bool,
    },
}
fn playback_controls(
    session: &mut bedrock::offline::OfflineConnector,
    lease: &mut Option<PlaybackClockLease>,
    commands: Vec<serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    if commands.len() > 32 {
        return Err(arctic_core::Error::Other(
            t!("bedrock.too_many_replay_controls_in_one_playback_job").into(),
        ));
    }
    let mut result = None;
    for command in commands {
        if matches!(
            command["op"].as_str(),
            Some("reset" | "seek" | "step" | "export")
        ) {
            if let Some(owned) = lease.as_mut() {
                owned.release_worker()?;
            }
            *lease = None;
        }
        let reply = if matches!(command["op"].as_str(), Some("pause" | "speed"))
            && let Some(clock) = lease.as_mut().and_then(|lease| lease.clock.as_mut())
        {
            clock.control(session, &command)?
        } else {
            session.request_control(&command, std::time::Duration::from_secs(5))?
        };
        result = Some(reply.state);
        if let Some(clock) = lease.as_mut().and_then(|lease| lease.clock.as_mut()) {
            clock.service(session)?;
        }
    }
    Ok(result)
}
pub struct PlaybackClockResult {
    pub id: uuid::Uuid,
    pub edition: Edition,
    session: Option<bedrock::offline::OfflineConnector>,
    lease: Option<PlaybackClockLease>,
    result: Outcome<Option<serde_json::Value>>,
}
impl PlaybackClockResult {
    pub fn take(
        mut self,
    ) -> (
        uuid::Uuid,
        Edition,
        Option<bedrock::offline::OfflineConnector>,
        Option<PlaybackClockLease>,
        Outcome<Option<serde_json::Value>>,
    ) {
        (
            self.id,
            self.edition,
            self.session.take(),
            self.lease.take(),
            std::mem::replace(&mut self.result, Ok(None)),
        )
    }
}
impl Drop for PlaybackClockResult {
    fn drop(&mut self) {
        let mut session = self.session.take();
        let mut lease = self.lease.take();
        if session.is_some() || lease.is_some() {
            std::thread::spawn(move || {
                if let Some(lease) = lease.as_mut() {
                    let _ = lease.release_worker();
                    lease.drop_worker();
                }
                // Drop the controller on this worker before stopping the helper.
                drop(lease);
                if let Some(session) = session.as_mut() {
                    let _ = session.stop();
                }
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingAction {
    Begin,
    Finish,
    Cancel,
}

/// Returning the connection and lease is mandatory even on failure: the UI
/// can retry a slow finalization without destroying the only live world.
pub struct NativeRecordingResult {
    pub id: uuid::Uuid,
    pub edition: Edition,
    pub session: Option<bedrock::offline::OfflineConnector>,
    pub lease: Option<bedrock::recording::PairLease>,
    pub result: Outcome<Option<bedrock::recording::PairOutcome>>,
}
impl NativeRecordingResult {
    pub fn take(
        mut self,
    ) -> (
        uuid::Uuid,
        Edition,
        Option<bedrock::offline::OfflineConnector>,
        Option<bedrock::recording::PairLease>,
        Outcome<Option<bedrock::recording::PairOutcome>>,
    ) {
        (
            self.id,
            self.edition,
            self.session.take(),
            self.lease.take(),
            std::mem::replace(&mut self.result, Ok(None)),
        )
    }
}
impl Drop for NativeRecordingResult {
    fn drop(&mut self) {
        if self.session.is_some() || self.lease.is_some() {
            // Also runs when an enqueued event is never consumed during exit.
            release_recording(self.edition, self.session.take(), self.lease.take());
        }
    }
}
fn release_recording(
    edition: Edition,
    mut session: Option<bedrock::offline::OfflineConnector>,
    lease: Option<bedrock::recording::PairLease>,
) {
    std::thread::spawn(move || {
        if let Some(lease) = lease {
            let saved = session
                .as_mut()
                .map(|s| bedrock::recording::finish(s, edition, &lease));
            if !matches!(saved, Some(Ok(_)))
                && let Err(error) = bedrock::recording::cancel(edition, &lease)
            {
                eprintln!(
                    "[Arctic Bedrock] appearance cleanup for {}: {error}", // i18n-ignore
                    lease.recording_id
                );
            }
        }
        if let Some(session) = session.as_mut() {
            let _ = session.stop();
        }
    });
}

/// A Bedrock job's result, for the Bedrock tab.
// Sent boxed (`Event::Bedrock`), so its largest variant costs nothing elsewhere.
#[allow(clippy::large_enum_variant)]
pub enum BedrockEvent {
    PlaybackClockFinished(PlaybackClockResult),
    Scanned(Edition, BedrockScan),
    Catalog(Outcome<Catalog>),
    InstallProgress(ProgressSnapshot),
    /// A version install finished (edition, version, result).
    Installed(Edition, String, Outcome<()>),
    Worlds(PathBuf, Vec<World>),
    Packs(PathBuf, Vec<Pack>),
    Servers(PathBuf, Vec<Server>),
    /// (address, status)
    ServerStatus(String, Outcome<BedrockStatus>),
    OfflineStarted(Outcome<bedrock::offline::OfflineConnector>),
    OfflineStopped(Outcome<String>),
    AppearancePicked {
        cape: bool,
        path: Option<PathBuf>,
    },
    ReplayPicked {
        recording: bool,
        path: Option<PathBuf>,
    },
    CameraLoaded(Outcome<bedrock::offline::CameraPath>),
    ReplayAppearancePicked(Option<PathBuf>),
    CameraExportPicked {
        file: Option<PathBuf>,
        path: bedrock::offline::CameraPath,
        fps: u32,
    },
    NativeExportFinished {
        id: uuid::Uuid,
        session: bedrock::offline::OfflineConnector,
        result: Outcome<Option<bedrock::export_job::ExportOutcome>>,
    },
    NativeRecordingFinished(NativeRecordingResult),
    /// A world, pack or server job finished (what to say).
    Done(Outcome<String>),
}

impl Tasks {
    pub fn bedrock_playback_clock(
        &self,
        id: uuid::Uuid,
        edition: Edition,
        mut session: bedrock::offline::OfflineConnector,
        mut lease: Option<PlaybackClockLease>,
        action: PlaybackClockAction,
    ) {
        self.run(move |t| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> Result<Option<serde_json::Value>> {
                    match action {
                        PlaybackClockAction::Attach => {
                            if lease.is_some() {
                                return Err(arctic_core::Error::Other(
                                    t!("bedrock.a_playback_clock_is_already_attached").into(),
                                ));
                            }
                            let Some(bundle) = rfd::FileDialog::new()
                                .set_title(t!("bedrock.choose_the_native_bundle_already_loaded_in"))
                                .pick_folder()
                            else {
                                return Ok(None);
                            };
                            let clock = bedrock::playback_clock::PlaybackClock::attach(
                                &mut session,
                                edition,
                                &bundle,
                            )?;
                            let status = clock.status();
                            lease = Some(PlaybackClockLease { clock: Some(clock) });
                            Ok(Some(status))
                        }
                        PlaybackClockAction::Release => {
                            if let Some(owned) = lease.as_mut() {
                                owned.release_worker()?;
                            }
                            lease = None;
                            Ok(None)
                        }
                        PlaybackClockAction::Service => {
                            let clock = lease
                                .as_mut()
                                .and_then(|lease| lease.clock.as_mut())
                                .ok_or_else(|| {
                                    arctic_core::Error::Other(
                                        t!("bedrock.no_playback_clock_is_attached").into(),
                                    )
                                })?;
                            clock.service(&mut session)?;
                            Ok(Some(clock.status()))
                        }
                        PlaybackClockAction::Commands {
                            commands,
                            release_first,
                        } => {
                            if release_first {
                                if let Some(owned) = lease.as_mut() {
                                    owned.release_worker()?;
                                }
                                lease = None;
                            }
                            playback_controls(&mut session, &mut lease, commands)
                        }
                    }
                },
            ))
            .unwrap_or_else(|_| {
                Err(arctic_core::Error::Other(
                    t!("bedrock.playback_clock_worker_stopped_unexpectedly").into(),
                ))
            })
            .map_err(|error| error.to_string());
            t.bedrock(BedrockEvent::PlaybackClockFinished(PlaybackClockResult {
                id,
                edition,
                session: Some(session),
                lease,
                result,
            }));
        });
    }
    pub fn bedrock_paired_recording(
        &self,
        id: uuid::Uuid,
        edition: Edition,
        mut session: Option<bedrock::offline::OfflineConnector>,
        mut lease: Option<bedrock::recording::PairLease>,
        action: RecordingAction,
    ) {
        self.run(move |t| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> Result<Option<bedrock::recording::PairOutcome>> {
                    match action {
                        RecordingAction::Begin => {
                            if lease.is_some() {
                                return Err(arctic_core::Error::Other(
                                    t!("bedrock.an_appearance_take_already_owns_this").into(),
                                ));
                            }
                            let session = session.as_mut().ok_or_else(|| {
                                arctic_core::Error::Other(
                                    t!("bedrock.the_recording_connection_is_unavailable").into(),
                                )
                            })?;
                            let Some(bundle) = rfd::FileDialog::new()
                                .set_title(t!("bedrock.choose_the_native_bundle_already_loaded_in"))
                                .pick_folder()
                            else {
                                return Ok(None);
                            };
                            bedrock::recording::begin(session, edition, &bundle, &mut lease)?;
                            Ok(None)
                        }
                        RecordingAction::Finish => {
                            let owned = lease.as_ref().ok_or_else(|| {
                                arctic_core::Error::Other(
                                    t!("bedrock.no_paired_recording_is_active").into(),
                                )
                            })?;
                            let session = session.as_mut().ok_or_else(|| {
                                arctic_core::Error::Other(
                                    t!("bedrock.the_packet_connection_ended_before_paired").into(),
                                )
                            })?;
                            let outcome = bedrock::recording::finish(session, edition, owned)?;
                            lease = None;
                            Ok(Some(outcome))
                        }
                        RecordingAction::Cancel => {
                            let owned = lease.as_ref().ok_or_else(|| {
                                arctic_core::Error::Other(
                                    t!("bedrock.no_owned_appearance_take_needs_cancellation")
                                        .into(),
                                )
                            })?;
                            bedrock::recording::cancel(edition, owned)?;
                            lease = None;
                            Ok(None)
                        }
                    }
                },
            ))
            .unwrap_or_else(|_| {
                Err(arctic_core::Error::Other(
                    t!("bedrock.paired_recording_worker_stopped_unexpectedly").into(),
                ))
            })
            .map_err(|error| error.to_string());
            t.bedrock(BedrockEvent::NativeRecordingFinished(
                NativeRecordingResult {
                    id,
                    edition,
                    session,
                    lease,
                    result,
                },
            ));
        });
    }
    pub fn bedrock_release_recording(
        &self,
        edition: Edition,
        session: Option<bedrock::offline::OfflineConnector>,
        lease: Option<bedrock::recording::PairLease>,
    ) {
        release_recording(edition, session, lease);
    }
    /// Own the existing connection on this worker, including through dialog
    /// cancellation and export failure; always return it to its matching UI job.
    pub fn bedrock_export_video(
        &self,
        mut session: bedrock::offline::OfflineConnector,
        request: ReplayVideoRequest,
    ) {
        self.run(move |t| {
            let ReplayVideoRequest {
                id,
                edition,
                path,
                fps,
                frames,
                ffmpeg_only,
                appearance_snapshot,
                start_ms,
                range,
                cancel,
                progress,
                mut playback_clock,
                controls,
            } = request;
            use std::sync::atomic::Ordering;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> Result<Option<bedrock::export_job::ExportOutcome>> {
                    playback_controls(&mut session, &mut playback_clock, controls)?;
                    if let Some(clock) = playback_clock.as_mut() {
                        clock.release_worker()?;
                    }
                    playback_clock = None;
                    *progress.lock().unwrap_or_else(|e| e.into_inner()) =
                        bedrock::export_job::ExportProgress {
                            stage: "choosing_files",
                            frames_queued: 0,
                            frames_total: frames,
                        };
                    t.repaint();
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    let Some(bundle) = rfd::FileDialog::new()
                        .set_title(t!("bedrock.choose_the_native_bundle_already_loaded_in"))
                        .pick_folder()
                    else {
                        return Ok(None);
                    };
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    let Some((ffmpeg, encoder)) = choose_video_encoder(t.dirs(), ffmpeg_only)
                    else {
                        return Ok(None);
                    };
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    let Some(output) = rfd::FileDialog::new()
                        .set_title(t!("bedrock.export_a_new_replay_mp4"))
                        .add_filter(t!("bedrock.mp4_video"), &["mp4"])
                        .set_file_name("Bedrock replay.mp4")
                        .save_file()
                    else {
                        return Ok(None);
                    };
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                    let options = bedrock::export_job::ExportOptions {
                        bundle,
                        ffmpeg,
                        encoder,
                        output,
                        fps,
                        frames,
                        camera_path: path.clone(),
                        settle_frames: 3,
                        appearance_snapshot,
                        start_ms,
                        range,
                    };
                    bedrock::export_job::run(&mut session, edition, options, &cancel, |next| {
                        *progress.lock().unwrap_or_else(|e| e.into_inner()) = next;
                        t.repaint();
                    })
                    .map(Some)
                },
            ))
            .unwrap_or_else(|_| {
                Err(arctic_core::Error::Other(
                    t!("bedrock.replay_export_worker_stopped_unexpectedly").into(),
                ))
            })
            .map_err(|error| error.to_string());
            if let Some(clock) = playback_clock.as_mut() {
                clock.drop_worker();
            }
            t.bedrock(BedrockEvent::NativeExportFinished {
                id,
                session,
                result,
            });
        });
    }
    pub fn bedrock_record_video(&self, edition: Edition, seconds: u32, fps: u32) {
        self.bedrock_record_video_with_encoder(edition, seconds, fps, false);
    }
    pub fn bedrock_record_video_with_encoder(
        &self,
        edition: Edition,
        seconds: u32,
        fps: u32,
        ffmpeg_only: bool,
    ) {
        self.run(move |t| {
            let result = (|| -> Result<String> {
                let Some(bundle) = rfd::FileDialog::new()
                    .set_title(t!("bedrock.choose_the_native_bundle_already_loaded_in"))
                    .pick_folder()
                else {
                    return Ok(t!("bedrock.video_recording_cancelled").into());
                };
                let Some((ffmpeg, encoder)) = choose_video_encoder(t.dirs(), ffmpeg_only) else {
                    return Ok(t!("bedrock.video_recording_cancelled").into());
                };
                let Some(output) = rfd::FileDialog::new()
                    .set_title(t!("bedrock.record_a_new_mp4_clip"))
                    .add_filter(t!("bedrock.mp4_video"), &["mp4"])
                    .set_file_name("Bedrock clip.mp4")
                    .save_file()
                else {
                    return Ok(t!("bedrock.video_recording_cancelled").into());
                };
                let file = bedrock::native::record_video_with_encoder(
                    edition, &bundle, &ffmpeg, encoder, &output, seconds, fps,
                )?;
                Ok(tf!("bedrock.video_saved", file = file.display()))
            })();
            t.bedrock(BedrockEvent::Done(
                result.map_err(|error| error.to_string()),
            ));
        });
    }
    fn bedrock(&self, event: BedrockEvent) {
        self.send(Event::Bedrock(Box::new(event)));
    }

    pub fn bedrock_offline(&self, root: PathBuf, options: bedrock::offline::OfflineOptions) {
        self.run(move |t| {
            let result = bedrock::offline::start(&root, &options).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::OfflineStarted(result));
        });
    }
    pub fn bedrock_replay(&self, root: PathBuf, options: bedrock::offline::PlaybackOptions) {
        self.run(move |t| {
            t.bedrock(BedrockEvent::OfflineStarted(
                bedrock::offline::playback(&root, &options).map_err(|e| e.to_string()),
            ));
        });
    }
    pub fn bedrock_stop_offline(&self, session: bedrock::offline::OfflineConnector) {
        self.bedrock_stop_playback(session, None);
    }
    pub fn bedrock_stop_playback(
        &self,
        mut session: bedrock::offline::OfflineConnector,
        mut clock: Option<PlaybackClockLease>,
    ) {
        self.run(move |t| {
            let clock_result = clock
                .as_mut()
                .map_or(Ok(()), |clock| clock.release_worker())
                .map_err(|error| error.to_string());
            if let Some(clock) = clock.as_mut() {
                clock.drop_worker();
            }
            let result = session
                .stop()
                .map_err(|e| e.to_string())
                .and_then(|events| {
                    if let Some(error) = events.iter().find(|e| e.event == "error") {
                        return Err(error
                            .message
                            .clone()
                            .unwrap_or_else(|| t!("bedrock.bedrock_session_failed").into()));
                    }
                    Ok(events
                        .iter()
                        .find(|e| e.event == "recording_saved")
                        .and_then(|e| e.message.as_ref())
                        .map_or_else(
                            || t!("bedrock.local_bedrock_session_stopped").into(),
                            |file| tf!("bedrock.replay_saved", file = file),
                        ))
                });
            let result = match (result, clock_result) {
                (Ok(message), Ok(())) => Ok(message),
                (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
                (Err(error), Err(clock)) => Err(tf!(
                    "bedrock.native_clock_cleanup",
                    error = error,
                    clock = clock
                )),
            };
            t.bedrock(BedrockEvent::OfflineStopped(result));
        });
    }
    pub fn bedrock_replay_file(&self, recording: bool) {
        self.run(move |t| {
            let picker =
                rfd::FileDialog::new().add_filter(t!("bedrock.arctic_bedrock_replay"), &["abr"]);
            let path = if recording {
                picker
                    .set_title(t!("bedrock.record_a_new_bedrock_replay"))
                    .set_file_name("Bedrock recording.abr")
                    .save_file()
            } else {
                picker
                    .set_title(t!("bedrock.watch_a_bedrock_replay"))
                    .pick_file()
            };
            t.bedrock(BedrockEvent::ReplayPicked { recording, path });
        });
    }

    pub fn bedrock_replay_appearance(&self) {
        self.run(move |t| {
            let file = rfd::FileDialog::new()
                .set_title(t!("bedrock.choose_a_recorded_arctic_appearance_snapshot"))
                .add_filter(t!("bedrock.arctic_appearance_snapshot"), &["zip"])
                .pick_file();
            t.bedrock(BedrockEvent::ReplayAppearancePicked(file));
        });
    }

    pub fn bedrock_camera_open(&self) {
        self.run(move |t| {
            let Some(file) = rfd::FileDialog::new()
                .set_title(t!("bedrock.open_camera_path_or_java_arctic_arctic_json"))
                .add_filter(t!("bedrock.camera_path"), &["json"])
                .pick_file()
            else {
                return;
            };
            let result = (|| -> Result<bedrock::offline::CameraPath> {
                if std::fs::metadata(&file)
                    .map_err(|e| arctic_core::Error::Other(e.to_string()))?
                    .len()
                    > 1024 * 1024
                {
                    return Err(arctic_core::Error::Other(
                        t!("bedrock.camera_path_exceeds_1_mib").into(),
                    ));
                }
                // Bound the actual read too, in case the file changes after
                // metadata was read. Parsing rejects the sentinel extra byte.
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(file)
                    .and_then(|file| file.take(1024 * 1024 + 1).read_to_end(&mut bytes))
                    .map_err(|e| arctic_core::Error::Other(e.to_string()))?;
                bedrock::offline::CameraPath::from_json(&bytes)
            })();
            t.bedrock(BedrockEvent::CameraLoaded(
                result.map_err(|e| e.to_string()),
            ));
        });
    }

    pub fn bedrock_camera_save(&self, path: bedrock::offline::CameraPath) {
        self.run(move |t| {
            use std::io::Write;
            let Some(file) = rfd::FileDialog::new()
                .set_title(t!("bedrock.save_replay_camera_path"))
                .add_filter(t!("bedrock.camera_path"), &["json"])
                .set_file_name("Bedrock camera.json")
                .save_file()
            else {
                return;
            };
            let result = (|| -> Result<String> {
                path.validate()?;
                let bytes = serde_json::to_vec_pretty(&path)?;
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&file)
                    .map_err(|e| {
                        arctic_core::Error::Other(tf!(
                            "bedrock.choose_a_new_camera_path_filename",
                            e = e
                        ))
                    })?;
                output
                    .write_all(&bytes)
                    .map_err(|e| arctic_core::Error::Other(e.to_string()))?;
                Ok(tf!("bedrock.camera_path_saved", file = file.display()))
            })();
            t.bedrock(BedrockEvent::Done(result.map_err(|e| e.to_string())));
        });
    }

    pub fn bedrock_camera_export_file(&self, path: bedrock::offline::CameraPath, fps: u32) {
        self.run(move |t| {
            let file = rfd::FileDialog::new()
                .set_title(t!("bedrock.export_replay_with_camera_path"))
                .add_filter(t!("bedrock.arctic_bedrock_replay"), &["abr"])
                .set_file_name("Bedrock edited.abr")
                .save_file();
            t.bedrock(BedrockEvent::CameraExportPicked { file, path, fps });
        });
    }

    pub fn bedrock_appearance(&self, cape: bool) {
        self.run(move |t| {
            let path = rfd::FileDialog::new()
                .set_title(if cape {
                    t!("bedrock.choose_a_static_cape_png")
                } else {
                    t!("bedrock.choose_a_minecraft_skin_png")
                })
                .add_filter(t!("bedrock.png_image"), &["png"])
                .pick_file();
            t.bedrock(BedrockEvent::AppearancePicked { cape, path });
        });
    }

    pub fn bedrock_scan(&self, edition: Edition) {
        self.run(move |t| {
            let install = bedrock::find(edition);
            let running = install.is_some() && bedrock::running(edition);
            t.bedrock(BedrockEvent::Scanned(
                edition,
                BedrockScan {
                    install,
                    running,
                    gaming_services: bedrock::gaming_services(),
                    folders: bedrock::folders(edition),
                },
            ));
        });
    }

    /// Start Bedrock, with `client` (a third-party client) loaded into it when one is chosen.
    pub fn bedrock_launch(&self, edition: Edition, client: Option<bedrock::client::Client>) {
        self.run(move |t| {
            let reporter = t.clone();
            let result = bedrock::launch_with_client(t.dirs(), edition, client, move |message| {
                reporter.bedrock(BedrockEvent::Done(Ok(message)));
            })
            .map(|_| format!("Starting {}", edition.name()))
            .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
            // It shows as running once its window is up.
            std::thread::sleep(std::time::Duration::from_secs(4));
            t.bedrock_scan(edition);
        });
    }

    pub fn bedrock_native_client(&self, edition: Edition) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("bedrock.choose_the_built_arctic_bedrock_native"))
                .pick_folder();
            let result = match picked {
                Some(bundle) => bedrock::native::launch(edition, &bundle)
                    .map(|()| t!("bedrock.native_preview_is_rendering_right_shift").to_owned())
                    .map_err(|e| e.to_string()),
                None => Ok(t!("bedrock.native_client_selection_cancelled").to_owned()),
            };
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_scan(edition);
        });
    }

    pub fn bedrock_catalog(&self) {
        self.run(|t| {
            let result = versions::catalog(&t.dirs().cache()).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Catalog(result));
        });
    }

    pub fn bedrock_install(&self, version: Version, cancel: Arc<AtomicBool>) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| {
                t.bedrock(BedrockEvent::InstallProgress(ProgressSnapshot::from(p)));
            };
            let result = versions::install(&version, &t.dirs().cache(), &cancel, &progress)
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Installed(
                version.edition,
                version.id.clone(),
                result,
            ));
            t.bedrock_scan(version.edition);
        });
    }

    pub fn bedrock_worlds(&self, player: Player) {
        self.run(move |t| {
            let list = worlds::list(&player);
            t.bedrock(BedrockEvent::Worlds(player.com_mojang, list));
        });
    }

    pub fn bedrock_packs(&self, shared: Player) {
        self.run(move |t| {
            let list = packs::list(&shared);
            t.bedrock(BedrockEvent::Packs(shared.com_mojang, list));
        });
    }

    pub fn bedrock_servers(&self, player: Player) {
        self.run(move |t| {
            let list = servers::list(&player);
            let addresses: Vec<String> = list.iter().map(Server::address).collect();
            t.bedrock(BedrockEvent::Servers(player.com_mojang, list));
            servers::ping_each(&addresses, |i, result| {
                t.bedrock(BedrockEvent::ServerStatus(
                    addresses[i].clone(),
                    result.map_err(|e| e.to_string()),
                ));
            });
        });
    }

    pub fn bedrock_ping(&self, address: String) {
        self.run(move |t| {
            let result = servers::ping(&address).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::ServerStatus(address, result));
        });
    }

    /// Ask for `.mcworld` files and import them for `player`.
    pub fn bedrock_import_worlds(&self, player: Player) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("bedrock.choose_bedrock_worlds"))
                .add_filter(t!("bedrock.bedrock_world"), &["mcworld", "zip"])
                .pick_files();
            t.note_if_picker_missing(&picked);
            let Some(files) = picked else {
                return;
            };
            let result = each(&files, |f| worlds::import(f, &player), "world");
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_worlds(player);
        });
    }

    /// Import dropped files: worlds, packs or add-ons, by their extension.
    pub fn bedrock_import_files(&self, files: Vec<PathBuf>, player: Player, shared: Player) {
        self.run(move |t| {
            let mut said = Vec::new();
            let mut error = None;
            for file in &files {
                let ext = file
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let result = if ext == "mcworld" || file.join("level.dat").is_file() {
                    worlds::import(file, &player).map(|name| vec![name])
                } else {
                    packs::import(file, &shared)
                };
                match result {
                    Ok(names) => said.extend(names),
                    Err(e) => error = Some(e.to_string()),
                }
            }
            let result = match (said.len(), error) {
                (0, Some(e)) => Err(e),
                (0, None) => Ok(t!("bedrock.nothing_to_import").to_owned()),
                (1, _) => Ok(tf!("bedrock.imported", value = said[0])),
                (n, _) => Ok(tf!("bedrock.imported_items", n_value = n)),
            };
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_worlds(player);
            t.bedrock_packs(shared);
        });
    }

    /// Save a world as a `.mcworld` where the player picks.
    pub fn bedrock_export_world(&self, world: World) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("bedrock.export_world"))
                .set_file_name(format!("{}.mcworld", world.name))
                .add_filter(t!("bedrock.bedrock_world"), &["mcworld"])
                .save_file();
            t.note_if_picker_missing(&picked);
            let Some(to) = picked else {
                return;
            };
            let result = worlds::export(&world, &to)
                .map(|()| tf!("bedrock.exported", name = world.name))
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
        });
    }

    pub fn bedrock_backup_world(&self, world: World, backups: PathBuf) {
        self.run(move |t| {
            let result = worlds::backup(&world, &backups)
                .map(|_| {
                    let _ = open::that_detached(&backups);
                    tf!("bedrock.backed_up", name = world.name)
                })
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
        });
    }

    /// Ask for `.mcpack` / `.mcaddon` files and import them.
    pub fn bedrock_import_packs(&self, shared: Player) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("bedrock.choose_packs_or_add_ons"))
                .add_filter(t!("bedrock.bedrock_pack"), &["mcpack", "mcaddon", "zip"])
                .pick_files();
            t.note_if_picker_missing(&picked);
            let Some(files) = picked else {
                return;
            };
            let result = (|| -> Result<Vec<String>> {
                let mut names = Vec::new();
                for f in &files {
                    names.extend(packs::import(f, &shared)?);
                }
                Ok(names)
            })()
            .map(|names| match names.as_slice() {
                [one] => tf!("bedrock.imported_2", one = one),
                many => tf!("bedrock.imported_packs", count = many.len()),
            })
            .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_packs(shared);
        });
    }
}

/// Run `job` for each file; say how it went.
fn each(
    files: &[PathBuf],
    job: impl Fn(&std::path::Path) -> Result<String>,
    what: &str,
) -> Outcome<String> {
    let mut done = Vec::new();
    let mut error = None;
    for f in files {
        match job(f) {
            Ok(name) => done.push(name),
            Err(e) => error = Some(e.to_string()),
        }
    }
    match (done.as_slice(), error) {
        ([], Some(e)) => Err(e),
        ([one], _) => Ok(tf!("bedrock.imported_2", one = one)),
        (many, _) => Ok(tf!("bedrock.imported_s", count = many.len(), what = what)),
    }
}
