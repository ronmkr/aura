pub mod file_tree;
pub mod hashing;
pub mod merkle;

use super::logic::Torrent;
use crate::{Error, Result};

impl Torrent {
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
