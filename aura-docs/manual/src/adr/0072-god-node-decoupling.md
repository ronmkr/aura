# Decision 0072: Architectural Decoupling of Engine God Nodes

## Status

Implemented (2026-06-13, refactor/engine-storage-decoupling)

## Context

Initial implementations of the `Engine` and `StorageEngine` followed a "God Node" pattern. The `Engine` struct centralized over 30 disparate concerns (Sandboxing, Telemetry, System Configuration, Task Lifecycle), resulting in a high degree of betweenness centrality in the knowledge graph. Similarly, protocol workers were tightly coupled to the storage layer via raw MPSC channels, making unit testing difficult without spinning up the entire background system.

## Decision

1. **Interface Segregation**: Extract fine-grained traits from the monolithic `Engine` struct to isolate responsibilities.
   - `EventSubscriber`: Handles telemetry and event broadcasting.
   - `TaskController`: Handles state mutations (pause, resume, remove).
   - `TaskQuerier`: Handles metadata retrieval.
   - `EngineApi`: A unified trait object combining the above for high-level handlers.
2. **Dependency Inversion**: Update `TaskHandle` and other components to depend on traits (`Arc<dyn EngineApi>`) rather than the concrete `Engine` implementation.
3. **Storage Dispatch Abstraction**: Introduce a `StorageDispatch` trait to decouple data retrieval from disk persistence.
   - Created `StorageClient` as the primary implementation wrapping the MPSC channel.
   - Updated all `ProtocolWorker` implementations (HTTP, FTP, BitTorrent, S3, GDrive, NNTP) to use `Arc<dyn StorageDispatch>`.

## Alternatives Considered

- **Keeping concrete types**: Simpler but maintains high architectural rigidity and technical debt.
- **Single monolithic trait**: Reduces coupling to the concrete type but violates the Interface Segregation Principle.

## Consequences

- **Pros**:
  - significantly reduced architectural coupling (demonstrated by Graphify topological maps).
  - Enables easy unit testing via mocks (e.g., `MockStorageDispatch`).
  - Clearer module boundaries and reduced "circular dependency" risks.
- **Cons**:
  - Increased boilerplate for defining and implementing traits.
  - Minor runtime overhead for dynamic dispatch (trait objects), though negligible in this context.
