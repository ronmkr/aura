# ADR 0064: Process Resilience — Panic Recovery, Crash Reporting, and File Descriptor Management

## Status
Partially Implemented (2026-06-04, branch fix/high-priority-bugs-and-security — Issues #247, #225):
panic hook + crash.log (Decision 1-2) and double-signal shutdown timeout (Decision part of 4) implemented.
FD limit management (Decision 3), /metrics auth (Decision 4, done in ADR-0056 update), RPC rate limiting (Decision 5), and task count cap (Decision 6) remain for subsequent PRs.

## Context
The Aura daemon is a long-running async process that must remain stable under adversarial conditions. Three systemic resilience gaps exist: (1) **Panic handling**: unhandled panics in Tokio tasks silently terminate the task or crash the process without writing crash reports, flushing download state, or persisting DHT routing tables. The orchestrator task is spawned via `tokio::spawn()` with only `Err` handling — panics propagate undetected. (2) **File descriptor exhaustion**: the default configuration allows up to 128 connections per task × 10 concurrent tasks = 1,280 file descriptors, far exceeding the default OS limits (macOS: 256, many Linux distros: 1024). Silent `EMFILE: too many open files` errors manifest as broken connections rather than actionable errors. (3) **RPC endpoint hardening**: the `/metrics` Prometheus endpoint is unauthenticated, and the JSON-RPC endpoint has no request rate limiter, enabling local denial-of-service via task flooding. Related: GitHub Issues #249, #250 (to be created).

## Decision
1. **Panic Hook**: Install a `std::panic::set_hook()` at process startup (before Tokio runtime initialization) that: writes the panic message and backtrace to `~/.aura/crash.log` with a timestamp; flushes stderr; and calls `std::process::exit(101)`. This ensures crash information survives even when the tokio runtime is in an invalid state.
2. **Task Panic Recovery**: Wrap crash-critical spawned tasks (orchestrator, storage engine, DHT actor) in a `JoinHandle` and match on `JoinError::is_panic()`. On panic detection, log the panic, attempt emergency state flush (write active TaskStates to their `.aura` control files), then call `std::process::exit(101)` with the crash log path shown to the user.
3. **File Descriptor Limit Management**: At daemon startup, calculate `required_fds = (max_concurrent_downloads * max_connections_per_task * 2) + 512`. Use the `rlimit` crate to attempt `setrlimit(RLIMIT_NOFILE, required_fds)` on Unix. If the OS hard limit is below `required_fds`, log a startup warning. On Windows (no `RLIMIT_NOFILE`), document the 2048 handle limit and suggest `HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager` configuration.
4. **`/metrics` Authentication**: Apply the existing `X-Aura-Token` bearer authentication middleware to the `/metrics` route. Alternatively, support a separate `X-Prometheus-Token` header configured via `Aura.toml [monitoring] scrape_token`. The `/health` endpoint (ADR-0051) must remain unauthenticated for container liveness probes.
5. **RPC Rate Limiting**: Add a `tower_governor` or `tower::ServiceBuilder` rate-limiting layer to the axum router. Default: 120 requests per minute per connection. Configurable via `Aura.toml [security] rpc_max_requests_per_minute`. When the limit is exceeded, return HTTP 429 with `Retry-After: 60`.
6. **Global Task Count Cap**: Enforce a `[limits] max_active_tasks` config value (default: 500) in `orchestrator/commands/add.rs` for the non-tenant code path. Reject `add_task` calls beyond this cap with `EngineError::TooManyTasks`.

## Edge Cases
1. **Panic in Panic Hook**: If the panic hook itself panics (e.g., writing to `crash.log` fails because the disk is full), the process will abort. The hook must use `eprintln!` as a last-resort fallback since stderr is always available.
2. **Double Panic during Emergency Flush**: If the emergency state flush in the `JoinHandle` error handler itself panics (e.g., serialization failure), the second panic must not recurse. Wrap the flush in `std::panic::catch_unwind()` and skip it on failure.
3. **`ulimit` Hard Limit Too Low**: On some hardened Linux systems, the hard limit for `RLIMIT_NOFILE` may be set below the calculated requirement by the system administrator. In this case, `setrlimit()` to the hard limit as a best-effort, emit a warning, and reduce `max_connections_per_task` automatically to fit within the available FD budget.
4. **Rate Limiter Bypass via Multiple Connections**: A local attacker may open many WebSocket or HTTP connections to bypass per-connection rate limits. Mitigate by adding a global in-memory counter of active RPC connections (configurable `rpc_max_connections`, default 32).

## Alternatives Considered
- **External process supervisor (systemd, supervisord)**: Let the OS restart the daemon on crash. *Not rejected but complementary:* A supervisor provides restart capability, but without a panic hook, crash information is lost and download state is left corrupt. Both are needed.
- **Fearless Rust — panics should not happen**: Eliminate all `unwrap()`/`expect()` calls. *Rejected as sole strategy:* Realistic — third-party library panics, integer overflows in edge cases, and OS-level FFI panics cannot be fully eliminated. Defense-in-depth requires a hook.

## Consequences
- **Pros**: Crash reports dramatically reduce time-to-diagnosis; FD management prevents silent connection failures at scale; rate limiting prevents local DoS; panic recovery preserves download progress across crashes.
- **Cons**: `setrlimit` requires the `rlimit` crate (new dependency); panic hook adds ~5 lines of startup boilerplate; rate limiting adds latency for legitimate high-frequency RPC callers (mitigated by the generous default limit).
