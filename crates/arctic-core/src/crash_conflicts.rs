//! Crashes caused by mods that cannot be installed together: a mod and its fork, two jars of one
//! mod, or mods that declare each other incompatible. Builds on `crash` (the log rules) by also
//! looking at the instance's mods, and names the mod whose mixin failed.

use std::cmp::Ordering;
use std::path::Path;

use regex::Regex;

use crate::crash::{Diagnosis, Fix};

/// Mods that are forks of one another and break when both are installed: (original, fork).
///
/// Mods say what they cannot run with in `breaks`/`conflicts` (read below), which is preferred;
/// this list is only for pairs that declare nothing. Keep it small and give the source.
/// - noisium / noisiumed: Noisiumed's Modrinth page says it is "a maintained fork of Noisium";
///   neither jar declares the other in `breaks` (checked with Noisium 2.3.0 and Noisiumed 3.0.6),
///   and with both installed Noisiumed's Lithium compat mixin fails (crash report #267).
const KNOWN_FORKS: &[(&str, &str)] = &[("noisium", "noisiumed")];

/// A mod found in the instance's mods folder (or in a crash report's mod list).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstalledMod {
    /// The jar's file name; empty when read from a crash report.
    pub file: String,
    pub id: String,
    pub name: String,
    pub version: String,
    /// Mixin config files (`*.mixins.json`) the mod brings.
    pub mixins: Vec<String>,
    /// Mods (id, accepted version ranges) this one says it cannot run with.
    pub breaks: Vec<(String, Vec<String>)>,
}

/// Why two installed mods clash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// `b` is a fork of `a`.
    Fork,
    /// The same mod id twice.
    SameId,
    /// `a` declares that it breaks/conflicts with `b`.
    Declared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub a: usize,
    pub b: usize,
    pub why: Why,
}

/// The enabled mods in `mods_dir`. The launcher's own files are left out (never offered to switch off).
pub fn scan(mods_dir: &Path) -> Vec<InstalledMod> {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let mut out: Vec<InstalledMod> = entries
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let skip = !file.to_lowercase().ends_with(".jar")
                || file == crate::arctic_mod::FILE_NAME
                || file == crate::mods::polarium::FILE_NAME;
            if skip { None } else { read_jar(&e.path()) }
        })
        .collect();
    out.sort_by(|x, y| x.file.cmp(&y.file));
    out
}

/// A jar's mod from its `fabric.mod.json` or (Forge/NeoForge) `mods.toml`.
pub fn read_jar(jar: &Path) -> Option<InstalledMod> {
    let file = jar.file_name()?.to_string_lossy().into_owned();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut read = |name: &str| {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(name).ok()?, &mut text).ok()?;
        Some(text)
    };
    if let Some(text) = read("fabric.mod.json") {
        return from_fabric_json(&file, &text);
    }
    let toml = read("META-INF/neoforge.mods.toml").or_else(|| read("META-INF/mods.toml"))?;
    from_mods_toml(&file, &toml)
}

fn from_fabric_json(file: &str, text: &str) -> Option<InstalledMod> {
    // Some mods put raw newlines inside strings; lenient enough for what is read here.
    let json: serde_json::Value =
        serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok()?;
    let id = json.get("id")?.as_str()?.to_owned();
    let text_of = |key: &str| json.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    let mut mixins = Vec::new();
    for m in json
        .get("mixins")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
    {
        // Either "x.mixins.json" or {"config": "x.mixins.json", "environment": ...}.
        if let Some(s) = m
            .as_str()
            .or_else(|| m.get("config").and_then(|c| c.as_str()))
        {
            mixins.push(s.to_owned());
        }
    }
    let mut breaks = Vec::new();
    for key in ["breaks", "conflicts"] {
        let Some(map) = json.get(key).and_then(|b| b.as_object()) else {
            continue;
        };
        for (other, ranges) in map {
            let ranges = match ranges {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(a) => a
                    .iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect(),
                _ => continue,
            };
            breaks.push((other.to_lowercase(), ranges));
        }
    }
    Some(InstalledMod {
        file: file.to_owned(),
        name: text_of("name").unwrap_or_else(|| id.clone()),
        version: text_of("version").unwrap_or_default(),
        id,
        mixins,
        breaks,
    })
}

fn from_mods_toml(file: &str, text: &str) -> Option<InstalledMod> {
    let value = |key: &str| {
        Regex::new(&format!(r#"(?m)^\s*{key}\s*=\s*"([^"]*)""#))
            .ok()?
            .captures(text)
            .map(|c| c[1].to_owned())
    };
    let id = value("modId")?;
    let mixins = Regex::new(r#"(?m)^\s*config\s*=\s*"([^"]+\.mixins\.json)""#)
        .ok()?
        .captures_iter(text)
        .map(|c| c[1].to_owned())
        .collect();
    Some(InstalledMod {
        file: file.to_owned(),
        name: value("displayName").unwrap_or_else(|| id.clone()),
        version: value("version").unwrap_or_default(),
        id,
        mixins,
        breaks: Vec::new(),
    })
}

/// The mods a crash report lists ("\t\tnoisium: Noisium 2.7.0+mc1.21-1.21.1"): ids, names and
/// versions only. A fallback for when the instance's mods folder can't be read.
pub fn from_crash_report(text: &str) -> Vec<InstalledMod> {
    let Ok(re) = Regex::new(r"(?m)^\t+([a-z][a-z0-9_\-]*): (.+) (\S+)\r?$") else {
        return Vec::new();
    };
    let mut out: Vec<InstalledMod> = Vec::new();
    for c in re.captures_iter(text) {
        if !out.iter().any(|m| m.id == c[1]) {
            out.push(InstalledMod {
                id: c[1].to_owned(),
                name: c[2].to_owned(),
                version: c[3].to_owned(),
                ..InstalledMod::default()
            });
        }
    }
    out
}

/// Version as numbers plus whether it has a pre-release tag ("1.2.3-beta+build" → [1,2,3], true).
fn parse_version(v: &str) -> Option<(Vec<u64>, bool)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split('+').next()?;
    let (nums, pre) = match core.split_once('-') {
        Some((n, _)) => (n, true),
        None => (core, false),
    };
    let nums: Option<Vec<u64>> = nums.split('.').map(|p| p.parse().ok()).collect();
    nums.map(|n| (n, pre))
}

fn compare(a: &str, b: &str) -> Option<Ordering> {
    let ((mut x, px), (mut y, py)) = (parse_version(a)?, parse_version(b)?);
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    Some(x.cmp(&y).then(py.cmp(&px)))
}

/// Does `version` fall in Fabric-style `range` ("*", ">=1.2 <2", "1.x", "^1.2", "<1 || >=3")?
/// A range or version that can't be read matches nothing, so nothing is switched off on a guess.
fn in_range(range: &str, version: &str) -> bool {
    range.split("||").any(|alt| {
        let alt = alt.trim();
        alt == "*"
            || (!alt.is_empty() && alt.split_whitespace().all(|pred| predicate(pred, version)))
    })
}

fn predicate(pred: &str, version: &str) -> bool {
    if pred == "*" {
        return true;
    }
    let (op, rest) = ["<=", ">=", "<", ">", "=", "^", "~"]
        .iter()
        .find_map(|op| pred.strip_prefix(op).map(|r| (*op, r)))
        .unwrap_or(("=", pred));
    // "1.x" / "1.2.*": only the numbers before the wildcard count.
    let fixed = rest
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"))
        .collect::<Vec<_>>()
        .join(".");
    let wildcard = fixed.len() != rest.len();
    let (Some((want, _)), Some((have, _))) = (parse_version(&fixed), parse_version(version)) else {
        return false;
    };
    let head = |n: usize| have.iter().take(n).eq(want.iter().take(n));
    match (op, compare(version, &fixed)) {
        (_, None) => false,
        ("=", Some(o)) => {
            if wildcard {
                head(want.len())
            } else {
                o == Ordering::Equal
            }
        }
        (">=", Some(o)) => o != Ordering::Less,
        (">", Some(o)) => o == Ordering::Greater,
        ("<=", Some(o)) => o != Ordering::Greater,
        ("<", Some(o)) => o == Ordering::Less,
        ("^", Some(o)) => o != Ordering::Less && head(1),
        ("~", Some(o)) => o != Ordering::Less && head(2),
        _ => false,
    }
}

/// The pairs of installed mods that cannot be used together.
pub fn find_conflicts(mods: &[InstalledMod]) -> Vec<Conflict> {
    let mut out = Vec::new();
    for a in 0..mods.len() {
        for b in a + 1..mods.len() {
            let (x, y) = (&mods[a], &mods[b]);
            let (xi, yi) = (x.id.to_lowercase(), y.id.to_lowercase());
            let declares = |m: &InstalledMod, other: &InstalledMod| {
                m.breaks.iter().any(|(id, ranges)| {
                    *id == other.id.to_lowercase()
                        && ranges.iter().any(|r| in_range(r, &other.version))
                })
            };
            let fork =
                |orig: &str, fork: &str| KNOWN_FORKS.iter().any(|(o, f)| *o == orig && *f == fork);
            let found = if xi == yi {
                Some((a, b, Why::SameId))
            } else if fork(&xi, &yi) {
                Some((a, b, Why::Fork))
            } else if fork(&yi, &xi) {
                Some((b, a, Why::Fork))
            } else if declares(x, y) {
                Some((a, b, Why::Declared))
            } else if declares(y, x) {
                Some((b, a, Why::Declared))
            } else {
                None
            };
            if let Some((a, b, why)) = found {
                out.push(Conflict { a, b, why });
            }
        }
    }
    out
}

/// The mod whose mixin config failed to apply, as the log names it: (config file, mod id when the
/// log says so).
fn mixin_failure(text: &str) -> Option<(String, Option<String>)> {
    let gate = Regex::new(
        r"Critical injection failure|InvalidInjectionException|InvalidMixinException|MixinApplyError|MixinTransformerError|@Mixin target \S+ was not found|Mixin apply for mod|InjectionError",
    )
    .ok()?;
    let config = Regex::new(r"([\w\-.]+\.mixins\.json)").ok()?;
    let owner = Regex::new(r"(?:from mod|apply for mod) ([\w\-]+)").ok()?;
    text.lines().filter(|l| gate.is_match(l)).find_map(|l| {
        let cfg = config.captures(l)?[1].to_owned();
        Some((cfg, owner.captures(l).map(|o| o[1].to_lowercase())))
    })
}

/// Which installed mod owns mixin config `config`: the log's own answer, else the jar that lists
/// the config, else the mod whose id starts the config's name ("noisiumed-common.mixins.json").
fn owner_of<'a>(
    config: &str,
    said: Option<&str>,
    mods: &'a [InstalledMod],
) -> Option<&'a InstalledMod> {
    let by_id = |id: &str| mods.iter().find(|m| m.id.eq_ignore_ascii_case(id));
    said.and_then(by_id)
        .or_else(|| mods.iter().find(|m| m.mixins.iter().any(|c| c == config)))
        .or_else(|| {
            let stem = config.to_lowercase();
            mods.iter()
                .filter(|m| {
                    let id = m.id.to_lowercase();
                    stem.strip_prefix(&id)
                        .is_some_and(|r| r.starts_with(['-', '.', '_']))
                })
                .max_by_key(|m| m.id.len())
        })
}

/// Which side of a conflict to switch off: the mod whose mixin failed, else the fork, the older of
/// two copies, or the mod that declares the clash. Returns (switch off, keep).
fn choose(c: &Conflict, mods: &[InstalledMod], failed: Option<&InstalledMod>) -> (usize, usize) {
    if let Some(f) = failed {
        // Compare by position within the pair: two copies of one id share their id.
        if mods[c.a].id == f.id && mods[c.b].id != f.id {
            return (c.a, c.b);
        }
        if mods[c.b].id == f.id && mods[c.a].id != f.id {
            return (c.b, c.a);
        }
    }
    match c.why {
        Why::SameId => match compare(&mods[c.a].version, &mods[c.b].version) {
            Some(Ordering::Greater) => (c.b, c.a),
            _ => (c.a, c.b),
        },
        Why::Fork => (c.b, c.a),
        Why::Declared => (c.a, c.b),
    }
}

fn explain(
    c: &Conflict,
    mods: &[InstalledMod],
    failed: Option<&InstalledMod>,
    blamed: bool,
) -> Diagnosis {
    let (off, keep) = choose(c, mods, failed);
    let (a, b) = (&mods[c.a], &mods[c.b]);
    let (title, why) = match c.why {
        Why::Fork => (
            format!("{} and {} are both installed", a.name, b.name),
            format!(
                "{} and {} are both installed. {} is a fork of {}, so they clash. Keep one of them.",
                a.name, b.name, b.name, a.name
            ),
        ),
        Why::SameId => (
            format!("{} is installed twice", a.name),
            format!(
                "{} is in the mods folder twice ({} and {}). Two copies of one mod clash. Keep one of them.",
                a.name,
                or_file(a),
                or_file(b)
            ),
        ),
        Why::Declared => (
            format!("{} and {} can't be used together", a.name, b.name),
            format!(
                "{} says it doesn't work with {}, and both are installed. Keep one of them.",
                a.name, b.name
            ),
        ),
    };
    let cause = match (blamed, failed) {
        (true, Some(f)) => format!(
            " {}'s changes to the game failed to apply, which is what crashed it.",
            f.name
        ),
        _ => String::new(),
    };
    let off_mod = &mods[off];
    // Two copies of one id are told apart by file; anything else by id.
    let target = if c.why == Why::SameId && !off_mod.file.is_empty() {
        off_mod.file.clone()
    } else {
        off_mod.id.clone()
    };
    Diagnosis {
        fix: Some(Fix::DisableMods(vec![target])),
        ..Diagnosis::new(
            title,
            format!(
                "{why}{cause} \"Fix it\" switches off {} and keeps {}; you can switch it back on in the instance's Mods page.",
                off_mod.name, mods[keep].name
            ),
            vec![mods[off].id.clone(), mods[keep].id.clone()],
        )
    }
}

fn or_file(m: &InstalledMod) -> String {
    if m.file.is_empty() {
        format!("{} {}", m.name, m.version)
    } else {
        m.file.clone()
    }
}

/// Whether the run implicates this pair, rather than merely listing an installed mod.
/// An incomplete log is not proof of an early failure: require a mod-loading message before
/// using that fallback, and stop using it once the game reaches its entrypoints or renderer.
fn clash_mattered(text: &str, c: &Conflict, mods: &[InstalledMod]) -> bool {
    let named = [&mods[c.a], &mods[c.b]].map(|m| {
        let names = [&m.id, &m.name, &m.file]
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| regex::escape(s))
            .collect::<Vec<_>>()
            .join("|");
        Regex::new(&format!(r"(?i)(?:^|[^\w-])(?:{names})(?:$|[^\w-])"))
            .expect("escaped mod names are valid")
    });
    let duplicate = Regex::new(
        r"(?i)duplicate mods?|duplicatemodsfoundexception|duplicate versions for mod id|found a duplicate mod",
    )
    .expect("duplicate mod pattern is valid");
    let rejection = Regex::new(
        r"(?i)duplicate mods?|duplicatemodsfoundexception|duplicate versions for mod id|found a duplicate mod|incompatible mods? (?:found|set)|mod resolution encountered an incompatible mod set|mod '.+' .+ is incompatible with|NEG_HARD_DEP .+\{breaks ",
    )
    .expect("loader rejection pattern is valid");
    let progressed = Regex::new(
        r"(?i)Reloading ResourceManager|Initializing mods \(entrypoint|entrypoints? (?:ran|running)|Backend library:|LWJGL (?:Version|version):|OpenGL (?:Vendor|Renderer|Version):|OpenAL initialized|Setting user:|Created: .*texture|Sound engine started|Starting integrated minecraft server|Connecting to ",
    )
    .expect("game progress pattern is valid");
    let lines: Vec<_> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        // Fabric/Quilt and Forge/NeoForge can put the ids/files on subsequent lines.
        // Don't borrow a name from a later successful mod inventory or game log record.
        if rejection.is_match(line)
            && (duplicate.is_match(line) == (c.why == Why::SameId))
            && !lines[i + 1..].iter().any(|l| progressed.is_match(l))
        {
            let context = std::iter::once(*line)
                .chain(lines[i + 1..].iter().take(12).copied().take_while(|l| {
                    !l.trim().is_empty() && !l.contains("/INFO]") && !l.starts_with("----")
                }))
                .collect::<Vec<_>>()
                .join("\n");
            // For incompatible pairs, both sides must be named. A rejection involving one
            // of them and some third mod does not establish that this pair caused the exit.
            if named.iter().all(|n| n.is_match(&context)) {
                return true;
            }
        }
        let line = line.trim_start();
        if (line.starts_with("at ")
            || line.starts_with("Caused by:")
            || line.starts_with("Suspected Mod")
            || line.starts_with("-- MOD ")
            || line.starts_with("Failure message:")
            || line.starts_with("Mod File:"))
            && named.iter().any(|n| n.is_match(line))
        {
            return true;
        }
    }
    let loading =
        Regex::new(r"(?i)Loading \d+ mods|Loading mods|Discovering mods|Scanning mod files")
            .expect("mod loading pattern is valid");
    !rejection.is_match(text) && loading.is_match(text) && !progressed.is_match(text)
}

/// `base` (what the log rules found) improved with what the installed mods say. A failed mixin
/// owned by a clashing mod gets the clash explanation; other clashes require evidence from the
/// run before they can explain an otherwise unknown crash.
pub fn refine(base: Option<Diagnosis>, text: &str, mods: &[InstalledMod]) -> Option<Diagnosis> {
    let from_log;
    let mods = if mods.is_empty() {
        from_log = from_crash_report(text);
        &from_log[..]
    } else {
        mods
    };
    let conflicts = find_conflicts(mods);
    let failure = mixin_failure(text);
    let failed = failure
        .as_ref()
        .and_then(|(cfg, said)| owner_of(cfg, said.as_deref(), mods));
    if let Some(f) = failed {
        let involved = conflicts.iter().find(|c| {
            mods[c.a].id.eq_ignore_ascii_case(&f.id) || mods[c.b].id.eq_ignore_ascii_case(&f.id)
        });
        if let Some(c) = involved {
            return Some(explain(c, mods, Some(f), true));
        }
    }
    if base.is_some() {
        return base;
    }
    if failed.is_none()
        && let Some(c) = conflicts.iter().find(|c| clash_mattered(text, c, mods))
    {
        return Some(explain(c, mods, None, false));
    }
    let f = failed?;
    Some(Diagnosis::new(
        format!("{} failed to change the game", f.name),
        format!(
            "{}'s changes to the game could not be applied: it is probably made for another Minecraft version or clashes with another mod. Update or remove it.",
            f.name
        ),
        vec![f.id.clone()],
    ))
}

/// `crash::diagnose` plus the clash check against the mods in `mods_dir`.
pub fn diagnose_in(text: &str, mods_dir: &Path) -> Option<Diagnosis> {
    refine(crate::crash::diagnose(text), text, &scan(mods_dir))
}

/// The exit diagnosis, with incidental folder clashes kept as advice in the details. The UI has
/// one fix action, so don't replace the actual cause's action with an unrelated DisableMods fix.
pub fn diagnose_exit_in(text: &str, code: i32, mods_dir: &Path) -> Option<Diagnosis> {
    let mods = scan(mods_dir);
    let with_exit = format!(
        "{text}\n(exit code {})",
        crate::crash::exit_description(code)
    );
    let diagnosis = refine(crate::crash::diagnose(&with_exit), &with_exit, &mods)
        .or_else(|| crate::crash::silent_exit(text, code));
    let conflicts = find_conflicts(&mods);
    let Some(c) = conflicts.first() else {
        return diagnosis;
    };
    let clash = explain(c, &mods, None, false);
    let mut diagnosis = diagnosis.unwrap_or_else(|| {
        Diagnosis::new(
            format!("Game exited with code {code} (0x{:08X})", code as u32),
            format!(
                "The game stopped with exit {}. The logs do not identify the cause.",
                crate::crash::exit_description(code)
            ),
            Vec::new(),
        )
    });
    if diagnosis.title != clash.title {
        diagnosis.detail.push_str(&format!(
            "\n\nAlso found a possible mods-folder clash: {}. This alone does not establish why the game stopped. Review {} and {} in the instance's Mods page.",
            clash.title, or_file(&mods[c.a]), or_file(&mods[c.b])
        ));
    }
    Some(diagnosis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(dir: &Path, file: &str, manifest: &str) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(file)).unwrap());
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    /// Do the diagnosis' fix and list the folder afterwards.
    fn apply_off(dir: &Path, d: &Diagnosis) -> Vec<String> {
        let Some(Fix::DisableMods(names)) = &d.fix else {
            panic!("no disable fix");
        };
        crate::crash_fix::disable_named(dir, names).unwrap();
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        files
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("arctic-conflicts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Crash report #267 (Minecraft 1.21.1): Noisium and its fork Noisiumed both installed; the
    /// fork's Lithium compat mixin could not find its target. Trimmed, with the personal parts out.
    const REPORT_267: &str = include_str!("../tests/fixtures/crash-267-noisiumed.log");

    fn duplicate_mods() -> Vec<InstalledMod> {
        ["1.0.0", "1.2.0"]
            .into_iter()
            .map(|version| InstalledMod {
                id: "foo".into(),
                name: "Foo".into(),
                file: format!("foo-{version}.jar"),
                version: version.into(),
                ..InstalledMod::default()
            })
            .collect()
    }

    #[test]
    fn loader_rejection_implicates_the_named_clash() {
        let mods = duplicate_mods();
        let c = &find_conflicts(&mods)[0];
        for text in [
            "net.fabricmc.loader.impl.discovery.ModResolutionException: Duplicate versions for mod ID 'foo': [1.0.0, 1.2.0]",
            "org.quiltmc.loader.api.ModResolutionException: Incompatible mod set!\n - Duplicate mod foo",
            "net.minecraftforge.fml.common.DuplicateModsFoundException\nMod ID: 'foo' from mod files: foo-1.0.0.jar, foo-1.2.0.jar",
            "[main/ERROR] [net.neoforged.fml.loading.UniqueModListBuilder/LOADING]: Found duplicate mods:\n\tMod ID: 'foo' from mod files: foo-1.0.0.jar, foo-1.2.0.jar",
        ] {
            assert!(clash_mattered(text, c, &mods), "{text}");
            assert_eq!(
                refine(None, text, &mods).unwrap().title,
                "Foo is installed twice"
            );
        }
        assert!(!clash_mattered(
            "Loading 10 mods\nDuplicate versions for mod ID 'other': [1, 2]",
            c,
            &mods
        ));
        assert!(!clash_mattered(
            "Incompatible mods found!\n - Mod 'Foo' (foo) 1.0.0 is incompatible with any version of mod 'Bar' (bar), but a matching version is present!",
            c,
            &mods,
        ));
    }

    #[test]
    fn crash_blame_implicates_the_clash_but_an_inventory_does_not() {
        let mods = duplicate_mods();
        let c = &find_conflicts(&mods)[0];
        for text in [
            "---- Minecraft Crash Report ----\nSuspected Mods: Foo (foo)",
            "java.lang.RuntimeException: boom\n\tat knot//example.foo.Client.init(Client.java:42)",
            "---- Minecraft Crash Report ----\nMod File: foo-1.0.0.jar\nFailure message: Failed to initialize",
        ] {
            assert!(clash_mattered(text, c, &mods), "{text}");
        }
        for text in [
            "java.lang.RuntimeException: boom",
            "---- Minecraft Crash Report ----\n\tFabric Mods:\n\t\tfoo: Foo 1.2.0",
            "Reloading ResourceManager: vanilla, foo\n\tat example.foobar.Client.init(Client.java:42)",
            "Duplicate versions for mod ID 'other': [1, 2]\n[main/INFO] Loading 10 mods:\n - foo 1.2.0\nReloading ResourceManager: foo",
            "Duplicate versions for mod ID 'foo': [1, 2]\nReloading ResourceManager: foo",
        ] {
            assert!(!clash_mattered(text, c, &mods), "{text}");
            assert!(refine(None, text, &mods).is_none(), "{text}");
        }
    }

    #[test]
    fn early_loading_requires_positive_evidence_and_no_later_progress() {
        let mods = duplicate_mods();
        let c = &find_conflicts(&mods)[0];
        let loading = "[main/INFO] Loading 10 mods:\n - foo 1.2.0";
        assert!(clash_mattered(loading, c, &mods));
        for progress in [
            "Initializing mods (entrypoint yumi:init)...",
            "Backend library: LWJGL version 3.3.3",
            "OpenGL Renderer: Intel(R) HD Graphics 4000",
            "Reloading ResourceManager: vanilla, foo",
        ] {
            let text = format!("{loading}\n{progress}");
            assert!(!clash_mattered(&text, c, &mods), "{text}");
            assert!(refine(None, &text, &mods).is_none());
        }
        assert!(!clash_mattered("", c, &mods));
    }

    #[test]
    fn an_unrelated_failed_mixin_does_not_blame_a_folder_clash() {
        let mut mods = duplicate_mods();
        mods.push(InstalledMod {
            id: "bar".into(),
            name: "Bar".into(),
            ..InstalledMod::default()
        });
        let text = "Loading 10 mods\nInvalidInjectionException: Critical injection failure in bar.mixins.json:XMixin from mod bar failed";
        let d = refine(None, text, &mods).unwrap();
        assert_eq!(d.title, "Bar failed to change the game");
        assert!(d.fix.is_none());
    }

    #[test]
    fn fork_pairs_and_declared_breaks_also_need_evidence() {
        for (a, b, breaks) in [
            ("noisium", "noisiumed", Vec::new()),
            ("alpha", "beta", vec![("beta".into(), vec!["*".into()])]),
        ] {
            let mods = vec![
                InstalledMod {
                    id: a.into(),
                    name: a.into(),
                    version: "1.0.0".into(),
                    breaks,
                    ..InstalledMod::default()
                },
                InstalledMod {
                    id: b.into(),
                    name: b.into(),
                    version: "1.0.0".into(),
                    ..InstalledMod::default()
                },
            ];
            assert_eq!(find_conflicts(&mods).len(), 1);
            let conflicts = find_conflicts(&mods);
            let c = &conflicts[0];
            assert!(clash_mattered(
                &format!("Incompatible mods found!\n - {a} conflicts with {b}"),
                c,
                &mods,
            ));
            assert!(!clash_mattered(
                &format!("Incompatible mods found!\n - {a} conflicts with other"),
                c,
                &mods,
            ));
            assert!(!clash_mattered(
                &format!("Duplicate versions for mod ID '{a}': [1, 2]"),
                c,
                &mods,
            ));
            let text = format!(
                "Loading 142 mods:\n - {a}\n - {b}\nReloading ResourceManager: vanilla, {a}, {b}"
            );
            assert!(refine(None, &text, &mods).is_none());
        }
    }

    #[test]
    fn report_301_keeps_the_exit_diagnosis_despite_two_kotlin_jars() {
        let dir = tempfile::tempdir().unwrap();
        for version in ["1.13.9+kotlin.2.3.10", "1.14.2+kotlin.2.4.21"] {
            jar(
                dir.path(),
                &format!("fabric-language-kotlin-{version}.jar"),
                &format!(
                    r#"{{"id":"fabric-language-kotlin","name":"Fabric Language Kotlin","version":"{version}"}}"#
                ),
            );
        }
        let text = include_str!("../tests/fixtures/crash-301-kotlin-hang.log");
        assert_eq!(find_conflicts(&scan(dir.path())).len(), 1);
        assert!(diagnose_in(text, dir.path()).is_none());
        let d = diagnose_exit_in(text, -805306369, dir.path()).unwrap();
        assert_eq!(d.title, "Game exited with code -805306369 (0xCFFFFFFF)");
        assert_ne!(d.title, "Fabric Language Kotlin is installed twice");
        assert!(!d.title.contains("resource-pack"));
        assert!(d.detail.contains("hang termination or native failure"));
        assert!(
            d.detail
                .contains("Also found a possible mods-folder clash:")
        );
        assert!(
            d.detail
                .contains("fabric-language-kotlin-1.13.9+kotlin.2.3.10.jar")
        );
        assert!(
            d.detail
                .contains("fabric-language-kotlin-1.14.2+kotlin.2.4.21.jar")
        );
        assert!(d.fix.is_none());

        // A fresh freeze snapshot establishes a hang; the incidental duplicate still stays advice.
        let frozen = format!("{text}\nHang at exit: client hung for 12s; no recovery recorded");
        let d = diagnose_exit_in(&frozen, -805306369, dir.path()).unwrap();
        assert_eq!(d.title, "The game froze and was closed");
        assert!(
            d.detail
                .contains("Also found a possible mods-folder clash:")
        );
        assert!(d.fix.is_none());

        // A silent start and a known native exit also retain their normal explanations.
        let d = diagnose_exit_in("", 1, dir.path()).unwrap();
        assert!(
            d.title
                .starts_with("The game stopped before it wrote anything")
        );
        let d = diagnose_exit_in(text, 0xC0000005_u32 as i32, dir.path()).unwrap();
        assert!(d.title.contains("0xC0000005"));
        assert!(d.detail.contains("STATUS_ACCESS_VIOLATION"));

        let d = diagnose_exit_in(
            "Mod 'ModernFix' (modernfix) 5.2 requires version 0.16.10 or later of mod 'Fabric Loader' (fabricloader), but only the wrong version is present: 0.15.11!",
            1,
            dir.path(),
        ).unwrap();
        assert_eq!(d.title, "Fabric Loader is too old for these mods");
        assert_eq!(d.fix, Some(Fix::UpdateLoader));
        assert!(d.detail.contains("possible mods-folder clash"));
    }

    #[test]
    fn report_267_names_the_fork_clash_from_the_log_alone() {
        let text = REPORT_267;
        // The old rules had nothing to say about this one.
        assert!(crate::crash::diagnose(text).is_none());
        let d = refine(None, text, &[]).unwrap();
        assert_eq!(d.title, "Noisium and Noisiumed are both installed");
        assert!(d.detail.starts_with(
            "Noisium and Noisiumed are both installed. Noisiumed is a fork of Noisium, so they clash. Keep one of them."
        ));
        assert!(
            d.detail
                .contains("Noisiumed's changes to the game failed to apply")
        );
        assert_eq!(d.fix, Some(Fix::DisableMods(vec!["noisiumed".into()])));
    }

    #[test]
    fn report_267_with_real_jars_switches_off_the_failed_fork_only() {
        let dir = tmp("267");
        jar(
            &dir,
            "noisium.jar",
            r#"{"id":"noisium","name":"Noisium","version":"2.7.0+mc1.21-1.21.1","mixins":["noisium-common.mixins.json"]}"#,
        );
        jar(
            &dir,
            "noisiumed.jar",
            r#"{"id":"noisiumed","name":"Noisiumed","version":"3.0.6-fabric-1.21.1","mixins":["noisiumed-common.mixins.json"]}"#,
        );
        let d = diagnose_in(REPORT_267, &dir).unwrap();
        assert_eq!(d.fix, Some(Fix::DisableMods(vec!["noisiumed".into()])));
        assert_eq!(
            apply_off(&dir, &d),
            vec!["noisium.jar", "noisiumed.jar.disabled"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mixin_failure_is_attributed_by_config_name_when_the_log_has_no_owner() {
        let text = "org.spongepowered.asm.mixin.transformer.throwables.InvalidMixinException: @Mixin target net.minecraft.Foo was not found fancy-core.mixins.json:FooMixin";
        let mods = vec![InstalledMod {
            id: "fancy".into(),
            name: "Fancy".into(),
            ..InstalledMod::default()
        }];
        // `fancy-core.mixins.json` starts with the id `fancy`.
        let d = refine(None, text, &mods).unwrap();
        assert_eq!(d.title, "Fancy failed to change the game");
        assert_eq!(d.mods, vec!["fancy"]);
        assert_eq!(d.fix, None);
    }

    #[test]
    fn the_same_mod_twice_switches_off_the_older_copy_by_file() {
        let dir = tmp("dupe");
        jar(
            &dir,
            "foo-1.0.jar",
            r#"{"id":"foo","name":"Foo","version":"1.0.0"}"#,
        );
        jar(
            &dir,
            "foo-1.2.jar",
            r#"{"id":"foo","name":"Foo","version":"1.2.0"}"#,
        );
        jar(
            &dir,
            "bar.jar",
            r#"{"id":"bar","name":"Bar","version":"1.0.0"}"#,
        );
        let d = diagnose_in("net.fabricmc.loader.impl.discovery.ModResolutionException: Duplicate versions for mod ID 'foo': [1.0.0, 1.2.0]", &dir).unwrap();
        assert_eq!(d.title, "Foo is installed twice");
        assert_eq!(d.fix, Some(Fix::DisableMods(vec!["foo-1.0.jar".into()])));
        assert_eq!(
            apply_off(&dir, &d),
            vec!["bar.jar", "foo-1.0.jar.disabled", "foo-1.2.jar"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn declared_breaks_and_conflicts_count_only_for_matching_versions() {
        let dir = tmp("breaks");
        jar(
            &dir,
            "a.jar",
            r#"{"id":"alpha","name":"Alpha","version":"1.0.0","breaks":{"beta":"<2.0.0"},"conflicts":{"gamma":["1.x","3.1.0"]}}"#,
        );
        jar(
            &dir,
            "b.jar",
            r#"{"id":"beta","name":"Beta","version":"1.5.0"}"#,
        );
        let rejected = "[main/ERROR]: Incompatible mods found!\n - alpha conflicts with beta";
        let d = diagnose_in(rejected, &dir).unwrap();
        assert_eq!(d.title, "Alpha and Beta can't be used together");
        assert_eq!(d.fix, Some(Fix::DisableMods(vec!["alpha".into()])));
        // A newer Beta is fine, and Gamma 2.0 is outside "1.x".
        jar(
            &dir,
            "b.jar",
            r#"{"id":"beta","name":"Beta","version":"2.0.0"}"#,
        );
        jar(
            &dir,
            "c.jar",
            r#"{"id":"gamma","name":"Gamma","version":"2.0.0"}"#,
        );
        assert!(diagnose_in("java.lang.RuntimeException: boom", &dir).is_none());
        jar(
            &dir,
            "c.jar",
            r#"{"id":"gamma","name":"Gamma","version":"1.4.2"}"#,
        );
        let d = diagnose_in(
            "[main/ERROR]: Incompatible mods found!\n - alpha conflicts with gamma",
            &dir,
        )
        .unwrap();
        assert_eq!(d.title, "Alpha and Gamma can't be used together");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_mixin_picks_which_side_to_switch_off() {
        let dir = tmp("pick");
        jar(
            &dir,
            "a.jar",
            r#"{"id":"alpha","name":"Alpha","version":"1.0.0","breaks":{"beta":"*"}}"#,
        );
        jar(
            &dir,
            "b.jar",
            r#"{"id":"beta","name":"Beta","version":"1.0.0","mixins":[{"config":"beta.mixins.json"}]}"#,
        );
        let text = "InvalidInjectionException: Critical injection failure: @Inject in beta.mixins.json:XMixin from mod beta failed";
        let d = diagnose_in(text, &dir).unwrap();
        assert_eq!(d.fix, Some(Fix::DisableMods(vec!["beta".into()])));
        assert!(
            d.detail
                .contains("Beta's changes to the game failed to apply")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_explanations_and_clean_instances_are_left_alone() {
        let dir = tmp("clean");
        jar(
            &dir,
            "a.jar",
            r#"{"id":"alpha","name":"Alpha","version":"1.0.0"}"#,
        );
        assert!(diagnose_in("java.lang.RuntimeException: boom", &dir).is_none());
        // A real cause in the log wins over a latent clash with no mixin failure.
        jar(
            &dir,
            "n1.jar",
            r#"{"id":"noisium","name":"Noisium","version":"2.3.0"}"#,
        );
        jar(
            &dir,
            "n2.jar",
            r#"{"id":"noisiumed","name":"Noisiumed","version":"3.0.6"}"#,
        );
        let d = diagnose_in("java.lang.OutOfMemoryError: Java heap space", &dir).unwrap();
        assert_eq!(d.title, "Minecraft ran out of memory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_ranges() {
        assert!(in_range("*", "1.0"));
        assert!(in_range(">=1.2 <2", "1.9.9+build"));
        assert!(!in_range(">=1.2 <2", "2.0.0"));
        assert!(in_range("1.x", "1.4.2"));
        assert!(!in_range("1.x", "2.0"));
        assert!(in_range("<1 || >=3", "3.1"));
        assert!(in_range("^1.2", "1.9") && !in_range("^1.2", "2.0"));
        assert!(in_range("~1.2", "1.2.7") && !in_range("~1.2", "1.3"));
        assert!(in_range("=1.0", "1.0.0"));
        // Unreadable: matches nothing.
        assert!(!in_range("<banana", "1.0"));
    }
}
