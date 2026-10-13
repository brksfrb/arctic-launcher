//! Choosing folders: Settings → Storage (where the launcher keeps everything) and an instance's
//! game folder (its worlds, mods and packs).

use std::path::{Path, PathBuf};

use arctic_core::instances::{FolderChange, Instance};
use arctic_core::storage::RootSource;
use arctic_core::storage::location::{self, Target};
use eframe::egui::{self, Id, Modal, RichText};

use super::dialogs::dialog_frame;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::motion::format_bytes;
use crate::storage_tasks::{Checked, StorageEvent};
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

const DIALOG_WIDTH: f32 = 460.0;
/// Made inside a picked folder that already holds other things.
const OWN_FOLDER: &str = "Arctic Launcher";

#[derive(Default)]
pub struct StorageUi {
    /// Where the data folder is and why (read once).
    source: Option<RootSource>,
    dialog: Option<Dialog>,
    folder: Option<FolderDialog>,
}

enum Dialog {
    Check(Checked),
    Moving { to: PathBuf, done: u64, total: u64 },
    Failed(String),
}

/// Switching an instance's game folder.
struct FolderDialog {
    id: String,
    /// `None`: back inside the instance's own folder.
    folder: Option<PathBuf>,
    empty: bool,
    busy: bool,
}

fn path_text(p: &Palette, path: &Path) -> RichText {
    RichText::new(path.display().to_string())
        .monospace()
        .color(p.text)
}

fn note(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).small().color(p.muted));
}

impl ArcticApp {
    fn root_source(&mut self) -> RootSource {
        let dirs_root = self.dirs.root().to_path_buf();
        // Screenshots run on a throwaway folder (ARCTIC_DATA_DIR): show it as the usual one,
        // and with `dialog` also the move dialog for a folder next to it.
        if crate::devshot::enabled()
            && let Ok(mode) = std::env::var("ARCTIC_DEVSHOT_STORAGE")
        {
            if mode == "dialog" && self.storage.dialog.is_none() && self.storage.source.is_none() {
                self.tasks
                    .storage_check(dirs_root.clone(), dirs_root.with_file_name("Arctic on D"));
            }
            return self
                .storage
                .source
                .get_or_insert(RootSource::Default(dirs_root))
                .clone();
        }
        self.storage
            .source
            .get_or_insert_with(|| {
                arctic_core::storage::root_source().unwrap_or(RootSource::Default(dirs_root))
            })
            .clone()
    }

    /// Settings → Storage.
    pub(crate) fn storage_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let source = self.root_source();
        note(
            ui,
            p,
            "Game files, Java, instances (worlds, mods, packs), accounts and settings are kept in:",
        );
        ui.label(path_text(p, self.dirs.root()));
        ui.add_space(4.0);
        widgets::row(ui, |ui| {
            if widgets::button(ui, p, Some(Icon::Folder), "Open folder", false).clicked()
                && let Err(e) = open::that_detached(self.dirs.root())
            {
                self.toasts
                    .push(Kind::Error, "Could not open folder", e.to_string());
            }
            let movable = source.movable();
            let change = ui.add_enabled_ui(movable, |ui| {
                widgets::button(ui, p, Some(Icon::Import), "Change folder…", false)
            });
            if change.inner.clicked() {
                self.tasks.storage_pick(self.dirs.root().to_path_buf());
            }
        });
        match &source {
            RootSource::Env(_) => note(ui, p, "Set by the ARCTIC_DATA_DIR environment variable."),
            RootSource::Portable(_) => note(
                ui,
                p,
                "Portable mode: portable.txt is next to the launcher, so everything stays in the data folder beside it.",
            ),
            RootSource::ChosenMissing { chosen, .. } => {
                ui.label(
                    RichText::new(format!(
                        "{} isn't available (an unplugged drive?). Arctic uses its usual folder until it's back.",
                        chosen.display()
                    ))
                    .small()
                    .color(p.error),
                );
            }
            RootSource::Chosen(_) | RootSource::Default(_) => note(
                ui,
                p,
                "Another drive or a bigger disk? Change the folder and everything moves there.",
            ),
        }
    }

    /// First start: where the files go (optional; the usual folder is fine for most).
    pub(crate) fn storage_choice(&mut self, ui: &mut egui::Ui, p: &Palette) {
        if !self.root_source().movable() {
            return;
        }
        ui.add_space(18.0);
        ui.label(RichText::new("Where Arctic keeps its files").color(p.text));
        note(
            ui,
            p,
            "Game files, Java and your worlds. Another drive has more room? Pick a folder there.",
        );
        ui.label(path_text(p, self.dirs.root()));
        ui.add_space(4.0);
        if widgets::button(ui, p, Some(Icon::Import), "Change folder…", false).clicked() {
            self.tasks.storage_pick(self.dirs.root().to_path_buf());
        }
    }

    pub(crate) fn on_storage_event(&mut self, event: StorageEvent, ctx: &egui::Context) {
        match event {
            StorageEvent::Checked(checked) => self.storage.dialog = Some(Dialog::Check(checked)),
            StorageEvent::Progress(done, total) => {
                if let Some(Dialog::Moving {
                    done: d, total: t, ..
                }) = &mut self.storage.dialog
                {
                    (*d, *t) = (done, total);
                }
            }
            StorageEvent::Moved(Ok(())) => {
                self.storage.dialog = None;
                self.restart_after_update(ctx);
            }
            StorageEvent::Moved(Err(e)) => self.storage.dialog = Some(Dialog::Failed(e)),
            StorageEvent::InstancePicked(id, folder, empty) => {
                self.storage.folder = Some(FolderDialog {
                    id,
                    folder: Some(folder),
                    empty,
                    busy: false,
                });
            }
            StorageEvent::InstanceMoved(id, result) => {
                self.storage.folder = None;
                match result {
                    Ok(updated) => {
                        self.update_instance(&id, |i| *i = updated);
                        self.toasts.push(Kind::Success, "Game folder changed", "");
                    }
                    Err(e) => self
                        .toasts
                        .push(Kind::Error, "Couldn't change the game folder", e),
                }
            }
        }
    }

    /// The Storage dialogs (a folder to look at, the move, what went wrong).
    pub(crate) fn storage_dialog(&mut self, ctx: &egui::Context) {
        self.instance_folder_dialog(ctx);
        let Some(dialog) = self.storage.dialog.take() else {
            return;
        };
        let p = self.palette();
        let moving = matches!(dialog, Dialog::Moving { .. });
        let from = self.dirs.root().to_path_buf();
        let game_running = self.runs.any_active();
        let mut next: Option<Option<Dialog>> = None;
        let modal = Modal::new(Id::new("storage_dialog"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(DIALOG_WIDTH);
                match &dialog {
                    Dialog::Check(c) => next = self.check_view(ui, p, c, &from, game_running),
                    Dialog::Moving { to, done, total } => {
                        title(ui, p, "Moving Arctic's files…");
                        ui.label(path_text(p, to));
                        ui.add_space(8.0);
                        let fraction = if *total == 0 {
                            0.0
                        } else {
                            *done as f32 / *total as f32
                        };
                        ui.add(egui::ProgressBar::new(fraction).text(format!(
                            "{} of {}",
                            format_bytes(*done),
                            format_bytes(*total)
                        )));
                        note(ui, p, "Arctic restarts when it's done.");
                    }
                    Dialog::Failed(e) => {
                        title(ui, p, "The files couldn't be moved");
                        ui.label(RichText::new(e).color(p.error));
                        note(
                            ui,
                            p,
                            format!("Nothing changed: Arctic still uses {}.", from.display()),
                        );
                        ui.add_space(8.0);
                        if widgets::button(ui, p, None, "OK", true).clicked() {
                            next = Some(None);
                        }
                    }
                }
            });
        if modal.should_close() && !moving {
            next = Some(None);
        }
        self.storage.dialog = match next {
            Some(next) => next,
            None => Some(dialog),
        };
    }

    /// A picked folder: what happens if it's used. Returns the dialog's next state, if it changes.
    fn check_view(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        c: &Checked,
        from: &Path,
        game_running: bool,
    ) -> Option<Option<Dialog>> {
        let mut next = None;
        let default = location::default_root().ok();
        if let Some(problem) = &c.problem {
            title(ui, p, "Can't use this folder");
            ui.label(RichText::new(problem).color(p.error));
        } else {
            match c.target {
                Target::Empty => {
                    title(ui, p, "Move Arctic's files here?");
                    ui.label(path_text(p, &c.to));
                    note(
                        ui,
                        p,
                        format!(
                            "{} move from {}. Arctic restarts when it's done, then removes the old copy.",
                            format_bytes(c.size),
                            from.display()
                        ),
                    );
                    if game_running {
                        ui.label(RichText::new("Close Minecraft first.").color(p.error));
                    }
                }
                Target::ArcticData => {
                    title(ui, p, "This folder already has Arctic's files");
                    ui.label(path_text(p, &c.to));
                    note(
                        ui,
                        p,
                        format!(
                            "Use them as they are? Your current files stay in {}.",
                            from.display()
                        ),
                    );
                }
                Target::Other => {
                    title(ui, p, "This folder has other things in it");
                    note(
                        ui,
                        p,
                        "Arctic can keep its files in a folder of its own inside it:",
                    );
                    ui.label(path_text(p, &c.to.join(OWN_FOLDER)));
                }
            }
        }
        ui.add_space(10.0);
        widgets::row(ui, |ui| {
            match (c.problem.is_some(), c.target) {
                (false, Target::Empty) => {
                    let go = ui.add_enabled_ui(!game_running && default.is_some(), |ui| {
                        widgets::button(ui, p, None, "Move files", true)
                    });
                    if go.inner.clicked()
                        && let Some(default) = default.clone()
                    {
                        self.tasks
                            .storage_move(default, from.to_path_buf(), c.to.clone());
                        next = Some(Some(Dialog::Moving {
                            to: c.to.clone(),
                            done: 0,
                            total: c.size,
                        }));
                    }
                }
                (false, Target::ArcticData) => {
                    if widgets::button(ui, p, None, "Use these files", true).clicked()
                        && let Some(default) = &default
                    {
                        match location::adopt(default, &c.to) {
                            Ok(()) => {
                                next = Some(None);
                                self.restart_after_update(ui.ctx());
                            }
                            Err(e) => next = Some(Some(Dialog::Failed(e.to_string()))),
                        }
                    }
                }
                (false, Target::Other) => {
                    if widgets::button(ui, p, None, "Use that folder", true).clicked() {
                        self.tasks
                            .storage_check(from.to_path_buf(), c.to.join(OWN_FOLDER));
                        next = Some(None);
                    }
                }
                (true, _) => {}
            }
            if widgets::button(ui, p, None, "Pick another", false).clicked() {
                self.tasks.storage_pick(from.to_path_buf());
                next = Some(None);
            }
            if widgets::button(ui, p, None, "Cancel", false).clicked() {
                next = Some(None);
            }
        });
        next
    }

    /// Instance settings: where this instance keeps its game.
    pub(crate) fn game_folder_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let id = instance.id.clone();
        let folder = instance.game_dir(&self.dirs);
        let running = self.runs.instance_active(&id);
        ui.add_space(12.0);
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Game folder").size(17.0).strong().color(p.text));
            note(ui, p, "This instance's worlds, mods, resource packs, screenshots and options:");
            ui.label(path_text(p, &folder));
            ui.add_space(4.0);
            widgets::row(ui, |ui| {
                if widgets::button(ui, p, Some(Icon::Folder), "Open", false).clicked() {
                    let _ = std::fs::create_dir_all(&folder);
                    if let Err(e) = open::that_detached(&folder) {
                        self.toasts.push(Kind::Error, "Could not open folder", e.to_string());
                    }
                }
                let change = ui.add_enabled_ui(!running, |ui| {
                    widgets::button(ui, p, Some(Icon::Import), "Change…", false)
                });
                if change.inner.clicked() {
                    self.tasks.instance_folder_pick(id.clone());
                }
                if instance.game_folder.is_some() {
                    let back = ui.add_enabled_ui(!running, |ui| {
                        widgets::button(ui, p, None, "Use the instance's own folder", false)
                    });
                    if back.inner.clicked() {
                        let own = instance.default_game_dir(&self.dirs);
                        self.storage.folder = Some(FolderDialog {
                            id: id.clone(),
                            folder: None,
                            empty: arctic_core::instances::folder_is_empty(&own),
                            busy: false,
                        });
                    }
                }
            });
            if running {
                note(ui, p, "Close this instance to change its folder.");
            } else {
                note(ui, p, "Pick a folder on any drive, or an existing .minecraft-style folder to use as it is.");
            }
        });
    }

    fn instance_folder_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.storage.folder else {
            return;
        };
        let Some(instance) = self.instance_by_id(&dialog.id).cloned() else {
            self.storage.folder = None;
            return;
        };
        let p = self.palette();
        let current = instance.game_dir(&self.dirs);
        let target = dialog
            .folder
            .clone()
            .unwrap_or_else(|| instance.default_game_dir(&self.dirs));
        let (empty, busy, back) = (dialog.empty, dialog.busy, dialog.folder.is_none());
        let mut choice: Option<Option<FolderChange>> = None;
        let modal = Modal::new(Id::new("instance_folder")).frame(dialog_frame(p)).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            if busy {
                title(ui, p, "Moving the instance's files…");
                ui.label(path_text(p, &target));
                ui.add(egui::ProgressBar::new(0.0).animate(true));
                return;
            }
            title(ui, p, if back { "Use the instance's own folder?" } else { "Use this game folder?" });
            ui.label(path_text(p, &target));
            if empty {
                note(ui, p, format!("Move this instance's worlds, mods and settings from {} there?", current.display()));
            } else {
                note(
                    ui,
                    p,
                    format!(
                        "It already has files. Use it as it is (an existing .minecraft, say)? This instance's current files stay in {}.",
                        current.display()
                    ),
                );
            }
            ui.add_space(10.0);
            widgets::row(ui, |ui| {
                if empty && widgets::button(ui, p, None, "Move files there", true).clicked() {
                    choice = Some(Some(FolderChange::MoveFiles));
                }
                if widgets::button(ui, p, None, "Use it as it is", !empty).clicked() {
                    choice = Some(Some(FolderChange::UseAsIs));
                }
                if widgets::button(ui, p, None, "Cancel", false).clicked() {
                    choice = Some(None);
                }
            });
        });
        if modal.should_close() && !busy {
            choice = Some(None);
        }
        match choice {
            Some(Some(change)) => {
                let folder = self.storage.folder.as_mut().and_then(|d| {
                    d.busy = true;
                    d.folder.clone()
                });
                self.tasks.instance_folder_set(instance, folder, change);
            }
            Some(None) => self.storage.folder = None,
            None => {}
        }
    }
}

fn title(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(20.0).strong().color(p.text));
    ui.add_space(6.0);
}
