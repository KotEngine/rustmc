//! Address resolution: plain A/AAAA, optional SRV lookup (`srv` feature),
//! optional caching via an explicitly-attached `DnsCache` (`dns_cache`
//! feature). Caching is opt-in (`JavaServer::with_cache`/
//! `BedrockServer::with_cache`) — without it, every call re-resolves, same
//! as if the `dns_cache` feature weren't compiled in at all.

use std::net::{IpAddr, SocketAddr};

use crate::address::Address;
use crate::error::RustmcError;
use crate::response::DnsInfo;

#[cfg(feature = "dns_cache")]
use std::sync::Arc;

#[cfg(feature = "dns_cache")]
use crate::cache::DnsCache;

pub struct AddressResolver {
    #[cfg(feature = "dns_cache")]
    cache: Option<Arc<DnsCache>>,
}

impl AddressResolver {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "dns_cache")]
            cache: None,
        }
    }

    #[cfg(feature = "dns_cache")]
    pub fn with_cache(cache: Arc<DnsCache>) -> Self {
        Self { cache: Some(cache) }
    }

    #[cfg(feature = "dns_cache")]
    pub fn cache_stats(&self) -> Option<crate::cache::CacheStats> {
        self.cache.as_ref().map(|c| c.stats())
    }

    /// Resolves to a plain IP, used on the hot path (opening a socket)
    /// where `DnsInfo` isn't needed.
    pub fn resolve(&self, address: &Address) -> Result<IpAddr, RustmcError> {
        if let Ok(ip) = address.host.parse::<IpAddr>() {
            return Ok(ip);
        }
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            if let Some(ip) = cache.get_dns(&address.host) {
                return Ok(ip);
            }
        }
        let ip = address.resolve_ip()?;
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            cache.insert_dns(&address.host, ip);
        }
        Ok(ip)
    }

    pub fn resolve_socket_addr(&self, address: &Address) -> Result<SocketAddr, RustmcError> {
        Ok(SocketAddr::new(self.resolve(address)?, address.port))
    }

    #[cfg(feature = "async")]
    pub async fn resolve_async(&self, address: &Address) -> Result<IpAddr, RustmcError> {
        if let Ok(ip) = address.host.parse::<IpAddr>() {
            return Ok(ip);
        }
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            if let Some(ip) = cache.get_dns(&address.host) {
                return Ok(ip);
            }
        }
        let ip = address.resolve_ip_async().await?;
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            cache.insert_dns(&address.host, ip);
        }
        Ok(ip)
    }

    #[cfg(feature = "async")]
    pub async fn resolve_socket_addr_async(&self, address: &Address) -> Result<SocketAddr, RustmcError> {
        Ok(SocketAddr::new(self.resolve_async(address).await?, address.port))
    }

    /// More expensive path — called once when building
    /// `JavaStatusResponse`/`BedrockStatusResponse`, not on every
    /// connection. One network resolve on the uncached path (`std::net`'s
    /// `ToSocketAddrs` already returns every A/AAAA record, so the first
    /// one doubles as the primary IP — no second lookup needed).
    pub fn resolve_with_info(&self, address: &Address) -> Result<(SocketAddr, Option<DnsInfo>), RustmcError> {
        if let Ok(ip) = address.host.parse::<IpAddr>() {
            return Ok((SocketAddr::new(ip, address.port), None));
        }

        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            if let Some(ip) = cache.get_dns(&address.host) {
                let (cname, ttl) = self.lookup_cname_and_ttl_or_default(&address.host);
                return Ok((SocketAddr::new(ip, address.port), Some(DnsInfo { a_records: vec![ip], cname, ttl })));
            }
        }

        let addrs: Vec<IpAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(address.host.as_str(), address.port))
            .map_err(|e| RustmcError::Dns(format!("failed to resolve {}: {e}", address.host)))?
            .map(|sa| sa.ip())
            .collect();
        let ip = *addrs
            .first()
            .ok_or_else(|| RustmcError::Dns(format!("no A/AAAA records for {}", address.host)))?;

        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            cache.insert_dns(&address.host, ip);
        }

        let (cname, ttl) = self.lookup_cname_and_ttl_or_default(&address.host);
        Ok((SocketAddr::new(ip, address.port), Some(DnsInfo { a_records: addrs, cname, ttl })))
    }

    fn lookup_cname_and_ttl_or_default(&self, host: &str) -> (Option<String>, u32) {
        #[cfg(feature = "srv")]
        {
            crate::dns::lookup_cname_and_ttl(host).unwrap_or((None, self.cache_ttl_seconds()))
        }
        #[cfg(not(feature = "srv"))]
        {
            (None, self.cache_ttl_seconds())
        }
    }

    #[cfg(feature = "dns_cache")]
    fn cache_ttl_seconds(&self) -> u32 {
        self.cache.as_ref().map(|c| c.ttl().as_secs() as u32).unwrap_or(60)
    }
    #[cfg(not(feature = "dns_cache"))]
    fn cache_ttl_seconds(&self) -> u32 {
        60
    }

    /// SRV resolution routed through the same cache as `resolve`/
    /// `resolve_async` — this is what makes `cache_stats().srv_entries`
    /// actually reflect SRV lookups instead of always reading `0`.
    #[cfg(feature = "srv")]
    pub fn resolve_srv(&self, host: &str, default_port: u16) -> Result<Address, RustmcError> {
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            if let Some(addr) = cache.get_srv(host) {
                return Ok(addr);
            }
        }
        let addr = crate::address::srv_lookup(host, default_port)?;
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            cache.insert_srv(host, addr.clone());
        }
        Ok(addr)
    }

    #[cfg(all(feature = "srv", feature = "async"))]
    pub async fn resolve_srv_async(&self, host: &str, default_port: u16) -> Result<Address, RustmcError> {
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            if let Some(addr) = cache.get_srv(host) {
                return Ok(addr);
            }
        }
        let addr = crate::address::srv_lookup_async(host, default_port).await?;
        #[cfg(feature = "dns_cache")]
        if let Some(ref cache) = self.cache {
            cache.insert_srv(host, addr.clone());
        }
        Ok(addr)
    }
}

impl Default for AddressResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// SRV lookup internals (used by `Address::srv_lookup`). Kept here rather
/// than in `address.rs` because it needs `hickory_resolver::Resolver`
/// construction, which only `resolver.rs`/`dns.rs` otherwise deal with.
///
/// **Not independently verified against docs.rs in this environment** (no
/// network access here) — the synchronous `hickory-resolver` 0.24 `Resolver`
/// API is used based on its established shape in earlier
/// `trust-dns-resolver`/`hickory-resolver` releases. Double check
/// `srv_lookup`'s exact signature against the version that actually
/// resolves in `Cargo.lock` before relying on this in production.
#[cfg(feature = "srv")]
pub mod srv {
    use super::*;
    use hickory_resolver::config::{ResolverConfig, ResolverOpts};
    use hickory_resolver::Resolver;

    pub fn resolve_srv_record(host: &str) -> Result<Option<(String, u16)>, RustmcError> {
        let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default())
            .map_err(|e| RustmcError::Dns(format!("failed to build resolver: {e}")))?;
        let name = format!("_minecraft._tcp.{host}");

        match resolver.srv_lookup(&name) {
            Ok(lookup) => {
                let record = lookup.iter().next();
                Ok(record.map(|r| {
                    let target = r.target().to_utf8();
                    (target.trim_end_matches('.').to_owned(), r.port())
                }))
            }
            Err(e) => {
                if is_no_records(&e) {
                    Ok(None)
                } else {
                    Err(RustmcError::Dns(format!("SRV lookup for {name} failed: {e}")))
                }
            }
        }
    }

    fn is_no_records(e: &hickory_resolver::error::ResolveError) -> bool {
        use hickory_resolver::error::ResolveErrorKind;
        matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. })
    }

    /// Full `host:port` resolution mimicking the Minecraft client's own
    /// address field behavior: explicit port wins; no port -> SRV lookup;
    /// no SRV record -> `default_port`.
    pub fn minecraft_srv_lookup(address_str: &str, default_port: u16) -> Result<Address, RustmcError> {
        let parsed = Address::parse(address_str, default_port)?;
        let had_explicit_port =
            address_str.trim().starts_with('[') || address_str.rsplit_once(':').is_some();
        if had_explicit_port {
            return Ok(parsed);
        }
        match resolve_srv_record(&parsed.host)? {
            Some((target, port)) => Ok(Address { host: target, port }),
            None => Ok(parsed),
        }
    }
}
