//! torrent: Parsing and handling of .torrent (metainfo) files.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;

pub use super::metadata::{File, Info, V2File};

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
                files
                    .iter()
                    .map(|f: &crate::torrent::V2File| {
                        if f.length == 0 {
                            0
                        } else {
                            (f.length as usize).div_ceil(piece_len)
                        }
                    })
                    .sum()
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

    pub fn piece_hash_v2(&self, index: usize, db: Option<&sled::Db>) -> Result<[u8; 32]> {
        if self.info.meta_version != Some(2) {
            return Err(Error::Protocol("Not a v2 torrent".to_string()));
        }

        let piece_len = self.info.piece_length as usize;
        let files = self
            .flatten_v2_files()
            .ok_or_else(|| Error::Protocol("No v2 files found".to_string()))?;

        let mut current_piece_offset = 0;
        for file in files {
            let file_pieces = if file.length == 0 {
                0
            } else {
                (file.length as usize).div_ceil(piece_len)
            };

            if index >= current_piece_offset && index < current_piece_offset + file_pieces {
                let file_piece_idx = index - current_piece_offset;

                let root = file
                    .pieces_root
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("Missing pieces root".to_string()))?;
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
                    let layer_bytes = if let Some(db) = db {
                        if let Some(bytes) = db
                            .get(root.as_slice())
                            .map_err(|e| Error::Storage(e.to_string()))?
                        {
                            bytes.to_vec()
                        } else {
                            return Err(Error::Protocol(
                                "Missing piece layer in DB for file".to_string(),
                            ));
                        }
                    } else {
                        // Fallback to in-memory piece_layers
                        let layers = self
                            .piece_layers
                            .as_ref()
                            .ok_or_else(|| Error::Protocol("Missing piece layers".to_string()))?;
                        if let serde_bencode::value::Value::Dict(dict) = layers {
                            let layer = dict.get(root.as_slice()).ok_or_else(|| {
                                Error::Protocol("Missing piece layer for file".to_string())
                            })?;
                            if let serde_bencode::value::Value::Bytes(layer_bytes) = layer {
                                layer_bytes.clone()
                            } else {
                                return Err(Error::Protocol(
                                    "Invalid piece layers format".to_string(),
                                ));
                            }
                        } else {
                            return Err(Error::Protocol("Invalid piece layers format".to_string()));
                        }
                    };

                    let start = file_piece_idx * 32;
                    if start + 32 > layer_bytes.len() {
                        return Err(Error::Protocol("Piece layer too short".to_string()));
                    }
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&layer_bytes[start..start + 32]);
                    return Ok(hash);
                }
            }

            current_piece_offset += file_pieces;
        }

        Err(Error::Protocol(
            "Piece index out of range for v2".to_string(),
        ))
    }

    pub fn piece_align_offset(&self, index: usize) -> Result<u64> {
        if self.info.meta_version != Some(2) {
            return Ok((index as u64) * self.info.piece_length);
        }

        let piece_len = self.info.piece_length;
        let files = self
            .flatten_v2_files()
            .ok_or_else(|| Error::Protocol("No v2 files found".to_string()))?;

        let mut current_piece_offset = 0;
        let mut byte_offset = 0;

        for file in files {
            let file_pieces = if file.length == 0 {
                0
            } else {
                (file.length as usize).div_ceil(piece_len as usize)
            };

            if index >= current_piece_offset && index < current_piece_offset + file_pieces {
                let file_piece_idx = index - current_piece_offset;
                return Ok(byte_offset + (file_piece_idx as u64 * piece_len));
            }

            current_piece_offset += file_pieces;
            byte_offset += (file_pieces as u64) * piece_len;
        }

        Err(Error::Protocol(
            "Piece index out of range for v2 alignment".to_string(),
        ))
    }

    pub fn piece_actual_length(&self, index: usize) -> Result<u64> {
        let piece_len = self.info.piece_length;

        if self.info.meta_version != Some(2) {
            let total = self.total_length();
            let piece_start = index as u64 * piece_len;
            if piece_start >= total {
                return Err(Error::Protocol("Piece index out of range".to_string()));
            }
            return Ok(std::cmp::min(piece_len, total - piece_start));
        }

        let files = self
            .flatten_v2_files()
            .ok_or_else(|| Error::Protocol("No v2 files found".to_string()))?;

        let mut current_piece_offset = 0;
        for file in files {
            let file_pieces = if file.length == 0 {
                0
            } else {
                (file.length as usize).div_ceil(piece_len as usize)
            };

            if index >= current_piece_offset && index < current_piece_offset + file_pieces {
                let file_piece_idx = index - current_piece_offset;

                // If it's the last piece of the file, its actual length might be smaller
                if file_piece_idx == file_pieces - 1 {
                    let remainder = file.length % piece_len;
                    if remainder == 0 {
                        return Ok(piece_len);
                    } else {
                        return Ok(remainder);
                    }
                } else {
                    return Ok(piece_len);
                }
            }

            current_piece_offset += file_pieces;
        }

        Err(Error::Protocol(
            "Piece index out of range for v2".to_string(),
        ))
    }
}
