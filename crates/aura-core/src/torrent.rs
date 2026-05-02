//! torrent: Parsing and handling of .torrent (metainfo) files.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct File {
    pub length: u64,
    pub path: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: u64,
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,
    pub length: Option<u64>,
    pub files: Option<Vec<File>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<String>>>,
    pub comment: Option<String>,
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    #[serde(rename = "creation date")]
    pub creation_date: Option<u64>,
}

impl Torrent {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        serde_bencode::from_bytes(data)
            .map_err(|e| Error::Protocol(format!("Failed to parse torrent file: {}", e)))
    }

    pub fn info_hash(&self) -> Result<[u8; 20]> {
        let info_bytes = serde_bencode::to_bytes(&self.info)
            .map_err(|e| Error::Protocol(format!("Failed to bencode info: {}", e)))?;
        let mut hasher = Sha1::new();
        hasher.update(&info_bytes);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&hasher.finalize());
        Ok(hash)
    }

    pub fn total_length(&self) -> u64 {
        if let Some(len) = self.info.length {
            len
        } else if let Some(files) = &self.info.files {
            files.iter().map(|f| f.length).sum()
        } else {
            0
        }
    }

    pub fn pieces_count(&self) -> usize {
        self.info.pieces.len() / 20
    }

    pub fn piece_hash(&self, index: usize) -> Result<[u8; 20]> {
        let start = index * 20;
        if start + 20 > self.info.pieces.len() {
            return Err(Error::Protocol("Piece index out of range".to_string()));
        }
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&self.info.pieces[start..start + 20]);
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_parsing_simple() {
        let info = Info {
            name: "test.txt".to_string(),
            piece_length: 1024,
            pieces: vec![0; 20],
            length: Some(1024),
            files: None,
        };
        let torrent = Torrent {
            announce: "http://tracker.com/announce".to_string(),
            info,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
        };

        let encoded = serde_bencode::to_bytes(&torrent).unwrap();
        let decoded = Torrent::from_bytes(&encoded).unwrap();
        assert_eq!(decoded.announce, "http://tracker.com/announce");
        assert_eq!(decoded.info.name, "test.txt");
        assert_eq!(decoded.total_length(), 1024);

        let hash = decoded.info_hash().unwrap();
        assert_eq!(hash.len(), 20);
    }
}
