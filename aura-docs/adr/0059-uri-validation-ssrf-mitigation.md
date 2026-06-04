# ADR 0059: URI Validation and SSRF Mitigation

## Status
Implemented (2026-06-04, PR #258 — Issues #241, #244)

## Context
The `aria2.addUri` RPC endpoint (ADR-0016, ADR-0056) currently accepts any URI string verbatim and passes it directly to the HTTP/FTP worker pipeline. This enables Server-Side Request Forgery (SSRF): an attacker can pass `file:///etc/shadow` to exfiltrate local files (reqwest processes `file://` URIs on Linux/macOS), or `http://169.254.169.254/latest/meta-data/` to steal cloud instance credentials (AWS/GCP/Azure metadata endpoints are on link-local addresses). The `file://` URI sub-case is especially critical: any non-`.torrent` URI is classified as `TaskType::Http` in `aura-daemon/src/jsonrpc.rs` and passed to reqwest, which silently reads and returns local file contents as a successful download. This issue is exacerbated by the CORS wildcard `chrome-extension://*` origin whitelist (ADR-0056) which creates a browser-based SSRF attack surface. No scheme validation, private IP blocking, or URL sanitization currently exists anywhere in the codebase. Related: GitHub Issue #241.

## Decision
1. Implement a `validate_download_uri(uri: &str) -> Result<(), UriValidationError>` function in `aura-core/src/net_util/` that is called before any URI enters the task pipeline.
2. Enforce a strict URI scheme allowlist: only `http://`, `https://`, `ftp://`, `ftps://`, and `magnet:` are permitted. All other schemes (including `file://`, `data://`, `javascript:`, `blob:`) must be rejected with `UriValidationError::ForbiddenScheme`.
3. Block private, loopback, and link-local destination addresses: after URI parsing, resolve the hostname to IP addresses and reject RFC 1918 ranges (`10.x.x.x`, `172.16.0.0/12`, `192.168.0.0/16`), loopback (`127.0.0.0/8`, `::1`), link-local (`169.254.0.0/16`, `fe80::/10`), and the unspecified address (`0.0.0.0`).
4. Enforce a maximum URI length of 8192 characters to prevent memory exhaustion from maliciously crafted long URIs.
5. URI validation must be performed in `aura-daemon/src/jsonrpc.rs::handle_add_uri()` before calling `engine.add_task_with_options()`. A secondary validation must also be performed in `aura-core/src/orchestrator/commands/add.rs` as a defense-in-depth layer.
6. Expose a configurable allowlist of private IP ranges via `Aura.toml` under `[security]` for legitimate intranet download use cases (e.g., enterprise NAS at `192.168.1.100`).

## Edge Cases
1. **DNS Rebinding Attack**: A hostname resolves to a public IP at validation time but is dynamically rebound to `127.0.0.1` by the time the connection is made. Mitigation: re-validate the resolved IP immediately before each TCP connection attempt in the HTTP worker, not just at URI acceptance time.
2. **IPv6-mapped IPv4 addresses**: `::ffff:10.0.0.1` is an IPv4-mapped IPv6 address that resolves to a private range. The validator must detect and block IPv6-mapped private addresses.
3. **Redirects to private IPs**: The server at `http://public.example.com` redirects to `http://192.168.1.1/admin`. The redirect follower in `worker/http/` must re-validate each redirected URL against the same rules.
4. **Magnet links with embedded tracker URLs**: `magnet:?tr=http://169.254.169.254/announce` — tracker URLs extracted from magnet links must also be validated before being passed to `TrackerClient`.
5. **IDNA hostnames**: Internationalized domain names (e.g., `аpple.com` using Cyrillic 'а') must be normalized and validated to prevent homograph attacks.

## Alternatives Considered
- **Firewall-level blocking**: Relying on OS firewall rules to block private IPs. *Rejected:* Not portable across platforms; requires elevated privileges; cannot be enforced at the application layer where URI intent is known.
- **Allowlist-only approach**: Only accept URIs on a user-configured allowlist. *Rejected:* Too restrictive for a general-purpose download manager.

## Consequences
- **Pros**: Eliminates the SSRF attack surface; protects users in shared or cloud environments; provides clear error messages for invalid URIs.
- **Cons**: Legitimate intranet downloads require explicit `security.allowed_private_ranges` configuration; adds a DNS resolution step at validation time (~10–50ms overhead per new task).
