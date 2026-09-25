//! Instances tab: grid of instances, per-instance pages (mods, browse,
//! settings), the create dialog and the icon picker.

mod browse;
mod create;
mod detail;
mod icon_picker;
mod mods_page;

use std::collections::{HashMap, HashSet};

use arctic_core::instances::{self, Instance};
use arctic_core::loaders::{LoaderKind, LoaderVersion};
use arctic_core::mods::{self, ModFile, SearchPage, SortBy};
use eframe::egui::{self, Align2, FontId, RichText, vec2};

pub use create::CreateForm;

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::tasks::{Event, ProgressSnapshot};
use crate::toasts::Kind;
use crate::widgets;

const CARD: [f32; 2] = [260.0, 104.0];

/// Something fetched in the background.
#[derive(Debug, Clone)]
pub enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconState {
    Loading,
    /// Registered with egui under this URI.
    Ready(String),
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InstancePage {
    #[default]
    Mods,
    Browse,
    Settings,
}

#[derive(Default)]
pub struct SearchState {
    pub text: String,
    pub sort: SortBy,
    /// Id of the newest request; older responses are dropped.
    pub request: u64,
    pub loading: bool,
    pub results: Option<Result<SearchPage, String>>,
    /// Instance the results were fetched for.
    pub instance: Option<String>,
    /// When the search text last changed (search runs shortly after typing stops).
    pub typed_at: Option<f64>,
}

/// Stable, unique egui texture URI for a remote icon URL.
fn icon_texture_uri(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("bytes://modicon/{:016x}", hasher.finish())
}

/// All Instances-tab UI state.
#[derive(Default)]
pub struct InstancesUi {
    /// Instance whose page is open (`None` = the grid).
    pub open: Option<String>,
    pub page: InstancePage,
    pub create: Option<CreateForm>,
    pub confirm_delete: bool,
    pub rename: Option<String>,
    /// Mods folder listing for the open instance.
    pub mod_files: Option<(String, Vec<ModFile>)>,
    pub search: SearchState,
    pub installing: HashSet<String>,
    pub install_progress: Option<ProgressSnapshot>,
    pub icons: HashMap<String, IconState>,
    pub loader_games: HashMap<LoaderKind, Load<HashSet<String>>>,
    pub loader_versions: HashMap<(LoaderKind, String), Load<Vec<LoaderVersion>>>,
}

impl ArcticApp {
    pub(crate) fn instance_by_id(&self, id: &str) -> Option<&Instance> {
        if id == self.instance.id {
            return Some(&self.instance);
        }
        self.custom_instances.iter().find(|i| i.id == id)
    }

    pub(crate) fn instance_mut(&mut self, id: &str) -> Option<&mut Instance> {
        if id == self.instance.id {
            return Some(&mut self.instance);
        }
        self.custom_instances.iter_mut().find(|i| i.id == id)
    }

    /// Instance chosen on the Play tab (falls back to Vanilla).
    pub(crate) fn selected_instance(&self) -> &Instance {
        self.settings
            .last_instance
            .as_deref()
            .and_then(|id| self.custom_instances.iter().find(|i| i.id == id))
            .unwrap_or(&self.instance)
    }

    /// "Fabric 0.16.9 · Minecraft 1.21.4" / "Plays any release".
    pub(crate) fn instance_subtitle(instance: &Instance) -> String {
        match (&instance.version, instance.loader.version()) {
            (Some(game), Some(loader)) => {
                format!("{} {loader} · Minecraft {game}", instance.loader.label())
            }
            (Some(game), None) => format!("Minecraft {game}"),
            (None, _) => "Plays any release".to_owned(),
        }
    }

    pub(crate) fn reload_instances(&mut self) {
        self.custom_instances = instances::list_custom(&self.dirs).unwrap_or_default();
    }

    pub(crate) fn instances_tab(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.inst.open.clone() {
            if self.instance_by_id(&id).is_some() {
                self.instance_page(ui, &id);
                return;
            }
            self.inst.open = None;
        }
        self.instances_grid(ui);
    }

    fn instances_grid(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                widgets::page_header(
                    ui,
                    p,
                    "Instances",
                    "Separate game folders, each with its own version, mods and snowflake.",
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if widgets::button(ui, p, Some(Icon::Plus), "New instance", true).clicked() {
                    self.open_create_dialog();
                }
            });
        });
        let mut all = vec![self.instance.clone()];
        all.extend(self.custom_instances.iter().cloned());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
            for instance in &all {
                if self.instance_card(ui, instance) {
                    self.open_instance(&instance.id);
                }
            }
        });
        if self.custom_instances.is_empty() {
            ui.add_space(18.0);
            ui.label(
                RichText::new(
                    "Create a Fabric, Quilt, NeoForge or Forge instance to play with mods.",
                )
                .color(p.muted),
            );
        }
    }

    /// Returns true when clicked.
    fn instance_card(&mut self, ui: &mut egui::Ui, instance: &Instance) -> bool {
        let p = self.palette();
        let selected = self.selected_instance().id == instance.id;
        let (rect, response, _) = widgets::hover_card(ui, p, CARD.into(), selected);
        let emblem_rect =
            egui::Rect::from_min_size(rect.left_top() + vec2(16.0, 22.0), vec2(58.0, 58.0));
        let emblem = widgets::emblem_at(
            ui,
            p,
            &instance.icon,
            emblem_rect,
            response.id.with("emblem"),
        );
        self.icon_picker(&emblem, &instance.id);
        let text_x = emblem_rect.right() + 14.0;
        let text_w = rect.right() - text_x - 12.0;
        widgets::text_elided(
            ui.painter(),
            egui::pos2(text_x, rect.top() + 36.0),
            &instance.name,
            FontId::proportional(16.5),
            p.text,
            text_w,
        );
        widgets::text_elided(
            ui.painter(),
            egui::pos2(text_x, rect.top() + 60.0),
            &Self::instance_subtitle(instance),
            FontId::proportional(12.0),
            p.muted,
            text_w,
        );
        if selected {
            ui.painter().text(
                egui::pos2(text_x, rect.top() + 80.0),
                Align2::LEFT_CENTER,
                "Selected on Play",
                FontId::proportional(11.0),
                p.accent,
            );
        }
        response.clicked() && !emblem.clicked()
    }

    pub(crate) fn open_instance(&mut self, id: &str) {
        self.inst.open = Some(id.to_owned());
        self.inst.page = if self
            .instance_by_id(id)
            .is_some_and(|i| i.loader.kind().is_some())
        {
            InstancePage::Mods
        } else {
            InstancePage::Settings
        };
        self.inst.confirm_delete = false;
        self.inst.rename = None;
        self.refresh_mods(id);
    }

    /// Re-read the mods folder of `id`.
    pub(crate) fn refresh_mods(&mut self, id: &str) {
        let Some(instance) = self.instance_by_id(id) else {
            return;
        };
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        let index = mods::index_path(&self.dirs.instance_dir(id));
        let files = mods::list(&mods_dir, &index).unwrap_or_default();
        self.inst.mod_files = Some((id.to_owned(), files));
    }

    /// Texture URI for an icon URL, fetching it on first use.
    pub(crate) fn icon_uri(&mut self, url: &str) -> Option<String> {
        match self.inst.icons.get(url) {
            Some(IconState::Ready(uri)) => Some(uri.clone()),
            Some(_) => None,
            None => {
                self.inst.icons.insert(url.to_owned(), IconState::Loading);
                self.tasks.mod_icon(url.to_owned());
                None
            }
        }
    }

    /// Background results for this module.
    pub(crate) fn on_instances_event(&mut self, event: Event, ctx: &egui::Context) {
        match event {
            Event::LoaderGames(kind, result) => {
                let state = match result {
                    Ok(list) => Load::Ready(list.into_iter().collect()),
                    Err(e) => Load::Failed(e),
                };
                self.inst.loader_games.insert(kind, state);
                self.create_form_defaults();
            }
            Event::LoaderVersions(kind, game, result) => {
                let state = match result {
                    Ok(list) => Load::Ready(list),
                    Err(e) => Load::Failed(e),
                };
                self.inst.loader_versions.insert((kind, game), state);
                self.create_form_defaults();
            }
            Event::ModSearch(request, result) => {
                if request == self.inst.search.request {
                    self.inst.search.loading = false;
                    self.inst.search.results = Some(result);
                }
            }
            Event::ModProgress(p) => self.inst.install_progress = Some(p),
            Event::ModInstalled(instance_id, project_id, result) => {
                self.inst.installing.remove(&project_id);
                if self.inst.installing.is_empty() {
                    self.inst.install_progress = None;
                }
                match result {
                    Ok(installed) => {
                        let names: Vec<&str> = installed.iter().map(|m| m.title.as_str()).collect();
                        let title = match names.split_first() {
                            Some((first, [])) => format!("Installed {first}"),
                            Some((first, rest)) => {
                                format!("Installed {first} + {} dependencies", rest.len())
                            }
                            None => "Already installed".to_owned(),
                        };
                        self.toasts.push(Kind::Success, title, "");
                    }
                    Err(e) => self.toasts.push(Kind::Error, "Could not install mod", e),
                }
                if self.inst.open.as_deref() == Some(instance_id.as_str()) {
                    self.refresh_mods(&instance_id);
                }
            }
            Event::ModIcon(url, result) => {
                let state = match result {
                    Ok(bytes) => {
                        let uri = icon_texture_uri(&url);
                        ctx.include_bytes(uri.clone(), bytes);
                        IconState::Ready(uri)
                    }
                    Err(_) => IconState::Failed,
                };
                self.inst.icons.insert(url, state);
            }
            _ => {}
        }
    }
}
