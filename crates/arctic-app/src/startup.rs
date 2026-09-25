//! Command-line flags of the GUI, for shortcuts and `arctic open`:
//!
//! ```text
//! arctic-launcher.exe [--launch <version|latest>] [--account <name|uuid>]
//!                     [--tab <play|accounts|instances|skins|together|logs|settings|about>] [--no-intro]
//!                     [--profile <name|id>]
//! ```
//! Unknown flags are ignored so old shortcuts never stop the app starting.

use crate::app::Tab;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartupOptions {
    /// Launch this version as soon as the version list is loaded.
    pub launch: Option<String>,
    /// Account to make active first (username, UUID or id).
    pub account: Option<String>,
    pub tab: Option<Tab>,
    pub no_intro: bool,
    /// Profile to open (name or id).
    pub profile: Option<String>,
}

impl StartupOptions {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut opts = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--launch" => opts.launch = args.next(),
                "--account" => opts.account = args.next(),
                "--tab" => opts.tab = args.next().as_deref().and_then(parse_tab),
                "--no-intro" => opts.no_intro = true,
                "--profile" => opts.profile = args.next(),
                other => log::warn!("ignoring unknown argument {other}"),
            }
        }
        opts
    }
}

fn parse_tab(name: &str) -> Option<Tab> {
    match name.to_ascii_lowercase().as_str() {
        "play" => Some(Tab::Play),
        "accounts" => Some(Tab::Accounts),
        "instances" => Some(Tab::Instances),
        "skins" => Some(Tab::Skins),
        "together" => Some(Tab::Together),
        "logs" => Some(Tab::Logs),
        "settings" => Some(Tab::Settings),
        "about" => Some(Tab::About),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> StartupOptions {
        StartupOptions::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_all_flags() {
        let o = parse(&[
            "--launch",
            "1.21",
            "--account",
            "Steve",
            "--tab",
            "Logs",
            "--no-intro",
        ]);
        assert_eq!(o.launch.as_deref(), Some("1.21"));
        assert_eq!(o.account.as_deref(), Some("Steve"));
        assert_eq!(o.tab, Some(Tab::Logs));
        assert!(o.no_intro);
    }

    #[test]
    fn tolerates_junk() {
        let o = parse(&["--bogus", "--tab", "nope", "--launch"]);
        assert_eq!(o, StartupOptions::default());
    }
}
