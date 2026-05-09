# Testing & Verification

Aura is a safety-critical tool. To ensure data integrity and system stability, we use a multi-tiered testing strategy.

## Unit Testing

Most core components (Bitfield, PiecePicker, Throttler) have exhaustive unit tests within their respective files. We prioritize 100% branch coverage for logic-heavy modules.

## BDD Integration Suite (Cucumber)

We use **Behavior-Driven Development (BDD)** to verify the engine's behavior against real-world scenarios. Our feature files are located in `aura-core/tests/features/`.

### Key Scenarios Tested
- **Persistence**: Pausing a download, restarting the engine, and resuming without data loss.
- **VPN Kill-switch**: Dropping a virtual interface and verifying that all workers immediately halt.
- **Work Stealing**: Simulating a slow mirror and verifying that another worker "races" to finish the range.
- **BitTorrent v2**: Verifying pieces against SHA-256 Merkle trees.

### Running Integration Tests
```bash
cd aura-core
cargo test --test cucumber
```

## Performance Benchmarking

Aura includes a benchmarking suite in `aura-core/benches/` to prevent performance regressions in the "Hot Path."

- **Buffer Bench**: Measures the latency of the `BufferPool` and memory-copy overhead.
- **Storage Bench**: Measures write-aggregation throughput on different filesystems.

## The Green Loop

Every commit to Aura must pass the **Green Loop**, a strict CI check:
1.  **Format**: `cargo fmt`
2.  **Lint**: `cargo clippy -- -D warnings`
3.  **Unit Tests**: `cargo test --lib`
4.  **Integration Tests**: `cargo test --test cucumber`
