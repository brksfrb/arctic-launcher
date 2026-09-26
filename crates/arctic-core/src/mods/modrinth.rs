//! Modrinth API v2: wire types, URL building and HTTP calls.
//!
//! Only the fields the launcher needs are modelled; everything else in the
//! responses is ignored. Every field that Modrinth may omit or null has a
//! serde default so a slightly different payload never fails a whole page.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::{ProjectHit, SearchPage, SearchQuery, SortBy};
use crate::loaders::LoaderKind;
use crate::net::{agent, read_json_any_status};
use crate::{Error, Result};

const API: &str = "https://api.modrinth.com/v2";
const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

/// Modrinth asks API clients for a uniquely identifying User-Agent with
/// contact information: <https://docs.modrinth.com/api/#user-agents>.
pub(super) fn user_agent() -> String {
    format!(
        "brksfrb/{}/{} (arcticlauncher.com)",
        crate::LAUNCHER_BRAND,
        crate::APP_VERSION
    )
}

// ---------------------------------------------------------------- wire types

#[derive(Debug, Deserialize)]
pub(super) struct SearchResponse {
    #[serde(default)]
    pub hits: Vec<SearchHit>,
    #[serde(default)]
    pub total_hits: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchHit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

impl SearchResponse {
    pub fn into_page(self) -> SearchPage {
        SearchPage {
            hits: self.hits.into_iter().map(SearchHit::into_hit).collect(),
            total: self.total_hits,
        }
    }
}

impl SearchHit {
    fn into_hit(self) -> ProjectHit {
        ProjectHit {
            project_id: self.project_id,
            slug: self.slug,
            title: self.title,
            description: self.description,
            author: self.author,
            downloads: self.downloads,
            icon_url: non_empty(self.icon_url),
            categories: self.categories,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Version {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub hashes: FileHashes,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct FileHashes {
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Project {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

impl Version {
    /// The file to install: the one flagged primary, else the first.
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }

    /// Runs on `loaders` (any of them) and exactly `game_version`.
    pub fn is_compatible(&self, loaders: &[&str], game_version: &str) -> bool {
        self.game_versions.iter().any(|g| g == game_version)
            && self.loaders.iter().any(|l| loaders.contains(&l.as_str()))
    }
}

pub(super) fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.trim().is_empty())
}

// ---------------------------------------------------------- version choice

/// Lower is better: releases first, then betas, then alphas.
fn channel_rank(version_type: &str) -> u8 {
    match version_type {
        "release" => 0,
        "beta" => 1,
        "alpha" => 2,
        _ => 3,
    }
}

/// Newest compatible version, preferring the most stable channel available.
/// Ties on the publish date keep the API order (newest first).
pub(super) fn pick_version<'a>(
    versions: &'a [Version],
    loaders: &[&str],
    game_version: &str,
) -> Option<&'a Version> {
    versions
        .iter()
        .filter(|v| v.is_compatible(loaders, game_version) && v.primary_file().is_some())
        .min_by(|a, b| {
            channel_rank(&a.version_type)
                .cmp(&channel_rank(&b.version_type))
                .then_with(|| b.date_published.cmp(&a.date_published))
        })
}

/// Loaders whose mods run on `loader`. Quilt loads Fabric mods, which is
/// also what Modrinth's own app assumes.
pub(super) fn compatible_loaders(loader: LoaderKind) -> Vec<&'static str> {
    match loader {
        LoaderKind::Quilt => vec!["quilt", "fabric"],
        other => vec![other.modrinth_id()],
    }
}

// ------------------------------------------------------------------- URLs

/// Percent-encode everything except RFC 3986 unreserved characters.
pub(super) fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A JSON array of strings, as Modrinth's query parameters expect.
fn json_list(items: &[&str]) -> String {
    serde_json::Value::from(items.to_vec()).to_string()
}

fn sort_index(sort: SortBy) -> &'static str {
    match sort {
        SortBy::Relevance => "relevance",
        SortBy::Downloads => "downloads",
        SortBy::Updated => "updated",
        SortBy::Newest => "newest",
    }
}

fn search_facets(query: &SearchQuery) -> String {
    let kind = if query.modpacks { "modpack" } else { "mod" };
    let mut facets = vec![vec![format!("project_type:{kind}")]];
    if !query.game_version.trim().is_empty() {
        facets.push(vec![format!("versions:{}", query.game_version.trim())]);
    }
    if let Some(loader) = query.loader {
        let any_of = compatible_loaders(loader)
            .into_iter()
            .map(|l| format!("categories:{l}"))
            .collect();
        facets.push(any_of);
    }
    serde_json::Value::from(facets).to_string()
}

pub(super) fn search_url(query: &SearchQuery) -> String {
    let limit = match query.limit {
        0 => DEFAULT_LIMIT,
        n => n.min(MAX_LIMIT),
    };
    format!(
        "{API}/search?query={}&facets={}&index={}&offset={}&limit={limit}",
        encode(query.text.trim()),
        encode(&search_facets(query)),
        sort_index(query.sort),
        query.offset,
    )
}

pub(super) fn project_versions_url(project_id: &str, loaders: &[&str], game: &str) -> String {
    format!(
        "{API}/project/{}/version?loaders={}&game_versions={}",
        encode(project_id),
        encode(&json_list(loaders)),
        encode(&json_list(&[game])),
    )
}

pub(super) fn version_url(version_id: &str) -> String {
    format!("{API}/version/{}", encode(version_id))
}

pub(super) fn projects_url(ids: &[&str]) -> String {
    format!("{API}/projects?ids={}", encode(&json_list(ids)))
}

// ------------------------------------------------------------------- HTTP

/// A readable message for non-success statuses.
pub(super) fn status_error(code: u16, what: &str) -> Option<Error> {
    let msg = match code {
        200..=299 => return None,
        404 => format!("{what} was not found on Modrinth"),
        410 => format!("{what} is no longer available on Modrinth"),
        429 => "Modrinth rate limit reached; please wait a minute and try again".into(),
        500..=599 => format!("Modrinth is having trouble right now (HTTP {code})"),
        _ => format!("Modrinth request for {what} failed (HTTP {code})"),
    };
    Some(Error::Other(msg))
}

/// GET a Modrinth API URL and parse its JSON. `what` names the resource in
/// error messages ("project sodium").
pub(super) fn get_api<T: DeserializeOwned>(url: &str, what: &str) -> Result<T> {
    log::debug!("modrinth GET {url}");
    let mut resp = agent()
        .get(url)
        .header("User-Agent", user_agent())
        .header("Accept", "application/json")
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    if let Some(e) = status_error(resp.status().as_u16(), what) {
        return Err(e);
    }
    read_json_any_status(&mut resp)
}

pub(super) fn search(query: &SearchQuery) -> Result<SearchPage> {
    let resp: SearchResponse = get_api(&search_url(query), "search")?;
    Ok(resp.into_page())
}

pub(super) fn project_versions(
    project_id: &str,
    loaders: &[&str],
    game: &str,
) -> Result<Vec<Version>> {
    get_api(
        &project_versions_url(project_id, loaders, game),
        &format!("project {project_id}"),
    )
}

pub(super) fn version(version_id: &str) -> Result<Version> {
    get_api(&version_url(version_id), &format!("version {version_id}"))
}

pub(super) fn projects(ids: &[&str]) -> Result<Vec<Project>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    get_api(&projects_url(ids), "projects")
}
