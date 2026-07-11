//! Forge mod data attached to a Java status response.
//!
//! **Known limitation:** Forge servers on 1.18.1+ compress this data into a
//! binary blob packed inside a UTF-16 string (see upstream `mcstatus`'s
//! `responses/forge.py` for the full decoder). That binary format is not
//! implemented here yet — only the pre-1.18.1 plain-JSON `forgeData`/
//! `modinfo` shape is parsed. Modern compressed Forge data will currently
//! result in `forge_data: None` rather than an error.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeData {
    pub mods: Vec<ForgeMod>,
    pub channels: Vec<ForgeChannel>,
    pub fml_network_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMod {
    pub modid: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeChannel {
    pub name: String,
    pub version: String,
    pub required: bool,
}

impl ForgeData {
    /// Attempts to parse the legacy (pre-1.18.1) plain-JSON `forgeData` or
    /// `modinfo` object. Returns `None` if the shape doesn't match (this
    /// includes the modern compressed-binary format, see module docs).
    pub fn try_parse(raw: &Value) -> Option<Self> {
        let mods: Vec<ForgeMod> = raw
            .get("modList")
            .or_else(|| raw.get("mods"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some(ForgeMod {
                            modid: m.get("modid")?.as_str()?.to_owned(),
                            version: m.get("version")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let channels: Vec<ForgeChannel> = raw
            .get("channels")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        Some(ForgeChannel {
                            name: c.get("res")?.as_str()?.to_owned(),
                            version: c.get("version")?.as_str()?.to_owned(),
                            required: c.get("required")?.as_bool()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let fml_network_version = raw
            .get("fmlNetworkVersion")
            .and_then(Value::as_u64)
            .map(|v| v as u32);

        if mods.is_empty() && channels.is_empty() && fml_network_version.is_none() {
            return None;
        }
        Some(Self { mods, channels, fml_network_version })
    }
}
