//! "Discover servers": the public server list in a new random order each
//! time (nothing paid or pinned), filtered by premium / cracked, with a
//! "Surprise me" pick; plus the form owners use to list their own server.

use arctic_core::launch::QuickPlay;
use arctic_core::servers::public::{self, Access, PublicServer, Submitted};
use eframe::egui::{self, Id, Modal, RichText, Stroke, vec2};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::server_tasks::SubmitForm;
use crate::tasks::Outcome;
use crate::toasts::Kind;
use crate::widgets;

const WIDTH: f32 = 640.0;
const LIST_HEIGHT: f32 = 380.0;
const MAX_TAGS: usize = 5;

#[derive(Default)]
pub struct DiscoverUi {
    open: bool,
    list: Option<Outcome<Vec<PublicServer>>>,
    access: Access,
    search: String,
    /// "Surprise me" result, shown first.
    picked: Option<String>,
    form: Option<ListForm>,
}

#[derive(Default)]
struct ListForm {
    fields: SubmitForm,
    tags: String,
    busy: bool,
    submitted: Option<Submitted>,
    /// What the last step said (error or state).
    message: Option<(bool, String)>,
}

impl ArcticApp {
    pub(crate) fn open_discover(&mut self) {
        self.discover.open = true;
        self.discover.picked = None;
        self.discover.list = None;
        self.tasks.public_servers();
    }

    pub(crate) fn discover_dialog(&mut self, ctx: &egui::Context) {
        if !self.discover.open {
            return;
        }
        let p = self.palette();
        let mut close = false;
        let modal = Modal::new(Id::new("discover_servers"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                if self.discover.form.is_some() {
                    self.list_form(ui);
                } else {
                    self.discover_list(ui);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if self.discover.form.is_some()
                        && widgets::button(ui, p, Some(Icon::ChevronLeft), "Back", false).clicked()
                    {
                        self.discover.form = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close |= widgets::button(ui, p, None, "Close", false).clicked();
                    });
                });
            });
        if close || modal.should_close() {
            self.discover.open = false;
            self.discover.form = None;
        }
    }

    fn discover_list(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Discover servers")
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new("A new random order every time. Nobody pays for a spot.")
                        .color(p.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if widgets::button(ui, p, Some(Icon::Plus), "List your server", false).clicked() {
                    self.discover.form = Some(ListForm::default());
                }
            });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (access, label) in [
                (Access::Any, "All"),
                (Access::Premium, "Premium"),
                (Access::Cracked, "Cracked"),
                (Access::Partners, "Partners"),
            ] {
                let tip = match access {
                    Access::Any => "Every server",
                    Access::Premium => "Need a Microsoft account",
                    Access::Cracked => "Let any name in",
                    Access::Partners => "Servers hosted with Flash Hosting, and ours",
                };
                if ui
                    .selectable_label(self.discover.access == access, label)
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.discover.access = access;
                    self.discover.picked = None;
                }
            }
            ui.add_space(8.0);
            ui.add(
                widgets::text_field(&mut self.discover.search)
                    .hint_text("Search names and tags")
                    .desired_width(200.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, None, "Surprise me", true).clicked() {
                    self.surprise();
                }
            });
        });
        ui.add_space(8.0);
        let list = match &self.discover.list {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Getting the list…").color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                ui.label(RichText::new(format!("The list couldn't be loaded: {e}")).color(p.error));
                return;
            }
            Some(Ok(list)) => list.clone(),
        };
        let search = self.discover.search.trim().to_lowercase();
        let mut shown: Vec<&PublicServer> = list
            .iter()
            .filter(|s| self.discover.access.allows(s))
            .filter(|s| {
                search.is_empty()
                    || s.name.to_lowercase().contains(&search)
                    || s.tags.iter().any(|t| t.contains(&search))
            })
            .collect();
        if let Some(picked) = &self.discover.picked
            && let Some(i) = shown.iter().position(|s| &s.id == picked)
        {
            let s = shown.remove(i);
            shown.insert(0, s);
        }
        if shown.is_empty() {
            // The same room as a full list, so the window doesn't jump.
            ui.allocate_ui(vec2(ui.available_width(), LIST_HEIGHT), |ui| {
                ui.label(RichText::new("No servers match.").color(p.muted));
            });
            return;
        }
        egui::ScrollArea::vertical()
            .min_scrolled_height(LIST_HEIGHT)
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Our own servers sit on top of every list; the Partners tab holds
                // those and the servers hosted with Flash Hosting.
                let partners_tab = self.discover.access == Access::Partners;
                let (partners, others): (Vec<&PublicServer>, Vec<&PublicServer>) = shown
                    .into_iter()
                    .partition(|s| partners_tab || s.partner_tier == "exclusive");
                let mut partners = partners;
                partners.sort_by_key(|s| s.partner_tier != "exclusive");
                let top = if partners_tab {
                    "Partners"
                } else {
                    "Exclusive partners"
                };
                for (title, group) in [(top, partners), ("All servers", others)] {
                    if group.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(title).small().strong().color(p.muted));
                    ui.add_space(4.0);
                    for s in group {
                        let picked = self.discover.picked.as_ref() == Some(&s.id);
                        self.public_row(ui, s, picked);
                        ui.add_space(6.0);
                    }
                }
            });
    }

    fn surprise(&mut self) {
        let Some(Ok(list)) = &self.discover.list else {
            return;
        };
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos().into());
        self.discover.picked = public::pick(list, self.discover.access, seed).map(|s| s.id.clone());
        if self.discover.picked.is_none() {
            self.toasts.push(Kind::Info, "No servers to pick from", "");
        }
    }

    fn public_row(&mut self, ui: &mut egui::Ui, s: &PublicServer, picked: bool) {
        let p = self.palette();
        let stroke = if picked {
            Stroke::new(2.0, p.accent)
        } else {
            Stroke::new(1.0, p.card_stroke)
        };
        let mut join = false;
        let mut add = false;
        egui::Frame::new()
            .fill(p.card_fill)
            .stroke(stroke)
            .corner_radius(10)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 220.0).max(160.0));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&s.name).strong().color(p.text));
                            ui.label(RichText::new(&s.address).small().color(p.muted));
                            if !s.partner.is_empty() {
                                ui.label(RichText::new(&s.partner).small().color(p.accent))
                                    .on_hover_text("A partner of Arctic Launcher");
                            }
                        });
                        let mut detail = Vec::new();
                        if !s.tags.is_empty() {
                            detail.push(s.tags.join(" · "));
                        }
                        if !s.version.is_empty() {
                            detail.push(s.version.clone());
                        }
                        if !detail.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(detail.join("  |  ")).small().color(p.muted),
                                )
                                .truncate(),
                            );
                        }
                        if !s.description.is_empty() {
                            ui.add(
                                egui::Label::new(RichText::new(&s.description).color(p.muted))
                                    .truncate(),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        join = widgets::button(ui, p, Some(Icon::Play), "Join", true).clicked();
                        add = widgets::icon_button(ui, p, Icon::Plus, "Add to your server list")
                            .clicked();
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                                ui.label(
                                    RichText::new(format!("{} / {}", s.players, s.max_players))
                                        .color(p.text),
                                );
                                let (label, tip) = match s.cracked {
                                    Some(false) => ("premium", "Needs a Microsoft account"),
                                    Some(true) => ("cracked", "Lets any name in"),
                                    None => ("", ""),
                                };
                                if !label.is_empty() {
                                    ui.label(RichText::new(label).small().color(p.muted))
                                        .on_hover_text(tip);
                                }
                            });
                        });
                    });
                });
            });
        if add {
            let instance = self.selected_instance().clone();
            match arctic_core::servers::add(&instance.game_dir(&self.dirs), &s.name, &s.address) {
                Ok(true) => {
                    self.toasts.push(
                        Kind::Success,
                        format!("Added {}", s.name),
                        format!("It's in {}'s server list.", instance.name),
                    );
                    self.servers_changed();
                }
                Ok(false) => self.toasts.push(
                    Kind::Info,
                    format!("{} is already on your list", s.name),
                    "",
                ),
                Err(e) => self
                    .toasts
                    .push(Kind::Error, "Couldn't add it", e.to_string()),
            }
        }
        if join {
            self.discover.open = false;
            self.launch_into(QuickPlay::Server(s.address.clone()));
        }
    }

    fn list_form(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.label(
            RichText::new("List your server")
                .size(22.0)
                .strong()
                .color(p.text),
        );
        ui.label(
            RichText::new(
                "You get a code to put in your server's MOTD, which proves it's yours. \
                 Then it waits for a quick review before it shows up.",
            )
            .color(p.muted),
        );
        ui.add_space(10.0);
        let account = self.accounts.active().cloned();
        let Some(form) = self.discover.form.as_mut() else {
            return;
        };
        if let Some(sub) = form.submitted.clone() {
            ui.label(RichText::new("Put this anywhere in your MOTD:").color(p.text));
            ui.horizontal(|ui| {
                ui.label(RichText::new(&sub.code).size(24.0).strong().color(p.accent));
                if widgets::icon_button(ui, p, Icon::Copy, "Copy").clicked() {
                    ui.ctx().copy_text(sub.code.clone());
                }
            });
            ui.label(
                RichText::new("Restart the server (or reload its MOTD), then check. You can remove the code once it's listed.")
                    .small()
                    .color(p.muted),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let check =
                    !form.busy && widgets::button(ui, p, None, "Check my MOTD", true).clicked();
                if form.busy {
                    ui.spinner();
                }
                if check && let Some(account) = account.clone() {
                    form.busy = true;
                    form.message = None;
                    self.tasks.public_verify(account, sub.id.clone());
                }
            });
        } else {
            egui::Grid::new("list_form")
                .num_columns(2)
                .spacing(vec2(12.0, 8.0))
                .show(ui, |ui| {
                    ui.label(RichText::new("Address").color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.address)
                            .hint_text("play.example.net")
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Name").color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.name)
                            .char_limit(40)
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("About it").color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.description)
                            .hint_text("Optional, one line")
                            .char_limit(200)
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Tags").color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.tags)
                            .hint_text("smp, pvp, minigames (up to 5)")
                            .desired_width(360.0),
                    );
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ready = !form.busy
                    && !form.fields.address.trim().is_empty()
                    && !form.fields.name.trim().is_empty();
                let submit = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, p, None, "Get my code", true)
                    })
                    .inner
                    .clicked();
                if form.busy {
                    ui.spinner();
                }
                if submit {
                    match account.clone() {
                        Some(account) => {
                            form.fields.tags = form
                                .tags
                                .split(',')
                                .map(|t| t.trim().to_owned())
                                .filter(|t| !t.is_empty())
                                .take(MAX_TAGS)
                                .collect();
                            form.busy = true;
                            form.message = None;
                            self.tasks.public_submit(account, form.fields.clone());
                        }
                        None => form.message = Some((true, "Add an account first.".into())),
                    }
                }
            });
        }
        if let Some((is_error, text)) = &form.message {
            ui.add_space(6.0);
            ui.label(RichText::new(text).color(if *is_error { p.error } else { p.accent }));
        }
    }

    pub(crate) fn on_public_servers(&mut self, list: Outcome<Vec<PublicServer>>) {
        self.discover.list = Some(list);
    }

    pub(crate) fn on_server_submitted(&mut self, result: Outcome<Submitted>) {
        let Some(form) = self.discover.form.as_mut() else {
            return;
        };
        form.busy = false;
        match result {
            Ok(sub) => form.submitted = Some(sub),
            Err(e) => form.message = Some((true, e)),
        }
    }

    pub(crate) fn on_server_verified(&mut self, result: Outcome<String>) {
        let Some(form) = self.discover.form.as_mut() else {
            return;
        };
        form.busy = false;
        form.message = Some(match result {
            Ok(state) if state == "pending" => (
                false,
                "Found it! Your server now waits for a quick review.".into(),
            ),
            Ok(state) if state == "listed" => (false, "It's listed.".into()),
            Ok(state) => (false, format!("Its state: {state}")),
            Err(e) => (true, e),
        });
    }
}
