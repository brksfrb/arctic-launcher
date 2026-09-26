use serde::{Deserialize, Serialize};

use crate::storage::{DataDirs, load_json};
use crate::{Error, Result};

/// Runtime override for the Azure application (client) ID.
pub const CLIENT_ID_ENV: &str = "ARCTIC_MSA_CLIENT_ID";

/// Public-client configuration. The client ID is not a secret (it ships in
/// every build), but it is kept out of git so forks register their own app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsaConfig {
    pub client_id: String,
}

impl MsaConfig {
    /// Older "title" client IDs (16 hex digits starting with zeros) sign in
    /// through login.live.com with the Xbox service scope instead of the
    /// Microsoft identity platform.
    pub fn is_live(&self) -> bool {
        let id = self.client_id.trim();
        id.len() == 16 && id.starts_with("00000000") && id.bytes().all(|b| b.is_ascii_hexdigit())
    }

    /// Lookup order: `ARCTIC_MSA_CLIENT_ID` env var → `<data>/msa.json` →
    /// value baked in at compile time (CI release builds set the same env var).
    pub fn load(dirs: &DataDirs) -> Result<Self> {
        if let Some(id) = std::env::var(CLIENT_ID_ENV)
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            return Ok(Self {
                client_id: id.trim().to_owned(),
            });
        }
        if let Some(cfg) = load_json::<Self>(&dirs.msa_config_file())?
            && !cfg.client_id.trim().is_empty()
        {
            return Ok(cfg);
        }
        if let Some(id) = option_env!("ARCTIC_MSA_CLIENT_ID").filter(|s| !s.is_empty()) {
            return Ok(Self {
                client_id: id.to_owned(),
            });
        }
        log::debug!(
            "no Microsoft client ID: set {CLIENT_ID_ENV} or {} (see docs/building.md)",
            dirs.msa_config_file().display()
        );
        Err(Error::NotConfigured(
            "Microsoft sign-in isn't available in this build of Arctic Launcher.".into(),
        ))
    }
}
