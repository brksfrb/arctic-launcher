//! The Play tab's servers: the selected instance's multiplayer list with
//! who's online, the MOTD and ping, and a Join button that starts the game
//! straight into the server.

use std::collections::HashMap;

use arctic_core::launch::QuickPlay;
use arctic_core::servers::{Server, Status};
use eframe::egui::{self, Color32, CornerRadius, RichText, emath::TSTransform, pos2, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::tasks::Outcome;
use crate::toasts::Kind;
use crate::widgets;

const ICON: f32 = 40.0;
/// The held row's frame around its content.
const HELD_PAD: f32 = 8.0;
/// Ping again after this long while the Play tab is open (seconds).
const REFRESH_EVERY: f64 = 120.0;
/// Names listed inline before "+N more".
const INLINE_NAMES: usize = 4;
/// How long typing in "Direct join" must pause before the address is pinged (seconds).
const DIRECT_PAUSE: f64 = 0.7;
/// Space between rows (a line is drawn in its middle).
const ROW_GAP: f32 = 12.0;
/// Seconds the other rows take to glide aside, and a dropped row to settle.
const SLIDE: f32 = 0.14;
/// Near the page's top or bottom edge (points), a dragged row scrolls it.
const EDGE: f32 = 48.0;
/// Fastest edge scroll, points a second.
const EDGE_SCROLL: f32 = 900.0;
const GOOD_PING_MS: u32 = 100;
const OKAY_PING_MS: u32 = 250;

enum Ping {
    Waiting,
    Up(Status),
    Down(String),
}

/// A server picked up to move: which row, where on it it was held, how tall it is.
#[derive(Clone)]
struct Drag {
    index: usize,
    address: String,
    grab_dy: f32,
    height: f32,
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
    /// The row being dragged.
    drag: Option<Drag>,
    /// Each row's place last frame (where a dragged row would land is worked out from them).
    row_rects: Vec<egui::Rect>,
    /// A row just dropped: its address and where it was last drawn (it glides into place).
    settle: Option<(String, f32)>,
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
        self.server_list(ui, &list, &instance);
    }

    /// The rows. A row picked up (anywhere but its buttons) follows the pointer, lifted; the
    /// others glide aside to show where it will land; near the page's edges it scrolls; let go,
    /// it settles into its new place.
    fn server_list(
        &mut self,
        ui: &mut egui::Ui,
        list: &[Server],
        instance: &arctic_core::instances::Instance,
    ) {
        let p = self.palette();
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let before = std::mem::take(&mut self.servers.row_rects);
        let drag = self
            .servers
            .drag
            .clone()
            .filter(|d| d.index < list.len() && list[d.index].address == d.address);
        if drag.is_none() {
            self.servers.drag = None;
        }
        // Where the held row would land: past every other row whose middle is above its own.
        let target = match (&drag, pointer) {
            (Some(d), Some(at)) if before.len() == list.len() => {
                let middle = at.y - d.grab_dy + d.height / 2.0;
                let above = (0..list.len())
                    .filter(|&k| k != d.index && before[k].center().y < middle)
                    .count();
                Some(above)
            }
            _ => drag.as_ref().map(|d| d.index),
        };
        let shift = drag.as_ref().map_or(0.0, |d| d.height + ROW_GAP);
        let settle = self.servers.settle.take();
        let mut rects = Vec::with_capacity(list.len());
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, server) in list.iter().enumerate() {
                if i > 0 {
                    ui.add_space(ROW_GAP);
                }
                let held = drag.as_ref().is_some_and(|d| d.index == i);
                let goal = match (&drag, target) {
                    (Some(d), Some(t)) if d.index < i && i <= t => -shift,
                    (Some(d), Some(t)) if t <= i && i < d.index => shift,
                    _ => 0.0,
                };
                let id = ui.id().with(("server_row", i, &server.address));
                let first_shape = ui.ctx().graphics(|g| {
                    g.get(ui.layer_id())
                        .map_or(egui::layers::ShapeIdx(0), |l| l.next_idx())
                });
                let top = ui.cursor().top();
                if i > 0 {
                    let y = top - ROW_GAP / 2.0;
                    ui.painter().hline(
                        ui.max_rect().x_range(),
                        y,
                        egui::Stroke::new(1.0, p.card_stroke),
                    );
                }
                let rect = if held {
                    // Its place stays (the held copy is drawn above everything).
                    ui.scope(|ui| {
                        ui.set_invisible();
                        self.server_row(ui, server, None)
                    })
                    .inner
                } else {
                    self.server_row(ui, server, Some(i))
                };
                // A row just dropped starts from where it was let go.
                if let Some((address, from)) = &settle
                    && *address == server.address
                {
                    ui.ctx().animate_value_with_time(id, from - rect.top(), 0.0);
                }
                let offset = ui.ctx().animate_value_with_time(id, goal, SLIDE);
                if offset != 0.0 {
                    ui.ctx().graphics_mut(|g| {
                        let list = g.entry(ui.layer_id());
                        let end = list.next_idx();
                        list.transform_range(
                            first_shape,
                            end,
                            TSTransform::from_translation(vec2(0.0, offset)),
                        );
                    });
                }
                rects.push(rect);
            }
        });
        self.servers.row_rects = rects;
        let Some(d) = drag else {
            return;
        };
        let Some(at) = pointer else {
            return;
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let width = self.servers.row_rects.first().map_or(0.0, |r| r.width());
        let left = self.servers.row_rects.first().map_or(0.0, |r| r.left());
        let top = at.y - d.grab_dy;
        egui::Area::new(ui.id().with("server_held"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(left - HELD_PAD, top - HELD_PAD))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0, p.accent))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(HELD_PAD)
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(120),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width);
                        self.server_row(ui, &list[d.index], None);
                    });
            });
        // Near the page's edges the list scrolls under it.
        let clip = ui.clip_rect();
        let near_top = (clip.top() + EDGE - at.y).max(0.0) / EDGE;
        let near_bottom = (at.y - (clip.bottom() - EDGE)).max(0.0) / EDGE;
        let speed = (near_top - near_bottom).clamp(-1.0, 1.0) * EDGE_SCROLL * dt;
        if speed != 0.0 {
            ui.scroll_with_delta(vec2(0.0, speed));
        }
        ui.ctx().request_repaint();
        if ui.input(|i| !i.pointer.primary_down()) {
            self.servers.drag = None;
            let to = target.unwrap_or(d.index);
            if to != d.index {
                let mut order = list.to_vec();
                let moved = order.remove(d.index);
                order.insert(to, moved);
                let after = order.get(to + 1).map(|s| s.address.clone());
                let dir = instance.game_dir(&self.dirs);
                match arctic_core::servers::move_before(&dir, &d.address, after.as_deref()) {
                    Ok(_) => {
                        // Shown in its new place now; the file is read again in a moment.
                        self.servers.list = Some(Ok(order));
                        self.servers_changed();
                    }
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, "Couldn't move the server", e.to_string())
                    }
                }
            }
            self.servers.settle = Some((d.address, top));
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

    /// One row of the list; returns the space it takes. `index`: its place in the list, for
    /// picking it up (None: a copy that can't be, like the one being held).
    fn server_row(&mut self, ui: &mut egui::Ui, server: &Server, index: Option<usize>) -> egui::Rect {
        let p = self.palette();
        let ping = self.servers.status.get(&server.address);
        let mut join = false;
        let row = crate::widgets::row(ui, |ui| {
            // Grip: a hint that the row can be picked up (anywhere on it).
            let (grip, _) = ui.allocate_exact_size(vec2(14.0, ICON), egui::Sense::hover());
            let lit = index.is_some_and(|i| self.servers.row_rects.get(i).is_some_and(|r| ui.rect_contains_pointer(*r)));
            for dy in [-6.0, 0.0, 6.0] {
                for dx in [-2.5, 2.5] {
                    ui.painter().circle_filled(
                        grip.center() + vec2(dx, dy),
                        1.3,
                        if lit { p.text } else { p.muted },
                    );
                }
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
        let rect = row.response.rect;
        if let Some(index) = index {
            // Over the row: only dragging (its buttons still take clicks).
            let hold = ui
                .interact(rect, ui.id().with(("server_hold", index)), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if hold.drag_started()
                && self.servers.drag.is_none()
                && let Some(at) = hold.interact_pointer_pos()
            {
                self.servers.drag = Some(Drag {
                    index,
                    address: server.address.clone(),
                    grab_dy: at.y - rect.top(),
                    height: rect.height(),
                });
            }
        }
        rect
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
