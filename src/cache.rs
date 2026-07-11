//! TTL-based DNS/SRV cache. Thread-safe (`dashmap`), used behind
//! `AddressResolver` — see `resolver.rs`.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::address::Address;

struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

pub struct DnsCache {
    dns: DashMap<String, CacheEntry<IpAddr>>,
    srv: DashMap<String, CacheEntry<Address>>,
    ttl: Duration,
}

impl DnsCache {
    pub fn new(ttl: Duration) -> Self {
        Self { dns: DashMap::new(), srv: DashMap::new(), ttl }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn get_dns(&self, host: &str) -> Option<IpAddr> {
        let entry = self.dns.get(host)?;
        if entry.expires_at > Instant::now() {
            Some(entry.value)
        } else {
            None
        }
    }

    pub fn insert_dns(&self, host: &str, ip: IpAddr) {
        self.dns.insert(host.to_owned(), CacheEntry { value: ip, expires_at: Instant::now() + self.ttl });
    }

    pub fn get_srv(&self, host: &str) -> Option<Address> {
        let entry = self.srv.get(host)?;
        if entry.expires_at > Instant::now() {
            Some(entry.value.clone())
        } else {
            None
        }
    }

    pub fn insert_srv(&self, host: &str, addr: Address) {
        self.srv.insert(host.to_owned(), CacheEntry { value: addr, expires_at: Instant::now() + self.ttl });
    }

    pub fn clear_dns(&self) {
        self.dns.clear();
    }

    pub fn clear_srv(&self) {
        self.srv.clear();
    }

    pub fn clear_all(&self) {
        self.clear_dns();
        self.clear_srv();
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats { dns_entries: self.dns.len(), srv_entries: self.srv.len() }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub dns_entries: usize,
    pub srv_entries: usize,
}
