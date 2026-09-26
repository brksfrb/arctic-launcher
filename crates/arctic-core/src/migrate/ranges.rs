//! Which Minecraft versions a mod jar says it runs on, read the way the
//! loaders read it (`fabric.mod.json` predicates, `mods.toml` Maven
//! ranges), so a mod that would stop the game can come over switched off.

use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::loaders::LoaderKind;

/// `Some(false)` when the jar says it can't run on `game`; `None` when it
/// doesn't say or can't be read (it's then kept on).
pub fn game_fits(jar: &Path, game: &str, loader: LoaderKind) -> Option<bool> {
    let game = numbers(game)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut read = |name: &str| {
        let mut s = String::new();
        zip.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    // The loader's own description (a jar can carry one per loader).
    let (file, text) = super::detect::metadata_files(loader)
        .iter()
        .find_map(|f| read(f).map(|t| (*f, t)))?;
    if file.ends_with(".json") {
        let json: Value = serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok()?;
        return match json.pointer("/depends/minecraft")? {
            Value::String(s) => fabric_matches(s, &game),
            Value::Array(list) => {
                let results: Vec<Option<bool>> = list
                    .iter()
                    .map(|v| v.as_str().and_then(|s| fabric_matches(s, &game)))
                    .collect();
                if results.contains(&Some(true)) {
                    Some(true)
                } else if results.iter().all(|r| *r == Some(false)) {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        };
    }
    if file == "mcmod.info" {
        return None;
    }
    maven_matches(&minecraft_range(&text)?, &game)
}

/// The numbers of a release version (`1.21.4` → [1, 21, 4]); `None` for
/// snapshots and anything else that doesn't order simply.
fn numbers(v: &str) -> Option<Vec<u64>> {
    let core = v.split(['-', '+']).next()?;
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

/// Compare with missing trailing parts as zero (`1.21` == `1.21.0`).
fn cmp(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    let at = |v: &[u64], i: usize| v.get(i).copied().unwrap_or(0);
    (0..len)
        .map(|i| at(a, i).cmp(&at(b, i)))
        .find(|o| o.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}

/// A Fabric version predicate: space-separated comparisons that must all
/// hold (`>=1.21.8 <1.21.9`, `~1.20`, `1.21.x`, `*`).
fn fabric_matches(pred: &str, game: &[u64]) -> Option<bool> {
    let mut all = true;
    for token in pred.split_whitespace() {
        all &= fabric_token(token, game)?;
    }
    Some(all)
}

fn fabric_token(token: &str, game: &[u64]) -> Option<bool> {
    use std::cmp::Ordering::{Equal, Greater, Less};
    if token == "*" {
        return Some(true);
    }
    let (op, v) = [">=", "<=", ">", "<", "=", "~", "^"]
        .iter()
        .find_map(|op| token.strip_prefix(op).map(|rest| (*op, rest)))
        .unwrap_or(("", token));
    if v.contains(['x', 'X', '*']) {
        let fixed: Option<Vec<u64>> = v
            .split('.')
            .take_while(|p| !matches!(*p, "x" | "X" | "*"))
            .map(|p| p.parse().ok())
            .collect();
        let fixed = fixed?;
        return Some(game.len() >= fixed.len() && game[..fixed.len()] == fixed[..]);
    }
    let want = numbers(v)?;
    let o = cmp(game, &want);
    Some(match op {
        ">=" => o != Less,
        "<=" => o != Greater,
        ">" => o == Greater,
        "<" => o == Less,
        "~" => {
            // Same major.minor, at least this version.
            let mut next = want.clone();
            next.truncate(2.min(next.len()).max(1));
            if let Some(last) = next.last_mut() {
                *last += 1;
            }
            o != Less && cmp(game, &next) == Less
        }
        "^" => {
            let next = vec![want.first().copied().unwrap_or(0) + 1];
            o != Less && cmp(game, &next) == Less
        }
        _ => o == Equal,
    })
}

/// `versionRange` of the `minecraft` dependency in a `mods.toml`.
fn minecraft_range(toml: &str) -> Option<String> {
    let mut in_minecraft = false;
    for line in toml.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with("[[") {
            in_minecraft = false;
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_matches('"');
        match k.trim() {
            "modId" => in_minecraft = v == "minecraft",
            "versionRange" if in_minecraft => return Some(v.to_owned()),
            _ => {}
        }
    }
    None
}

/// A Maven range: `[1.21.8,1.21.9)`, `[1.20.1]`, `[1.20,)`; a bare version
/// is only a preference (any version loads).
fn maven_matches(range: &str, game: &[u64]) -> Option<bool> {
    use std::cmp::Ordering::{Greater, Less};
    let r = range.trim();
    if r.is_empty() || r == "*" || !r.starts_with(['[', '(']) {
        return Some(true);
    }
    // Several ranges (`[1.20,1.20.1],[1.21,)`): any of them.
    let mut any = false;
    for part in split_ranges(r) {
        let lo_inclusive = part.starts_with('[');
        let hi_inclusive = part.ends_with(']');
        let inner = part.get(1..part.len().saturating_sub(1))?;
        let (lo, hi) = match inner.split_once(',') {
            Some((lo, hi)) => (lo.trim(), hi.trim()),
            None => (inner.trim(), inner.trim()),
        };
        let lo_ok = lo.is_empty() || {
            let o = cmp(game, &numbers(lo)?);
            if lo_inclusive {
                o != Less
            } else {
                o == Greater
            }
        };
        let hi_ok = hi.is_empty() || {
            let o = cmp(game, &numbers(hi)?);
            if hi_inclusive {
                o != Greater
            } else {
                o == Less
            }
        };
        any |= lo_ok && hi_ok;
    }
    Some(any)
}

fn split_ranges(r: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in r.char_indices() {
        if c == ']' || c == ')' {
            out.push(r[start..=i].trim_start_matches(','));
            start = i + 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<u64> {
        numbers(s).unwrap()
    }

    #[test]
    fn fabric_predicates() {
        assert_eq!(fabric_matches(">=1.21.8 <1.21.9", &v("1.21.8")), Some(true));
        assert_eq!(
            fabric_matches(">=1.21.8 <1.21.9", &v("1.21.4")),
            Some(false)
        );
        assert_eq!(fabric_matches("~1.20", &v("1.20.4")), Some(true));
        assert_eq!(fabric_matches("~1.20", &v("1.21")), Some(false));
        assert_eq!(fabric_matches("1.21.x", &v("1.21.11")), Some(true));
        assert_eq!(fabric_matches("1.21.x", &v("1.20.6")), Some(false));
        assert_eq!(fabric_matches("*", &v("1.8.9")), Some(true));
        assert_eq!(fabric_matches("1.21.11", &v("1.21.11")), Some(true));
        assert_eq!(fabric_matches(">=1.21-", &v("1.21.4")), Some(true));
        assert_eq!(fabric_matches(">=1.21-alpha.1", &v("26.3")), Some(true));
    }

    #[test]
    fn maven_ranges() {
        assert_eq!(maven_matches("[1.21.8,1.21.9)", &v("1.21.4")), Some(false));
        assert_eq!(maven_matches("[1.21.8,1.21.9)", &v("1.21.8")), Some(true));
        assert_eq!(maven_matches("[1.20.1]", &v("1.20.1")), Some(true));
        assert_eq!(maven_matches("[1.20,)", &v("1.21.4")), Some(true));
        assert_eq!(maven_matches("1.20.1", &v("1.18.2")), Some(true));
        assert_eq!(
            maven_matches("[1.19,1.19.2],[1.20,)", &v("1.19.4")),
            Some(false)
        );
        assert_eq!(
            maven_matches("[1.19,1.19.2],[1.20,)", &v("1.20.1")),
            Some(true)
        );
    }

    #[test]
    fn finds_the_minecraft_range() {
        let toml = r#"
[[dependencies."peak"]]
modId = "neoforge" #mandatory
versionRange = "[21.8,)"
[[dependencies."peak"]]
modId = "minecraft"
# a comment
versionRange = "[1.21.8,1.21.9)"
"#;
        assert_eq!(minecraft_range(toml).as_deref(), Some("[1.21.8,1.21.9)"));
    }

    #[test]
    fn snapshots_are_not_judged() {
        assert!(numbers("25w14a").is_none());
    }
}
