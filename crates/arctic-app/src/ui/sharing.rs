//! Share and Import dialogs: an instance (as a mod list), the HUD layout,
//! the crosshair, all client settings, or the whole profile, as a short
//! code, one line of text, or a .json file.

use arctic_core::instances::Instance;
use arctic_core::sharing::{self, Bundle, ClientPart, Input, InstancePack, Part, ProfilePack};
use eframe::egui::{self, Id, Modal, RichText};

use crate::app::{ArcticApp, Tab};
use crate::art::icons::Icon;
use crate::share_tasks::Imported;
use crate::tasks::ProgressSnapshot;
use crate::toasts::Kind;
use crate::widgets;

const WIDTH: f32 = 500.0;
const MAX_LISTED_MODS: usize = 8;
const MAX_LISTED_WARNINGS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareKind {
    Instance,
    Hud,
    Crosshair,
    Client,
    Profile,
}

impl ShareKind {
    const FOR_INSTANCE: [(ShareKind, &'static str); 4] = [
        (ShareKind::Instance, "Mods & version"),
        (ShareKind::Hud, "HUD layout"),
        (ShareKind::Crosshair, "Crosshair"),
        (ShareKind::Client, "All client settings"),
    ];

    fn part(self) -> Option<Part> {
        match self {
            ShareKind::Hud => Some(Part::Hud),
            ShareKind::Crosshair => Some(Part::Crosshair),
            ShareKind::Client => Some(Part::All),
            _ => None,
        }
    }
}

pub enum CodeState {
    Making,
    Ready(String),
    Failed(String),
}

pub struct ExportState {
    /// Instance being shared from (`None` for the profile).
    instance: Option<String>,
    kind: ShareKind,
    /// The bundle and mod files left out of it, or why it can't be made.
    built: Result<(Bundle, Vec<String>), String>,
    code: Option<CodeState>,
    note: Option<String>,
}

enum ImportStage {
    Edit(Option<String>),
    Loading,
    Preview(Box<Bundle>),
    Working,
}

pub struct ImportState {
    input: String,
    stage: ImportStage,
    /// Instance client settings are applied to.
    target: String,
}

#[derive(Default)]
pub struct ShareUi {
    pub export: Option<ExportState>,
    pub import: Option<ImportState>,
    pub progress: Option<ProgressSnapshot>,
}

impl ArcticApp {
    /// Open the Share dialog for an instance (`None`: the whole profile).
    pub(crate) fn open_share(&mut self, instance: Option<String>) {
        let kind = if instance.is_some() {
            ShareKind::Instance
        } else {
            ShareKind::Profile
        };
        let built = self.build_share(instance.as_deref(), kind);
        self.sharing.export = Some(ExportState {
            instance,
            kind,
            built,
            code: None,
            note: None,
        });
    }

    pub(crate) fn open_share_import(&mut self) {
        self.sharing.import = Some(ImportState {
            input: String::new(),
            stage: ImportStage::Edit(None),
            target: self.selected_instance().id.clone(),
        });
    }

    fn build_share(
        &self,
        instance: Option<&str>,
        kind: ShareKind,
    ) -> Result<(Bundle, Vec<String>), String> {
        let found = instance.and_then(|id| self.instance_by_id(id)).cloned();
        let result = match (kind, found) {
            (ShareKind::Profile, _) => {
                ProfilePack::export(&self.dirs).map(|(p, left)| (Bundle::Profile(p), left))
            }
            (ShareKind::Instance, Some(i)) => {
                let play = self.settings.last_version.as_deref();
                InstancePack::export(&self.dirs, &i, play, false).and_then(|e| {
                    if e.pack.game_version.is_none() {
                        return Err(arctic_core::Error::Other(
                            "Pick a version on the Play tab first.".into(),
                        ));
                    }
                    Ok((Bundle::Instance(e.pack), e.left_out))
                })
            }
            (k, Some(i)) => {
                ClientPart::read(&i.game_dir(&self.dirs), k.part().unwrap_or(Part::All))
                    .map(|c| (Bundle::Client(c), Vec::new()))
            }
            (_, None) => Err(arctic_core::Error::Other("That instance is gone.".into())),
        };
        result.map_err(|e| e.to_string())
    }

    pub(crate) fn share_dialogs(&mut self, ctx: &egui::Context) {
        self.export_dialog(ctx);
        self.import_dialog(ctx);
    }

    fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.sharing.export.take() else {
            return;
        };
        let p = self.palette();
        let mut rebuild = None;
        let mut close = false;
        let modal = Modal::new(Id::new("share_export"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                let title = if state.instance.is_some() {
                    "Share"
                } else {
                    "Share your profile"
                };
                ui.label(RichText::new(title).size(22.0).strong().color(p.text));
                if state.instance.is_some() {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for (kind, label) in ShareKind::FOR_INSTANCE {
                            if ui.selectable_label(state.kind == kind, label).clicked()
                                && state.kind != kind
                            {
                                rebuild = Some(kind);
                            }
                        }
                    });
                } else {
                    ui.label(
                        RichText::new(
                            "Settings, instances (as mod lists) and client settings. Never accounts or passwords.",
                        )
                        .color(p.muted),
                    );
                }
                ui.add_space(10.0);
                match state.built.clone() {
                    Err(e) => {
                        ui.label(RichText::new(e).color(p.error));
                    }
                    Ok((bundle, left_out)) => {
                        ui.label(RichText::new(bundle.summary()).strong().color(p.text));
                        let notes: Vec<String> = left_out
                            .iter()
                            .map(|f| {
                                format!("{f} isn't from Modrinth, so it's left out; send it separately.")
                            })
                            .collect();
                        warnings(ui, p, &notes);
                        ui.add_space(10.0);
                        self.export_buttons(ui, &mut state, bundle);
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Codes are kept on the Arctic server; text and files work anywhere.")
                            .small()
                            .color(p.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, p, None, "Done", false).clicked() {
                            close = true;
                        }
                    });
                });
            });
        if let Some(kind) = rebuild {
            state.kind = kind;
            state.built = self.build_share(state.instance.as_deref(), kind);
            state.code = None;
            state.note = None;
        }
        if !(close || modal.should_close()) {
            self.sharing.export = Some(state);
        }
    }

    fn export_buttons(&mut self, ui: &mut egui::Ui, state: &mut ExportState, bundle: Bundle) {
        let p = self.palette();
        let account = self.accounts.active().cloned();
        ui.horizontal(|ui| {
            let making = matches!(state.code, Some(CodeState::Making));
            let can_code = account.is_some() && !making;
            let code = ui
                .add_enabled_ui(can_code, |ui| {
                    widgets::button(ui, p, Some(Icon::Share), "Get a code", true)
                })
                .inner
                .on_disabled_hover_text(
                    "Add an account first; codes are made by signed-in players.",
                );
            if code.clicked()
                && let Some(account) = account
            {
                state.code = Some(CodeState::Making);
                self.tasks.share_code(bundle.clone(), account);
            }
            if widgets::button(ui, p, Some(Icon::Copy), "Copy as text", false).clicked() {
                ui.ctx().copy_text(bundle.to_text());
                state.note = Some("Copied. Paste it anywhere; it works without the server.".into());
            }
            if widgets::button(ui, p, Some(Icon::Document), "Save file…", false).clicked() {
                let name = format!("arctic-{}.json", bundle.kind());
                self.tasks.share_save(bundle.clone(), name);
            }
        });
        match &state.code {
            Some(CodeState::Making) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Making a code…").color(p.muted));
                });
            }
            Some(CodeState::Ready(code)) => {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(code)
                            .size(26.0)
                            .strong()
                            .monospace()
                            .color(p.accent),
                    );
                    if widgets::icon_button(ui, p, Icon::Copy, "Copy code").clicked() {
                        ui.ctx().copy_text(code.clone());
                    }
                });
                ui.label(
                    RichText::new("Copied. Friends paste it into Import (Instances tab) or run `arctic import`.")
                        .small()
                        .color(p.muted),
                );
            }
            Some(CodeState::Failed(e)) => {
                ui.label(RichText::new(e).color(p.error));
            }
            None => {}
        }
        if let Some(note) = &state.note {
            ui.label(RichText::new(note).small().color(p.muted));
        }
    }

    fn import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.sharing.import.take() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let modal = Modal::new(Id::new("share_import"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.label(RichText::new("Import").size(22.0).strong().color(p.text));
                ui.add_space(8.0);
                close = match &state.stage {
                    ImportStage::Edit(_) => self.import_edit(ui, &mut state),
                    ImportStage::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Looking it up…").color(p.muted));
                        });
                        false
                    }
                    ImportStage::Preview(_) => self.import_preview(ui, &mut state),
                    ImportStage::Working => {
                        self.import_progress(ui);
                        false
                    }
                };
            });
        let busy = matches!(state.stage, ImportStage::Working | ImportStage::Loading);
        if !(close || (modal.should_close() && !busy)) {
            self.sharing.import = Some(state);
        }
    }

    /// Returns true to close.
    fn import_edit(&mut self, ui: &mut egui::Ui, state: &mut ImportState) -> bool {
        let p = self.palette();
        ui.label(
            RichText::new("Paste a code (like abcd-efgh) or share text, or open a .json file.")
                .color(p.muted),
        );
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::multiline(&mut state.input)
                .hint_text("abcd-efgh")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if let ImportStage::Edit(Some(e)) = &state.stage {
            ui.label(RichText::new(e).color(p.error));
        }
        ui.add_space(10.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if widgets::button(ui, p, Some(Icon::Folder), "Open file…", false).clicked() {
                state.stage = ImportStage::Loading;
                self.tasks.share_open_file();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready = !state.input.trim().is_empty();
                let go = ui
                    .add_enabled_ui(ready, |ui| widgets::button(ui, p, None, "Continue", true))
                    .inner;
                if go.clicked() {
                    state.stage = self.read_input(&state.input);
                }
                if widgets::button(ui, p, None, "Cancel", false).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// Pasted text: a bundle to preview, or a code to fetch.
    fn read_input(&self, input: &str) -> ImportStage {
        match sharing::read(input) {
            Ok(Input::Bundle(b)) => ImportStage::Preview(Box::new(b)),
            Ok(Input::Code(code)) => {
                self.tasks.share_fetch(code);
                ImportStage::Loading
            }
            Err(e) => ImportStage::Edit(Some(e.to_string())),
        }
    }

    /// Debug builds: `ARCTIC_DEVSHOT_SHARE=export|profile|import:<text>`
    /// opens a sharing dialog once (for screenshots).
    pub(crate) fn devshot_share(&mut self) {
        static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !cfg!(debug_assertions) || OPENED.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let Ok(what) = std::env::var("ARCTIC_DEVSHOT_SHARE") else {
            return;
        };
        OPENED.store(true, std::sync::atomic::Ordering::Relaxed);
        match what.split_once(':') {
            Some(("import", text)) => {
                self.open_share_import();
                self.devshot_import_text(text);
            }
            _ if what == "profile" => self.open_share(None),
            _ if what == "migrate" => self.open_migrate(),
            _ => self.open_share(self.inst.open.clone()),
        }
    }

    /// Debug screenshots: import `text` as if pasted.
    pub(crate) fn devshot_import_text(&mut self, text: &str) {
        let stage = self.read_input(text);
        if let Some(state) = &mut self.sharing.import {
            state.input = text.to_owned();
            state.stage = stage;
        }
    }

    fn import_preview(&mut self, ui: &mut egui::Ui, state: &mut ImportState) -> bool {
        let p = self.palette();
        let ImportStage::Preview(bundle) = &state.stage else {
            return false;
        };
        let bundle = (**bundle).clone();
        ui.label(RichText::new(bundle.summary()).strong().color(p.text));
        ui.add_space(4.0);
        let mut blocked = None;
        match &bundle {
            Bundle::Instance(pack) => {
                let mut titles: Vec<&str> = pack
                    .mods
                    .iter()
                    .filter(|m| !m.dependency)
                    .map(|m| m.title.as_str())
                    .collect();
                let more = titles.len().saturating_sub(MAX_LISTED_MODS);
                titles.truncate(MAX_LISTED_MODS);
                if !titles.is_empty() {
                    let mut line = titles.join(", ");
                    if more > 0 {
                        line.push_str(&format!(" and {more} more"));
                    }
                    ui.label(RichText::new(line).color(p.muted));
                }
                ui.label(
                    RichText::new("It becomes a new instance; the exact same mod versions are downloaded from Modrinth.")
                        .small()
                        .color(p.muted),
                );
            }
            Bundle::Client(_) => {
                let mut all = vec![self.instance.clone()];
                all.extend(self.custom_instances.iter().cloned());
                let current = all
                    .iter()
                    .find(|i| i.id == state.target)
                    .map_or_else(|| "Vanilla".to_owned(), |i| i.name.clone());
                ui.horizontal(|ui| {
                    ui.label("Apply to");
                    egui::ComboBox::from_id_salt("share_target")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for i in &all {
                                ui.selectable_value(&mut state.target, i.id.clone(), &i.name);
                            }
                        });
                });
                let in_use = self
                    .instance_by_id(&state.target)
                    .is_some_and(|i| sharing::client::in_use(&i.game_dir(&self.dirs)));
                if in_use || self.runs.instance_active(&state.target) {
                    blocked =
                        Some("Close that game first; it saves its own settings when it quits.");
                }
            }
            Bundle::Profile(_) => {
                ui.label(
                    RichText::new("Your launcher settings change to these. New instances are added; yours with the same names stay as they are.")
                        .small()
                        .color(p.muted),
                );
                if self.runs.any_game()
                    || sharing::client::in_use(&self.instance.game_dir(&self.dirs))
                {
                    blocked = Some("Close your games first.");
                }
            }
        }
        if let Some(why) = blocked {
            ui.label(RichText::new(why).color(p.error));
        }
        ui.add_space(12.0);
        let mut back = false;
        ui.horizontal(|ui| {
            if widgets::button(ui, p, None, "Back", false).clicked() {
                back = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let go = ui
                    .add_enabled_ui(blocked.is_none(), |ui| {
                        widgets::button(ui, p, Some(Icon::Import), "Import", true)
                    })
                    .inner;
                if go.clicked() {
                    let target: Option<Instance> = self.instance_by_id(&state.target).cloned();
                    self.sharing.progress = None;
                    self.tasks.share_import(bundle.clone(), target);
                    state.stage = ImportStage::Working;
                }
            });
        });
        if back {
            state.stage = ImportStage::Edit(None);
        }
        false
    }

    fn import_progress(&self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.spinner();
            match &self.sharing.progress {
                Some(s) if s.bytes_total > 0 => {
                    let f = s.bytes_done as f32 / s.bytes_total as f32;
                    ui.add(
                        egui::ProgressBar::new(f)
                            .desired_width(360.0)
                            .text(s.stage.clone()),
                    );
                }
                Some(s) => {
                    ui.label(RichText::new(&s.stage).color(p.muted));
                }
                None => {
                    ui.label(RichText::new("Importing…").color(p.muted));
                }
            }
        });
    }

    pub(crate) fn on_share_code(&mut self, result: Result<String, String>, ctx: &egui::Context) {
        let Some(state) = &mut self.sharing.export else {
            return;
        };
        state.code = Some(match result {
            Ok(code) => {
                ctx.copy_text(code.clone());
                CodeState::Ready(code)
            }
            Err(e) => CodeState::Failed(e),
        });
    }

    pub(crate) fn on_share_saved(&mut self, result: Result<Option<std::path::PathBuf>, String>) {
        match result {
            Ok(Some(path)) => {
                if let Some(state) = &mut self.sharing.export {
                    state.note = Some(format!("Saved to {}", path.display()));
                }
            }
            Ok(None) => {}
            Err(e) => self.toasts.push(Kind::Error, "Could not save", e),
        }
    }

    pub(crate) fn on_share_loaded(&mut self, result: Result<Option<Bundle>, String>) {
        let Some(state) = &mut self.sharing.import else {
            return;
        };
        state.stage = match result {
            Ok(Some(bundle)) => ImportStage::Preview(Box::new(bundle)),
            Ok(None) => ImportStage::Edit(None),
            Err(e) => ImportStage::Edit(Some(e)),
        };
    }

    pub(crate) fn on_share_imported(&mut self, result: Result<Imported, String>, now: f64) {
        self.sharing.progress = None;
        match result {
            Ok(done) => {
                self.sharing.import = None;
                if done.settings_changed {
                    self.reload_after_import();
                } else {
                    self.reload_instances();
                }
                self.toasts.push(Kind::Success, done.title, done.detail);
                let shown = done.warnings.len().min(MAX_LISTED_WARNINGS);
                if shown > 0 {
                    let mut text = done.warnings[..shown].join("\n");
                    if done.warnings.len() > shown {
                        text.push_str(&format!("\n…and {} more", done.warnings.len() - shown));
                    }
                    self.toasts
                        .push(Kind::Info, "Some things were left out", text);
                }
                if let Some(id) = done.open {
                    self.set_tab(Tab::Instances, now);
                    self.open_instance(&id);
                }
            }
            Err(e) => {
                if let Some(state) = &mut self.sharing.import {
                    state.stage = ImportStage::Edit(Some(e));
                }
            }
        }
    }
}

fn warnings(ui: &mut egui::Ui, p: &crate::theme::Palette, list: &[String]) {
    for w in list.iter().take(MAX_LISTED_WARNINGS) {
        ui.label(RichText::new(w).small().color(p.error));
    }
    if list.len() > MAX_LISTED_WARNINGS {
        ui.label(
            RichText::new(format!("…and {} more", list.len() - MAX_LISTED_WARNINGS))
                .small()
                .color(p.error),
        );
    }
}
