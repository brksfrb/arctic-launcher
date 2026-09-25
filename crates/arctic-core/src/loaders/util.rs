//! Helpers shared by every loader: version ordering, URL building and
//! reading/writing loader profiles.

use std::cmp::Ordering;
use std::path::PathBuf;

use serde_json::Value;

use crate::storage::{DataDirs, load_json, save_json};
use crate::versions::VersionJson;
use crate::{Error, Result};

/// Compare two version strings the way loader metadata expects:
/// dot/number aware (`1.21.10` > `1.21.9`), and a release sorts after its
/// own pre-releases (`21.0.0` > `21.0.0-beta`, `26.1` > `26.1-snapshot-1`).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (a_core, a_pre) = split_pre(a);
    let (b_core, b_pre) = split_pre(b);
    compare_natural(a_core, b_core).then_with(|| match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => compare_natural(x, y),
    })
}

/// Sort newest first.
pub fn sort_newest_first(versions: &mut [String]) {
    versions.sort_by(|a, b| compare_versions(b, a));
}

fn split_pre(v: &str) -> (&str, Option<&str>) {
    match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Token<'a> {
    // Declaration order matters: text sorts before numbers so `1.0.a` < `1.0.1`.
    Text(&'a str),
    Num(u64),
}

fn tokens(s: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let is_digit = c.is_ascii_digit();
        let end = rest
            .find(|ch: char| ch.is_ascii_digit() != is_digit)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        if is_digit {
            out.push(Token::Num(run.parse().unwrap_or(u64::MAX)));
        } else {
            let text = run.trim_matches(|ch| ch == '.' || ch == '+' || ch == '_');
            if !text.is_empty() {
                out.push(Token::Text(text));
            }
        }
        rest = tail;
    }
    out
}

fn compare_natural(a: &str, b: &str) -> Ordering {
    tokens(a).cmp(&tokens(b))
}

/// Percent-encode one URL path segment (game versions such as
/// `1.14 Pre-Release 1` contain spaces).
pub fn encode_segment(segment: &str) -> String {
    segment
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'+' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `versions/<id>/<id>.json`
pub fn profile_path(dirs: &DataDirs, id: &str) -> PathBuf {
    dirs.version_dir(id).join(format!("{id}.json"))
}

/// Parse a loader profile. Forge ships `"logging": {}`, which would hide
/// vanilla's logging config after merging, so empty blocks are dropped.
pub fn parse_profile(raw: &Value) -> Result<VersionJson> {
    let mut value = raw.clone();
    if let Some(obj) = value.as_object_mut() {
        let empty_logging = obj
            .get("logging")
            .is_some_and(|l| l.get("client").is_none());
        if empty_logging {
            obj.remove("logging");
        }
    }
    serde_json::from_value(value)
        .map_err(|e| Error::Other(format!("the loader profile could not be read: {e}")))
}

/// Profile id of a raw profile document.
pub fn profile_id(raw: &Value) -> Result<String> {
    raw.get("id")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("the loader profile has no valid id".into()))
}

/// Ids become directory names; refuse anything that could escape `versions/`.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains(['/', '\\', ':'])
        && id != "."
        && id != ".."
        && !id.starts_with('.')
}

/// Save a raw profile under `versions/<id>/<id>.json` and return its id.
pub fn save_profile(dirs: &DataDirs, raw: &Value) -> Result<String> {
    let id = profile_id(raw)?;
    save_json(&profile_path(dirs, &id), raw)?;
    Ok(id)
}

/// A previously saved profile, if present and readable.
pub fn load_saved_profile(dirs: &DataDirs, id: &str) -> Option<VersionJson> {
    match load_json::<Value>(&profile_path(dirs, id)) {
        Ok(Some(raw)) => parse_profile(&raw).ok(),
        Ok(None) => None,
        Err(e) => {
            log::warn!("ignoring unreadable loader profile {id}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(list: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = list.iter().map(|s| s.to_string()).collect();
        sort_newest_first(&mut v);
        v
    }

    #[test]
    fn orders_numbers_and_prereleases() {
        assert_eq!(
            sorted(&["0.9.1", "0.10.0", "0.10.0-beta.2", "0.10.0-beta.10"]),
            ["0.10.0", "0.10.0-beta.10", "0.10.0-beta.2", "0.9.1"]
        );
        assert_eq!(
            sorted(&[
                "1.21.9",
                "1.21.10",
                "26.1-snapshot-1",
                "26.1",
                "26.1.1",
                "1.7.10"
            ]),
            [
                "26.1.1",
                "26.1",
                "26.1-snapshot-1",
                "1.21.10",
                "1.21.9",
                "1.7.10"
            ]
        );
        assert_eq!(
            compare_versions("0.7.2+build.175", "0.16.10"),
            Ordering::Less
        );
    }

    #[test]
    fn encodes_path_segments() {
        assert_eq!(
            encode_segment("1.14 Pre-Release 1"),
            "1.14%20Pre-Release%201"
        );
        assert_eq!(encode_segment("1.21.4"), "1.21.4");
    }

    #[test]
    fn drops_empty_forge_logging_block() {
        let raw: Value = serde_json::from_str(
            r#"{"id":"1.12.2-forge","mainClass":"M","logging":{},"inheritsFrom":"1.12.2"}"#,
        )
        .unwrap();
        let v = parse_profile(&raw).unwrap();
        assert!(v.logging.is_none());
        assert_eq!(profile_id(&raw).unwrap(), "1.12.2-forge");
    }

    #[test]
    fn rejects_path_like_ids() {
        for bad in ["", "..", "../x", "a/b", "a\\b", ".hidden", "c:x"] {
            let raw = serde_json::json!({ "id": bad });
            assert!(profile_id(&raw).is_err(), "{bad}");
        }
    }
}
