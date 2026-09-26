//! Tab contents and window chrome. Each module adds methods to `ArcticApp`.

mod about;
mod accounts;
mod client_style;
mod dialogs;
mod instances;
mod logs;

pub use instances::{InstancePage, InstancesUi};
pub use logs::LogViewKey;
pub use profiles::ProfileDialog;
mod nav;
mod network;
pub use network::NetworkUi;
mod onboarding;
pub use onboarding::Onboarding;
mod play;
pub use play::VersionView;
mod migrate;
mod profiles;
pub use migrate::MigrateUi;
mod settings;
mod sharing;
pub use sharing::ShareUi;
mod skins;
pub use skins::SkinsUi;
mod together;
pub use together::TogetherUi;
mod shell;
