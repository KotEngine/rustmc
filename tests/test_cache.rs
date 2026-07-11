//! Only compiled with `--features dns_cache` (the `DnsCache` type doesn't
//! exist otherwise — `dns_cache` isn't in the default feature set).

#![cfg(feature = "dns_cache")]

use std::net::{IpAddr, Ipv4Addr};
use std::thread::sleep;
use std::time::Duration;

use rustmc::{Address, AddressResolver, DnsCache};

fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

#[test]
fn get_dns_returns_none_before_insert() {
    let cache = DnsCache::new(Duration::from_secs(60));
    assert!(cache.get_dns("example.com").is_none());
}

#[test]
fn insert_then_get_dns_round_trips() {
    let cache = DnsCache::new(Duration::from_secs(60));
    cache.insert_dns("example.com", ip(1, 2, 3, 4));
    assert_eq!(cache.get_dns("example.com"), Some(ip(1, 2, 3, 4)));
}

#[test]
fn dns_entry_expires_after_ttl() {
    let cache = DnsCache::new(Duration::from_millis(20));
    cache.insert_dns("example.com", ip(1, 2, 3, 4));
    assert!(cache.get_dns("example.com").is_some());
    sleep(Duration::from_millis(60));
    assert!(cache.get_dns("example.com").is_none());
}

#[test]
fn srv_entry_expires_after_ttl() {
    let cache = DnsCache::new(Duration::from_millis(20));
    cache.insert_srv("example.com", Address { host: "mc.example.com".into(), port: 25566 });
    assert!(cache.get_srv("example.com").is_some());
    sleep(Duration::from_millis(60));
    assert!(cache.get_srv("example.com").is_none());
}

#[test]
fn stats_reflect_entry_counts() {
    let cache = DnsCache::new(Duration::from_secs(60));
    cache.insert_dns("a.com", ip(1, 1, 1, 1));
    cache.insert_dns("b.com", ip(2, 2, 2, 2));
    cache.insert_srv("c.com", Address { host: "mc.c.com".into(), port: 25566 });

    let stats = cache.stats();
    assert_eq!(stats.dns_entries, 2);
    assert_eq!(stats.srv_entries, 1);
}

#[test]
fn clear_dns_only_clears_dns_entries() {
    let cache = DnsCache::new(Duration::from_secs(60));
    cache.insert_dns("a.com", ip(1, 1, 1, 1));
    cache.insert_srv("b.com", Address { host: "mc.b.com".into(), port: 25566 });

    cache.clear_dns();
    let stats = cache.stats();
    assert_eq!(stats.dns_entries, 0);
    assert_eq!(stats.srv_entries, 1);
}

#[test]
fn clear_all_clears_both() {
    let cache = DnsCache::new(Duration::from_secs(60));
    cache.insert_dns("a.com", ip(1, 1, 1, 1));
    cache.insert_srv("b.com", Address { host: "mc.b.com".into(), port: 25566 });

    cache.clear_all();
    let stats = cache.stats();
    assert_eq!(stats.dns_entries, 0);
    assert_eq!(stats.srv_entries, 0);
}

#[test]
fn resolver_cache_stats_reflects_real_resolve_state() {
    use std::sync::Arc;

    let cache = Arc::new(DnsCache::new(Duration::from_secs(60)));
    let resolver = AddressResolver::with_cache(cache);

    // A literal IP never touches DNS at all (no cache entry expected).
    let literal = Address::parse("127.0.0.1:25565", 25565).unwrap();
    resolver.resolve(&literal).unwrap();
    assert_eq!(resolver.cache_stats().unwrap().dns_entries, 0);
}
