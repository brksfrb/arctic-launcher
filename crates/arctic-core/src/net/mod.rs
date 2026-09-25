//! HTTP helpers and the parallel, checksum-verified downloader.

mod download;

pub(crate) use download::body_timeout;
pub use download::{Compressed, DownloadJob, download_all, sha1_file};

use std::sync::OnceLock;
use std::time::Duration;

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

/// Shared agent so connections are pooled across the whole app.
///
/// Uses the OS certificate store (not bundled roots) so machines behind
/// corporate proxies or antivirus HTTPS scanning still work.
pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        // ureq keeps only 3 idle connections per host by default, which made
        // most parallel downloads redo a TLS handshake. Keep one per worker.
        ureq::Agent::config_builder()
            .tls_config(tls)
            .max_idle_connections(download::WORKERS * 2)
            .max_idle_connections_per_host(download::WORKERS)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_recv_body(Some(BODY_TIMEOUT))
            .user_agent(format!("{}/{}", crate::LAUNCHER_BRAND, crate::APP_VERSION))
            .build()
            .into()
    })
}

/// GET a URL and deserialize its JSON body.
pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let mut resp = agent().get(url).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_json()?)
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
