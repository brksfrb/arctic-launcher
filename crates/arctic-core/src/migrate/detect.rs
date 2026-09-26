//! What a version folder runs (official launcher, TLauncher), and which
//! loader a mod jar is for.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::FoundLoader;
use crate::loaders::LoaderKind;

/// What a version id resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub game_version: String,
    pub loader: Option<FoundLoader>,
    /// OptiFine's jar, when the version adds OptiFine.
    pub optifine: Option<PathBuf>,
    pub notes: Vec<String>,
}

static FABRIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"net\.fabricmc:fabric-loader:([0-9][\w.+-]*)").unwrap());
static QUILT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"org\.quiltmc:quilt-loader:([0-9][\w.+-]*)").unwrap());
static INTERMEDIARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:net\.fabricmc:intermediary|org\.quiltmc:hashed):([0-9][\w.+-]*)").unwrap()
});
static FORGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"net[./]minecraftforge[:/](?:forge|fmlloader|fmlcore)[:/]([0-9][\w.]*)-([0-9][\w.]*)",
    )
    .unwrap()
});
static NEOFORGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"net[./]neoforged[:/]neoforge[:/]([0-9]+\.[0-9]+\.[0-9]+[\w.+-]*?)(?:[:/\x22]|-client|-universal)")
        .unwrap()
});
static NEO_FORGE_1201: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"net[./]neoforged[:/]forge[:/]").unwrap());
static OPTIFINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"optifine:OptiFine:(?:OptiFine-)?([0-9][\w.]*_HD_\w+)").unwrap());
static CLIENT_LIB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"net/minecraft/client/([0-9][\w.-]*?)-[0-9]{8}\.[0-9]{6}/").unwrap()
});

/// A version JSON (a UTF-8 BOM is fine; TLauncher writes one).
pub fn read_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    serde_json::from_slice(bytes).ok()
}

/// Client jar SHA-1 → Minecraft version, from the plain versions installed
/// next to it (TLauncher drops the fields that name the version).
pub fn vanilla_by_client(versions_dir: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(versions_dir) else {
        return out;
    };
    for e in entries.flatten() {
        let id = e.file_name().to_string_lossy().into_owned();
        let Some(json) = read_json(&e.path().join(format!("{id}.json"))) else {
            continue;
        };
        let plain = json.get("inheritsFrom").is_none()
            && json.get("mainClass").and_then(Value::as_str)
                == Some("net.minecraft.client.main.Main");
        if let (true, Some(sha1)) = (plain, client_sha1(&json)) {
            out.entry(sha1).or_insert(id);
        }
    }
    out
}

fn client_sha1(json: &Value) -> Option<String> {
    json.pointer("/downloads/client/sha1")?
        .as_str()
        .map(str::to_owned)
}

/// Resolve version `id` in `versions_dir`: Minecraft version, loader,
/// OptiFine. `None` when it can't be told.
pub fn version_info(
    versions_dir: &Path,
    id: &str,
    vanilla: &HashMap<String, String>,
) -> Option<VersionInfo> {
    resolve(versions_dir, id, vanilla, 0)
}

fn resolve(
    versions_dir: &Path,
    id: &str,
    vanilla: &HashMap<String, String>,
    depth: u8,
) -> Option<VersionInfo> {
    let dir = versions_dir.join(id);
    let json = read_json(&dir.join(format!("{id}.json")))?;
    let mut text = json.to_string();
    // TLauncher keeps the loader's library list (and the base version) here.
    let extra = read_json(&dir.join("TLauncherAdditional.json"));
    if let Some(extra) = &extra {
        text.push_str(&extra.to_string());
    }
    let base_jar = extra
        .as_ref()
        .and_then(|e| e.get("jar").and_then(Value::as_str))
        .map(str::to_owned);
    let args = fml_args(&json);
    let parent = json
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .filter(|_| depth < 4);
    let parent_info = parent.and_then(|p| resolve(versions_dir, p, vanilla, depth + 1));
    let mut notes = Vec::new();
    let loader = loader_of(&text, &args, &mut notes)
        .or_else(|| parent_info.as_ref().and_then(|p| p.loader.clone()));
    let game_version = args
        .get("mcVersion")
        .cloned()
        .or_else(|| FORGE.captures(&text).map(|c| c[1].to_owned()))
        .or_else(|| INTERMEDIARY.captures(&text).map(|c| c[1].to_owned()))
        .or_else(|| parent_info.as_ref().map(|p| p.game_version.clone()))
        .or_else(|| parent.map(str::to_owned))
        .or(base_jar)
        .or_else(|| CLIENT_LIB.captures(&text).map(|c| c[1].to_owned()))
        .or_else(|| client_sha1(&json).and_then(|s| vanilla.get(&s).cloned()))
        .or_else(|| {
            let plain = json.get("mainClass").and_then(Value::as_str)
                == Some("net.minecraft.client.main.Main");
            plain.then(|| id.to_owned())
        })?;
    let optifine = OPTIFINE.captures(&text).map(|c| {
        let v = &c[1];
        let root = versions_dir.parent().unwrap_or(versions_dir);
        let name = format!("OptiFine-{v}");
        root.join("libraries/optifine/OptiFine")
            .join(&name)
            .join(format!("{name}.jar"))
    });
    let optifine = optifine
        .filter(|p| p.is_file())
        .or_else(|| parent_info.as_ref().and_then(|p| p.optifine.clone()));
    if optifine.is_some() && loader.is_none() {
        notes.push(
            "OptiFine needs Forge here; you get Sodium and friends instead (add Iris for shaders)"
                .into(),
        );
    }
    notes.extend(parent_info.map(|p| p.notes).unwrap_or_default());
    notes.dedup();
    Some(VersionInfo {
        game_version,
        loader,
        optifine,
        notes,
    })
}

fn loader_of(
    text: &str,
    args: &HashMap<String, String>,
    notes: &mut Vec<String>,
) -> Option<FoundLoader> {
    let exact = |kind, v: &str| {
        Some(FoundLoader {
            kind,
            version: Some(v.to_owned()),
        })
    };
    if let Some(v) = args.get("neoForgeVersion") {
        return exact(LoaderKind::NeoForge, v);
    }
    if let Some(c) = NEOFORGE.captures(text) {
        return exact(LoaderKind::NeoForge, &c[1]);
    }
    if NEO_FORGE_1201.is_match(text) {
        notes.push("NeoForge for 1.20.1 isn't supported; this comes over without a loader".into());
        return None;
    }
    if let Some(v) = args.get("forgeVersion") {
        return exact(LoaderKind::Forge, v);
    }
    if let Some(c) = FORGE.captures(text) {
        return exact(LoaderKind::Forge, &c[2]);
    }
    if let Some(c) = QUILT.captures(text) {
        return exact(LoaderKind::Quilt, &c[1]);
    }
    if let Some(c) = FABRIC.captures(text) {
        return exact(LoaderKind::Fabric, &c[1]);
    }
    None
}

/// `--fml.forgeVersion 47.2.0` style game arguments.
fn fml_args(json: &Value) -> HashMap<String, String> {
    let list: Vec<&str> = json
        .pointer("/arguments/game")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    list.windows(2)
        .filter_map(|w| {
            let key = w[0].strip_prefix("--fml.")?;
            Some((key.to_owned(), w[1].to_owned()))
        })
        .collect()
}

/// A jar's description files for `loader`, in the order the loader reads
/// them. Jars made for several loaders carry one per loader.
pub fn metadata_files(loader: LoaderKind) -> &'static [&'static str] {
    match loader {
        LoaderKind::Fabric => &["fabric.mod.json"],
        LoaderKind::Quilt => &["quilt.mod.json", "fabric.mod.json"],
        LoaderKind::Forge => &["META-INF/mods.toml", "mcmod.info"],
        LoaderKind::NeoForge => &["META-INF/neoforge.mods.toml", "META-INF/mods.toml"],
    }
}

const ALL_METADATA: [&str; 5] = [
    "fabric.mod.json",
    "quilt.mod.json",
    "META-INF/mods.toml",
    "META-INF/neoforge.mods.toml",
    "mcmod.info",
];

/// Could this jar load on `loader`? Jars that describe no loader at all
/// (OptiFine, libraries) are kept.
pub fn jar_fits(path: &Path, loader: Option<LoaderKind>) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return true;
    };
    let Ok(zip) = zip::ZipArchive::new(file) else {
        return true;
    };
    let has = |name: &str| zip.index_for_name(name).is_some();
    if !ALL_METADATA.iter().any(|m| has(m)) {
        return true;
    }
    loader.is_some_and(|l| metadata_files(l).iter().any(|m| has(m)))
}

/// `-Xmx4G` / `-Xmx4096M` in JVM arguments, in MiB.
pub fn xmx_mb(args: &str) -> Option<u32> {
    let v = args
        .split_whitespace()
        .find_map(|a| a.strip_prefix("-Xmx"))?;
    let (num, unit) = v.split_at(v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len()));
    let n: u32 = num.parse().ok()?;
    match unit.to_ascii_lowercase().as_str() {
        "g" => n.checked_mul(1024),
        "m" | "" => Some(n),
        "k" => Some(n / 1024),
        _ => None,
    }
}

/// Mod id and version from a jar's `fabric.mod.json` / `quilt.mod.json`
/// / `mods.toml`.
fn mod_identity(path: &Path) -> Option<(String, String)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut read = |name: &str| {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(name).ok()?, &mut s).ok()?;
        Some(s)
    };
    if let Some(text) = read("fabric.mod.json") {
        // Some mods put raw newlines inside strings; lenient enough for id/version.
        let json: Value = serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok()?;
        return Some((
            json.get("id")?.as_str()?.to_owned(),
            json.get("version")?.as_str()?.to_owned(),
        ));
    }
    if let Some(text) = read("quilt.mod.json") {
        let json: Value = serde_json::from_str(&text).ok()?;
        let q = json.get("quilt_loader")?;
        return Some((
            q.get("id")?.as_str()?.to_owned(),
            q.get("version")?.as_str()?.to_owned(),
        ));
    }
    let toml = read("META-INF/neoforge.mods.toml").or_else(|| read("META-INF/mods.toml"))?;
    let field = |key: &str| {
        toml.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_owned())
        })
    };
    let version = field("version").filter(|v| !v.contains("${"))?;
    Some((field("modId")?, version))
}

/// Of mods sharing an id (two Fabric API versions), keep the newest.
pub fn newest_only(files: Vec<(PathBuf, String)>) -> Vec<(PathBuf, String)> {
    let ids: Vec<Option<(String, String)>> = files.iter().map(|(p, _)| mod_identity(p)).collect();
    let mut keep = vec![true; files.len()];
    for i in 0..files.len() {
        for j in 0..files.len() {
            let (Some((a, va)), Some((b, vb))) = (&ids[i], &ids[j]) else {
                continue;
            };
            if i != j && a == b && keep[i] && keep[j] {
                let loser = if newer(va, vb) { j } else { i };
                keep[loser] = false;
            }
        }
    }
    files
        .into_iter()
        .zip(keep)
        .filter_map(|(f, k)| k.then_some(f))
        .collect()
}

/// Version order on the numbers in them (`0.141.3` > `0.140.0`).
fn newer(a: &str, b: &str) -> bool {
    let nums = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    nums(a) >= nums(b)
}
