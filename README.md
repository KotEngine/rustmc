# rustmc

[English](README.md) | [Русский](README_RU.md)

Minecraft server status library for Rust: Java Edition (SLP), Bedrock Edition (RakNet), Query protocol, and Legacy SLP (Minecraft < 1.7). Sync and async APIs.

[![Crates.io](https://img.shields.io/crates/v/rustmc.svg)](https://crates.io/crates/rustmc)
[![Docs.rs](https://docs.rs/rustmc/badge.svg)](https://docs.rs/rustmc)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

## Features

- Java Edition server list ping (status + latency)
- Bedrock Edition unconnected ping
- Legacy Server List Ping for Minecraft < 1.7 (Beta 1.8–1.6.4)
- Query protocol (full stat + lightweight basic stat)
- MOTD parsing with plain/ANSI/HTML/Minecraft-formatted output
- SRV record resolution with optional DNS caching (`DnsInfo` exposed: A-records, CNAME, TTL)
- Cache observability via `cache_stats()`
- Batch ping for many servers at once (`Vec` or `Stream`, per-target timeout)
- Sync and async (tokio) APIs
- CLI binary (`rustmc`)

## Install

```toml
[dependencies]
rustmc = "0.0.1"
```

## Usage

```rust
use rustmc::JavaServer;

let server = JavaServer::lookup("mc.hypixel.net")?;
let status = server.status()?;

println!("{}/{}", status.players.online, status.players.max);
println!("{}", status.motd.to_plain());
println!("ping: {:.2}ms", status.latency);
```

Async:

```rust
let server = JavaServer::lookup_async("mc.hypixel.net").await?;
let status = server.status_async().await?;
```

Bedrock:

```rust
use rustmc::BedrockServer;

let server = BedrockServer::lookup("geo.hivebedrock.network:19132")?;
let status = server.status()?;
println!("{} ({}/{})", status.version.name, status.players.online, status.players.max);
```

Legacy (Minecraft < 1.7):

```rust
use rustmc::LegacyServer;

let server = LegacyServer::lookup("oldserver.example.net")?;
let status = server.status()?;
println!("{} ({}/{})", status.motd.to_plain(), status.players.online, status.players.max);
```

Query (full and basic stat):

```rust
use rustmc::JavaServer;

let server = JavaServer::lookup("mc.hypixel.net")?;
let full = server.query()?;
println!("plugins: {:?}", full.software.plugins);

let basic = server.query_basic()?;
println!("{} ({}/{})", basic.motd.to_plain(), basic.online, basic.max);
```

DNS info and cache stats:

```rust
if let Some(dns) = &status.dns {
    println!("A records: {:?}, CNAME: {:?}, TTL: {}s", dns.a_records, dns.cname, dns.ttl);
}

if let Some(stats) = server.cache_stats() {
    println!("dns entries: {}, srv entries: {}", stats.dns_entries, stats.srv_entries);
}
```

Batch ping:

```rust
use rustmc::batch::{ServerTarget, Edition, ping_many};
use std::time::Duration;

let targets = vec![
    ServerTarget::new("mc.hypixel.net", Edition::Java),
    ServerTarget::with_timeout("geo.hivebedrock.network:19132", Edition::Bedrock, Duration::from_secs(1)),
];

let results = ping_many(&targets, 10, Duration::from_secs(3)).await;
```

## CLI

```
cargo install rustmc

rustmc mc.hypixel.net status
rustmc mc.hypixel.net ping
rustmc geo.hivebedrock.network --bedrock status
rustmc oldserver.example.net --legacy status
rustmc mc.hypixel.net json
```

## License

Apache-2.0
