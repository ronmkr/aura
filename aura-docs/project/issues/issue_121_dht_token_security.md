---
title      : "Bug: DHT token is hardcoded [1,2,3,4] — security vulnerability"
labels     : [type:bug, priority:critical, area:dht]
description: |
  The DHT actor in `dht/actor/` uses a hardcoded token `[1, 2, 3, 4]` for all `announce_peer` responses and outgoing announcements. This means any remote peer can forge announce_peer requests using this predictable token, potentially poisoning the DHT routing table or redirecting peer lookups.

  Per BEP 5, tokens should be generated using a random secret that rotates periodically, and validated against the requesting node's IP address.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Generate tokens using a SHA-256 HMAC of the requesting node's IP + a rotating secret (rotated every 10 minutes, keeping the previous secret for validation).
  - Validate incoming announce_peer tokens against the requesting IP + both current and previous secrets.
  - Reject announce_peer requests with invalid tokens.
  - Add unit tests for token generation, validation, and rotation.
---
