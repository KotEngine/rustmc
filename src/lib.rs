//! Minecraft server status library: Java Edition (SLP), Bedrock Edition
//! (RakNet), Query protocol, and Legacy SLP (Minecraft < 1.7). Sync and
//! async APIs.
//!
//! ```rust,no_run
//! use rustmc::JavaServer;
//!
//! let server = JavaServer::lookup("mc.hypixel.net")?;
//! let status = server.status()?;
//! println!("{}/{}", status.players.online, status.players.max);
//! println!("{}", status.motd.to_plain());
//! # Ok::<(), rustmc::RustmcError>(())
//! ```

pub mod address;
#[cfg(feature = "async")]
pub mod batch;
#[cfg(feature = "dns_cache")]
pub mod cache;
#[cfg(feature = "srv")]
pub mod dns;
pub mod error;
pub mod motd;
pub mod protocol;
pub mod resolver;
pub mod response;
pub mod server;

pub use address::Address;
#[cfg(feature = "dns_cache")]
pub use cache::{CacheStats, DnsCache};
pub use error::RustmcError;
pub use motd::Motd;
pub use resolver::AddressResolver;
pub use response::{
    BedrockStatusPlayers, BedrockStatusResponse, BedrockStatusVersion, DnsInfo, ForgeChannel,
    ForgeData, ForgeMod, JavaStatusPlayer, JavaStatusPlayers, JavaStatusResponse,
    JavaStatusVersion, LegacyStatusPlayers, LegacyStatusResponse, LegacyStatusVersion,
    QueryBasicResponse, QueryPlayers, QueryResponse, QuerySoftware,
};
pub use server::{BedrockServer, JavaServer, LegacyServer};
