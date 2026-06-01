use super::BtWorker;
use crate::worker::bittorrent::task::BtTask;
use crate::Result;
use futures_util::SinkExt;
use sha2::Digest;
use tracing::{debug, error};

impl BtWorker {
    pub(crate) async fn verify_block_v2(
        &self,
        task: &BtTask,
        index: u32,
        begin: u32,
        block: &[u8],
    ) -> Result<bool> {
        let torrent_guard = task.state.torrent.lock().await;
        let torrent = match torrent_guard.as_ref() {
            Some(t) => t,
            None => return Ok(true),
        };

        if torrent.info.meta_version != Some(2) {
            return Ok(true);
        }

        let block_idx_in_piece = (begin / 16384) as usize;
        if let Ok(expected_block_hash) =
            torrent.block_hash_v2(index as usize, block_idx_in_piece, Some(&task.state.db))
        {
            let mut hasher = sha2::Sha256::new();
            hasher.update(block);
            let actual_block_hash: [u8; 32] = hasher.finalize().into();

            if actual_block_hash != expected_block_hash {
                error!(addr = %self.peer_addr, %index, begin, "Block hash mismatch! Discarding block.");
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub(crate) async fn check_and_request_hashes<S>(
        &mut self,
        framed: &mut tokio_util::codec::Framed<S, super::protocol::PeerCodec>,
        task: &BtTask,
        piece_idx: usize,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let torrent_guard = task.state.torrent.lock().await;
        let torrent = match torrent_guard.as_ref() {
            Some(t) => t,
            None => return Ok(()),
        };

        if let Some(root) = torrent.get_pieces_root_for_piece(piece_idx) {
            if !self.requested_hashes.contains(&root) {
                let mut key = Vec::with_capacity(36);
                key.extend_from_slice(&root);
                key.extend_from_slice(&0u32.to_be_bytes()); // Layer 0 (leaves)

                let in_db = task.state.db.contains_key(&key).unwrap_or(false);

                if !in_db {
                    debug!(addr = %self.peer_addr, %piece_idx, "Requesting block hashes for v2 file");
                    framed
                        .send(super::protocol::PeerMessage::HashRequest {
                            pieces_root: root,
                            index: 0,
                            base: 0,
                            length: 0, // BEP 52: 0 means all hashes in the layer
                            proof_layers: 0,
                        })
                        .await?;
                    self.requested_hashes.insert(root);
                }
            }
        }
        Ok(())
    }
}
