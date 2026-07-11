//! Legacy Server List Ping (Minecraft Beta 1.8 – 1.6.x) response.

use crate::error::RustmcError;
use crate::motd::Motd;

#[derive(Debug, Clone)]
pub struct LegacyStatusResponse {
    pub motd: Motd,
    pub players: LegacyStatusPlayers,
    pub version: LegacyStatusVersion,
    pub latency: f64,
}

#[derive(Debug, Clone)]
pub struct LegacyStatusPlayers {
    pub online: u32,
    pub max: u32,
}

#[derive(Debug, Clone)]
pub struct LegacyStatusVersion {
    /// `<1.4` for servers older than 12w42b, which didn't report a
    /// version at all.
    pub name: String,
    /// `-1` for servers older than 12w42b.
    pub protocol: i32,
}

impl LegacyStatusResponse {
    /// Builds a response from the `\0`-split kick-packet fields, *after*
    /// the leading `§1` marker (if present) has already been stripped by
    /// the caller — i.e. `fields` is `[protocol, version, motd, online,
    /// max]` for a >=1.4 server.
    pub fn build(fields: &[String], latency: f64) -> Result<Self, RustmcError> {
        let get = |i: usize, name: &str| -> Result<&str, RustmcError> {
            fields
                .get(i)
                .map(String::as_str)
                .ok_or_else(|| RustmcError::InvalidResponse(format!("legacy response missing field '{name}'")))
        };
        let protocol: i32 = get(0, "protocol")?
            .parse()
            .map_err(|_| RustmcError::InvalidResponse("invalid protocol field in legacy response".into()))?;
        let version_name = get(1, "version")?.to_owned();
        let motd_str = get(2, "motd")?;
        let online: u32 = get(3, "online")?.parse().unwrap_or(0);
        let max: u32 = get(4, "max")?.parse().unwrap_or(0);

        Ok(Self {
            motd: Motd::parse(motd_str, false),
            players: LegacyStatusPlayers { online, max },
            version: LegacyStatusVersion { name: version_name, protocol },
            latency,
        })
    }
}
