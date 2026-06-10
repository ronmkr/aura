# Resource Governor & Memory Backpressure

Aura handles extreme high-speed downloads by dynamically managing its memory consumption. The **Resource Governor** (ADR 0057) is the central component responsible for preventing Out-Of-Memory (OOM) crashes through global backpressure.

## 1. Decentralized Allocation Tracking

Unlike traditional clients that use a fixed-size buffer pool, Aura allows workers to allocate memory directly for speed. However, every allocation is gated by the **Resource Governor**:
- **Atomic Counters**: The governor tracks the total number of bytes currently held in RAM across all actors (Orchestrator, Storage, and Workers).
- **Budget Requests**: Before a worker can allocate a new piece buffer (e.g., 16MB), it must request a budget from the governor.

## 2. Memory Backpressure Mechanism

If the global memory limit (default: 512MB) is breached, the governor applies "backpressure" to slow down ingestion:
- **Piece Picking Choke**: The `PiecePicker` in the Orchestrator will refuse to assign new work to any worker if the governor's budget is exhausted.
- **Worker Stall**: Workers already in progress will stall their network requests until the **Storage Engine** flushes pending data to disk, releasing memory back to the governor.
- **Natural Equilibrium**: This creates a self-regulating loop where network speed automatically scales to match the physical write speed of your disk.

## 3. Safety Margins & Critical Allocations

To prevent deadlocks (where a system is so full it can't even process the "flush" command), the governor maintains a **Safety Margin**:
- **Metadata Protection**: A portion of the memory budget is reserved exclusively for metadata (e.g., `.torrent` files) and internal control messages.
- **Unchokable Tasks**: Critical operations like block verification and session saving are always allowed to proceed, even if the data limit is reached.

## 4. Multi-Tenant Fairness

In shared environments, the Resource Governor ensures that one high-speed download doesn't starve others of memory:
- **Fair-Share Limits**: The governor calculates a `limit / active_tenants` quota.
- **Tenant Isolation**: If Tenant A is saturating their memory quota, Tenant B can still allocate buffers for their own tasks, ensuring responsive performance for all users.

## 5. Configuration

Adjust these limits in `Aura.toml`:

```toml
[storage]
memory_limit_mb = 512          # Total RAM allowed for download data
memory_safety_margin_mb = 50   # Reserved RAM for metadata and control
```
