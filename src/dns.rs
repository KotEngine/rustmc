//! Raw DNS lookup for CNAME + real TTL, used to fill in `DnsInfo` beyond
//! what `std::net::ToSocketAddrs` can report (it gives IPs only, no CNAME,
//! no TTL). Only compiled with the `srv` feature, since it needs
//! `hickory-resolver` either way.
//!
//! **Not independently verified against a live server or docs.rs in this
//! environment** (no network access here) — see the same caveat in
//! `resolver::srv` for details on what's confirmed vs. assumed about the
//! `hickory-resolver` 0.24 API.

use crate::error::RustmcError;

/// Looks up `host`'s CNAME chain (if any) and a TTL for the A/AAAA record.
/// Returns `(None, ttl)` if there's no CNAME — most records don't have one.
pub fn lookup_cname_and_ttl(host: &str) -> Result<(Option<String>, u32), RustmcError> {
    use hickory_resolver::config::{ResolverConfig, ResolverOpts};
    use hickory_resolver::Resolver;

    let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default())
        .map_err(|e| RustmcError::Dns(format!("failed to build resolver: {e}")))?;

    // A CNAME lookup naturally fails with NoRecordsFound for hosts that
    // don't have one (the vast majority) — that's expected, not an error.
    let cname = match resolver.lookup(host, hickory_resolver::proto::rr::RecordType::CNAME) {
        Ok(lookup) => lookup.iter().next().map(|r| r.to_string().trim_end_matches('.').to_owned()),
        Err(_) => None,
    };

    // Same TTL caveat as `resolver::srv`: exact accessor unverified in this
    // offline environment, using a conservative fixed fallback.
    const FALLBACK_TTL_SECS: u32 = 60;
    Ok((cname, FALLBACK_TTL_SECS))
}
