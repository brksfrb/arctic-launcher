//! Background jobs for linked instances: building and publishing an owner's
//! manifest, checking friends' copies for updates, fetching a link by code,
//! and applying an approved diff (a new copy, an update, or the mods a
//! server said were missing).

use arctic_core::auth::Account;
use arctic_core::cosmetics;
use arctic_core::instances::Instance;
use arctic_core::linked::{
    self, Built, Changes, Diff, LinkState, MissingLookup, Report, WantedMod,
};
use arctic_core::{ProgressInfo, Result};

use crate::tasks::{Event, ProgressSnapshot, Tasks};

/// How often friends' copies ask the server for news while the launcher runs.
pub const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(15 * 60);

type Outcome<T> = std::result::Result<T, String>;

/// Results of link jobs, for the UI.
pub enum LinkEvent {
    /// The owner's instance as it would be shared now, and what changed since the last publish.
    Built(String, Outcome<(Built, Option<Changes>)>),
    /// A link was made or published (instance id).
    Published(String, Outcome<(LinkState, Built)>),
    /// Sharing stopped (instance id).
    Stopped(String, Outcome<()>),
    /// Fresh states of friends' copies.
    Checked(Vec<(String, LinkState)>),
    /// A link fetched by code, for a new copy (code, link, diff).
    Preview(Outcome<(String, linked::server::Fetched, Diff)>),
    /// An update of a copy, fetched for review (instance id).
    Review(String, Outcome<(linked::server::Fetched, Diff)>),
    Progress(ProgressSnapshot),
    /// A diff was applied: a new copy (its id), an update, or missing mods (`None`).
    Applied(Option<String>, Outcome<Report>),
    /// The mods a server named, looked up (run, server).
    MissingFound(u64, String, Outcome<MissingLookup>),
}

impl Tasks {
    fn link_send(&self, e: LinkEvent) {
        self.send(Event::Linked(e));
    }

    fn link_progress(&self) -> impl Fn(ProgressInfo) + Sync + '_ {
        move |p: ProgressInfo| self.link_send(LinkEvent::Progress(ProgressSnapshot::from(p)))
    }

    /// What sharing `instance` would send, offline first, then checked with Modrinth.
    pub fn link_build(&self, instance: Instance, include_configs: bool, last: Option<LinkState>) {
        self.run(move |t| {
            let result = linked::build_checked(t.dirs(), &instance, include_configs)
                .map(|built| {
                    let changes = last
                        .and_then(|s| s.manifest)
                        .map(|old| linked::changes(&old, &built.manifest));
                    (built, changes)
                })
                .map_err(|e| e.to_string());
            t.link_send(LinkEvent::Built(instance.id, result));
        });
    }

    /// Make a link for `instance` (a new code), or publish it again when it has one.
    pub fn link_publish(
        &self,
        instance: Instance,
        include_configs: bool,
        account: Option<Account>,
    ) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = if linked::load(t.dirs(), &instance.id).is_some_and(|s| s.is_owner()) {
                linked::publish(t.dirs(), &base, &instance, include_configs)
            } else {
                // An Arctic session helps the server against abuse; a link works without.
                let token = account.and_then(|a| {
                    let a = t.fresh_account(a).ok()?;
                    cosmetics::token_for(t.dirs(), &base, &a).ok()
                });
                linked::share(
                    t.dirs(),
                    &base,
                    token.as_deref(),
                    &instance,
                    include_configs,
                )
            };
            t.link_send(LinkEvent::Published(
                instance.id,
                result.map_err(|e| e.to_string()),
            ));
        });
    }

    pub fn link_stop(&self, instance_id: String) {
        self.run(move |t| {
            let result = linked::stop_sharing(t.dirs(), &cosmetics::base_url(), &instance_id)
                .map_err(|e| e.to_string());
            t.link_send(LinkEvent::Stopped(instance_id, result));
        });
    }

    /// Ask the server about each friend's copy.
    pub fn link_check(&self, ids: Vec<String>) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let mut states = Vec::new();
            for id in ids {
                match linked::check(t.dirs(), &base, &id) {
                    Ok(state) => states.push((id, state)),
                    Err(e) => log::info!("linked: checking {id}: {e}"),
                }
            }
            t.link_send(LinkEvent::Checked(states));
        });
    }

    /// Fetch a link by code for a new copy.
    pub fn link_preview(&self, code: String) {
        self.run(move |t| {
            let result = linked::preview(&cosmetics::base_url(), &code)
                .map(|(f, d)| (code, f, d))
                .map_err(|e| e.to_string());
            t.link_send(LinkEvent::Preview(result));
        });
    }

    /// Fetch the owner's newest revision and what it would change in a copy.
    pub fn link_review(&self, instance: Instance) {
        self.run(move |t| {
            let result = linked::review(t.dirs(), &cosmetics::base_url(), &instance)
                .map_err(|e| e.to_string());
            t.link_send(LinkEvent::Review(instance.id, result));
        });
    }

    /// Make a new linked copy with the accepted part of `diff`.
    pub fn link_follow(&self, code: String, fetched: linked::server::Fetched, diff: Diff) {
        self.run(move |t| {
            let progress = t.link_progress();
            match linked::follow(t.dirs(), &code, &fetched, &diff, &progress) {
                Ok((instance, report)) => {
                    t.link_send(LinkEvent::Applied(Some(instance.id), Ok(report)))
                }
                Err(e) => t.link_send(LinkEvent::Applied(None, Err(e.to_string()))),
            }
        });
    }

    /// Apply the accepted part of an update to a copy.
    pub fn link_take(&self, instance: Instance, fetched: linked::server::Fetched, diff: Diff) {
        self.run(move |t| {
            let progress = t.link_progress();
            let result = linked::take(t.dirs(), &instance, &fetched, &diff, &progress)
                .map_err(|e| e.to_string());
            t.link_send(LinkEvent::Applied(Some(instance.id), result));
        });
    }

    /// Look up the mods a server said a game lacks.
    pub fn link_find_missing(
        &self,
        run: u64,
        server: String,
        instance: Instance,
        mods: Vec<WantedMod>,
    ) {
        self.run(move |t| {
            let result =
                linked::find_missing(t.dirs(), &instance, &mods).map_err(|e| e.to_string());
            t.link_send(LinkEvent::MissingFound(run, server, result));
        });
    }

    /// Install the accepted missing mods.
    pub fn link_install_missing(&self, instance: Instance, diff: Diff) {
        self.run(move |t| {
            let progress = t.link_progress();
            let result: Result<Report> =
                linked::install_missing(t.dirs(), &instance, &diff, &progress);
            t.link_send(LinkEvent::Applied(None, result.map_err(|e| e.to_string())));
        });
    }
}
