//! Bedrock Edition (RakNet Unconnected Pong) status response.

use crate::error::RustmcError;
use crate::motd::Motd;

#[derive(Debug, Clone)]
pub struct BedrockStatusResponse {
    pub motd: Motd,
    pub players: BedrockStatusPlayers,
    pub version: BedrockStatusVersion,
    pub map_name: Option<String>,
    pub gamemode: Option<String>,
    pub latency: f64,
    /// The raw, semicolon-separated Unconnected Pong string, unparsed.
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct BedrockStatusPlayers {
    pub online: u32,
    pub max: u32,
}

#[derive(Debug, Clone)]
pub struct BedrockStatusVersion {
    pub name: String,
    pub protocol: i32,
    /// `MCPE` or `MCEE` (Education Edition).
    pub brand: String,
}

impl BedrockStatusResponse {
    /// Parses the `;`-separated Unconnected Pong payload.
    ///
    /// Field order: `[edition, motd_line1, protocol, version, online, max,
    /// server_id, map_name, gamemode, gamemode_num, port4, port6]`. Uses
    /// `.get(i)` throughout rather than direct indexing — Geyser,
    /// Waterdog, and other proxies sometimes send fewer than all 12
    /// fields, and indexing directly would panic on those.
    pub fn build(raw: &str, latency: f64) -> Result<Self, RustmcError> {
        let fields: Vec<&str> = raw.split(';').collect();
        let get = |i: usize| fields.get(i).copied();

        let brand = get(0)
            .ok_or_else(|| RustmcError::InvalidResponse("missing edition/brand field".into()))?
            .to_owned();
        let motd_line1 = get(1).unwrap_or("");
        let protocol: i32 = get(2)
            .ok_or_else(|| RustmcError::InvalidResponse("missing protocol field".into()))?
            .parse()
            .map_err(|_| RustmcError::InvalidResponse("invalid protocol field".into()))?;
        let version_name = get(3).unwrap_or("").to_owned();
        let online: u32 = get(4)
            .ok_or_else(|| RustmcError::InvalidResponse("missing online-players field".into()))?
            .parse()
            .unwrap_or(0);
        let max: u32 = get(5)
            .ok_or_else(|| RustmcError::InvalidResponse("missing max-players field".into()))?
            .parse()
            .unwrap_or(0);
        let map_name = get(7).filter(|s| !s.is_empty()).map(str::to_owned);
        let gamemode = get(8).filter(|s| !s.is_empty()).map(str::to_owned);

        Ok(Self {
            motd: Motd::parse(motd_line1, true),
            players: BedrockStatusPlayers { online, max },
            version: BedrockStatusVersion { name: version_name, protocol, brand },
            map_name,
            gamemode,
            latency,
            raw: raw.to_owned(),
        })
    }
}
