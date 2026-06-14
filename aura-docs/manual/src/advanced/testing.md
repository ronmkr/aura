# Testing & Verification

Aura is a safety-critical tool. To ensure data integrity and system stability, we use a multi-tiered testing strategy rooted in **Test-Driven Development (TDD)** and exhaustive verification.

## Unit Testing

Most core components (Bitfield, PiecePicker, Throttler) have exhaustive unit tests. We prioritize 100% branch coverage for logic-heavy modules.

### Testing Frameworks
- **`rstest`**: Used for clean, parameterized test cases and fixture management.
- **`proptest`**: Employed for property-based testing of complex boundary calculations, glob parsing, and segmenter logic.
- **Mocking (`mockall`)**: We use `mockall` to isolate protocol workers from system dependencies.

### MockStorage (Decision 0072)
To enable protocol testing without disk I/O side effects, we use the `MockStorage` helper (`aura-core/src/test_helpers/mock_storage.rs`). This implements the `StorageDispatch` trait, allowing us to verify network ingestion logic in isolation.

## BDD Integration Suite (Cucumber)

We use **Behavior-Driven Development (BDD)** to verify the engine's behavior against real-world scenarios. Our feature files are located in `aura-core/tests/features/`.

### Key Scenarios Tested
- **Aggregation**: Verifying racing work stealing across multiple mirrors.
- **Reliability**: Testing panic recovery and graceful state flushes.
- **VPN Kill-switch**: Verifying that all workers immediately halt if the secure tunnel drops.
- **Cloud & Swarm**: End-to-end verification of BitTorrent v2 Merkle trees and cloud provider integrations (S3, GDrive).
- **Governance**: Enforcing global memory backpressure and CPU prioritization.

### Running Integration Tests
```bash
cd aura-core
cargo test --test cucumber
```

## Performance Benchmarking

Aura includes a benchmarking suite in `aura-core/benches/` using **Criterion** to prevent performance regressions in the "Hot Path."

- **Buffer Bench**: Measures the latency of the `BufferPool` and memory-copy overhead.
- **Storage Bench**: Measures write-aggregation throughput and disk-seek minimization.

## The Green Loop

Every commit to Aura must pass the **Green Loop**, a strict CI gate enforced via `make green-loop`:
1.  **Format**: `cargo fmt --all` (Standard Rust formatting).
2.  **Lint**: `cargo clippy` (Strict **Zero-Warning** policy).
3.  **Unit Tests**: `cargo test --workspace` (Parallel execution).
4.  **Modularity**: `bash scripts/check_file_length.sh` (Enforcing the 350-line soft ceiling).
5.  **Benchmarks**: `cargo bench --no-run` (Ensuring benchmarks remain compilable).
6.  **Integration Tests**: `cargo test --test cucumber`.

## Standards & Conventions
- **Separation of Tests**: We do not use inline `mod tests {` blocks. Unit tests reside in separate `tests.rs` files to keep production code clean.
- **Professional Tone**: No emojis are allowed in commit messages or documentation.
- **Result Propagation**: Avoid `unwrap()` or `expect()`. Use `?` and context-aware error handling to ensure tests fail with helpful diagnostic information.
