# ADR 0055: SecretScrubber for Log Sanitization

## Status
Implemented (2026-06-03, remediation/immediate-security)

## Context
Aura handles sensitive authentication credentials (bearer tokens, basic auth credentials, session cookies, and `.netrc` passwords) across multiple protocol workers. During debugging and production monitoring, standard logging (`tracing` spans and events) can inadvertently print these credentials, leaking secrets to stdout, systemd journals, files, or distributed OpenTelemetry spans (GAP-03).

## Decision
1. **SecretScrubber Subsystem**: Implement a centralized `SecretScrubber` layer using the `tracing-subscriber::Layer` API.
2. **Log/Trace Filtering**: Intercept all logs and structured telemetry spans. Apply a regex-based or token-aware scanning pass to identify and redact sensitive headers, authorization fields, URI credentials (e.g., `user:pass@host`), and environment configurations before they are serialized or printed.
3. **Common Secret Patterns**: Maintain a static compile-time dictionary of patterns to scrub, including:
   - `Authorization: Bearer <token>`
   - `Authorization: Basic <base64>`
   - `Cookie: <session>`
   - Inline URL credentials (`http://username:password@domain`)
   - Config parameters matching keys like `rpc-secret` or `password`.

## Edge Cases
1. **PEM Private Keys & Multi-line Secrets**: Multi-line credentials (e.g., SSH private keys, custom TLS certificates) do not match single-line token search regexes. The scrubber must support multi-line stateful scanning or detect boundaries (e.g., `-----BEGIN PRIVATE KEY-----`).
2. **False Positives in User Content**: A user search query, folder name, or torrent title might contain words like `Bearer` or `token`. The scrubber must limit scrubbing to header values, structured connection spans, or config serialization logs, avoiding general log payload search queries where possible.
3. **High Throughput and CPU Exhaustion**: Compiling regexes at runtime on hot logging paths can degrade CPU performance under high network throughput. Regexes must be pre-compiled using static primitives (e.g., `once_cell` / `lazy_static`) and run only on target-specific tracing fields rather than flat log strings.
4. **Partially Transmitted Spans**: If log messages are truncated or split across buffer lines, secret patterns could be broken and bypass the scrubber. Scrubber must inspect connection configuration structs before emission, not just the output formatted strings.

## Alternatives Considered
- **Manual Censoring in Code**: Requiring developers to manually filter variables in `tracing::info!` or `tracing::debug!`. *Rejected:* Highly error-prone and easy to forget, risking accidental leaks.
- **Log Level Isolation**: Restricting log levels to exclude verbose output in production. *Rejected:* Prevents detailed debugging when issues occur, and does not guarantee that secrets won't be printed at `info` level.

## Consequences
- **Pros**: Zero-risk credential leaks in production logging; automated security compliance.
- **Cons**: Minor CPU overhead for pattern matching on every log output line.
