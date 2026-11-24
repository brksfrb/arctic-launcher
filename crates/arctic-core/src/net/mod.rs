//! HTTP helpers and the parallel, checksum-verified downloader.

mod download;
mod download_transport;
mod resolver;

pub use download::{Compressed, DownloadJob, download_all, set_gentle, sha1_file};
pub(crate) use download::{
    DownloadAlternates, Policy, body_timeout, download_all_with_alternates,
    download_all_with_replacements, download_before, download_single, fetch_text, is_not_found,
    mirror_for, part_path,
};

use std::sync::{PoisonError, RwLock};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

use crate::Result;

/// Upper bound for JSON documents we parse (manifests are well below this).
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
/// Per-phase limits instead of one global timeout, so big downloads on slow
/// connections are not cut off while a dead connection is still noticed.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
/// Body limit for API/JSON calls (file downloads compute their own).
const BODY_TIMEOUT: Duration = Duration::from_secs(60);

static AGENT: RwLock<Option<ureq::Agent>> = RwLock::new(None);

/// Shared agent so connections are pooled across the whole app (cheap to
/// clone; clones share the pool). Goes through the proxy set with
/// [`set_proxy`].
pub fn agent() -> ureq::Agent {
    if let Some(agent) = AGENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        return agent.clone();
    }
    let mut slot = AGENT.write().unwrap_or_else(PoisonError::into_inner);
    slot.get_or_insert_with(|| build_agent(None)).clone()
}

/// Keep the addresses hosts had when they were last found in `file`, so a launcher whose DNS
/// stops working (and whose DNS-over-HTTPS lookups are blocked too) still reaches them.
pub fn set_dns_memory(file: std::path::PathBuf) {
    resolver::set_memory_file(file);
}

/// Route all launcher traffic through `proxy` (or directly with `None`).
/// Invalid proxy settings are refused rather than silently ignored, so
/// traffic never goes direct when a proxy was asked for.
pub fn set_proxy(proxy: Option<&crate::proxy::ProxySettings>) -> Result<()> {
    let built = proxy.map(to_ureq_proxy).transpose();
    let agent = match &built {
        Ok(p) => build_agent(p.clone()),
        // Unusable settings: send everything to a closed port so requests
        // fail visibly instead of going around the proxy.
        Err(_) => build_agent(ureq::Proxy::new(BLOCKED_PROXY).ok()),
    };
    *AGENT.write().unwrap_or_else(PoisonError::into_inner) = Some(agent);
    built.map(|_| ())
}

/// Where traffic goes when a proxy was asked for but can't be used.
const BLOCKED_PROXY: &str = "socks5h://127.0.0.1:9";

/// Check that a proxy works: reach Mojang's services through it. Any HTTP
/// answer counts (the proxy connected), only connection errors fail.
pub fn test_proxy(proxy: &crate::proxy::ProxySettings) -> Result<()> {
    proxy.validate().map_err(crate::Error::Other)?;
    let agent = build_agent(Some(to_ureq_proxy(proxy)?));
    match agent.get("https://api.minecraftservices.com/").call() {
        Ok(_) | Err(ureq::Error::StatusCode(_)) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn to_ureq_proxy(proxy: &crate::proxy::ProxySettings) -> Result<ureq::Proxy> {
    proxy.validate().map_err(crate::Error::Other)?;
    ureq::Proxy::new(&proxy.url()).map_err(|e| crate::Error::Other(format!("proxy: {e}")))
}

/// Uses the OS certificate store (not bundled roots) so machines behind
/// corporate proxies or antivirus HTTPS scanning still work.
fn build_agent(proxy: Option<ureq::Proxy>) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    // ureq keeps only 3 idle connections per host by default, which made
    // most parallel downloads redo a TLS handshake. Keep one per worker.
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        // Without an Arctic proxy, keep honoring ALL_PROXY / HTTPS_PROXY.
        .proxy(proxy.or_else(ureq::Proxy::try_from_env))
        .max_idle_connections(download::WORKERS * 2)
        .max_idle_connections_per_host(download::WORKERS)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .timeout_recv_body(Some(BODY_TIMEOUT))
        .user_agent(format!("{}/{}", crate::LAUNCHER_BRAND, crate::APP_VERSION))
        .build();
    use ureq::unversioned::transport::{Connector, DefaultConnector};
    // Names the system cannot find are looked up over HTTPS instead (see `resolver`).
    ureq::Agent::with_parts(
        config,
        DefaultConnector::default().chain(download_transport::DownloadIdleConnector),
        resolver::FallbackResolver::default(),
    )
}

/// GET a URL and deserialize its JSON body.
pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let mut resp = agent().get(url).call().map_err(download::request_error)?;
    resp.body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_json()
        .map_err(download::request_error)
}

/// Try the official equivalent when a Java runtime document cannot be read.
pub(crate) fn get_mojang_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let fetch = |url: &str| {
        download::with_download_idle(|| get_json(url))
            .map_err(|e| download::metadata_error(url, e))
            .inspect_err(|e| log::warn!("{e}"))
    };
    fetch(url).or_else(|primary| {
        let Some(alternate) = download::mojang_alternate(url) else {
            return Err(primary);
        };
        fetch(&alternate).map_err(|_| primary)
    })
}

pub(crate) fn time_remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| crate::Error::Other("background download timed out".into()))
}

pub(crate) fn get_json_before<T: DeserializeOwned>(url: &str, deadline: Instant) -> Result<T> {
    let mut resp = agent()
        .get(url)
        .config()
        .timeout_global(Some(time_remaining(deadline)?))
        .build()
        .call()?;
    read_json_any_status(&mut resp)
}

/// Read a JSON body regardless of HTTP status (used by OAuth/Xbox endpoints
/// that return structured errors with 4xx codes).
pub(crate) fn read_json_any_status<T: DeserializeOwned>(
    resp: &mut ureq::http::Response<ureq::Body>,
) -> Result<T> {
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_json()?)
}
