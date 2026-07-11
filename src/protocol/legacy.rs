//! Legacy Server List Ping, synchronous client.
//!
//! Deliberately simpler than a literal reading of the wiki's "1.6 SLP"
//! spec (handshake magic byte + `MC|PingHost` plugin message carrying
//! hostname/port/protocol): matching `mcstatus`'s proven-in-production
//! approach, this sends only the bare 3-byte `FE 01 FA` request and
//! determines the server's dialect from the *shape of the response*
//! instead of doing a second round-trip:
//!
//! - `>=1.4` (12w42b+): kick string starts with `§1`, followed by
//!   `\0`-separated `protocol\0version\0motd\0online\0max`.
//! - `<1.4`: kick string is just `motd§online§max` with no `§1` marker at
//!   all — reconstructed into the same 5-field shape with `protocol = -1`
//!   and `version = "<1.4"` so callers get a uniform response either way.
//!
//! This avoids doubling worst-case latency for a fallback most servers
//! will never need, at the cost of not sending a virtual host via
//! `MC|PingHost` (which only matters for BungeeCord-style routing by
//! hostname, not for a status ping).

use std::time::Instant;

use crate::error::RustmcError;
use crate::protocol::io::{Buffer, TcpConnection};
use crate::response::legacy::LegacyStatusResponse;

const REQUEST: [u8; 3] = [0xFE, 0x01, 0xFA];

pub struct LegacyClient {
    conn: TcpConnection,
}

impl LegacyClient {
    pub fn new(conn: TcpConnection) -> Self {
        Self { conn }
    }

    pub fn read_status(&mut self) -> Result<LegacyStatusResponse, RustmcError> {
        let start = Instant::now();
        self.conn.write_all(&REQUEST)?;

        let id = self.conn.read_exact(1)?[0];
        if id != 0xFF {
            return Err(RustmcError::InvalidResponse(format!(
                "expected legacy kick packet id 0xFF, got {id:#04x}"
            )));
        }
        let mut len_buf = Buffer::from_vec(self.conn.read_exact(2)?);
        let len_units = len_buf.read_u16_be()? as usize;
        let data = self.conn.read_exact(len_units * 2)?;
        let end = Instant::now();

        let decoded = decode_utf16_be(&data)?;
        let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
        parse_response(&decoded, latency_ms)
    }
}

fn decode_utf16_be(data: &[u8]) -> Result<String, RustmcError> {
    if !data.len().is_multiple_of(2) {
        return Err(RustmcError::InvalidResponse(
            "legacy kick payload has an odd byte length for UTF-16".into(),
        ));
    }
    let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    String::from_utf16(&units)
        .map_err(|e| RustmcError::InvalidResponse(format!("invalid UTF-16 in legacy kick packet: {e}")))
}

fn parse_response(decoded: &str, latency_ms: f64) -> Result<LegacyStatusResponse, RustmcError> {
    let mut parts: Vec<String> = decoded.split('\0').map(str::to_owned).collect();

    if parts.first().map(String::as_str) != Some("§1") {
        // Pre-1.4 (before 12w42a): no §1 marker, no protocol/version info —
        // it's just "motd§online§max" as a single element.
        let legacy_fields: Vec<String> = parts
            .first()
            .map(|s| s.split('§').map(str::to_owned).collect())
            .unwrap_or_default();
        if legacy_fields.len() != 3 {
            return Err(RustmcError::InvalidResponse(
                "received invalid pre-1.4 legacy kick packet".into(),
            ));
        }
        parts = vec!["-1".to_owned(), "<1.4".to_owned()];
        parts.extend(legacy_fields);
        return LegacyStatusResponse::build(&parts, latency_ms);
    }

    LegacyStatusResponse::build(&parts[1..], latency_ms)
}

#[cfg(feature = "async")]
pub use async_impl::AsyncLegacyClient;

#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use crate::protocol::io::{AsyncTcpConnection, Buffer};

    pub struct AsyncLegacyClient {
        conn: AsyncTcpConnection,
    }

    impl AsyncLegacyClient {
        pub fn new(conn: AsyncTcpConnection) -> Self {
            Self { conn }
        }

        pub async fn read_status(&mut self) -> Result<LegacyStatusResponse, RustmcError> {
            let start = Instant::now();
            self.conn.write_all(&REQUEST).await?;

            let id = self.conn.read_exact(1).await?[0];
            if id != 0xFF {
                return Err(RustmcError::InvalidResponse(format!(
                    "expected legacy kick packet id 0xFF, got {id:#04x}"
                )));
            }
            let mut len_buf = Buffer::from_vec(self.conn.read_exact(2).await?);
            let len_units = len_buf.read_u16_be()? as usize;
            let data = self.conn.read_exact(len_units * 2).await?;
            let end = Instant::now();

            let decoded = decode_utf16_be(&data)?;
            let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
            parse_response(&decoded, latency_ms)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_kick_packet() {
        let decoded = "§1\05\01.4\0A Minecraft Server\03\020";
        let resp = parse_response(decoded, 0.0).unwrap();
        assert_eq!(resp.version.protocol, 5);
        assert_eq!(resp.version.name, "1.4");
        assert_eq!(resp.players.online, 3);
        assert_eq!(resp.players.max, 20);
    }

    #[test]
    fn parses_pre_1_4_kick_packet() {
        let decoded = "A Minecraft Server§3§20";
        let resp = parse_response(decoded, 0.0).unwrap();
        assert_eq!(resp.version.protocol, -1);
        assert_eq!(resp.version.name, "<1.4");
        assert_eq!(resp.players.online, 3);
        assert_eq!(resp.players.max, 20);
    }
}
