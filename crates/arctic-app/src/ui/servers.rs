//! The Play tab's servers: the selected instance's multiplayer list with
//! who's online, the MOTD and ping, and a Join button that starts the game
//! straight into the server.

use std::collections::HashMap;

use arctic_core::launch::QuickPlay;
use arctic_core::servers::{Server, Status};
use eframe::egui::{self, Color32, CornerRadius, RichText, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::tasks::Outcome;
use crate::toasts::Kind;
use crate::widgets;

const ICON: f32 = 40.0;
/// Ping again after this long while the Play tab is open (seconds).
const REFRESH_EVERY: f64 = 120.0;
/// Names listed inline before "+N more".
const INLINE_NAMES: usize = 4;
/// How long typing in "Direct join" must pause before the address is pinged (seconds).
const DIRECT_PAUSE: f64 = 0.7;
const GOOD_PING_MS: u32 = 100;
const OKAY_PING_MS: u32 = 250;

enum Ping {
    Waiting,
    Up(Status),
    Down(String),
}

#[derive(Default)]
pub struct ServersUi {
    /// The address typed into "Direct join".
    direct: String,
    /// The address last pinged for it, and what came back (`None` while waiting).
    direct_asked: String,
    direct_status: Option<Outcome<Status>>,
    /// When the box's text last changed (egui time): pinged once typing pauses.
    direct_changed: f64,
    /// The text `direct_changed` was set for.
    direct_asked_for: String,
    request: u64,
    /// Instance the list is for.
    instance: Option<String>,
    list: Option<Outcome<Vec<Server>>>,
    status: HashMap<String, Ping>,
    /// Image URI for each address (the live favicon, else the cached one).
    icons: HashMap<String, String>,
    refreshed_at: f64,
}

impl ArcticApp {
    pub(crate) fn servers_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let instance = self.selected_instance().clone();
        let now = ui.input(|i| i.time);
        let stale = now - self.servers.refreshed_at > REFRESH_EVERY;
        if self.servers.instance.as_ref() != Some(&instance.id) || stale {
            self.refresh_servers(now);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(REFRESH_EVERY));
        let list = match &self.servers.list {
            Some(Ok(list)) => list.clone(),
            Some(Err(e)) => {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(format!("Your server list couldn't be read: {e}"))
                        .small()
                        .color(p.muted),
                );
                return;
            }
            None => return,
        };
        ui.add_space(18.0);
        crate::widgets::row(ui, |ui| {
            ui.label(RichText::new("SERVERS").small().color(p.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = self
                    .servers
                    .status
                    .values()
                    .any(|s| matches!(s, Ping::Waiting));
                if widgets::button(ui, p, None, "Discover servers", false).clicked() {
                    self.open_discover();
                }
                if busy {
                    ui.spinner();
                } else if !list.is_empty()
                    && widgets::button(ui, p, None, "Refresh", false).clicked()
                {
                    self.refresh_servers(now);
                }
            });
        });
        ui.add_space(4.0);
        self.direct_join(ui, &instance);
        ui.add_space(6.0);
        if list.is_empty() {
            ui.label(
                RichText::new(
                    "Servers you add in Minecraft (or from Discover) show up here with who's online.",
                )
                .small()
                .color(p.muted),
            );
            return;
        }
        let mut rows = Vec::new();
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, server) in list.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                let rect = self.server_row(ui, server);
                rows.push((server.address.clone(), rect));
            }
        });
        self.finish_server_drag(ui, &rows, &instance);
    }

    /// While a server is being dragged by its grip: show where it would
    /// land, and move it there when let go.
    fn finish_server_drag(
        &mut self,
        ui: &egui::Ui,
        rows: &[(String, egui::Rect)],
        instance: &arctic_core::instances::Instance,
    ) {
        let p = self.palette();
        let Some(dragged) = egui::DragAndDrop::payload::<String>(ui.ctx()) else {
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
            return;
        };
        // It goes in front of the first row whose middle the pointer is above.
        let slot = rows.iter().position(|(_, r)| pointer.y < r.center().y);
        let y = match slot {
            Some(i) => rows[i].1.top(),
            None => last.1.bottom(),
        };
        let x = first.1.x_range();
        ui.painter().hline(x, y, egui::Stroke::new(2.0, p.accent));
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if ui.input(|i| i.pointer.any_released()) {
            let before = slot.map(|i| rows[i].0.as_str());
            if before != Some(dragged.as_str()) {
                let dir = instance.game_dir(&self.dirs);
                match arctic_core::servers::move_before(&dir, &dragged, before) {
                    Ok(_) => self.servers_changed(),
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, "Couldn't move the server", e.to_string())
                    }
                }
            }
            egui::DragAndDrop::clear_payload(ui.ctx());
        }
    }

    /// Type an address to join it now, or to keep it on the list.
    fn direct_join(&mut self, ui: &mut egui::Ui, instance: &arctic_core::instances::Instance) {
        let p = self.palette();
        let typed = self.servers.direct.trim().to_owned();
        let valid = arctic_core::servers::Address::parse(&typed).is_some();
        let now = ui.input(|i| i.time);
        if typed != self.servers.direct_asked {
            if self.servers.direct_changed == 0.0 || self.servers.direct_asked_for != typed {
                self.servers.direct_changed = now;
                self.servers.direct_asked_for = typed.clone();
            }
            // Ask once typing pauses, so half-typed names aren't looked up.
            if valid && now - self.servers.direct_changed >= DIRECT_PAUSE {
                self.servers.direct_asked = typed.clone();
                self.servers.direct_status = None;
                self.tasks.direct_ping(typed.clone());
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(DIRECT_PAUSE));
            }
        }
        let (mut join, mut add) = (false, false);
        crate::widgets::row(ui, |ui| {
            let field = ui.add(
                widgets::text_field(&mut self.servers.direct)
                    .hint_text("Direct join: play.example.net")
                    .desired_width(240.0),
            );
            join = widgets::button(ui, p, Some(Icon::Play), "Join", valid).clicked()
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && valid);
            add = widgets::button(ui, p, Some(Icon::Plus), "Add to list", false).clicked();
            if !typed.is_empty() && !valid {
                ui.label(RichText::new("Not a server address").small().color(p.muted));
            }
        });
        if valid && typed == self.servers.direct_asked {
            match &self.servers.direct_status {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Pinging…").small().color(p.muted));
                    });
                }
                Some(Ok(status)) => {
                    let text = format!(
                        "{} / {} online · {} ms · {}",
                        status.online, status.max, status.ping_ms, status.version
                    );
                    ui.label(RichText::new(text).small().color(p.text));
                    if !status.motd.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(&status.motd).small().color(p.muted))
                                .truncate(),
                        );
                    }
                }
                Some(Err(e)) => {
                    ui.label(
                        RichText::new(format!("No answer: {e}"))
                            .small()
                            .color(p.muted),
                    );
                }
            }
        }
        if !valid {
            return;
        }
        if add {
            match arctic_core::servers::add(&instance.game_dir(&self.dirs), "", &typed) {
                Ok(true) => {
                    self.toasts.push(
                        Kind::Success,
                        format!("Added {typed}"),
                        format!("It's in {}'s server list.", instance.name),
                    );
                    self.servers.direct.clear();
                    self.servers_changed();
                }
                Ok(false) => {
                    self.toasts
                        .push(Kind::Info, format!("{typed} is already on your list"), "")
                }
                Err(e) => self
                    .toasts
                    .push(Kind::Error, "Couldn't add it", e.to_string()),
            }
        }
        if join {
            self.launch_into(QuickPlay::Server(typed));
        }
    }

    /// The typed address answered: show it if it's still what's typed.
    pub(crate) fn on_direct_ping(&mut self, address: String, status: Outcome<Status>) {
        if address == self.servers.direct_asked {
            self.servers.direct_status = Some(status);
        }
    }

    /// The list changed on disk: read and ping it again.
    pub(crate) fn servers_changed(&mut self) {
        self.servers.refreshed_at = f64::NEG_INFINITY;
    }

    fn refresh_servers(&mut self, now: f64) {
        let instance = self.selected_instance().clone();
        let s = &mut self.servers;
        s.request += 1;
        s.refreshed_at = now;
        if s.instance.as_ref() != Some(&instance.id) {
            s.instance = Some(instance.id.clone());
            s.list = None;
            s.status.clear();
            s.icons.clear();
        }
        for ping in s.status.values_mut() {
            *ping = Ping::Waiting;
        }
        self.tasks
            .servers_refresh(self.servers.request, instance.game_dir(&self.dirs));
    }

    /// One row of the list; returns the space it takes.
    fn server_row(&mut self, ui: &mut egui::Ui, server: &Server) -> egui::Rect {
        let p = self.palette();
        let ping = self.servers.status.get(&server.address);
        let mut join = false;
        let row = crate::widgets::row(ui, |ui| {
            // Grip: drag the server up or down the list.
            let (grip, response) =
                ui.allocate_exact_size(vec2(14.0, ICON), egui::Sense::click_and_drag());
            for dy in [-6.0, 0.0, 6.0] {
                for dx in [-2.5, 2.5] {
                    ui.painter().circle_filled(
                        grip.center() + vec2(dx, dy),
                        1.3,
                        if response.hovered() { p.text } else { p.muted },
                    );
                }
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            response.clone().on_hover_text("Drag to reorder");
            if response.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), server.address.clone());
            }
            let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), egui::Sense::hover());
            match self.servers.icons.get(&server.address) {
                Some(uri) => {
                    egui::Image::new(uri.clone())
                        .corner_radius(CornerRadius::same(6))
                        .texture_options(egui::TextureOptions::NEAREST)
                        .paint_at(ui, rect);
                }
                None => {
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(6), p.surface_hover);
                    icons::draw(ui.painter(), Icon::Friends, rect.shrink(11.0), p.muted);
                }
            }
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 190.0).max(120.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&server.name).strong().color(p.text));
                    if server.name != server.address {
                        ui.label(RichText::new(&server.address).small().color(p.muted));
                    }
                });
                match ping {
                    Some(Ping::Up(status)) => {
                        if !status.motd.is_empty() {
                            // Colored like the in-game server list (plain text for older data).
                            let text = if status.motd_formatted.is_empty() {
                                &status.motd
                            } else {
                                &status.motd_formatted
                            };
                            let font = egui::TextStyle::Body.resolve(ui.style());
                            ui.add(
                                egui::Label::new(widgets::mc_text(text, p.muted, font)).truncate(),
                            );
                        }
                        if let Some(names) = inline_names(status) {
                            ui.add(
                                egui::Label::new(RichText::new(names).small().color(p.text))
                                    .truncate(),
                            );
                        }
                    }
                    Some(Ping::Down(reason)) => {
                        ui.label(RichText::new(reason).small().color(p.muted));
                    }
                    _ => {}
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Play), "Join", false)
                    .on_hover_text(format!("Start {} and join", server.name))
                    .clicked()
                {
                    join = true;
                }
                ui.add_space(6.0);
                self.server_state(ui, ping);
            });
        });
        if join {
            self.launch_into(QuickPlay::Server(server.address.clone()));
        }
        row.response.rect
    }

    /// Players and ping (or offline / pinging), right-aligned.
    fn server_state(&self, ui: &mut egui::Ui, ping: Option<&Ping>) {
        let p = self.palette();
        match ping {
            Some(Ping::Up(status)) => {
                ui.vertical(|ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        let players = ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                group(status.online),
                                group(status.max)
                            ))
                            .color(p.text),
                        );
                        if !status.players.is_empty() {
                            players.on_hover_text(status.players.join("\n"));
                        }
                        let (text, color) = match status.ping_ms {
                            0 => ("online".to_owned(), p.accent),
                            ms => (format!("{ms} ms"), ping_color(p, ms)),
                        };
                        ui.label(RichText::new(text).small().color(color))
                            .on_hover_text(&status.version);
                    });
                });
            }
            Some(Ping::Down(_)) => {
                ui.label(RichText::new("offline").small().color(p.error));
            }
            _ => {
                ui.spinner();
            }
        }
    }

    pub(crate) fn on_server_list(
        &mut self,
        ctx: &egui::Context,
        request: u64,
        list: Outcome<Vec<Server>>,
    ) {
        if request != self.servers.request {
            return;
        }
        if let Ok(list) = &list {
            for server in list {
                self.servers
                    .status
                    .entry(server.address.clone())
                    .or_insert(Ping::Waiting);
                if let Some(png) = &server.icon
                    && !self.servers.icons.contains_key(&server.address)
                {
                    self.set_server_icon(ctx, &server.address, png.clone());
                }
            }
            let known: std::collections::HashSet<&String> =
                list.iter().map(|s| &s.address).collect();
            self.servers.status.retain(|a, _| known.contains(a));
        }
        self.servers.list = Some(list);
    }

    pub(crate) fn on_server_status(
        &mut self,
        ctx: &egui::Context,
        request: u64,
        address: String,
        status: Outcome<Status>,
    ) {
        if request != self.servers.request {
            return;
        }
        let ping = match status {
            Ok(status) => {
                if let Some(png) = &status.icon {
                    self.set_server_icon(ctx, &address, png.clone());
                }
                Ping::Up(status)
            }
            Err(e) => Ping::Down(e),
        };
        self.servers.status.insert(address, ping);
    }

    /// One image URI per address, replaced (not added to) on each refresh.
    fn set_server_icon(&mut self, ctx: &egui::Context, address: &str, png: Vec<u8>) {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        address.hash(&mut h);
        let uri = format!("bytes://server/{:016x}.png", h.finish());
        ctx.forget_image(&uri);
        ctx.include_bytes(uri.clone(), png);
        self.servers.icons.insert(address.to_owned(), uri);
    }
}

/// "Alice, Bob, Carol, Dan +3 more" from the player sample.
fn inline_names(status: &Status) -> Option<String> {
    if status.players.is_empty() {
        return None;
    }
    let shown: Vec<&str> = status
        .players
        .iter()
        .take(INLINE_NAMES)
        .map(String::as_str)
        .collect();
    let rest = status.online.saturating_sub(shown.len() as u32);
    Some(if rest > 0 {
        format!("{} +{} more", shown.join(", "), group(rest))
    } else {
        shown.join(", ")
    })
}

fn ping_color(p: &crate::theme::Palette, ms: u32) -> Color32 {
    if ms <= GOOD_PING_MS {
        p.accent
    } else if ms <= OKAY_PING_MS {
        p.warn
    } else {
        p.error
    }
}

/// 32051 → "32,051".
fn group(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(32_051), "32,051");
        assert_eq!(group(1_200_000), "1,200,000");
    }

    #[test]
    fn names_with_the_rest_counted() {
        let status = Status {
            version: String::new(),
            protocol: 0,
            online: 7,
            max: 10,
            players: ["a", "b", "c", "d", "e"].map(String::from).to_vec(),
            motd: String::new(),
            motd_formatted: String::new(),
            icon: None,
            ping_ms: 1,
        };
        assert_eq!(inline_names(&status).unwrap(), "a, b, c, d +3 more");
    }
}
