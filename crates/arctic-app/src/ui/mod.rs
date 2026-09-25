//! Tab contents and window chrome. Each module adds methods to `ArcticApp`.

mod about;
mod accounts;
mod dialogs;
mod instances;
mod logs;

pub use logs::LogViewKey;
pub use profiles::ProfileDialog;
mod nav;
mod play;
mod profiles;
mod settings;
mod shell;
