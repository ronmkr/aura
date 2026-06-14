# Aura Project Management & Issue Tracking 

To ensure high-quality implementation and avoid "stub features," we use the following management strategy for all open and future issues.

##  Issue Labeling System

We use precise labels to track the *maturity* of a feature, not just its intent.

| Label | Meaning |
| :--- | :--- |
| `type:enhancement` | New functionality being planned. |
| `status:stub` | API/Structs exist but the logic is missing or returning empty results. |
| `status:unverified` | Implementation is merged but lacks automated integration tests. |
| `status:stable` | Feature is implemented and has 80%+ test coverage. |
| `module:core` | Affects the core orchestration or worker logic. |
| `module:storage` | Affects disk I/O, aggregation, or memory management. |

##  Definition of Done (DoD)

An issue is not considered "Closed" unless it meets these criteria:
1.  **Code**: Implementation follows Decision specs.
2.  **Documentation**: Relevant Decision status is updated to `Verified`.
3.  **Tests**: At least one unit test and one integration test (in `tests/`) are added.
4.  **Formatting**: `cargo fmt` and `cargo clippy -- -D warnings` pass.

##  Implementation Linkage

Every Issue description must include an **Implementation Map**:
- **Decision**: Link to the relevant Decision.
- **Primary File**: The main file where logic resides.
- **Test File**: The file containing verification logic.

##  Milestone Alignment

Issues are grouped into GitHub Milestones corresponding to `ROADMAP.md`. PRs should only be merged into `main` if they fulfill an active milestone's requirements.
