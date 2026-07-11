//! Bedrock Edition status via RakNet's Unconnected Ping/Pong.
//!
//! Request layout confirmed against two independent working
//! implementations (Python `mcstatus`'s `bedrock_client.py`, and the Rust
//! `elytra-ping` crate, both tested against live servers): 33 bytes total —
//! `0x01` (packet id) + 8-byte timestamp + 16-byte RakNet magic + 8-byte
//! client GUID. `mcstatus` sends timestamp and GUID as all-zero, which
//! works fine in production (the server just echoes the timestamp back;
//! it doesn't validate either field for an Unconnected Ping), so we match
//! that instead of inventing random values that add no real benefit.
//!
//! Response layout: `0x1c` (packet id) + 8-byte timestamp echo + 8-byte
//! server GUID + 16-byte magic + 2-byte big-endian MOTD string length +
//! the MOTD string itself (semicolon-separated fields, see
//! `response::bedrock::BedrockStatusResponse::build`).

use std::time::Instant;

use crate::error::RustmcError;
use crate::protocol::io::{Buffer, UdpConnection};
use crate::response::bedrock::BedrockStatusResponse;

const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

const UNCONNECTED_PING: u8 = 0x01;
const UNCONNECTED_PONG: u8 = 0x1c;

pub struct BedrockClient {
    conn: UdpConnection,
}

impl BedrockClient {
    pub fn new(conn: UdpConnection) -> Self {
        Self { conn }
    }

    pub fn read_status(&self) -> Result<BedrockStatusResponse, RustmcError> {
        let mut request = Buffer::new();
        request.write_u8(UNCONNECTED_PING);
        request.write_i64_be(0); // timestamp: server only echoes this, not validated
        request.write_bytes(&MAGIC);
        request.write_i64_be(0); // client GUID: same, unused for a status-only ping

        let start = Instant::now();
        self.conn.send(&request.into_packet_unframed())?;
        let data = self.conn.recv()?;
        let end = Instant::now();

        let mut response = Buffer::from_vec(data);
        let packet_id = response.read_u8()?;
        if packet_id != UNCONNECTED_PONG {
            return Err(RustmcError::InvalidResponse(format!(
                "expected Unconnected Pong (0x1c), got {packet_id:#04x}"
            )));
        }
        let _timestamp = response.read_i64_be()?;
        let _server_guid = response.read_i64_be()?;
        let magic = response.read_bytes(16)?;
        if magic != MAGIC {
            return Err(RustmcError::InvalidResponse("RakNet magic mismatch in Unconnected Pong".into()));
        }
        let name_length = response.read_u16_be()? as usize;
        let motd_bytes = response.read_bytes(name_length)?;
        let motd_str = String::from_utf8(motd_bytes)
            .map_err(|e| RustmcError::InvalidResponse(format!("invalid UTF-8 in Bedrock MOTD: {e}")))?;

        let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
        BedrockStatusResponse::build(&motd_str, latency_ms)
    }
}

#[cfg(feature = "async")]
pub use async_impl::AsyncBedrockClient;

#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use crate::protocol::io::AsyncUdpConnection;

    pub struct AsyncBedrockClient {
        conn: AsyncUdpConnection,
    }

    impl AsyncBedrockClient {
        pub fn new(conn: AsyncUdpConnection) -> Self {
            Self { conn }
        }

        pub async fn read_status(&self) -> Result<BedrockStatusResponse, RustmcError> {
            let mut request = Buffer::new();
            request.write_u8(UNCONNECTED_PING);
            request.write_i64_be(0);
            request.write_bytes(&MAGIC);
            request.write_i64_be(0);

            let start = Instant::now();
            self.conn.send(&request.into_packet_unframed()).await?;
            let data = self.conn.recv().await?;
            let end = Instant::now();

            let mut response = Buffer::from_vec(data);
            let packet_id = response.read_u8()?;
            if packet_id != UNCONNECTED_PONG {
                return Err(RustmcError::InvalidResponse(format!(
                    "expected Unconnected Pong (0x1c), got {packet_id:#04x}"
                )));
            }
            let _timestamp = response.read_i64_be()?;
            let _server_guid = response.read_i64_be()?;
            let magic = response.read_bytes(16)?;
            if magic != MAGIC {
                return Err(RustmcError::InvalidResponse("RakNet magic mismatch in Unconnected Pong".into()));
            }
            let name_length = response.read_u16_be()? as usize;
            let motd_bytes = response.read_bytes(name_length)?;
            let motd_str = String::from_utf8(motd_bytes)
                .map_err(|e| RustmcError::InvalidResponse(format!("invalid UTF-8 in Bedrock MOTD: {e}")))?;

            let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
            BedrockStatusResponse::build(&motd_str, latency_ms)
        }
    }
}
