//! Bedrock tab: Minecraft for Windows (Release and Preview). Play it,
//! switch it to another version, and look after its worlds, packs and
//! servers. Everything here works on the game's own folders.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus, Server};
use arctic_core::bedrock::versions::{self, Catalog, Version};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Player};
use eframe::egui::{self, CornerRadius, Id, Modal, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::bedrock_tasks::{BedrockEvent, BedrockScan};
use crate::motion::format_bytes;
use crate::tasks::{Outcome, ProgressSnapshot};
use crate::theme;
use crate::toasts::Kind;
use crate::widgets::{self, PlayState};

const ICON: f32 = 44.0;
/// While the tab is open, the game's state and folders are looked at again this often (seconds).
const RESCAN_EVERY: f64 = 5.0;
/// Servers are pinged again after this long (seconds).
const PING_EVERY: f64 = 120.0;
/// Versions shown before "Show all".
const VERSIONS_SHOWN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Worlds,
    Versions,
    Packs,
    Servers,
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
}

impl ArcticApp {
    pub(crate) fn bedrock_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let now = ui.input(|i| i.time);
        if !bedrock::supported() {
            widgets::page_header(ui, p, "Bedrock Edition", "");
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("Bedrock Edition is played on Windows")
                        .size(17.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(
                        "Minecraft for Windows only runs there, so it can't be started or \
                         installed from Arctic on this computer.",
                    )
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
                    "Bedrock Edition",
                    "Minecraft for Windows: play it, switch versions, and manage its worlds, packs and servers.",
                );
            });
        });
        widgets::row(ui, |ui| {
            for e in Edition::ALL {
                if ui
                    .selectable_label(edition == e, RichText::new(e.name()).size(15.0))
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
            ui.selectable_value(s, Section::Worlds, "Worlds");
            ui.selectable_value(s, Section::Packs, "Packs");
            ui.selectable_value(s, Section::Servers, "Servers");
            ui.selectable_value(s, Section::Versions, "Versions");
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
        }
        self.bedrock_confirm(ui.ctx());
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
            .on_hover_text("Each Xbox account that played here has its own worlds and servers.");
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
                    format!("Installing {}", versions::display(&i.version, edition)),
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
                        RichText::new(format!("Minecraft {}", edition.name()))
                            .size(19.0)
                            .strong()
                            .color(p.text),
                    );
                    let line = match &scan {
                        None => "Looking for the game…".to_owned(),
                        Some(BedrockScan {
                            install: Some(install),
                            running,
                            ..
                        }) => {
                            let mut text =
                                format!("Version {}", versions::display(&install.version, edition));
                            if !install.gdk {
                                text.push_str(" (old UWP build)");
                            }
                            if *running {
                                text.push_str(" · running");
                            }
                            text
                        }
                        Some(_) => "Not installed on this computer".to_owned(),
                    };
                    ui.label(RichText::new(line).color(p.muted));
                });
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| match (&scan, &installing) {
                        (_, Some((label, fraction))) => {
                            if widgets::button(ui, p, Some(Icon::Close), "Cancel", false).clicked()
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
                                "Get it from the Microsoft Store",
                                true,
                            )
                            .clicked()
                                && let Err(e) = bedrock::open_store(edition)
                            {
                                self.toasts.push(
                                    Kind::Error,
                                    "Couldn't open the Store",
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
            let client = self
                .settings
                .bedrock_client
                .as_deref()
                .and_then(bedrock::client::Client::parse);
            self.tasks.bedrock_launch(edition, client);
        }
        if cancel && let Some(i) = &self.bedrock.installing {
            i.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// The third-party client loaded into the game when it is started from here (or none).
    fn bedrock_client_card(&mut self, ui: &mut egui::Ui) {
        use bedrock::client::Client;
        let p = self.palette();
        let mut chosen = self
            .settings
            .bedrock_client
            .as_deref()
            .and_then(Client::parse);
        let before = chosen;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Client")
                    .size(17.0)
                    .strong()
                    .color(p.text),
            );
            ui.label(
                RichText::new(
                    "Bedrock has no mod loader. A client is a program loaded into the game when you press Play: it adds a HUD (FPS, CPS, keystrokes), zoom and more.",
                )
                .color(p.muted),
            );
            ui.add_space(6.0);
            ui.radio_value(&mut chosen, None, "None: the game as it is");
            for client in Client::ALL {
                ui.radio_value(&mut chosen, Some(client), client.name())
                    .on_hover_text(client.about());
            }
            if let Some(client) = chosen {
                ui.add_space(4.0);
                ui.label(RichText::new(client.about()).small().color(p.muted));
                ui.label(
                    RichText::new(format!(
                        "{} is made by someone else, not Arctic, and is downloaded from its own releases when you press Play ({}). Some servers ban players for modified clients: check a server's rules before joining with one. Needs the newer game (1.21.120 and later).",
                        client.name(),
                        client.site()
                    ))
                    .small()
                    .color(p.warn),
                );
            }
        });
        if chosen != before {
            self.settings.bedrock_client = chosen.map(|c| c.id().to_owned());
            self.persist_settings();
        }
    }

    fn bedrock_versions(&mut self, ui: &mut egui::Ui, scan: &BedrockScan) {
        let p = self.palette();
        let edition = self.bedrock.edition;
        if self.bedrock.catalog.is_none() && !self.bedrock.catalog_loading {
            self.bedrock.catalog_loading = true;
            self.tasks.bedrock_catalog();
        }
        ui.label(
            RichText::new(
                "Switch the game to any version since 1.21.120 (older or newer). Windows downloads \
                 it from Microsoft and installs it over the current one; your worlds, packs and \
                 settings stay. You need to own Minecraft on the Microsoft account signed in to the \
                 Microsoft Store. The Store may update the game again later.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        if !scan.gaming_services {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::row(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "Gaming Services isn't installed. Windows needs it to install Bedrock.",
                        )
                        .color(p.warn),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, p, Some(Icon::External), "Get Gaming Services", true)
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
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading the version list…").color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                let e = e.clone();
                widgets::row(ui, |ui| {
                    ui.label(
                        RichText::new(format!("The version list couldn't be loaded: {e}"))
                            .color(p.muted),
                    );
                    if widgets::button(ui, p, None, "Try again", false).clicked() {
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
                        ui.label(RichText::new("Latest").small().color(p.accent));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if current {
                            ui.label(RichText::new("Installed").color(p.muted));
                        } else if !busy {
                            let newer = installed
                                .as_deref()
                                .is_some_and(|v| versions::compare(&version.id, v).is_gt());
                            let label = match (&installed, newer) {
                                (None, _) => "Install",
                                (Some(_), true) => "Update",
                                (Some(_), false) => "Switch",
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
                && widgets::button(ui, p, None, &format!("Show all {}", list.len()), false)
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
                RichText::new(format!(
                    "Install {} {}?",
                    version.edition.name(),
                    version.display()
                ))
                .size(18.0)
                .strong()
                .color(p.text),
            );
            ui.add_space(6.0);
            let replaces = match &installed {
                Some(i) => format!(
                    "It replaces {} (the game closes if it's running).",
                    versions::display(&i.version, version.edition)
                ),
                None => "Minecraft must be owned on your Microsoft account.".to_owned(),
            };
            ui.label(
                RichText::new(format!(
                    "Windows downloads it from Microsoft (about 1.5 GB). {replaces} Worlds made \
                     in a newer version may not open in an older one: back them up first."
                ))
                .color(p.muted),
            );
            ui.add_space(12.0);
            widgets::row(ui, |ui| {
                if widgets::button(ui, p, None, "Install", true).clicked() {
                    go = true;
                }
                if widgets::button(ui, p, None, "Cancel", false).clicked() {
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
                "No worlds yet",
                if scan.install.is_some() {
                    "Play once and the game makes its folders; then worlds show up here."
                } else {
                    "Install the game first."
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
            ui.label(RichText::new(format!("{} worlds", list.len())).color(p.muted));
            if self.bedrock.busy {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Import), "Import .mcworld", true).clicked() {
                    self.tasks.bedrock_import_worlds(player.clone());
                    self.bedrock.busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open the worlds folder").clicked() {
                    let dir = player.worlds_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that_detached(&dir);
                }
            });
        });
        ui.label(
            RichText::new("Drop .mcworld, .mcpack or .mcaddon files here to import them.")
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        if list.is_empty() {
            self.bedrock_empty(
                ui,
                "No worlds yet",
                "Make one in the game, or import a .mcworld.",
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
                        let field = ui.add(widgets::text_field(name).desired_width(220.0));
                        field.request_focus();
                        if field.lost_focus() {
                            let name = name.clone();
                            self.bedrock.renaming = None;
                            if ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && let Err(e) = worlds::rename(world, &name)
                            {
                                self.toasts
                                    .push(Kind::Error, "Couldn't rename it", e.to_string());
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
                            .on_hover_text("Double-click to rename");
                        if label.double_clicked() {
                            self.bedrock.renaming = Some((world.path.clone(), world.name.clone()));
                        }
                    }
                }
                let detail = format!("{} · {}", ago(world.last_played), format_bytes(world.size));
                ui.label(RichText::new(detail).small().color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, "Move to trash").clicked() {
                    match worlds::trash(world, player) {
                        Ok(()) => self.toasts.push(
                            Kind::Info,
                            format!("Moved {} to trash", world.name),
                            "com.mojang\\arctic-trash",
                        ),
                        Err(e) => {
                            self.toasts
                                .push(Kind::Error, "Couldn't move the world", e.to_string())
                        }
                    }
                    changed = true;
                }
                if widgets::icon_button(
                    ui,
                    p,
                    Icon::Copy,
                    "Back up (a .mcworld in Arctic's folder)",
                )
                .clicked()
                {
                    self.tasks
                        .bedrock_backup_world(world.clone(), backups.clone());
                }
                if widgets::icon_button(ui, p, Icon::Share, "Export as .mcworld").clicked() {
                    self.tasks.bedrock_export_world(world.clone());
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open the world's folder").clicked() {
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
                "No packs yet",
                "Play once and the game makes its folders; then packs show up here.",
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
            ui.label(RichText::new(format!("{} packs", list.len())).color(p.muted));
            if self.bedrock.busy {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Import), "Import .mcpack / .mcaddon", true)
                    .clicked()
                {
                    self.tasks.bedrock_import_packs(shared.clone());
                    self.bedrock.busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open the packs folder").clicked() {
                    let _ = open::that_detached(&shared.com_mojang);
                }
            });
        });
        ui.label(
            RichText::new(
                "Packs installed here are available to every world; turn them on in a world's \
                 settings in the game.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        if list.is_empty() {
            self.bedrock_empty(
                ui,
                "No packs yet",
                "Import resource packs, behavior packs, skin packs or whole add-ons.",
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
                RichText::new(format!("{}S", kind.name().to_uppercase()))
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
                                        RichText::new(format!("v{}", pack.version))
                                            .small()
                                            .color(p.muted),
                                    );
                                }
                                if pack.development {
                                    ui.label(RichText::new("development").small().color(p.accent));
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
                            if widgets::icon_button(ui, p, Icon::Trash, "Move to trash").clicked() {
                                match packs::trash(pack, &shared) {
                                    Ok(()) => self.toasts.push(
                                        Kind::Info,
                                        format!("Moved {} to trash", pack.name),
                                        "com.mojang\\arctic-trash",
                                    ),
                                    Err(e) => self.toasts.push(
                                        Kind::Error,
                                        "Couldn't move the pack",
                                        e.to_string(),
                                    ),
                                }
                                changed = true;
                            }
                            if widgets::icon_button(ui, p, Icon::Folder, "Open the pack's folder")
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
                "No server list yet",
                "Play once and the game makes its folders; then you can add servers here.",
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
                    .hint_text("Name")
                    .desired_width(150.0),
            );
            let field = ui.add(
                widgets::text_field(&mut self.bedrock.server_address)
                    .hint_text("play.example.net:19132")
                    .desired_width(220.0),
            );
            add = widgets::button(ui, p, Some(Icon::Plus), "Add server", true).clicked()
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let waiting = self
                    .bedrock
                    .status
                    .values()
                    .any(|s| matches!(s, Ping::Waiting));
                if waiting {
                    ui.spinner();
                } else if widgets::button(ui, p, None, "Refresh", false).clicked() {
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
                RichText::new("Servers you add (here or in the game's Servers tab) show up here with who's online.")
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
                    Err(e) => self
                        .toasts
                        .push(Kind::Error, "Couldn't remove it", e.to_string()),
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
        ui.label(RichText::new("POPULAR SERVERS").small().color(p.muted));
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
                    format!("Added {}", if name.is_empty() { address } else { name }),
                    "It's in the game's Servers tab.",
                );
                self.bedrock.server_name.clear();
                self.bedrock.server_address.clear();
                self.bedrock_refresh_servers(player, now);
            }
            Ok(false) => {
                self.toasts
                    .push(Kind::Info, format!("{address} is already on your list"), "")
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Couldn't add it", e.to_string()),
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
                        let mut detail = format!("{} / {} online", s.status.online, s.status.max);
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
                    widgets::icon_button(ui, p, Icon::Trash, "Remove from the list").clicked()
                } else {
                    widgets::button(ui, p, Some(Icon::Plus), "Add", false).clicked()
                };
                ui.add_space(6.0);
                match self.bedrock.status.get(address) {
                    Some(Ping::Up(s)) => {
                        let color = match s.status.ping_ms {
                            0..=100 => p.accent,
                            101..=250 => p.warn,
                            _ => p.error,
                        };
                        ui.label(RichText::new(format!("{} ms", s.status.ping_ms)).color(color));
                    }
                    Some(Ping::Down(_)) => {
                        ui.label(RichText::new("offline").small().color(p.error));
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
                "Play Bedrock once first",
                "The game makes its folders the first time it starts.",
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
                        format!("Installed {name}"),
                        "Press Play to start it.",
                    ),
                    Err(e) if e == "cancelled" => {
                        self.toasts
                            .push(Kind::Info, format!("Stopped installing {name}"), "")
                    }
                    Err(e) => self
                        .toasts
                        .push(Kind::Error, format!("Couldn't install {name}"), e),
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
                    Err(e) => self.toasts.push(Kind::Error, "Bedrock", e),
                }
            }
        }
    }
}

/// "3 days ago"
fn ago(time: Option<std::time::SystemTime>) -> String {
    let Some(secs) = time.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()) else {
        return "never played".into();
    };
    let (n, unit) = match secs {
        0..60 => return "just now".into(),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        86_400..2_592_000 => (secs / 86_400, "day"),
        _ => (secs / 2_592_000, "month"),
    };
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
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
