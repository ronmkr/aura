# Download History Log

Aura maintains a persistent, append-only log of all completed, failed, and stopped downloads. This provides users with auditability, session persistence, and compatibility with standard download manager tools.

## 1. Overview (Decision 0062)

The Download History system automatically records task completion statuses, start and end timestamps, total download size, protocols used, and any termination errors. 

Unlike active session states that are removed from memory when a task is deleted or cleared, the history log persists indefinitely (subject to rotation limits) at:
- **Linux/macOS**: `~/.aura/history.jsonl`
- **Windows**: `%AppData%\aura\history.jsonl`

## 2. CLI Usage

You can query, filter, and format the history log using the `aura history` subcommand.

### Filtering Records
Filter the log by state: `completed`, `failed`, or `removed`.
```bash
aura history --filter failed
```

### Adjusting Results
Limit the number of returned records (default: 10).
```bash
aura history --limit 25
```

### Output Formatting
Export history records as a clean terminal table (default) or raw JSON for automation.
```bash
aura history --format json
```

## 3. RPC Method

Integration scripts and frontends can fetch history logs programmatically via the JSON-RPC interface:
- **Method**: `aura.tellStopped`
- **Parameters**: `[offset: Option<usize>, limit: Option<usize>, filter: Option<String>]`
- **Response**: Array of historical task structures.

## 4. Log Rotation & Retention

To prevent the history file from consuming excessive disk space over months of continuous download activity, the background engine enforces automatic rotation limits configured in `Aura.toml`:

```toml
[limits]
history_record_limit = 100000        # Hard cap on historical records in DB
history_rotation_mb = 10.0          # Max size of log file before rotation
history_rotation_records = 10000    # Max records in log file before rotation
history_retention_records = 5000    # Records to retain after a rotation event
```

When the file size or record count exceeds these limits, the oldest records are pruned, keeping only the most recent entries.
