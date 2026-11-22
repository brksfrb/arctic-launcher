//! Names the mod that crashed the game: the game's own "Suspected Mods" line, or else the first
//! frame of the fatal exception's stack trace (following "Caused by" chains, deepest first) that
//! belongs to an installed mod, including Mixin-merged methods (`handler$zbh000$modid$...`).
//! Anything unsure gives `None` so the caller keeps its generic message.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::crash::{Diagnosis, secondary_mod_failure, secondary_stack};
use crate::crash_conflicts::InstalledMod;

/// Name shown for the Arctic mod's own frames.
const ARCTIC_NAME: &str = "Arctic Client";
const ARCTIC_PACKAGE: &str = "com.arcticlauncher.";
const POLARIUM_PACKAGE: &str = "com.arcticlauncher.polarium.";

/// Game, loader and JDK code never gets blamed.
const NOT_MODS: &[&str] = &[
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "net.minecraft.",
    "net.minecraftforge.",
    "net.neoforged.",
    "net.fabricmc.loader.",
    "net.fabricmc.api.",
    "net.fabricmc.devlaunchinjector.",
    "org.quiltmc.loader.",
    "org.quiltmc.json5.",
    "org.spongepowered.",
    "cpw.mods.",
    "com.mojang.",
    "org.lwjgl.",
    "org.objectweb.",
    "org.apache.",
    "com.google.",
    "io.netty.",
    "it.unimi.",
    "org.slf4j.",
    "org.jetbrains.",
    "kotlin.",
    "kotlinx.",
    "scala.",
    "oshi.",
    "com.electronwill.",
];

/// Mod ids that are the game or its loader, not something to remove.
const BASE_IDS: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "quilt-loader",
    "forge",
    "neoforge",
    "fml",
    "javafml",
    "lowcodefml",
    "mixin",
    "mixinextras",
];

/// Mods that mostly provide code for other mods; a crash passing through them is rarely theirs.
const LIBRARIES: &[&str] = &[
    "fabric-api",
    "fabric",
    "cloth-config",
    "cloth-config2",
    "cloth_config",
    "architectury",
    "geckolib",
    "geckolib3",
    "owo",
    "owo-lib",
    "placeholder-api",
    "forgeconfigapiport",
    "forge-config-api-port",
    "midnightlib",
    "yet_another_config_lib_v3",
    "yacl",
    "modmenu",
    "kotlinforforge",
    "fabric-language-kotlin",
    "fabric-permissions-api-v0",
    "collective",
    "balm",
    "bclib",
    "puzzleslib",
    "cardinal-components",
    "cca-base",
    "trinkets",
    "curios",
    "playerabilitylib",
    "supermartijn642corelib",
    "supermartijn642configlib",
    "azurelib",
    "terraform-api",
    "resourcefulconfig",
    "resourcefullib",
    "creativecore",
    "moonlight",
    "jankson",
    "mixinextras",
    "iris_shader_mod_dummy",
];

/// A mod in the instance's mods folder with what is needed to match stack frames to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    pub id: String,
    pub name: String,
    pub file: String,
    /// Packages (dotted) its classes live in.
    pub packages: Vec<String>,
    pub library: bool,
}

/// The mod a crash points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suspect {
    pub id: String,
    pub name: String,
    /// Part of Arctic (the Arctic Client or Polarium).
    pub ours: bool,
}

fn norm(id: &str) -> String {
    id.to_lowercase().replace(['_', '-'], "")
}

fn is_arctic_id(id: &str) -> bool {
    matches!(norm(id).as_str(), "arctic" | "polarium")
}

fn is_library(id: &str, json_library: bool) -> bool {
    json_library
        || id.starts_with("fabric-")
        || id.starts_with("quilted_fabric_api")
        || LIBRARIES.iter().any(|l| norm(l) == norm(id))
}

/// Every jar in `mods_dir` with the packages its classes use.
pub fn scan(mods_dir: &Path) -> Vec<Known> {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let mut out: Vec<Known> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(".jar")
        })
        .filter_map(|e| read_known(&e.path()))
        .collect();
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

fn read_known(jar: &Path) -> Option<Known> {
    let file = jar.file_name()?.to_string_lossy().into_owned();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut packages: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let Ok(entry) = zip.by_index_raw(i) else {
            continue;
        };
        let name = entry.name();
        if let Some(path) = name.strip_suffix(".class")
            && !name.starts_with("META-INF/")
            && let Some((dir, _)) = path.rsplit_once('/')
        {
            let pkg = dir.replace('/', ".");
            if packages.last() != Some(&pkg) && !packages.contains(&pkg) {
                packages.push(pkg);
            }
        }
    }
    let mut read = |name: &str| {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(name).ok()?, &mut text).ok()?;
        Some(text)
    };
    let (m, json_library) = if let Some(text) = read("fabric.mod.json") {
        let library =
            serde_json::from_str::<serde_json::Value>(&text.replace(['\n', '\r', '\t'], " "))
                .ok()
                .and_then(|j| {
                    j.pointer("/custom/modmenu/badges")?
                        .as_array()
                        .map(|b| b.iter().any(|x| x.as_str() == Some("library")))
                })
                .unwrap_or(false);
        (
            crate::crash_conflicts::from_fabric_json(&file, &text)?,
            library,
        )
    } else {
        let toml = read("META-INF/neoforge.mods.toml").or_else(|| read("META-INF/mods.toml"))?;
        (crate::crash_conflicts::from_mods_toml(&file, &toml)?, false)
    };
    Some(Known {
        library: is_library(&m.id, json_library),
        id: m.id,
        name: m.name,
        file,
        packages,
    })
}

/// One stack frame, reduced to what identifies its owner.
pub(crate) struct Frame {
    pub(crate) class: String,
    pub(crate) method: String,
    /// Module/mod id written into the frame (`TRANSFORMER/modid@1.0/...`).
    module: Option<String>,
    /// Jar named at the end of the frame (`~[file.jar:?]`).
    jar: Option<String>,
}

pub(crate) fn frame(line: &str) -> Option<Frame> {
    parse_frame(line.trim_start().strip_prefix("at ")?)
}

impl Frame {
    pub(crate) fn is_platform(&self) -> bool {
        NOT_MODS.iter().any(|p| self.class.starts_with(p))
    }

    pub(crate) fn config_owner<'a>(&self, mods: &'a [InstalledMod]) -> Option<&'a InstalledMod> {
        if let Some(owner) = mods.iter().find(|m| {
            self.module.as_ref().is_some_and(|id| id == &m.id)
                || self.jar.as_ref().is_some_and(|jar| jar == &m.file)
        }) {
            return Some(owner);
        }
        let (package, _) = self.class.rsplit_once('.')?;
        package.rsplit('.').find_map(|part| {
            let part = norm(part);
            mods.iter()
                .find(|m| part == norm(&m.id) || part == norm(&m.name))
        })
    }
}

fn parse_frame(rest: &str) -> Option<Frame> {
    static JAR: OnceLock<Regex> = OnceLock::new();
    let (token, tail) = rest.split_once('(')?;
    let mut parts: Vec<&str> = token.split('/').collect();
    let call = parts.pop()?;
    let (class, method) = call.rsplit_once('.')?;
    let module = parts
        .iter()
        .find_map(|p| p.split_once('@').map(|(id, _)| id.to_owned()));
    let jar = JAR
        .get_or_init(|| Regex::new(r"\[([^\]\s:%!]+\.jar)[:%!\]]").unwrap())
        .captures(tail)
        .map(|c| c[1].to_owned());
    Some(Frame {
        class: class.to_owned(),
        method: method.to_owned(),
        module,
        jar,
    })
}

/// `handler$zbh000$modid$onTick` → `modid`.
fn mixin_owner(method: &str) -> Option<&str> {
    static OWNER: OnceLock<Regex> = OnceLock::new();
    OWNER
        .get_or_init(|| Regex::new(r"\$[a-z]{1,4}\d{3}\$([A-Za-z0-9_.\-]+)\$").unwrap())
        .captures(method)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

/// The stack of the exception that ended the game, split into its chain, deepest cause first.
/// `None` unless the text shows the exception was fatal (not just logged).
fn fatal_chain(text: &str) -> Option<Vec<Vec<&str>>> {
    let region = if let Some(stack) = crate::crash::crash_exception(text) {
        stack
    } else {
        let fatal = text.contains("Exception in thread")
            || text.contains("Reported exception thrown")
            || text.contains("Unreported exception");
        if !fatal {
            return None;
        }
        static HEAD: OnceLock<Regex> = OnceLock::new();
        let head = HEAD.get_or_init(|| {
            Regex::new(r#"^(?:Exception in thread "[^"]*" )?[\w$]+(?:\.[\w$]+)+(?::.*)?$"#).unwrap()
        });
        // The last unindented exception line that is followed by stack frames.
        let mut start = None;
        let mut pos = 0;
        let mut lines = text.split_inclusive('\n').peekable();
        while let Some(line) = lines.next() {
            let next_is_frame = lines
                .peek()
                .is_some_and(|n| n.trim_start().starts_with("at "));
            if next_is_frame && head.is_match(line.trim_end()) && !secondary_mod_failure(line) {
                start = Some(pos);
            }
            pos += line.len();
        }
        let start = start?;
        let rest = &text[start..];
        let mut end = rest.len();
        let mut pos = 0;
        for (i, line) in rest.split_inclusive('\n').enumerate() {
            let l = line.trim_end();
            let part = i == 0
                || l.starts_with('\t')
                || l.starts_with("    ")
                || l.starts_with("Caused by:")
                || l.starts_with("Suppressed:");
            if !part {
                end = pos;
                break;
            }
            pos += line.len();
        }
        &rest[..end]
    };
    let secondary = secondary_stack(region);
    let mut chain: Vec<Vec<&str>> = vec![Vec::new()];
    for (i, line) in region.lines().enumerate() {
        if i > 0
            && line.trim_start().starts_with("Caused by:")
            && (!line.starts_with(char::is_whitespace) || secondary)
        {
            chain.push(Vec::new());
        }
        if let Some(last) = chain.last_mut() {
            last.push(line);
        }
    }
    chain.reverse();
    if let Some(secondary) = chain.iter().position(|block| {
        block
            .first()
            .is_some_and(|line| secondary_mod_failure(line))
    }) {
        chain.truncate(secondary);
    }
    Some(chain)
}

struct Index<'a> {
    mods: &'a [Known],
    by_package: HashMap<&'a str, Option<usize>>,
}

impl<'a> Index<'a> {
    fn new(mods: &'a [Known]) -> Self {
        let mut by_package: HashMap<&str, Option<usize>> = HashMap::new();
        for (i, m) in mods.iter().enumerate() {
            for p in &m.packages {
                by_package
                    .entry(p.as_str())
                    .and_modify(|owner| {
                        if *owner != Some(i) {
                            *owner = None;
                        }
                    })
                    .or_insert(Some(i));
            }
        }
        Self { mods, by_package }
    }

    fn by_id(&self, id: &str) -> Option<usize> {
        let n = norm(id);
        self.mods.iter().position(|m| norm(&m.id) == n)
    }

    fn owner(&self, f: &Frame) -> Option<usize> {
        if let Some(i) = f.module.as_deref().and_then(|m| self.by_id(m)) {
            return Some(i);
        }
        if let Some(jar) = &f.jar
            && let Some(i) = self.mods.iter().position(|m| &m.file == jar)
        {
            return Some(i);
        }
        let mut package = f.class.rsplit_once('.').map(|(p, _)| p)?;
        loop {
            if let Some(owner) = self.by_package.get(package) {
                return *owner;
            }
            // Parent packages only count once they name a vendor and a mod (three parts).
            let (parent, _) = package.rsplit_once('.')?;
            if parent.matches('.').count() < 2 {
                return None;
            }
            package = parent;
        }
    }
}

fn arctic(id: &str) -> Suspect {
    Suspect {
        id: id.to_owned(),
        name: if norm(id) == "polarium" {
            crate::mods::polarium::NAME
        } else {
            ARCTIC_NAME
        }
        .to_owned(),
        ours: true,
    }
}

pub(crate) struct HangCulprit<'a> {
    pub mod_info: Suspect,
    pub class: String,
    pub method: String,
    pub percent: u8,
    pub sample: &'a str,
}

/// Find the one mod whose code appears in most of the frozen client-thread samples.
pub(crate) fn hang_culprit<'a>(text: &'a str, mods_dir: Option<&Path>) -> Option<HangCulprit<'a>> {
    let snapshot = crate::crash::latest_freeze_snapshot(text);
    // A finished freeze can establish a hang with the Windows exit code, but its
    // samples may be from an earlier, recovered stall.
    if snapshot.len() < text.len()
        && snapshot
            .lines()
            .find(|l| l.starts_with("duration: "))
            .is_some_and(|l| !l.ends_with(" (still frozen when written)"))
    {
        return None;
    }
    let mut frames = crate::crash::freeze_hot_frames(snapshot)
        .filter_map(|(percent, entry, line)| {
            if !(60..=100).contains(&percent) {
                return None;
            }
            let frame = parse_frame(entry)?;
            if NOT_MODS.iter().any(|p| frame.class.starts_with(p))
                || frame.class.starts_with("net.fabricmc.fabric.")
            {
                return None;
            }
            Some((frame, percent, line))
        })
        .peekable();
    frames.peek()?;
    let mods = mods_dir.map(scan).unwrap_or_default();
    let index = Index::new(&mods);
    let startup = crate::crash::compact_mod_inventory(text, "");
    let inventory: HashMap<_, _> = startup
        .lines()
        .chain(uploaded_mod_inventory(text).lines())
        .filter_map(|line| line.split_once(' ').map(|(id, _)| (norm(id), id)))
        .collect();
    let mut culprit: Option<HangCulprit<'_>> = None;
    for (frame, percent, sample) in frames {
        let Some((package, _)) = frame.class.rsplit_once('.') else {
            continue;
        };
        let mod_info = if frame.class.starts_with(POLARIUM_PACKAGE) {
            arctic("polarium")
        } else if frame.class.starts_with(ARCTIC_PACKAGE) {
            arctic("arctic")
        } else if let Some(i) = index.owner(&frame) {
            let m = &mods[i];
            if BASE_IDS.contains(&m.id.as_str()) {
                continue;
            }
            Suspect {
                id: m.id.clone(),
                name: m.name.clone(),
                ours: is_arctic_id(&m.id),
            }
        } else {
            // Match a complete package component to an installed id, falling back to the package.
            let id = package
                .rsplit('.')
                .find_map(|p| inventory.get(&norm(p)).copied())
                .unwrap_or(package);
            Suspect {
                id: id.to_owned(),
                name: id.to_owned(),
                ours: is_arctic_id(id),
            }
        };
        if let Some(best) = &culprit {
            // Each number is a share of stacks containing the frame. The shares can overlap.
            if best.mod_info.id != mod_info.id {
                return None;
            }
            if percent <= best.percent {
                continue;
            }
        }
        culprit = Some(HangCulprit {
            mod_info,
            class: frame.class,
            method: frame.method,
            percent,
            sample,
        });
    }
    culprit
}

fn uploaded_mod_inventory(text: &str) -> &str {
    text.split_once("---- full mod list (id version) ----")
        .and_then(|(_, list)| list.split_once('\n'))
        .map_or("", |(_, list)| list.split("\n---- ").next().unwrap_or(list))
}

pub(crate) const THREADING_TITLE: &str = "A mod used the world's random/data from another thread";

/// ThreadingDetector prints each thread's name with a colon, followed by its Java frames.
fn threading_worker_frame(report: &str) -> Option<Frame> {
    use crate::crash::minecraft_report_section;

    let lines = report
        .lines()
        .skip_while(|line| minecraft_report_section(line) != Some("Thread dumps"))
        .skip(1)
        .map(str::trim)
        .take_while(|line| {
            minecraft_report_section(line).is_none()
                && *line != "Stacktrace:"
                && !line.starts_with("Mixins in Stack")
                && !line.starts_with("Suspected Mod")
        });
    let mut worker = false;
    for line in lines {
        if line.is_empty() || line == "Details:" {
            continue;
        }
        if let Some(frame) = frame(line) {
            if worker
                && frame.class.contains('.')
                && !frame.is_platform()
                && !frame.class.starts_with("net.fabricmc.fabric.")
            {
                return Some(frame);
            }
        } else {
            let name = line.strip_prefix("Thread dumps:").unwrap_or(line).trim();
            worker = name.strip_suffix(':').is_some_and(|name| {
                let name = name.trim().trim_matches('"');
                ![
                    "main",
                    "main thread",
                    "render thread",
                    "client thread",
                    "server thread",
                ]
                .iter()
                .any(|main| name.eq_ignore_ascii_case(main))
            });
        }
    }
    None
}

pub(crate) fn diagnose_threading<'a>(
    text: &'a str,
    mods_dir: Option<&Path>,
    installed: &[InstalledMod],
) -> Option<(Diagnosis, &'a str)> {
    if !text.contains(" from multiple threads") {
        return None;
    }
    static EXCEPTION: OnceLock<Regex> = OnceLock::new();
    let exception = EXCEPTION.get_or_init(|| {
        Regex::new(r#"^(?:Caused by: |Exception in thread "[^"]+" |(?:\[[^\]]+\] )?\[[^\]]+/(?:ERROR|FATAL)\]:? )?java\.lang\.IllegalStateException: Accessing .+ from multiple threads$"#).unwrap()
    });
    let report = text.rsplit_once("---- Minecraft Crash Report ----");
    let crash = report.map_or(text, |(_, report)| report);
    let preamble = crash.lines().take_while(|line| {
        crate::crash::minecraft_report_section(line).is_none()
            && !line.starts_with("A detailed walkthrough of the error")
    });
    let error = preamble
        .clone()
        .map(str::trim)
        .find(|line| exception.is_match(line))
        .or_else(|| {
            preamble.map(str::trim).find(|line| {
                line.strip_prefix("Description: ")
                    .unwrap_or(line)
                    .strip_prefix("Accessing ")
                    .and_then(|name| name.strip_suffix(" from multiple threads"))
                    .is_some_and(|name| !name.is_empty())
            })
        })?;

    let Some(frame) = threading_worker_frame(crash) else {
        return Some((
            Diagnosis::new(
                THREADING_TITLE,
                "The culprit is unknown. This usually happens when a content or particle mod uses world.random off-thread. Disable half of those mods at a time and retest to narrow it down, then update or remove the mod you identify.",
                Vec::new(),
            ),
            error,
        ));
    };
    let known = mods_dir.map(scan).unwrap_or_default();
    let index = Index::new(&known);
    let reported = crate::crash_conflicts::from_crash_report(text);
    let (package, _) = frame.class.rsplit_once('.')?;
    let inventory = crate::crash::compact_mod_inventory(text, "");
    let ids: HashMap<_, _> = inventory
        .lines()
        .chain(uploaded_mod_inventory(text).lines())
        .filter_map(|line| line.split_once(' ').map(|(id, _)| (norm(id), id)))
        .collect();
    let (id, name) = if let Some(i) = index.owner(&frame) {
        (known[i].id.as_str(), known[i].name.as_str())
    } else if let Some(owner) = frame
        .config_owner(installed)
        .or_else(|| frame.config_owner(&reported))
    {
        (owner.id.as_str(), owner.name.as_str())
    } else {
        let id = package
            .rsplit('.')
            .find_map(|part| ids.get(&norm(part)).copied())
            .unwrap_or(package);
        (id, id)
    };
    Some((
        Diagnosis::new(
            THREADING_TITLE,
            format!("{name} appears in the worker-thread stack. Update or remove that mod."),
            vec![id.to_owned()],
        ),
        error,
    ))
}

pub(crate) const UNASSIGNED_ENTITY_TITLE: &str =
    "A mod created an entity without an ID (26.2+ change)";

fn unassigned_entity_failure(text: &str) -> Option<(&str, impl Iterator<Item = &str>)> {
    const EXCEPTION: &str =
        "java.lang.IllegalStateException: Tried to access entity ID before ID assignment";
    let mut lines = crate::crash::crash_exception(text).unwrap_or(text).lines();
    let error = lines.find(|line| {
        line.trim().strip_suffix(EXCEPTION).is_some_and(|prefix| {
            prefix.is_empty()
                || prefix == "Caused by: "
                || prefix.starts_with("Exception in thread \"")
                || prefix.ends_with("/ERROR] ")
                || prefix.ends_with("/ERROR]: ")
        })
    })?;
    Some((
        error.trim(),
        lines.take_while(|line| line.trim_start().starts_with("at ")),
    ))
}

fn unassigned_entity_culprit<'a>(
    text: &str,
    frames: impl Iterator<Item = &'a str>,
    known: &[Known],
    installed: &[InstalledMod],
) -> Option<Suspect> {
    const RENDERERS: &[&str] = &[
        "iris",
        "sodium",
        "sodiumextra",
        "polarium",
        "oculus",
        "embeddium",
        "rubidium",
        "indium",
        "vulkanmod",
        "optifine",
    ];
    let screen = crate::crash::latest_freeze_snapshot(text)
        .lines()
        .rev()
        .filter_map(|line| {
            let line = line
                .trim()
                .strip_prefix("[Arctic diagnostics] ")
                .unwrap_or(line.trim());
            line.strip_prefix("screen: ").or_else(|| {
                let (_, class) = line.strip_prefix("context: ")?.rsplit_once(", screen: ")?;
                Some(class)
            })
        })
        .next()
        .filter(|class| class.contains('.') && !class.contains(char::is_whitespace))
        .map(|class| Frame {
            class: class.to_owned(),
            method: String::new(),
            module: None,
            jar: None,
        });
    let index = Index::new(known);
    let reported = crate::crash_conflicts::from_crash_report(text);
    let inventory = crate::crash::compact_mod_inventory(text, "");
    let ids: HashMap<_, _> = inventory
        .lines()
        .chain(uploaded_mod_inventory(text).lines())
        .filter_map(|line| line.split_once(' ').map(|(id, _)| (norm(id), id)))
        .collect();

    screen
        .into_iter()
        .chain(frames.filter_map(frame))
        .find_map(|f| {
            if f.is_platform() || f.class.starts_with("net.fabricmc.fabric.") {
                return None;
            }
            let (package, _) = f.class.rsplit_once('.')?;
            if package
                .rsplit('.')
                .any(|part| RENDERERS.contains(&norm(part).as_str()))
            {
                return None;
            }
            let (id, name) = if let Some(i) = index.owner(&f) {
                let m = &known[i];
                if m.library {
                    return None;
                }
                (m.id.as_str(), m.name.as_str())
            } else if let Some(m) = f
                .config_owner(installed)
                .or_else(|| f.config_owner(&reported))
            {
                (m.id.as_str(), m.name.as_str())
            } else if package.split('.').any(|part| part == "randomcutscene") {
                ("randomcutscene", "Random Cutscene")
            } else {
                let id = package
                    .rsplit('.')
                    .find_map(|part| ids.get(&norm(part)).copied())
                    .unwrap_or(package);
                (id, id)
            };
            if BASE_IDS.contains(&id)
                || is_library(id, false)
                || RENDERERS.contains(&norm(id).as_str())
            {
                return None;
            }
            Some(Suspect {
                id: id.to_owned(),
                name: name.to_owned(),
                ours: is_arctic_id(id),
            })
        })
}

pub(crate) fn diagnose_unassigned_entity<'a>(
    text: &'a str,
    mods_dir: Option<&Path>,
    mods: &[InstalledMod],
) -> Option<(Diagnosis, &'a str)> {
    let (error, frames) = unassigned_entity_failure(text)?;
    let known = mods_dir.map(scan).unwrap_or_default();
    let (detail, names) = match unassigned_entity_culprit(text, frames, &known, mods) {
        Some(culprit) => (
            format!("{} appears to have created the entity. Update it for this Minecraft version or remove it.", culprit.name),
            vec![culprit.name],
        ),
        None => (
            "The log doesn't identify which mod created the entity. Check for updates to mods that spawn fake players, such as cutscene, replay, or NPC mods, or try disabling them one at a time.".to_owned(),
            Vec::new(),
        ),
    };
    Some((
        Diagnosis::new(UNASSIGNED_ENTITY_TITLE, detail, names),
        error,
    ))
}

/// The mod to blame for the crash in `text`, if the evidence is clear.
pub fn suspect(text: &str, mods: &[Known]) -> Option<Suspect> {
    if let Some((_, frames)) = unassigned_entity_failure(text) {
        return unassigned_entity_culprit(text, frames, mods, &[]);
    }
    let index = Index::new(mods);
    let named = |i: usize| {
        let m = &mods[i];
        Suspect {
            id: m.id.clone(),
            name: m.name.clone(),
            ours: is_arctic_id(&m.id),
        }
    };
    let secondary_report = crate::crash::crash_exception(text).is_some_and(secondary_stack);
    let by_game = if secondary_report {
        Vec::new()
    } else {
        suspected_line(text)
    };
    let mut game_library = None;
    for (id, name) in &by_game {
        let known = index.by_id(id);
        let library = known.map_or_else(|| is_library(id, false), |i| mods[i].library);
        let s = match known {
            Some(i) => named(i),
            None if is_arctic_id(id) => arctic(id),
            None => Suspect {
                id: id.clone(),
                name: name.clone(),
                ours: false,
            },
        };
        if library {
            game_library.get_or_insert(s);
        } else {
            return Some(s);
        }
    }
    let chain = fatal_chain(text);
    let earlier = text
        .rsplit_once("---- Minecraft Crash Report ----")
        .filter(|_| secondary_report)
        .and_then(|(earlier, _)| fatal_chain(earlier));
    for block in chain.iter().chain(&earlier).flatten() {
        for line in block {
            let Some(f) = frame(line) else {
                continue;
            };
            if let Some(owner) = mixin_owner(&f.method) {
                if is_arctic_id(owner) {
                    return Some(arctic(owner));
                }
                match index.by_id(owner) {
                    Some(i) if !mods[i].library => return Some(named(i)),
                    _ => continue,
                }
            }
            if f.class.starts_with(POLARIUM_PACKAGE) {
                return Some(arctic("polarium"));
            }
            if f.class.starts_with(ARCTIC_PACKAGE) {
                return Some(arctic("arctic"));
            }
            if f.is_platform() {
                continue;
            }
            match index.owner(&f) {
                Some(i) if BASE_IDS.contains(&mods[i].id.as_str()) => {}
                Some(i) if mods[i].library => {}
                Some(i) => return Some(named(i)),
                None => {}
            }
        }
    }
    game_library
}

/// Ids named on the game's own "Suspected Mod(s): Name (id), ..." line.
fn suspected_line(text: &str) -> Vec<(String, String)> {
    static LINE: OnceLock<Regex> = OnceLock::new();
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let line = LINE.get_or_init(|| Regex::new(r"(?m)^\s*Suspected Mods?:[ \t]*(.*)$").unwrap());
    let item = ITEM.get_or_init(|| Regex::new(r"([^,()\n]+?)\s*\(([A-Za-z0-9_.\-]+)\)").unwrap());
    let Some(c) = line.captures_iter(text).last() else {
        return Vec::new();
    };
    item.captures_iter(&c[1])
        .filter(|i| !BASE_IDS.contains(&&i[2]))
        .map(|i| (i[2].to_owned(), i[1].trim().to_owned()))
        .collect()
}

/// "Minecraft (minecraft)": a Suspected Mods entry that is the game or loader itself.
pub(crate) fn is_base_entry(entry: &str) -> bool {
    entry
        .rsplit_once('(')
        .and_then(|(_, id)| id.split(')').next())
        .is_some_and(|id| BASE_IDS.contains(&id))
}

/// Title for a named mod. The app recognises it (see [`named`]) to show a translation.
pub fn title(name: &str) -> String {
    format!("{name} crashed the game")
}

/// The diagnosis for `text`, if one mod can be named.
pub fn diagnosis(text: &str, mods: &[Known]) -> Option<Diagnosis> {
    let s = suspect(text, mods)?;
    let detail = if s.ours {
        format!(
            "The game crashed in {}, which is part of Arctic. Please send the crash report so it can be fixed.",
            s.name
        )
    } else {
        format!(
            "The game crashed inside {}. Update it to the newest version for this Minecraft version, or remove it on the instance's Mods page. If it keeps crashing, tell the mod's author.",
            s.name
        )
    };
    Some(Diagnosis::new(title(&s.name), detail, vec![s.name]))
}

/// If `d` is a diagnosis made by [`diagnosis`]: the mod's name and whether it is Arctic's own.
pub fn named(d: &Diagnosis) -> Option<(&str, bool)> {
    let name = d.mods.first()?;
    (d.mods.len() == 1 && d.title == title(name) && d.fix.is_none()).then(|| {
        let ours = name == ARCTIC_NAME || name == crate::mods::polarium::NAME;
        (name.as_str(), ours)
    })
}

/// Titles that only restate an exit code, which a named mod replaces.
pub fn is_generic(d: &Diagnosis) -> bool {
    d.title.starts_with("Game exited with code") || d.title.starts_with("The game stopped before")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crash_conflicts::{diagnose_exit_in, diagnose_in};
    use std::io::Write;

    const REPORT_458: &str = include_str!("../tests/fixtures/crash-458-threading-detector.log");
    const THREADING_REPORT: &str =
        include_str!("../tests/fixtures/threading-detector-with-worker.log");

    #[test]
    fn report_458_explains_the_threading_crash_without_a_culprit() {
        let dir = tempfile::tempdir().unwrap();
        let diagnosis = diagnose_exit_in(REPORT_458, -1, dir.path()).unwrap();
        assert_eq!(diagnosis.title, THREADING_TITLE);
        assert!(diagnosis.mods.is_empty());
        assert!(diagnosis.detail.contains("The culprit is unknown"));
        assert!(diagnosis.detail.contains("world.random off-thread"));
        assert!(diagnosis.detail.contains("Disable half"));

        let (log, report) = REPORT_458.split_once("---- crash report ----\n").unwrap();
        assert_eq!(crate::crash::diagnose(log), Some(diagnosis.clone()));
        let upload = crate::crash::Report::build(
            "1.20.1",
            "Fabric",
            "Game exited with code -1 (0xFFFFFFFF)",
            &[log],
            &[],
            Some(report),
            None,
        );
        assert_eq!(upload.title, diagnosis.title);
        assert!(upload.log.contains("matched: java.lang.IllegalStateException: Accessing LegacyRandomSource from multiple threads"));
    }

    #[test]
    fn threading_detector_uses_the_first_worker_mod_frame() {
        let dir = tempfile::tempdir().unwrap();
        jar(
            dir.path(),
            "example.jar",
            "different_mod_id",
            "Spark Particles",
            &["com.example.sparkparticles.async"],
            "",
        );
        let diagnosis = diagnose_exit_in(THREADING_REPORT, -1, dir.path()).unwrap();
        assert_eq!(diagnosis.title, THREADING_TITLE);
        assert_eq!(diagnosis.mods, ["different_mod_id"]);
        assert!(diagnosis.detail.contains("Spark Particles"));
        assert!(diagnosis.detail.contains("Update or remove that mod"));

        let uploaded = crate::crash::diagnose(THREADING_REPORT).unwrap();
        assert_eq!(uploaded.mods, ["spark_particles"]);

        let (main, worker) = THREADING_REPORT
            .split_once("Particle worker #1:\n")
            .unwrap();
        let (_, rest) = worker.split_once("Stacktrace:\n").unwrap();
        let render_only = format!("{main}Stacktrace:\n{rest}");
        let diagnosis = diagnose_exit_in(&render_only, -1, dir.path()).unwrap();
        assert_eq!(diagnosis.title, THREADING_TITLE);
        assert!(diagnosis.mods.is_empty());
        assert!(diagnosis.detail.contains("The culprit is unknown"));
    }

    /// A mod jar with a fabric.mod.json and empty class files in the given packages.
    fn jar(dir: &Path, file: &str, id: &str, name: &str, packages: &[&str], extra: &str) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(file)).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("fabric.mod.json", options).unwrap();
        write!(
            zip,
            r#"{{"schemaVersion":1,"id":"{id}","name":"{name}","version":"1.0.0"{extra}}}"#
        )
        .unwrap();
        for p in packages {
            zip.start_file(format!("{}/Main.class", p.replace('.', "/")), options)
                .unwrap();
            zip.write_all(b"\xca\xfe\xba\xbe").unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn unassigned_entity_uses_a_mod_caller_and_skips_renderer_frames() {
        let dir = tempfile::tempdir().unwrap();
        jar(
            dir.path(),
            "fake-players.jar",
            "fake_players",
            "Fake Players",
            &["example.cinematic"],
            "",
        );
        let text = "---- Minecraft Crash Report ----\n\
            java.lang.RuntimeException: Ticking entity\n\
            Caused by: java.lang.IllegalStateException: Tried to access entity ID before ID assignment\n\
            \tat knot//net.minecraft.world.entity.Entity.getId(Entity.java:446)\n\
            \tat knot//net.minecraft.client.renderer.entity.EntityRenderDispatcher.handler$abc000$sodium$render(EntityRenderDispatcher.java:10)\n\
            \tat knot//net.irisshaders.iris.render.EntityTracker.add(EntityTracker.java:10)\n\
            \tat knot//net.minecraft.client.multiplayer.ClientLevel.addEntity(ClientLevel.java:11)\n\
            \tat knot//example.cinematic.PlayerSpawner.spawn(PlayerSpawner.java:12)\n\n\
            Suspected Mods: Sodium (sodium)";
        let d = diagnose_exit_in(text, 1, dir.path()).unwrap();
        assert_eq!(d.title, UNASSIGNED_ENTITY_TITLE);
        assert_eq!(d.mods, ["Fake Players"]);
        assert_eq!(suspect(text, &scan(dir.path())).unwrap().id, "fake_players");

        let text = text.replace(
            "\tat knot//example.cinematic.PlayerSpawner.spawn(PlayerSpawner.java:12)\n",
            "",
        );
        let d = diagnose_exit_in(&text, 1, dir.path()).unwrap();
        assert_eq!(d.title, UNASSIGNED_ENTITY_TITLE);
        assert!(d.mods.is_empty());
        assert!(d.detail.contains("mods that spawn fake players"));
        assert!(suspect(&text, &scan(dir.path())).is_none());
    }

    fn pack() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        jar(
            p,
            "enhancedblockentities-0.9.jar",
            "enhancedblockentities",
            "Enhanced Block Entities",
            &[
                "foundationgames.enhancedblockentities",
                "foundationgames.enhancedblockentities.util",
                "foundationgames.enhancedblockentities.client.screen",
            ],
            "",
        );
        jar(
            p,
            "sodium-0.5.jar",
            "sodium",
            "Sodium",
            &["me.jellysquid.mods.sodium.client.render"],
            "",
        );
        jar(
            p,
            "lithium-0.11.jar",
            "lithium",
            "Lithium",
            &["me.jellysquid.mods.lithium.common"],
            "",
        );
        jar(
            p,
            "cloth-config-11.jar",
            "cloth-config",
            "Cloth Config API",
            &["me.shedaniel.clothconfig2.gui"],
            "",
        );
        jar(
            p,
            "somelib.jar",
            "somelib",
            "Some Lib",
            &["net.example.somelib"],
            r#","custom":{"modmenu":{"badges":["library"]}}"#,
        );
        dir
    }

    #[test]
    fn a_secondary_report_skips_its_frames_and_suspected_mods() {
        let dir = pack();
        let report = "---- Minecraft Crash Report ----\n\n\
            java.lang.IllegalStateException: Another mod has failed to load\n\
            \tat me.jellysquid.mods.sodium.client.render.Renderer.start(Renderer.java:12)\n\n\
            Suspected Mods: Sodium (sodium)";
        assert!(diagnose_in(report, dir.path()).is_none());

        let earlier = "Exception in thread \"main\" java.lang.IllegalStateException: Startup failed\n\
            \tat me.jellysquid.mods.lithium.common.Startup.init(Startup.java:25)";
        let log = format!("{earlier}\n\n{report}");
        let d = diagnose_in(&log, dir.path()).unwrap();
        assert_eq!(d.title, "Lithium crashed the game");
    }

    const EBE: &str = "---- Minecraft Crash Report ----\n\
// Don't do that.\n\n\
Time: 2026-11-17 20:11:03\n\
Description: Unexpected error\n\n\
java.nio.file.ProviderMismatchException\n\
\tat java.base/sun.nio.fs.AbstractPath.toAbsolutePath(AbstractPath.java:90)\n\
\tat java.base/java.nio.file.Path.of(Path.java:208)\n\
\tat foundationgames.enhancedblockentities.util.ResourceUtil.dumpResources(ResourceUtil.java:105)\n\
\tat foundationgames.enhancedblockentities.client.screen.ConfigScreen.method_25402(ConfigScreen.java:88)\n\
\tat net.minecraft.class_437.method_25402(class_437.java:300)\n\
\tat net.minecraft.class_310.method_1523(class_310.java:1234)\n\n\
-- System Details --\n\
Details:\n\
\tMinecraft Version: 1.20.1\n";

    #[test]
    fn enhanced_block_entities_crash_names_the_mod() {
        let dir = pack();
        let d = diagnose_exit_in(EBE, -1, dir.path()).unwrap();
        assert_eq!(d.title, "Enhanced Block Entities crashed the game");
        assert_eq!(d.mods, ["Enhanced Block Entities"]);
        assert!(d.detail.contains("Update it"));
        assert_eq!(named(&d), Some(("Enhanced Block Entities", false)));
        // The same text without an exit code reaches the same answer.
        assert_eq!(diagnose_in(EBE, dir.path()).unwrap(), d);
    }

    #[test]
    fn game_suspected_mods_line_wins_over_frames() {
        let dir = pack();
        let text = EBE.replace(
            "-- System Details --",
            "-- System Details --\nSuspected Mods: Lithium (lithium)",
        );
        let found = suspect(&text, &scan(dir.path())).unwrap();
        assert_eq!(
            (found.id.as_str(), found.name.as_str()),
            ("lithium", "Lithium")
        );
        // Fabric's own rule already words this one; it must still name the mod.
        let d = diagnose_exit_in(&text, -1, dir.path()).unwrap();
        assert!(d.title.contains("Lithium"), "{}", d.title);
        // "NONE" and loader entries say nothing, so the frames decide.
        for line in [
            "Suspected Mods: NONE",
            "Suspected Mods: Minecraft (minecraft)",
        ] {
            let text = EBE.replace(
                "-- System Details --",
                &format!("-- System Details --\n{line}"),
            );
            let d = diagnose_exit_in(&text, -1, dir.path()).unwrap();
            assert_eq!(d.title, "Enhanced Block Entities crashed the game");
        }
        // A library on the game's line gives way to a real mod; mods missing on disk keep the game's name.
        let text = EBE.replace(
            "-- System Details --",
            "-- System Details --\nSuspected Mods: Cloth Config API (cloth-config), Ghost Mod (ghost)",
        );
        let found = suspect(&text, &scan(dir.path())).unwrap();
        assert_eq!(
            (found.id.as_str(), found.name.as_str()),
            ("ghost", "Ghost Mod")
        );
    }

    #[test]
    fn mixin_handler_frame_names_the_mixin_owner() {
        let dir = pack();
        let text = "Exception in thread \"Render thread\" java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\"\n\
\tat net.minecraft.class_761.handler$zbh000$sodium$onRender(class_761.java:12) ~[client.jar:?]\n\
\tat net.minecraft.class_761.method_22710(class_761.java:1) ~[client.jar:?]\n\
\tat net.minecraft.class_757.method_3192(class_757.java:900) ~[client.jar:?]\n\
\tat net.minecraft.class_310.method_1523(class_310.java:1200) ~[client.jar:?]\n";
        let d = diagnose_exit_in(text, 1, dir.path()).unwrap();
        assert_eq!(d.title, "Sodium crashed the game");
        // Unknown or library owners are not blamed.
        let lib = text.replace("sodium", "fabric-rendering-v1");
        assert!(diagnose_exit_in(&lib, 1, dir.path()).is_none_or(|d| d.mods.is_empty()));
    }

    #[test]
    fn library_frames_are_skipped_for_the_mod_that_used_them() {
        let dir = pack();
        let text = "[12:00:01] [Render thread/ERROR]: Reported exception thrown!\n\
net.minecraft.class_148: Rendering screen\n\
\tat net.minecraft.class_310.method_1523(class_310.java:1)\n\
Caused by: java.lang.IllegalStateException: no\n\
\tat me.shedaniel.clothconfig2.gui.Entry.render(Entry.java:5)\n\
\tat foundationgames.enhancedblockentities.client.screen.ConfigScreen.method_25394(ConfigScreen.java:1)\n\
\tat net.minecraft.class_437.method_25394(class_437.java:3)\n";
        let d = diagnose_exit_in(text, -1, dir.path()).unwrap();
        assert_eq!(d.title, "Enhanced Block Entities crashed the game");
        // Only library frames: nobody is named.
        let only = text.replace("\tat foundationgames.enhancedblockentities.client.screen.ConfigScreen.method_25394(ConfigScreen.java:1)\n", "");
        let d = diagnose_exit_in(&only, -1, dir.path());
        assert!(d.is_none_or(|d| d.mods.is_empty() && !d.title.contains("crashed the game")));
    }

    #[test]
    fn vanilla_and_driver_crashes_name_no_mod() {
        let dir = pack();
        let text = "---- Minecraft Crash Report ----\n\n\
Description: Rendering overlay\n\n\
java.lang.IllegalStateException: Failed to create window\n\
\tat net.minecraft.class_310.method_1523(class_310.java:1234)\n\
\tat org.lwjgl.glfw.GLFW.glfwCreateWindow(GLFW.java:1)\n\
\tat com.mojang.blaze3d.systems.RenderSystem.initRenderer(RenderSystem.java:5)\n\
\tat java.base/java.lang.Thread.run(Thread.java:833)\n\n";
        let d = diagnose_exit_in(text, -1, dir.path());
        assert!(d.is_none_or(|d| !d.title.contains("crashed the game")));
        assert!(suspect(text, &scan(dir.path())).is_none());
        // A logged (non-fatal) exception in a mod is not the reason for a native exit.
        let log = "[main/WARN]: caught\njava.io.IOException: x\n\tat foundationgames.enhancedblockentities.util.ResourceUtil.a(R.java:1)\n[main/INFO]: bye\n";
        assert!(suspect(log, &scan(dir.path())).is_none());
    }

    #[test]
    fn arctic_frames_name_arctic_only_when_they_are_on_top() {
        let dir = pack();
        let top = "---- Minecraft Crash Report ----\n\n\
java.lang.RuntimeException: boom\n\
\tat com.arcticlauncher.mod.NameTagDecor.render(NameTagDecor.java:40)\n\
\tat foundationgames.enhancedblockentities.util.ResourceUtil.a(ResourceUtil.java:1)\n\
\tat net.minecraft.class_310.method_1523(class_310.java:1)\n\n";
        let d = diagnose_exit_in(top, -1, dir.path()).unwrap();
        assert_eq!(d.title, "Arctic Client crashed the game");
        assert_eq!(named(&d), Some(("Arctic Client", true)));
        assert!(d.detail.contains("send the crash report"));
        let polarium = top.replace(
            "com.arcticlauncher.mod.NameTagDecor",
            "com.arcticlauncher.polarium.ChestBatch",
        );
        assert_eq!(
            diagnose_exit_in(&polarium, -1, dir.path()).unwrap().title,
            "Polarium crashed the game"
        );
        // Another mod above Arctic's frames is the culprit.
        let below = "---- Minecraft Crash Report ----\n\n\
java.lang.RuntimeException: boom\n\
\tat me.jellysquid.mods.lithium.common.Cache.get(Cache.java:1)\n\
\tat com.arcticlauncher.mod.NameTagDecor.render(NameTagDecor.java:40)\n\n";
        assert_eq!(
            diagnose_exit_in(below, -1, dir.path()).unwrap().title,
            "Lithium crashed the game"
        );
    }

    #[test]
    fn forge_module_prefix_and_jar_hint_map_frames() {
        let dir = pack();
        let forge = "Exception in thread \"main\" java.lang.RuntimeException: x\n\
\tat TRANSFORMER/sodium@0.5/me.example.Unseen.run(Unseen.java:3)\n";
        assert_eq!(suspect(forge, &scan(dir.path())).unwrap().id, "sodium");
        let hint = "Exception in thread \"main\" java.lang.RuntimeException: x\n\
\tat knot//com.unknown.pkg.Thing.run(Thing.java:3) ~[lithium-0.11.jar:?]\n";
        assert_eq!(suspect(hint, &scan(dir.path())).unwrap().id, "lithium");
    }
}
