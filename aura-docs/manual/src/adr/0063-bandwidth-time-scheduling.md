# Decision 0063: Bandwidth Time Scheduling

## Status

Implemented (2026-06-04, PR #259 — Issue #249)

## Context

Aura's token bucket throttler (Decision-0009) enforces static global and per-task bandwidth limits configured via `Aura.toml`. However, many real-world use cases require time-varying bandwidth policies: ISPs with off-peak unlimited data windows (e.g., 2AM–6AM), office environments requiring download throttling during business hours, or home users wanting to prioritize gaming bandwidth in the evenings. No scheduling mechanism exists in the current architecture. The `global_download_limit` and `global_upload_limit` fields in `BandwidthConfig` are static values that can only be changed via config hot-reload (Decision-0011) but not automatically based on time-of-day. Related: GitHub Issue #248 (to be created).

## Decision

1. Add a `[[bandwidth.schedule]]` array to the TOML config schema. Each entry has fields: `from: String` (HH:MM in 24-hour local time), `to: String` (HH:MM), `download_limit: u64` (bytes/sec, 0 = unlimited), `upload_limit: u64` (bytes/sec, 0 = unlimited), and optionally `days: Vec<String>` (e.g., `["Mon", "Tue", "Wed", "Thu", "Fri"]` to limit to weekdays).
2. Introduce a `BandwidthScheduler` component in `aura-core/src/config/` that evaluates the schedule array against the current local time and returns the effective limits.
3. The Orchestrator starts a background task that calls `BandwidthScheduler::effective_limits()` every 60 seconds and applies any changed limits to the global `TokenBucket` (Decision-0009) via the existing hot-reload mechanism (Decision-0011).
4. Schedule entries are sorted by specificity (more specific `days` patterns take priority). If multiple entries match the current time, the most recently listed entry wins (last-write-wins, consistent with TOML array ordering).
5. The active schedule window and next transition time are exposed via the `aura.getConfig` RPC method and `aura status` CLI output.

## Edge Cases

1. **Midnight Spanning Windows**: A schedule entry `from = "22:00", to = "06:00"` spans midnight. The scheduler must handle `to < from` as a cross-day window, not an error.
2. **DST Transitions**: Daylight Saving Time shifts can cause a 1-hour window to be skipped or repeated. The scheduler must use the `chrono` crate's local time with DST awareness and recalculate on each tick, not pre-compute next-transition times.
3. **Empty Schedule**: If no `[[bandwidth.schedule]]` entries exist, use the static `global_download_limit` and `global_upload_limit` values. No background timer is started in this case.
4. **Overlapping Windows**: Two entries both match the current time. Document and enforce last-entry-wins semantics clearly in `Aura.example.toml`.
5. **Timezone Configuration**: The schedule uses local system time by default. An optional `timezone = "UTC"` or `timezone = "America/New_York"` field should be supported for headless/server deployments where system timezone may be UTC.

## Alternatives Considered

- **External cron scheduling**: Let users use system cron to run `aura config set global_download_limit ...`. *Rejected:* Fragile; requires system cron access; doesn't survive daemon restarts; poor UX.
- **aura `changeGlobalOption` method**: Allow RPC clients to update limits dynamically. *Not rejected but complementary:* aura compatibility requires this method anyway; scheduling is a higher-level built-in convenience.

## Consequences

- **Pros**: Native time-based bandwidth management; works without external cron; particularly valuable for users on ISP plans with off-peak unlimited tiers.
- **Cons**: Adds complexity to the config schema and a background timer; timezone handling adds subtle correctness requirements.
