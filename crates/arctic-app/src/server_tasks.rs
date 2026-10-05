//! Background jobs for the Play tab's servers: read the instance's server
//! list, then ping each one (results arrive as they come in).

use std::path::PathBuf;

use arctic_core::proxy::ProxySettings;
use arctic_core::servers;

use crate::tasks::{Event, Tasks};

impl Tasks {
    /// `request` tells answers for an older refresh (another instance) apart.
    pub fn servers_refresh(&self, request: u64, game_dir: PathBuf) {
        self.run(move |t| {
            let list = servers::list(&game_dir).map_err(|e| e.to_string());
            let addresses: Vec<String> = list
                .as_ref()
                .map(|l| l.iter().map(|s| s.address.clone()).collect())
                .unwrap_or_default();
            t.send(Event::ServerList(request, list));
            let proxy = ProxySettings::load(t.dirs());
            let proxy = proxy.active();
            servers::ping_each(&addresses, proxy, |i, status| {
                let status = status.map_err(|e| e.to_string());
                t.send(Event::ServerStatus(request, addresses[i].clone(), status));
            });
        });
    }
}

impl Tasks {
    /// Ping one typed-in address for the "Direct join" box.
    pub fn direct_ping(&self, address: String) {
        self.run(move |t| {
            let proxy = ProxySettings::load(t.dirs());
            let status = servers::ping(&address, proxy.active()).map_err(|e| e.to_string());
            t.send(Event::DirectPing(address, status));
        });
    }
}

impl Tasks {
    /// The public server list (shuffled by the Arctic server).
    pub fn public_servers(&self) {
        self.run(|t| {
            let result = servers::public::browse(&arctic_core::cosmetics::base_url())
                .map_err(|e| e.to_string());
            t.send(Event::PublicServers(result));
        });
    }

    /// Submit the player's own server; the answer holds the MOTD code.
    pub fn public_submit(&self, account: arctic_core::auth::Account, form: SubmitForm) {
        self.run(move |t| {
            let result = (|| {
                let account = t.fresh_account(account)?;
                let base = arctic_core::cosmetics::base_url();
                let token = arctic_core::cosmetics::token_for(t.dirs(), &base, &account)?;
                servers::public::submit(
                    &base,
                    &token,
                    &form.address,
                    &form.name,
                    &form.description,
                    &form.tags,
                )
            })()
            .map_err(|e| e.to_string());
            t.send(Event::ServerSubmitted(result));
        });
    }

    /// Ask the Arctic server to find the code in the MOTD.
    pub fn public_verify(&self, account: arctic_core::auth::Account, id: String) {
        self.run(move |t| {
            let result = (|| {
                let account = t.fresh_account(account)?;
                let base = arctic_core::cosmetics::base_url();
                let token = arctic_core::cosmetics::token_for(t.dirs(), &base, &account)?;
                servers::public::verify(&base, &token, &id)
            })()
            .map_err(|e| e.to_string());
            t.send(Event::ServerVerified(result));
        });
    }
}

/// What an owner fills in to list their server.
#[derive(Debug, Clone, Default)]
pub struct SubmitForm {
    pub address: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}
