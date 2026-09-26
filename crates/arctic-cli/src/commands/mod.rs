//! Command implementations. Each returns the process exit code.

pub mod accounts;
pub mod instances;
pub mod launch;
pub mod looks;
pub mod migrate;
pub mod misc;
pub mod mods;
pub mod network;
pub mod packs;
pub mod profiles;
pub mod settings;
pub mod sharing;
pub mod together;
pub mod versions;
pub mod worlds;

use arctic_core::auth::{self, Account, AccountStore};
use arctic_core::instances::Instance;
use arctic_core::storage::DataDirs;
use arctic_core::versions::{VersionEntry, VersionManifest};
use arctic_core::{Error, Result};

use crate::output::Out;

/// Shared state for a command run.
pub struct Ctx {
    /// Launcher-wide layout (profiles.json, shared downloads).
    pub root: DataDirs,
    /// Layout scoped to the selected profile.
    pub dirs: DataDirs,
    pub out: Out,
}

/// The instance matching `query`, or Vanilla.
pub fn instance_or_default(ctx: &Ctx, query: Option<&str>) -> Result<Instance> {
    match query {
        Some(q) => arctic_core::instances::find(&ctx.dirs, q),
        None => arctic_core::instances::load_default(&ctx.dirs),
    }
}

/// The account matching `query` (or the active one), with its Microsoft
/// session refreshed and saved if it had expired.
pub fn fresh_account(ctx: &Ctx, query: Option<&str>) -> Result<Account> {
    let mut store = AccountStore::load(&ctx.dirs)?;
    let account = match query {
        Some(query) => store.find(query).cloned().ok_or_else(|| {
            Error::Other(format!(
                "no account matches '{query}' (see `arctic accounts list`)"
            ))
        })?,
        None => store.active().cloned().ok_or_else(|| {
            Error::Other(
                "no account selected: use --account NAME or sign in with `arctic accounts login`"
                    .into(),
            )
        })?,
    };
    let (fresh, changed) = auth::ensure_fresh(&ctx.dirs, &account)?;
    if changed {
        store.upsert(fresh.clone());
        store.save(&ctx.dirs)?;
    }
    Ok(fresh)
}

/// `latest`, `latest-snapshot`, or an exact version id.
pub fn resolve_version(manifest: &VersionManifest, query: &str) -> Result<VersionEntry> {
    let id = match query {
        "latest" | "latest-release" => manifest.latest.release.as_str(),
        "latest-snapshot" => manifest.latest.snapshot.as_str(),
        other => other,
    };
    manifest.find(id).cloned().ok_or_else(|| {
        Error::Other(format!(
            "unknown version '{query}' (see `arctic versions --all`)"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aliases() {
        let manifest: VersionManifest = serde_json::from_str(
            r#"{"latest":{"release":"1.21","snapshot":"24w01a"},"versions":[
                {"id":"24w01a","type":"snapshot","url":"u","releaseTime":"t","sha1":"s"},
                {"id":"1.21","type":"release","url":"u","releaseTime":"t","sha1":"s"}]}"#,
        )
        .unwrap();
        assert_eq!(resolve_version(&manifest, "latest").unwrap().id, "1.21");
        assert_eq!(
            resolve_version(&manifest, "latest-snapshot").unwrap().id,
            "24w01a"
        );
        assert_eq!(resolve_version(&manifest, "1.21").unwrap().id, "1.21");
        assert!(resolve_version(&manifest, "9.9").is_err());
    }
}
