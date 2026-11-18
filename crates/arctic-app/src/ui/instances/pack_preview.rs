//! "View mods": what a modpack version contains, read without installing it.

use arctic_core::mods::pack_preview::{PackPreview, Side};
use eframe::egui::{self, Id, Modal, RichText};

use super::Load;
use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::widgets;
use crate::{t, tf};

/// Name and version, the summary, and what it requires: three lines.
const ROW: f32 = 64.0;

/// The open "View mods" dialog.
#[derive(Debug, Clone)]
pub struct PreviewUi {
    project_id: String,
    title: String,
    /// The version looked at; the newest one until the list has loaded and one is picked.
    version_id: Option<String>,
}

impl ArcticApp {
    /// Open the dialog for a modpack: one version, or the newest when `None`.
    pub(super) fn open_pack_preview(
        &mut self,
        project_id: &str,
        title: &str,
        version_id: Option<String>,
    ) {
        self.inst.pack_preview = Some(PreviewUi {
            project_id: project_id.to_owned(),
            title: title.to_owned(),
            version_id,
        });
        if !self.inst.pack_versions.contains_key(project_id) {
            self.inst
                .pack_versions
                .insert(project_id.to_owned(), Load::Loading);
            self.tasks.pack_versions(project_id.to_owned());
        }
    }

    pub(super) fn pack_preview_dialog(&mut self, ctx: &egui::Context) {
        let Some(open) = self.inst.pack_preview.clone() else {
            return;
        };
        let p = self.palette();
        let versions = match self.inst.pack_versions.get(&open.project_id) {
            Some(Load::Ready(list)) => list.clone(),
            _ => Vec::new(),
        };
        let version_id = open
            .version_id
            .clone()
            .or_else(|| versions.first().map(|v| v.id.clone()));
        // Load it the first time it is looked at; later views come from the session cache.
        if let Some(id) = &version_id
            && !self.inst.pack_previews.contains_key(id)
        {
            self.inst.pack_previews.insert(id.clone(), Load::Loading);
            self.tasks.pack_preview(id.clone());
        }
        let state = version_id
            .as_ref()
            .and_then(|id| self.inst.pack_previews.get(id).cloned());
        let versions_failed = match self.inst.pack_versions.get(&open.project_id) {
            Some(Load::Failed(e)) => Some(e.clone()),
            _ => None,
        };
        let mut close = false;
        let mut picked = None;
        let mut retry = false;
        let mut install = false;
        Modal::new(Id::new("pack_preview"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.label(
                    RichText::new(tf!("instances.mods_in_pack", name = open.title))
                        .font(crate::fonts::heading(22.0))
                        .color(p.text),
                );
                if !versions.is_empty() {
                    let current = versions
                        .iter()
                        .find(|v| Some(&v.id) == version_id.as_ref())
                        .map_or_else(String::new, |v| v.label.clone());
                    ui.add_space(6.0);
                    widgets::modal_combo("pack_preview_version")
                        .width(260.0)
                        .height(320.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for v in &versions {
                                let label = format!(
                                    "{}  ·  {}",
                                    v.label,
                                    v.game_versions.last().map_or("", String::as_str)
                                );
                                if ui
                                    .selectable_label(Some(&v.id) == version_id.as_ref(), label)
                                    .clicked()
                                {
                                    picked = Some(v.id.clone());
                                }
                            }
                        });
                }
                ui.add_space(10.0);
                if let Some(e) = versions_failed {
                    ui.label(
                        RichText::new(tf!("instances.could_not_load_the_versions", e = e))
                            .color(p.error),
                    );
                } else {
                    match state {
                        None | Some(Load::Loading) => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new(t!("instances.reading_the_modpack"))
                                        .color(p.muted),
                                );
                            });
                        }
                        Some(Load::Failed(e)) => {
                            ui.label(
                                RichText::new(tf!("instances.could_not_read_the_modpack", e = e))
                                    .color(p.error),
                            );
                            if widgets::button(ui, p, None, t!("instances.try_again"), false)
                                .clicked()
                            {
                                retry = true;
                            }
                        }
                        Some(Load::Ready(preview)) => self.preview_contents(ui, &preview),
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let busy = self.inst.pack_installing.is_some();
                    if ui
                        .add_enabled_ui(!busy && version_id.is_some(), |ui| {
                            widgets::button(ui, p, Some(Icon::Plus), t!("instances.install"), true)
                        })
                        .inner
                        .clicked()
                    {
                        install = true;
                    }
                    if widgets::button(ui, p, None, t!("instances.close"), false).clicked() {
                        close = true;
                    }
                });
            });
        if let Some(id) = picked
            && let Some(ui_state) = self.inst.pack_preview.as_mut()
        {
            ui_state.version_id = Some(id);
        }
        if retry && let Some(id) = &version_id {
            self.inst.pack_previews.remove(id);
        }
        if install {
            self.inst.pack_installing = Some(open.project_id.clone());
            self.inst.install_progress = None;
            self.inst.pack_choosing = None;
            self.inst.pack_preview = None;
            self.tasks.install_modpack(open.project_id, version_id);
        } else if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.inst.pack_preview = None;
        }
    }

    fn preview_contents(&mut self, ui: &mut egui::Ui, preview: &PackPreview) {
        let p = self.palette();
        ui.label(
            RichText::new(tf!(
                "instances.pack_counts",
                mods = preview.mods.len(),
                configs = preview.configs,
                resource_packs = preview.resource_packs,
                shader_packs = preview.shader_packs
            ))
            .color(p.muted),
        );
        ui.add_space(6.0);
        if preview.mods.is_empty() {
            ui.label(RichText::new(t!("instances.this_modpack_lists_no_mods")).color(p.muted));
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("pack_preview_mods")
            .max_height(360.0)
            .auto_shrink([false, true])
            .show_rows(ui, ROW, preview.mods.len(), |ui, range| {
                for m in &preview.mods[range] {
                    ui.allocate_ui(egui::vec2(ui.available_width(), ROW), |ui| {
                        widgets::row(ui, |ui| {
                            mod_icon(ui, self, m.icon_url.as_deref());
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&m.name).strong().color(p.text));
                                let side = match m.side {
                                    Side::Both => None,
                                    Side::Client => Some(t!("instances.client_only")),
                                    Side::Server => Some(t!("instances.server_only")),
                                };
                                let detail = [Some(m.version.as_str()), side]
                                    .into_iter()
                                    .flatten()
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(" · ");
                                if !detail.is_empty() {
                                    ui.label(RichText::new(detail).small().color(p.muted));
                                }
                                if !m.description.is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&m.description).small().color(p.muted),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&m.description);
                                }
                                if !m.requires.is_empty() {
                                    let requires = m.requires.join(", ");
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(tf!(
                                                "instances.requires",
                                                names = requires
                                            ))
                                            .small()
                                            .color(p.accent),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(requires);
                                }
                            });
                        });
                    });
                }
            });
    }
}
