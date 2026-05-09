# BitTorrent Feature Gaps: aria2 vs. Modern Clients

While aria2 is an exceptionally fast and lightweight multi-protocol tool, it lacks several features found in modern, specialized BitTorrent clients (like qBittorrent, Transmission, or BiglyBT).

## 1. BitTorrent v2 (BEP 52) Support
*   **Status**: **Missing**
*   **Details**: Modern clients are moving towards the BitTorrent v2 specification, which uses SHA-256 for piece hashes and a Merkle tree structure. aria2 currently only supports BitTorrent v1 (SHA-1).

## 2. Automatic Port Forwarding (UPnP / NAT-PMP)
*   **Status**: **Missing**
*   **Details**: aria2 does not automatically negotiate port forwarding with routers. Users must manually forward ports (default 6881-6999) or rely on a pre-configured network environment.

## 3. IP Filtering / Blocklists
*   **Status**: **Limited/Missing**
*   **Details**: Many modern clients allow loading an `ipfilter.dat` or similar blocklist to automatically ban known malicious or "anti-p2p" IPs. aria2 lacks a native, easy-to-use IP blocklist feature for peers.

## 4. Built-in Search & RSS
*   **Status**: **Missing**
*   **Details**: There is no native RSS feed aggregator or torrent search engine integration. These must be handled by external scripts or frontends via the RPC interface.

## 5. Peer Exchange (PEX) & DHT Nuances
*   **Status**: **Supported**, but lacks the fine-grained control found in GUI clients (e.g., manual peer adding, easy tracker swapping, or per-tracker statistics).

## 6. Web UI Integration
*   **Status**: **External Only**
*   **Details**: aria2 provides a powerful RPC interface but no built-in Web UI. Users must host or use a separate frontend like AriaNg, webui-aria2, or yaaw.

## 7. Advanced Seeding Management
*   **Status**: **Limited**
*   **Details**: While it supports `--seed-time` and `--seed-ratio`, it lacks advanced features like "Super Seeding" (Initial Seeding) and fine-grained control over seeding priority based on health.

---

### Conclusion
aria2 is optimized for **automation, speed, and low resource usage**. It is often used as a "backend engine" for other applications. For a traditional "desktop" torrenting experience with privacy blocklists and automatic port configuration, a dedicated BitTorrent client is still superior.
