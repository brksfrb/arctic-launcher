//! Name lookup that does not give up when Windows cannot find a host.
//!
//! The system resolver is asked first. When it fails (a broken DNS service, a VPN or security
//! suite in the way, a provider that poisons names), the name is looked up over HTTPS at
//! Cloudflare instead, the way browsers do. The connection is still made to the real host with
//! its own certificate check, so nothing about the security of the download changes.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// Cloudflare's DNS-over-HTTPS endpoint by address, so reaching it needs no lookup itself.
const DOH_URL: &str = "https://1.1.1.1/dns-query";
const DOH_TIMEOUT: Duration = Duration::from_secs(5);
/// How long an answer from the fallback is reused.
const CACHE_FOR: Duration = Duration::from_secs(300);
/// DNS record type A (an IPv4 address).
const TYPE_A: u32 = 1;

#[derive(Debug, Default)]
pub(crate) struct FallbackResolver {
    cache: Mutex<HashMap<String, (Instant, Vec<IpAddr>)>>,
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
}

/// The IPv4 addresses of `host` according to Cloudflare, or `None` when that fails too.
fn doh_lookup(host: &str) -> Option<Vec<IpAddr>> {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(DOH_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(DOH_URL)
        .query("name", host)
        .query("type", "A")
        .header("accept", "application/dns-json")
        .call()
        .ok()?;
    let reply: DohReply = response.body_mut().read_json().ok()?;
    let ips: Vec<IpAddr> = reply
        .answer
        .iter()
        .filter(|a| a.kind == TYPE_A)
        .filter_map(|a| a.data.parse().ok())
        .collect();
    (!ips.is_empty()).then_some(ips)
}

impl Resolver for FallbackResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let system_error = match DefaultResolver::default().resolve(uri, config, timeout) {
            Ok(addrs) => return Ok(addrs),
            Err(e) => e,
        };
        let (Some(scheme), Some(authority)) = (uri.scheme(), uri.authority()) else {
            return Err(system_error);
        };
        let Some(port) = DefaultResolver::host_and_port(scheme, authority)
            .and_then(|hp| hp.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()))
        else {
            return Err(system_error);
        };
        let host = authority.host();
        let Some(ips) = self.lookup(host) else {
            return Err(system_error);
        };
        log::info!("{host}: the system could not find it ({system_error}); using DNS over HTTPS");
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
        let ips: Vec<IpAddr> = reply
            .answer
            .iter()
            .filter(|a| a.kind == TYPE_A)
            .filter_map(|a| a.data.parse().ok())
            .collect();
        assert_eq!(ips, ["203.0.113.7".parse::<IpAddr>().unwrap()]);
    }

    #[test]
    fn an_answerless_reply_parses_as_empty() {
        let reply: DohReply = serde_json::from_str(r#"{"Status":3}"#).unwrap();
        assert!(reply.answer.is_empty());
    }
}
