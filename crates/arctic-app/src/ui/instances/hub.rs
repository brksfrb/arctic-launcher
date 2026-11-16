//! Discover content: one page for Modrinth's mods, resource packs, shaders and modpacks, with
//! filters (game version, loader, category, environment), five sort orders and endless results.
//! Mods, packs and shaders install into the instance picked at the top; modpacks become a new
//! instance.

use std::collections::HashSet;
use std::sync::Arc;

use arctic_core::instances::Instance;
use arctic_core::loaders::LoaderKind;
use arctic_core::mods::discovery::{self, ShaderLoader, Tags};
use arctic_core::mods::packs::PackKind;
use arctic_core::mods::{Environment, ProjectHit, ProjectType, SearchQuery, SortBy};
use eframe::egui::{self, RichText};

use super::Load;
use super::browse::compact;
use super::mods_page::mod_icon;
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme;
use crate::widgets;
use crate::{t, tf};

const PAGE_SIZE: usize = 20;
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
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Mods,
        Section::ResourcePacks,
        Section::Shaders,
        Section::Modpacks,
    ];

    pub fn project_type(self) -> ProjectType {
        match self {
            Section::Mods => ProjectType::Mod,
            Section::ResourcePacks => ProjectType::ResourcePack,
            Section::Shaders => ProjectType::Shader,
            Section::Modpacks => ProjectType::Modpack,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Mods => t!("instances.hub.mods"),
            Section::ResourcePacks => t!("instances.hub.resource_packs"),
            Section::Shaders => t!("instances.hub.shaders"),
            Section::Modpacks => t!("instances.modpacks"),
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Section::Mods => t!("instances.search_mods_on_modrinth"),
            Section::ResourcePacks => t!("instances.search_resource_packs_on_modrinth"),
            Section::Shaders => t!("instances.search_shaders_on_modrinth"),
            Section::Modpacks => t!("instances.search_modpacks_on_modrinth"),
        }
    }

    /// Does an instance have to be picked (modpacks make their own)?
    fn needs_instance(self) -> bool {
        self != Section::Modpacks
    }

    /// Mods and modpacks care about the loader; shaders have their own (Iris...) and resource
    /// packs none.
    fn has_loader_filter(self) -> bool {
        self != Section::ResourcePacks
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
    /// A Modrinth category id; "" = any.
    pub category: String,
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
            category: String::new(),
            environment: None,
            typed_at: None,
        }
    }
}

impl Filters {
    /// Starting filters for `section` when installing into `instance`: mods fit its game version
    /// and loader; packs, shaders and modpacks start open, since few are tagged for every version.
    fn defaults(section: Section, instance: Option<&Instance>) -> Self {
        let mut filters = Self::default();
        if section == Section::Mods
            && let Some(instance) = instance
        {
            filters.game_version = instance.version.clone().unwrap_or_default();
            filters.loader = instance
                .loader
                .kind()
                .map(|l| l.modrinth_id().to_owned())
                .unwrap_or_default();
        }
        filters
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
            categories: if self.category.is_empty() {
                Vec::new()
            } else {
                vec![self.category.clone()]
            },
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

#[derive(Default)]
pub struct HubUi {
    pub section: Section,
    /// The instance mods, packs and shaders install into.
    pub target: Option<String>,
    tags: Option<Load<Arc<Tags>>>,
    filters: [Filters; 4],
    /// Which (section, target) the filters were last set up for.
    showing: Option<(Section, Option<String>)>,
}

impl ArcticApp {
    /// Open the discovery page on `section`.
    pub(crate) fn open_hub(&mut self, section: Section) {
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
        let query = self.inst.hub.filters[section.index()].query(section, offset);
        let search = &mut self.inst.search;
        search.request += 1;
        search.loading = true;
        search.more_from = offset;
        if offset == 0 {
            search.results = None;
        }
        self.tasks.mod_search(search.request, query);
    }

    /// The page's body below the title: sections, target, filters and results.
    pub(super) fn hub_body(&mut self, ui: &mut egui::Ui) {
        let section = self.inst.hub.section;
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
        // Switching section or target: set the filters up, search once.
        let key = (section, self.inst.hub.target.clone());
        if self.inst.hub.showing.as_ref() != Some(&key)
            || self.inst.search.instance.as_deref() != Some(SEARCH_OWNER)
        {
            let target = self.hub_target();
            let target_changed = self
                .inst
                .hub
                .showing
                .as_ref()
                .is_none_or(|(_, t)| *t != key.1);
            if target_changed {
                // Mods follow the instance; the other sections keep what was chosen.
                self.inst.hub.filters[Section::Mods.index()] =
                    Filters::defaults(Section::Mods, target.as_ref());
            }
            self.inst.hub.showing = Some(key.clone());
            self.inst.search.instance = Some(SEARCH_OWNER.to_owned());
            if let Some(id) = &key.1 {
                self.check_mods(id, false);
            }
            self.run_hub_search(0);
        }

        // Sections.
        ui.horizontal(|ui| {
            for s in Section::ALL {
                if ui.selectable_label(section == s, s.label()).clicked() {
                    self.inst.hub.section = s;
                }
            }
        });
        ui.add_space(6.0);
        if section.needs_instance() {
            self.hub_target_picker(ui);
            ui.add_space(6.0);
        }
        self.hub_filter_bar(ui);
        ui.add_space(8.0);
        if section == Section::Modpacks {
            self.pack_install_progress(ui);
        } else {
            self.install_bar(ui);
        }
        self.hub_results(ui);
    }

    fn hub_target_picker(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let instances = self.hub_instances();
        let current = self.hub_target();
        let mut chosen = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("instances.hub.install_into")).color(p.muted));
            egui::ComboBox::from_id_salt("hub_target")
                .width(240.0)
                .selected_text(current.as_ref().map_or("", |i| i.name.as_str()))
                .show_ui(ui, |ui| {
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
        widgets::row(ui, |ui| {
            let field = ui.add(
                widgets::text_field(&mut filters.text)
                    .hint_text(section.hint())
                    .desired_width(360.0),
            );
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
            search |= widgets::button(ui, p, None, t!("instances.search"), true).clicked();
        });
        ui.add_space(4.0);
        let before = filters.clone();
        ui.horizontal_wrapped(|ui| {
            let kind = section.project_type();
            // Game version: the instance's own first, then the releases.
            let mut versions: Vec<String> = Vec::new();
            if let Some(v) = target.as_ref().and_then(|i| i.version.clone()) {
                versions.push(v);
            }
            if let Some(tags) = &tags {
                for v in tags.releases() {
                    if !versions.iter().any(|x| x == v) {
                        versions.push(v.to_owned());
                    }
                }
            }
            option_combo(
                ui,
                "hub_game",
                &mut filters.game_version,
                t!("instances.hub.any_version"),
                versions.iter().map(|v| (v.clone(), v.clone())),
            );
            if section.has_loader_filter() {
                let loaders: Vec<(String, String)> = tags
                    .as_ref()
                    .map(|t| t.loaders_for(kind))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|l| (l.to_owned(), discovery::pretty_name(l)))
                    .collect();
                option_combo(
                    ui,
                    "hub_loader",
                    &mut filters.loader,
                    t!("instances.hub.any_loader"),
                    loaders.into_iter(),
                );
            }
            let categories: Vec<(String, String)> = tags
                .as_ref()
                .map(|t| t.categories_for(kind))
                .unwrap_or_default()
                .into_iter()
                .map(|c| (c.to_owned(), discovery::pretty_name(c)))
                .collect();
            option_combo(
                ui,
                "hub_category",
                &mut filters.category,
                t!("instances.hub.any_category"),
                categories.into_iter(),
            );
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
            let unfiltered = Filters {
                text: filters.text.clone(),
                sort: filters.sort,
                ..Filters::default()
            };
            if filters != unfiltered
                && widgets::button(ui, p, None, t!("instances.hub.clear_filters"), false).clicked()
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
            || filters.category != before.category
            || filters.environment != before.environment;
        if search {
            filters.typed_at = None;
        }
        self.inst.hub.filters[idx] = filters;
        if search {
            self.run_hub_search(0);
        }
    }

    fn hub_results(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let section = self.inst.hub.section;
        let hits: Vec<ProjectHit> = match &self.inst.search.results {
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
                ui.label(RichText::new(t!("instances.hub.nothing_matches")).color(p.muted));
                return;
            }
            Some(Ok(page)) => page.hits.clone(),
        };
        let target = self.hub_target();
        let installed = self.hub_installed(target.as_ref());
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for hit in &hits {
                if section == Section::Modpacks {
                    self.pack_row(ui, hit);
                } else if let Some(target) = &target {
                    self.hub_row(ui, section, target, hit, &installed);
                }
                ui.separator();
            }
        });
        if let Some(from) = super::more_on_scroll(ui, p, &self.inst.search) {
            self.run_hub_search(from);
        }
    }

    /// Tracked mod project ids of the target instance.
    fn hub_installed(&self, target: Option<&Instance>) -> HashSet<String> {
        target
            .and_then(|i| self.inst.mod_scans.get(&i.id))
            .and_then(|s| s.data.as_ref())
            .map(|data| {
                data.files
                    .iter()
                    .filter_map(|f| f.tracked.as_ref().map(|m| m.project_id.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What installing a shader pack into `target` needs, as a note under the row.
    pub(super) fn shader_note(&self, target: &Instance) -> Option<String> {
        let installed = self.hub_installed(Some(target));
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
        installed: &HashSet<String>,
    ) {
        let p = self.palette();
        let kind = section.pack_kind();
        let game = self.pack_game(target);
        let mut open = false;
        let vanilla_mods = section == Section::Mods && target.loader.kind().is_none();
        widgets::row(ui, |ui| {
            open |= ui
                .scope(|ui| mod_icon(ui, self, hit.icon_url.as_deref()))
                .response
                .interact(egui::Sense::click())
                .on_hover_text(t!("instances.preview_project"))
                .clicked();
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width() - 130.0);
                ui.horizontal(|ui| {
                    open |= ui
                        .add(
                            egui::Label::new(RichText::new(&hit.title).strong().color(p.text))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text(t!("instances.preview_project"))
                        .clicked();
                    ui.label(
                        RichText::new(tf!("common.by_author", author = hit.author))
                            .small()
                            .color(p.muted),
                    );
                });
                open |= ui
                    .add(
                        egui::Label::new(RichText::new(&hit.description).color(p.muted))
                            .sense(egui::Sense::click()),
                    )
                    .clicked();
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
                if section == Section::Shaders
                    && let Some(note) = self.shader_note(target)
                {
                    ui.label(RichText::new(note).small().color(p.accent));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = match kind {
                    Some(_) => self.inst.packs.installing.contains(&hit.project_id),
                    None => self.inst.installing.contains(&hit.project_id),
                };
                if kind.is_none() && installed.contains(&hit.project_id) {
                    ui.label(RichText::new(t!("instances.installed")).color(p.accent));
                } else if busy {
                    ui.spinner();
                } else {
                    let button = ui
                        .add_enabled_ui(!vanilla_mods, |ui| {
                            widgets::button(ui, p, Some(Icon::Plus), t!("instances.install"), true)
                        })
                        .inner;
                    if vanilla_mods {
                        button.on_disabled_hover_text(t!("instances.hub.vanilla_has_no_mods"));
                    } else if button.clicked() {
                        self.hub_install(section, target, hit);
                    }
                }
            });
        });
        if open {
            self.open_project_preview_in(ui.ctx(), target.id.clone(), hit.clone(), kind, false);
        }
    }

    /// Install `hit` into `target`, in the folder its project type belongs in.
    pub(super) fn hub_install(&mut self, section: Section, target: &Instance, hit: &ProjectHit) {
        match section.pack_kind() {
            Some(kind) => {
                self.inst.packs.installing.insert(hit.project_id.clone());
                let game = self.pack_game(target);
                self.tasks
                    .pack_install(target.clone(), kind, hit.project_id.clone(), game);
            }
            None => {
                self.inst.installing.insert(hit.project_id.clone());
                self.tasks
                    .mod_install(target.clone(), hit.project_id.clone(), None);
            }
        }
    }
}

/// "1.2M downloads · 3.4K follows · Optimization, Utility".
pub(super) fn stats_line(hit: &ProjectHit) -> String {
    let mut parts = vec![
        tf!("instances.downloads", compact = compact(hit.downloads)),
        tf!("instances.hub.follows", compact = compact(hit.follows)),
    ];
    let categories: Vec<String> = hit
        .categories
        .iter()
        .filter(|c| loader_kind(c).is_none())
        .take(3)
        .map(|c| discovery::pretty_name(c))
        .collect();
    if !categories.is_empty() {
        parts.push(categories.join(", "));
    }
    parts.join(" · ")
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
    egui::ComboBox::from_id_salt(id)
        .width(140.0)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), any);
            for (v, label) in options {
                ui.selectable_value(value, v, label);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters() -> Filters {
        Filters {
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            category: "optimization".into(),
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
        assert_eq!(q.categories, ["optimization"]);
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
    fn only_mods_start_filtered_by_the_instance() {
        assert_eq!(
            Filters::defaults(Section::Shaders, None),
            Filters::default()
        );
        assert_eq!(Filters::defaults(Section::Mods, None), Filters::default());
    }
}
