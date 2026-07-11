//! `rustmc` CLI binary.
//!
//! **Status of this build:** async batch-pinging and DNS/SRV features are
//! not wired up here yet — everything else (`status`, `ping`, `query`,
//! `query-basic`, `json`, `--bedrock`, `--legacy`) works.

use std::time::Duration;

use clap::{Parser, ValueEnum};
use rustmc::{BedrockServer, JavaServer, LegacyServer};

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Command {
    Ping,
    Status,
    Query,
    QueryBasic,
    Json,
}

/// Minecraft server status CLI.
#[derive(Parser)]
#[command(name = "rustmc")]
struct Cli {
    /// Server address, e.g. `mc.hypixel.net` or `mc.hypixel.net:25565`.
    address: String,

    /// Command to run.
    #[arg(value_enum, default_value_t = Command::Status)]
    command: Command,

    /// Query a Bedrock server (RakNet, default port 19132).
    #[arg(long)]
    bedrock: bool,

    /// Query a pre-1.7 Java server (Legacy SLP).
    #[arg(long)]
    legacy: bool,

    /// Timeout in seconds.
    #[arg(long, default_value_t = 3)]
    timeout: u64,
}

fn main() {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(cli.timeout);

    let result = run(&cli, timeout);
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: &Cli, timeout: Duration) -> Result<(), rustmc::RustmcError> {
    if cli.bedrock {
        let server = BedrockServer::lookup(&cli.address)?.with_timeout(timeout);
        let status = server.status()?;
        match cli.command {
            Command::Ping => println!("ping: {:.2} ms", status.latency),
            Command::Json => unimplemented!("JSON output for Bedrock isn't wired up yet"),
            _ => {
                println!("edition: {} (protocol {})", status.version.brand, status.version.protocol);
                println!("version: {}", status.version.name);
                println!("motd: {}", status.motd.to_plain());
                println!("players: {}/{}", status.players.online, status.players.max);
                if let Some(map) = &status.map_name {
                    println!("map: {map}");
                }
                if let Some(gm) = &status.gamemode {
                    println!("gamemode: {gm}");
                }
                println!("ping: {:.2} ms", status.latency);
            }
        }
        return Ok(());
    }

    if cli.legacy {
        let server = LegacyServer::lookup(&cli.address)?.with_timeout(timeout);
        let status = server.status()?;
        println!("version: {} (protocol {})", status.version.name, status.version.protocol);
        println!("motd: {}", status.motd.to_plain());
        println!("players: {}/{}", status.players.online, status.players.max);
        println!("ping: {:.2} ms", status.latency);
        return Ok(());
    }

    let server = JavaServer::lookup(&cli.address)?.with_timeout(timeout);

    match cli.command {
        Command::Ping => {
            let latency = server.ping()?;
            println!("ping: {latency:.2} ms");
        }
        Command::Status => {
            let status = server.status()?;
            println!("version: Java {} (protocol {})", status.version.name, status.version.protocol);
            println!("motd: {}", status.motd.to_plain());
            println!("players: {}/{}", status.players.online, status.players.max);
            println!("ping: {:.2} ms", status.latency);
        }
        Command::Json => {
            let status = server.status()?;
            let s = serde_json::to_string_pretty(&status).map_err(rustmc::RustmcError::from)?;
            println!("{s}");
        }
        Command::Query => {
            let q = server.query()?;
            println!("motd: {}", q.motd.to_plain());
            println!("map: {}", q.map_name);
            println!("players: {}/{} {:?}", q.players.online, q.players.max, q.players.list);
            println!("software: {} ({})", q.software.version, q.software.brand);
            if !q.software.plugins.is_empty() {
                println!("plugins: {:?}", q.software.plugins);
            }
        }
        Command::QueryBasic => {
            let q = server.query_basic()?;
            println!("motd: {}", q.motd.to_plain());
            println!("gametype: {}", q.game_type);
            println!("map: {}", q.map);
            println!("players: {}/{}", q.online, q.max);
        }
    }

    Ok(())
}
