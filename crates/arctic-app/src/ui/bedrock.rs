//! Bedrock tab: Minecraft for Windows (Release and Preview). Play it,
//! switch it to another version, and look after its worlds, packs and
//! servers. Everything here works on the game's own folders.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus, Server};
use arctic_core::bedrock::versions::{self, Catalog, Version};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Player};
use eframe::egui::{self, CornerRadius, Id, Modal, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::bedrock_tasks::{
    BedrockEvent, BedrockScan, PlaybackClockAction, PlaybackClockLease, RecordingAction,
    ReplayVideoRequest,
};
use crate::motion::format_bytes;
use crate::tasks::{Outcome, ProgressSnapshot};
use crate::theme;
use crate::toasts::Kind;
use crate::widgets::{self, PlayState};
use crate::{t, tf};
const ICON: f32 = 44.0;
/// While the tab is open, the game's state and folders are looked at again this often (seconds).
const RESCAN_EVERY: f64 = 5.0;
/// Servers are pinged again after this long (seconds).
const PING_EVERY: f64 = 120.0;
/// Versions shown before "Show all".
const VERSIONS_SHOWN: usize = 12;

#[derive(Default)]
struct ReplayEditor {
    generation: u64,
    position: u32,
    duration: u32,
    paused: bool,
    seek: u32,
    keyframe_time: u32,
    pose: bedrock::offline::CameraPose,
    frames: Vec<bedrock::offline::CameraKeyframe>,
    export_fps: u32,
    video_fps: u32,
    video_frames: u32,
    ffmpeg_only: bool,
    rebuild_video: bool,
    video_start_ms: u32,
    video_range: bool,
    video_end_ms: u32,
    appearance_snapshot: Option<PathBuf>,
    client_initialized: bool,
    packet_ready: bool,
    bootstrap_time: u32,
    viewers: u32,
    initialized_viewers: u32,
    resource_packs: Option<bedrock::offline::ReplayResourcePacks>,
    automatic_reconnect_available: bool,
    world_reset: Option<bedrock::offline::ReplayWorldReset>,
}

struct ReplayVideoExport {
    id: uuid::Uuid,
    address: String,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<bedrock::export_job::ExportProgress>>,
    stop_after: bool,
}
struct PairedRecording {
    edition: Edition,
    lease: bedrock::recording::PairLease,
}
struct RecordingWork {
    id: uuid::Uuid,
    edition: Edition,
    action: RecordingAction,
    stop_after: bool,
}
struct PlaybackClockWork {
    id: uuid::Uuid,
    edition: Edition,
    address: String,
    stop_after: bool,
}
impl Drop for ReplayVideoExport {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Worlds,
    Versions,
    Packs,
    Servers,
    Client,
    Offline,
}

enum Ping {
    Waiting,
    Up(BedrockStatus),
    Down(String),
}

/// A version being installed.
struct Installing {
    edition: Edition,
    version: String,
    progress: Option<ProgressSnapshot>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct BedrockUi {
    edition: Edition,
    section: Section,
    scans: HashMap<Edition, BedrockScan>,
    scanning: HashSet<Edition>,
    scanned_at: f64,
    /// The account folder picked (worlds and servers are per account).
    player: Option<String>,
    catalog: Option<Outcome<Catalog>>,
    catalog_loading: bool,
    all_versions: bool,
    installing: Option<Installing>,
    /// Asks before installing over the game.
    confirm: Option<Version>,
    /// (folder listed, list)
    worlds: Option<(PathBuf, Vec<World>)>,
    packs: Option<(PathBuf, Vec<Pack>)>,
    servers: Option<(PathBuf, Vec<Server>)>,
    status: HashMap<String, Ping>,
    pinged_at: f64,
    /// "Add a server" fields.
    server_name: String,
    server_address: String,
    /// A world being renamed: (its folder, the new name).
    renaming: Option<(PathBuf, String)>,
    busy: bool,
    offline_name: String,
    offline_server: String,
    offline_skin: Option<PathBuf>,
    offline_cape: Option<PathBuf>,
    offline_slim: bool,
    offline_record_to: Option<PathBuf>,
    replay_file: Option<PathBuf>,
    replay_speed: f64,
    replay_start_ms: u32,
    replay_mode: bool,
    replay_lan: bool,
    capture_seconds: u32,
    capture_fps: u32,
    replay_editor: ReplayEditor,
    video_export: Option<ReplayVideoExport>,
    playback_clock: Option<PlaybackClockLease>,
    playback_work: Option<PlaybackClockWork>,
    playback_service_at: Option<std::time::Instant>,
    pending_replay_controls: Vec<(u64, serde_json::Value)>,
    paired_recording: Option<PairedRecording>,
    recording_work: Option<RecordingWork>,
    recording_finalized: bool,
    close_after_recording: bool,
    offline_starting: bool,
    offline_stopping: bool,
    offline_session: Option<bedrock::offline::OfflineConnector>,
    offline_message: String,
}
impl BedrockUi {
    pub(crate) fn background_active(&self) -> bool {
        self.offline_session.is_some()
            || self.video_export.is_some()
            || self.recording_work.is_some()
            || self.playback_work.is_some()
            || self.paired_recording.is_some()
            || self.offline_starting
            || self.offline_stopping
    }
    pub(crate) fn recording_close_ready(&mut self) -> bool {
        if self.close_after_recording
            && self.recording_work.is_none()
            && self.paired_recording.is_none()
            && self.offline_session.is_none()
            && !self.offline_stopping
            && self.playback_work.is_none()
        {
            self.close_after_recording = false;
            return true;
        }
        false
    }
    pub(crate) fn cancel_video_export(&self) {
        if let Some(export) = self.video_export.as_ref() {
            export.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub(crate) fn close_recording(&mut self, tasks: &crate::tasks::Tasks) {
        if let Some(work) = self.playback_work.as_mut() {
            work.stop_after = true;
        }
        if let Some(clock) = self.playback_clock.take()
            && let Some(session) = self.offline_session.take()
        {
            tasks.bedrock_stop_playback(session, Some(clock));
        }
        if let Some(owned) = self.paired_recording.take() {
            tasks.bedrock_release_recording(
                owned.edition,
                self.offline_session.take(),
                Some(owned.lease),
            );
        }
    }
}

impl ArcticApp {
    pub(crate) fn bedrock_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let now = ui.input(|i| i.time);
        if !bedrock::supported() {
            widgets::page_header(ui, p, t!("bedrock.bedrock_edition"), "");
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(t!("bedrock.bedrock_edition_is_played_on_windows"))
                        .size(17.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(t!("bedrock.minecraft_for_windows_only_runs_there_so_it"))
                        .color(p.muted),
                );
            });
            return;
        }
        let edition = self.bedrock.edition;
        if now - self.bedrock.scanned_at >= RESCAN_EVERY {
            self.bedrock_rescan(now);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(RESCAN_EVERY));
        self.bedrock_dropped_files(ui.ctx());

        widgets::row(ui, |ui| {
            ui.vertical(|ui| {
                widgets::page_header(
                    ui,
                    p,
                    t!("bedrock.bedrock_edition"),
                    t!("bedrock.minecraft_for_windows_play_it_switch"),
                );
            });
        });
        widgets::row(ui, |ui| {
            for e in Edition::ALL {
                if ui
                    .add_enabled(
                        self.bedrock.offline_session.is_none()
                            && self.bedrock.video_export.is_none()
                            && self.bedrock.playback_work.is_none()
                            && self.bedrock.recording_work.is_none()
                            && self.bedrock.paired_recording.is_none()
                            && !self.bedrock.offline_starting
                            && !self.bedrock.offline_stopping,
                        egui::Button::new(RichText::new(e.name()).size(15.0))
                            .selected(edition == e),
                    )
                    .clicked()
                    && edition != e
                {
                    self.bedrock.edition = e;
                    self.bedrock.player = None;
                    self.bedrock_forget_lists();
                    self.bedrock.scanned_at = f64::NEG_INFINITY;
                }
            }
        });
        ui.add_space(8.0);
        self.bedrock_hero(ui);
        if bedrock::supported() {
            ui.add_space(10.0);
            self.bedrock_client_card(ui);
        }
        ui.add_space(14.0);

        let Some(scan) = self.bedrock.scans.get(&edition).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            let s = &mut self.bedrock.section;
            ui.selectable_value(s, Section::Worlds, t!("bedrock.worlds"));
            ui.selectable_value(s, Section::Packs, t!("bedrock.packs"));
            ui.selectable_value(s, Section::Servers, t!("bedrock.servers"));
            ui.selectable_value(s, Section::Versions, t!("bedrock.versions"));
            ui.selectable_value(s, Section::Client, t!("bedrock.arctic_client"));
            ui.selectable_value(s, Section::Offline, t!("bedrock.offline_play"));
            if scan.folders.players.len() > 1
                && matches!(self.bedrock.section, Section::Worlds | Section::Servers)
            {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.bedrock_player_picker(ui, &scan);
                });
            }
        });
        ui.add_space(8.0);
        match self.bedrock.section {
            Section::Versions => self.bedrock_versions(ui, &scan),
            Section::Worlds => self.bedrock_worlds(ui, &scan),
            Section::Packs => self.bedrock_packs(ui, &scan),
            Section::Servers => self.bedrock_servers(ui, &scan, now),
            Section::Client => self.bedrock_native_client(ui),
            Section::Offline => self.bedrock_offline(ui),
        }
        self.bedrock_confirm(ui.ctx());
    }

    pub(crate) fn bedrock_poll_offline(&mut self) {
        let b = &mut self.bedrock;
        let Some(session) = &mut b.offline_session else {
            return;
        };
        while let Some(event) = session.try_event() {
            if event
                .generation
                .is_some_and(|generation| generation < b.replay_editor.generation)
            {
                continue;
            }
            if let Some(generation) = event.generation {
                b.replay_editor.generation = generation;
            }
            if event.event == "replay_state" {
                b.replay_editor.position = event.position_ms.unwrap_or_default();
                b.replay_editor.duration = event.duration_ms.unwrap_or_default();
                b.replay_editor.paused = event.paused.unwrap_or_default();
                b.replay_editor.client_initialized = event.client_initialized.unwrap_or(false);
                b.replay_editor.packet_ready = event.packet_ready.unwrap_or(false);
                b.replay_editor.bootstrap_time = event.bootstrap_time_ms.unwrap_or_default();
                b.replay_editor.viewers = event.viewers.unwrap_or_default();
                b.replay_editor.initialized_viewers = event.initialized_viewers.unwrap_or_default();
                b.replay_editor.resource_packs = event.resource_packs;
                b.replay_editor.automatic_reconnect_available =
                    event.automatic_reconnect_available.unwrap_or(false);
                b.replay_editor.world_reset = event.world_reset;
                b.replay_speed = event.speed.unwrap_or(1.0);
                continue;
            }
            if matches!(
                event.event.as_str(),
                "replay_joined"
                    | "replay_reset"
                    | "replay_disconnected"
                    | "replay_reset_failed"
                    | "error"
            ) {
                b.replay_editor.client_initialized = false;
                b.replay_editor.packet_ready = false;
            }
            b.offline_message = match event.event.as_str() {
                "joined" => tf!(
                    "bedrock.connected_as_no_xbox_authentication",
                    username = session.username
                ),
                "disconnected" | "error" | "control_error" => event.message.unwrap_or(event.event),
                "recording_started" => {
                    tf!(
                        "bedrock.recording_to",
                        message = event.message.unwrap_or_default()
                    )
                }
                "recording_saved" => tf!(
                    "bedrock.replay_saved_2",
                    message = event.message.unwrap_or_default()
                ),
                "replay_joined" => t!("bedrock.replay_connected_waiting_for_the_clients").into(),
                "replay_initialized" => t!("bedrock.replay_client_acknowledged_world").into(),
                "replay_reset" => event.message.unwrap_or_else(|| {
                    t!("bedrock.waiting_for_a_fresh_replay_world_connection").into()
                }),
                "replay_reset_ready" => {
                    t!("bedrock.fresh_replay_viewer_initialized_recorded").into()
                }
                "replay_reset_failed" => event
                    .message
                    .unwrap_or_else(|| t!("bedrock.replay_reconnect_failed_join_the_local").into()),
                "replay_disconnected" => {
                    t!("bedrock.replay_viewer_disconnected_waiting_for_a").into()
                }
                "replay_finished" => event.message.unwrap_or_default(),
                "replay_exported" => tf!(
                    "bedrock.edited_replay_exported",
                    message = event.message.unwrap_or_default()
                ),
                _ => continue,
            };
        }
        if !session.running() {
            b.replay_editor.client_initialized = false;
            b.replay_editor.packet_ready = false;
            b.replay_editor.viewers = 0;
            b.replay_editor.initialized_viewers = 0;
            b.offline_session = None;
            b.playback_clock = None;
            b.pending_replay_controls.clear();
            if b.offline_message.is_empty() {
                b.offline_message = t!("bedrock.offline_connector_stopped").into();
            }
            if let Some(owned) = b.paired_recording.take() {
                let id = uuid::Uuid::new_v4();
                b.recording_work = Some(RecordingWork {
                    id,
                    edition: owned.edition,
                    action: RecordingAction::Cancel,
                    stop_after: false,
                });
                b.offline_message =
                    t!("bedrock.packet_connection_ended_releasing_its_owned").into();
                self.tasks.bedrock_paired_recording(
                    id,
                    owned.edition,
                    None,
                    Some(owned.lease),
                    RecordingAction::Cancel,
                );
            }
        }
        if b.offline_session.is_some() && b.playback_work.is_none() && b.video_export.is_none() {
            let generation = b.replay_editor.generation;
            let pending = std::mem::take(&mut b.pending_replay_controls);
            let queued = pending.len();
            let commands: Vec<_> = pending
                .into_iter()
                .filter_map(|(owner, command)| (owner == generation).then_some(command))
                .collect();
            if commands.len() != queued {
                b.offline_message = t!("bedrock.replay_world_changed_before_the_queued").into();
            }
            if !commands.is_empty() {
                if b.playback_clock.is_some() {
                    self.bedrock_playback_job(PlaybackClockAction::Commands {
                        commands,
                        release_first: false,
                    });
                    return;
                }
                if let Some(session) = b.offline_session.as_mut() {
                    for command in commands {
                        if let Err(error) = session.control(&command) {
                            b.offline_message = error.to_string();
                        }
                    }
                }
            }
        }
        if self.bedrock.playback_work.is_none()
            && self.bedrock.video_export.is_none()
            && self.bedrock.offline_session.is_some()
            && self
                .bedrock
                .playback_clock
                .as_ref()
                .is_some_and(PlaybackClockLease::active)
            && self
                .bedrock
                .playback_service_at
                .is_none_or(|at| std::time::Instant::now() >= at)
        {
            self.bedrock_playback_job(PlaybackClockAction::Service);
        }
    }

    fn bedrock_recording_job(&mut self, action: RecordingAction, stop_after: bool) {
        let b = &mut self.bedrock;
        if b.recording_work.is_some()
            || b.video_export.is_some()
            || b.playback_work.is_some()
            || b.playback_clock.is_some()
        {
            return;
        }
        let owned = b.paired_recording.take();
        let edition = owned.as_ref().map_or(b.edition, |r| r.edition);
        let id = uuid::Uuid::new_v4();
        b.recording_work = Some(RecordingWork {
            id,
            edition,
            action,
            stop_after,
        });
        self.tasks.bedrock_paired_recording(
            id,
            edition,
            b.offline_session.take(),
            owned.map(|r| r.lease),
            action,
        );
    }
    pub(crate) fn prepare_bedrock_close(&mut self) -> bool {
        if self.bedrock.playback_work.is_some() || self.bedrock.playback_clock.is_some() {
            let b = &mut self.bedrock;
            b.close_after_recording = true;
            b.pending_replay_controls.clear();
            if let Some(work) = b.playback_work.as_mut() {
                work.stop_after = true;
            } else if let Some(session) = b.offline_session.take() {
                b.offline_stopping = true;
                self.tasks
                    .bedrock_stop_playback(session, b.playback_clock.take());
            } else {
                b.playback_clock = None;
            }
            return true;
        }
        if self.bedrock.recording_work.is_some() || self.bedrock.paired_recording.is_some() {
            self.bedrock.close_after_recording = true;
            if let Some(work) = self.bedrock.recording_work.as_mut() {
                work.stop_after = true;
            } else {
                let action = if self.bedrock.offline_session.is_some() {
                    RecordingAction::Finish
                } else {
                    RecordingAction::Cancel
                };
                self.bedrock_recording_job(action, true);
            }
            return true;
        }
        // A successful paired save can still be waiting for helper shutdown.
        self.bedrock.close_after_recording && self.bedrock.offline_stopping
    }

    fn bedrock_offline(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(
                    if self.bedrock.replay_mode
                        && (self.bedrock.offline_session.is_some()
                            || self.bedrock.video_export.is_some()
                            || self.bedrock.playback_work.is_some())
                    {
                        t!("bedrock.watch_an_offline_replay")
                    } else {
                        t!("bedrock.play_with_an_offline_name")
                    },
                )
                .strong()
                .color(p.text),
            );
            ui.label(t!("bedrock.connect_to_a_server_that_accepts_unsigned"));
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!("bedrock.the_connector_needs_node_js_24_with_npm"))
                    .small()
                    .color(p.muted),
            );
            ui.add_space(10.0);
            if let Some(work) = self.bedrock.recording_work.as_mut() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(match work.action {
                        RecordingAction::Begin => t!("bedrock.starting_arctic_appearance_capture"),
                        RecordingAction::Finish => {
                            t!("bedrock.saving_packet_and_appearance_recordings")
                        }
                        RecordingAction::Cancel => {
                            t!("bedrock.releasing_the_owned_appearance_capture")
                        }
                    });
                });
                if ui
                    .add_enabled(
                        !work.stop_after,
                        egui::Button::new(t!("bedrock.stop_local_session_after_saving")),
                    )
                    .clicked()
                {
                    work.stop_after = true;
                }
                ui.label(t!("bedrock.keep_the_recording_world_open_until_saving"));
            } else if let Some(work) = self.bedrock.playback_work.as_mut() {
                ui.label(tf!("bedrock.replay_at", address = work.address));
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("bedrock.coordinating_the_game_clock"));
                });
                if ui
                    .add_enabled(
                        !work.stop_after,
                        egui::Button::new(t!("bedrock.stop_local_session")),
                    )
                    .clicked()
                {
                    work.stop_after = true;
                }
            } else if self.bedrock.paired_recording.is_some()
                && self.bedrock.offline_session.is_none()
            {
                ui.label(&self.bedrock.offline_message);
                ui.label(t!("bedrock.the_connection_ended_before_its_appearance"));
                if ui.button(t!("bedrock.retry_appearance_cleanup")).clicked() {
                    self.bedrock_recording_job(RecordingAction::Cancel, false);
                }
            } else if let Some(export) = self.bedrock.video_export.as_mut() {
                ui.label(tf!("bedrock.replay_at", address = export.address));
                let canceling = export.cancel.load(Ordering::Relaxed);
                let progress = export
                    .progress
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let stage = if canceling {
                    t!("bedrock.canceling_export")
                } else {
                    match progress.stage {
                        "choosing_files" => t!("bedrock.choose_export_files"),
                        "preparing" => t!("bedrock.preparing_replay_video"),
                        "rebuilding_world" => {
                            t!("bedrock.reconnecting_and_preparing_a_fresh_replay")
                        }
                        "binding_source" => t!("bedrock.binding_the_recording_and_camera_inputs"),
                        "capturing" => t!("bedrock.rendering_replay_frames"),
                        "encoding" => t!("bedrock.finishing_video_encoding"),
                        "checking_manifest" => t!("bedrock.checking_captured_frame_records"),
                        "checking_video" => t!("bedrock.checking_decoded_video_frames"),
                        "checking_file" => t!("bedrock.hashing_the_finished_video"),
                        "binding_export" => t!("bedrock.saving_the_export_identity"),
                        _ => t!("bedrock.finishing_export"),
                    }
                };
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(stage);
                });
                ui.add(
                    egui::ProgressBar::new(
                        progress.frames_queued as f32 / progress.frames_total.max(1) as f32,
                    )
                    .text(tf!(
                        "bedrock.frames_progress",
                        queued = progress.frames_queued,
                        total = progress.frames_total
                    )),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!canceling, egui::Button::new(t!("bedrock.cancel_export")))
                        .clicked()
                    {
                        export.cancel.store(true, Ordering::Relaxed);
                    }
                    if ui
                        .add_enabled(
                            !export.stop_after,
                            egui::Button::new(t!("bedrock.stop_local_session")),
                        )
                        .clicked()
                    {
                        export.stop_after = true;
                        export.cancel.store(true, Ordering::Relaxed);
                    }
                });
            } else if let Some(session) = &self.bedrock.offline_session {
                let address = session.address.clone();
                ui.label(tf!(
                    "bedrock.join_in_bedrock",
                    address = address,
                    username = session.username,
                    version = session.version
                ));
                ui.label(&self.bedrock.offline_message);
                let mut stop = false;
                let mut action = None;
                widgets::row(ui, |ui| {
                    if widgets::button(
                        ui,
                        p,
                        Some(Icon::Copy),
                        t!("bedrock.copy_local_address"),
                        false,
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(address.clone());
                    }
                    if widgets::button(ui, p, Some(Icon::Play), t!("bedrock.open_bedrock"), true)
                        .clicked()
                    {
                        self.tasks.bedrock_launch(self.bedrock.edition, None);
                    }
                    stop = widgets::button(
                        ui,
                        p,
                        Some(Icon::Stop),
                        t!("bedrock.stop_local_session"),
                        false,
                    )
                    .clicked();
                });
                if !self.bedrock.replay_mode {
                    if let Some(owned) = self.bedrock.paired_recording.as_ref() {
                        ui.label(tf!(
                            "bedrock.arctic_appearance_capture",
                            to_string_lossy = owned
                                .lease
                                .source
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                        ));
                        ui.horizontal(|ui| {
                            if ui
                                .button(t!("bedrock.save_replay_with_appearance"))
                                .clicked()
                            {
                                action = Some(RecordingAction::Finish);
                            }
                            if ui.button(t!("bedrock.discard_appearance_take")).clicked() {
                                action = Some(RecordingAction::Cancel);
                            }
                        });
                    } else if self.bedrock.offline_record_to.is_some()
                        && !self.bedrock.recording_finalized
                    {
                        if ui.button(t!("bedrock.capture_arctic_appearance")).clicked() {
                            action = Some(RecordingAction::Begin);
                        }
                        ui.label(
                            RichText::new(t!(
                                "bedrock.start_after_joining_the_recording_world_with"
                            ))
                            .small()
                            .color(p.muted),
                        );
                    }
                }
                if stop {
                    if self.bedrock.paired_recording.is_some() {
                        self.bedrock_recording_job(RecordingAction::Finish, true);
                    } else {
                        self.bedrock.offline_stopping = true;
                        if let Some(session) = self.bedrock.offline_session.take() {
                            self.tasks
                                .bedrock_stop_playback(session, self.bedrock.playback_clock.take());
                        }
                        self.bedrock.offline_message =
                            t!("bedrock.stopping_local_session_and_finishing_any").into();
                    }
                } else if let Some(action) = action {
                    self.bedrock_recording_job(action, false);
                }
            } else if self.bedrock.offline_starting || self.bedrock.offline_stopping {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("bedrock.preparing_the_offline_connector"));
                });
            } else {
                ui.add(
                    widgets::text_field(&mut self.bedrock.offline_name)
                        .hint_text(t!("bedrock.offline_name_3_16_letters_digits_or"))
                        .desired_width(330.0),
                );
                ui.add(
                    widgets::text_field(&mut self.bedrock.offline_server)
                        .hint_text(t!("bedrock.offline_server_host_or_host_port"))
                        .desired_width(330.0),
                );
                ui.add_space(6.0);
                ui.label(t!("bedrock.optional_appearance_sent_to_the_server"));
                for cape in [false, true] {
                    widgets::row(ui, |ui| {
                        if widgets::button(
                            ui,
                            p,
                            Some(Icon::Folder),
                            if cape {
                                t!("bedrock.choose_cape")
                            } else {
                                t!("bedrock.choose_skin")
                            },
                            false,
                        )
                        .clicked()
                        {
                            self.tasks.bedrock_appearance(cape);
                        }
                        let selected = if cape {
                            &mut self.bedrock.offline_cape
                        } else {
                            &mut self.bedrock.offline_skin
                        };
                        if let Some(path) = selected.as_ref() {
                            ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                                .on_hover_text(path.display().to_string());
                            if ui.small_button(t!("bedrock.clear")).clicked() {
                                *selected = None;
                                if !cape {
                                    self.bedrock.offline_slim = false;
                                }
                            }
                        }
                    });
                }
                ui.add_enabled(
                    self.bedrock.offline_skin.is_some(),
                    egui::Checkbox::new(
                        &mut self.bedrock.offline_slim,
                        t!("bedrock.slim_skin_arms"),
                    ),
                );
                ui.label(
                    RichText::new(t!("bedrock.reconnect_to_apply_changes_the_server"))
                        .small()
                        .color(p.muted),
                );
                widgets::row(ui, |ui| {
                    if widgets::button(
                        ui,
                        p,
                        Some(Icon::Folder),
                        t!("bedrock.record_connection"),
                        false,
                    )
                    .clicked()
                    {
                        self.tasks.bedrock_replay_file(true);
                    }
                    if let Some(path) = &self.bedrock.offline_record_to {
                        ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                        if ui.small_button(t!("bedrock.clear")).clicked() {
                            self.bedrock.offline_record_to = None;
                        }
                    }
                });
                if !self.bedrock.offline_message.is_empty() {
                    ui.label(&self.bedrock.offline_message);
                }
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Play),
                    t!("bedrock.start_offline_connector"),
                    true,
                )
                .clicked()
                {
                    let options = bedrock::offline::OfflineOptions::new(
                        &self.bedrock.offline_server,
                        &self.bedrock.offline_name,
                        19142,
                        None,
                    )
                    .and_then(|mut options| {
                        options.appearance = bedrock::appearance::Appearance::read(
                            self.bedrock.offline_skin.as_deref(),
                            self.bedrock.offline_cape.as_deref(),
                            self.bedrock.offline_slim,
                        )?;
                        options.record_to = self.bedrock.offline_record_to.clone();
                        Ok(options)
                    });
                    match options {
                        Ok(options) => {
                            self.bedrock.offline_starting = true;
                            self.bedrock.replay_mode = false;
                            self.tasks
                                .bedrock_offline(self.root.root().to_path_buf(), options);
                        }
                        Err(e) => self.bedrock.offline_message = e.to_string(),
                    }
                }
            }
        });
        if self.bedrock.offline_session.is_none()
            && self.bedrock.video_export.is_none()
            && self.bedrock.recording_work.is_none()
            && self.bedrock.paired_recording.is_none()
            && !self.bedrock.offline_starting
            && !self.bedrock.offline_stopping
        {
            ui.add_space(10.0);
            theme::card(p).show(ui, |ui| {
                ui.label(
                    RichText::new(t!("bedrock.watch_a_bedrock_replay"))
                        .strong()
                        .color(p.text),
                );
                ui.checkbox(
                    &mut self.bedrock.replay_lan,
                    t!("bedrock.show_as_a_local_lan_world_preview"),
                );
                if self.bedrock.replay_lan {
                    ui.label(t!("bedrock.advertise_the_recording_under_play_worlds"));
                } else {
                    ui.label(t!("bedrock.serve_a_recorded_offline_connection_locally"));
                }
                ui.label(
                    RichText::new(t!("bedrock.camera_paths_support_protocol_1_26_51_2193"))
                        .small()
                        .color(p.muted),
                );
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Folder),
                    t!("bedrock.choose_recording"),
                    false,
                )
                .clicked()
                {
                    self.tasks.bedrock_replay_file(false);
                }
                if let Some(path) = self.bedrock.replay_file.clone() {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                    if self.bedrock.replay_speed <= 0.0 {
                        self.bedrock.replay_speed = 1.0;
                    }
                    ui.horizontal(|ui| {
                        super::bedrock_camera::speed(ui, &mut self.bedrock.replay_speed);
                        ui.label(t!("bedrock.start_at_ms"));
                        ui.add(
                            egui::DragValue::new(&mut self.bedrock.replay_start_ms)
                                .range(0..=86_400_000),
                        );
                    });
                    if widgets::button(
                        ui,
                        p,
                        Some(Icon::Play),
                        t!("bedrock.start_local_playback"),
                        true,
                    )
                    .clicked()
                    {
                        self.bedrock.offline_starting = true;
                        self.bedrock.replay_mode = true;
                        let transport = if self.bedrock.replay_lan {
                            bedrock::offline::ReplayTransport::Nethernet
                        } else {
                            bedrock::offline::ReplayTransport::Raknet
                        };
                        self.tasks.bedrock_replay(
                            self.root.root().to_path_buf(),
                            bedrock::offline::PlaybackOptions {
                                file: path,
                                listen_port: transport.default_port(),
                                transport,
                                speed: self.bedrock.replay_speed,
                                start_ms: self.bedrock.replay_start_ms,
                            },
                        );
                    }
                }
            });
        }
        if self.bedrock.replay_file.is_some() {
            self.bedrock_replay_editor(ui);
        }
    }

    fn bedrock_playback_job(&mut self, action: PlaybackClockAction) {
        let b = &mut self.bedrock;
        if b.playback_work.is_some()
            || b.video_export.is_some()
            || b.recording_work.is_some()
            || b.paired_recording.is_some()
        {
            return;
        }
        let Some(session) = b.offline_session.take() else {
            return;
        };
        let id = uuid::Uuid::new_v4();
        b.playback_work = Some(PlaybackClockWork {
            id,
            edition: b.edition,
            address: session.address.clone(),
            stop_after: false,
        });
        self.tasks
            .bedrock_playback_clock(id, b.edition, session, b.playback_clock.take(), action);
    }

    fn bedrock_replay_editor(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let resetting = self
            .bedrock
            .replay_editor
            .world_reset
            .as_ref()
            .is_some_and(|r| {
                r.automatic_reconnect
                    && matches!(
                        r.stage.as_str(),
                        "awaiting_connection" | "initializing" | "catching_up"
                    )
            });
        let active =
            self.bedrock.replay_mode && self.bedrock.offline_session.is_some() && !resetting;
        let mut commands = Vec::new();
        let mut start_export = None;
        let mut clock_action = None;
        ui.add_space(10.0);
        theme::card(p).show(ui, |ui| {
            ui.label(RichText::new(t!("bedrock.replay_camera_editor")).strong().color(p.text));
            ui.label(t!("bedrock.edit_position_rotation_and_fov_apply_the"));
            let b = &mut self.bedrock;
            let e = &mut b.replay_editor;
            ui.horizontal_wrapped(|ui| {
                if let Some(clock)=b.playback_clock.as_ref() {
                    let status=clock.status();
                    ui.label(if clock.active() {tf!("bedrock.game_clock_preview", value = status["rate"])}else{t!("bedrock.game_clock_needs_release_before_retrying").into()});
                    if ui.add_enabled(active,egui::Button::new(t!("bedrock.release_game_clock"))).clicked() {clock_action=Some(PlaybackClockAction::Release);}
                } else if ui.add_enabled(active&&e.client_initialized&&e.packet_ready&&e.viewers==1&&e.initialized_viewers==1,
                    egui::Button::new(t!("bedrock.synchronize_game_clock")))
                    .on_hover_text(t!("bedrock.match_replay_speed_and_pause_with_the_native"))
                    .clicked() {clock_action=Some(PlaybackClockAction::Attach);}
            });
            ui.add_enabled_ui(active, |ui| {
                if let Some(reset)=e.world_reset.as_ref().filter(|r|r.stage!="ready") {
                    ui.label(match reset.stage.as_str() {
                        "awaiting_connection" if reset.automatic_reconnect=>t!("bedrock.reconnecting_the_replay_viewer"),
                        "awaiting_connection"=>t!("bedrock.join_the_local_replay_address_again_to_load"),
                        "initializing"=>t!("bedrock.initializing_the_fresh_replay_world"),
                        "catching_up"=>t!("bedrock.queuing_recorded_packets_to_the_requested"),
                        "failed"=>reset.message.as_deref().unwrap_or(t!("bedrock.replay_reconnect_failed_join_the_local_2")),
                        _=>t!("bedrock.preparing_the_replay_world"),
                    });
                }
                if !e.client_initialized {
                    ui.label(t!("bedrock.waiting_for_the_connected_client_to"));
                } else if !e.packet_ready {
                    ui.label(t!("bedrock.catching_up_replay_packets_to_the_playhead"));
                }
                if e.viewers > 1 {
                    ui.label(tf!("bedrock.of_viewers_initialized", initialized_viewers = e.initialized_viewers, viewers = e.viewers));
                }
                if let Some(packs) = e.resource_packs.filter(|packs| packs.offered > 0) {
                    ui.label(tf!("bedrock.replay_resource_packs_of_included", embedded = packs.embedded, offered = packs.offered, format_bytes = format_bytes(packs.embedded_bytes)));
                }
                ui.label(tf!("bedrock.time_progress", position = e.position, duration = e.duration));
                ui.horizontal(|ui| {
                    if ui.add_enabled(!resetting,egui::Button::new(if e.paused { t!("bedrock.resume") } else { t!("bedrock.pause") })).clicked() {
                        commands.push(serde_json::json!({"op":"pause","paused":!e.paused}));
                    }
                    if ui.add_enabled_ui(!resetting,|ui|super::bedrock_camera::speed(ui,&mut b.replay_speed)).inner {
                        commands.push(serde_json::json!({"op":"speed","speed":b.replay_speed}));
                    }
                    ui.add(egui::DragValue::new(&mut e.seek).range(e.bootstrap_time..=e.duration.max(e.bootstrap_time)).suffix(t!("bedrock.ms_suffix")));
                    if ui.add_enabled(!resetting,egui::Button::new(t!("bedrock.seek"))).clicked() {
                        commands.push(if e.seek<=e.position {serde_json::json!({"op":"reset","timeMs":e.seek,"reconnect":e.automatic_reconnect_available})}
                            else {serde_json::json!({"op":"seek","timeMs":e.seek})});
                    }
                    if ui.add_enabled(!resetting,egui::Button::new(t!("bedrock.restart_world"))).clicked() {
                        commands.push(serde_json::json!({"op":"reset","timeMs":e.bootstrap_time,"reconnect":e.automatic_reconnect_available}));
                    }
                });
                ui.label(RichText::new(if e.automatic_reconnect_available {
                    t!("bedrock.backward_seeks_request_a_fresh_local")
                } else {t!("bedrock.backward_seeks_reset_playback_and_require")}).small().color(p.muted));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [("X", &mut e.pose.x), ("Y", &mut e.pose.y), ("Z", &mut e.pose.z),
                    (t!("bedrock.yaw"), &mut e.pose.yaw), (t!("bedrock.pitch"), &mut e.pose.pitch), ("FOV", &mut e.pose.fov)] {
                    ui.label(label); ui.add(egui::DragValue::new(value).speed(0.1));
                }
            });
            ui.horizontal(|ui| {
                ui.label(t!("bedrock.keyframe_time"));
                ui.add(egui::DragValue::new(&mut e.keyframe_time).range(0..=86_400_000).suffix(t!("bedrock.ms_suffix")));
                if ui.add_enabled(active,egui::Button::new(t!("bedrock.use_playhead"))).clicked() {e.keyframe_time=e.position;}
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(active, egui::Button::new(t!("bedrock.preview_position"))).clicked() {
                    commands.push(serde_json::json!({"op":"camera","pose":e.pose}));
                }
                let existing=e.frames.iter().any(|frame|frame.time_ms==e.keyframe_time);
                if ui.add_enabled(existing||e.frames.len()<4096,egui::Button::new(if existing{t!("bedrock.update_keyframe")}else{t!("bedrock.add_keyframe_here")})).clicked() {
                    if let Some(frame) = e.frames.iter_mut().find(|f| f.time_ms == e.keyframe_time) { frame.pose = e.pose; }
                    else { e.frames.push(bedrock::offline::CameraKeyframe {time_ms:e.keyframe_time,pose:e.pose,ease:"catmull_rom".into()}); }
                    e.frames.sort_by_key(|f| f.time_ms);
                }
                if ui.add_enabled(active, egui::Button::new(t!("bedrock.release_camera"))).clicked() {commands.push(serde_json::json!({"op":"camera_clear"}));}
            });
            let mut remove = None;
            egui::ScrollArea::both().max_height(240.0).id_salt("bedrock-camera-frames").show(ui, |ui| {
                egui::Grid::new("bedrock-camera-grid").striped(true).show(ui, |ui| {
                    for heading in [t!("bedrock.time_ms"), "X", "Y", "Z", t!("bedrock.yaw"), t!("bedrock.pitch"), "FOV", t!("bedrock.to_next"), ""] {ui.label(heading);} ui.end_row();
                    for (index, frame) in e.frames.iter_mut().enumerate() {
                        ui.add(egui::DragValue::new(&mut frame.time_ms).range(0..=86_400_000));
                        for value in [&mut frame.pose.x,&mut frame.pose.y,&mut frame.pose.z,&mut frame.pose.yaw,&mut frame.pose.pitch,&mut frame.pose.fov] {
                            ui.add(egui::DragValue::new(value).speed(0.1));
                        }
                        egui::ComboBox::from_id_salt(("bedrock-camera-ease",index)).selected_text(&frame.ease).show_ui(ui,|ui| {
                            for (ease,label) in [("catmull_rom",t!("bedrock.java_curve")),("linear",t!("bedrock.linear")),("smooth",t!("bedrock.smooth")),("hold",t!("bedrock.hold"))] {
                                ui.selectable_value(&mut frame.ease,ease.into(),label);
                            }
                        });
                        if ui.small_button(t!("bedrock.remove")).clicked() {remove=Some(index);} ui.end_row();
                    }
                });
            });
            if let Some(index)=remove {e.frames.remove(index);}
            let path=bedrock::offline::CameraPath {format:1,keyframes:e.frames.clone()};
            if let Some(pick)=super::bedrock_camera::show(ui,p,&path,e.position,e.duration,e.keyframe_time) {
                e.keyframe_time=pick.time;e.pose=pick.pose;
            }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(active,egui::Button::new(t!("bedrock.apply_path"))).clicked() {
                    match path.validate() {Ok(())=>commands.push(serde_json::json!({"op":"camera_path","path":path})),Err(error)=>b.offline_message=error.to_string()}
                }
                if ui.button(t!("bedrock.open_path")).clicked() {self.tasks.bedrock_camera_open();}
                if ui.button(t!("bedrock.sort_times")).clicked() {e.frames.sort_by_key(|frame|frame.time_ms);}
                if ui.add_enabled(!e.frames.is_empty(),egui::Button::new(t!("bedrock.save_path"))).clicked() {self.tasks.bedrock_camera_save(path.clone());}
                if e.export_fps==0 {e.export_fps=25;}
                ui.add(egui::DragValue::new(&mut e.export_fps).range(1..=120).suffix(t!("bedrock.camera_samples_s")));
                if ui.add_enabled(active && !e.frames.is_empty(),egui::Button::new(t!("bedrock.export_edited_replay"))).clicked() {
                    match path.validate() {Ok(())=>self.tasks.bedrock_camera_export_file(path.clone(),e.export_fps),Err(error)=>b.offline_message=error.to_string()}
                }
            });
            ui.checkbox(&mut e.ffmpeg_only,t!("bedrock.use_ffmpeg")).on_hover_text(t!("bedrock.use_an_installed_ffmpeg_encoder_instead_of"));
            if ui.add_enabled(active && e.client_initialized && e.packet_ready && !b.busy,egui::Button::new(t!("bedrock.record_mp4"))).clicked() {
                b.busy=true;
                self.tasks.bedrock_record_video_with_encoder(b.edition,if b.capture_seconds==0 {10}else{b.capture_seconds},if b.capture_fps==0 {30}else{b.capture_fps},e.ffmpeg_only);
            }
            if e.video_fps==0 {e.video_fps=30;}
            if e.video_frames==0 {e.video_frames=300;}
            ui.add_enabled_ui(b.video_export.is_none(),|ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut e.video_range,t!("bedrock.select_export_range"))
                        .on_hover_text(t!("bedrock.render_from_the_selected_start_through_the")).changed()&&e.video_range {
                        e.rebuild_video=true;
                        e.video_start_ms=e.position.max(e.bootstrap_time);
                        e.video_end_ms=e.video_start_ms.saturating_add(10_000).min(e.duration);
                    }
                    if ui.add_enabled(e.frames.len()>=2,egui::Button::new(t!("bedrock.use_camera_path_range"))).clicked() {
                        e.video_range=true;e.rebuild_video=true;
                        e.video_start_ms=e.frames.iter().map(|key|key.time_ms).min().unwrap_or(e.position).max(e.bootstrap_time);
                        e.video_end_ms=e.frames.iter().map(|key|key.time_ms).max().unwrap_or(e.duration).min(e.duration);
                    }
                    if e.video_range {e.rebuild_video=true;}
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!e.video_range,egui::Checkbox::new(&mut e.rebuild_video,t!("bedrock.reload_world_before_export"))).changed()&&e.rebuild_video {
                        e.video_start_ms=e.frames.first().map_or(e.position,|key|key.time_ms).max(e.bootstrap_time);
                    }
                    if e.rebuild_video {
                        ui.label(t!("bedrock.start_at"));ui.add(egui::DragValue::new(&mut e.video_start_ms)
                            .range(e.bootstrap_time..=e.duration.max(e.bootstrap_time)).suffix(t!("bedrock.ms_suffix")));
                        if e.video_range&&e.video_start_ms<e.duration {
                            e.video_end_ms=e.video_end_ms.clamp(e.video_start_ms+1,e.duration);
                            ui.label(t!("bedrock.end_at"));ui.add(egui::DragValue::new(&mut e.video_end_ms)
                                .range(e.video_start_ms+1..=e.duration).suffix(t!("bedrock.ms_suffix")));
                        }
                        if !e.automatic_reconnect_available {ui.label(t!("bedrock.reload_export_needs_the_local_raknet"));}
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("bedrock.recorded_appearance"));
                    if ui.button(t!("bedrock.choose_appearance")).clicked() {self.tasks.bedrock_replay_appearance();}
                    if let Some(snapshot)=e.appearance_snapshot.as_ref() {
                        ui.label(snapshot.file_name().unwrap_or_default().to_string_lossy()).on_hover_text(snapshot.display().to_string());
                        if ui.button(t!("bedrock.clear_selection")).clicked() {e.appearance_snapshot=None;}
                    } else {ui.label(t!("bedrock.recording_default")).on_hover_text(t!("bedrock.uses_linked_recorded_appearance_when"));}
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("bedrock.video_fps"));ui.add(egui::DragValue::new(&mut e.video_fps).range(1..=60));
                    e.video_fps=e.video_fps.clamp(1,60);
                    let selected_range=if e.video_range {
                        if e.video_start_ms>=e.duration {
                            ui.colored_label(p.error,t!("bedrock.start_must_precede_the_recording_end"));None
                        } else {
                            match bedrock::export_range::ExportRange::new(e.video_start_ms,e.video_end_ms)
                                .and_then(|range|range.frames(e.video_fps).map(|frames|(range,frames))) {
                                Ok((range,frames))=>{e.video_frames=frames;Some(range)},
                                Err(error)=>{ui.colored_label(p.error,error.to_string());None},
                            }
                        }
                    } else {None};
                    e.video_frames=e.video_frames.clamp(1,e.video_fps*7200+1);
                    ui.label(t!("bedrock.frames"));ui.add_enabled(!e.video_range,egui::DragValue::new(&mut e.video_frames).range(1..=e.video_fps*7200+1));
                    e.video_frames=e.video_frames.clamp(1,e.video_fps*7200+1);
                    ui.label(tf!("bedrock.seconds_origin", seconds = format!("{:.2}",e.video_frames as f64/e.video_fps as f64), origin = if e.rebuild_video {t!("bedrock.from_selected_start")} else {t!("bedrock.from_playhead")}));
                    if ui.add_enabled(active && e.client_initialized && e.packet_ready && e.viewers==1 && e.initialized_viewers==1
                        && !b.busy && !e.frames.is_empty()&&(!e.rebuild_video||e.automatic_reconnect_available)
                        &&(!e.video_range||selected_range.is_some()),egui::Button::new(t!("bedrock.export_mp4_preview"))).clicked() {
                        match path.validate() {
                            Ok(())=>start_export=Some((path.clone(),e.video_fps,e.video_frames,e.appearance_snapshot.clone(),
                                e.rebuild_video.then_some(e.video_start_ms),e.ffmpeg_only,selected_range)),
                            Err(error)=>b.offline_message=error.to_string(),
                        }
                    }
                });
            });
            ui.label(RichText::new(t!("bedrock.mp4_export_renders_fixed_frames_from_the")).small().color(p.muted));
            ui.label(RichText::new(t!("bedrock.new_keyframes_use_javas_camera_curve_open")).small().color(p.muted));
        });
        let export_controls = if start_export.is_some() {
            commands
        } else {
            if !commands.is_empty() && self.bedrock.playback_clock.is_some() {
                let release_first =
                    matches!(clock_action.as_ref(), Some(PlaybackClockAction::Release));
                clock_action = Some(PlaybackClockAction::Commands {
                    commands,
                    release_first,
                });
            } else {
                for command in commands {
                    if let Some(session) = &mut self.bedrock.offline_session
                        && let Err(error) = session.control(&command)
                    {
                        self.bedrock.offline_message = error.to_string();
                    }
                }
            }
            Vec::new()
        };
        if start_export.is_none()
            && let Some(action) = clock_action
        {
            self.bedrock_playback_job(action);
        }
        if let Some((path, fps, frames, appearance_snapshot, start_ms, ffmpeg_only, range)) =
            start_export
        {
            let b = &mut self.bedrock;
            if b.video_export.is_none()
                && let Some(session) = b.offline_session.take()
            {
                let id = uuid::Uuid::new_v4();
                b.busy = true;
                let cancel = Arc::new(AtomicBool::new(false));
                let progress = Arc::new(Mutex::new(bedrock::export_job::ExportProgress {
                    stage: "choosing_files",
                    frames_queued: 0,
                    frames_total: frames,
                }));
                b.video_export = Some(ReplayVideoExport {
                    id,
                    address: session.address.clone(),
                    cancel: Arc::clone(&cancel),
                    progress: Arc::clone(&progress),
                    stop_after: false,
                });
                b.offline_message = t!("bedrock.preparing_replay_video_export").into();
                self.tasks.bedrock_export_video(
                    session,
                    ReplayVideoRequest {
                        id,
                        edition: b.edition,
                        path,
                        fps,
                        frames,
                        ffmpeg_only,
                        appearance_snapshot,
                        start_ms,
                        range,
                        cancel,
                        progress,
                        playback_clock: b.playback_clock.take(),
                        controls: export_controls,
                    },
                );
            }
        }
    }

    fn bedrock_native_client(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!("bedrock.arctic_client_native_preview"))
                    .strong()
                    .color(p.text),
            );
            ui.label(t!("bedrock.the_native_windows_adapter_reuses_arctics"));
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!("bedrock.in_world_skins_capes_cosmetics_emotes_game"))
                    .color(p.muted),
            );
            ui.add_space(8.0);
            ui.label(t!("bedrock.build_the_bundle_from_mod_versions_bedrock"));
            if self.bedrock.busy || self.bedrock.video_export.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("bedrock.working_with_the_native_client"));
                });
            } else if widgets::button(
                ui,
                p,
                Some(Icon::Folder),
                t!("bedrock.load_a_native_client_bundle"),
                true,
            )
            .clicked()
            {
                self.bedrock.busy = true;
                self.tasks.bedrock_native_client(self.bedrock.edition);
            }
            ui.add_space(12.0);
            ui.label(RichText::new(t!("bedrock.record_mp4_2")).strong());
            ui.label(t!("bedrock.capture_the_currently_rendered_game_and"));
            if self.bedrock.capture_seconds == 0 {
                self.bedrock.capture_seconds = 10;
            }
            if self.bedrock.capture_fps == 0 {
                self.bedrock.capture_fps = 30;
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.bedrock.capture_seconds)
                        .range(1..=7200)
                        .suffix(t!("bedrock.seconds_suffix")),
                );
                ui.add(
                    egui::DragValue::new(&mut self.bedrock.capture_fps)
                        .range(1..=60)
                        .suffix(t!("bedrock.fps_suffix")),
                );
                if ui
                    .add_enabled(
                        !self.bedrock.busy && self.bedrock.video_export.is_none(),
                        egui::Button::new(t!("bedrock.record_clip")),
                    )
                    .clicked()
                {
                    self.bedrock.busy = true;
                    self.tasks.bedrock_record_video(
                        self.bedrock.edition,
                        self.bedrock.capture_seconds,
                        self.bedrock.capture_fps,
                    );
                }
            });
        });
    }

    fn bedrock_rescan(&mut self, now: f64) {
        self.bedrock.scanned_at = now;
        let edition = self.bedrock.edition;
        if self.bedrock.scanning.insert(edition) {
            self.tasks.bedrock_scan(edition);
        }
        // Folders can change in game (a new world, an imported pack): read them again.
        if let Some(player) = self.bedrock_player() {
            self.tasks.bedrock_worlds(player);
        }
        if let Some(shared) = self.bedrock_shared() {
            self.tasks.bedrock_packs(shared);
        }
    }

    fn bedrock_forget_lists(&mut self) {
        self.bedrock.worlds = None;
        self.bedrock.packs = None;
        self.bedrock.servers = None;
        self.bedrock.status.clear();
        self.bedrock.pinged_at = f64::NEG_INFINITY;
    }

    /// The account folder shown (worlds, servers).
    fn bedrock_player(&self) -> Option<Player> {
        let scan = self.bedrock.scans.get(&self.bedrock.edition)?;
        scan.folders.player(self.bedrock.player.as_deref()).cloned()
    }

    /// Where packs go.
    fn bedrock_shared(&self) -> Option<Player> {
        let scan = self.bedrock.scans.get(&self.bedrock.edition)?;
        scan.folders.packs.clone()
    }

    fn bedrock_player_picker(&mut self, ui: &mut egui::Ui, scan: &BedrockScan) {
        let current = scan
            .folders
            .player(self.bedrock.player.as_deref())
            .map_or_else(String::new, Player::label);
        let mut picked = None;
        egui::ComboBox::from_id_salt("bedrock_player")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for player in &scan.folders.players {
                    if ui.selectable_label(false, player.label()).clicked() {
                        picked = Some(player.id.clone());
                    }
                }
            })
            .response
            .on_hover_text(t!("bedrock.each_xbox_account_that_played_here_has_its"));
        if let Some(id) = picked {
            self.bedrock.player = Some(id);
            self.bedrock_forget_lists();
            self.bedrock.scanned_at = f64::NEG_INFINITY;
        }
    }

    /// The game: its version and the Play button, or how to get it.
    fn bedrock_hero(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let edition = self.bedrock.edition;
        let scan = self.bedrock.scans.get(&edition).cloned();
        let installing = self
            .bedrock
            .installing
            .as_ref()
            .filter(|i| i.edition == edition)
            .map(|i| {
                let fraction = i
                    .progress
                    .as_ref()
                    .filter(|p| p.total > 0)
                    .map(|p| p.done as f32 / p.total as f32);
                (
                    tf!(
                        "bedrock.installing",
                        display = versions::display(&i.version, edition)
                    ),
                    fraction,
                )
            });
        let mut play = false;
        let mut cancel = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::row(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(12), p.surface_hover);
                icons::draw(ui.painter(), Icon::Cube, rect.shrink(14.0), p.accent);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(tf!("bedrock.minecraft", name = edition.name()))
                            .size(19.0)
                            .strong()
                            .color(p.text),
                    );
                    let line = match &scan {
                        None => t!("bedrock.looking_for_the_game").to_owned(),
                        Some(BedrockScan {
                            install: Some(install),
                            running,
                            ..
                        }) => {
                            let mut text = tf!(
                                "bedrock.version",
                                display = versions::display(&install.version, edition)
                            );
                            if !install.gdk {
                                text.push_str(t!("bedrock.old_uwp_build"));
                            }
                            if *running {
                                text.push_str(t!("bedrock.running_suffix"));
                            }
                            text
                        }
                        Some(_) => t!("bedrock.not_installed_on_this_computer").to_owned(),
                    };
                    ui.label(RichText::new(line).color(p.muted));
                });
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| match (&scan, &installing) {
                        (_, Some((label, fraction))) => {
                            if widgets::button(
                                ui,
                                p,
                                Some(Icon::Close),
                                t!("bedrock.cancel"),
                                false,
                            )
                            .clicked()
                            {
                                cancel = true;
                            }
                            widgets::play_button(
                                ui,
                                p,
                                PlayState::Progress {
                                    fraction: *fraction,
                                    label,
                                },
                                vec2(220.0, 50.0),
                            );
                        }
                        (
                            Some(BedrockScan {
                                install: Some(_),
                                running,
                                ..
                            }),
                            None,
                        ) => {
                            let state = if *running {
                                PlayState::Running
                            } else {
                                PlayState::Ready
                            };
                            if widgets::play_button(ui, p, state, vec2(180.0, 50.0)).clicked() {
                                play = true;
                            }
                        }
                        (Some(_), None) => {
                            if widgets::button(
                                ui,
                                p,
                                Some(Icon::External),
                                t!("bedrock.get_it_from_the_microsoft_store"),
                                true,
                            )
                            .clicked()
                                && let Err(e) = bedrock::open_store(edition)
                            {
                                self.toasts.push(
                                    Kind::Error,
                                    t!("bedrock.couldnt_open_the_store"),
                                    e.to_string(),
                                );
                            }
                        }
                        (None, None) => {
                            ui.spinner();
                        }
                    },
                );
            });
        });
        if play {
            // No third-party clients for now: Arctic is building its own.
            self.tasks.bedrock_launch(edition, None);
        }
        if cancel && let Some(i) = &self.bedrock.installing {
            i.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn bedrock_client_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let edition = self.bedrock.edition;
        let available = self.bedrock.installing.is_none()
            && self
                .bedrock
                .scans
                .get(&edition)
                .is_some_and(|scan| scan.install.as_ref().is_some_and(|install| install.gdk));
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!("bedrock.client_heading"))
                    .size(17.0)
                    .strong()
                    .color(p.text),
            );
            ui.label(RichText::new(t!("bedrock.client_development")).color(p.muted));
            ui.label(
                RichText::new(t!("bedrock.third_party_notice"))
                    .small()
                    .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for (client, label) in [
                    (bedrock::client::Client::Latite, t!("bedrock.launch_latite")),
                    (
                        bedrock::client::Client::Flarial,
                        t!("bedrock.launch_flarial"),
                    ),
                ] {
                    if ui
                        .add_enabled(available, egui::Button::new(label))
                        .clicked()
                    {
                        self.tasks.bedrock_launch(edition, Some(client));
                    }
                }
            });
        });
    }

    fn bedrock_versions(&mut self, ui: &mut egui::Ui, scan: &BedrockScan) {
        let p = self.palette();
        let edition = self.bedrock.edition;
        if self.bedrock.catalog.is_none() && !self.bedrock.catalog_loading {
            self.bedrock.catalog_loading = true;
            self.tasks.bedrock_catalog();
        }
        ui.label(
            RichText::new(t!("bedrock.switch_version_notice"))
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        if !scan.gaming_services {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::row(ui, |ui| {
                    ui.label(
                        RichText::new(t!("bedrock.gaming_services_isnt_installed_windows_needs"))
                            .color(p.warn),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(
                            ui,
                            p,
                            Some(Icon::External),
                            t!("bedrock.get_gaming_services"),
                            true,
                        )
                        .clicked()
                        {
                            let _ = open::that_detached(bedrock::GAMING_SERVICES_URL);
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
        let catalog = match &self.bedrock.catalog {
            None => {
                crate::widgets::row(ui, |ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("bedrock.loading_the_version_list")).color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                let e = e.clone();
                widgets::row(ui, |ui| {
                    ui.label(
                        RichText::new(tf!("bedrock.the_version_list_couldnt_be_loaded", e = e))
                            .color(p.muted),
                    );
                    if widgets::button(ui, p, None, t!("bedrock.try_again"), false).clicked() {
                        self.bedrock.catalog = None;
                    }
                });
                return;
            }
            Some(Ok(catalog)) => catalog.clone(),
        };
        let list = catalog.of(edition);
        let installed = scan.install.as_ref().map(|i| i.version.clone());
        let shown = if self.bedrock.all_versions {
            list.len()
        } else {
            VERSIONS_SHOWN.min(list.len())
        };
        let busy = self.bedrock.installing.is_some();
        let mut install = None;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, version) in list.iter().take(shown).enumerate() {
                let current = installed.as_deref() == Some(version.id.as_str());
                widgets::row(ui, |ui| {
                    ui.label(RichText::new(version.display()).strong().color(if current {
                        p.accent
                    } else {
                        p.text
                    }));
                    if version.display() != version.id {
                        ui.label(RichText::new(&version.id).small().color(p.muted));
                    }
                    if i == 0 {
                        ui.label(RichText::new(t!("bedrock.latest")).small().color(p.accent));
                    }
                    if version.is_legacy() {
                        ui.label(
                            RichText::new(t!("bedrock.older_kind"))
                                .small()
                                .color(p.muted),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if current {
                            ui.label(RichText::new(t!("bedrock.installed")).color(p.muted));
                        } else if !busy {
                            let newer = installed
                                .as_deref()
                                .is_some_and(|v| versions::compare(&version.id, v).is_gt());
                            let label = match (&installed, newer) {
                                (None, _) => t!("bedrock.install"),
                                (Some(_), true) => t!("bedrock.update"),
                                (Some(_), false) => t!("bedrock.switch"),
                            };
                            if widgets::button(ui, p, None, label, false).clicked() {
                                install = Some(version.clone());
                            }
                        }
                    });
                });
                ui.add_space(2.0);
            }
            if list.len() > shown
                && widgets::button(
                    ui,
                    p,
                    None,
                    &tf!("bedrock.show_all", count = list.len()),
                    false,
                )
                .clicked()
            {
                self.bedrock.all_versions = true;
            }
        });
        if let Some(version) = install {
            self.bedrock.confirm = Some(version);
        }
    }

    /// "Install this version?" before replacing the game.
    fn bedrock_confirm(&mut self, ctx: &egui::Context) {
        let Some(version) = self.bedrock.confirm.clone() else {
            return;
        };
        let p = self.palette();
        let installed = self
            .bedrock
            .scans
            .get(&version.edition)
            .and_then(|s| s.install.clone());
        let mut close = false;
        let mut go = false;
        let modal = Modal::new(Id::new("bedrock_confirm")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(
                RichText::new(tf!(
                    "bedrock.install_2",
                    name = version.edition.name(),
                    version = version.display()
                ))
                .size(18.0)
                .strong()
                .color(p.text),
            );
            ui.add_space(6.0);
            let replaces = match &installed {
                Some(i) => tf!(
                    "bedrock.it_replaces_the_game_closes_if_its_running",
                    display = versions::display(&i.version, version.edition)
                ),
                None => t!("bedrock.minecraft_must_be_owned_on_your_microsoft").to_owned(),
            };
            ui.label(
                RichText::new(tf!(
                    "bedrock.windows_downloads_it_from_microsoft_about_1",
                    replaces = replaces
                ))
                .color(p.muted),
            );
            ui.add_space(12.0);
            widgets::row(ui, |ui| {
                if widgets::button(ui, p, None, t!("bedrock.install"), true).clicked() {
                    go = true;
                }
                if widgets::button(ui, p, None, t!("bedrock.cancel"), false).clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if go {
            let cancel = Arc::new(AtomicBool::new(false));
            self.bedrock.installing = Some(Installing {
                edition: version.edition,
                version: version.id.clone(),
                progress: None,
                cancel: cancel.clone(),
            });
            self.tasks.bedrock_install(version, cancel);
            close = true;
        }
        if close {
            self.bedrock.confirm = None;
        }
    }

    fn bedrock_worlds(&mut self, ui: &mut egui::Ui, scan: &BedrockScan) {
        let p = self.palette();
        let Some(player) = self.bedrock_player() else {
            self.bedrock_empty(
                ui,
                t!("bedrock.no_worlds_yet"),
                if scan.install.is_some() {
                    t!("bedrock.play_once_and_the_game_makes_its_folders")
                } else {
                    t!("bedrock.install_the_game_first")
                },
            );
            return;
        };
        let list = match &self.bedrock.worlds {
            Some((dir, list)) if *dir == player.com_mojang => list.clone(),
            _ => {
                if self.bedrock.worlds.is_none() {
                    self.tasks.bedrock_worlds(player.clone());
                }
                Vec::new()
            }
        };
        widgets::row(ui, |ui| {
            ui.label(RichText::new(tf!("bedrock.world_count", count = list.len())).color(p.muted));
            if self.bedrock.busy {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Import),
                    t!("bedrock.import_mcworld"),
                    true,
                )
                .clicked()
                {
                    self.tasks.bedrock_import_worlds(player.clone());
                    self.bedrock.busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, t!("bedrock.open_the_worlds_folder"))
                    .clicked()
                {
                    let dir = player.worlds_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that_detached(&dir);
                }
            });
        });
        ui.label(
            RichText::new(t!("bedrock.drop_mcworld_mcpack_or_mcaddon_files_here_to"))
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        if list.is_empty() {
            self.bedrock_empty(
                ui,
                t!("bedrock.no_worlds_yet"),
                t!("bedrock.make_one_in_the_game_or_import_a_mcworld"),
            );
            return;
        }
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for world in &list {
                self.bedrock_world_row(ui, &player, world);
                ui.add_space(4.0);
            }
        });
    }

    fn bedrock_world_row(&mut self, ui: &mut egui::Ui, player: &Player, world: &World) {
        let p = self.palette();
        let backups = self.root.root().join("bedrock").join("backups");
        let mut changed = false;
        widgets::row(ui, |ui| {
            match world
                .icon
                .as_ref()
                .and_then(|icon| image_uri(ui.ctx(), "bedrock-world", icon))
            {
                Some(uri) => {
                    ui.add(
                        egui::Image::new(uri)
                            .fit_to_exact_size(vec2(ICON * 16.0 / 9.0, ICON))
                            .corner_radius(CornerRadius::same(8)),
                    );
                }
                None => placeholder(ui, p, Icon::Cube, vec2(ICON * 16.0 / 9.0, ICON)),
            }
            ui.vertical(|ui| {
                let renaming = self
                    .bedrock
                    .renaming
                    .as_mut()
                    .filter(|(path, _)| *path == world.path);
                match renaming {
                    Some((_, name)) => {
                        let field =
                            ui.add(widgets::text_field(name).desired_width(widgets::INPUT_WIDTH));
                        field.request_focus();
                        if field.lost_focus() {
                            let name = name.clone();
                            self.bedrock.renaming = None;
                            if ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && let Err(e) = worlds::rename(world, &name)
                            {
                                self.toasts.push(
                                    Kind::Error,
                                    t!("bedrock.couldnt_rename_it"),
                                    e.to_string(),
                                );
                            }
                            changed = true;
                        }
                    }
                    None => {
                        let label = ui
                            .add(
                                egui::Label::new(RichText::new(&world.name).strong().color(p.text))
                                    .sense(Sense::click()),
                            )
                            .on_hover_text(t!("bedrock.double_click_to_rename"));
                        if label.double_clicked() {
                            self.bedrock.renaming = Some((world.path.clone(), world.name.clone()));
                        }
                    }
                }
                let detail = format!("{} · {}", ago(world.last_played), format_bytes(world.size));
                ui.label(RichText::new(detail).small().color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, t!("bedrock.move_to_trash")).clicked() {
                    match worlds::trash(world, player) {
                        Ok(()) => self.toasts.push(
                            Kind::Info,
                            tf!("bedrock.moved_to_trash", name = world.name),
                            "com.mojang\\arctic-trash",
                        ),
                        Err(e) => self.toasts.push(
                            Kind::Error,
                            t!("bedrock.couldnt_move_the_world"),
                            e.to_string(),
                        ),
                    }
                    changed = true;
                }
                if widgets::icon_button(
                    ui,
                    p,
                    Icon::Copy,
                    t!("bedrock.back_up_a_mcworld_in_arctics_folder"),
                )
                .clicked()
                {
                    self.tasks
                        .bedrock_backup_world(world.clone(), backups.clone());
                }
                if widgets::icon_button(ui, p, Icon::Share, t!("bedrock.export_as_mcworld"))
                    .clicked()
                {
                    self.tasks.bedrock_export_world(world.clone());
                }
                if widgets::icon_button(ui, p, Icon::Folder, t!("bedrock.open_the_worlds_folder_2"))
                    .clicked()
                {
                    let _ = open::that_detached(&world.path);
                }
            });
        });
        if changed {
            self.tasks.bedrock_worlds(player.clone());
        }
    }

    fn bedrock_packs(&mut self, ui: &mut egui::Ui, scan: &BedrockScan) {
        let p = self.palette();
        let Some(shared) = scan.folders.packs.clone() else {
            self.bedrock_empty(
                ui,
                t!("bedrock.no_packs_yet"),
                t!("bedrock.play_once_and_the_game_makes_its_folders_2"),
            );
            return;
        };
        let list = match &self.bedrock.packs {
            Some((dir, list)) if *dir == shared.com_mojang => list.clone(),
            _ => {
                if self.bedrock.packs.is_none() {
                    self.tasks.bedrock_packs(shared.clone());
                }
                Vec::new()
            }
        };
        widgets::row(ui, |ui| {
            ui.label(RichText::new(tf!("bedrock.pack_count", count = list.len())).color(p.muted));
            if self.bedrock.busy {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Import),
                    t!("bedrock.import_mcpack_mcaddon"),
                    true,
                )
                .clicked()
                {
                    self.tasks.bedrock_import_packs(shared.clone());
                    self.bedrock.busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, t!("bedrock.open_the_packs_folder"))
                    .clicked()
                {
                    let _ = open::that_detached(&shared.com_mojang);
                }
            });
        });
        ui.label(
            RichText::new(t!("bedrock.packs_installed_here_are_available_to_every"))
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        if list.is_empty() {
            self.bedrock_empty(
                ui,
                t!("bedrock.no_packs_yet"),
                t!("bedrock.import_resource_packs_behavior_packs_skin"),
            );
            return;
        }
        let mut changed = false;
        for kind in packs::PackKind::ALL {
            let of_kind: Vec<&Pack> = list.iter().filter(|p| p.kind == kind).collect();
            if of_kind.is_empty() {
                continue;
            }
            ui.label(
                RichText::new(match kind {
                    packs::PackKind::Resource => t!("bedrock.resource_packs_heading"),
                    packs::PackKind::Behavior => t!("bedrock.behavior_packs_heading"),
                    packs::PackKind::Skin => t!("bedrock.skin_packs_heading"),
                })
                .small()
                .color(p.muted),
            );
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for pack in of_kind {
                    widgets::row(ui, |ui| {
                        match pack
                            .icon
                            .as_ref()
                            .and_then(|icon| image_uri(ui.ctx(), "bedrock-pack", icon))
                        {
                            Some(uri) => {
                                ui.add(
                                    egui::Image::new(uri)
                                        .fit_to_exact_size(vec2(ICON, ICON))
                                        .corner_radius(CornerRadius::same(8)),
                                );
                            }
                            None => placeholder(ui, p, Icon::Layers, vec2(ICON, ICON)),
                        }
                        ui.vertical(|ui| {
                            ui.set_max_width((ui.available_width() - 120.0).max(120.0));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&pack.name).strong().color(p.text));
                                if !pack.version.is_empty() {
                                    ui.label(
                                        RichText::new(tf!(
                                            "bedrock.pack_version",
                                            version = pack.version
                                        ))
                                        .small()
                                        .color(p.muted),
                                    );
                                }
                                if pack.development {
                                    ui.label(
                                        RichText::new(t!("bedrock.development"))
                                            .small()
                                            .color(p.accent),
                                    );
                                }
                            });
                            let detail = if pack.description.is_empty() {
                                format_bytes(pack.size)
                            } else {
                                format!("{} · {}", pack.description, format_bytes(pack.size))
                            };
                            ui.add(
                                egui::Label::new(RichText::new(detail).small().color(p.muted))
                                    .truncate(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_button(ui, p, Icon::Trash, t!("bedrock.move_to_trash"))
                                .clicked()
                            {
                                match packs::trash(pack, &shared) {
                                    Ok(()) => self.toasts.push(
                                        Kind::Info,
                                        tf!("bedrock.moved_to_trash", name = pack.name),
                                        "com.mojang\\arctic-trash",
                                    ),
                                    Err(e) => self.toasts.push(
                                        Kind::Error,
                                        t!("bedrock.couldnt_move_the_pack"),
                                        e.to_string(),
                                    ),
                                }
                                changed = true;
                            }
                            if widgets::icon_button(
                                ui,
                                p,
                                Icon::Folder,
                                t!("bedrock.open_the_packs_folder_2"),
                            )
                            .clicked()
                            {
                                let _ = open::that_detached(&pack.path);
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            });
            ui.add_space(8.0);
        }
        if changed {
            self.tasks.bedrock_packs(shared);
        }
    }

    fn bedrock_servers(&mut self, ui: &mut egui::Ui, scan: &BedrockScan, now: f64) {
        let p = self.palette();
        let Some(player) = self.bedrock_player() else {
            let _ = scan;
            self.bedrock_empty(
                ui,
                t!("bedrock.no_server_list_yet"),
                t!("bedrock.play_once_and_the_game_makes_its_folders_3"),
            );
            return;
        };
        let current = matches!(&self.bedrock.servers, Some((dir, _)) if *dir == player.com_mojang);
        if !current || now - self.bedrock.pinged_at >= PING_EVERY {
            self.bedrock_refresh_servers(&player, now);
        }
        let list = match &self.bedrock.servers {
            Some((dir, list)) if *dir == player.com_mojang => list.clone(),
            _ => Vec::new(),
        };
        // Add a server.
        let mut add = false;
        widgets::row(ui, |ui| {
            ui.add(
                widgets::text_field(&mut self.bedrock.server_name)
                    .hint_text(t!("bedrock.name"))
                    .desired_width(widgets::INPUT_WIDTH),
            );
            let field = ui.add(
                widgets::text_field(&mut self.bedrock.server_address)
                    .hint_text("play.example.net:19132")
                    .desired_width(widgets::INPUT_WIDTH),
            );
            add = widgets::button(ui, p, Some(Icon::Plus), t!("bedrock.add_server"), true)
                .clicked()
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let waiting = self
                    .bedrock
                    .status
                    .values()
                    .any(|s| matches!(s, Ping::Waiting));
                if waiting {
                    ui.spinner();
                } else if widgets::button(ui, p, None, t!("bedrock.refresh"), false).clicked() {
                    self.bedrock_refresh_servers(&player, now);
                }
            });
        });
        if add {
            let address = self.bedrock.server_address.trim().to_owned();
            let name = self.bedrock.server_name.trim().to_owned();
            self.bedrock_add_server(&player, &name, &address, now);
        }
        ui.add_space(8.0);
        if list.is_empty() {
            ui.label(
                RichText::new(t!("bedrock.servers_you_add_here_or_in_the_games_servers"))
                    .small()
                    .color(p.muted),
            );
        } else {
            let mut remove = None;
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for server in &list {
                    let address = server.address();
                    if self.bedrock_server_row(ui, &server.name, &address, true) {
                        remove = Some(address);
                    }
                    ui.add_space(6.0);
                }
            });
            if let Some(address) = remove {
                match servers::remove(&player, &address) {
                    Ok(_) => self.bedrock_refresh_servers(&player, now),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        t!("bedrock.couldnt_remove_it"),
                        e.to_string(),
                    ),
                }
            }
        }
        // Featured servers not on the list yet.
        let featured: Vec<(&str, &str)> = servers::FEATURED
            .iter()
            .copied()
            .filter(|(_, address)| !list.iter().any(|s| s.host.eq_ignore_ascii_case(address)))
            .collect();
        if featured.is_empty() {
            return;
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new(t!("bedrock.popular_servers"))
                .small()
                .color(p.muted),
        );
        ui.add_space(4.0);
        let mut picked = None;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (name, address) in featured {
                if !self.bedrock.status.contains_key(address) {
                    self.bedrock
                        .status
                        .insert(address.to_owned(), Ping::Waiting);
                    self.tasks.bedrock_ping(address.to_owned());
                }
                if self.bedrock_server_row(ui, name, address, false) {
                    picked = Some((name.to_owned(), address.to_owned()));
                }
                ui.add_space(6.0);
            }
        });
        if let Some((name, address)) = picked {
            self.bedrock_add_server(&player, &name, &address, now);
        }
    }

    fn bedrock_add_server(&mut self, player: &Player, name: &str, address: &str, now: f64) {
        if address.is_empty() {
            return;
        }
        match servers::add(player, name, address) {
            Ok(true) => {
                self.toasts.push(
                    Kind::Success,
                    tf!(
                        "bedrock.added",
                        value = if name.is_empty() { address } else { name }
                    ),
                    t!("bedrock.server_saved_hint"),
                );
                self.bedrock.server_name.clear();
                self.bedrock.server_address.clear();
                self.bedrock_refresh_servers(player, now);
            }
            Ok(false) => self.toasts.push(
                Kind::Info,
                tf!("bedrock.is_already_on_your_list", address = address),
                "",
            ),
            Err(e) => self
                .toasts
                .push(Kind::Error, t!("bedrock.couldnt_add_it"), e.to_string()),
        }
    }

    fn bedrock_refresh_servers(&mut self, player: &Player, now: f64) {
        self.bedrock.pinged_at = now;
        let stale = !matches!(&self.bedrock.servers, Some((dir, _)) if *dir == player.com_mojang);
        if stale {
            self.bedrock.servers = Some((player.com_mojang.clone(), Vec::new()));
        }
        for ping in self.bedrock.status.values_mut() {
            *ping = Ping::Waiting;
        }
        self.tasks.bedrock_servers(player.clone());
    }

    /// One server: name, MOTD, players and ping. `listed`: it's on the
    /// player's list (removable), else it can be added. Returns whether its
    /// button was pressed.
    fn bedrock_server_row(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        address: &str,
        listed: bool,
    ) -> bool {
        let p = self.palette();
        let mut pressed = false;
        widgets::row(ui, |ui| {
            placeholder(ui, p, Icon::Friends, vec2(ICON, ICON));
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 220.0).max(120.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).strong().color(p.text));
                    if name != address {
                        ui.label(RichText::new(address).small().color(p.muted));
                    }
                });
                match self.bedrock.status.get(address) {
                    Some(Ping::Up(s)) => {
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        if !s.status.motd.is_empty() {
                            ui.add(
                                egui::Label::new(widgets::mc_text(
                                    &s.status.motd_formatted,
                                    p.muted,
                                    font,
                                ))
                                .truncate(),
                            );
                        }
                        let mut detail = tf!(
                            "bedrock.online_count",
                            online = s.status.online,
                            max = s.status.max
                        );
                        if !s.status.version.is_empty() {
                            detail.push_str(&format!(" · {}", s.status.version));
                        }
                        if !s.game_mode.is_empty() {
                            detail.push_str(&format!(" · {}", s.game_mode));
                        }
                        ui.label(RichText::new(detail).small().color(p.text));
                    }
                    Some(Ping::Down(reason)) => {
                        ui.label(RichText::new(reason).small().color(p.muted));
                    }
                    _ => {}
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                pressed = if listed {
                    widgets::icon_button(ui, p, Icon::Trash, t!("bedrock.remove_from_the_list"))
                        .clicked()
                } else {
                    widgets::button(ui, p, Some(Icon::Plus), t!("bedrock.add"), false).clicked()
                };
                ui.add_space(6.0);
                match self.bedrock.status.get(address) {
                    Some(Ping::Up(s)) => {
                        let color = match s.status.ping_ms {
                            0..=100 => p.accent,
                            101..=250 => p.warn,
                            _ => p.error,
                        };
                        ui.label(
                            RichText::new(tf!("bedrock.ping_ms", ping = s.status.ping_ms))
                                .color(color),
                        );
                    }
                    Some(Ping::Down(_)) => {
                        ui.label(
                            RichText::new(t!("bedrock.offline_status"))
                                .small()
                                .color(p.error),
                        );
                    }
                    _ => {
                        ui.spinner();
                    }
                }
            });
        });
        pressed
    }

    fn bedrock_empty(&self, ui: &mut egui::Ui, title: &str, text: &str) {
        let p = self.palette();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new(title).size(17.0).strong().color(p.text));
                ui.label(RichText::new(text).color(p.muted));
                ui.add_space(10.0);
            });
        });
    }

    /// Files dropped on the window while this tab is open: worlds, packs, add-ons.
    fn bedrock_dropped_files(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if files.is_empty() {
            return;
        }
        let (Some(player), Some(shared)) = (self.bedrock_player(), self.bedrock_shared()) else {
            self.toasts.push(
                Kind::Info,
                t!("bedrock.play_bedrock_once_first"),
                t!("bedrock.the_game_makes_its_folders_the_first_time_it"),
            );
            return;
        };
        self.bedrock.busy = true;
        self.tasks.bedrock_import_files(files, player, shared);
    }

    pub(crate) fn on_bedrock_event(&mut self, event: BedrockEvent) {
        let b = &mut self.bedrock;
        match event {
            BedrockEvent::Scanned(edition, scan) => {
                b.scanning.remove(&edition);
                b.scans.insert(edition, scan);
            }
            BedrockEvent::Catalog(result) => {
                b.catalog_loading = false;
                b.catalog = Some(result);
            }
            BedrockEvent::InstallProgress(progress) => {
                if let Some(i) = &mut b.installing {
                    i.progress = Some(progress);
                }
            }
            BedrockEvent::Installed(edition, version, result) => {
                b.installing = None;
                let name = format!(
                    "{} {}",
                    edition.name(),
                    versions::display(&version, edition)
                );
                match result {
                    Ok(()) => self.toasts.push(
                        Kind::Success,
                        tf!("bedrock.installed_2", name = name),
                        t!("bedrock.press_play_hint"),
                    ),
                    Err(e) if e == "cancelled" => self.toasts.push(
                        Kind::Info,
                        tf!("bedrock.stopped_installing", name = name),
                        "",
                    ),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        tf!("bedrock.couldnt_install", name = name),
                        e,
                    ),
                }
            }
            BedrockEvent::Worlds(dir, list) => b.worlds = Some((dir, list)),
            BedrockEvent::Packs(dir, list) => b.packs = Some((dir, list)),
            BedrockEvent::Servers(dir, list) => {
                if b.servers.as_ref().is_none_or(|(d, _)| *d == dir) {
                    for server in &list {
                        b.status.insert(server.address(), Ping::Waiting);
                    }
                    b.servers = Some((dir, list));
                }
            }
            BedrockEvent::ServerStatus(address, result) => {
                b.status.insert(
                    address,
                    match result {
                        Ok(s) => Ping::Up(s),
                        Err(e) => Ping::Down(e),
                    },
                );
            }
            BedrockEvent::Done(result) => {
                b.busy = false;
                match result {
                    Ok(text) => self.toasts.push(Kind::Success, text, ""),
                    Err(e) => self.toasts.push(Kind::Error, t!("bedrock.bedrock"), e),
                }
            }
            BedrockEvent::PlaybackClockFinished(returned) => {
                if !b
                    .playback_work
                    .as_ref()
                    .is_some_and(|work| work.id == returned.id && work.edition == returned.edition)
                    || b.offline_session.is_some()
                    || b.playback_clock.is_some()
                {
                    return;
                }
                let (_, _, session, lease, result) = returned.take();
                let work = b.playback_work.take().expect("matched playback clock job");
                b.playback_service_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(1));
                b.offline_message = match result {
                    Ok(_) => {
                        if lease.as_ref().is_some_and(PlaybackClockLease::active) {
                            t!("bedrock.game_clock_coordination_acknowledged_replay").into()
                        } else {
                            t!("bedrock.game_clock_coordination_released_or_canceled").into()
                        }
                    }
                    Err(error) => {
                        self.toasts.push(
                            Kind::Error,
                            t!("bedrock.replay_game_clock"),
                            error.clone(),
                        );
                        format!("{error} · Replay connection retained; playback may remain paused.") // i18n-ignore
                    }
                };
                b.offline_session = session;
                b.playback_clock = lease;
                if work.stop_after
                    && let Some(session) = b.offline_session.take()
                {
                    b.offline_stopping = true;
                    self.tasks
                        .bedrock_stop_playback(session, b.playback_clock.take());
                }
            }
            BedrockEvent::NativeRecordingFinished(returned) => {
                let (id, edition, session, lease, result) = returned.take();
                if !b
                    .recording_work
                    .as_ref()
                    .is_some_and(|job| job.id == id && job.edition == edition)
                    || b.offline_session.is_some()
                    || b.paired_recording.is_some()
                {
                    self.tasks
                        .bedrock_release_recording(edition, session, lease);
                    return;
                }
                let job = b
                    .recording_work
                    .take()
                    .expect("matched native recording job");
                let succeeded = result.is_ok();
                b.offline_message = match result {
                    Ok(Some(outcome)) => {
                        b.recording_finalized = true;
                        self.toasts.push(
                            Kind::Success,
                            t!("bedrock.replay_and_appearance_saved"),
                            outcome.source.display().to_string(),
                        );
                        tf!(
                            "bedrock.replay_saved_appearance_captured_from_to_ms",
                            source = outcome.source.display(),
                            first_time_ms = outcome.first_time_ms,
                            end_time_ms = outcome.end_time_ms
                        )
                    }
                    Ok(None) => match job.action {
                        RecordingAction::Begin if lease.is_some() => {
                            t!("bedrock.arctic_appearance_capture_started").into()
                        }
                        RecordingAction::Begin => t!("bedrock.appearance_capture_canceled").into(),
                        RecordingAction::Cancel => {
                            t!("bedrock.owned_appearance_capture_released").into()
                        }
                        RecordingAction::Finish => t!("bedrock.paired_recording_finished").into(),
                    },
                    Err(error) => {
                        b.close_after_recording = false;
                        self.toasts.push(
                            Kind::Error,
                            t!("bedrock.paired_recording"),
                            error.clone(),
                        );
                        tf!(
                            "bedrock.retry_saving_or_release_the_retained",
                            error = error
                        )
                    }
                };
                b.paired_recording = lease.map(|lease| PairedRecording { edition, lease });
                b.offline_session = session;
                if succeeded && job.stop_after {
                    if b.paired_recording.is_some() {
                        // Stop was requested while the begin dialog/marker was
                        // pending. Finalize that returned owner before stopping.
                        self.bedrock_recording_job(RecordingAction::Finish, true);
                    } else if let Some(session) = b.offline_session.take() {
                        b.offline_stopping = true;
                        self.tasks.bedrock_stop_offline(session);
                    }
                }
            }
            BedrockEvent::NativeExportFinished {
                id,
                mut session,
                result,
            } => {
                if !b
                    .video_export
                    .as_ref()
                    .is_some_and(|export| export.id == id)
                {
                    self.tasks.run(move |_| {
                        let _ = session.stop();
                    });
                    return;
                }
                let export = b.video_export.take().expect("matched replay export job");
                b.busy = false;
                let message = match result {
                    Ok(Some(outcome)) => {
                        let message = tf!(
                            "bedrock.replay_video_saved",
                            output = outcome.output.display()
                        );
                        self.toasts.push(
                            Kind::Success,
                            t!("bedrock.replay_video_saved_2"),
                            outcome.output.display().to_string(),
                        );
                        message
                    }
                    Ok(None) => t!("bedrock.replay_export_canceled").into(),
                    Err(error) => {
                        self.toasts.push(
                            Kind::Error,
                            t!("bedrock.replay_video_export"),
                            error.clone(),
                        );
                        error
                    }
                };
                b.offline_message = message;
                if b.offline_session.is_some() {
                    // Preserve any newer session; release this returned owner
                    // on a worker rather than blocking the UI in Child::wait.
                    self.tasks.run(move |_| {
                        let _ = session.stop();
                    });
                } else {
                    b.replay_editor.client_initialized = false;
                    b.replay_editor.packet_ready = false;
                    b.replay_editor.viewers = 0;
                    b.replay_editor.initialized_viewers = 0;
                    if export.stop_after {
                        b.offline_stopping = true;
                        b.offline_message = t!("bedrock.stopping_local_session").into();
                        self.tasks.bedrock_stop_offline(session);
                    } else {
                        b.offline_session = Some(session);
                    }
                }
            }
            BedrockEvent::OfflineStarted(result) => {
                b.offline_starting = false;
                match result {
                    Ok(session) => {
                        if b.video_export.is_some()
                            || b.offline_session.is_some()
                            || b.recording_work.is_some()
                            || b.paired_recording.is_some()
                            || b.playback_work.is_some()
                        {
                            self.tasks.run(move |_| {
                                let mut session = session;
                                let _ = session.stop();
                            });
                            return;
                        }
                        b.replay_editor.client_initialized = false;
                        b.replay_editor.packet_ready = false;
                        b.replay_editor.bootstrap_time = 0;
                        b.replay_editor.viewers = 0;
                        b.replay_editor.initialized_viewers = 0;
                        b.replay_editor.resource_packs = None;
                        b.replay_editor.generation = 0;
                        b.replay_editor.world_reset = None;
                        b.replay_editor.automatic_reconnect_available = false;
                        b.recording_finalized = false;
                        b.pending_replay_controls.clear();
                        b.offline_message =
                            t!("bedrock.waiting_for_your_game_to_join_the_local").into();
                        b.offline_session = Some(session);
                    }
                    Err(error) => b.offline_message = error,
                }
            }
            BedrockEvent::OfflineStopped(result) => {
                b.offline_stopping = false;
                if result.is_err() {
                    b.close_after_recording = false;
                }
                b.offline_message = result.unwrap_or_else(|error| error);
            }
            BedrockEvent::AppearancePicked { cape, path } => {
                if let Some(path) = path {
                    if cape {
                        b.offline_cape = Some(path);
                    } else {
                        b.offline_skin = Some(path);
                    }
                }
            }
            BedrockEvent::ReplayPicked { recording, path } => {
                if let Some(path) = path {
                    if recording {
                        b.offline_record_to = Some(path);
                    } else {
                        b.replay_file = Some(path);
                    }
                }
            }
            BedrockEvent::ReplayAppearancePicked(file) => {
                if let Some(file) = file {
                    if file
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().ends_with(".arctic-looks.zip"))
                    {
                        b.replay_editor.appearance_snapshot = Some(file);
                    } else {
                        b.offline_message =
                            t!("bedrock.choose_an_arctic_looks_zip_appearance").into();
                    }
                }
            }
            BedrockEvent::CameraLoaded(result) => match result {
                Ok(path) => {
                    b.replay_editor.frames = path.keyframes;
                    b.offline_message = t!("bedrock.camera_path_opened_apply_it_to_preview").into();
                }
                Err(error) => b.offline_message = error,
            },
            BedrockEvent::CameraExportPicked { file, path, fps } => {
                if let Some(file) = file
                    && b.replay_mode
                    && (b.offline_session.is_some() || b.playback_work.is_some())
                    && b.video_export.is_none()
                {
                    if b.pending_replay_controls.len() < 32 {
                        b.pending_replay_controls.push((
                            b.replay_editor.generation,
                            serde_json::json!({"op":"export","file":file,"path":path,"fps":fps}),
                        ));
                        b.offline_message = t!("bedrock.edited_replay_export_queued").into();
                    } else {
                        b.offline_message =
                            t!("bedrock.replay_control_queue_is_full_retry_the").into();
                    }
                }
            }
        }
    }
}

/// "3 days ago"
fn ago(time: Option<std::time::SystemTime>) -> String {
    let Some(secs) = time.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()) else {
        return t!("bedrock.never_played").into();
    };
    let (n, unit) = match secs {
        0..60 => return t!("bedrock.just_now").into(),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        86_400..2_592_000 => (secs / 86_400, "day"),
        _ => (secs / 2_592_000, "month"),
    };
    let key = match unit {
        "minute" => "bedrock.ago_minute",
        "hour" => "bedrock.ago_hour",
        "day" => "bedrock.ago_day",
        _ => "bedrock.ago_month",
    };
    crate::i18n::plural(key, n as i64, &[])
}

fn placeholder(ui: &mut egui::Ui, p: &crate::theme::Palette, icon: Icon, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), p.surface_hover);
    icons::draw(
        ui.painter(),
        icon,
        egui::Rect::from_center_size(rect.center(), vec2(20.0, 20.0)),
        p.muted,
    );
}

/// An image file as an egui URI (read once per path and change time).
fn image_uri(ctx: &egui::Context, kind: &str, path: &std::path::Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    modified.hash(&mut hasher);
    let ext = path
        .extension()
        .map_or("png".into(), |e| e.to_string_lossy().to_ascii_lowercase());
    let uri = format!("bytes://{kind}/{:016x}.{ext}", hasher.finish());
    let known = ctx.data(|d| d.get_temp::<bool>(Id::new(&uri)).is_some());
    if !known {
        let bytes = std::fs::read(path).ok()?;
        ctx.include_bytes(uri.clone(), bytes);
        ctx.data_mut(|d| d.insert_temp(Id::new(&uri), true));
    }
    Some(uri)
}
