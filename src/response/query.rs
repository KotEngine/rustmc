//! GameSpy4 Query protocol response types.

use std::collections::HashMap;

use crate::error::RustmcError;
use crate::motd::Motd;

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub motd: Motd,
    pub map_name: String,
    pub players: QueryPlayers,
    pub software: QuerySoftware,
    pub ip: String,
    pub port: u16,
    pub game_type: String,
    pub game_id: String,
}

#[derive(Debug, Clone)]
pub struct QueryPlayers {
    pub online: u32,
    pub max: u32,
    pub list: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QuerySoftware {
    pub version: String,
    pub brand: String,
    pub plugins: Vec<String>,
}

impl QueryResponse {
    /// Builds a response from the flat key/value map parsed off the wire
    /// (see `protocol::query::parse_full_stat`) plus the separately parsed
    /// player list.
    pub fn build(
        mut data: HashMap<String, String>,
        players_list: Vec<String>,
    ) -> Result<Self, RustmcError> {
        fn take(data: &mut HashMap<String, String>, key: &str) -> Result<String, RustmcError> {
            data.remove(key)
                .ok_or_else(|| RustmcError::InvalidResponse(format!("query response missing '{key}'")))
        }

        let hostname = take(&mut data, "hostname")?;
        let map = take(&mut data, "map")?;
        let numplayers = take(&mut data, "numplayers")?;
        let maxplayers = take(&mut data, "maxplayers")?;
        let version = take(&mut data, "version")?;
        let plugins = data.remove("plugins").unwrap_or_default();
        let hostip = take(&mut data, "hostip")?;
        let hostport = take(&mut data, "hostport")?;
        let gametype = take(&mut data, "gametype")?;
        let game_id = data.remove("game_id").unwrap_or_else(|| "MINECRAFT".to_owned());

        let (brand, parsed_plugins) = parse_plugins(&plugins);

        Ok(Self {
            motd: Motd::parse(&hostname, false),
            map_name: map,
            players: QueryPlayers {
                online: numplayers.trim().parse().unwrap_or(0),
                max: maxplayers.trim().parse().unwrap_or(0),
                list: players_list,
            },
            software: QuerySoftware { version, brand, plugins: parsed_plugins },
            ip: hostip,
            port: hostport.trim().parse().unwrap_or(0),
            game_type: gametype,
            game_id,
        })
    }
}

/// Splits Query's `plugins` field (`"brand: plugin1; plugin2"`, or empty)
/// into a software brand and a plugin list.
fn parse_plugins(plugins: &str) -> (String, Vec<String>) {
    if plugins.is_empty() {
        return ("vanilla".to_owned(), Vec::new());
    }
    match plugins.split_once(':') {
        Some((brand, rest)) => (
            brand.trim().to_owned(),
            rest.split(';')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect(),
        ),
        None => (plugins.trim().to_owned(), Vec::new()),
    }
}

/// Response to a basic (non-full) stat request: no player list or
/// plugins, but cheaper for the server to answer.
#[derive(Debug, Clone)]
pub struct QueryBasicResponse {
    pub motd: Motd,
    pub game_type: String,
    pub map: String,
    pub online: u32,
    pub max: u32,
    pub port: u16,
    pub host_ip: String,
}
