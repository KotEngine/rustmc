//! Server address parsing and resolution.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use crate::error::RustmcError;

/// A `host:port` pair identifying a Minecraft server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

impl Address {
    /// Parses `address` into host/port.
    ///
    /// Supports:
    /// - `host:port` / `host` (falls back to `default_port`)
    /// - IPv6 in brackets: `[2001:db8::1]:25565` or bare `[2001:db8::1]`
    ///
    /// IPv6 addresses without brackets are ambiguous with `host:port`
    /// splitting (the address itself contains colons), so the bracketed
    /// form is required whenever a port follows an IPv6 host.
    pub fn parse(address: &str, default_port: u16) -> Result<Self, RustmcError> {
        let address = address.trim();
        if address.is_empty() {
            return Err(RustmcError::InvalidAddress("empty address".into()));
        }

        if let Some(rest) = address.strip_prefix('[') {
            let close = rest.find(']').ok_or_else(|| {
                RustmcError::InvalidAddress(format!("unterminated '[' in address {address:?}"))
            })?;
            let host = &rest[..close];
            let after = &rest[close + 1..];
            let port = if let Some(p) = after.strip_prefix(':') {
                p.parse::<u16>().map_err(|_| {
                    RustmcError::InvalidAddress(format!("invalid port in address {address:?}"))
                })?
            } else if after.is_empty() {
                default_port
            } else {
                return Err(RustmcError::InvalidAddress(format!(
                    "unexpected trailing data after ']' in address {address:?}"
                )));
            };
            return Ok(Self { host: host.to_owned(), port });
        }

        match address.rsplit_once(':') {
            Some((host, port_str)) if !host.is_empty() && port_str.parse::<u16>().is_ok() => {
                Ok(Self {
                    host: host.to_owned(),
                    port: port_str.parse().unwrap(),
                })
            }
            _ => Ok(Self { host: address.to_owned(), port: default_port }),
        }
    }

    /// Resolves the host to an IP address. If `host` is already a literal
    /// IP, no DNS lookup happens. Otherwise performs a system A/AAAA
    /// lookup via `ToSocketAddrs` and returns the first result.
    pub fn resolve_ip(&self) -> Result<IpAddr, RustmcError> {
        if let Ok(ip) = self.host.parse::<IpAddr>() {
            return Ok(ip);
        }
        let mut addrs = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|e| RustmcError::Dns(format!("failed to resolve {}: {e}", self.host)))?;
        addrs
            .next()
            .map(|s: SocketAddr| s.ip())
            .ok_or_else(|| RustmcError::Dns(format!("no A/AAAA records for {}", self.host)))
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, RustmcError> {
        Ok(SocketAddr::new(self.resolve_ip()?, self.port))
    }

    /// Async counterpart to `resolve_ip`. Uses `tokio::net::lookup_host`
    /// (which itself runs the blocking system resolver on tokio's blocking
    /// thread pool internally) rather than a raw DNS library — this is
    /// the same system resolver behavior as `resolve_ip`, just not
    /// blocking the calling task's executor thread while it runs.
    #[cfg(feature = "async")]
    pub async fn resolve_ip_async(&self) -> Result<IpAddr, RustmcError> {
        if let Ok(ip) = self.host.parse::<IpAddr>() {
            return Ok(ip);
        }
        let mut addrs = tokio::net::lookup_host((self.host.as_str(), self.port))
            .await
            .map_err(|e| RustmcError::Dns(format!("failed to resolve {}: {e}", self.host)))?;
        addrs
            .next()
            .map(|s: SocketAddr| s.ip())
            .ok_or_else(|| RustmcError::Dns(format!("no A/AAAA records for {}", self.host)))
    }
}

/// Looks up the `_minecraft._tcp.<host>` SRV record and returns the
/// resolved `Address` (target host + port from the record), or the
/// original host with `default_port` if there's no SRV record.
///
/// Only does anything with an explicit port already present — Minecraft's
/// own client only consults SRV when the user didn't type a port, and a
/// server operator who wrote out `host:port` explicitly should always get
/// exactly that back, never an SRV-redirected target.
#[cfg(feature = "srv")]
pub fn srv_lookup(host: &str, default_port: u16) -> Result<Address, RustmcError> {
    crate::resolver::srv::minecraft_srv_lookup(host, default_port)
}

/// Async counterpart to `srv_lookup`.
///
/// Implementation note: `hickory-resolver`'s synchronous `Resolver` runs
/// its own internal Tokio runtime for the actual network I/O, which can't
/// be nested inside a caller's existing async runtime. Rather than take on
/// the API-surface risk of `hickory-resolver`'s separate async resolver
/// type (unverifiable here, see `resolver::srv`'s caveat), this offloads
/// the same sync lookup to `tokio::task::spawn_blocking` — correct and
/// non-blocking for the caller's executor, at the cost of one blocking-pool
/// thread for the duration of the DNS query.
#[cfg(all(feature = "srv", feature = "async"))]
pub async fn srv_lookup_async(host: &str, default_port: u16) -> Result<Address, RustmcError> {
    let host = host.to_owned();
    tokio::task::spawn_blocking(move || srv_lookup(&host, default_port))
        .await
        .map_err(|e| RustmcError::Dns(format!("SRV lookup task panicked: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_port() {
        let a = Address::parse("mc.hypixel.net:25565", 25565).unwrap();
        assert_eq!(a.host, "mc.hypixel.net");
        assert_eq!(a.port, 25565);
    }

    #[test]
    fn parses_bare_host_with_default_port() {
        let a = Address::parse("mc.hypixel.net", 25565).unwrap();
        assert_eq!(a.host, "mc.hypixel.net");
        assert_eq!(a.port, 25565);
    }

    #[test]
    fn parses_ipv6_bracketed_with_port() {
        let a = Address::parse("[2001:db8::1]:25565", 25565).unwrap();
        assert_eq!(a.host, "2001:db8::1");
        assert_eq!(a.port, 25565);
    }

    #[test]
    fn parses_ipv6_bracketed_without_port() {
        let a = Address::parse("[2001:db8::1]", 19132).unwrap();
        assert_eq!(a.host, "2001:db8::1");
        assert_eq!(a.port, 19132);
    }

    #[test]
    fn rejects_empty() {
        assert!(Address::parse("", 25565).is_err());
    }
}
