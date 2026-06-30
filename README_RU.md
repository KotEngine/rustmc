# rustmc

[English](README.md) | [Русский](README_RU.md)

Библиотека для получения статуса серверов Minecraft на Rust: Java Edition (SLP), Bedrock Edition (RakNet) и Query-протокол. Sync и async API.

[![Crates.io](https://img.shields.io/crates/v/rustmc.svg)](https://crates.io/crates/rustmc)
[![Docs.rs](https://docs.rs/rustmc/badge.svg)](https://docs.rs/rustmc)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

## Возможности

- Server list ping для Java Edition (статус + latency)
- Unconnected ping для Bedrock Edition
- Query-протокол (полный stat)
- Парсинг MOTD с выводом в plain/ANSI/HTML/Minecraft-форматировании
- Резолв SRV-записей
- Sync и async (tokio) API
- CLI-бинарник (`rustmc`)

## Установка

```toml
[dependencies]
rustmc = "0.0.1"
```

## Использование

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

let server = BedrockServer::lookup("play.example.net:19132")?;
let status = server.status()?;
println!("{} ({}/{})", status.version.name, status.players.online, status.players.max);
```

## CLI

```
cargo install rustmc

rustmc mc.hypixel.net status
rustmc mc.hypixel.net ping
rustmc play.example.net --bedrock status
rustmc mc.hypixel.net json
```

## Лицензия

Apache-2.0
