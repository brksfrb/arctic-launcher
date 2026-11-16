//! Look inside a Modrinth modpack version without installing it: which mods it
//! has (names, icons, versions, sides) and how many configs, resource packs and
//! shader packs it ships.
//!
//! The pack's `modrinth.index.json` lists every downloadable file, so that is the
//! main source; reading it needs only the zip's central directory and the one
//! index entry, fetched with HTTP `Range` requests ([`RangeReader`]) instead of the
//! whole `.mrpack`. The index paths and the `overrides/` listing give the config
//! and pack counts. Files are matched to Modrinth projects by SHA-1 (batch
//! `POST /version_files`), then names and icons come from `GET /projects?ids=`.
//! The version's own `dependencies` (when the author filled them in) add the
//! projects the index doesn't list.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Seek, SeekFrom};

use super::identify;
use super::modpack::{PackIndex, check_url, sanitize};
use super::modrinth::{self, Dependency, Project, Version};
use crate::net::{DownloadJob, agent, download_all};
use crate::storage::DataDirs;
use crate::{Error, Result};

const INDEX: &str = "modrinth.index.json";
/// Largest index file we read (it is JSON, a few MB at most for huge packs).
const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
/// Bytes per range request. Big enough that the tail (end of central directory and
/// most of the directory) and a typical index arrive in one or two requests.
const BLOCK: u64 = 256 * 1024;
/// Hashes per `version_files` request / ids per `projects` request.
const HASH_BATCH: usize = 100;
const ID_BATCH: usize = 50;

/// Which side(s) a mod is needed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Both,
    Client,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewMod {
    /// Project title, or the file name when Modrinth doesn't know the file.
    pub name: String,
    pub icon_url: Option<String>,
    /// Version number ("" when unknown).
    pub version: String,
    pub side: Side,
    /// Modrinth project, if the file is on Modrinth.
    pub project_id: Option<String>,
    /// The project's short summary ("" when unknown).
    pub description: String,
    /// Names of the projects this mod requires, sorted.
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackPreview {
    pub mods: Vec<PreviewMod>,
    pub configs: usize,
    pub resource_packs: usize,
    pub shader_packs: usize,
}

// ------------------------------------------------------------- the index

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mod,
    ResourcePack,
    ShaderPack,
    Config,
    Other,
}

fn classify(path: &str) -> Kind {
    let lower = path.to_ascii_lowercase();
    match lower.split('/').next().unwrap_or("") {
        "mods" if lower.ends_with(".jar") => Kind::Mod,
        "resourcepacks" => Kind::ResourcePack,
        "shaderpacks" => Kind::ShaderPack,
        "config" | "defaultconfigs" => Kind::Config,
        _ => Kind::Other,
    }
}

/// A mod listed in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexMod {
    file_name: String,
    sha1: Option<String>,
    side: Option<Side>,
}

/// What the index and the overrides folders hold.
#[derive(Debug, Default, PartialEq, Eq)]
struct Contents {
    mods: Vec<IndexMod>,
    configs: usize,
    resource_packs: usize,
    shader_packs: usize,
}

fn side_of(client: &str, server: &str) -> Option<Side> {
    match (client, server) {
        ("unsupported", "unsupported") | ("", "") => None,
        (_, "unsupported") => Some(Side::Client),
        ("unsupported", _) => Some(Side::Server),
        ("", _) | (_, "") => None,
        _ => Some(Side::Both),
    }
}

/// Count what a pack contains from its index and the zip's entry names.
fn summarize(index: &PackIndex, entry_names: &[String]) -> Contents {
    let mut out = Contents::default();
    let count = |kind: Kind, out: &mut Contents| match kind {
        Kind::ResourcePack => out.resource_packs += 1,
        Kind::ShaderPack => out.shader_packs += 1,
        Kind::Config => out.configs += 1,
        Kind::Mod | Kind::Other => {}
    };
    for file in &index.files {
        let env = |side: &str| {
            file.env
                .as_ref()
                .and_then(|e| e.get(side))
                .map_or("", String::as_str)
        };
        // Files the client doesn't install aren't part of what you'd get.
        if env("client") == "unsupported" {
            continue;
        }
        let kind = classify(&file.path);
        if kind == Kind::Mod {
            out.mods.push(IndexMod {
                file_name: file
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file.path)
                    .to_owned(),
                sha1: file.hashes.get("sha1").cloned(),
                side: side_of(env("client"), env("server")),
            });
        }
        count(kind, &mut out);
    }
    for name in entry_names {
        let Some(inner) = name
            .strip_prefix("overrides/")
            .or_else(|| name.strip_prefix("client-overrides/"))
        else {
            continue;
        };
        if inner.is_empty() || inner.ends_with('/') {
            continue;
        }
        let kind = classify(inner);
        if kind == Kind::Mod {
            out.mods.push(IndexMod {
                file_name: inner.rsplit('/').next().unwrap_or(inner).to_owned(),
                sha1: None,
                side: None,
            });
        }
        count(kind, &mut out);
    }
    out
}

// ------------------------------------------------------- dependencies

/// Projects (and version numbers) the version's `dependencies` name, resolved with the
/// versions fetched for their `version_id`s. Incompatible ones are left out.
fn resolve_dependencies(
    deps: &[Dependency],
    versions: &HashMap<String, Version>,
) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dep in deps {
        if dep.dependency_type == "incompatible" {
            continue;
        }
        let version = dep.version_id.as_ref().and_then(|id| versions.get(id));
        let Some(project) = dep
            .project_id
            .clone()
            .or_else(|| version.map(|v| v.project_id.clone()))
        else {
            continue;
        };
        if seen.insert(project.clone()) {
            let number = version
                .map(|v| v.version_number.clone())
                .unwrap_or_default();
            out.push((project, number));
        }
    }
    out
}

// ----------------------------------------------------------- assembly

fn project_side(p: &Project) -> Side {
    side_of(&p.client_side, &p.server_side).unwrap_or(Side::Both)
}

/// Project ids a version lists as required.
fn required_projects(version: &Version) -> impl Iterator<Item = &str> {
    version
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
        .filter_map(|d| d.project_id.as_deref())
}

/// Titles of the required dependencies of `version` that Modrinth told us about.
fn requires_of(version: Option<&Version>, projects: &HashMap<String, Project>) -> Vec<String> {
    let mut names: Vec<String> = version
        .into_iter()
        .flat_map(required_projects)
        .filter_map(|id| projects.get(id))
        .map(|p| p.title.clone())
        .filter(|t| !t.is_empty())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

fn file_title(file_name: &str) -> String {
    file_name
        .strip_suffix(".jar")
        .unwrap_or(file_name)
        .to_owned()
}

/// Merge index files (matched by SHA-1 to `found`), dependencies and project
/// info into the sorted mod list. A project is listed once.
fn assemble(
    index_mods: &[IndexMod],
    found: &HashMap<String, Version>,
    deps: &[(String, String)],
    projects: &HashMap<String, Project>,
) -> Vec<PreviewMod> {
    let mut out = Vec::new();
    let mut listed = HashSet::new();
    for m in index_mods {
        let version = m.sha1.as_ref().and_then(|h| found.get(h));
        let project = version.and_then(|v| projects.get(&v.project_id));
        if let Some(v) = version
            && !listed.insert(v.project_id.clone())
        {
            continue;
        }
        out.push(PreviewMod {
            name: project
                .map(|p| p.title.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| file_title(&m.file_name)),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            version: version
                .map(|v| v.version_number.clone())
                .unwrap_or_default(),
            side: m
                .side
                .or_else(|| project.map(project_side))
                .unwrap_or(Side::Both),
            project_id: version.map(|v| v.project_id.clone()),
            description: project.map(|p| p.description.clone()).unwrap_or_default(),
            requires: requires_of(version, projects),
        });
    }
    for (project_id, number) in deps {
        if !listed.insert(project_id.clone()) {
            continue;
        }
        let project = projects.get(project_id);
        out.push(PreviewMod {
            name: project
                .map(|p| p.title.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| project_id.clone()),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            version: number.clone(),
            side: project.map_or(Side::Both, project_side),
            project_id: Some(project_id.clone()),
            description: project.map(|p| p.description.clone()).unwrap_or_default(),
            requires: Vec::new(),
        });
    }
    out.sort_by_key(|m| m.name.to_lowercase());
    out
}

// ------------------------------------------------- reading a zip remotely

/// Somewhere bytes of a file can be read from.
trait RangeSource {
    fn len(&self) -> u64;
    /// Bytes `start..end` (`end` ≤ `len()`).
    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>>;
}

/// `Read + Seek` over a [`RangeSource`], fetching [`BLOCK`]-sized pieces on demand, so
/// the zip reader only pulls what it touches: the end of the file (central directory)
/// and the entries it opens.
struct RangeReader<S: RangeSource> {
    source: S,
    pos: u64,
    blocks: HashMap<u64, Vec<u8>>,
}

impl<S: RangeSource> RangeReader<S> {
    fn new(source: S) -> Self {
        Self {
            source,
            pos: 0,
            blocks: HashMap::new(),
        }
    }
}

impl<S: RangeSource> Read for RangeReader<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.source.len();
        if buf.is_empty() || self.pos >= len {
            return Ok(0);
        }
        let number = self.pos / BLOCK;
        if !self.blocks.contains_key(&number) {
            let start = number * BLOCK;
            let data = self.source.read_range(start, (start + BLOCK).min(len))?;
            // Keep memory bounded: the zip reader moves forward through entries.
            if self.blocks.len() >= 16 {
                self.blocks.clear();
            }
            self.blocks.insert(number, data);
        }
        let block = &self.blocks[&number];
        let offset = (self.pos - number * BLOCK) as usize;
        let n = buf.len().min(block.len().saturating_sub(offset));
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short range response",
            ));
        }
        buf[..n].copy_from_slice(&block[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<S: RangeSource> Seek for RangeReader<S> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let len = self.source.len() as i128;
        let next = match to {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::End(n) => len + i128::from(n),
            SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
        };
        if next < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start",
            ));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

/// A file on a server that answers `Range` requests.
struct HttpSource {
    url: String,
    len: u64,
}

impl HttpSource {
    /// `None` when the server doesn't do ranges (it answers 200 with the whole file).
    fn open(url: &str) -> Result<Option<Self>> {
        let resp = Self::get(url, 0, 1)?;
        if resp.status().as_u16() != 206 {
            return Ok(None);
        }
        let total = resp
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|n| n.parse::<u64>().ok());
        Ok(total.map(|len| Self {
            url: url.to_owned(),
            len,
        }))
    }

    fn get(url: &str, start: u64, end: u64) -> Result<ureq::http::Response<ureq::Body>> {
        let resp = agent()
            .get(url)
            .header("User-Agent", modrinth::user_agent())
            .header("Range", format!("bytes={start}-{}", end - 1))
            .config()
            .http_status_as_error(false)
            .build()
            .call()?;
        let code = resp.status().as_u16();
        if code != 206 && code != 200 {
            return Err(Error::Other(format!(
                "could not read the modpack (HTTP {code})"
            )));
        }
        Ok(resp)
    }
}

impl RangeSource for HttpSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        let mut resp = Self::get(&self.url, start, end).map_err(io::Error::other)?;
        if resp.status().as_u16() != 206 {
            return Err(io::Error::other("the server stopped honouring ranges"));
        }
        let want = end - start;
        let bytes = resp
            .body_mut()
            .with_config()
            .limit(want + 1)
            .read_to_vec()
            .map_err(io::Error::other)?;
        if bytes.len() as u64 != want {
            return Err(io::Error::other("short range response"));
        }
        Ok(bytes)
    }
}

/// The index and the entry names of a `.mrpack`, read through any seekable source.
fn read_pack<R: Read + Seek>(reader: R) -> Result<(PackIndex, Vec<String>)> {
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| Error::Other(format!("not a modpack file: {e}")))?;
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    let entry = zip.by_name(INDEX).map_err(|_| {
        if names.iter().any(|n| n == "manifest.json") {
            Error::Other("this is a CurseForge modpack, which Arctic can't preview".into())
        } else {
            Error::Other("not a Modrinth modpack (no modrinth.index.json)".into())
        }
    })?;
    if entry.size() > MAX_INDEX_BYTES {
        return Err(Error::Other("the modpack index is too large".into()));
    }
    let mut text = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_INDEX_BYTES)
        .read_to_end(&mut text)
        .map_err(|e| Error::Other(format!("could not read the modpack index: {e}")))?;
    Ok((serde_json::from_slice(&text)?, names))
}

// ------------------------------------------------------------ the job

/// What is inside one version of a Modrinth modpack.
pub fn preview(dirs: &DataDirs, version_id: &str) -> Result<PackPreview> {
    let version = modrinth::version(version_id)?;
    let file = version
        .primary_file()
        .ok_or_else(|| Error::Other("this modpack version has no file".into()))?;
    check_url(&file.url)?;
    let (index, names) = match HttpSource::open(&file.url)? {
        Some(source) => read_pack(RangeReader::new(source))?,
        None => {
            // The server can't send parts of the file: download the whole `.mrpack`
            // into the cache (the installer reuses it) and read it from disk.
            let path = dirs.cache().join("modpacks").join(sanitize(&file.filename));
            download_all(
                "Downloading modpack",
                vec![DownloadJob {
                    url: file.url.clone(),
                    dest: path.clone(),
                    sha1: file.hashes.sha1.clone(),
                    size: file.size,
                    lzma: None,
                }],
                &|_| {},
            )?;
            let f = std::fs::File::open(&path).map_err(|e| Error::io(&path, e))?;
            read_pack(f)?
        }
    };
    let contents = summarize(&index, &names);

    // Files on Modrinth, by SHA-1.
    let mut hashes: Vec<String> = contents
        .mods
        .iter()
        .filter_map(|m| m.sha1.clone())
        .collect();
    hashes.sort();
    hashes.dedup();
    let mut found = HashMap::new();
    for chunk in hashes.chunks(HASH_BATCH) {
        found.extend(identify::lookup_retrying(chunk)?);
    }

    // Dependencies the author listed on the version.
    let dep_version_ids: Vec<&str> = version
        .dependencies
        .iter()
        .filter_map(|d| d.version_id.as_deref())
        .collect();
    let mut dep_versions = HashMap::new();
    for chunk in dep_version_ids.chunks(ID_BATCH) {
        for v in modrinth::versions(chunk)? {
            dep_versions.insert(v.id.clone(), v);
        }
    }
    let deps = resolve_dependencies(&version.dependencies, &dep_versions);

    // Names and icons.
    let mut project_ids: Vec<&str> = found
        .values()
        .map(|v| v.project_id.as_str())
        .chain(deps.iter().map(|(p, _)| p.as_str()))
        .chain(found.values().flat_map(required_projects))
        .collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let mut projects = HashMap::new();
    for chunk in project_ids.chunks(ID_BATCH) {
        for p in modrinth::projects(chunk)? {
            projects.insert(p.id.clone(), p);
        }
    }
    Ok(PackPreview {
        mods: assemble(&contents.mods, &found, &deps, &projects),
        configs: contents.configs,
        resource_packs: contents.resource_packs,
        shader_packs: contents.shader_packs,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn index_json() -> String {
        serde_json::json!({
            "name": "Test Pack",
            "versionId": "1.0",
            "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.0" },
            "files": [
                { "path": "mods/sodium.jar", "hashes": { "sha1": "aaa" }, "downloads": [],
                  "env": { "client": "required", "server": "unsupported" } },
                { "path": "mods/lithium.jar", "hashes": { "sha1": "bbb" }, "downloads": [],
                  "env": { "client": "required", "server": "required" } },
                { "path": "mods/servertool.jar", "hashes": { "sha1": "ccc" }, "downloads": [],
                  "env": { "client": "unsupported", "server": "required" } },
                { "path": "resourcepacks/faithful.zip", "hashes": {}, "downloads": [] },
                { "path": "shaderpacks/bsl.zip", "hashes": {}, "downloads": [] },
                { "path": "shaderpacks/complementary.zip", "hashes": {}, "downloads": [] },
                { "path": "config/sodium-options.json", "hashes": {}, "downloads": [] }
            ]
        })
        .to_string()
    }

    fn build_zip(padding: usize) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file(INDEX, deflated).unwrap();
        zip.write_all(index_json().as_bytes()).unwrap();
        zip.start_file("overrides/config/a.toml", deflated).unwrap();
        zip.write_all(b"a=1").unwrap();
        zip.start_file("overrides/config/sub/b.toml", deflated)
            .unwrap();
        zip.write_all(b"b=1").unwrap();
        zip.start_file("client-overrides/resourcepacks/extra.zip", deflated)
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("overrides/mods/local.jar", deflated)
            .unwrap();
        zip.write_all(b"jar").unwrap();
        zip.start_file("server-overrides/config/server.toml", deflated)
            .unwrap();
        zip.write_all(b"s").unwrap();
        zip.add_directory("overrides/config/sub2/", deflated)
            .unwrap();
        // A big blob after the index that a ranged read must skip.
        zip.start_file("overrides/saves/big.bin", stored).unwrap();
        zip.write_all(&vec![7u8; padding]).unwrap();
        zip.finish().unwrap().into_inner()
    }

    struct VecSource {
        data: Vec<u8>,
        fetched: AtomicU64,
    }

    impl RangeSource for VecSource {
        fn len(&self) -> u64 {
            self.data.len() as u64
        }
        fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
            self.fetched.fetch_add(end - start, Ordering::SeqCst);
            Ok(self.data[start as usize..end as usize].to_vec())
        }
    }

    #[test]
    fn range_reader_reads_index_without_the_whole_file() {
        let data = build_zip(4 * 1024 * 1024);
        let total = data.len() as u64;
        let source = VecSource {
            data,
            fetched: AtomicU64::new(0),
        };
        let mut reader = RangeReader::new(source);
        let (index, names) = read_pack(&mut reader).unwrap();
        assert_eq!(index.name, "Test Pack");
        assert_eq!(index.files.len(), 7);
        assert!(names.iter().any(|n| n == "overrides/config/a.toml"));
        let fetched = reader.source.fetched.load(Ordering::SeqCst);
        assert!(fetched < total / 4, "fetched {fetched} of {total}");
    }

    #[test]
    fn range_reader_seeks_like_a_file() {
        let source = VecSource {
            data: (0..=255u8).cycle().take(1_000_000).collect(),
            fetched: AtomicU64::new(0),
        };
        let mut r = RangeReader::new(source);
        r.seek(SeekFrom::End(-10)).unwrap();
        let mut buf = [0u8; 20];
        let n = r.read(&mut buf).unwrap();
        assert_eq!(n, 10);
        let want: Vec<u8> = (999_990u32..1_000_000).map(|i| (i % 256) as u8).collect();
        assert_eq!(buf[..10], want[..]);
        assert_eq!(r.read(&mut buf).unwrap(), 0);
        r.seek(SeekFrom::Start(BLOCK - 2)).unwrap(); // across a block edge
        let mut two = [0u8; 4];
        io::Read::read_exact(&mut r, &mut two).unwrap();
        assert_eq!(two[0], ((BLOCK - 2) % 256) as u8);
        assert_eq!(two[3], ((BLOCK + 1) % 256) as u8);
        assert!(r.seek(SeekFrom::Current(-2_000_000)).is_err());
    }

    #[test]
    fn rejects_files_that_are_not_modrinth_packs() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        let data = zip.finish().unwrap().into_inner();
        let err = read_pack(Cursor::new(data)).unwrap_err().to_string();
        assert!(err.contains("CurseForge"), "{err}");
        assert!(read_pack(Cursor::new(b"nope".to_vec())).is_err());
    }

    #[test]
    fn counts_mods_configs_and_packs() {
        let (index, names) = read_pack(Cursor::new(build_zip(10))).unwrap();
        let c = summarize(&index, &names);
        // sodium + lithium from the index; servertool is server-only (not installed);
        // local.jar comes from overrides.
        let files: Vec<&str> = c.mods.iter().map(|m| m.file_name.as_str()).collect();
        assert_eq!(files, ["sodium.jar", "lithium.jar", "local.jar"]);
        // index: 1 config + overrides: a.toml, sub/b.toml (server-overrides ignored).
        assert_eq!(c.configs, 3);
        // index: faithful + client-overrides: extra.
        assert_eq!(c.resource_packs, 2);
        assert_eq!(c.shader_packs, 2);
        assert_eq!(c.mods[0].side, Some(Side::Client));
        assert_eq!(c.mods[1].side, Some(Side::Both));
        assert_eq!(c.mods[2].side, None);
        assert_eq!(c.mods[0].sha1.as_deref(), Some("aaa"));
    }

    #[test]
    fn classifies_paths() {
        assert_eq!(classify("mods/a.jar"), Kind::Mod);
        assert_eq!(classify("mods/readme.txt"), Kind::Other);
        assert_eq!(classify("Config/x.json"), Kind::Config);
        assert_eq!(classify("defaultconfigs/x.toml"), Kind::Config);
        assert_eq!(classify("resourcepacks/p.zip"), Kind::ResourcePack);
        assert_eq!(classify("shaderpacks/s.zip"), Kind::ShaderPack);
        assert_eq!(classify("options.txt"), Kind::Other);
    }

    fn version(id: &str, project: &str, number: &str) -> Version {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": project, "version_number": number
        }))
        .unwrap()
    }

    fn project(id: &str, title: &str, icon: Option<&str>) -> Project {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "icon_url": icon,
            "client_side": "required", "server_side": "optional"
        }))
        .unwrap()
    }

    fn dep(project: Option<&str>, version: Option<&str>, kind: &str) -> Dependency {
        serde_json::from_value(serde_json::json!({
            "project_id": project, "version_id": version, "dependency_type": kind
        }))
        .unwrap()
    }

    #[test]
    fn dependencies_resolve_to_projects_and_versions() {
        let versions: HashMap<String, Version> = [
            ("v1".to_owned(), version("v1", "P1", "1.2.3")),
            ("v2".to_owned(), version("v2", "P2", "4.5")),
        ]
        .into();
        let deps = [
            dep(None, Some("v1"), "required"),       // project from the version
            dep(Some("P2"), Some("v2"), "embedded"), // both given
            dep(Some("P3"), None, "optional"),       // project only: no number
            dep(Some("P1"), Some("v1"), "required"), // duplicate project
            dep(Some("P4"), None, "incompatible"),   // left out
            dep(None, Some("gone"), "required"),     // version not found: nothing to show
            dep(None, None, "required"),
        ];
        let got = resolve_dependencies(&deps, &versions);
        assert_eq!(
            got,
            [
                ("P1".to_owned(), "1.2.3".to_owned()),
                ("P2".to_owned(), "4.5".to_owned()),
                ("P3".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn assembles_names_icons_versions_and_sides() {
        let index_mods = vec![
            IndexMod {
                file_name: "sodium.jar".into(),
                sha1: Some("aaa".into()),
                side: Some(Side::Client),
            },
            IndexMod {
                file_name: "mystery-1.0.jar".into(),
                sha1: Some("zzz".into()),
                side: None,
            },
            IndexMod {
                file_name: "sodium-copy.jar".into(),
                sha1: Some("aaa".into()),
                side: None,
            },
        ];
        let found: HashMap<String, Version> =
            [("aaa".to_owned(), version("v1", "P1", "0.6.0"))].into();
        let projects: HashMap<String, Project> = [
            (
                "P1".to_owned(),
                project("P1", "Sodium", Some("https://i/s.png")),
            ),
            ("P9".to_owned(), project("P9", "Fabric API", None)),
        ]
        .into();
        let deps = vec![
            ("P1".to_owned(), "0.6.0".to_owned()), // already listed via the index
            ("P9".to_owned(), "0.100".to_owned()),
            ("PX".to_owned(), String::new()), // project info missing
        ];
        let mods = assemble(&index_mods, &found, &deps, &projects);
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Fabric API", "mystery-1.0", "PX", "Sodium"]);
        let sodium = mods.iter().find(|m| m.name == "Sodium").unwrap();
        assert_eq!(sodium.version, "0.6.0");
        assert_eq!(sodium.icon_url.as_deref(), Some("https://i/s.png"));
        assert_eq!(sodium.side, Side::Client);
        assert_eq!(sodium.project_id.as_deref(), Some("P1"));
        let api = mods.iter().find(|m| m.name == "Fabric API").unwrap();
        assert_eq!(api.side, Side::Both); // required / optional
        assert_eq!(api.version, "0.100");
        let mystery = mods.iter().find(|m| m.name == "mystery-1.0").unwrap();
        assert_eq!(mystery.project_id, None);
        assert_eq!(mystery.version, "");
    }

    #[test]
    fn mods_carry_their_summary_and_required_dependencies() {
        let index_mods = vec![IndexMod {
            file_name: "sodium.jar".into(),
            sha1: Some("aaa".into()),
            side: None,
        }];
        let sodium: Version = serde_json::from_value(serde_json::json!({
            "id": "v1", "project_id": "P1", "version_number": "0.6.0",
            "dependencies": [
                {"project_id": "P9", "dependency_type": "required"},
                {"project_id": "P8", "dependency_type": "optional"},
                {"project_id": "P7", "dependency_type": "required"},
                {"project_id": "P6", "dependency_type": "required"}
            ]
        }))
        .unwrap();
        let found: HashMap<String, Version> = [("aaa".to_owned(), sodium)].into();
        let with_summary: Project = serde_json::from_value(serde_json::json!({
            "id": "P1", "title": "Sodium", "description": "Fast renderer"
        }))
        .unwrap();
        let projects: HashMap<String, Project> = [
            ("P1".to_owned(), with_summary),
            ("P9".to_owned(), project("P9", "Fabric API", None)),
            ("P8".to_owned(), project("P8", "Optional Thing", None)),
            ("P7".to_owned(), project("P7", "Another", None)),
        ]
        .into();
        let mods = assemble(&index_mods, &found, &[], &projects);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].description, "Fast renderer");
        // Required only, by title; P6 is unknown to Modrinth's answer and left out.
        assert_eq!(mods[0].requires, ["Another", "Fabric API"]);
    }
}
