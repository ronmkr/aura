# Telemetry & Metrics

Aura provides deep visibility into its internal state through a dedicated telemetry actor and a Prometheus-compatible metrics exporter (ADR 0004).

## Prometheus Exporter

The Aura Daemon can expose a scraping endpoint for Prometheus to collect real-time performance data.

### Configuration
Enable the exporter in `Aura.toml`:
```toml
[monitoring]
metrics_enabled = true
metrics_port = 9100
scrape_token = "your-secure-token" # Recommended for security
```

### Authentication
The `/metrics` endpoint is protected by bearer authentication:
- **X-Aura-Token**: By default, the endpoint accepts the same token used for JSON-RPC.
- **Dedicated Token**: If `scrape_token` is set in the `[monitoring]` section, that token must be provided in the `Authorization: Bearer <token>` header.
- **Health Checks**: The `/health` endpoint remains unauthenticated for liveness probes.

### Key Metrics
| Metric | Type | Description |
|--------|------|-------------|
| `aura_download_speed_bytes`| Gauge | Current global download throughput. |
| `aura_active_tasks_count` | Gauge | Total number of tasks in the `Downloading` phase. |
| `aura_peer_connections` | Gauge | Total number of active BitTorrent peer connections. |
| `aura_storage_write_latency_ms`| Histogram| Latency for sequential write aggregation flushes. |
| `aura_error_total` | Counter | Cumulative count of task-level errors. |

## Structured Event Bus

All internal state transitions (e.g., `TaskAdded`, `PieceFinished`) are dispatched to a centralized **Event Bus**. This bus powers:
1.  **WebSocket Telemetry**: Real-time streaming to the TUI and WebUI.
2.  **Lifecycle Hooks**: Triggering user-defined scripts.
3.  **Audit Logs**: Persistence of critical events for compliance.

## Distributed Tracing (ADR 0032)

For deep debugging, Aura supports **Span Tagging**. Every network request is assigned a unique `trace_id`. If a piece fails hash verification, the trace log allows you to identify exactly which peer delivered the corrupted block and which protocol worker was responsible.
