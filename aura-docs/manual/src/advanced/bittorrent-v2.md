# BitTorrent V2 & Merkle Trees

Aura is a modern BitTorrent client supporting the **BEP 52** specification (BitTorrent v2). This modernization solves several long-standing issues with the v1 protocol (BEP 3).

## Sha-256 Merkle Trees (bep 52)

In BitTorrent v1, piece hashes (SHA-1) were stored linearly in the `.torrent` file. For large files (e.g., 100GB+), the metadata itself could reach several megabytes, causing high latency for initial swarm joining.

**BitTorrent v2 utilizes SHA-256 Merkle Trees:**
- **Per-file Trees**: Every file in a multi-file torrent has its own independent Merkle tree. This allows for file-level data deduplication across different swarms.
- **Logarithmic Metadata**: Verification requires only the **Root Hash**. Intermediate "uncle" hashes are fetched dynamically from peers as needed.
- **Block-Level Verification**: Aura verifies integrity at the 16KB block level (standard request size) rather than waiting for an entire piece (often 4MB-16MB) to finish, allowing for instant detection and rejection of malicious data.

## Hybrid Torrents

Aura fully supports **Hybrid Torrents** (BEP 52), which contain both v1 (SHA-1) and v2 (SHA-256) metadata.
- **Swarm Bridging**: Aura can connect to both v1 and v2 peers for the same task, effectively acting as a bridge between the two swarms.
- **Unified Storage**: Data retrieved from a v1 peer is automatically verified against the v2 Merkle tree (and vice versa) before being committed to disk by the central Storage Engine.

## Traffic Encryption (MSE/PE) (Decision 0066)

Aura supports **Message Stream Encryption (MSE)** and **Protocol Encryption (PE)** to obfuscate BitTorrent traffic. This prevents Deep Packet Inspection (DPI) by ISPs from throttling or blocking BitTorrent connections.

### Encryption Policies

You can configure the encryption policy in `Aura.toml`:
- **`prefer`** (Default): Attempt to use encryption but fall back to plain text if the peer doesn't support it.
- **`require`**: Only connect to peers that support encryption. This maximizes privacy but may reduce the number of available peers.
- **`disable`**: Never use encryption.

---

## Tracker Scrape & Swarm Statistics (issue #289)

Standard BitTorrent "Announces" only provide a list of active peers. To provide a complete picture of swarm health, Aura implements **Tracker Scraping**:

- **Real-time Statistics**: Aura periodically queries trackers for total **Seeders**, **Leechers**, and **Completed** counts for every active task.
- **TUI Visualization**: These statistics are displayed in the TUI Mission Details panel (e.g., `Seeds: 4 (of 180)`), giving you a better understanding of the swarm's overall health beyond your direct connections.
- **Efficiency**: Scrape requests are compact and efficient, allowing Aura to track thousands of tasks without significant network overhead.

---

## Peer Exchange (pex) & Dht

Aura implements high-performance discovery protocols:
- **PEX (BEP 11)**: A gossip protocol where peers share their own lists of known good peers. Aura uses a reputation-based filter to prioritize high-uptime peers from PEX messages.
- **Mainline DHT (BEP 5)**: A Kademlia-based Distributed Hash Table for trackerless discovery. Aura periodically refreshes its routing table and stores high-uptime "bootstrap" candidates to ensure fast joining even if primary trackers are offline.

## Choking Algorithm (tit-for-tat)

To ensure swarm health, Aura implements a standard **Choking Algorithm**:
- **Unchoking**: Aura unchokes the top 4 peers providing the highest download rates.
- **Optimistic Unchoke**: Every 30 seconds, Aura randomly unchokes a peer to discover new high-speed sources.
- **Anti-Snubbing**: If a peer hasn't sent data for 60 seconds, it is marked as "Snubbed," and its priority is lowered in the piece picker.

## Endgame Mode (Decision 0039)

To prevent the common "99.9% stall" caused by a single slow peer holding the final blocks:
- **Trigger**: Aura enters Endgame Mode when fewer than 20 blocks (320KB) remain.
- **Redundant Requests**: Aura broadcasts requests for the remaining blocks to *every* unchoked peer simultaneously.
- **Atomic Cancellation**: The moment the first valid block arrives and passes hash verification, Aura sends `CANCEL` messages to all other peers for that specific block.

## Prioritized Streaming Mode (issue #28)

To support real-time media playback and progressive file previewing, Aura implements a prioritized piece picking strategy when streaming mode is enabled:
- **Metadata/Header Prioritization**: When `streaming_mode` is active, the first `N` and last `N` pieces of the file (configured via `streaming_metadata_pieces`, default `4`) are prioritized.
 The first pieces typically contain container headers (e.g., MP4/MKV headers), while the last pieces contain structural indexes (e.g., moov atoms/metadata).
- **Sequential Acquisition**: These boundary pieces are picked sequentially in chronological/index order to allow media players to parse metadata and begin playback with minimal buffering delay.
- **Middle Swarm Health**: Once the critical header/index boundary pieces are downloaded, Aura falls back to standard piece picking strategies (such as rarest-first) for all middle pieces. This protects swarm health and prevents the degradation of overall download speed.
- **Dynamic Activation**: Streaming mode can be enabled or disabled dynamically for any active task via the `aura.changeOption` JSON-RPC method or the CLI.
