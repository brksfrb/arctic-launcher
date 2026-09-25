//! Arctic Launcher core library.
//!
//! Everything that is not UI lives here so it can be unit-tested and reused
//! (e.g. by a future CLI or Linux build). The egui app in `arctic-app` only
//! drives these APIs from background threads.

pub mod auth;
pub mod error;
pub mod instances;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod mods;
pub mod net;
pub mod profiles;
pub mod settings;
pub mod skins;
pub mod storage;
pub mod system;
pub mod update;
pub mod versions;

pub use error::{Error, Result};

/// Human-readable product name.
pub const APP_NAME: &str = "Arctic Launcher";
/// Crate version, used for update checks and `${launcher_version}`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Brand passed to the game as `${launcher_name}`.
pub const LAUNCHER_BRAND: &str = "arctic-launcher";

/// Snapshot of a long-running operation for progress UIs. `total == 0`
/// means "indeterminate".
#[derive(Debug, Clone, Copy, Default)]
pub struct ProgressInfo<'a> {
    pub stage: &'a str,
    /// Files (or steps) completed / total.
    pub done: u64,
    pub total: u64,
    /// Bytes downloaded / expected (0 when unknown).
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl<'a> ProgressInfo<'a> {
    /// A step without measurable progress (e.g. "Refreshing login").
    pub fn stage(stage: &'a str) -> Self {
        Self {
            stage,
            ..Self::default()
        }
    }
}

/// Progress callback shared by long-running operations. Called from worker
/// threads, at most ~20 times per second per operation.
pub type Progress<'a> = &'a (dyn Fn(ProgressInfo<'_>) + Sync);
