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
        Err(Error::NotConfigured(format!(
            "Microsoft login is not configured. Set {CLIENT_ID_ENV} or create {} \
             with {{\"client_id\": \"…\"}} (see docs/microsoft-auth.md).",
            dirs.msa_config_file().display()
        )))
    }
}
