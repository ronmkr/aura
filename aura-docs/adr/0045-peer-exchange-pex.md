Status: Implemented

# ADR 0045: Peer Exchange (PEX) Implementation (BEP 11)

## Status
Implemented (2026-05-30, PR #159)

## Context
Peer Exchange (PEX) allows BitTorrent clients to exchange known peers directly with each other without relying solely on Trackers or the DHT. This reduces tracker load, creates a more robust swarm, and enables faster discovery of active peers.

BitTorrent Extension Protocol (BEP 10) provides the foundation, while BEP 11 specifies the actual PEX `ut_pex` message format.

## Decision
We will implement BEP 11 PEX within the existing `BtWorker` architecture with the following constraints:

1. **Protocol Support**: We will advertise support for PEX by injecting `"ut_pex": 2` (or similar ID) into the `ExtendedHandshake` `m` dictionary.
2. **Delta Enforcement**: A peer must only send the peers that have been added or dropped since the last PEX message sent to that specific peer. We will maintain a `last_sent_pex_peers: HashSet<SocketAddr>` within each `BtWorker`'s state.
3. **Timer-Based Exchange**: We will evaluate the deltas and send a `ut_pex` message every 60 seconds (adhering to the BEP 11 maximum frequency constraint). If the deltas are empty, no message will be sent to conserve bandwidth.
4. **Data Encoding**: The `added` and `dropped` fields are contiguous byte sequences. IPv4 addresses use 6 bytes (4 bytes IP, 2 bytes port). IPv6 addresses use 18 bytes (`added6` / `dropped6`).
5. **Architectural Flow**:
   - `BtWorker` regularly fetches active peers from the `BtTask`'s `PeerRegistry`.
   - `BtWorker` calculates the delta against its local `last_sent_pex_peers`.
   - Incoming PEX peers are forwarded to the Orchestrator via `SubTaskEvent::PexPeersDiscovered`.
   - Orchestrator injects them directly into the `PeerRegistry`, where they become candidates for new outbound connections.

## Consequences
- Reduces dependency on DHT and Trackers, improving swarm resilience.
- Increases memory usage slightly per active connection (maintaining a `HashSet` of known peers).
- PEX parsing logic must be robust to prevent panic on malformed byte lengths (e.g., length not divisible by 6).

## Implementation
- **Peer Exchange (PEX)**: Implemented in `aura-core/src/bt_worker/extended/pex.rs` and integrated with registry (2026-05-30, PR #159).
