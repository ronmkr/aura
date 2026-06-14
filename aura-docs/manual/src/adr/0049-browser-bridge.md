# Decision 0049: Browser Bridge (Extension Support)

Date: 2026-05-27

## Status

Partially Implemented (2026-05-27, PR #102) — Chrome extension codebase deferred; see Priority note below.

## Priority

**Lowest** — The Chrome extension companion codebase is explicitly deferred until all P0–P3 issues are resolved. This feature will be the last major addition to the project. See GitHub issue #230.

## Context

Users often want to seamlessly offload large downloads from their web browser to a dedicated download manager. To facilitate this without requiring users to manually copy and paste URIs into the Aura CLI or TUI, we need a mechanism to intercept downloads from the browser.

**Scope Decision (2026-06-04)**: Aura will support **Chrome only** via a Manifest V3 extension. Firefox and Edge are explicitly out of scope. Rationale:
- Chrome has the largest market share for desktop users likely to use a download manager
- Manifest V3 has significant differences from Firefox's MV3 implementation (e.g., `declarativeNetRequest` vs `webRequest` API gaps); supporting both requires maintaining distinct codebases
- Firefox's WebExtensions API diverges enough that a separate codebase (not a thin shim) would be needed for full feature parity
- Edge supports Chrome extensions via the Chrome Web Store — Edge users can install the Chrome extension directly with no additional effort

## Decision

1. The Aura Daemon exposes a lightweight, authenticated local HTTP/WebSocket endpoint designed to receive download task payloads from the browser extension (`aura-daemon/src/extension.rs`, PR #102).
2. We will provide a **Chrome-only** companion extension (Manifest V3) that captures file downloads, extracts the URI, cookies, User-Agent, and Referer headers, and securely transmits them to the local Aura Daemon via this bridge.
3. The bridge validates incoming requests using a pre-shared local secret or token to prevent XSS/CSRF attacks from malicious websites attempting to enqueue tasks.
4. The CORS policy on the bridge endpoint permits only `chrome-extension://` origins. Firefox (`moz-extension://`) and Edge (`chrome-extension://` re-used) origins are not explicitly whitelisted.

## Deferred

- **Firefox support**: Deferred indefinitely. Firefox's MV3 implementation is incomplete as of 2026; `webRequest` blocking requires re-evaluation when Firefox MV3 stabilizes.
- **Edge support**: Edge users can sideload the Chrome extension from the Chrome Web Store — no separate extension needed.
- **Safari support**: Out of scope. Safari Web Extensions require macOS-specific tooling and App Store distribution, making it disproportionately complex.

## Alternatives Considered

- **Cross-browser support (Chrome + Firefox + Edge)**: Would require maintaining three distinct extension bundles due to API differences. *Rejected:* Disproportionate maintenance burden for diminishing returns; Edge re-uses Chrome extensions natively.
- **Native Messaging Protocol**: Using the browser's native messaging API instead of localhost HTTP. *Rejected:* Requires OS-level installation of a native messaging host manifest; more complex setup than a simple localhost endpoint.

## Consequences

- **Pros**: Significantly improves user experience by integrating into the Chrome browsing workflow. Eliminates the need to manually copy cookies for authenticated downloads. Single extension codebase to maintain.
- **Cons**: Chrome-only; Firefox and Safari users must use the CLI/TUI. The daemon's bridge endpoint increases the attack surface — strict local authentication (Decision-0056) and SSRF protection (Decision-0059) are prerequisites before the extension ships.

## Implementation

- **Browser Bridge Daemon Side**: Implemented via `aura-daemon/src/extension.rs` (PR #102).
- **Chrome Extension**: Pending — tracked in GitHub issue #230. Blocked on Decision-0059 (SSRF protection) being implemented first.
