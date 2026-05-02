//! magnet: Parsing and handling of BitTorrent Magnet URIs.

use crate::{Error, Result};
use url::Url;

#[derive(Debug, Clone)]
pub struct Magnet {
    pub info_hash: [u8; 20],
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
                        if hash_hex.len() == 40 {
                            let mut hash = [0u8; 20];
                            for i in 0..20 {
                                hash[i] = u8::from_str_radix(&hash_hex[i * 2..i * 2 + 2], 16)
                                    .map_err(|_| {
                                        Error::Protocol("Invalid hex in info_hash".to_string())
                                    })?;
                            }
                            info_hash = Some(hash);
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

        let info_hash = info_hash
            .ok_or_else(|| Error::Protocol("Missing xt (info_hash) in magnet URI".to_string()))?;

        Ok(Self {
            info_hash,
            trackers,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_magnet() {
        let uri = "magnet:?xt=urn:btih:d2474436908143d52cdeee8d4c96510d3301cdc4&dn=Ubuntu&tr=http://tracker.com/announce";
        let magnet = Magnet::parse(uri).unwrap();

        assert_eq!(magnet.name, Some("Ubuntu".to_string()));
        assert_eq!(magnet.trackers.len(), 1);
        assert_eq!(magnet.trackers[0], "http://tracker.com/announce");
        assert_eq!(magnet.info_hash[0], 0xd2);
    }
}
