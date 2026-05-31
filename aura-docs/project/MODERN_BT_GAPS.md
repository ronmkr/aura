# BitTorrent Feature Gaps: aria2 vs. Aura

While aria2 is an exceptionally fast and lightweight multi-protocol tool, it lacks several features found in modern, specialized BitTorrent clients. Aura was designed specifically to bridge these gaps while maintaining aria2's lightweight performance.

## 1. BitTorrent v2 (BEP 52) Support
* **aria2 Status**: Missing (supports only BitTorrent v1 with SHA-1)
* **Aura Status**: Implemented (supports native v2 and hybrid v1+v2 swarms using SHA-256 Merkle trees and Sled-based persistence)

## 2. Automatic Port Forwarding (UPnP / NAT-PMP)
* **aria2 Status**: Missing (requires manual port forwarding configuration)
* **Aura Status**: Implemented (automatic router negotiation using UPnP and NAT-PMP with 30-minute refresh cycles)

## 3. IP Filtering and Eviction
* **aria2 Status**: Limited/Missing (no native peer rating or eviction)
* **Aura Status**: Implemented (latency and reputation-based peer health scoring with automatic bottom-10% peer eviction)

## 4. Built-in Search and RSS
* **aria2 Status**: Missing
* **Aura Status**: Planned (tracked in the future backlog)

## 5. Peer Exchange (PEX) and DHT Robustness
* **aria2 Status**: Limited (lacks modern PEX advertising or DHT bootstrap refreshing)
* **Aura Status**: Implemented (BEP 11 Peer Exchange with active delta calculations, and dynamic DHT bootstrap node refreshing)

## 6. Web UI Integration
* **aria2 Status**: External Only (requires external Web UI hosting)
* **Aura Status**: Implemented (built-in Web UI dashboard served directly from the daemon binary using embedded assets)

## 7. Advanced Seeding Management
* **aria2 Status**: Limited
* **Aura Status**: Implemented (enforceable upload ratios and custom seed time limits in task state tracking)

---

### Conclusion
Aura successfully upgrades the classic multi-protocol downloading capabilities of aria2 by incorporating modern, privacy-focused, and robust BitTorrent standards natively, making it a state-of-the-art backend service.
