use super::super::logic::Torrent;
use crate::{Error, Result};

impl Torrent {
    pub fn get_piece_layer_index(&self) -> u32 {
        let piece_len = self.info.piece_length;
        (piece_len as f64 / 16384.0).log2() as u32
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
                        // 1. Try composite key (pieces_root + index)
                        let mut key = Vec::with_capacity(36);
                        key.extend_from_slice(root);
                        key.extend_from_slice(&self.get_piece_layer_index().to_be_bytes());

                        if let Some(bytes) =
                            db.get(&key).map_err(|e| Error::Storage(e.to_string()))?
                        {
                            bytes.to_vec()
                        } else if let Some(bytes) = db
                            .get(root.as_slice())
                            .map_err(|e| Error::Storage(e.to_string()))?
                        {
                            // 2. Fallback to legacy key (pieces_root only)
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

    pub fn block_hash_v2(
        &self,
        piece_index: usize,
        block_index_in_piece: usize,
        db: Option<&sled::Db>,
    ) -> Result<[u8; 32]> {
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

            if piece_index >= current_piece_offset
                && piece_index < current_piece_offset + file_pieces
            {
                let file_piece_idx = piece_index - current_piece_offset;
                let blocks_per_piece = piece_len / 16384;
                let file_block_idx = (file_piece_idx * blocks_per_piece) + block_index_in_piece;

                let root = file
                    .pieces_root
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("Missing pieces root".to_string()))?;

                // Look up layer 0 (leaves) in DB
                if let Some(db) = db {
                    let mut key = Vec::with_capacity(36);
                    key.extend_from_slice(root);
                    key.extend_from_slice(&0u32.to_be_bytes()); // Layer 0

                    if let Some(layer_bytes) =
                        db.get(key).map_err(|e| Error::Storage(e.to_string()))?
                    {
                        let start = file_block_idx * 32;
                        if start + 32 <= layer_bytes.len() {
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&layer_bytes[start..start + 32]);
                            return Ok(hash);
                        }
                    }
                }

                return Err(Error::Protocol("Block hash not found in DB".to_string()));
            }
            current_piece_offset += file_pieces;
        }

        Err(Error::Protocol("Piece index out of range".to_string()))
    }
}
