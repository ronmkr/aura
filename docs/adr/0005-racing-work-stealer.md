# ADR 0005: Racing Work Stealer for Slow Stream Mitigation

## Status
Accepted

## Context
In multi-source downloads, individual network streams (HTTP mirrors or BitTorrent peers) can become "stalled" or significantly slower than the rest of the swarm. We need a way to prevent these "long-tail" pieces from delaying the completion of a download.

## Decision
We will implement a **Racing Work Stealer**.
1. **Trigger**: The **Piece Selector** monitors the throughput of all active **Protocol Workers**. If a piece's estimated time to completion is significantly higher than a threshold based on the current average speed, it is marked as a "Steal Candidate."
2. **Action**: When a high-speed worker becomes idle and asks for work, the **Piece Selector** assigns it a "Steal Candidate" piece, even if it is already being fetched.
3. **Resolution**: The **Storage Engine** receives data from both workers. The first worker to successfully complete and verify the piece wins. The **Orchestrator** then signals the "loser" to abort its request.

## Alternatives Considered
- **Cancel-and-Reassign**: Immediately kill the slow connection and give the piece to someone else. *Rejected:* The "slow" connection might be 90% done; killing it wastes that progress. Racing preserves the chance that the original worker finishes first.
- **Fixed Timeout**: Steal after a piece has been outstanding for X seconds. *Rejected:* Doesn't account for variations in file size or total swarm speed.

## Consequences
- **Pros**: Maximizes throughput, eliminates "stuck" downloads at 99%, and avoids wasting nearly-finished work.
- **Cons**: Increases bandwidth usage slightly due to redundant data fetching during the "race."
