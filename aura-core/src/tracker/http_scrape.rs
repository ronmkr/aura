use super::TrackerClient;
use crate::torrent::Torrent;
use crate::{Error, Result};
use url::Url;

impl TrackerClient {
    pub(crate) async fn scrape_http(
        &self,
        url_str: &str,
        torrent: &Torrent,
    ) -> Result<(u32, u32, u32)> {
        let info_hash = if let Some(h2) = torrent.info_hash_v2()? {
            let mut truncated = [0u8; 20];
            truncated.copy_from_slice(&h2[..20]);
            truncated
        } else {
            torrent
                .info_hash_v1()?
                .ok_or_else(|| Error::Protocol("No info hash available".to_string()))?
        };

        let info_hash_encoded: String = info_hash.iter().map(|b| format!("%{:02x}", b)).collect();

        let scrape_url_str = if let Some(s) = get_scrape_url(url_str) {
            s
        } else {
            return Err(Error::Protocol(format!(
                "Cannot derive scrape URL from announce URL: {}",
                url_str
            )));
        };

        let url = Url::parse(&scrape_url_str)
            .map_err(|e| Error::Protocol(format!("Invalid scrape URL: {}", e)))?;

        let final_url = if url.query().is_some() {
            format!("{}&info_hash={}", scrape_url_str, info_hash_encoded)
        } else {
            format!("{}?info_hash={}", scrape_url_str, info_hash_encoded)
        };

        let bytes = self
            .client
            .get(&final_url)
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Scrape request failed: {}", e)))?
            .bytes()
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read scrape response: {}", e)))?;

        let res_val: serde_bencode::value::Value = serde_bencode::from_bytes(&bytes)
            .map_err(|e| Error::Protocol(format!("Failed to bdecode scrape response: {}", e)))?;

        if let serde_bencode::value::Value::Dict(dict) = res_val {
            if let Some(serde_bencode::value::Value::Bytes(reason)) =
                dict.get(b"failure reason".as_slice())
            {
                let reason_str = String::from_utf8_lossy(reason).to_string();
                return Err(Error::Protocol(format!(
                    "Tracker reported scrape failure: {}",
                    reason_str
                )));
            }

            if let Some(serde_bencode::value::Value::Dict(files)) = dict.get(b"files".as_slice()) {
                if let Some(serde_bencode::value::Value::Dict(stats)) = files.get(&info_hash[..]) {
                    let complete = match stats.get(b"complete".as_slice()) {
                        Some(serde_bencode::value::Value::Int(c)) => *c as u32,
                        _ => 0,
                    };
                    let incomplete = match stats.get(b"incomplete".as_slice()) {
                        Some(serde_bencode::value::Value::Int(i)) => *i as u32,
                        _ => 0,
                    };
                    let downloaded = match stats.get(b"downloaded".as_slice()) {
                        Some(serde_bencode::value::Value::Int(d)) => *d as u32,
                        _ => 0,
                    };
                    return Ok((complete, incomplete, downloaded));
                }
            }
        }

        Err(Error::Protocol(
            "Invalid scrape response format".to_string(),
        ))
    }
}

pub(crate) fn get_scrape_url(announce_url: &str) -> Option<String> {
    if let Ok(mut url) = Url::parse(announce_url) {
        let path = url.path();
        if let Some(pos) = path.rfind('/') {
            let last_part = &path[pos + 1..];
            if last_part == "announce" {
                let mut new_path = path[..pos + 1].to_string();
                new_path.push_str("scrape");
                url.set_path(&new_path);
                return Some(url.to_string());
            }
        }
    }
    None
}
