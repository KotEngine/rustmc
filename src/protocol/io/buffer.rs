//! In-memory read/write buffer used by all wire protocols in this crate.

use std::io::{Cursor, Read, Write};

use crate::error::RustmcError;

/// Upper bound on any single "declared" size parsed from a server response
/// (string length, raw byte length, etc) *before* that value is used for
/// allocation. Any server — malicious or simply buggy — can send a VarInt
/// length up to `i32::MAX` (~2.1 GiB); without this cap a single such packet
/// would make the client try to allocate gigabytes of memory for a couple of
/// real bytes on the wire. 1 MiB comfortably covers any real Java SLP JSON
/// response, favicon included.
pub const MAX_DECLARED_LEN: usize = 1024 * 1024;

/// Growable read/write buffer with Minecraft's wire primitives (VarInt,
/// length-prefixed strings, null-terminated ASCII, big-endian integers).
pub struct Buffer {
    cursor: Cursor<Vec<u8>>,
}

impl Buffer {
    /// New, empty buffer (used for building outgoing packets).
    pub fn new() -> Self {
        Self { cursor: Cursor::new(Vec::new()) }
    }

    /// Wrap already-received bytes for reading.
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self { cursor: Cursor::new(data) }
    }

    /// Bytes not yet read.
    pub fn remaining(&self) -> usize {
        (self.cursor.get_ref().len() as u64).saturating_sub(self.cursor.position()) as usize
    }

    /// Current read position (bytes already consumed).
    pub fn position(&self) -> usize {
        self.cursor.position() as usize
    }

    /// Seeks the read position. Used by protocols (e.g. Query) that need
    /// to scan ahead for a marker and then jump past it, rather than
    /// consuming byte-by-byte.
    pub fn set_position(&mut self, pos: usize) {
        self.cursor.set_position(pos as u64);
    }

    /// Full underlying buffer, including already-read bytes. Used for
    /// scanning ahead (e.g. Query's hostname/MOTD field, which has no
    /// reliable length prefix or terminator of its own).
    pub fn as_slice(&self) -> &[u8] {
        self.cursor.get_ref()
    }

    // ---- primitives ----

    pub fn write_u8(&mut self, val: u8) {
        let _ = self.cursor.write_all(&[val]);
    }

    pub fn read_u8(&mut self) -> Result<u8, RustmcError> {
        let mut buf = [0u8; 1];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(buf[0])
    }

    pub fn write_u16_be(&mut self, val: u16) {
        let _ = self.cursor.write_all(&val.to_be_bytes());
    }

    pub fn read_u16_be(&mut self) -> Result<u16, RustmcError> {
        let mut buf = [0u8; 2];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(u16::from_be_bytes(buf))
    }

    /// Little-endian u16. Only the Query protocol's basic-stat `hostport`
    /// field uses little-endian on the wire; everything else in this
    /// crate is big-endian.
    pub fn write_u16_le(&mut self, val: u16) {
        let _ = self.cursor.write_all(&val.to_le_bytes());
    }

    pub fn read_u16_le(&mut self) -> Result<u16, RustmcError> {
        let mut buf = [0u8; 2];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn write_u32_be(&mut self, val: u32) {
        let _ = self.cursor.write_all(&val.to_be_bytes());
    }

    pub fn read_u32_be(&mut self) -> Result<u32, RustmcError> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(u32::from_be_bytes(buf))
    }

    pub fn write_i32_be(&mut self, val: i32) {
        let _ = self.cursor.write_all(&val.to_be_bytes());
    }

    pub fn read_i32_be(&mut self) -> Result<i32, RustmcError> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(i32::from_be_bytes(buf))
    }

    pub fn write_i64_be(&mut self, val: i64) {
        let _ = self.cursor.write_all(&val.to_be_bytes());
    }

    pub fn read_i64_be(&mut self) -> Result<i64, RustmcError> {
        let mut buf = [0u8; 8];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(i64::from_be_bytes(buf))
    }

    // ---- raw bytes ----

    pub fn write_bytes(&mut self, data: &[u8]) {
        let _ = self.cursor.write_all(data);
    }

    /// Reads exactly `n` bytes. `n` must already be validated by the caller
    /// against `MAX_DECLARED_LEN` if it came from untrusted input (see
    /// `read_string`) — this function additionally refuses to allocate past
    /// `remaining()`/`MAX_DECLARED_LEN` on its own, so a stray direct call
    /// can't be used to bypass the check.
    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, RustmcError> {
        if n > MAX_DECLARED_LEN {
            return Err(RustmcError::InvalidResponse(format!(
                "declared length {n} exceeds max {MAX_DECLARED_LEN}"
            )));
        }
        if n > self.remaining() {
            return Err(RustmcError::InvalidResponse(format!(
                "requested {n} bytes, only {} remaining",
                self.remaining()
            )));
        }
        let mut buf = vec![0u8; n];
        self.cursor.read_exact(&mut buf).map_err(RustmcError::Io)?;
        Ok(buf)
    }

    // ---- VarInt ----

    pub fn write_varint(&mut self, val: i32) {
        let mut val = val as u32;
        loop {
            if val & !0x7F == 0 {
                self.write_u8(val as u8);
                return;
            }
            self.write_u8(((val & 0x7F) | 0x80) as u8);
            val >>= 7;
        }
    }

    /// Reads a signed 32-bit VarInt (max 5 bytes). The 5th byte may only
    /// carry the low 4 of its 7 payload bits — anything else means the
    /// value would overflow `i32`.
    pub fn read_varint(&mut self) -> Result<i32, RustmcError> {
        let mut result: i32 = 0;
        let mut position: u32 = 0;

        for i in 0..5 {
            let byte = self.read_u8()?;

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

    // ---- length-prefixed UTF-8 string ----

    pub fn write_string(&mut self, s: &str) {
        self.write_varint(s.len() as i32);
        self.write_bytes(s.as_bytes());
    }

    /// Reads a VarInt length, validates it is non-negative and within
    /// bounds (`MAX_DECLARED_LEN` and `remaining()`) *before* allocating,
    /// then reads and UTF-8-decodes the payload.
    pub fn read_string(&mut self) -> Result<String, RustmcError> {
        let len = self.read_varint()?;
        if len < 0 {
            return Err(RustmcError::InvalidResponse(format!(
                "negative string length: {len}"
            )));
        }
        let bytes = self.read_bytes(len as usize)?;
        String::from_utf8(bytes)
            .map_err(|e| RustmcError::InvalidResponse(format!("invalid UTF-8 string: {e}")))
    }

    // ---- null-terminated ISO-8859-1 ASCII string (Query protocol) ----

    /// Scans forward for a `0x00` terminator. There is no declared-length
    /// field here (unlike `read_string`) so `MAX_DECLARED_LEN` doesn't
    /// apply literally — the scan is naturally bounded by `remaining()`,
    /// and a missing terminator is an error rather than a panic.
    pub fn read_ascii_null(&mut self) -> Result<String, RustmcError> {
        let start = self.cursor.position() as usize;
        let data = self.cursor.get_ref();
        let end = data.len();
        let mut i = start;
        while i < end && data[i] != 0 {
            i += 1;
        }
        if i >= end {
            return Err(RustmcError::InvalidResponse(
                "missing NUL terminator in ASCII string".into(),
            ));
        }
        let s: String = data[start..i].iter().map(|&b| b as char).collect();
        self.cursor.set_position((i + 1) as u64);
        Ok(s)
    }

    // ---- packet framing ----

    /// Prefixes the buffer's content with its VarInt length, per Minecraft
    /// packet framing (`VarInt(len) + payload`).
    pub fn into_packet(self) -> Vec<u8> {
        let payload = self.cursor.into_inner();
        let mut out = Buffer::new();
        out.write_varint(payload.len() as i32);
        out.write_bytes(&payload);
        out.cursor.into_inner()
    }

    /// Raw bytes with no length-prefix framing. UDP-based protocols
    /// (Bedrock, Query) don't use Java's VarInt-length packet framing —
    /// the whole datagram *is* the packet.
    pub fn into_packet_unframed(self) -> Vec<u8> {
        self.cursor.into_inner()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
