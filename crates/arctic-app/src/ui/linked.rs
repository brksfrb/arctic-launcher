//! Linked instances in the launcher: the owner's Share / Link dialog, joining
//! a friend's instance by code, the link bar on an instance's page, and the
//! diff screen every change goes through (a new copy, an owner's update, or
//! the mods a server said were missing). Nothing is applied without the
//! player ticking it and pressing Accept.

use std::collections::HashMap;
use std::time::Instant;

use arctic_core::instances::Instance;
use arctic_core::linked::{
    self, Built, Changes, ConfigChange, Diff, LeftOutReason, LinkState, MissingLookup, ModChange,
    Report, Role,
};
use eframe::egui::{self, Id, Modal, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::bridge_host::MissingRequest;
use crate::link_tasks::{CHECK_EVERY, LinkEvent};
use crate::tasks::ProgressSnapshot;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf, tn};

const WIDTH: f32 = 540.0;
const LIST_HEIGHT: f32 = 300.0;
const LEFT_OUT_HEIGHT: f32 = 110.0;

type Outcome<T> = std::result::Result<T, String>;

#[derive(Default)]
pub struct LinkedUi {
    /// Link of each instance that has one (read from disk, refreshed after every link job).
    pub(crate) states: HashMap<String, LinkState>,
    loaded: bool,
    checked_at: Option<Instant>,
    owner: Option<OwnerDialog>,
    join: Option<JoinDialog>,
    review: Option<Review>,
    progress: Option<ProgressSnapshot>,
    /// Instance asked to unlink (waiting for a yes).
    unlink_confirm: Option<String>,
    /// A game that quit to rejoin a server after installing mods (instance, address).
    rejoin: Option<(String, String)>,
}

struct OwnerDialog {
    instance: String,
    include_configs: bool,
    built: Option<Outcome<(Built, Option<Changes>)>>,
    busy: bool,
    error: Option<String>,
    confirm_stop: bool,
}

#[derive(Default)]
struct JoinDialog {
    code: String,
    busy: bool,
    error: Option<String>,
}

enum ReviewFor {
    /// A new linked copy from a code.
    NewCopy { code: String },
    /// An owner's update for a copy.
    Update { instance: String },
    /// Mods a server said the game lacks.
    Missing {
        run: u64,
        server: String,
        instance: String,
        not_found: Vec<String>,
        installed: Vec<String>,
    },
}

struct Review {
    of: ReviewFor,
    fetched: Option<linked::server::Fetched>,
    diff: Diff,
    busy: bool,
    error: Option<String>,
}

impl LinkedUi {
    /// A game quit to rejoin `address`: start `instance` again into it (next frame).
    pub(crate) fn rejoin_after(&mut self, instance: String, address: String) {
        self.rejoin = Some((instance, address));
    }
}

impl ArcticApp {
    // ------------------------------------------------------------- state

    /// Link states from disk (instances may have come and gone).
    pub(crate) fn refresh_links(&mut self) {
        let mut ids: Vec<&str> = self
            .custom_instances
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        ids.push(self.instance.id.as_str());
        self.linked.states = linked::all(&self.dirs, &ids);
        self.linked.loaded = true;
    }

    /// Every frame (also while the window is hidden): load the links once, ask the server about
    /// friends' copies now and then, look up a game's missing mods, and start a game again into
    /// its server after it quit for that.
    pub(crate) fn linked_tick(&mut self) {
        if !self.linked.loaded {
            self.refresh_links();
        }
        let due = self
            .linked
            .checked_at
            .is_none_or(|at| at.elapsed() >= CHECK_EVERY);
        if due {
            self.linked.checked_at = Some(Instant::now());
            let followers: Vec<String> = self
                .linked
                .states
                .iter()
                .filter(|(_, s)| !s.is_owner())
                .map(|(id, _)| id.clone())
                .collect();
            if !followers.is_empty() {
                self.tasks.link_check(followers);
            }
        }
        if let Some((run, request)) = self.bridge.take_missing_request() {
            self.on_missing_request(run, request);
        }
        if let Some((instance, address)) = self.linked.rejoin.take() {
            self.settings.last_instance =
                (instance != arctic_core::instances::VANILLA_ID).then(|| instance.clone());
            self.launch_unchecked(false, Some(arctic_core::launch::QuickPlay::Server(address)));
        }
    }

    pub(crate) fn link_state(&self, instance_id: &str) -> Option<&LinkState> {
        self.linked.states.get(instance_id)
    }

    fn instance_named(&self, id: &str) -> Option<Instance> {
        self.instance_by_id(id).cloned()
    }

    // ------------------------------------------------------------ events

    pub(crate) fn on_link_event(&mut self, event: LinkEvent) {
        match event {
            LinkEvent::Built(id, result) => {
                if let Some(d) = self.linked.owner.as_mut().filter(|d| d.instance == id) {
                    d.built = Some(result);
                }
            }
            LinkEvent::Published(id, result) => {
                self.refresh_links();
                let Some(d) = self.linked.owner.as_mut().filter(|d| d.instance == id) else {
                    return;
                };
                d.busy = false;
                match result {
                    Ok((state, built)) => {
                        d.error = None;
                        d.built = Some(Ok((built, Some(Changes::default()))));
                        self.toasts.push(
                            Kind::Success,
                            t!("instances.link.published"),
                            tf!("instances.link.code_is", code = state.pretty_code()),
                        );
                    }
                    Err(e) => d.error = Some(e),
                }
            }
            LinkEvent::Stopped(id, result) => {
                self.refresh_links();
                match result {
                    Ok(()) => {
                        if self.linked.owner.as_ref().is_some_and(|d| d.instance == id) {
                            self.linked.owner = None;
                        }
                        self.toasts.push(
                            Kind::Success,
                            t!("instances.link.stopped_sharing"),
                            t!("instances.link.the_code_no_longer_works"),
                        );
                    }
                    Err(e) => {
                        if let Some(d) = self.linked.owner.as_mut() {
                            d.busy = false;
                            d.error = Some(e);
                        }
                    }
                }
            }
            LinkEvent::Checked(states) => {
                for (id, state) in states {
                    let news = state.reminds()
                        && !self
                            .linked
                            .states
                            .get(&id)
                            .is_some_and(|old| old.latest == state.latest);
                    if news && let Some(i) = self.instance_by_id(&id) {
                        let name = i.name.clone();
                        self.toasts.push(
                            Kind::Info,
                            t!("instances.link.update_ready"),
                            tf!("instances.link.update_for", name = name),
                        );
                    }
                    self.linked.states.insert(id, state);
                }
            }
            LinkEvent::Preview(result) => match result {
                Ok((code, fetched, diff)) => {
                    self.linked.join = None;
                    self.linked.review = Some(Review {
                        of: ReviewFor::NewCopy { code },
                        fetched: Some(fetched),
                        diff,
                        busy: false,
                        error: None,
                    });
                }
                Err(e) => {
                    if let Some(j) = self.linked.join.as_mut() {
                        j.busy = false;
                        j.error = Some(e);
                    }
                }
            },
            LinkEvent::Review(id, result) => match result {
                Ok((fetched, diff)) if diff.is_empty() => {
                    // Nothing to change here: the link simply stands at that revision now.
                    if let Some(mut state) = linked::load(&self.dirs, &id) {
                        state.revision = fetched.revision;
                        state.latest = state.latest.max(fetched.revision);
                        state.manifest = Some(fetched.manifest);
                        let _ = linked::save(&self.dirs, &id, &state);
                    }
                    self.refresh_links();
                    self.linked.review = None;
                    self.toasts.push(
                        Kind::Success,
                        t!("instances.link.up_to_date"),
                        t!("instances.link.same_as_the_owners"),
                    );
                }
                Ok((fetched, diff)) => {
                    self.linked.review = Some(Review {
                        of: ReviewFor::Update { instance: id },
                        fetched: Some(fetched),
                        diff,
                        busy: false,
                        error: None,
                    });
                }
                Err(e) => {
                    self.linked.review = None;
                    self.toasts
                        .push(Kind::Error, t!("instances.link.couldnt_get_the_update"), e);
                }
            },
            LinkEvent::Progress(p) => self.linked.progress = Some(p),
            LinkEvent::Applied(id, result) => self.on_link_applied(id, result),
            LinkEvent::MissingFound(run, server, result) => {
                self.on_missing_found(run, server, result)
            }
        }
    }

    fn on_link_applied(&mut self, id: Option<String>, result: Outcome<Report>) {
        self.linked.progress = None;
        let review = self.linked.review.take();
        if let Some(Review {
            of: ReviewFor::Missing { run, server, .. },
            ..
        }) = &review
        {
            match &result {
                Ok(report) => {
                    self.bridge.set_missing_state(
                        *run,
                        "installed",
                        server,
                        report.added,
                        &[],
                        None,
                    );
                }
                Err(e) => {
                    self.bridge
                        .set_missing_state(*run, "failed", server, 0, &[], Some(e));
                }
            }
        }
        match result {
            Ok(report) => {
                self.reload_instances();
                self.refresh_links();
                if let Some(id) = &id {
                    self.refresh_mods(id);
                    if matches!(
                        review.as_ref().map(|r| &r.of),
                        Some(ReviewFor::NewCopy { .. })
                    ) {
                        self.open_instance(id);
                    }
                }
                self.toasts.push(
                    Kind::Success,
                    t!("instances.link.changes_applied"),
                    report_text(&report),
                );
            }
            Err(e) => {
                // The diff stays up, so nothing the player picked is lost.
                if let Some(mut r) = review {
                    r.busy = false;
                    r.error = Some(e.clone());
                    if !matches!(r.of, ReviewFor::Missing { .. }) {
                        self.linked.review = Some(r);
                    }
                }
                self.toasts
                    .push(Kind::Error, t!("instances.link.nothing_was_changed"), e);
            }
        }
    }

    fn on_missing_request(&mut self, run: u64, request: MissingRequest) {
        let instance = self
            .runs
            .get(run)
            .and_then(|r| self.instance_named(&r.instance_id.clone()));
        let fail = |app: &mut Self, message: &str| {
            app.bridge
                .set_missing_state(run, "failed", &request.server, 0, &[], Some(message));
        };
        let Some(instance) = instance else {
            fail(self, t!("instances.link.that_game_isnt_running_any_more"));
            return;
        };
        if instance.loader.kind().is_none() {
            fail(self, t!("instances.link.vanilla_cant_take_mods"));
            return;
        }
        if request.mods.is_empty() {
            fail(self, t!("instances.link.the_server_didnt_name_its_mods"));
            return;
        }
        log::info!(
            "missing mods for {} (needs {:?}): {}",
            request.server,
            request.loader,
            request.reason
        );
        self.tasks
            .link_find_missing(run, request.server.clone(), instance, request.mods.clone());
    }

    fn on_missing_found(&mut self, run: u64, server: String, result: Outcome<MissingLookup>) {
        let instance = self.runs.get(run).map(|r| r.instance_id.clone());
        match (result, instance) {
            (Ok(lookup), Some(instance)) => match lookup.diff {
                Some(diff) => {
                    self.bridge.set_missing_state(
                        run,
                        "review",
                        &server,
                        0,
                        &lookup.not_found,
                        None,
                    );
                    crate::window::show();
                    self.linked.review = Some(Review {
                        of: ReviewFor::Missing {
                            run,
                            server,
                            instance,
                            not_found: lookup.not_found,
                            installed: lookup.installed,
                        },
                        fetched: None,
                        diff,
                        busy: false,
                        error: None,
                    });
                }
                None => {
                    let message = if lookup.not_found.is_empty() {
                        t!("instances.link.everything_named_is_already_installed")
                    } else {
                        t!("instances.link.none_of_them_are_on_modrinth")
                    };
                    self.bridge.set_missing_state(
                        run,
                        "failed",
                        &server,
                        0,
                        &lookup.not_found,
                        Some(message),
                    );
                }
            },
            (Err(e), _) => {
                self.bridge
                    .set_missing_state(run, "failed", &server, 0, &[], Some(&e));
            }
            (Ok(_), None) => {}
        }
    }

    // ----------------------------------------------------------- opening

    /// The Share / Link dialog of an instance.
    pub(crate) fn open_link(&mut self, instance_id: &str) {
        let Some(instance) = self.instance_named(instance_id) else {
            return;
        };
        let state = linked::load(&self.dirs, instance_id);
        let include_configs = match state.as_ref().map(|s| &s.role) {
            Some(Role::Owner {
                include_configs, ..
            }) => *include_configs,
            _ => true,
        };
        let follower = state.as_ref().is_some_and(|s| !s.is_owner());
        if !follower && instance.version.is_some() {
            self.tasks
                .link_build(instance, include_configs, state.clone());
        }
        self.linked.owner = Some(OwnerDialog {
            instance: instance_id.to_owned(),
            include_configs,
            built: None,
            busy: false,
            error: None,
            confirm_stop: false,
        });
    }

    pub(crate) fn open_link_join(&mut self) {
        self.linked.join = Some(JoinDialog::default());
    }

    fn start_review(&mut self, instance_id: &str) {
        if let Some(instance) = self.instance_named(instance_id) {
            self.tasks.link_review(instance);
            self.linked.review = Some(Review {
                of: ReviewFor::Update {
                    instance: instance_id.to_owned(),
                },
                fetched: None,
                diff: linked::diff::additions(Vec::new()),
                busy: true,
                error: None,
            });
        }
    }

    // ----------------------------------------------------------- drawing

    /// The link's bar on an instance's page.
    pub(crate) fn link_bar(&mut self, ui: &mut egui::Ui, instance_id: &str) {
        let Some(state) = self.link_state(instance_id).cloned() else {
            return;
        };
        let p = self.palette();
        let running = self.runs.instance_active(instance_id);
        ui.add_space(8.0);
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::row(ui, |ui| {
                let (text, color) = if state.is_owner() {
                    (
                        tf!("instances.link.shared_as", code = state.pretty_code()),
                        p.text,
                    )
                } else if state.gone {
                    (
                        t!("instances.link.the_owner_stopped_sharing").to_owned(),
                        p.warn,
                    )
                } else if state.has_update() {
                    (
                        tf!(
                            "instances.link.an_update_from_the_owner_is_ready",
                            code = state.pretty_code()
                        ),
                        p.accent,
                    )
                } else {
                    (
                        tf!("instances.link.linked_copy", code = state.pretty_code()),
                        p.text,
                    )
                };
                ui.label(RichText::new(text).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !state.is_owner() {
                        if widgets::button(ui, p, None, t!("instances.link.unlink"), false)
                            .clicked()
                        {
                            self.linked.unlink_confirm = Some(instance_id.to_owned());
                        }
                        if state.reminds()
                            && widgets::button(ui, p, None, t!("instances.link.not_now"), false)
                                .clicked()
                        {
                            let _ = linked::dismiss(&self.dirs, instance_id);
                            self.refresh_links();
                        }
                        if !state.gone {
                            let label = if state.has_update() {
                                t!("instances.link.review_update")
                            } else {
                                t!("instances.link.check_for_updates")
                            };
                            let review = ui
                                .add_enabled_ui(self.linked.review.is_none(), |ui| {
                                    widgets::button(ui, p, None, label, state.has_update())
                                })
                                .inner;
                            if review.clicked() {
                                self.start_review(instance_id);
                            }
                        }
                    } else if widgets::button(ui, p, None, t!("instances.link.manage"), false)
                        .clicked()
                    {
                        self.open_link(instance_id);
                    }
                });
            });
            if running && !state.is_owner() && state.has_update() {
                ui.label(
                    RichText::new(t!("instances.link.close_minecraft_to_apply"))
                        .small()
                        .color(p.muted),
                );
            }
        });
    }

    /// A small tag on an instance's card: linked, or an update waits.
    pub(crate) fn link_tag(&self, instance_id: &str) -> Option<(&'static str, bool)> {
        let state = self.link_state(instance_id)?;
        Some(if state.reminds() {
            (t!("instances.link.tag_update"), true)
        } else if state.is_owner() {
            (t!("instances.link.tag_shared"), false)
        } else {
            (t!("instances.link.tag_linked"), false)
        })
    }

    pub(crate) fn linked_dialogs(&mut self, ctx: &egui::Context) {
        self.owner_dialog(ctx);
        self.join_dialog(ctx);
        self.review_dialog(ctx);
        self.unlink_dialog(ctx);
    }

    fn owner_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.linked.owner.take() else {
            return;
        };
        let p = self.palette();
        let state = self.link_state(&d.instance).cloned();
        let instance = self.instance_named(&d.instance);
        let mut close = false;
        let mut rebuild = false;
        let modal = Modal::new(Id::new("link_owner"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.label(
                    RichText::new(t!("instances.link.share_link"))
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(t!("instances.link.what_is_shared"))
                        .small()
                        .color(p.muted),
                );
                ui.add_space(10.0);
                match (&state, &instance) {
                    (_, None) => {
                        close = true;
                    }
                    (Some(s), Some(_)) if !s.is_owner() => {
                        ui.label(
                            RichText::new(tf!(
                                "instances.link.linked_copy",
                                code = s.pretty_code()
                            ))
                            .color(p.text),
                        );
                        ui.label(
                            RichText::new(t!("instances.link.unlink_to_share_it_yourself"))
                                .color(p.muted),
                        );
                    }
                    (_, Some(i)) if i.version.is_none() => {
                        ui.label(
                            RichText::new(t!("instances.link.vanilla_follows_the_play_tab"))
                                .color(p.muted),
                        );
                    }
                    (state, Some(_)) => {
                        if let Some(s) = state {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(s.pretty_code())
                                        .size(26.0)
                                        .strong()
                                        .monospace()
                                        .color(p.accent),
                                );
                                if widgets::icon_button(
                                    ui,
                                    p,
                                    Icon::Copy,
                                    t!("instances.link.copy_code"),
                                )
                                .clicked()
                                {
                                    ui.ctx().copy_text(s.pretty_code());
                                    self.toasts.push(
                                        Kind::Success,
                                        t!("instances.link.code_copied"),
                                        "",
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(t!("instances.link.friends_enter_this_code"))
                                    .small()
                                    .color(p.muted),
                            );
                            ui.add_space(8.0);
                        }
                        ui.horizontal(|ui| {
                            if widgets::switch(ui, p, &mut d.include_configs).changed() {
                                rebuild = true;
                            }
                            ui.label(
                                RichText::new(t!("instances.link.include_config_files"))
                                    .color(p.text),
                            );
                        });
                        ui.add_space(6.0);
                        owner_built(ui, p, &d, state.is_some());
                        if let Some(e) = &d.error {
                            ui.add_space(6.0);
                            ui.label(RichText::new(e).color(p.error));
                        }
                        ui.add_space(10.0);
                        self.owner_buttons(ui, &mut d, state.as_ref());
                    }
                }
                ui.add_space(12.0);
                if state.as_ref().is_none_or(LinkState::is_owner) {
                    ui.label(
                        RichText::new(t!("instances.link.no_account_needed"))
                            .small()
                            .color(p.muted),
                    );
                    ui.add_space(6.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, p, None, t!("instances.link.done"), false).clicked() {
                        close = true;
                    }
                });
            });
        if rebuild && let Some(i) = instance {
            d.built = None;
            self.tasks.link_build(i, d.include_configs, state);
        }
        if !(close || modal.should_close()) || d.busy {
            self.linked.owner = Some(d);
        }
    }

    fn owner_buttons(&mut self, ui: &mut egui::Ui, d: &mut OwnerDialog, state: Option<&LinkState>) {
        let p = self.palette();
        let Some(instance) = self.instance_named(&d.instance) else {
            return;
        };
        let ready = matches!(d.built, Some(Ok(_))) && !d.busy;
        let changed = match &d.built {
            Some(Ok((_, Some(c)))) => !c.is_empty(),
            _ => state.is_none(),
        };
        widgets::row(ui, |ui| {
            if d.busy {
                ui.spinner();
            }
            let (label, enabled) = match state {
                None => (t!("instances.link.create_link_code"), ready),
                Some(_) => (t!("instances.link.publish_update"), ready && changed),
            };
            let go = ui
                .add_enabled_ui(enabled, |ui| {
                    widgets::button(ui, p, Some(Icon::Share), label, true)
                })
                .inner;
            if go.clicked() {
                d.busy = true;
                d.error = None;
                self.tasks.link_publish(
                    instance.clone(),
                    d.include_configs,
                    self.accounts.active().cloned(),
                );
            }
            if state.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if d.confirm_stop {
                        if widgets::button(ui, p, None, t!("instances.link.yes_stop_sharing"), true)
                            .clicked()
                        {
                            d.busy = true;
                            d.confirm_stop = false;
                            self.tasks.link_stop(d.instance.clone());
                        }
                        if widgets::button(ui, p, None, t!("common.cancel"), false).clicked() {
                            d.confirm_stop = false;
                        }
                    } else {
                        if widgets::button(ui, p, None, t!("instances.link.stop_sharing"), false)
                            .on_hover_text(t!("instances.link.stop_sharing_hint"))
                            .clicked()
                        {
                            d.confirm_stop = true;
                        }
                        if widgets::button(ui, p, None, t!("instances.link.unlink"), false)
                            .on_hover_text(t!("instances.link.owner_unlink_hint"))
                            .clicked()
                        {
                            self.linked.unlink_confirm = Some(d.instance.clone());
                        }
                    }
                });
            }
        });
    }

    fn join_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut j) = self.linked.join.take() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let modal = Modal::new(Id::new("link_join"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH * 0.8);
                ui.label(
                    RichText::new(t!("instances.link.join_with_code"))
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(RichText::new(t!("instances.link.join_explained")).color(p.muted));
                ui.add_space(10.0);
                let field = ui.add(
                    widgets::text_field(&mut j.code)
                        .hint_text("abcd-efgh") // i18n-ignore
                        .desired_width(200.0),
                );
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(e) = &j.error {
                    ui.label(RichText::new(e).color(p.error));
                }
                ui.add_space(10.0);
                widgets::row(ui, |ui| {
                    let valid = arctic_core::sharing::codes::clean(&j.code).is_some();
                    if j.busy {
                        ui.spinner();
                    }
                    let go = ui
                        .add_enabled_ui(valid && !j.busy, |ui| {
                            widgets::button(ui, p, None, t!("instances.link.look_up"), true)
                        })
                        .inner;
                    if (go.clicked() || (entered && valid)) && !j.busy {
                        j.busy = true;
                        j.error = None;
                        self.tasks.link_preview(j.code.trim().to_owned());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, p, None, t!("common.cancel"), false).clicked() {
                            close = true;
                        }
                    });
                });
            });
        if !(close || modal.should_close()) {
            self.linked.join = Some(j);
        }
    }

    fn unlink_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.linked.unlink_confirm.clone() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let modal = Modal::new(Id::new("link_unlink"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH * 0.8);
                ui.label(
                    RichText::new(t!("instances.link.unlink_this_instance"))
                        .size(20.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(RichText::new(t!("instances.link.unlink_explained")).color(p.muted));
                ui.add_space(10.0);
                widgets::row(ui, |ui| {
                    if widgets::button(ui, p, None, t!("instances.link.unlink"), true).clicked() {
                        match linked::unlink(&self.dirs, &id) {
                            Ok(()) => {
                                self.toasts.push(
                                    Kind::Success,
                                    t!("instances.link.unlinked"),
                                    t!("instances.link.its_a_normal_instance_now"),
                                );
                                if self.linked.owner.as_ref().is_some_and(|d| d.instance == id) {
                                    self.linked.owner = None;
                                }
                            }
                            Err(e) => self.toasts.push(
                                Kind::Error,
                                t!("instances.link.couldnt_unlink"),
                                e.to_string(),
                            ),
                        }
                        self.refresh_links();
                        close = true;
                    }
                    if widgets::button(ui, p, None, t!("common.cancel"), false).clicked() {
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            self.linked.unlink_confirm = None;
        }
    }

    fn review_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.linked.review.take() else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let mut accept = false;
        let running = match &r.of {
            ReviewFor::Update { instance } => self.runs.instance_active(instance),
            _ => false,
        };
        let modal = Modal::new(Id::new("link_review"))
            .frame(super::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                let (title, subtitle) = match &r.of {
                    ReviewFor::NewCopy { code } => (
                        t!("instances.link.new_linked_instance").to_owned(),
                        r.fetched.as_ref().map_or_else(String::new, |f| {
                            tf!(
                                "instances.link.from_code",
                                name = f.manifest.name,
                                code = arctic_core::sharing::codes::pretty(
                                    &arctic_core::sharing::codes::clean(code).unwrap_or_default()
                                ),
                                version = f.manifest.game_version,
                                loader = f.manifest.loader.label()
                            )
                        }),
                    ),
                    ReviewFor::Update { instance } => (
                        t!("instances.link.review_update_title").to_owned(),
                        self.instance_by_id(instance)
                            .map(|i| i.name.clone())
                            .unwrap_or_default(),
                    ),
                    ReviewFor::Missing { server, .. } => (
                        t!("instances.link.missing_mods_title").to_owned(),
                        tf!("instances.link.missing_for_server", server = server),
                    ),
                };
                ui.label(RichText::new(title).size(22.0).strong().color(p.text));
                ui.label(RichText::new(subtitle).color(p.muted));
                ui.add_space(8.0);
                if r.fetched.is_none() && !matches!(r.of, ReviewFor::Missing { .. }) {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(t!("instances.link.fetching")).color(p.muted));
                    });
                } else {
                    diff_list(ui, p, &mut r.diff, !r.busy);
                    if let ReviewFor::Missing {
                        not_found,
                        installed,
                        ..
                    } = &r.of
                    {
                        missing_notes(ui, p, not_found, installed);
                    }
                }
                if let Some(e) = &r.error {
                    ui.add_space(6.0);
                    ui.label(RichText::new(e).color(p.error));
                }
                if running && arctic_core::linked::apply::needs_game_closed(&r.diff.selected()) {
                    ui.label(
                        RichText::new(t!("instances.link.close_minecraft_to_apply")).color(p.warn),
                    );
                }
                if r.busy && r.fetched.is_some()
                    || r.busy && matches!(r.of, ReviewFor::Missing { .. })
                {
                    progress_line(ui, p, self.linked.progress.as_ref());
                }
                ui.add_space(10.0);
                widgets::row(ui, |ui| {
                    let n = r.diff.selected().accepted_count();
                    let ready = (r.fetched.is_some() || matches!(r.of, ReviewFor::Missing { .. }))
                        && !r.busy;
                    let enabled = ready
                        && (n > 0 || matches!(r.of, ReviewFor::NewCopy { .. }))
                        && !(running
                            && arctic_core::linked::apply::needs_game_closed(&r.diff.selected()));
                    let go = ui
                        .add_enabled_ui(enabled, |ui| {
                            widgets::button(
                                ui,
                                p,
                                Some(Icon::Check),
                                &tn!("instances.link.accept_n", n),
                                true,
                            )
                        })
                        .inner;
                    if go.clicked() {
                        accept = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = match r.of {
                            ReviewFor::Update { .. } => t!("instances.link.not_now"),
                            _ => t!("common.cancel"),
                        };
                        let cancel = ui
                            .add_enabled_ui(!r.busy, |ui| {
                                widgets::button(ui, p, None, label, false)
                            })
                            .inner;
                        if cancel.clicked() {
                            close = true;
                        }
                    });
                });
            });
        if accept {
            r.busy = true;
            r.error = None;
            self.linked.progress = None;
            let diff = r.diff.clone();
            match &r.of {
                ReviewFor::NewCopy { code } => {
                    if let Some(f) = r.fetched.clone() {
                        self.tasks.link_follow(code.clone(), f, diff);
                    }
                }
                ReviewFor::Update { instance } => {
                    if let (Some(i), Some(f)) = (self.instance_named(instance), r.fetched.clone()) {
                        self.tasks.link_take(i, f, diff);
                    }
                }
                ReviewFor::Missing {
                    run,
                    server,
                    instance,
                    ..
                } => {
                    if let Some(i) = self.instance_named(instance) {
                        self.bridge
                            .set_missing_state(*run, "installing", server, 0, &[], None);
                        self.tasks.link_install_missing(i, diff);
                    }
                }
            }
        }
        let closing = (close || modal.should_close()) && !r.busy;
        if closing {
            match &r.of {
                ReviewFor::Missing { run, server, .. } => {
                    self.bridge
                        .set_missing_state(*run, "declined", server, 0, &[], None);
                }
                ReviewFor::Update { instance } => {
                    let _ = linked::dismiss(&self.dirs, instance);
                    self.refresh_links();
                }
                ReviewFor::NewCopy { .. } => {}
            }
        } else {
            self.linked.review = Some(r);
        }
    }
}

/// What the owner's instance would share, and what's left out.
fn owner_built(ui: &mut egui::Ui, p: &Palette, d: &OwnerDialog, shared: bool) {
    match &d.built {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new(t!("instances.link.checking_mods_with_modrinth")).color(p.muted),
                );
            });
        }
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(p.error));
        }
        Some(Ok((built, changes))) => {
            let (mods, configs) = built.manifest.summary_counts();
            ui.label(
                RichText::new(tf!(
                    "instances.link.shares_mods_and_configs",
                    mods = mods,
                    configs = configs
                ))
                .strong()
                .color(p.text),
            );
            if shared {
                match changes {
                    Some(c) if !c.is_empty() => changes_list(ui, p, c),
                    Some(_) => {
                        ui.label(
                            RichText::new(t!("instances.link.nothing_changed_since_publish"))
                                .color(p.muted),
                        );
                    }
                    None => {}
                }
            }
            if !built.left_out.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(tn!("instances.link.left_out", built.left_out.len()))
                        .color(p.warn),
                );
                egui::ScrollArea::vertical()
                    .id_salt("link_left_out")
                    .max_height(LEFT_OUT_HEIGHT)
                    .show(ui, |ui| {
                        for l in &built.left_out {
                            ui.label(
                                RichText::new(format!(
                                    "{} — {}",
                                    l.name,
                                    left_out_reason(l.reason)
                                ))
                                .small()
                                .color(p.muted),
                            );
                        }
                    });
            }
        }
    }
}

fn left_out_reason(reason: LeftOutReason) -> &'static str {
    use arctic_core::linked::paths::Refused;
    match reason {
        LeftOutReason::NotFromModrinth => t!("instances.link.reason_not_from_modrinth"),
        LeftOutReason::DiffersFromModrinth => t!("instances.link.reason_differs"),
        LeftOutReason::TooBig => t!("instances.link.reason_too_big"),
        LeftOutReason::Config(Refused::Personal) => t!("instances.link.reason_personal"),
        LeftOutReason::Config(Refused::NotConfig) => t!("instances.link.reason_not_config"),
        LeftOutReason::Config(Refused::OutsideConfig) => t!("instances.link.reason_outside"),
    }
}

fn changes_list(ui: &mut egui::Ui, p: &Palette, c: &Changes) {
    ui.label(RichText::new(t!("instances.link.not_published_yet")).color(p.accent));
    if let Some((from, to)) = &c.target {
        ui.label(
            RichText::new(tf!("instances.link.moves_to", from = from, to = to))
                .small()
                .color(p.text),
        );
    }
    let mut line = |key: &'static str, items: &[String]| {
        if !items.is_empty() {
            ui.label(
                RichText::new(format!("{} {}", t!(key), items.join(", ")))
                    .small()
                    .color(p.text),
            );
        }
    };
    line("instances.link.added_colon", &c.added);
    line("instances.link.updated_colon", &c.updated);
    line("instances.link.removed_colon", &c.removed);
    line("instances.link.configs_colon", &c.configs);
}

/// The diff, each item with its tick box.
fn diff_list(ui: &mut egui::Ui, p: &Palette, diff: &mut Diff, enabled: bool) {
    if diff.is_empty() {
        ui.label(RichText::new(t!("instances.link.nothing_to_change")).color(p.muted));
        return;
    }
    ui.horizontal(|ui| {
        if ui.small_button(t!("instances.link.tick_all")).clicked() && enabled {
            set_all(diff, true);
        }
        if ui.small_button(t!("instances.link.tick_none")).clicked() && enabled {
            set_all(diff, false);
        }
    });
    let blocked_before = diff.mods_blocked();
    egui::ScrollArea::vertical()
        .id_salt("link_diff")
        .max_height(LIST_HEIGHT)
        .show(ui, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                if let Some(t) = diff.target.as_mut() {
                    section(ui, p, t!("instances.link.minecraft"));
                    let from = target_label(
                        &t.change.from_loader,
                        t.change.from_version.as_deref().unwrap_or("?"),
                    );
                    let to = target_label(&t.change.to_loader, &t.change.to_version);
                    item(
                        ui,
                        p,
                        &mut t.accept,
                        t!("instances.link.tag_change"),
                        p.warn,
                        &tf!("instances.link.moves_to", from = from, to = to),
                        true,
                    );
                }
                if !diff.mods.is_empty() {
                    section(ui, p, t!("instances.link.mods"));
                }
                for m in diff.mods.iter_mut() {
                    let allowed = !blocked_before || matches!(m.change, ModChange::Remove { .. });
                    let (tag, color, text) = match &m.change {
                        ModChange::Add(w) => (
                            t!("instances.link.tag_add"),
                            p.accent,
                            if w.enabled {
                                w.title.clone()
                            } else {
                                tf!("instances.link.switched_off", title = w.title)
                            },
                        ),
                        ModChange::Update { local, to } => (
                            t!("instances.link.tag_update_item"),
                            p.warn,
                            tf!(
                                "instances.link.update_from",
                                title = local.title,
                                from = local.version_number,
                                file = to.file
                            ),
                        ),
                        ModChange::Remove { local, from_link } => (
                            t!("instances.link.tag_remove"),
                            p.error,
                            if *from_link {
                                local.title.clone()
                            } else {
                                tf!("instances.link.you_added_this", title = local.title)
                            },
                        ),
                    };
                    item(ui, p, &mut m.accept, tag, color, &text, allowed);
                }
                if !diff.configs.is_empty() {
                    section(ui, p, t!("instances.link.config_files"));
                }
                for c in diff.configs.iter_mut() {
                    let (tag, color, text) = match &c.change {
                        ConfigChange::Add(f) => {
                            (t!("instances.link.tag_add"), p.accent, f.path.clone())
                        }
                        ConfigChange::Change(f) => {
                            (t!("instances.link.tag_change"), p.warn, f.path.clone())
                        }
                        ConfigChange::Remove { path, edited } => (
                            t!("instances.link.tag_remove"),
                            p.error,
                            if *edited {
                                tf!("instances.link.you_changed_this", path = path)
                            } else {
                                path.clone()
                            },
                        ),
                    };
                    item(ui, p, &mut c.accept, tag, color, &text, true);
                }
            });
        });
    if diff.mods_blocked() {
        ui.label(
            RichText::new(t!("instances.link.mods_need_the_version_change"))
                .small()
                .color(p.warn),
        );
    }
}

/// "Fabric 0.17.2 · 1.21.11" (the loader's own version too: it can change alone).
fn target_label(loader: &arctic_core::instances::Loader, game: &str) -> String {
    match loader.version() {
        Some(v) => format!("{} {v} · {game}", loader.label()),
        None => format!("{} · {game}", loader.label()),
    }
}

fn set_all(diff: &mut Diff, on: bool) {
    if let Some(t) = diff.target.as_mut() {
        t.accept = on;
    }
    diff.mods.iter_mut().for_each(|m| m.accept = on);
    diff.configs.iter_mut().for_each(|c| c.accept = on);
}

fn section(ui: &mut egui::Ui, p: &Palette, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().color(p.text));
}

fn item(
    ui: &mut egui::Ui,
    p: &Palette,
    accept: &mut bool,
    tag: &str,
    color: egui::Color32,
    text: &str,
    allowed: bool,
) {
    ui.horizontal(|ui| {
        ui.add_enabled(allowed, egui::Checkbox::without_text(accept));
        ui.label(RichText::new(tag).small().strong().color(color));
        let shown = if allowed { p.text } else { p.muted };
        ui.label(RichText::new(text).color(shown));
    });
}

fn missing_notes(ui: &mut egui::Ui, p: &Palette, not_found: &[String], installed: &[String]) {
    if !installed.is_empty() {
        ui.label(
            RichText::new(tf!(
                "instances.link.already_installed",
                mods = installed.join(", ")
            ))
            .small()
            .color(p.muted),
        );
    }
    if !not_found.is_empty() {
        ui.label(
            RichText::new(tf!(
                "instances.link.not_on_modrinth",
                mods = not_found.join(", ")
            ))
            .small()
            .color(p.warn),
        );
        ui.label(
            RichText::new(t!("instances.link.ask_the_server_for_a_link"))
                .small()
                .color(p.muted),
        );
    }
}

fn progress_line(ui: &mut egui::Ui, p: &Palette, progress: Option<&ProgressSnapshot>) {
    ui.horizontal(|ui| {
        ui.spinner();
        match progress {
            Some(s) if s.bytes_total > 0 => {
                let f = s.bytes_done as f32 / s.bytes_total as f32;
                ui.add(
                    egui::ProgressBar::new(f)
                        .desired_width(380.0)
                        .text(crate::i18n::core_text(&s.stage)),
                );
            }
            Some(s) => {
                ui.label(RichText::new(crate::i18n::core_text(&s.stage)).color(p.muted));
            }
            None => {
                ui.label(RichText::new(t!("instances.link.working")).color(p.muted));
            }
        }
    });
}

fn report_text(r: &Report) -> String {
    tf!(
        "instances.link.report",
        added = r.added,
        updated = r.updated,
        removed = r.removed,
        configs = r.configs
    )
}
