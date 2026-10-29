//! Name lookup that does not give up when Windows cannot find a host.
//!
//! The system resolver is asked first. When it fails (a broken DNS service, a VPN or security
//! suite in the way, a provider that poisons names), the name is looked up over HTTPS the way
//! browsers do, at Cloudflare, Google and Quad9 at the same time (whichever is not blocked
//! answers first). When all of those fail too, the addresses the host had the last time it was
//! found are tried. The connection is still made to the real host with its own certificate
//! check, so nothing about the security of the download changes.
//!
//! The fallback never runs when a proxy is set (it would reach Cloudflare around the proxy), and
//! `ARCTIC_NO_DOH=1` turns it off for people who want their DNS policy (a filtering resolver, say)
//! to be final.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError, mpsc};
use std::time::{Duration, Instant};

use serde::Deserialize;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// DNS-over-HTTPS endpoints by address (so reaching them needs no lookup itself), all answering
/// the JSON form of the protocol. Asked in parallel.
const DOH_URLS: [&str; 3] = [
    "https://1.1.1.1/dns-query",
    "https://8.8.8.8/resolve",
    "https://9.9.9.9:5053/dns-query",
];
const DOH_TIMEOUT: Duration = Duration::from_secs(5);
/// How long an answer from the fallback is reused.
const CACHE_FOR: Duration = Duration::from_secs(300);
/// Set (to anything) to keep the system resolver's answer final.
const OPT_OUT_ENV: &str = "ARCTIC_NO_DOH";
/// DNS record type A (an IPv4 address).
const TYPE_A: u32 = 1;
/// Hosts kept in the remembered-addresses file.
const MAX_REMEMBERED: usize = 64;

/// Where the last known addresses are kept (set once at start-up); without it they are not kept.
static MEMORY_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Keep the addresses hosts had when they were last found in `file`, for when every lookup fails.
pub(crate) fn set_memory_file(file: PathBuf) {
    let _ = MEMORY_FILE.set(file);
}

#[derive(Debug, Default)]
pub(crate) struct FallbackResolver {
    cache: Mutex<HashMap<String, (Instant, Vec<IpAddr>)>>,
    /// Last known addresses per host; `None` until the file has been read.
    remembered: Mutex<Option<HashMap<String, Vec<IpAddr>>>>,
}

#[derive(Deserialize)]
struct DohAnswer {
    #[serde(rename = "type")]
    kind: u32,
    data: String,
}

#[derive(Deserialize)]
struct DohReply {
    #[serde(rename = "Answer", default)]
    answer: Vec<DohAnswer>,
}

impl DohReply {
    fn ipv4(&self) -> Vec<IpAddr> {
        self.answer
            .iter()
            .filter(|a| a.kind == TYPE_A)
            .filter_map(|a| a.data.parse().ok())
            .collect()
    }
}

impl FallbackResolver {
    fn cached(&self, host: &str) -> Option<Vec<IpAddr>> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let (at, ips) = cache.get(host)?;
        (at.elapsed() < CACHE_FOR).then(|| ips.clone())
    }

    fn lookup(&self, host: &str) -> Option<Vec<IpAddr>> {
        if let Some(ips) = self.cached(host) {
            return Some(ips);
        }
        let ips = doh_lookup(host)?;
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(host.to_owned(), (Instant::now(), ips.clone()));
        Some(ips)
    }

    /// Note the addresses the system found for `host`; the file is rewritten only when they changed.
    fn remember(&self, host: &str, found: &[IpAddr]) {
        let Some(file) = MEMORY_FILE.get() else {
            return;
        };
        let mut guard = self
            .remembered
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let map = guard.get_or_insert_with(|| read_memory(file));
        if map.get(host).is_some_and(|known| known == found) {
            return;
        }
        if map.len() >= MAX_REMEMBERED && !map.contains_key(host) {
            return;
        }
        map.insert(host.to_owned(), found.to_vec());
        if let Ok(json) = serde_json::to_vec(&*map) {
            let _ = std::fs::write(file, json);
        }
    }

    /// The addresses `host` had when it was last found, from an earlier run or this one.
    fn last_known(&self, host: &str) -> Option<Vec<IpAddr>> {
        let file = MEMORY_FILE.get()?;
        let mut guard = self
            .remembered
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let map = guard.get_or_insert_with(|| read_memory(file));
        map.get(host).filter(|ips| !ips.is_empty()).cloned()
    }
}

fn read_memory(file: &Path) -> HashMap<String, Vec<IpAddr>> {
    std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The IPv4 addresses of `host` according to the first DoH endpoint that answers, or `None` when
/// none does.
fn doh_lookup(host: &str) -> Option<Vec<IpAddr>> {
    let (tx, rx) = mpsc::channel();
    for url in DOH_URLS {
        let tx = tx.clone();
        let host = host.to_owned();
        std::thread::spawn(move || {
            let _ = tx.send(doh_query(url, &host));
        });
    }
    drop(tx);
    // Each answer (or failure) arrives as it happens; the first usable one wins.
    rx.iter().flatten().find(|ips| !ips.is_empty())
}

fn doh_query(url: &str, host: &str) -> Option<Vec<IpAddr>> {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(DOH_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .query("name", host)
        .query("type", "A")
        .header("accept", "application/dns-json")
        .call()
        .ok()?;
    let reply: DohReply = response.body_mut().read_json().ok()?;
    Some(reply.ipv4())
}

impl Resolver for FallbackResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let system_error = match DefaultResolver::default().resolve(uri, config, timeout) {
            Ok(addrs) => {
                if let Some(authority) = uri.authority() {
                    let found: Vec<IpAddr> = addrs.iter().map(SocketAddr::ip).collect();
                    self.remember(authority.host(), &found);
                }
                return Ok(addrs);
            }
            Err(e) => e,
        };
        // A proxy was asked for: a direct request to a resolver would go around it. Or the user opted out.
        if config.proxy().is_some() || std::env::var_os(OPT_OUT_ENV).is_some() {
            return Err(system_error);
        }
        let (Some(scheme), Some(authority)) = (uri.scheme(), uri.authority()) else {
            return Err(system_error);
        };
        let Some(port) = DefaultResolver::host_and_port(scheme, authority)
            .and_then(|hp| hp.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()))
        else {
            return Err(system_error);
        };
        let host = authority.host();
        let (ips, how) = match self.lookup(host) {
            Some(ips) => (ips, "DNS over HTTPS"),
            None => match self.last_known(host) {
                Some(ips) => (ips, "the address it had last time"),
                None => {
                    log::warn!(
                        "{host}: the system could not find it ({system_error}) and DNS over HTTPS \
                         (Cloudflare, Google, Quad9) did not answer either"
                    );
                    return Err(system_error);
                }
            },
        };
        log::info!("{host}: the system could not find it ({system_error}); using {how}");
        let mut out = self.empty();
        for ip in ips {
            out.push(SocketAddr::new(ip, port));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_keeps_only_ipv4_answers() {
        let json = r#"{"Status":0,"Answer":[
            {"name":"x","type":5,"TTL":60,"data":"alias.example."},
            {"name":"alias.example","type":1,"TTL":60,"data":"203.0.113.7"}]}"#;
        let reply: DohReply = serde_json::from_str(json).unwrap();
        let ips = reply.ipv4();
        assert_eq!(ips, ["203.0.113.7".parse::<IpAddr>().unwrap()]);
    }

    #[test]
    fn an_answerless_reply_parses_as_empty() {
        let reply: DohReply = serde_json::from_str(r#"{"Status":3}"#).unwrap();
        assert!(reply.answer.is_empty());
    }
}
