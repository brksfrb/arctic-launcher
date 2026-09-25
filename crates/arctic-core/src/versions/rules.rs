//! Mojang's rule system used by libraries and arguments.
//!
//! Semantics: with no rules, allowed. Otherwise start at "disallow" and every
//! rule that matches the environment overwrites the result with its action.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// Regex on the OS version. Not evaluated yet (TODO); treated as matching,
    /// which is correct for the only current use (Windows 10+ flags).
    pub version: Option<String>,
}

/// Environment the rules are evaluated against.
#[derive(Debug, Clone)]
pub struct RuleEnv {
    /// Mojang OS name: `windows`, `osx`, `linux`.
    pub os: &'static str,
    /// Mojang arch name: `x86` (32-bit), `x86_64`, `arm64`.
    pub arch: &'static str,
    pub features: HashSet<&'static str>,
}

impl RuleEnv {
    /// The host platform with no optional features enabled.
    pub fn current() -> Self {
        Self {
            os: current_os(),
            arch: current_arch(),
            features: HashSet::new(),
        }
    }

    pub fn with_feature(mut self, feature: &'static str) -> Self {
        self.features.insert(feature);
        self
    }
}

pub fn current_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

impl Rule {
    fn matches(&self, env: &RuleEnv) -> bool {
        let os_ok = self.os.as_ref().is_none_or(|os| {
            os.name.as_deref().is_none_or(|n| n == env.os)
                && os.arch.as_deref().is_none_or(|a| a == env.arch)
        });
        let features_ok = self.features.as_ref().is_none_or(|f| {
            f.iter()
                .all(|(name, want)| env.features.contains(name.as_str()) == *want)
        });
        os_ok && features_ok
    }
}

pub fn rules_allow(rules: Option<&[Rule]>, env: &RuleEnv) -> bool {
    let Some(rules) = rules.filter(|r| !r.is_empty()) else {
        return true;
    };
    rules
        .iter()
        .filter(|r| r.matches(env))
        .fold(RuleAction::Disallow, |_, r| r.action)
        == RuleAction::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(os: &'static str) -> RuleEnv {
        RuleEnv {
            os,
            arch: "x86_64",
            features: HashSet::new(),
        }
    }

    fn parse(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn no_rules_allows() {
        assert!(rules_allow(None, &env("windows")));
        assert!(rules_allow(Some(&[]), &env("windows")));
    }

    #[test]
    fn allow_all_except_osx() {
        let rules = parse(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert!(rules_allow(Some(&rules), &env("windows")));
        assert!(!rules_allow(Some(&rules), &env("osx")));
    }

    #[test]
    fn only_on_windows() {
        let rules = parse(r#"[{"action":"allow","os":{"name":"windows"}}]"#);
        assert!(rules_allow(Some(&rules), &env("windows")));
        assert!(!rules_allow(Some(&rules), &env("linux")));
    }

    #[test]
    fn arch_x86_is_32_bit_only() {
        let rules = parse(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert!(!rules_allow(Some(&rules), &env("windows")));
    }

    #[test]
    fn features_must_match() {
        let rules = parse(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
        assert!(!rules_allow(Some(&rules), &env("windows")));
        let with = env("windows").with_feature("has_custom_resolution");
        assert!(rules_allow(Some(&rules), &with));
    }
}
