//! DNS information collected while resolving a server's address.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// DNS information collected during address resolution.
///
/// Only present on `JavaStatusResponse`/`BedrockStatusResponse` once
/// resolution actually happened — `None` if the address was given as a
/// literal IP. Filled in by `AddressResolver::resolve_with_info` (added in
/// a later pass, alongside `cache.rs`); until then callers will always see
/// `None` here even for hostname addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsInfo {
    pub a_records: Vec<IpAddr>,
    pub cname: Option<String>,
    pub ttl: u32,
}
