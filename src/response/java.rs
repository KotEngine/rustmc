//! Java Edition status response types.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::RustmcError;
use crate::motd::Motd;
use crate::response::dns_info::DnsInfo;
use crate::response::forge::ForgeData;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaStatusResponse {
    pub players: JavaStatusPlayers,
    pub version: JavaStatusVersion,
    pub motd: Motd,
    /// Round-trip latency of the status request, in milliseconds.
    pub latency: f64,
    pub enforces_secure_chat: Option<bool>,
    /// Raw `data:image/png;base64,...` favicon URI, undecoded. Use
    /// [`icon_bytes`](Self::icon_bytes) to decode on demand.
    pub icon: Option<String>,
    pub forge_data: Option<ForgeData>,
    /// `None` until address-resolution DNS info collection is wired up
    /// (see `response/dns_info.rs`).
    pub dns: Option<DnsInfo>,
    /// The unmodified JSON response from the server.
    pub raw: Value,
}

impl JavaStatusResponse {
    /// Builds a response from the server's raw status JSON.
    ///
    /// # Errors
    /// Returns `RustmcError::InvalidResponse` if required fields
    /// (`players`, `version`) are missing or have the wrong shape.
    pub fn build(raw: Value, latency: f64) -> Result<Self, RustmcError> {
        let players_raw = raw
            .get("players")
            .ok_or_else(|| RustmcError::InvalidResponse("missing 'players' field".into()))?;
        let version_raw = raw
            .get("version")
            .ok_or_else(|| RustmcError::InvalidResponse("missing 'version' field".into()))?;

        let players = JavaStatusPlayers::build(players_raw)?;
        let version = JavaStatusVersion::build(version_raw)?;

        let motd = match raw.get("description") {
            Some(d) => Motd::parse_json(d, false),
            None => Motd::parse("", false),
        };

        let forge_data = raw
            .get("forgeData")
            .or_else(|| raw.get("modinfo"))
            .and_then(ForgeData::try_parse);

        Ok(Self {
            players,
            version,
            motd,
            latency,
            enforces_secure_chat: raw.get("enforcesSecureChat").and_then(Value::as_bool),
            icon: raw.get("favicon").and_then(Value::as_str).map(str::to_owned),
            forge_data,
            dns: None,
            raw,
        })
    }

    /// Favicon MIME type without decoding the payload (usually
    /// `"image/png"`).
    pub fn icon_mime_type(&self) -> Option<&str> {
        let icon = self.icon.as_deref()?;
        let header = icon.strip_prefix("data:")?.split(',').next()?;
        header.split(';').next()
    }

    /// Decodes the favicon's base64 payload into raw image bytes.
    ///
    /// Returns `None` if there is no favicon at all, or `Some(Err(..))` if
    /// there was one but it's malformed — these are different situations
    /// callers may want to distinguish.
    ///
    /// # Errors
    /// `RustmcError::InvalidResponse` if the data URI is malformed, base64
    /// decoding fails, or (for `image/png`) the decoded bytes don't start
    /// with the PNG signature.
    pub fn icon_bytes(&self) -> Option<Result<Vec<u8>, RustmcError>> {
        let icon = self.icon.as_deref()?;
        Some(Self::decode_icon(icon))
    }

    fn decode_icon(icon: &str) -> Result<Vec<u8>, RustmcError> {
        let (header, payload) = icon
            .split_once(',')
            .ok_or_else(|| RustmcError::InvalidResponse("malformed favicon data URI".into()))?;
        if !header.starts_with("data:") || !header.contains("base64") {
            return Err(RustmcError::InvalidResponse(
                "favicon data URI is not base64-encoded".into(),
            ));
        }
        let mime = header.trim_start_matches("data:").split(';').next().unwrap_or("");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| RustmcError::InvalidResponse(format!("invalid favicon base64: {e}")))?;

        if mime == "image/png" {
            const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
            if !bytes.starts_with(&PNG_SIG) {
                return Err(RustmcError::InvalidResponse(
                    "favicon claims image/png but signature doesn't match".into(),
                ));
            }
        }
        Ok(bytes)
    }

    /// Decodes and writes the favicon to `path`. Blocking (`std::fs::write`).
    ///
    /// # Errors
    /// `RustmcError::InvalidResponse` if decoding fails, `RustmcError::Io`
    /// if writing the file fails.
    pub fn save_icon(&self, path: &std::path::Path) -> Result<(), RustmcError> {
        let bytes = self
            .icon_bytes()
            .ok_or_else(|| RustmcError::InvalidResponse("server did not send a favicon".into()))??;
        std::fs::write(path, bytes).map_err(RustmcError::Io)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaStatusPlayers {
    pub online: u32,
    pub max: u32,
    pub sample: Option<Vec<JavaStatusPlayer>>,
}

impl JavaStatusPlayers {
    fn build(raw: &Value) -> Result<Self, RustmcError> {
        let online = raw
            .get("online")
            .and_then(Value::as_u64)
            .ok_or_else(|| RustmcError::InvalidResponse("missing/invalid 'players.online'".into()))?
            as u32;
        let max = raw
            .get("max")
            .and_then(Value::as_u64)
            .ok_or_else(|| RustmcError::InvalidResponse("missing/invalid 'players.max'".into()))?
            as u32;
        let sample = raw.get("sample").and_then(Value::as_array).map(|arr| {
            arr.iter()
                .filter_map(JavaStatusPlayer::build)
                .collect::<Vec<_>>()
        });
        Ok(Self { online, max, sample })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaStatusPlayer {
    pub name: String,
    /// Player UUID as a string (intentionally not a validated `uuid::Uuid`
    /// type — this field is only ever displayed, never parsed back into a
    /// binary UUID for protocol purposes, and a malformed value here
    /// shouldn't fail parsing the rest of the response).
    pub id: String,
}

impl JavaStatusPlayer {
    fn build(raw: &Value) -> Option<Self> {
        Some(Self {
            name: raw.get("name")?.as_str()?.to_owned(),
            id: raw.get("id")?.as_str()?.to_owned(),
        })
    }

    pub fn uuid(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaStatusVersion {
    pub name: String,
    pub protocol: u32,
}

impl JavaStatusVersion {
    fn build(raw: &Value) -> Result<Self, RustmcError> {
        Ok(Self {
            name: raw
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| RustmcError::InvalidResponse("missing/invalid 'version.name'".into()))?
                .to_owned(),
            protocol: raw
                .get("protocol")
                .and_then(Value::as_u64)
                .ok_or_else(|| RustmcError::InvalidResponse("missing/invalid 'version.protocol'".into()))?
                as u32,
        })
    }
}
