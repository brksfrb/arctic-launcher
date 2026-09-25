//! "New instance" dialog: name, loader, Minecraft version, loader version.

use arctic_core::instances::{self, Loader};
use arctic_core::loaders::LoaderKind;
use eframe::egui::{self, Id, Modal, RichText};

use super::Load;
use crate::app::{ArcticApp, ManifestState};
use crate::art::icons::Icon;
use crate::toasts::Kind;
use crate::widgets;

#[derive(Debug, Clone, Default)]
pub struct CreateForm {
    pub name: String,
    /// The user typed a name (stop auto-naming).
    pub name_edited: bool,
    pub loader: Option<LoaderKind>,
    pub game: Option<String>,
    pub loader_version: Option<String>,
}

impl ArcticApp {
    pub(crate) fn open_create_dialog(&mut self) {
        self.inst.create = Some(CreateForm {
            loader: Some(LoaderKind::Fabric),
            ..CreateForm::default()
        });
        self.create_form_defaults();
    }

    /// Minecraft versions offered for `loader` (respecting the snapshot /
    /// old-version settings), newest first. `None` while still loading.
    fn create_game_versions(&self, loader: Option<LoaderKind>) -> Option<Vec<String>> {
        let ManifestState::Ready(manifest) = &self.manifest else {
            return None;
        };
        let visible = self.visible_versions(manifest);
        match loader {
            None => Some(visible.into_iter().map(|v| v.id).collect()),
            Some(kind) => match self.inst.loader_games.get(&kind) {
                Some(Load::Ready(supported)) => Some(
                    visible
                        .into_iter()
                        .filter(|v| supported.contains(&v.id))
                        .map(|v| v.id)
                        .collect(),
                ),
                _ => None,
            },
        }
    }

    /// Fill in defaults and kick off metadata fetches as the form changes.
    pub(crate) fn create_form_defaults(&mut self) {
        let Some(form) = self.inst.create.clone() else {
            return;
        };
        let mut form = form;
        if let Some(kind) = form.loader
            && !self.inst.loader_games.contains_key(&kind)
        {
            self.inst.loader_games.insert(kind, Load::Loading);
            self.tasks.loader_games(kind);
        }
        let games = self.create_game_versions(form.loader);
        if let Some(games) = &games
            && form.game.as_ref().is_none_or(|g| !games.contains(g))
        {
            form.game = games.first().cloned();
            form.loader_version = None;
        }
        if let (Some(kind), Some(game)) = (form.loader, form.game.clone()) {
            match self.inst.loader_versions.get(&(kind, game.clone())) {
                None => {
                    self.inst
                        .loader_versions
                        .insert((kind, game.clone()), Load::Loading);
                    self.tasks.loader_versions(kind, game);
                }
                Some(Load::Ready(list)) if form.loader_version.is_none() => {
                    form.loader_version = list
                        .iter()
                        .find(|v| v.stable)
                        .or(list.first())
                        .map(|v| v.id.clone());
                }
                _ => {}
            }
        }
        if !form.name_edited {
            let loader = form.loader.map_or("Vanilla", LoaderKind::label);
            form.name = format!("{loader} {}", form.game.as_deref().unwrap_or(""))
                .trim()
                .to_owned();
        }
        self.inst.create = Some(form);
    }

    pub(crate) fn create_instance_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.inst.create.clone() else {
            return;
        };
        let p = self.palette();
        let before = (form.loader, form.game.clone());
        let mut submit = false;
        let modal = Modal::new(Id::new("create_instance"))
            .frame(super::detail::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.label(
                    RichText::new("New instance")
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(10.0);
                ui.label(RichText::new("LOADER").small().color(p.muted));
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut form.loader, None, "Vanilla");
                    for kind in LoaderKind::ALL {
                        ui.selectable_value(&mut form.loader, Some(kind), kind.label());
                    }
                });
                ui.add_space(8.0);
                self.create_version_rows(ui, &mut form);
                ui.add_space(8.0);
                ui.label(RichText::new("NAME").small().color(p.muted));
                if ui
                    .add(widgets::text_field(&mut form.name).desired_width(f32::INFINITY))
                    .changed()
                {
                    form.name_edited = true;
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                    let ready = !form.name.trim().is_empty()
                        && form.game.is_some()
                        && (form.loader.is_none() || form.loader_version.is_some());
                    let create = ui
                        .add_enabled_ui(ready, |ui| {
                            widgets::button(ui, p, Some(Icon::Plus), "Create instance", true)
                        })
                        .inner;
                    submit = ready && create.clicked();
                });
            });
        if modal.should_close() {
            self.inst.create = None;
            return;
        }
        if submit {
            self.inst.create = None;
            self.finish_create(form);
            return;
        }
        if (form.loader, form.game.clone()) != before {
            form.loader_version = None;
        }
        self.inst.create = Some(form);
        self.create_form_defaults();
    }

    fn create_version_rows(&self, ui: &mut egui::Ui, form: &mut CreateForm) {
        let p = self.palette();
        ui.label(RichText::new("MINECRAFT VERSION").small().color(p.muted));
        match self.create_game_versions(form.loader) {
            None => loading_row(
                ui,
                p,
                form.loader
                    .and_then(|k| match self.inst.loader_games.get(&k) {
                        Some(Load::Failed(e)) => Some(e.clone()),
                        _ => None,
                    }),
            ),
            Some(games) if games.is_empty() => {
                ui.label(RichText::new("No versions available for this loader.").color(p.warn));
            }
            Some(games) => {
                egui::ComboBox::from_id_salt("create_game")
                    .width(260.0)
                    .height(320.0)
                    .selected_text(form.game.clone().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for g in games {
                            ui.selectable_value(&mut form.game, Some(g.clone()), g);
                        }
                    });
            }
        }
        let (Some(kind), Some(game)) = (form.loader, form.game.clone()) else {
            return;
        };
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("{} VERSION", kind.label().to_uppercase()))
                .small()
                .color(p.muted),
        );
        match self.inst.loader_versions.get(&(kind, game)) {
            Some(Load::Ready(list)) if !list.is_empty() => {
                egui::ComboBox::from_id_salt("create_loader")
                    .width(260.0)
                    .height(320.0)
                    .selected_text(form.loader_version.clone().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for v in list {
                            let label = if v.stable {
                                format!("{}  (stable)", v.id)
                            } else {
                                v.id.clone()
                            };
                            ui.selectable_value(
                                &mut form.loader_version,
                                Some(v.id.clone()),
                                label,
                            );
                        }
                    });
            }
            Some(Load::Ready(_)) => {
                ui.label(RichText::new("No loader builds for this version.").color(p.warn));
            }
            Some(Load::Failed(e)) => loading_row(ui, p, Some(e.clone())),
            _ => loading_row(ui, p, None),
        }
    }

    fn finish_create(&mut self, form: CreateForm) {
        let (Some(game), name) = (form.game, form.name) else {
            return;
        };
        let loader = Loader::new(form.loader, form.loader_version.unwrap_or_default());
        match instances::create(&self.dirs, &name, &game, loader) {
            Ok(instance) => {
                let id = instance.id.clone();
                self.reload_instances();
                self.toasts
                    .push(Kind::Success, format!("Created {}", instance.name), "");
                self.open_instance(&id);
                if instance.loader.kind().is_some() {
                    self.inst.page = super::InstancePage::Browse;
                }
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not create instance", e.to_string()),
        }
    }
}

fn loading_row(ui: &mut egui::Ui, p: &crate::theme::Palette, error: Option<String>) {
    ui.horizontal(|ui| match error {
        Some(e) => {
            ui.label(RichText::new(e).color(p.error));
        }
        None => {
            ui.spinner();
            ui.label(RichText::new("Loading…").color(p.muted));
        }
    });
}
