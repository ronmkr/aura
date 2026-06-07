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

    pub fn selected_total_length(&self, selected_files: &[bool]) -> u64 {
        if self.info.length.is_some() {
            return self.total_length();
        }

        let mut total = 0;
        if let Some(files) = &self.info.files {
            for (idx, file) in files.iter().enumerate() {
                if selected_files.get(idx).copied().unwrap_or(true) {
                    total += file.length;
                }
            }
        } else if let Some(v2_files) = self.flatten_v2_files() {
            for (idx, file) in v2_files.iter().enumerate() {
                if selected_files.get(idx).copied().unwrap_or(true) {
                    total += file.length;
                }
            }
        }
        total
    }

    pub fn compute_selected_pieces(&self, selected_files: &[bool]) -> crate::bitfield::Bitfield {
        let num_pieces = self.pieces_count();
        let mut selected = crate::bitfield::Bitfield::new(num_pieces);

        let piece_length = self.info.piece_length;
        if self.info.length.is_some() {
            for p in 0..num_pieces {
                selected.set(p, true);
            }
            return selected;
        }

        let mut current_offset = 0;

        if let Some(files) = &self.info.files {
            for (idx, file) in files.iter().enumerate() {
                let start_offset = current_offset;
                let end_offset = start_offset + file.length;
                current_offset = end_offset;

                if selected_files.get(idx).copied().unwrap_or(true) && file.length > 0 {
                    let start_piece = (start_offset / piece_length) as usize;
                    let end_piece = ((end_offset - 1) / piece_length) as usize;
                    for p in start_piece..=end_piece {
                        if p < num_pieces {
                            selected.set(p, true);
                        }
                    }
                }
            }
        } else if let Some(v2_files) = self.flatten_v2_files() {
            for (idx, file) in v2_files.iter().enumerate() {
                let start_offset = current_offset;
                let end_offset = start_offset + file.length;
                current_offset = end_offset;

                if selected_files.get(idx).copied().unwrap_or(true) && file.length > 0 {
                    let start_piece = (start_offset / piece_length) as usize;
                    let end_piece = ((end_offset - 1) / piece_length) as usize;
                    for p in start_piece..=end_piece {
                        if p < num_pieces {
                            selected.set(p, true);
                        }
                    }
                }
            }
        }
        selected
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

    /// Returns a list of byte ranges that correspond to padding files (BEP 47) and unselected files.
    pub fn get_padding_ranges(&self, selected_files: Option<&[bool]>) -> Vec<crate::task::Range> {
        let mut ranges = Vec::new();
        let mut current_offset = 0;

        if let Some(files) = &self.info.files {
            for (idx, file) in files.iter().enumerate() {
                let is_padding = if let Some(ref attr) = file.attr {
                    attr.contains('p')
                } else {
                    // Fallback: many tools use .pad/ as a convention
                    file.path.first().map(|s| s == ".pad").unwrap_or(false)
                };

                let is_unselected = !selected_files
                    .map(|s| s.get(idx).copied().unwrap_or(true))
                    .unwrap_or(true);

                if is_padding || is_unselected {
                    ranges.push(crate::task::Range {
                        start: current_offset,
                        end: current_offset + file.length,
                    });
                }
                current_offset += file.length;
            }
        } else if let Some(v2_files) = self.flatten_v2_files() {
            for (idx, file) in v2_files.iter().enumerate() {
                let is_unselected = !selected_files
                    .map(|s| s.get(idx).copied().unwrap_or(true))
                    .unwrap_or(true);

                // Assuming v2 doesn't have explicit attr for padding in our flatten_v2_files logic,
                // but we can at least skip unselected.
                let is_padding = file.path.first().map(|s| s == ".pad").unwrap_or(false);

                if is_padding || is_unselected {
                    ranges.push(crate::task::Range {
                        start: current_offset,
                        end: current_offset + file.length,
                    });
                }
                current_offset += file.length;
            }
        }

        ranges
    }
}
