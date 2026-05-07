//! torrent: Parsing and handling of .torrent (metainfo) files.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct File {
    pub length: u64,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2File {
    pub path: Vec<String>,
    pub length: u64,
    pub pieces_root: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: u64,
    #[serde(with = "serde_bytes", skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<u8>>,
    pub length: Option<u64>,
    pub files: Option<Vec<File>>,
    #[serde(rename = "meta version", skip_serializing_if = "Option::is_none")]
    pub meta_version: Option<u64>,
    #[serde(rename = "file tree", skip_serializing_if = "Option::is_none")]
    pub file_tree: Option<serde_bencode::value::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
    #[serde(rename = "announce-list", skip_serializing_if = "Option::is_none")]
    pub announce_list: Option<Vec<Vec<String>>>,
    pub comment: Option<String>,
    #[serde(rename = "created by", skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(rename = "creation date", skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<u64>,
    #[serde(rename = "piece layers", skip_serializing_if = "Option::is_none")]
    pub piece_layers: Option<serde_bencode::value::Value>,
}

impl Torrent {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        serde_bencode::from_bytes(data)
            .map_err(|e| Error::Protocol(format!("Failed to parse torrent file: {}", e)))
    }

    pub fn info_hash_v1(&self) -> Result<Option<[u8; 20]>> {
        if self.info.pieces.is_none() {
            return Ok(None);
        }
        let info_bytes = serde_bencode::to_bytes(&self.info)
            .map_err(|e| Error::Protocol(format!("Failed to bencode info: {}", e)))?;
        let mut hasher = Sha1::new();
        hasher.update(&info_bytes);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&hasher.finalize());
        Ok(Some(hash))
    }

    pub fn info_hash_v2(&self) -> Result<Option<[u8; 32]>> {
        if self.info.meta_version != Some(2) {
            return Ok(None);
        }
        let info_bytes = serde_bencode::to_bytes(&self.info)
            .map_err(|e| Error::Protocol(format!("Failed to bencode info: {}", e)))?;
        let mut hasher = Sha256::new();
        hasher.update(&info_bytes);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        Ok(Some(hash))
    }

    pub fn total_length(&self) -> u64 {
        if let Some(len) = self.info.length {
            len
        } else if let Some(files) = &self.info.files {
            files.iter().map(|f| f.length).sum()
        } else if let Some(v2_files) = self.flatten_v2_files() {
            v2_files.iter().map(|f| f.length).sum()
        } else {
            0
        }
    }

    pub fn flatten_v2_files(&self) -> Option<Vec<V2File>> {
        let tree_val = self.info.file_tree.as_ref()?;
        let mut result = Vec::new();
        Self::traverse_file_tree(tree_val, &mut Vec::new(), &mut result);
        Some(result)
    }

    fn traverse_file_tree(
        node: &serde_bencode::value::Value,
        current_path: &mut Vec<String>,
        result: &mut Vec<V2File>,
    ) {
        use serde_bencode::value::Value;

        if let Value::Dict(dict) = node {
            for (key_bytes, val) in dict {
                let key_str = String::from_utf8_lossy(key_bytes).to_string();
                if key_str.is_empty() {
                    // This node is a file. The val is a dict containing length and pieces root.
                    if let Value::Dict(props) = val {
                        let mut length = 0;
                        let mut pieces_root = None;

                        if let Some(Value::Int(l)) = props.get(b"length".as_slice()) {
                            length = *l as u64;
                        }
                        if let Some(Value::Bytes(r)) = props.get(b"pieces root".as_slice()) {
                            pieces_root = Some(r.clone());
                        }

                        result.push(V2File {
                            path: current_path.clone(),
                            length,
                            pieces_root,
                        });
                    }
                } else {
                    current_path.push(key_str);
                    Self::traverse_file_tree(val, current_path, result);
                    current_path.pop();
                }
            }
        }
    }

    pub fn pieces_count(&self) -> usize {
        if let Some(pieces) = &self.info.pieces {
            pieces.len() / 20
        } else if self.info.meta_version == Some(2) {
            let piece_len = self.info.piece_length as usize;
            if let Some(files) = self.flatten_v2_files() {
                files.iter().map(|f| {
                    if f.length == 0 { 0 } else { (f.length as usize).div_ceil(piece_len) }
                }).sum()
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn piece_hash_v1(&self, index: usize) -> Result<[u8; 20]> {
        let pieces = self
            .info
            .pieces
            .as_ref()
            .ok_or_else(|| Error::Protocol("No v1 pieces in torrent".to_string()))?;
        let start = index * 20;
        if start + 20 > pieces.len() {
            return Err(Error::Protocol("Piece index out of range".to_string()));
        }
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&pieces[start..start + 20]);
        Ok(hash)
    }

    pub fn piece_hash_v2(&self, index: usize) -> Result<[u8; 32]> {
        if self.info.meta_version != Some(2) {
            return Err(Error::Protocol("Not a v2 torrent".to_string()));
        }

        let piece_len = self.info.piece_length as usize;
        let files = self.flatten_v2_files().ok_or_else(|| Error::Protocol("No v2 files found".to_string()))?;

        let mut current_piece_offset = 0;
        for file in files {
            let file_pieces = if file.length == 0 {
                0
            } else {
                (file.length as usize).div_ceil(piece_len)
            };

            if index >= current_piece_offset && index < current_piece_offset + file_pieces {
                let file_piece_idx = index - current_piece_offset;
                
                let root = file.pieces_root.as_ref().ok_or_else(|| Error::Protocol("Missing pieces root".to_string()))?;
                if root.len() != 32 {
                    return Err(Error::Protocol("Invalid pieces root length".to_string()));
                }

                if file_pieces == 1 {
                    // For single-piece files, the root IS the piece hash
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(root);
                    return Ok(hash);
                } else {
                    // Look up in piece_layers
                    let layers = self.piece_layers.as_ref().ok_or_else(|| Error::Protocol("Missing piece layers".to_string()))?;
                    if let serde_bencode::value::Value::Dict(dict) = layers {
                        let layer = dict.get(root.as_slice()).ok_or_else(|| Error::Protocol("Missing piece layer for file".to_string()))?;
                        if let serde_bencode::value::Value::Bytes(layer_bytes) = layer {
                            let start = file_piece_idx * 32;
                            if start + 32 > layer_bytes.len() {
                                return Err(Error::Protocol("Piece layer too short".to_string()));
                            }
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&layer_bytes[start..start + 32]);
                            return Ok(hash);
                        }
                    }
                    return Err(Error::Protocol("Invalid piece layers format".to_string()));
                }
            }

            current_piece_offset += file_pieces;
        }

        Err(Error::Protocol("Piece index out of range for v2".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_torrent_serialization() {
        let info = Info {
            name: "test.txt".to_string(),
            piece_length: 1024,
            pieces: Some(vec![0; 20]),
            length: Some(1024),
            files: None,
            meta_version: None,
            file_tree: None,
        };
        let torrent = Torrent {
            announce: "http://tracker.com/announce".to_string(),
            info,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            piece_layers: None,
        };

        let encoded = serde_bencode::to_bytes(&torrent).unwrap();
        let decoded = Torrent::from_bytes(&encoded).unwrap();

        assert_eq!(decoded.announce, "http://tracker.com/announce");
        assert_eq!(decoded.info.name, "test.txt");
        assert_eq!(decoded.total_length(), 1024);

        let hash = decoded.info_hash_v1().unwrap().unwrap();
        assert_eq!(hash.len(), 20);
    }

    #[test]
    fn test_flatten_v2_files() {
        use serde_bencode::value::Value;
        use std::collections::HashMap;

        let mut file1_props = HashMap::new();
        file1_props.insert(b"length".to_vec(), Value::Int(100));
        file1_props.insert(b"pieces root".to_vec(), Value::Bytes(vec![1; 32]));

        let mut file1_entry = HashMap::new();
        file1_entry.insert(b"".to_vec(), Value::Dict(file1_props));

        let mut file2_props = HashMap::new();
        file2_props.insert(b"length".to_vec(), Value::Int(200));
        file2_props.insert(b"pieces root".to_vec(), Value::Bytes(vec![2; 32]));

        let mut file2_entry = HashMap::new();
        file2_entry.insert(b"".to_vec(), Value::Dict(file2_props));

        let mut dir2 = HashMap::new();
        dir2.insert(b"file2.txt".to_vec(), Value::Dict(file2_entry));

        let mut dir1 = HashMap::new();
        dir1.insert(b"file1.txt".to_vec(), Value::Dict(file1_entry));
        dir1.insert(b"dir2".to_vec(), Value::Dict(dir2));

        let mut file_tree = HashMap::new();
        file_tree.insert(b"dir1".to_vec(), Value::Dict(dir1));

        let info = Info {
            name: "test".to_string(),
            piece_length: 1024,
            pieces: None,
            length: None,
            files: None,
            meta_version: Some(2),
            file_tree: Some(Value::Dict(file_tree)),
        };

        let torrent = Torrent {
            announce: "http://tracker.com/announce".to_string(),
            info,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            piece_layers: None,
        };

        assert_eq!(torrent.total_length(), 300);

        let v2_files = torrent.flatten_v2_files().unwrap();
        assert_eq!(v2_files.len(), 2);

        let f1 = v2_files.iter().find(|f| f.path.last().unwrap() == "file1.txt").unwrap();
        assert_eq!(f1.path, vec!["dir1".to_string(), "file1.txt".to_string()]);
        assert_eq!(f1.length, 100);
        assert_eq!(f1.pieces_root.as_ref().unwrap(), &vec![1; 32]);

        let f2 = v2_files.iter().find(|f| f.path.last().unwrap() == "file2.txt").unwrap();
        assert_eq!(f2.path, vec!["dir1".to_string(), "dir2".to_string(), "file2.txt".to_string()]);
        assert_eq!(f2.length, 200);
        assert_eq!(f2.pieces_root.as_ref().unwrap(), &vec![2; 32]);
    }
}
