//! Java Edition Server List Ping (SLP), synchronous client.
//!
//! Packet order: `Handshake(0x00)` -> `StatusRequest(0x00)` ->
//! `StatusResponse(0x00)` -> `PingRequest(0x01)` -> `PingResponse(0x01)`.
//! Framing: every packet is `VarInt(len) + payload`, where `payload` itself
//! starts with a `VarInt(packet_id)`.

use std::time::Instant;

use crate::address::Address;
use crate::error::RustmcError;
use crate::protocol::io::{Buffer, TcpConnection};

pub struct JavaClient {
    conn: TcpConnection,
    address: Address,
    version: i32,
    ping_token: i64,
}

impl JavaClient {
    pub fn new(conn: TcpConnection, address: Address, version: i32, ping_token: i64) -> Self {
        Self { conn, address, version, ping_token }
    }

    /// Reads one length-prefixed packet: a VarInt length, then exactly that
    /// many bytes, returned as a `Buffer` positioned at the start of the
    /// payload (i.e. at the packet ID).
    fn read_packet(&mut self) -> Result<Buffer, RustmcError> {
        let len = self.read_varint_from_socket()? as usize;
        // The 1 MiB cap in `Buffer`/`read_bytes` covers this too once we
        // hand the bytes over, but checking here avoids blocking trying to
        // `read_exact` an absurd byte count from a slow/malicious peer.
        if len > crate::protocol::io::buffer::MAX_DECLARED_LEN {
            return Err(RustmcError::InvalidResponse(format!(
                "declared packet length {len} exceeds max {}",
                crate::protocol::io::buffer::MAX_DECLARED_LEN
            )));
        }
        let data = self.conn.read_exact(len)?;
        Ok(Buffer::from_vec(data))
    }

    /// VarInt length prefixes arrive one byte at a time straight off the
    /// socket (we don't yet know how many bytes make up the packet, so we
    /// can't hand this to `Buffer` up front).
    fn read_varint_from_socket(&mut self) -> Result<i32, RustmcError> {
        let mut result: i32 = 0;
        let mut position: u32 = 0;
        for i in 0..5 {
            let byte = self.conn.read_exact(1)?[0];
            if i == 4 && (byte & 0x70) != 0 {
                return Err(RustmcError::VarIntOverflow);
            }
            result |= ((byte & 0x7F) as i32) << position;
            position += 7;
            if (byte & 0x80) == 0 {
                return Ok(result);
            }
        }
        Err(RustmcError::VarIntOverflow)
    }

    pub fn handshake(&mut self) -> Result<(), RustmcError> {
        let mut packet = Buffer::new();
        packet.write_varint(0); // packet id: Handshake
        packet.write_varint(self.version);
        packet.write_string(&self.address.host);
        packet.write_u16_be(self.address.port);
        packet.write_varint(1); // next state: status
        self.conn.write_all(&packet.into_packet())
    }

    /// Sends `StatusRequest` and parses `StatusResponse` (JSON status +
    /// round-trip latency).
    pub fn read_status(&mut self) -> Result<(serde_json::Value, f64), RustmcError> {
        let mut request = Buffer::new();
        request.write_varint(0); // packet id: Status Request
        self.conn.write_all(&request.into_packet())?;

        let start = Instant::now();
        let mut response = self.read_packet()?;
        let end = Instant::now();

        let packet_id = response.read_varint()?;
        if packet_id != 0 {
            return Err(RustmcError::InvalidResponse(format!(
                "expected status response packet id 0, got {packet_id}"
            )));
        }
        let json_str = response.read_string()?;
        let raw: serde_json::Value = serde_json::from_str(&json_str)?;
        let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
        Ok((raw, latency_ms))
    }

    /// Sends `PingRequest` with `ping_token` and validates the echoed token
    /// in `PingResponse`, returning the round-trip latency in ms.
    pub fn test_ping(&mut self) -> Result<f64, RustmcError> {
        let mut request = Buffer::new();
        request.write_varint(1); // packet id: Ping Request
        request.write_i64_be(self.ping_token);

        let start = Instant::now();
        self.conn.write_all(&request.into_packet())?;
        let mut response = self.read_packet()?;
        let end = Instant::now();

        let packet_id = response.read_varint()?;
        if packet_id != 1 {
            return Err(RustmcError::InvalidResponse(format!(
                "expected ping response packet id 1, got {packet_id}"
            )));
        }
        let token = response.read_i64_be()?;
        if token != self.ping_token {
            return Err(RustmcError::InvalidResponse(format!(
                "mangled ping response: expected token {}, got {token}",
                self.ping_token
            )));
        }
        Ok(end.duration_since(start).as_secs_f64() * 1000.0)
    }
}

#[cfg(feature = "async")]
pub use async_impl::AsyncJavaClient;

#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use crate::protocol::io::AsyncTcpConnection;

    /// Async counterpart to `JavaClient`. Same packet framing/logic, just
    /// `.await`ed I/O — see the module docs above for the protocol shape.
    pub struct AsyncJavaClient {
        conn: AsyncTcpConnection,
        address: Address,
        version: i32,
        ping_token: i64,
    }

    impl AsyncJavaClient {
        pub fn new(conn: AsyncTcpConnection, address: Address, version: i32, ping_token: i64) -> Self {
            Self { conn, address, version, ping_token }
        }

        async fn read_packet(&mut self) -> Result<Buffer, RustmcError> {
            let len = self.read_varint_from_socket().await? as usize;
            if len > crate::protocol::io::buffer::MAX_DECLARED_LEN {
                return Err(RustmcError::InvalidResponse(format!(
                    "declared packet length {len} exceeds max {}",
                    crate::protocol::io::buffer::MAX_DECLARED_LEN
                )));
            }
            let data = self.conn.read_exact(len).await?;
            Ok(Buffer::from_vec(data))
        }

        async fn read_varint_from_socket(&mut self) -> Result<i32, RustmcError> {
            let mut result: i32 = 0;
            let mut position: u32 = 0;
            for i in 0..5 {
                let byte = self.conn.read_exact(1).await?[0];
                if i == 4 && (byte & 0x70) != 0 {
                    return Err(RustmcError::VarIntOverflow);
                }
                result |= ((byte & 0x7F) as i32) << position;
                position += 7;
                if (byte & 0x80) == 0 {
                    return Ok(result);
                }
            }
            Err(RustmcError::VarIntOverflow)
        }

        pub async fn handshake(&mut self) -> Result<(), RustmcError> {
            let mut packet = Buffer::new();
            packet.write_varint(0);
            packet.write_varint(self.version);
            packet.write_string(&self.address.host);
            packet.write_u16_be(self.address.port);
            packet.write_varint(1);
            self.conn.write_all(&packet.into_packet()).await
        }

        pub async fn read_status(&mut self) -> Result<(serde_json::Value, f64), RustmcError> {
            let mut request = Buffer::new();
            request.write_varint(0);
            self.conn.write_all(&request.into_packet()).await?;

            let start = Instant::now();
            let mut response = self.read_packet().await?;
            let end = Instant::now();

            let packet_id = response.read_varint()?;
            if packet_id != 0 {
                return Err(RustmcError::InvalidResponse(format!(
                    "expected status response packet id 0, got {packet_id}"
                )));
            }
            let json_str = response.read_string()?;
            let raw: serde_json::Value = serde_json::from_str(&json_str)?;
            let latency_ms = end.duration_since(start).as_secs_f64() * 1000.0;
            Ok((raw, latency_ms))
        }

        pub async fn test_ping(&mut self) -> Result<f64, RustmcError> {
            let mut request = Buffer::new();
            request.write_varint(1);
            request.write_i64_be(self.ping_token);

            let start = Instant::now();
            self.conn.write_all(&request.into_packet()).await?;
            let mut response = self.read_packet().await?;
            let end = Instant::now();

            let packet_id = response.read_varint()?;
            if packet_id != 1 {
                return Err(RustmcError::InvalidResponse(format!(
                    "expected ping response packet id 1, got {packet_id}"
                )));
            }
            let token = response.read_i64_be()?;
            if token != self.ping_token {
                return Err(RustmcError::InvalidResponse(format!(
                    "mangled ping response: expected token {}, got {token}",
                    self.ping_token
                )));
            }
            Ok(end.duration_since(start).as_secs_f64() * 1000.0)
        }
    }
}
