# BitTorrent v2 & Merkle Trees

Aura is a modern BitTorrent client supporting the BEP 52 specification (BitTorrent v2). This modernization solves several long-standing issues with the v1 protocol.

## SHA-256 Merkle Trees

In BitTorrent v1, piece hashes (SHA-1) were stored linearly in the `.torrent` file. For large files, this made the torrent files themselves massive (several megabytes).

**BitTorrent v2 uses SHA-256 Merkle Trees:**
- **Per-file Trees**: Every file in a torrent has its own Merkle tree.
- **Efficient Verification**: Integrity can be verified at the block level using only the root hash and a logarithmic number of intermediate hashes.
- **Deduplication**: Files with the same content have the same root hash, allowing for seamless cross-swarm deduplication.

## Hybrid Torrents

Aura fully supports **Hybrid Torrents**, which contain both v1 (SHA-1) and v2 (SHA-256) metadata. This allows Aura to:
- Connect to older v1-only clients.
- Connect to modern v2 clients.
- Bridge data between both swarms using the same storage engine.

## Persistent Piece Layers

Because v2 piece layers can be large, Aura stores them in a high-performance **Sled** database (`metadata.db`). 
- **Cold Storage**: Metadata is only loaded into RAM when a task is active.
- **Verified Resumption**: On startup, Aura re-verifies the leaf hashes from the database to ensure no corruption has occurred on disk.

## Endgame Mode

To prevent "99% stalls" caused by a single slow peer holding the last few pieces, Aura enters **Endgame Mode** (ADR 0039).
- **Redundant Requests**: When only a few blocks remain, Aura broadcasts requests for those same blocks to *all* active peers.
- **First-Finish Wins**: The first peer to deliver a valid block wins; other requests are immediately canceled.
