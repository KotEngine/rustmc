//! GameSpy4 Query protocol, synchronous client.
//!
//! Two-step exchange over UDP: `handshake()` gets a numeric challenge
//! token from the server (defends against basic UDP spoofing/amplification
//! abuse), then a stat request echoes that token back to get the actual
//! data. Full stat includes players/plugins/motd; basic stat is a smaller
//! fixed-format response some servers expose without full stat enabled.

use std::time::Instant;

use crate::error::RustmcError;
use crate::protocol::io::{Buffer, UdpConnection};
use crate::response::query::{QueryBasicResponse, QueryResponse};

const MAGIC: [u8; 2] = [0xFE, 0xFD];
const TYPE_HANDSHAKE: u8 = 9;
const TYPE_STAT: u8 = 0;

pub struct QueryClient {
    conn: UdpConnection,
    challenge: i32,
}

impl QueryClient {
    pub fn new(conn: UdpConnection) -> Self {
        Self { conn, challenge: 0 }
    }

    /// Session ID: Minecraft's query implementation only respects the
    /// lower 4 bits of each byte (masking with `0x0F0F0F0F`), matching
    /// `mcstatus`. A fresh one is generated per packet — the server
    /// doesn't require it to match between handshake and stat request,
    /// it's only echoed back for the client to correlate replies.
    fn session_id() -> u32 {
        use std::collections::hash_map::RandomState;
        use std::hash::BuildHasher;
        (RandomState::new().hash_one(Instant::now()) as u32) & 0x0F0F0F0F
    }

    /// Performs the handshake, storing the challenge token for subsequent
    /// stat requests.
    pub fn handshake(&mut self) -> Result<(), RustmcError> {
        let mut packet = Buffer::new();
        packet.write_bytes(&MAGIC);
        packet.write_u8(TYPE_HANDSHAKE);
        packet.write_u32_be(Self::session_id());
        self.conn.send(&packet.into_packet_unframed())?;

        let mut response = self.read_response()?;
        let challenge_str = response.read_ascii_null()?;
        self.challenge = challenge_str
            .trim()
            .parse::<i32>()
            .map_err(|_| RustmcError::InvalidResponse(format!("bad challenge token: {challenge_str:?}")))?;
        Ok(())
    }

    fn read_response(&self) -> Result<Buffer, RustmcError> {
        let data = self.conn.recv()?;
        let mut buf = Buffer::from_vec(data);
        // type (1 byte) + session id (4 bytes), unused past validating length
        let _ = buf.read_bytes(1 + 4)?;
        Ok(buf)
    }

    /// Full stat: player list, plugins, MOTD, map, everything.
    pub fn full_stat(&mut self) -> Result<QueryResponse, RustmcError> {
        let mut packet = Buffer::new();
        packet.write_bytes(&MAGIC);
        packet.write_u8(TYPE_STAT);
        packet.write_u32_be(Self::session_id());
        packet.write_i32_be(self.challenge);
        packet.write_bytes(&[0, 0, 0, 0]); // padding: signals "full stat" to the server
        self.conn.send(&packet.into_packet_unframed())?;

        let response = self.read_response()?;
        parse_full_stat(response)
    }

    /// Basic stat: a smaller fixed-format response (motd, gametype, map,
    /// online/max, port, address) without the player list or plugins.
    /// Some servers expose this even with full query disabled.
    pub fn basic_stat(&mut self) -> Result<QueryBasicResponse, RustmcError> {
        let mut packet = Buffer::new();
        packet.write_bytes(&MAGIC);
        packet.write_u8(TYPE_STAT);
        packet.write_u32_be(Self::session_id());
        packet.write_i32_be(self.challenge);
        // no padding: this is what distinguishes a basic stat request
        self.conn.send(&packet.into_packet_unframed())?;

        let mut response = self.read_response()?;
        Ok(QueryBasicResponse {
            motd: crate::motd::Motd::parse(&response.read_ascii_null()?, false),
            game_type: response.read_ascii_null()?,
            map: response.read_ascii_null()?,
            online: response.read_ascii_null()?.trim().parse().unwrap_or(0),
            max: response.read_ascii_null()?.trim().parse().unwrap_or(0),
            port: response.read_u16_le()?,
            host_ip: response.read_ascii_null()?,
        })
    }
}

/// Known key names that can immediately follow the `hostname` (MOTD) value
/// in a full-stat key/value section. The MOTD value has no reliable length
/// prefix or guaranteed-unique terminator of its own (some proxied/modified
/// server implementations emit MOTDs containing stray `NUL`-adjacent
/// bytes), so — matching `mcstatus` — we scan forward for whichever of
/// these known keys appears first and treat everything before it as the
/// MOTD. Searching only for one fixed key (e.g. just `hostip`) is not
/// enough: server implementations don't all emit keys in the same order.
const KV_KEYS_AFTER_HOSTNAME: &[&[u8]] = &[
    b"\x00hostip\x00",
    b"\x00hostport\x00",
    b"\x00game_id\x00",
    b"\x00gametype\x00",
    b"\x00map\x00",
    b"\x00maxplayers\x00",
    b"\x00numplayers\x00",
    b"\x00plugins\x00",
    b"\x00version\x00",
];

fn parse_full_stat(mut response: Buffer) -> Result<QueryResponse, RustmcError> {
    // Fixed 11-byte "splitnum\x00\x80\x00" padding preceding the K/V section.
    let _ = response.read_bytes("splitnum".len() + 3)?;

    let mut data: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    loop {
        let key = response.read_ascii_null()?;
        if key == "hostname" {
            let start = response.position();
            let slice = response.as_slice();
            let mut end: Option<usize> = None;
            for marker in KV_KEYS_AFTER_HOSTNAME {
                if let Some(rel) = find_subslice(&slice[start..], marker) {
                    let candidate = start + rel;
                    end = Some(end.map_or(candidate, |e: usize| e.min(candidate)));
                }
            }
            let end = end.ok_or_else(|| {
                RustmcError::InvalidResponse(
                    "could not locate end of 'hostname' (MOTD) field in query response: \
                     none of the expected following keys were found"
                        .into(),
                )
            })?;
            let motd_bytes = &slice[start..end];
            let motd = motd_bytes.iter().map(|&b| b as char).collect::<String>();
            data.insert(key, motd);
            // end points at the leading \x00 of the next key marker; skip it.
            response.set_position(end + 1);
        } else if key.is_empty() {
            let _ = response.read_bytes(1)?; // trailing NUL of the K/V terminator
            break;
        } else {
            let value = response.read_ascii_null()?;
            data.insert(key, value);
        }
    }

    // Fixed 10-byte "player_\x00\x00\x01" padding preceding the player list.
    let _ = response.read_bytes("player_".len() + 2)?;

    let mut players = Vec::new();
    loop {
        let name = response.read_ascii_null()?;
        if name.is_empty() {
            break;
        }
        players.push(name);
    }

    QueryResponse::build(data, players)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(feature = "async")]
pub use async_impl::AsyncQueryClient;

#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use crate::protocol::io::AsyncUdpConnection;

    pub struct AsyncQueryClient {
        conn: AsyncUdpConnection,
        challenge: i32,
    }

    impl AsyncQueryClient {
        pub fn new(conn: AsyncUdpConnection) -> Self {
            Self { conn, challenge: 0 }
        }

        async fn read_response(&self) -> Result<Buffer, RustmcError> {
            let data = self.conn.recv().await?;
            let mut buf = Buffer::from_vec(data);
            let _ = buf.read_bytes(1 + 4)?;
            Ok(buf)
        }

        pub async fn handshake(&mut self) -> Result<(), RustmcError> {
            let mut packet = Buffer::new();
            packet.write_bytes(&MAGIC);
            packet.write_u8(TYPE_HANDSHAKE);
            packet.write_u32_be(QueryClient::session_id());
            self.conn.send(&packet.into_packet_unframed()).await?;

            let mut response = self.read_response().await?;
            let challenge_str = response.read_ascii_null()?;
            self.challenge = challenge_str.trim().parse::<i32>().map_err(|_| {
                RustmcError::InvalidResponse(format!("bad challenge token: {challenge_str:?}"))
            })?;
            Ok(())
        }

        pub async fn full_stat(&mut self) -> Result<QueryResponse, RustmcError> {
            let mut packet = Buffer::new();
            packet.write_bytes(&MAGIC);
            packet.write_u8(TYPE_STAT);
            packet.write_u32_be(QueryClient::session_id());
            packet.write_i32_be(self.challenge);
            packet.write_bytes(&[0, 0, 0, 0]);
            self.conn.send(&packet.into_packet_unframed()).await?;

            let response = self.read_response().await?;
            parse_full_stat(response)
        }

        pub async fn basic_stat(&mut self) -> Result<QueryBasicResponse, RustmcError> {
            let mut packet = Buffer::new();
            packet.write_bytes(&MAGIC);
            packet.write_u8(TYPE_STAT);
            packet.write_u32_be(QueryClient::session_id());
            packet.write_i32_be(self.challenge);
            self.conn.send(&packet.into_packet_unframed()).await?;

            let mut response = self.read_response().await?;
            Ok(QueryBasicResponse {
                motd: crate::motd::Motd::parse(&response.read_ascii_null()?, false),
                game_type: response.read_ascii_null()?,
                map: response.read_ascii_null()?,
                online: response.read_ascii_null()?.trim().parse().unwrap_or(0),
                max: response.read_ascii_null()?.trim().parse().unwrap_or(0),
                port: response.read_u16_le()?,
                host_ip: response.read_ascii_null()?,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic full-stat payload (post session-id-skip) with a
    /// MOTD containing a `§` color code, to make sure the hostname/MOTD
    /// boundary search doesn't get confused by extra bytes in the value
    /// and correctly finds the *next key*, not just any occurrence of a
    /// substring.
    #[test]
    fn parses_full_stat_kv_section_with_colored_motd() {
        let mut raw = Vec::new();
        raw.extend_from_slice(b"splitnum\x00\x80\x00");
        raw.extend_from_slice(b"hostname\x00A \xa7cServer\x00");
        raw.extend_from_slice(b"gametype\x00SMP\x00");
        raw.extend_from_slice(b"game_id\x00MINECRAFT\x00");
        raw.extend_from_slice(b"version\x001.20.1\x00");
        raw.extend_from_slice(b"plugins\x00\x00");
        raw.extend_from_slice(b"map\x00world\x00");
        raw.extend_from_slice(b"numplayers\x003\x00");
        raw.extend_from_slice(b"maxplayers\x0020\x00");
        raw.extend_from_slice(b"hostport\x0025565\x00");
        raw.extend_from_slice(b"hostip\x00127.0.0.1\x00");
        raw.extend_from_slice(b"\x00\x00"); // empty key + trailing NUL terminator
        raw.extend_from_slice(b"player_\x00\x00"); // 9-byte padding, matches the 9-byte skip below
        raw.extend_from_slice(b"Alice\x00Bob\x00\x00");
        let buf = Buffer::from_vec(raw);

        let resp = parse_full_stat(buf).unwrap();
        assert_eq!(resp.map_name, "world");
        assert_eq!(resp.players.online, 3);
        assert_eq!(resp.players.max, 20);
        assert_eq!(resp.players.list, vec!["Alice".to_owned(), "Bob".to_owned()]);
        assert_eq!(resp.ip, "127.0.0.1");
        assert_eq!(resp.port, 25565);
        assert_eq!(resp.software.version, "1.20.1");
        assert_eq!(resp.motd.to_plain(), "A Server");
    }

    /// Regression test lifted from mcstatus's
    /// `test_query_handles_unorderd_map_response`: real GeyserMC servers
    /// send K/V pairs in a *different order* than vanilla (hostip right
    /// after hostname, not near the end) — this is exactly the case the
    /// multi-key boundary search (`KV_KEYS_AFTER_HOSTNAME`) exists for.
    /// Searching only for a single fixed next-key (e.g. just `hostip`)
    /// happens to work for vanilla's key order but breaks here.
    #[test]
    fn parses_full_stat_with_geyser_key_order() {
        let raw: &[u8] = b"\x00\x00\x00\x00\x00GeyserMC\x00\x80\x00hostname\x00Geyser\x00hostip\x001.1.1.1\x00plugins\x00\x00numplayers\
\x001\x00gametype\x00SMP\x00maxplayers\x00100\x00hostport\x0019132\x00version\x00Geyser (git-master-0fd903e) 1.18.10\x00map\x00Geyser\x00game_id\x00MINECRAFT\x00\x00\x01player_\x00\x00\x00";
        // First 5 bytes are type+session_id, stripped by read_response()
        // in real use; parse_full_stat expects to start right after that.
        let buf = Buffer::from_vec(raw[5..].to_vec());
        let resp = parse_full_stat(buf).unwrap();
        assert_eq!(resp.game_id, "MINECRAFT");
        assert_eq!(resp.motd.to_plain(), "Geyser");
        assert_eq!(resp.software.version, "Geyser (git-master-0fd903e) 1.18.10");
        assert_eq!(resp.ip, "1.1.1.1");
    }

    /// Regression test lifted from mcstatus's
    /// `test_query_handles_unicode_motd_with_nulls`: the MOTD value can
    /// itself contain a literal `\x00` byte in the middle — the boundary
    /// search must not stop at *any* `\x00`, only at one immediately
    /// followed by a recognized key name. `0xD5` is Latin-1 `Õ` when
    /// decoded byte-for-byte, matching the Query protocol's ISO-8859-1
    /// (not UTF-8) MOTD encoding.
    #[test]
    fn parses_motd_containing_embedded_null_byte() {
        let raw: &[u8] = b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00hostname\x00\x00*K\xd5\x00gametype\x00SMP\
\x00game_id\x00MINECRAFT\x00version\x001.16.5\x00plugins\x00Paper on 1.16.5-R0.1-SNAPSHOT\x00map\x00world\
\x00numplayers\x000\x00maxplayers\x0020\x00hostport\x0025565\x00hostip\x00127.0.1.1\x00\x00\x01player_\x00\x00\x00";
        let buf = Buffer::from_vec(raw[5..].to_vec());
        let resp = parse_full_stat(buf).unwrap();
        assert_eq!(resp.game_id, "MINECRAFT");
        // "\x00*K" + Latin-1 0xD5 ('Õ')
        assert_eq!(resp.motd.to_plain(), "\u{0000}*K\u{00D5}");
    }

    #[test]
    fn find_subslice_locates_marker() {
        assert_eq!(find_subslice(b"abcXdefXghi", b"Xdef"), Some(3));
        assert_eq!(find_subslice(b"abc", b"Xdef"), None);
    }
}
