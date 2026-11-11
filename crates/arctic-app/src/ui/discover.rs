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
use crate::{t, tf};
const WIDTH: f32 = 640.0;
const LIST_HEIGHT: f32 = 380.0;
const MAX_TAGS: usize = 5;

#[derive(Default)]
pub struct DiscoverUi {
    open: bool,
    list: Option<Outcome<Vec<PublicServer>>>,
    access: Access,
    search: String,
    sort: Sort,
    /// "Surprise me" result, shown first.
    picked: Option<String>,
    form: Option<ListForm>,
}

/// How the list is ordered. Random (the server's own new order every time) is the point of the
/// list and the default; the others are there for a look at the biggest, the smallest or a name.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    Random,
    MostPlayers,
    FewestPlayers,
    Name,
}

impl Sort {
    const ALL: [(Sort, &'static str); 4] = [
        (Sort::Random, "instances.sort.random"),
        (Sort::MostPlayers, "instances.sort.most_players"),
        (Sort::FewestPlayers, "instances.sort.fewest_players"),
        (Sort::Name, "instances.sort.name"),
    ];

    fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map_or(t!("instances.sort.random"), |(_, l)| t!(*l))
    }

    /// Ordered in place; servers that don't answer go last when sorting by players.
    fn apply(self, list: &mut [&PublicServer]) {
        match self {
            Sort::Random => {}
            Sort::MostPlayers => {
                list.sort_by_key(|s| (!s.online, std::cmp::Reverse(s.players)));
            }
            Sort::FewestPlayers => list.sort_by_key(|s| (!s.online, s.players)),
            Sort::Name => list.sort_by_cached_key(|s| s.name.to_lowercase()),
        }
    }
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
                crate::widgets::row(ui, |ui| {
                    if self.discover.form.is_some()
                        && widgets::button(
                            ui,
                            p,
                            Some(Icon::ChevronLeft),
                            t!("instances.back"),
                            false,
                        )
                        .clicked()
                    {
                        self.discover.form = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close |=
                            widgets::button(ui, p, None, t!("instances.close"), false).clicked();
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
        crate::widgets::row(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(t!("instances.discover_servers"))
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(t!("instances.a_new_random_order_every_time_nobody_pays"))
                        .color(p.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Plus),
                    t!("instances.list_your_server"),
                    false,
                )
                .clicked()
                {
                    self.discover.form = Some(ListForm::default());
                }
            });
        });
        ui.add_space(10.0);
        crate::widgets::row(ui, |ui| {
            for (access, label) in [
                (Access::Any, t!("instances.all")),
                (Access::Premium, t!("instances.premium")),
                (Access::Cracked, t!("instances.cracked")),
                (Access::Partners, t!("instances.partners")),
            ] {
                let tip = match access {
                    Access::Any => t!("instances.every_server"),
                    Access::Premium => t!("instances.need_a_microsoft_account"),
                    Access::Cracked => t!("instances.let_any_name_in"),
                    Access::Partners => t!("instances.servers_hosted_with_flash_hosting_and_ours"),
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    widgets::text_field(&mut self.discover.search)
                        .hint_text(t!("instances.search_names_and_tags"))
                        .desired_width(ui.available_width().clamp(140.0, 240.0)),
                );
            });
        });
        // Their own row: next to the filters and search there wasn't room, and the button hid them.
        crate::widgets::row(ui, |ui| {
            egui::ComboBox::from_id_salt("discover-sort")
                .selected_text(tf!("instances.sort", label = self.discover.sort.label()))
                .show_ui(ui, |ui| {
                    for (sort, label) in Sort::ALL {
                        ui.selectable_value(&mut self.discover.sort, sort, t!(label));
                    }
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, None, t!("instances.surprise_me"), true).clicked() {
                    self.surprise();
                }
            });
        });
        ui.add_space(8.0);
        let list = match &self.discover.list {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("instances.getting_the_list")).color(p.muted));
                });
                return;
            }
            Some(Err(e)) => {
                ui.label(
                    RichText::new(tf!("instances.the_list_couldnt_be_loaded", e = e))
                        .color(p.error),
                );
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
                ui.label(RichText::new(t!("instances.no_servers_match")).color(p.muted));
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
                let (mut partners, mut others) = (partners, others);
                self.discover.sort.apply(&mut partners);
                self.discover.sort.apply(&mut others);
                // Stable: within each tier, the order picked above stays.
                partners.sort_by_key(|s| s.partner_tier != "exclusive");
                // "Surprise me" keeps its pick on top.
                if let Some(picked) = &self.discover.picked
                    && let Some(i) = others.iter().position(|s| &s.id == picked)
                {
                    let s = others.remove(i);
                    others.insert(0, s);
                }
                let top = if partners_tab {
                    t!("instances.partners")
                } else {
                    t!("instances.exclusive_partners")
                };
                for (title, group) in [(top, partners), (t!("instances.all_servers"), others)] {
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
            self.toasts
                .push(Kind::Info, t!("instances.no_servers_to_pick_from"), "");
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
                crate::widgets::row(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 220.0).max(160.0));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&s.name).strong().color(p.text));
                            ui.label(RichText::new(&s.address).small().color(p.muted));
                            if !s.partner.is_empty() {
                                ui.label(RichText::new(&s.partner).small().color(p.accent))
                                    .on_hover_text(t!("instances.a_partner_of_arctic_launcher"));
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
                        join = widgets::button(ui, p, Some(Icon::Play), t!("instances.join"), true)
                            .clicked();
                        add = widgets::icon_button(
                            ui,
                            p,
                            Icon::Plus,
                            t!("instances.add_to_your_server_list"),
                        )
                        .clicked();
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                                ui.label(
                                    RichText::new(format!("{} / {}", s.players, s.max_players))
                                        .color(p.text),
                                );
                                let (label, tip) = match s.cracked {
                                    Some(false) => {
                                        ("premium", t!("instances.needs_a_microsoft_account"))
                                    }
                                    Some(true) => ("cracked", t!("instances.lets_any_name_in")),
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
                        tf!("instances.added", name = s.name),
                        tf!("instances.its_in_s_server_list", name = instance.name),
                    );
                    self.servers_changed();
                }
                Ok(false) => self.toasts.push(
                    Kind::Info,
                    tf!("instances.is_already_on_your_list", name = s.name),
                    "",
                ),
                Err(e) => {
                    self.toasts
                        .push(Kind::Error, t!("instances.couldnt_add_it"), e.to_string())
                }
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
            RichText::new(t!("instances.list_your_server"))
                .size(22.0)
                .strong()
                .color(p.text),
        );
        ui.label(
            RichText::new(t!("instances.you_get_a_code_to_put_in_your_servers_motd"))
                .color(p.muted),
        );
        ui.add_space(10.0);
        let account = self.accounts.active().cloned();
        let Some(form) = self.discover.form.as_mut() else {
            return;
        };
        if let Some(sub) = form.submitted.clone() {
            ui.label(RichText::new(t!("instances.put_this_anywhere_in_your_motd")).color(p.text));
            ui.horizontal(|ui| {
                ui.label(RichText::new(&sub.code).size(24.0).strong().color(p.accent));
                if widgets::icon_button(ui, p, Icon::Copy, t!("instances.copy")).clicked() {
                    ui.ctx().copy_text(sub.code.clone());
                }
            });
            ui.label(
                RichText::new(t!("instances.restart_the_server_or_reload_its_motd_then"))
                    .small()
                    .color(p.muted),
            );
            ui.add_space(8.0);
            crate::widgets::row(ui, |ui| {
                let check = !form.busy
                    && widgets::button(ui, p, None, t!("instances.check_my_motd"), true).clicked();
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
                    ui.label(RichText::new(t!("instances.address")).color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.address)
                            .hint_text("play.example.net")
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new(t!("instances.name")).color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.name)
                            .char_limit(40)
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new(t!("instances.about_it")).color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.fields.description)
                            .hint_text(t!("instances.optional_one_line"))
                            .char_limit(200)
                            .desired_width(360.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new(t!("instances.tags")).color(p.muted));
                    ui.add(
                        widgets::text_field(&mut form.tags)
                            .hint_text(t!("instances.smp_pvp_minigames_up_to_5"))
                            .desired_width(360.0),
                    );
                    ui.end_row();
                });
            ui.add_space(8.0);
            crate::widgets::row(ui, |ui| {
                let ready = !form.busy
                    && !form.fields.address.trim().is_empty()
                    && !form.fields.name.trim().is_empty();
                let submit = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, p, None, t!("instances.get_my_code"), true)
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
                        None => {
                            form.message = Some((true, t!("instances.add_an_account_first").into()))
                        }
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
                t!("instances.found_it_your_server_now_waits_for_a_quick").into(),
            ),
            Ok(state) if state == "listed" => (false, t!("instances.its_listed").into()),
            Ok(state) => (false, tf!("instances.its_state", state = state)),
            Err(e) => (true, e),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, players: u32, online: bool) -> PublicServer {
        serde_json::from_value(serde_json::json!({
            "id": name, "address": name, "name": name, "players": players, "online": online
        }))
        .unwrap()
    }

    #[test]
    fn sorting_keeps_random_and_puts_silent_servers_last() {
        let all = [
            server("b", 10, true),
            server("a", 500, true),
            server("c", 900, false),
            server("D", 40, true),
        ];
        let names = |sort: Sort| {
            let mut list: Vec<&PublicServer> = all.iter().collect();
            sort.apply(&mut list);
            list.iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>()
                .join(",")
        };
        assert_eq!(names(Sort::Random), "b,a,c,D");
        assert_eq!(names(Sort::MostPlayers), "a,D,b,c");
        assert_eq!(names(Sort::FewestPlayers), "b,D,a,c");
        assert_eq!(names(Sort::Name), "a,b,c,D");
    }
}
