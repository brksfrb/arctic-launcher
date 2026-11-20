//! Discover's per-instance pack matches. Only the post-install name check runs on the UI thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use arctic_core::mods::ProjectType;
use arctic_core::mods::discovery::VersionInfo;
use arctic_core::mods::pack_index::{self, PackFile, PackMatches, PackScan};
use eframe::egui;

use crate::mod_tasks::POLL_INTERVAL;

#[derive(Default)]
pub(super) struct HubInstalled {
    packs: HashMap<(PathBuf, ProjectType), PackStatus>,
    known: HashMap<ProjectType, Arc<Vec<PackFile>>>,
}

impl HubInstalled {
    pub fn remember_versions(
        &mut self,
        kind: ProjectType,
        project: &str,
        versions: &[VersionInfo],
    ) {
        if !matches!(
            kind,
            ProjectType::ResourcePack | ProjectType::Shader | ProjectType::DataPack
        ) {
            return;
        }
        // Modrinth search hits have no file identities. Reuse the versions opened by the player.
        let known = Arc::make_mut(self.known.entry(kind).or_default());
        known.retain(|file| file.project_id != project);
        known.extend(versions.iter().filter_map(|version| {
            Some(PackFile {
                project_id: project.to_owned(),
                file_name: version.file_name.clone()?,
                sha1: version.file_sha1.clone(),
            })
        }));
        for ((_, scanned_kind), status) in &mut self.packs {
            if *scanned_kind == kind {
                status.invalidate();
            }
        }
    }

    pub fn refresh(&mut self, kind: ProjectType, game_dir: &Path) {
        let status = self
            .packs
            .entry((game_dir.to_path_buf(), kind))
            .or_default();
        let known = self
            .known
            .get(&kind)
            .map_or(&[][..], |files| files.as_slice());
        status.installed = Arc::new(pack_index::matches_by_name(kind, game_dir, known));
        status.invalidate();
    }

    pub fn known_files(&self, kind: ProjectType) -> Arc<Vec<PackFile>> {
        self.known.get(&kind).cloned().unwrap_or_default()
    }

    pub fn packs(
        &mut self,
        ctx: &egui::Context,
        kind: ProjectType,
        game_dir: PathBuf,
    ) -> Arc<PackMatches> {
        let known = self.known.get(&kind).cloned().unwrap_or_default();
        let status = self.packs.entry((game_dir.clone(), kind)).or_default();
        status.poll(ctx, kind, game_dir, known);
        Arc::clone(&status.installed)
    }
}

type PackScanResult = (PackScan, PackMatches);

#[derive(Default)]
struct PackStatus {
    scan: PackScan,
    pending: Option<Receiver<PackScanResult>>,
    dirty: bool,
    checked_at: Option<Instant>,
    installed: Arc<PackMatches>,
}

impl PackStatus {
    fn invalidate(&mut self) {
        self.dirty = self.pending.is_some();
        self.checked_at = None;
    }

    fn poll(
        &mut self,
        ctx: &egui::Context,
        kind: ProjectType,
        game_dir: PathBuf,
        known: Arc<Vec<PackFile>>,
    ) {
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok((scan, installed)) => {
                    self.scan = scan;
                    if !self.dirty {
                        self.installed = Arc::new(installed);
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {}
            }
            self.pending = None;
            if !std::mem::take(&mut self.dirty) {
                self.checked_at = Some(Instant::now());
            }
        }
        if let Some(remaining) = self
            .checked_at
            .and_then(|at| POLL_INTERVAL.checked_sub(at.elapsed()))
        {
            ctx.request_repaint_after(remaining);
            return;
        }

        let (send, pending) = mpsc::channel();
        self.pending = Some(pending);
        let mut scan = std::mem::take(&mut self.scan);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let installed = scan.matches(kind, &game_dir, &known);
            let _ = send.send((scan, installed));
            ctx.request_repaint();
        });
    }
}
