//! Mods that stop the game together, found before it starts: the same
//! mod twice, a mod that says it breaks another, a required mod that's
//! missing or too old, a mod made for another Minecraft version. Each
//! comes with the fixes Arctic can make for it.

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::loaders::LoaderKind;

/// Something Arctic can do about a clash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Switch this jar off (it's kept as `.jar.disabled`).
    TurnOff { file: String, name: String },
    /// Install this Modrinth project; `replacing` is switched off first.
    Get {
        project: String,
        name: String,
        replacing: Option<String>,
    },
}

impl Fix {
    pub fn label(&self) -> String {
        match self {
            Fix::TurnOff { name, .. } => format!("Turn off {name}"),
            Fix::Get {
                name,
                replacing: None,
                ..
            } => format!("Get {name}"),
            Fix::Get { name, .. } => format!("Update {name}"),
        }
    }
}

/// One problem, in plain words, with what can be done about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub problem: String,
    pub fixes: Vec<Fix>,
}

/// What a mod jar says about itself.
#[derive(Debug, Default)]
struct Jar {
    file: String,
    id: String,
    name: String,
    version: String,
    /// (mod id, version predicate) it can't run with.
    breaks: Vec<(String, Vec<String>)>,
    /// (mod id, version predicates, any of which will do) it needs.
    depends: Vec<(String, Vec<String>)>,
}

/// Ids the loaders and the game provide (checked elsewhere, or not at all).
const BUILT_IN: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "forge",
    "neoforge",
];

/// Every clash among the switched-on mods in `mods_dir`.
pub fn check(mods_dir: &Path, loader: LoaderKind, game: &str) -> Vec<Clash> {
    let jars = read_all(mods_dir, loader);
    let mut out = duplicates(&jars);
    out.extend(breaks(&jars));
    out.extend(missing(mods_dir, loader, &jars));
    out.extend(too_old(&jars));
    out.extend(wrong_game(mods_dir, loader, game, &jars));
    out
}

fn read_all(mods_dir: &Path, loader: LoaderKind) -> Vec<Jar> {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let mut jars: Vec<Jar> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
        // A jar for another loader is skipped by the loader, not a clash.
        .filter(|e| super::detect::jar_fits(&e.path(), Some(loader)))
        .filter_map(|e| {
            read_jar(&e.path(), loader).map(|j| Jar {
                file: e.file_name().to_string_lossy().into_owned(),
                ..j
            })
        })
        .collect();
    jars.sort_by(|a, b| a.file.cmp(&b.file));
    jars
}

fn read_jar(path: &Path, loader: LoaderKind) -> Option<Jar> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut read = |name: &str| {
        let mut s = String::new();
        zip.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    match loader {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            read("fabric.mod.json").and_then(|t| fabric_jar(&t))
        }
        LoaderKind::NeoForge => read("META-INF/neoforge.mods.toml")
            .or_else(|| read("META-INF/mods.toml"))
            .and_then(|t| toml_jar(&t)),
        LoaderKind::Forge => read("META-INF/mods.toml").and_then(|t| toml_jar(&t)),
    }
}

fn fabric_jar(text: &str) -> Option<Jar> {
    // Some mods put raw newlines inside strings.
    let json: Value = serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok()?;
    let id = json.get("id")?.as_str()?.to_owned();
    let field = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_owned);
    let predicates = |k: &str| -> Vec<(String, Vec<String>)> {
        json.get(k)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(dep, v)| {
                let preds = match v {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(list) => list
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                    _ => vec!["*".to_owned()],
                };
                (dep.clone(), preds)
            })
            .collect()
    };
    Some(Jar {
        name: field("name").unwrap_or_else(|| id.clone()),
        version: field("version").unwrap_or_default(),
        breaks: predicates("breaks"),
        depends: predicates("depends"),
        id,
        ..Jar::default()
    })
}

/// `mods.toml`: the first `[[mods]]`, and `incompatible` dependencies.
fn toml_jar(text: &str) -> Option<Jar> {
    let mut jar = Jar::default();
    let mut section = String::new();
    let (mut dep_id, mut incompatible) = (String::new(), false);
    let finish = |jar: &mut Jar, id: &mut String, bad: &mut bool| {
        if *bad && !id.is_empty() {
            jar.breaks.push((std::mem::take(id), vec!["*".to_owned()]));
        }
        id.clear();
        *bad = false;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with("[[") {
            finish(&mut jar, &mut dep_id, &mut incompatible);
            section = line.trim_matches(['[', ']']).trim().to_owned();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        if section == "mods" {
            match k {
                "modId" if jar.id.is_empty() => jar.id = v.to_owned(),
                "displayName" if jar.name.is_empty() => jar.name = v.to_owned(),
                "version" if jar.version.is_empty() && !v.contains("${") => {
                    jar.version = v.to_owned()
                }
                _ => {}
            }
        } else if section.starts_with("dependencies") {
            match k {
                "modId" => dep_id = v.to_owned(),
                "type" => incompatible = v.eq_ignore_ascii_case("incompatible"),
                _ => {}
            }
        }
    }
    finish(&mut jar, &mut dep_id, &mut incompatible);
    if jar.id.is_empty() {
        return None;
    }
    if jar.name.is_empty() {
        jar.name = jar.id.clone();
    }
    Some(jar)
}

/// "Sodium 0.5.3": which copy, when there are several.
fn named(jar: &Jar) -> String {
    if jar.version.is_empty() {
        jar.name.clone()
    } else {
        format!("{} {}", jar.name, jar.version)
    }
}

/// The same mod twice: the loader refuses to start. The newest stays.
fn duplicates(jars: &[Jar]) -> Vec<Clash> {
    let mut out = Vec::new();
    let mut done = HashSet::new();
    for jar in jars {
        if !done.insert(jar.id.as_str()) {
            continue;
        }
        let copies: Vec<&Jar> = jars.iter().filter(|j| j.id == jar.id).collect();
        if copies.len() < 2 {
            continue;
        }
        let newest = copies
            .iter()
            .copied()
            .reduce(|a, b| {
                if super::detect::newer(&b.version, &a.version) {
                    b
                } else {
                    a
                }
            })
            .unwrap_or(jar);
        out.push(Clash {
            problem: format!("{} is in the mods folder {} times", jar.name, copies.len()),
            fixes: copies
                .iter()
                .filter(|j| j.file != newest.file)
                .map(|j| Fix::TurnOff {
                    file: j.file.clone(),
                    name: named(j),
                })
                .collect(),
        });
    }
    out
}

/// Does `version` satisfy any of `preds`? `None` when it can't be told.
fn satisfies(preds: &[String], version: &str) -> Option<bool> {
    let results: Vec<Option<bool>> = preds
        .iter()
        .map(|p| super::ranges::version_matches(p, version))
        .collect();
    if results.contains(&Some(true)) {
        Some(true)
    } else if !results.is_empty() && results.iter().all(|r| *r == Some(false)) {
        Some(false)
    } else {
        None
    }
}

fn breaks(jars: &[Jar]) -> Vec<Clash> {
    let mut out = Vec::new();
    let mut pairs = HashSet::new();
    for a in jars {
        for (id, preds) in &a.breaks {
            for b in jars.iter().filter(|b| &b.id == id && b.file != a.file) {
                // Only when the version is certainly one it breaks.
                if satisfies(preds, &b.version) != Some(true) {
                    continue;
                }
                // Once per pair of mods, however many copies there are.
                let key = if a.id < b.id {
                    (&a.id, &b.id)
                } else {
                    (&b.id, &a.id)
                };
                if !pairs.insert(key) {
                    continue;
                }
                out.push(Clash {
                    problem: format!("{} and {} can't run together", a.name, b.name),
                    fixes: vec![
                        Fix::TurnOff {
                            file: a.file.clone(),
                            name: a.name.clone(),
                        },
                        Fix::TurnOff {
                            file: b.file.clone(),
                            name: b.name.clone(),
                        },
                    ],
                });
            }
        }
    }
    out
}

/// What people call a mod id (and the Modrinth project to get it from).
fn project_of(id: &str) -> (&str, String) {
    match id {
        // Old mods name Fabric API by its former id.
        "fabric" | "fabric-api" => ("fabric-api", "Fabric API".to_owned()),
        "fabric-language-kotlin" => (
            "fabric-language-kotlin",
            "Fabric Language Kotlin".to_owned(),
        ),
        "cloth-config" | "cloth-config2" => ("cloth-config", "Cloth Config".to_owned()),
        _ => (id, id.to_owned()),
    }
}

fn missing(mods_dir: &Path, loader: LoaderKind, jars: &[Jar]) -> Vec<Clash> {
    super::deps::missing(mods_dir, loader)
        .into_iter()
        .map(|m| {
            let (project, name) = project_of(&m.id);
            let by = jars.iter().find(|j| j.id == m.by);
            let mut fixes = vec![Fix::Get {
                project: project.to_owned(),
                name: name.clone(),
                replacing: None,
            }];
            if let Some(by) = by {
                fixes.push(Fix::TurnOff {
                    file: by.file.clone(),
                    name: by.name.clone(),
                });
            }
            let who = by.map_or(m.by.as_str(), |j| j.name.as_str());
            Clash {
                problem: format!("{who} needs {name}, which isn't installed"),
                fixes,
            }
        })
        .collect()
}

/// A required mod that's there, but older than what needs it says.
fn too_old(jars: &[Jar]) -> Vec<Clash> {
    let mut out = Vec::new();
    for a in jars {
        for (id, preds) in &a.depends {
            if BUILT_IN.contains(&id.as_str()) {
                continue;
            }
            let Some(b) = jars.iter().find(|b| &b.id == id) else {
                continue;
            };
            if satisfies(preds, &b.version) != Some(false) {
                continue;
            }
            let (project, name) = project_of(id);
            out.push(Clash {
                problem: format!("{} needs a newer {} (it has {})", a.name, b.name, b.version),
                fixes: vec![
                    Fix::Get {
                        project: project.to_owned(),
                        name,
                        replacing: Some(b.file.clone()),
                    },
                    Fix::TurnOff {
                        file: a.file.clone(),
                        name: a.name.clone(),
                    },
                ],
            });
        }
    }
    out
}

fn wrong_game(mods_dir: &Path, loader: LoaderKind, game: &str, jars: &[Jar]) -> Vec<Clash> {
    jars.iter()
        .filter(|j| super::ranges::game_fits(&mods_dir.join(&j.file), game, loader) == Some(false))
        .map(|j| Clash {
            problem: format!(
                "{} is made for another Minecraft version, not {game}",
                named(j)
            ),
            fixes: vec![Fix::TurnOff {
                file: j.file.clone(),
                name: j.name.clone(),
            }],
        })
        .collect()
}
