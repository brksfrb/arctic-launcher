//! Running the client-side "processors" of a Forge/NeoForge installer
//! (the steps that deobfuscate and patch the vanilla client jar).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

use crate::error::IoContext;
use crate::net::{DownloadJob, download_all, sha1_file};
use crate::storage::load_json;
use crate::versions::maven_path;
use crate::{Error, Progress, ProgressInfo, Result};

/// Lines of processor output quoted in error messages.
const ERROR_TAIL_LINES: usize = 12;
/// `CREATE_NO_WINDOW`: no console window flashes up while processors run.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Deserialize)]
pub struct Processor {
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Output file → expected sha1 (both may contain `{KEY}`s).
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    /// `None` = all sides.
    pub sides: Option<Vec<String>>,
}

impl Processor {
    pub fn runs_on_client(&self) -> bool {
        self.sides
            .as_ref()
            .is_none_or(|s| s.iter().any(|side| side == "client"))
    }
}

/// Everything processors need: resolved `{KEY}` values and where things live.
pub struct Context {
    pub vars: HashMap<String, String>,
    pub libraries: PathBuf,
    pub java: PathBuf,
    /// Raw vanilla version JSON (for the mappings download).
    pub vanilla_json: PathBuf,
}

/// Library path of a Maven coordinate.
pub fn lib_path(libraries: &Path, coords: &str) -> Result<PathBuf> {
    maven_path(coords)
        .map(|rel| libraries.join(rel))
        .ok_or_else(|| Error::Other(format!("invalid library name in installer: {coords}")))
}

/// Resolve an installer `data` value: `[maven:coords]` → library path,
/// `'literal'` → literal, `/path` → file extracted from the installer.
pub fn resolve_data_value(
    raw: &str,
    libraries: &Path,
    extract: &mut dyn FnMut(&str) -> Result<PathBuf>,
) -> Result<String> {
    if let Some(coords) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return Ok(lib_path(libraries, coords)?.display().to_string());
    }
    if let Some(lit) = raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return Ok(lit.to_owned());
    }
    if raw.starts_with('/') {
        return Ok(extract(raw)?.display().to_string());
    }
    Ok(raw.to_owned())
}

impl Context {
    /// Resolve one processor argument.
    pub fn resolve(&self, arg: &str) -> Result<String> {
        if let Some(coords) = arg.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            return Ok(lib_path(&self.libraries, coords)?.display().to_string());
        }
        if let Some(lit) = arg.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
            return Ok(lit.to_owned());
        }
        let mut out = String::with_capacity(arg.len());
        let mut rest = arg;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start..].find('}') else {
                break;
            };
            let key = &rest[start + 1..start + len];
            let value = self.vars.get(key).ok_or_else(|| {
                Error::Other(format!("installer step uses unknown value {{{key}}}"))
            })?;
            out.push_str(&rest[..start]);
            out.push_str(value);
            rest = &rest[start + len + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn outputs(&self, p: &Processor) -> Result<Vec<(PathBuf, String)>> {
        p.outputs
            .iter()
            .map(|(file, sha)| Ok((PathBuf::from(self.resolve(file)?), self.resolve(sha)?)))
            .collect()
    }
}

/// Run every client processor in order, skipping ones whose outputs are
/// already present and verified.
pub fn run_all(ctx: &Context, processors: &[&Processor], progress: Progress) -> Result<()> {
    for (i, p) in processors.iter().enumerate() {
        let stage = format!("Patching Minecraft ({}/{})", i + 1, processors.len());
        progress(ProgressInfo {
            stage: &stage,
            done: i as u64,
            total: processors.len() as u64,
            ..ProgressInfo::default()
        });
        let outputs = ctx.outputs(p)?;
        if !outputs.is_empty() && outputs.iter().all(|(f, sha)| matches_sha1(f, sha)) {
            log::debug!("processor {} up to date", p.jar);
            continue;
        }
        let args = p
            .args
            .iter()
            .map(|a| ctx.resolve(a))
            .collect::<Result<Vec<_>>>()?;
        if !download_mojmaps(ctx, &args, progress)? {
            run_one(ctx, p, &args)?;
        }
        verify_outputs(p, &outputs)?;
    }
    Ok(())
}

fn matches_sha1(file: &Path, sha: &str) -> bool {
    file.is_file() && sha1_file(file).is_ok_and(|h| h.eq_ignore_ascii_case(sha))
}

fn verify_outputs(p: &Processor, outputs: &[(PathBuf, String)]) -> Result<()> {
    match outputs.iter().find(|(f, sha)| !matches_sha1(f, sha)) {
        Some((file, _)) => Err(Error::Other(format!(
            "installer step {} produced an invalid {}",
            p.jar,
            file.display()
        ))),
        None => Ok(()),
    }
}

fn run_one(ctx: &Context, p: &Processor, args: &[String]) -> Result<()> {
    let jar = lib_path(&ctx.libraries, &p.jar)?;
    let main = main_class(&jar)?;
    let mut classpath = vec![jar];
    for coords in &p.classpath {
        classpath.push(lib_path(&ctx.libraries, coords)?);
    }
    let classpath = std::env::join_paths(&classpath)
        .map_err(|e| Error::Other(format!("invalid installer classpath: {e}")))?;
    let mut cmd = Command::new(&ctx.java);
    cmd.arg("-cp").arg(classpath).arg(&main).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    log::info!("running installer step {main} {}", args.join(" "));
    let out = cmd.output().at(&ctx.java)?;
    if out.status.success() {
        return Ok(());
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let tail: Vec<&str> = text.lines().rev().take(ERROR_TAIL_LINES).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(Error::Other(format!(
        "installer step {} failed ({}):\n{}",
        p.jar,
        out.status,
        tail.join("\n")
    )))
}

/// `Main-Class` from a jar's manifest.
fn main_class(jar: &Path) -> Result<String> {
    let file = File::open(jar).at(jar)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::Other(format!("{} is not a valid jar: {e}", jar.display())))?;
    let mut text = String::new();
    zip.by_name("META-INF/MANIFEST.MF")
        .map_err(|e| Error::Other(format!("{} has no manifest: {e}", jar.display())))?
        .read_to_string(&mut text)
        .at(jar)?;
    manifest_main_class(&text)
        .ok_or_else(|| Error::Other(format!("{} has no Main-Class", jar.display())))
}

/// Parse `Main-Class`, honouring 72-byte continuation lines.
fn manifest_main_class(manifest: &str) -> Option<String> {
    let mut joined: Vec<String> = Vec::new();
    for line in manifest.lines() {
        match (line.strip_prefix(' '), joined.last_mut()) {
            (Some(cont), Some(last)) => last.push_str(cont),
            _ => joined.push(line.to_owned()),
        }
    }
    joined.iter().find_map(|l| {
        l.strip_prefix("Main-Class:")
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    })
}

/// The `DOWNLOAD_MOJMAPS` task fetches Mojang's mappings from Java, which
/// fails behind TLS-intercepting antivirus (Java has its own trust store).
/// Download them here instead. Returns `false` to fall back to Java.
fn download_mojmaps(ctx: &Context, args: &[String], progress: Progress) -> Result<bool> {
    if !args.iter().any(|a| a == "DOWNLOAD_MOJMAPS") {
        return Ok(false);
    }
    let side = arg_after(args, "--side").unwrap_or("client");
    let (Some(output), Some(raw)) = (
        arg_after(args, "--output"),
        load_json::<Value>(&ctx.vanilla_json)?,
    ) else {
        return Ok(false);
    };
    let Some(job) = mappings_job(&raw, side, Path::new(output)) else {
        return Ok(false);
    };
    download_all("Downloading mappings", vec![job], progress)?;
    Ok(true)
}

fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).map(String::as_str)
}

fn mappings_job(vanilla: &Value, side: &str, dest: &Path) -> Option<DownloadJob> {
    let m = vanilla.get("downloads")?.get(format!("{side}_mappings"))?;
    Some(DownloadJob {
        url: m.get("url")?.as_str()?.to_owned(),
        sha1: m.get("sha1").and_then(Value::as_str).map(str::to_owned),
        size: m.get("size").and_then(Value::as_u64),
        dest: dest.to_path_buf(),
        lzma: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        let vars = HashMap::from([
            ("SIDE".to_string(), "client".to_string()),
            ("MC_SLIM_SHA".to_string(), "abc".to_string()),
            ("ROOT".to_string(), "/r".to_string()),
        ]);
        Context {
            vars,
            libraries: PathBuf::from("/libs"),
            java: PathBuf::from("java"),
            vanilla_json: PathBuf::new(),
        }
    }

    #[test]
    fn resolves_arguments() {
        let c = ctx();
        assert_eq!(c.resolve("{SIDE}").unwrap(), "client");
        assert_eq!(c.resolve("{ROOT}/libraries/").unwrap(), "/r/libraries/");
        assert_eq!(c.resolve("'lit'").unwrap(), "lit");
        assert_eq!(c.resolve("--task").unwrap(), "--task");
        let lib = c.resolve("[a.b:c:1:mappings@txt]").unwrap();
        assert_eq!(
            PathBuf::from(lib),
            Path::new("/libs").join("a/b/c/1/c-1-mappings.txt")
        );
        assert!(c.resolve("{MISSING}").is_err());
    }

    #[test]
    fn resolves_data_values() {
        let libs = Path::new("/libs");
        let mut extracted = Vec::new();
        let mut extract = |p: &str| {
            extracted.push(p.to_owned());
            Ok(PathBuf::from("/tmp/x").join(p.trim_start_matches('/')))
        };
        assert_eq!(
            resolve_data_value("'de86'", libs, &mut extract).unwrap(),
            "de86"
        );
        assert!(
            resolve_data_value("[net.minecraft:client:1.20.1:slim]", libs, &mut extract)
                .unwrap()
                .ends_with("client-1.20.1-slim.jar")
        );
        let lzma = resolve_data_value("/data/client.lzma", libs, &mut extract).unwrap();
        assert!(lzma.ends_with("client.lzma"));
        assert!(resolve_data_value("[broken]", libs, &mut extract).is_err());
        assert_eq!(extracted, ["/data/client.lzma"]);
    }

    #[test]
    fn filters_by_side() {
        let p: Vec<Processor> = serde_json::from_str(
            r#"[{"jar":"a:b:1","sides":["server"]},{"jar":"a:c:1","sides":["client"]},{"jar":"a:d:1","args":["x"]}]"#,
        )
        .unwrap();
        let client: Vec<&str> = p
            .iter()
            .filter(|p| p.runs_on_client())
            .map(|p| p.jar.as_str())
            .collect();
        assert_eq!(client, ["a:c:1", "a:d:1"]);
    }

    #[test]
    fn parses_manifest_main_class() {
        let mf = "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.installertools.Consol\r\n eTool\r\nX: y\r\n";
        assert_eq!(
            manifest_main_class(mf).unwrap(),
            "net.minecraftforge.installertools.ConsoleTool"
        );
        assert!(manifest_main_class("Manifest-Version: 1.0\n").is_none());
    }

    #[test]
    fn builds_mappings_job_from_vanilla_json() {
        let v: Value = serde_json::from_str(
            r#"{"downloads":{"client_mappings":{"sha1":"s","size":5,"url":"https://x/m.txt"}}}"#,
        )
        .unwrap();
        let job = mappings_job(&v, "client", Path::new("/o.txt")).unwrap();
        assert_eq!((job.url.as_str(), job.size), ("https://x/m.txt", Some(5)));
        assert!(mappings_job(&v, "server", Path::new("/o.txt")).is_none());
        let args: Vec<String> = ["--task", "DOWNLOAD_MOJMAPS", "--output", "/o"]
            .map(String::from)
            .to_vec();
        assert_eq!(arg_after(&args, "--output"), Some("/o"));
        assert_eq!(arg_after(&args, "--side"), None);
    }
}
