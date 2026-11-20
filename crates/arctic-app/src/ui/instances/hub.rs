//! Discover content from Modrinth, with per-section filters and paginated results.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use arctic_core::instances::Instance;
use arctic_core::loaders::LoaderKind;
use arctic_core::mods::discovery::{self, ShaderLoader, Tags};
use arctic_core::mods::pack_index::{self, PackMatches};
use arctic_core::mods::packs::PackKind;
use arctic_core::mods::{Environment, ProjectHit, ProjectType, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::Load;
use super::browse::compact;
use super::content_install::{prune_selection, select_content};
use super::hub_cards::HubBanners;
use super::hub_installed::HubInstalled;
use super::mods_page::{UpdateState, mod_icon};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::content_tasks::{ContentFiles, ContentRemoval};
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};

const PAGE_SIZE: usize = 20;
const FILTER_COMBO_WIDTH: f32 = 132.0;
/// Seconds after the last keystroke before searching.
const SEARCH_DELAY: f64 = 0.35;
/// Marks the shared search state as belonging to this page.
pub(super) const SEARCH_OWNER: &str = "\0hub";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Mods,
    ResourcePacks,
    Shaders,
    Modpacks,
    DataPacks,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Mods,
        Section::ResourcePacks,
        Section::Shaders,
        Section::Modpacks,
        Section::DataPacks,
    ];

    pub fn project_type(self) -> ProjectType {
        match self {
            Section::Mods => ProjectType::Mod,
            Section::ResourcePacks => ProjectType::ResourcePack,
            Section::Shaders => ProjectType::Shader,
            Section::Modpacks => ProjectType::Modpack,
            Section::DataPacks => ProjectType::DataPack,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Mods => t!("instances.hub.mods"),
            Section::ResourcePacks => t!("instances.hub.resource_packs"),
            Section::Shaders => t!("instances.hub.shaders"),
            Section::Modpacks => t!("instances.modpacks"),
            Section::DataPacks => t!("instances.hub.data_packs"),
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Section::Mods => t!("instances.search_mods_on_modrinth"),
            Section::ResourcePacks => t!("instances.search_resource_packs_on_modrinth"),
            Section::Shaders => t!("instances.search_shaders_on_modrinth"),
            Section::Modpacks => t!("instances.search_modpacks_on_modrinth"),
            Section::DataPacks => t!("instances.hub.search_data_packs"),
        }
    }

    /// Does an instance have to be picked (modpacks make their own)?
    fn needs_instance(self) -> bool {
        self != Section::Modpacks
    }

    /// Mods and modpacks care about the loader; shaders have their own (Iris...) and resource
    /// packs none.
    fn has_loader_filter(self) -> bool {
        matches!(self, Section::Mods | Section::Modpacks | Section::Shaders)
    }

    fn has_environment_filter(self) -> bool {
        matches!(self, Section::Mods | Section::Modpacks)
    }

    fn index(self) -> usize {
        Section::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn pack_kind(self) -> Option<PackKind> {
        match self {
            Section::ResourcePacks => Some(PackKind::Resource),
            Section::Shaders => Some(PackKind::Shader),
            _ => None,
        }
    }
}

/// What a section's search is filtered and sorted by.
#[derive(Debug, Clone, PartialEq)]
pub struct Filters {
    pub text: String,
    pub sort: SortBy,
    /// "" = any.
    pub game_version: String,
    /// A Modrinth loader id; "" = any.
    pub loader: String,
    pub categories: BTreeSet<String>,
    pub environment: Option<Environment>,
    /// When the text last changed (the search runs shortly after typing stops).
    pub typed_at: Option<f64>,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            text: String::new(),
            sort: SortBy::Downloads,
            game_version: String::new(),
            loader: String::new(),
            categories: BTreeSet::new(),
            environment: None,
            typed_at: None,
        }
    }
}

impl Filters {
    /// Set the version and loader from the instance used as the browsing context.
    fn follow_instance(&mut self, section: Section, game: &str, loader: Option<LoaderKind>) {
        self.game_version = game.to_owned();
        self.loader = match (section, loader) {
            (Section::Mods | Section::Modpacks, Some(loader)) => loader.modrinth_id(),
            (Section::Shaders, Some(LoaderKind::Forge)) => "optifine",
            (Section::Shaders, _) => "iris",
            _ => "",
        }
        .to_owned();
    }

    /// The Modrinth query for one page of `section`.
    pub fn query(&self, section: Section, offset: usize) -> SearchQuery {
        let kind = loader_kind(&self.loader).filter(|_| {
            // Their own facet (Quilt also takes Fabric mods, legacy Fabric...).
            matches!(section, Section::Mods | Section::Modpacks)
        });
        SearchQuery {
            text: self.text.trim().to_owned(),
            game_version: self.game_version.trim().to_owned(),
            loader: kind,
            loaders: match kind {
                Some(_) => Vec::new(),
                None if section.has_loader_filter() && !self.loader.is_empty() => {
                    vec![self.loader.clone()]
                }
                None => Vec::new(),
            },
            sort: self.sort,
            offset,
            limit: PAGE_SIZE,
            project_type: section.project_type(),
            categories: self.categories.iter().cloned().collect(),
            environment: self
                .environment
                .filter(|_| section.has_environment_filter()),
            // A version picked on purpose also narrows packs and shaders.
            strict_version: true,
        }
    }
}

/// The mod loader a Modrinth loader id names.
fn loader_kind(id: &str) -> Option<LoaderKind> {
    LoaderKind::ALL.into_iter().find(|l| l.modrinth_id() == id)
}

pub(super) enum InstalledProjects {
    Mods(HashSet<String>),
    Packs(Arc<PackMatches>),
}

impl InstalledProjects {
    pub(super) fn contains(&self, project: &str) -> bool {
        match self {
            Self::Mods(projects) => projects.contains(project),
            Self::Packs(projects) => projects.contains_key(project),
        }
    }

    fn pack_files(&self, kind: ProjectType, project: &str) -> Option<&[std::path::PathBuf]> {
        match self {
            Self::Packs(matches) => pack_index::removable_files(matches, kind, project),
            Self::Mods(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectAction {
    Install,
    Busy,
    Installed { update: bool, remove: bool },
}

impl ProjectAction {
    fn new(installed: bool, busy: bool, update: bool, remove: bool) -> Self {
        match (busy, installed) {
            (true, _) => Self::Busy,
            (false, true) => Self::Installed { update, remove },
            (false, false) => Self::Install,
        }
    }

    pub(super) fn selectable(self) -> bool {
        self == Self::Install
    }

    fn width(self, ui: &egui::Ui) -> f32 {
        let (label, icon, remove) = match self {
            Self::Install | Self::Busy => (t!("instances.install"), true, false),
            Self::Installed {
                update: true,
                remove,
            } => (t!("nav.update"), false, remove),
            Self::Installed {
                update: false,
                remove,
            } => (t!("instances.installed"), true, remove),
        };
        widgets::button_width(ui, icon, label)
            + if remove {
                widgets::ICON_BUTTON_SIZE + ui.spacing().item_spacing.x
            } else {
                0.0
            }
    }
}

#[derive(Default)]
pub struct HubUi {
    pub section: Section,
    /// The instance supplying search defaults and the default install target.
    pub target: Option<String>,
    pub(super) banners: HubBanners,
    pub(super) installed: HubInstalled,
    removing: HashSet<(String, ProjectType, String)>,
    needs_scroll_reset: bool,
    submitted_query: Option<SearchQuery>,
    tags: Option<Load<Arc<Tags>>>,
    filters: [Filters; Section::ALL.len()],
    showing: Option<Section>,
    defaults_from: Option<(String, String, Option<LoaderKind>)>,
}

impl ArcticApp {
    /// Open the discovery page on `section`.
    pub(crate) fn open_hub(&mut self, section: Section) {
        if !self.inst.modpacks_open {
            self.inst.hub.target = Some(self.selected_instance().id.clone());
        }
        self.inst.hub.section = section;
        self.inst.modpacks_open = true;
    }

    pub(crate) fn on_hub_tags(&mut self, result: Result<Tags, String>) {
        self.inst.hub.tags = Some(match result {
            Ok(tags) => Load::Ready(Arc::new(tags)),
            Err(e) => Load::Failed(e),
        });
    }

    /// Every instance an install can go into, Vanilla first.
    fn hub_instances(&self) -> Vec<Instance> {
        std::iter::once(self.instance.clone())
            .chain(self.custom_instances.iter().cloned())
            .collect()
    }

    /// The project page over the discovery page. Its owner is the instance it installs into
    /// (modpacks install nowhere, so the Vanilla instance stands in).
    pub(super) fn project_preview_in_hub(&mut self, ctx: &egui::Context) {
        let owner = match self.inst.hub.section {
            Section::Modpacks => None,
            _ => self.hub_target(),
        }
        .unwrap_or_else(|| self.instance.clone());
        self.project_preview(ctx, &owner);
    }

    fn hub_target(&self) -> Option<Instance> {
        let id = self.inst.hub.target.as_deref()?;
        self.instance_by_id(id).cloned()
    }

    /// Search again with the current filters (`offset` 0 replaces the results, more adds a page).
    pub(super) fn run_hub_search(&mut self, offset: usize) {
        let section = self.inst.hub.section;
        let hub = &mut self.inst.hub;
        let mut query = if offset == 0 {
            let filters = &mut hub.filters[section.index()];
            filters.typed_at = None;
            let query = filters.query(section, 0);
            hub.submitted_query = Some(query.clone());
            query
        } else if let Some(query) = &hub.submitted_query {
            query.clone()
        } else {
            return;
        };
        query.offset = offset;
        let search = &mut self.inst.search;
        search.request += 1;
        search.loading = true;
        search.more_from = offset;
        search.more_failed = None;
        if offset == 0 {
            search.results = None;
            self.inst.hub.needs_scroll_reset = true;
        }
        self.tasks.mod_search(search.request, query);
    }

    /// The page's body below the title: sections, target, filters and results.
    pub(super) fn hub_body(&mut self, ui: &mut egui::Ui) {
        if self.inst.hub.tags.is_none() {
            self.inst.hub.tags = Some(Load::Loading);
            self.tasks.hub_tags();
        }
        if self
            .inst
            .hub
            .target
            .as_deref()
            .and_then(|id| self.instance_by_id(id))
            .is_none()
        {
            self.inst.hub.target = Some(self.selected_instance().id.clone());
            self.inst.hub.showing = None;
        }
        ui.horizontal_wrapped(|ui| {
            for section in Section::ALL {
                let response =
                    ui.selectable_label(self.inst.hub.section == section, section.label());
                widgets::focus_ring(ui, &response);
                if response.clicked() {
                    self.inst.hub.section = section;
                }
            }
        });
        let section = self.inst.hub.section;
        ui.add_space(6.0);
        if section.needs_instance() {
            self.hub_target_picker(ui);
            ui.add_space(6.0);
        }
        let target = self
            .hub_target()
            .unwrap_or_else(|| self.selected_instance().clone());
        let game = self.pack_game(&target);
        let loader = target.loader.kind();
        let defaults_from = (target.id.clone(), game.clone(), loader);
        let defaults_changed = self.inst.hub.defaults_from.as_ref() != Some(&defaults_from);
        let section_changed = self.inst.hub.showing != Some(section);
        let browser_changed = self.inst.search.instance.as_deref() != Some(SEARCH_OWNER);
        if defaults_changed {
            for s in Section::ALL {
                self.inst.hub.filters[s.index()].follow_instance(s, &game, loader);
            }
            self.inst.hub.defaults_from = Some(defaults_from);
        }
        let shows_mod_projects = matches!(section, Section::Mods | Section::Shaders);
        if shows_mod_projects {
            ui.ctx()
                .request_repaint_after(crate::mod_tasks::POLL_INTERVAL);
        }
        if defaults_changed || shows_mod_projects {
            self.check_mods(&target.id, false);
        }
        if defaults_changed || section_changed || browser_changed {
            self.inst.search.selected.clear();
            self.inst.hub.showing = Some(section);
            self.inst.search.instance = Some(SEARCH_OWNER.to_owned());
            self.run_hub_search(0);
        }

        self.hub_filter_bar(ui);
        ui.add_space(8.0);
        if section == Section::Modpacks {
            self.pack_install_progress(ui);
        } else {
            self.install_bar(ui);
        }
        let kind = section.project_type();
        let installed = self.hub_installed(ui.ctx(), kind, &target);
        prune_selection(&mut self.inst.search.selected, |id| installed.contains(id));
        self.content_selection_bar(ui, kind, &target);

        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("discover_results")
            .auto_shrink(false);
        if std::mem::take(&mut self.inst.hub.needs_scroll_reset) {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| self.hub_results(ui, &target, &installed));
    }

    fn hub_target_picker(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let instances = self.hub_instances();
        let current = self.hub_target();
        let mut chosen = None;
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = widgets::FIELD_HEIGHT;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t!("instances.hub.install_into")).color(p.muted));
                let combo = egui::ComboBox::from_id_salt("hub_target")
                    .width(240.0)
                    .selected_text(current.as_ref().map_or("", |i| i.name.as_str()));
                widgets::field_combo(ui, combo, |ui| {
                    for i in &instances {
                        let selected = current.as_ref().is_some_and(|c| c.id == i.id);
                        if ui.selectable_label(selected, &i.name).clicked() {
                            chosen = Some(i.id.clone());
                        }
                    }
                });
                if let Some(i) = &current {
                    ui.label(
                        RichText::new(Self::instance_subtitle(i))
                            .small()
                            .color(p.muted),
                    );
                }
            });
        });
        if let Some(id) = chosen {
            self.inst.hub.target = Some(id);
        }
    }

    fn hub_filter_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let section = self.inst.hub.section;
        let idx = section.index();
        let target = self.hub_target();
        let tags = match &self.inst.hub.tags {
            Some(Load::Ready(tags)) => Some(Arc::clone(tags)),
            _ => None,
        };
        let mut search = false;
        let mut filters = self.inst.hub.filters[idx].clone();
        ui.horizontal_wrapped(|ui| {
            let field_width = (ui.available_width() - 48.0).clamp(72.0, 360.0);
            let field = ui
                .allocate_ui_with_layout(
                    egui::vec2(field_width + 48.0, widgets::FIELD_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let field = ui.add(
                            widgets::text_field(&mut filters.text)
                                .hint_text(section.hint())
                                .desired_width(field_width),
                        );
                        widgets::focus_ring(ui, &field);
                        if !filters.text.is_empty()
                            && widgets::icon_button(
                                ui,
                                p,
                                Icon::Close,
                                t!("instances.hub.clear_search"),
                            )
                            .clicked()
                        {
                            filters.text.clear();
                            field.request_focus();
                            search = true;
                        }
                        field
                    },
                )
                .inner;
            let now = ui.input(|i| i.time);
            if field.changed() {
                filters.typed_at = Some(now);
            }
            search |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // Search shortly after typing stops, not on every key.
            if let Some(at) = filters.typed_at {
                if now - at >= SEARCH_DELAY {
                    search = true;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(
                            SEARCH_DELAY - (now - at),
                        ));
                }
            }
            let before = filters.sort;
            hub_sort_combo(ui, &mut filters.sort);
            search |= filters.sort != before;
            search |= widgets::field_button(ui, p, t!("instances.search"), true).clicked();
        });
        ui.add_space(4.0);
        let before = filters.clone();
        ui.horizontal_wrapped(|ui| {
            let kind = section.project_type();
            // Game version: the browsing instance first, then all Modrinth versions.
            let mut versions: Vec<String> = Vec::new();
            if let Some(target) = &target {
                let game = self.pack_game(target);
                if !game.is_empty() {
                    versions.push(game);
                }
            }
            if let Some(tags) = &tags {
                let current = versions.first().cloned();
                versions.extend(
                    tags.game_versions
                        .iter()
                        .filter(|tag| Some(&tag.version) != current.as_ref())
                        .map(|tag| tag.version.clone()),
                );
            }
            option_combo(
                ui,
                "hub_game",
                &mut filters.game_version,
                t!("instances.hub.any_version"),
                versions.iter().map(|v| (v.clone(), v.clone())),
            );
            if section.has_loader_filter() {
                let mut ids: BTreeSet<&str> = tags
                    .as_ref()
                    .map(|t| t.loaders_for(kind))
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                if matches!(section, Section::Mods | Section::Modpacks) {
                    ids.extend(LoaderKind::ALL.map(LoaderKind::modrinth_id));
                } else {
                    ids.extend(["iris", "optifine", "vanilla"]);
                }
                option_combo(
                    ui,
                    "hub_loader",
                    &mut filters.loader,
                    t!("instances.hub.any_loader"),
                    ids.into_iter().map(|id| (id.to_owned(), loader_label(id))),
                );
            }
            if section.has_environment_filter() {
                let mut env = match filters.environment {
                    None => String::new(),
                    Some(Environment::Client) => "client".to_owned(),
                    Some(Environment::Server) => "server".to_owned(),
                };
                option_combo(
                    ui,
                    "hub_env",
                    &mut env,
                    t!("instances.hub.any_environment"),
                    [
                        ("client".to_owned(), t!("instances.hub.client").to_owned()),
                        ("server".to_owned(), t!("instances.hub.server").to_owned()),
                    ]
                    .into_iter(),
                );
                filters.environment = match env.as_str() {
                    "client" => Some(Environment::Client),
                    "server" => Some(Environment::Server),
                    _ => None,
                };
            }
            let any_category = t!("instances.hub.any_category");
            let selected_category = filters
                .categories
                .first()
                .map_or_else(|| any_category.to_owned(), |id| discovery::pretty_name(id));
            let categories = tags
                .as_ref()
                .map_or_else(Vec::new, |tags| tags.categories_for(kind));
            let combo = egui::ComboBox::from_id_salt("hub_category")
                .width(FILTER_COMBO_WIDTH)
                .selected_text(selected_category);
            widgets::field_combo(ui, combo, |ui| {
                if ui
                    .selectable_label(filters.categories.is_empty(), any_category)
                    .clicked()
                {
                    filters.categories.clear();
                }
                for category in categories {
                    if ui
                        .selectable_label(
                            filters.categories.contains(category),
                            discovery::pretty_name(category),
                        )
                        .clicked()
                    {
                        filters.categories.clear();
                        filters.categories.insert(category.to_owned());
                    }
                }
            })
            .on_hover_text(t!("instances.hub.category"));
            let unfiltered = Filters {
                text: filters.text.clone(),
                sort: filters.sort,
                ..Filters::default()
            };
            if filters != unfiltered
                && widgets::field_button(ui, p, t!("instances.hub.clear_filters"), false).clicked()
            {
                filters = unfiltered;
            }
        });
        if let Some(Load::Failed(e)) = &self.inst.hub.tags {
            ui.label(
                RichText::new(tf!("instances.hub.filters_failed", e = e))
                    .small()
                    .color(p.error),
            );
        }
        search |= filters.game_version != before.game_version
            || filters.loader != before.loader
            || filters.categories != before.categories
            || filters.environment != before.environment;
        if search {
            filters.typed_at = None;
        }
        self.inst.hub.filters[idx] = filters;
        if search {
            self.run_hub_search(0);
        }
    }

    fn hub_results(&mut self, ui: &mut egui::Ui, target: &Instance, installed: &InstalledProjects) {
        let p = self.palette();
        let section = self.inst.hub.section;
        let hits = match &self.inst.search.results {
            Some(Ok(page)) if !page.hits.is_empty() => page.hits.clone(),
            result => {
                let error = result.as_ref().and_then(|r| r.as_ref().err()).cloned();
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    if let Some(error) = error {
                        ui.colored_label(p.error, tf!("instances.search_failed", e = error));
                        ui.add_space(8.0);
                        if widgets::field_button(ui, p, t!("instances.retry"), true).clicked() {
                            self.run_hub_search(0);
                        }
                    } else if self.inst.search.loading {
                        ui.spinner();
                        ui.label(RichText::new(t!("instances.searching_modrinth")).color(p.muted));
                    } else {
                        ui.label(RichText::new(t!("instances.hub.nothing_matches")).color(p.muted));
                    }
                });
                return;
            }
        };
        if let Some(kind) = section.pack_kind() {
            self.hub_card_grid(ui, kind, target, &hits, installed);
        } else {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (index, hit) in hits.iter().enumerate() {
                    if index > 0 {
                        ui.separator();
                    }
                    ui.push_id(&hit.project_id, |ui| {
                        if section == Section::Modpacks {
                            self.pack_row(ui, hit);
                        } else {
                            self.hub_row(ui, section, target, hit, installed);
                        }
                    });
                }
            });
        }
        ui.add_space(8.0);
        if let Some(from) = super::more_on_scroll(ui, p, &self.inst.search) {
            self.run_hub_search(from);
        } else if matches!(&self.inst.search.results, Some(Ok(page)) if self.inst.search.loaded >= page.total)
        {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(t!("instances.hub.end_of_results"))
                        .small()
                        .color(p.muted),
                );
            });
        }
    }

    pub(super) fn hub_installed(
        &mut self,
        ctx: &egui::Context,
        kind: ProjectType,
        target: &Instance,
    ) -> InstalledProjects {
        match kind {
            ProjectType::Mod => InstalledProjects::Mods(self.hub_mod_projects(target)),
            ProjectType::ResourcePack | ProjectType::Shader | ProjectType::DataPack => {
                InstalledProjects::Packs(self.inst.hub.installed.packs(
                    ctx,
                    kind,
                    target.game_dir(&self.dirs),
                ))
            }
            ProjectType::Modpack => InstalledProjects::Mods(HashSet::new()),
        }
    }

    fn hub_mod_projects(&self, target: &Instance) -> HashSet<String> {
        self.inst
            .mod_scans
            .get(&target.id)
            .map(crate::mod_tasks::ModScan::tracked_projects)
            .unwrap_or_default()
    }

    /// What installing a shader pack into `target` needs, as a note under the row.
    pub(super) fn shader_note(&self, target: &Instance) -> Option<String> {
        let installed = self.hub_mod_projects(target);
        match discovery::shader_loader(target.loader.kind(), &installed) {
            ShaderLoader::Present => None,
            ShaderLoader::Switch => Some(t!("instances.hub.shaders_switched_on").to_owned()),
            ShaderLoader::Missing { name, .. } => {
                Some(tf!("instances.hub.shader_loader_missing", name = name))
            }
        }
    }

    fn hub_row(
        &mut self,
        ui: &mut egui::Ui,
        section: Section,
        target: &Instance,
        hit: &ProjectHit,
        installed: &InstalledProjects,
    ) {
        let p = self.palette();
        let game = self.pack_game(target);
        let kind = section.project_type();
        let action = self.hub_action(kind, target, &hit.project_id, installed);
        let width = action.width(ui);
        let stack = ui.available_width() < width + 260.0;
        let mut open = false;
        ui.horizontal(|ui| {
            select_content(
                ui,
                p,
                &mut self.inst.search.selected,
                hit,
                action.selectable(),
            );
            open |= ui
                .scope(|ui| mod_icon(ui, self, hit.icon_url.as_deref()))
                .response
                .interact(egui::Sense::CLICK)
                .on_hover_text(t!("instances.preview_project"))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
            let gap = ui.spacing().item_spacing.x;
            let text_width = if stack {
                ui.available_width()
            } else {
                ui.available_width() - width - gap
            };
            ui.vertical(|ui| {
                ui.set_width(text_width.max(1.0));
                ui.horizontal_wrapped(|ui| {
                    let title = ui
                        .add(
                            egui::Label::new(RichText::new(&hit.title).strong().color(p.text))
                                .wrap()
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(t!("instances.preview_project"));
                    widgets::focus_ring(ui, &title);
                    open |= title.clicked();
                    ui.label(
                        RichText::new(tf!("common.by_author", author = hit.author))
                            .small()
                            .color(p.muted),
                    );
                });
                ui.label(RichText::new(&hit.description).color(p.muted));
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
                ui.label(RichText::new(stats_line(hit)).small().color(p.muted));
            });
            if !stack {
                self.hub_install_button(ui, kind, target, hit, action);
            }
        });
        if stack {
            ui.horizontal_wrapped(|ui| {
                self.hub_install_button(ui, kind, target, hit, action);
            });
        }
        if open {
            self.open_discover_project(ui.ctx(), target, hit, kind, None);
        }
    }

    pub(super) fn hub_content_busy(
        &self,
        kind: ProjectType,
        target: &Instance,
        project: &str,
    ) -> bool {
        self.inst.content.installing(&target.id, kind, project)
            || self
                .inst
                .hub
                .removing
                .contains(&(target.id.clone(), kind, project.to_owned()))
            || match kind {
                ProjectType::Mod => {
                    self.inst.installing.contains(project)
                        || matches!(self.inst.mod_updates.get(&target.id),
                        Some(UpdateState::Applying { projects, .. }) if projects.iter().any(|id| id == project))
                }
                ProjectType::ResourcePack | ProjectType::Shader => {
                    self.inst.packs.installing.contains(project)
                }
                ProjectType::Modpack => self.inst.pack_installing.as_deref() == Some(project),
                ProjectType::DataPack => false,
            }
    }

    pub(super) fn hub_action(
        &self,
        kind: ProjectType,
        target: &Instance,
        project: &str,
        installed: &InstalledProjects,
    ) -> ProjectAction {
        let is_installed = installed.contains(project);
        let files = if kind == ProjectType::Mod && is_installed {
            self.hub_mod_files(target, project)
        } else {
            Vec::new()
        };
        let update = matches!(self.inst.mod_updates.get(&target.id),
            Some(UpdateState::Ready(updates)) if updates.iter().any(|u| u.project_id == project
                && files.iter().any(|f| f.file_name == u.file_name && f.enabled
                    && f.tracked.as_ref().is_some_and(|m| m.version_number == u.current))));
        let remove = !files.is_empty() || installed.pack_files(kind, project).is_some();
        ProjectAction::new(
            is_installed,
            self.hub_content_busy(kind, target, project),
            update,
            remove,
        )
    }

    fn hub_mod_files<'a>(
        &'a self,
        target: &Instance,
        project: &str,
    ) -> Vec<&'a arctic_core::mods::ModFile> {
        self.inst
            .mod_scans
            .get(&target.id)
            .and_then(|scan| scan.data.as_ref())
            .into_iter()
            .flat_map(|data| &data.files)
            .filter(|file| {
                file.tracked
                    .as_ref()
                    .is_some_and(|mod_| mod_.project_id == project)
            })
            .collect()
    }

    pub(super) fn hub_install_button(
        &mut self,
        ui: &mut egui::Ui,
        kind: ProjectType,
        target: &Instance,
        hit: &ProjectHit,
        action: ProjectAction,
    ) {
        let p = self.palette();
        match action {
            ProjectAction::Busy => {
                let size = egui::vec2(action.width(ui), widgets::BUTTON_HEIGHT);
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(rect, 9, p.surface);
                let spinner = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(18.0));
                egui::Spinner::new().paint_at(ui, spinner);
                response.on_hover_text(t!("instances.installing"));
            }
            ProjectAction::Install => {
                if widgets::button(ui, p, Some(Icon::Plus), t!("instances.install"), true).clicked()
                {
                    self.pick_single_content(ui.ctx(), kind, hit, None, target);
                }
            }
            ProjectAction::Installed { update, remove } => {
                if update {
                    if widgets::button(ui, p, None, t!("nav.update"), true).clicked() {
                        self.apply_mod_updates(target, vec![hit.project_id.clone()]);
                    }
                } else {
                    ui.add_enabled_ui(false, |ui| {
                        widgets::button(ui, p, Some(Icon::Check), t!("instances.installed"), false);
                    });
                }
                if remove
                    && widgets::icon_button(ui, p, Icon::Trash, t!("instances.remove")).clicked()
                {
                    self.remove_hub_content(ui.ctx(), kind, target, &hit.project_id);
                }
            }
        }
    }

    fn remove_hub_content(
        &mut self,
        ctx: &egui::Context,
        kind: ProjectType,
        target: &Instance,
        project: &str,
    ) {
        let files = if kind == ProjectType::Mod {
            ContentFiles::Mods(
                self.hub_mod_files(target, project)
                    .into_iter()
                    .map(|f| f.file_name.clone())
                    .collect(),
            )
        } else {
            let installed = self.hub_installed(ctx, kind, target);
            let Some(files) = installed.pack_files(kind, project) else {
                return;
            };
            ContentFiles::Packs {
                expected: files.to_vec(),
                known: self.inst.hub.installed.known_files(kind),
            }
        };
        self.inst
            .hub
            .removing
            .insert((target.id.clone(), kind, project.to_owned()));
        self.tasks.remove_discover_content(
            target.clone(),
            kind,
            self.pack_game(target),
            project.to_owned(),
            files,
        );
    }

    pub(super) fn on_hub_content_removed(&mut self, removal: ContentRemoval) {
        let ContentRemoval {
            target,
            kind,
            project,
            result,
        } = removal;
        if !self
            .inst
            .hub
            .removing
            .remove(&(target.clone(), kind, project))
        {
            return;
        }
        if let Err(error) = result {
            self.toasts
                .push(Kind::Error, t!("instances.could_not_delete_it"), error);
        }
        if kind == ProjectType::Mod {
            self.refresh_mods(&target);
        } else if let Some(instance) = self.instance_by_id(&target).cloned() {
            let dir = instance.game_dir(&self.dirs);
            self.inst.hub.installed.refresh(kind, &dir);
            if self
                .inst
                .packs
                .showing
                .as_ref()
                .is_some_and(|(id, _, _)| id == &target)
            {
                self.refresh_packs(&instance);
            }
        }
    }
}

/// "1.2M downloads · 3.4K follows · Optimization, Utility".
pub(super) fn stats_line(hit: &ProjectHit) -> String {
    let counts = stats_counts(hit);
    let tags = project_tags(hit);
    if tags.is_empty() {
        counts
    } else {
        format!("{counts} · {tags}")
    }
}

pub(super) fn stats_counts(hit: &ProjectHit) -> String {
    format!(
        "{} · {}",
        tf!("instances.downloads", compact = compact(hit.downloads)),
        tf!("instances.hub.follows", compact = compact(hit.follows)),
    )
}

pub(super) fn project_tags(hit: &ProjectHit) -> String {
    hit.categories
        .iter()
        .filter(|c| loader_kind(c).is_none())
        .take(3)
        .map(|c| discovery::pretty_name(c))
        .collect::<Vec<_>>()
        .join(", ")
}

fn loader_label(id: &str) -> String {
    match id {
        "iris" => "Iris".into(),
        "optifine" => "OptiFine".into(),
        "vanilla" => t!("instances.vanilla").into(),
        _ => loader_kind(id).map_or_else(
            || discovery::pretty_name(id),
            |loader| loader.label().to_owned(),
        ),
    }
}

/// Sort dropdown, as tall as the search field next to it.
fn hub_sort_combo(ui: &mut egui::Ui, sort: &mut SortBy) {
    super::browse::sort_combo(ui, "hub_sort", sort);
}

/// A drop-down of `options` (value, label) with an "any" entry that sets the value to "".
fn option_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    any: &str,
    options: impl Iterator<Item = (String, String)>,
) {
    let options: Vec<(String, String)> = options.collect();
    let shown = options.iter().find(|(v, _)| v == value).map_or_else(
        || {
            if value.is_empty() {
                any.to_owned()
            } else {
                value.clone()
            }
        },
        |(_, l)| l.clone(),
    );
    let combo = egui::ComboBox::from_id_salt(id)
        .width(FILTER_COMBO_WIDTH)
        .selected_text(shown);
    widgets::field_combo(ui, combo, |ui| {
        ui.selectable_value(value, String::new(), any);
        for (v, label) in options {
            ui.selectable_value(value, v, label);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_projects_replace_install_and_cannot_be_selected() {
        assert_eq!(
            ProjectAction::new(false, false, false, false),
            ProjectAction::Install
        );
        assert!(ProjectAction::Install.selectable());
        for (update, remove) in [(false, false), (true, false), (false, true), (true, true)] {
            let action = ProjectAction::new(true, false, update, remove);
            assert_eq!(action, ProjectAction::Installed { update, remove });
            assert!(!action.selectable());
        }
        assert_eq!(
            ProjectAction::new(true, true, true, true),
            ProjectAction::Busy
        );
        assert!(!ProjectAction::Busy.selectable());
    }

    fn filters() -> Filters {
        Filters {
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            categories: ["optimization".into(), "utility".into()].into(),
            environment: Some(Environment::Client),
            sort: SortBy::Follows,
            text: "  sodium ".into(),
            ..Filters::default()
        }
    }

    #[test]
    fn mod_query_uses_the_loader_facet_with_its_compat_set() {
        let q = filters().query(Section::Mods, 40);
        assert_eq!(q.project_type, ProjectType::Mod);
        assert_eq!(q.loader, Some(LoaderKind::Fabric));
        assert!(q.loaders.is_empty());
        assert_eq!(q.text, "sodium");
        assert_eq!(q.game_version, "1.21.1");
        assert_eq!(q.categories, ["optimization", "utility"]);
        assert_eq!(q.environment, Some(Environment::Client));
        assert_eq!(
            (q.offset, q.limit, q.sort),
            (40, PAGE_SIZE, SortBy::Follows)
        );
    }

    #[test]
    fn shader_query_names_its_loader_and_ignores_environment() {
        let mut f = filters();
        f.loader = "iris".into();
        let q = f.query(Section::Shaders, 0);
        assert_eq!(q.project_type, ProjectType::Shader);
        assert_eq!(q.loader, None);
        assert_eq!(q.loaders, ["iris"]);
        assert_eq!(q.environment, None);
        assert!(q.strict_version);
    }

    #[test]
    fn resource_packs_have_no_loader_filter() {
        let q = filters().query(Section::ResourcePacks, 0);
        assert_eq!(q.loader, None);
        assert!(q.loaders.is_empty());
        assert_eq!(q.project_type, ProjectType::ResourcePack);
    }

    #[test]
    fn empty_filters_search_everything() {
        let q = Filters::default().query(Section::Modpacks, 0);
        assert!(q.game_version.is_empty() && q.categories.is_empty());
        assert_eq!((q.loader, q.environment), (None, None));
    }

    #[test]
    fn browsing_context_supplies_defaults_and_data_pack_query_uses_its_own_type() {
        let mut f = Filters::default();
        f.follow_instance(Section::Mods, "1.21.4", Some(LoaderKind::NeoForge));
        assert_eq!(f.query(Section::Mods, 0).loader, Some(LoaderKind::NeoForge));
        f.follow_instance(Section::Shaders, "1.21.4", None);
        assert_eq!(f.query(Section::Shaders, 0).loaders, ["iris"]);
        f.follow_instance(Section::DataPacks, "1.21.4", Some(LoaderKind::Fabric));
        let q = f.query(Section::DataPacks, 0);
        assert_eq!(q.project_type, ProjectType::DataPack);
        assert_eq!(q.game_version, "1.21.4");
        assert!(q.loader.is_none() && q.loaders.is_empty());
    }
}
