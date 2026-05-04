# ADR 0039: BitTorrent Endgame Mode

## Status
Proposed

## Context
In a BitTorrent swarm, the final blocks of a download often take much longer to retrieve than the rest. This happens because some peers might be slow, or their connection might drop after we've requested a piece but before it's delivered. This is known as the "99% stall."

## Decision
We will implement "Endgame Mode." When a download is nearly complete and all remaining pieces have been requested at least once, the system will enter a specialized state where redundant requests are sent to multiple peers for the same blocks.

### 1. Trigger Condition
Endgame Mode is triggered when:
- The number of pending pieces is small (e.g., < 5 pieces or < 2% of total pieces).
- All pending pieces are already marked as `in_progress` in the `PiecePicker`.

### 2. Piece Picking in Endgame
In Endgame Mode, `PiecePicker::pick_next` will:
- Ignore the `in_progress` bitfield.
- Pick from pieces that the peer has but that we have not yet marked as `completed`.
- Still prioritize rarest-first if applicable.

### 3. Redundant Requests & Verification
- `BtWorker` will request blocks for endgame pieces even if they are already being downloaded by other workers.
- The `StorageEngine` and `PiecePicker` act as the source of truth. When a piece is successfully verified and written to disk, it is marked as `completed`.
- Workers must check if a piece/block is already completed before sending a request.

### 4. Cancellation (Optional but Recommended)
When a worker successfully verifies a piece, it should notify the `Orchestrator`. The `Orchestrator` should then send a signal to all other `BtWorkers` downloading that same piece to send `Cancel` messages to their respective peers. This saves swarm bandwidth.

## Consequences
- **Pros**: Significantly reduces the time to finish downloads. Eliminates the "slowest peer" bottleneck at the end of a task.
- **Cons**: Slightly increased bandwidth usage due to redundant block downloads (this is acceptable given it only happens at the very end).
- **Complexity**: Requires careful coordination between workers and the picker to avoid thundering herds.
