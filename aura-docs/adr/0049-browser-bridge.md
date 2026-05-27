# 49. Browser Bridge (Extension Support)

Date: 2026-05-27

## Status

Accepted

## Context

Users often want to seamlessly offload large downloads from their web browser (Chrome, Firefox, Edge) to a dedicated download manager. To facilitate this without requiring users to manually copy and paste URIs into the Aura CLI or TUI, we need a mechanism to intercept downloads from the browser.

## Decision

We will implement a "Browser Bridge" feature.
- The Aura Daemon will expose a lightweight, authenticated local HTTP/WebSocket endpoint specifically designed to receive download tasks payload from a browser extension.
- We will provide a companion browser extension (using Manifest V3) that captures file downloads, extracts the URI, cookies, User-Agent, and Referer headers, and securely transmits them to the local Aura Daemon via this bridge.
- The bridge will validate incoming requests using a pre-shared local secret or token to prevent cross-site scripting (XSS) or cross-site request forgery (CSRF) attacks from malicious websites attempting to enqueue tasks on the user's machine.

## Consequences

- **Pros:** Significantly improves user experience by integrating directly into their daily web browsing workflow. Eliminates the need to copy cookies manually for authenticated downloads.
- **Cons:** Requires maintaining separate codebases (or extensions) for different browsers. Increases the daemon's attack surface, requiring strict local authentication.
