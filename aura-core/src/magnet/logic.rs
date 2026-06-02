//! magnet: Parsing and handling of BitTorrent Magnet URIs.

use crate::{Error, InfoHash, Result};
use url::Url;

#[derive(Debug, Clone)]
pub struct Magnet {
    pub info_hash: InfoHash,
    pub trackers: Vec<String>,
    pub name: Option<String>,
}

impl Magnet {
    pub fn parse(uri: &str) -> Result<Self> {
        let url =
            Url::parse(uri).map_err(|e| Error::Protocol(format!("Invalid Magnet URI: {}", e)))?;

        if url.scheme() != "magnet" {
            return Err(Error::Protocol("Not a magnet URI".to_string()));
        }

        let mut info_hash = None;
        let mut trackers = Vec::new();
        let mut name = None;

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "xt" => {
                    if let Some(hash_hex) = value.strip_prefix("urn:btih:") {
                        if let Ok(h) = hex::decode(hash_hex) {
                            if h.len() == 20 {
                                let mut hash = [0u8; 20];
                                hash.copy_from_slice(&h);
                                info_hash = Some(InfoHash::V1(hash));
                            }
                        }
                    } else if let Some(hash_hex) = value.strip_prefix("urn:btmh:1220") {
                        // Multi-hash for SHA-256 is 1220 (0x12: sha256, 0x20: length 32)
                        if let Ok(h) = hex::decode(hash_hex) {
                            if h.len() == 32 {
                                let mut hash = [0u8; 32];
                                hash.copy_from_slice(&h);
                                info_hash = Some(InfoHash::V2(hash));
                            }
                        }
                    }
                }
                "tr" => {
                    trackers.push(value.into_owned());
                }
                "dn" => {
                    name = Some(value.into_owned());
                }
                _ => {}
            }
        }

        let info_hash = info_hash.ok_or_else(|| {
            Error::Protocol("Missing or unsupported xt in magnet URI".to_string())
        })?;

        Ok(Self {
            info_hash,
            trackers,
            name,
        })
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
