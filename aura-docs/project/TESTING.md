# Aura Real-World Verification Suite 🧪

This document defines the manual and automated test scenarios required to verify the engine's performance, stability, and protocol compliance in production environments.

## 🏗️ Test Categories

### 1. Protocol Aggregation (The "Hyper-Scale" Check)
Verify that the engine can combine disparate sources into a single consistent file.
- **Scenario 1.1**: Metalink with mixed HTTP and FTP mirrors.
- **Scenario 1.2**: BitTorrent + HTTP web-seed aggregation.
- **Scenario 1.3**: Racing/Work Stealing: Artificially slow down one source and verify the engine "steals" the remaining ranges via faster mirrors.

### 2. BitTorrent Swarm Health
Verify peer discovery and data integrity in large swarms.
- **Scenario 2.1**: Magnet Link to Metadata maturation.
- **Scenario 2.2**: DHT/PEX discovery (Bootstrap nodes).
- **Scenario 2.3**: BitTorrent v2 (Hybrid) integrity verification using Merkle trees.
- **Scenario 2.4**: Seeding: Verify the engine maintains the configured seed ratio/time.

### 3. Resource Governance & Throttling
Verify that the engine respects system limits and user configuration.
- **Scenario 3.1**: Global Throttling: Set a 1MB/s limit and verify `aura` does not exceed it.
- **Scenario 3.2**: Hierarchical Throttling: Set a global limit of 2MB/s and two tasks at 500KB/s each; verify both are capped correctly.
- **Scenario 3.3**: Adaptive Scaling: Use a mirror that caps per-connection speed; verify Aura spawns more connections to reach the global limit.

### 4. Persistence & Reliability
Verify that the engine handles interruptions without data loss.
- **Scenario 4.1**: Pause/Resume: Stop a download at 50% and resume; verify SHA-256 integrity of the final file.
- **Scenario 4.2**: Session Recovery: Kill the daemon and restart; verify `.aura` control files are reloaded and tasks resume automatically.
- **Scenario 4.3**: VPN Kill-switch: Artificially drop the configured network interface and verify all tasks immediately pause.

### 5. Storage Performance
Verify disk efficiency and atomic safety.
- **Scenario 5.1**: Atomic Completion: Verify the file only appears in the destination after 100% completion (renamed from `.part`).
- **Scenario 5.2**: Sequential Aggregation: Verify that out-of-order network chunks are written to disk as contiguous blocks where possible.

---

## 🛠️ Execution Log (Current Milestone)

| ID | Scenario | Status | Verified Version | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 1.1 | Metalink Multi-Source | ✅ Pass | v0.1.0 | Aggregates mixed mirrors correctly. |
| 3.1 | Global Throttling | ✅ Pass | v0.1.0 | Verified with 10MB test; requires progress batching. |
| 5.1 | Atomic Completion | ✅ Pass | v0.1.0 | File renamed from .part on 100% completion. |
| 5.2 | Sequential Aggregation| ✅ Pass | v0.1.0 | Verified correct assembly of out-of-order chunks. |
| 4.1 | Pause/Resume | ⏳ Pending | | |

---

## 🤖 Future Automation Strategy
We aim to automate these scenarios using:
1. **Local Infrastructure**: Docker containers for mock HTTP/FTP/SOCKS5 servers (using `nginx` and `dante`).
2. **Mock Swarm**: A local private tracker and peer set for BitTorrent verification.
3. **Property-based Testing**: Using `proptest` for protocol message parsing and bitfield logic.
