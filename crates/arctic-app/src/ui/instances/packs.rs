//! Resource packs and shaders for an instance: what's installed (switch on
//! or off, delete) and a Modrinth browser to add more.

use std::collections::HashSet;

use arctic_core::instances::Instance;
use arctic_core::mods::pack_groups::PackGroups;
use arctic_core::mods::packs::{self, Pack, PackKind};
use arctic_core::mods::{ProjectHit, SearchPage, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::browse::{compact, sort_combo};
use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};
const PAGE_SIZE: usize = 20;
const SEARCH_DELAY: f64 = 0.35;
const RESCAN_EVERY: f64 = 2.0;
const MB: f64 = 1024.0 * 1024.0;

pub struct PacksUi {
    kind: PackKind,
    text: String,
    sort: SortBy,
    request: u64,
    results: Option<Result<SearchPage, String>>,
    /// The instance, pack kind and game folder the results and list are for.
    showing: Option<(String, PackKind, std::path::PathBuf)>,
    rescan_at: f64,
    installed: Vec<Pack>,
    /// The groups the packs are filed under (shared with the game).
    groups: PackGroups,
    /// The name being typed for a new group.
    new_group: String,
    pub(super) installing: HashSet<String>,
    typed_at: Option<f64>,
}

impl PacksUi {
    pub(crate) fn show_shaders(&mut self) {
        self.kind = PackKind::Shader;
    }
}

impl Default for PacksUi {
    fn default() -> Self {
        Self {
            kind: PackKind::Resource,
            text: String::new(),
            sort: SortBy::Downloads,
            request: 0,
            results: None,
            showing: None,
            rescan_at: 0.0,
            installed: Vec::new(),
            groups: PackGroups::default(),
            new_group: String::new(),
            installing: HashSet::new(),
            typed_at: None,
        }
    }
}

impl ArcticApp {
    /// The instance's Minecraft version (Vanilla follows the Play tab).
    pub(super) fn pack_game(&self, instance: &Instance) -> String {
        instance
            .version
            .clone()
            .or_else(|| self.settings.last_version.clone())
            .unwrap_or_default()
    }

    pub(super) fn packs_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let key = (
            instance.id.clone(),
            self.inst.packs.kind,
            instance.game_dir(&self.dirs),
        );
        let changed = self.inst.packs.showing.as_ref() != Some(&key);
        let now = ui.input(|i| i.time);
        // Files can be added in Explorer while this page is open, or the game can change
        // which packs are on. Re-read the local list without repeating the network search.
        if changed || now >= self.inst.packs.rescan_at {
            self.refresh_packs(instance);
            self.inst.packs.rescan_at = now + RESCAN_EVERY;
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(RESCAN_EVERY));
        if changed {
            self.inst.packs.showing = Some(key);
            self.run_rp_search(instance);
        }
        ui.horizontal(|ui| {
            for kind in [PackKind::Resource, PackKind::Shader] {
                if ui
                    .selectable_label(self.inst.packs.kind == kind, kind.label())
                    .clicked()
                {
                    self.inst.packs.kind = kind;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Folder),
                    t!("instances.open_folder"),
                    false,
                )
                .clicked()
                {
                    let dir = instance
                        .game_dir(&self.dirs)
                        .join(self.inst.packs.kind.folder());
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that_detached(&dir);
                }
            });
        });
        ui.add_space(6.0);
        self.installed_packs(ui, instance);
        ui.add_space(12.0);
        self.rp_search_bar(ui, instance);
        ui.add_space(8.0);
        self.rp_results(ui, instance);
    }

    fn refresh_packs(&mut self, instance: &Instance) {
        let game_dir = instance.game_dir(&self.dirs);
        self.inst.packs.installed = packs::list(self.inst.packs.kind, &game_dir);
        self.inst.packs.groups = PackGroups::load(&game_dir);
    }

    fn installed_packs(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let kind = self.inst.packs.kind;
        let game_dir = instance.game_dir(&self.dirs);
        let list = self.inst.packs.installed.clone();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let hint = match kind {
                PackKind::Resource => {
                    t!("instances.switched_on_packs_stack_the_last_one")
                }
                PackKind::Shader => {
                    t!("instances.one_shader_at_a_time_shaders_run_through")
                }
            };
            ui.label(RichText::new(hint).small().color(p.muted));
            if list.is_empty() {
                ui.label(RichText::new(t!("instances.none_yet_find_some_below")).color(p.muted));
                return;
            }
            // Grouped packs sit together under the group's name; the rest come first.
            let groups = self.inst.packs.groups.clone();
            let mut ordered: Vec<&Pack> = list.iter().collect();
            ordered.sort_by_key(|pk| {
                groups
                    .group_of(kind, &pk.file_name)
                    .map(str::to_lowercase)
                    .unwrap_or_default()
            });
            let mut shown_group: Option<String> = None;
            for pack in ordered {
                let group = groups.group_of(kind, &pack.file_name).map(str::to_owned);
                if group != shown_group {
                    if let Some(name) = &group {
                        ui.add_space(4.0);
                        ui.label(RichText::new(name).strong().color(p.accent));
                    }
                    shown_group = group.clone();
                }
                crate::widgets::row(ui, |ui| {
                    let size = if pack.size > 0 {
                        format!("{:.1} MB", pack.size as f64 / MB)
                    } else {
                        "folder".into()
                    };
                    ui.label(RichText::new(&pack.file_name).color(p.text));
                    ui.label(RichText::new(size).small().color(p.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, p, Icon::Trash, t!("instances.delete"))
                            .clicked()
                        {
                            match packs::remove(kind, &game_dir, &pack.file_name) {
                                Ok(()) => self.refresh_packs(instance),
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    t!("instances.couldnt_delete_it"),
                                    e.to_string(),
                                ),
                            }
                        }
                        self.pack_group_menu(ui, instance, kind, &game_dir, pack, group.as_deref());
                        let label = if pack.active {
                            "On"
                        } else {
                            t!("instances.off")
                        };
                        if widgets::button(ui, p, None, label, pack.active).clicked() {
                            match packs::set_active(kind, &game_dir, &pack.file_name, !pack.active)
                            {
                                Ok(()) => self.refresh_packs(instance),
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    t!("instances.couldnt_switch_it"),
                                    e.to_string(),
                                ),
                            }
                        }
                    });
                });
            }
        });
    }

    /// "Group" on a pack's row: file it under a group, a new one, or none.
    fn pack_group_menu(
        &mut self,
        ui: &mut egui::Ui,
        instance: &Instance,
        kind: PackKind,
        game_dir: &std::path::Path,
        pack: &Pack,
        current: Option<&str>,
    ) {
        let mut choice: Option<Option<String>> = None;
        let names = self.inst.packs.groups.names(kind);
        egui::ComboBox::from_id_salt(("pack_group", &pack.file_name))
            .selected_text(current.unwrap_or(t!("instances.group")))
            .width(110.0)
            // Only a click outside closes it: a click in the "New group…" field must not.
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), t!("instances.no_group"))
                    .clicked()
                {
                    choice = Some(None);
                    ui.close();
                }
                for name in &names {
                    if ui
                        .selectable_label(current == Some(name.as_str()), name)
                        .clicked()
                    {
                        choice = Some(Some(name.clone()));
                        ui.close();
                    }
                }
                ui.separator();
                let field = ui.add(
                    widgets::text_field(&mut self.inst.packs.new_group)
                        .hint_text(t!("instances.new_group"))
                        .desired_width(120.0),
                );
                let typed = self.inst.packs.new_group.trim().to_owned();
                if field.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !typed.is_empty()
                {
                    choice = Some(Some(typed));
                    self.inst.packs.new_group.clear();
                    ui.close();
                }
            });
        if let Some(group) = choice {
            let mut groups = PackGroups::load(game_dir);
            groups.set(kind, &pack.file_name, group.as_deref());
            match groups.save(game_dir) {
                Ok(()) => self.refresh_packs(instance),
                Err(e) => self.toasts.push(
                    Kind::Error,
                    t!("instances.couldnt_save_the_group"),
                    e.to_string(),
                ),
            }
        }
    }

    fn rp_search_bar(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let mut search = false;
        crate::widgets::row(ui, |ui| {
            let hint = match self.inst.packs.kind {
                PackKind::Resource => t!("instances.search_resource_packs_on_modrinth"),
                PackKind::Shader => t!("instances.search_shaders_on_modrinth"),
            };
            let field = ui.add(
                widgets::text_field(&mut self.inst.packs.text)
                    .hint_text(hint)
                    .desired_width(360.0),
            );
            let now = ui.input(|i| i.time);
            if field.changed() {
                self.inst.packs.typed_at = Some(now);
            }
            search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(at) = self.inst.packs.typed_at {
                if now - at >= SEARCH_DELAY {
                    search = true;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(
                            SEARCH_DELAY - (now - at),
                        ));
                }
            }
            let before = self.inst.packs.sort;
            sort_combo(ui, "pack_sort", &mut self.inst.packs.sort);
            search |= self.inst.packs.sort != before;
            search |= widgets::button(ui, p, None, t!("instances.search"), true).clicked();
        });
        if search {
            self.inst.packs.typed_at = None;
            self.run_rp_search(instance);
        }
    }

    fn run_rp_search(&mut self, instance: &Instance) {
        let game = self.pack_game(instance);
        let packs = &mut self.inst.packs;
        packs.request += 1;
        packs.results = None;
        let query = SearchQuery {
            text: packs.text.trim().to_owned(),
            game_version: game,
            loader: None,
            sort: packs.sort,
            offset: 0,
            limit: PAGE_SIZE,
            project_type: packs.kind.project_type(),
        };
        self.tasks.pack_search(packs.request, query);
    }

    fn rp_results(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let game = self.pack_game(instance);
        let hits: Vec<ProjectHit> = match &self.inst.packs.results {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("instances.searching_modrinth")).color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                ui.label(RichText::new(tf!("instances.search_failed", e = e)).color(p.error));
                return;
            }
            Some(Ok(page)) if page.hits.is_empty() => {
                ui.label(RichText::new(t!("instances.nothing_found")).color(p.muted));
                return;
            }
            Some(Ok(page)) => page.hits.clone(),
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for hit in &hits {
                crate::widgets::row(ui, |ui| {
                    if ui
                        .scope(|ui| mod_icon(ui, self, hit.icon_url.as_deref()))
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_text(t!("instances.preview_project"))
                        .clicked()
                    {
                        self.open_project_preview(
                            ui.ctx(),
                            hit.clone(),
                            Some(self.inst.packs.kind),
                        );
                    }
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 130.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(&hit.title).strong().color(p.text),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_text(t!("instances.preview_project"))
                                .clicked()
                            {
                                self.open_project_preview(
                                    ui.ctx(),
                                    hit.clone(),
                                    Some(self.inst.packs.kind),
                                );
                            }
                            ui.label(
                                RichText::new(tf!("common.by_author", author = hit.author))
                                    .small()
                                    .color(p.muted),
                            );
                        });
                        if ui
                            .add(
                                egui::Label::new(RichText::new(&hit.description).color(p.muted))
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            self.open_project_preview(
                                ui.ctx(),
                                hit.clone(),
                                Some(self.inst.packs.kind),
                            );
                        }
                        if hit.is_incompatible(&game) {
                            ui.label(
                                RichText::new(t!("instances.incompatible"))
                                    .small()
                                    .color(p.error),
                            )
                            .on_hover_text(tf!(
                                "instances.no_file_is_tagged_for_minecraft_you_can",
                                game = game
                            ));
                        }
                        ui.label(
                            RichText::new(tf!(
                                "instances.downloads",
                                compact = compact(hit.downloads)
                            ))
                            .small()
                            .color(p.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.inst.packs.installing.contains(&hit.project_id) {
                            ui.spinner();
                        } else if widgets::button(
                            ui,
                            p,
                            Some(Icon::Plus),
                            t!("instances.install"),
                            true,
                        )
                        .clicked()
                        {
                            self.inst.packs.installing.insert(hit.project_id.clone());
                            self.tasks.pack_install(
                                instance.clone(),
                                self.inst.packs.kind,
                                hit.project_id.clone(),
                                self.pack_game(instance),
                            );
                        }
                    });
                });
                ui.separator();
            }
        });
    }

    pub(crate) fn on_pack_search(&mut self, request: u64, result: Result<SearchPage, String>) {
        if request == self.inst.packs.request {
            self.inst.packs.results = Some(result);
        }
    }

    pub(crate) fn on_pack_installed(
        &mut self,
        instance_id: String,
        project: String,
        result: Result<crate::pack_tasks::PackDone, String>,
    ) {
        self.inst.packs.installing.remove(&project);
        match result {
            Ok(done) => {
                if done.shaders_switched_on {
                    self.update_instance(&instance_id, |i| i.shaders = true);
                }
                self.toasts.push(
                    Kind::Success,
                    tf!("instances.installed_5", file = done.file),
                    done.note,
                );
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, t!("instances.couldnt_install_it"), e),
        }
        if self
            .inst
            .packs
            .showing
            .as_ref()
            .is_some_and(|(id, _, _)| *id == instance_id)
            && let Some(instance) = self.instance_by_id(&instance_id).cloned()
        {
            self.refresh_packs(&instance);
        }
    }
}
