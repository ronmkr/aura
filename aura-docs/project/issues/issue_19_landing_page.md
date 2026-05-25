---
title      : "Feat: Scrape HTML Landing Pages for Direct Asset Link Resolution"
labels     : [type:enhancement, priority:moderate, area:worker-http]
status     : RESOLVED
resolved   : 2026-05-17
description: |
  When users provide download URLs that point to intermediate HTML landing pages rather than direct binaries, the HTTP worker currently aborts with a Protocol error.

  We need to implement inline HTML scraping to search for direct links matching standard patterns, asset suffixes, or anchor tags, and automatically resolve the direct link.

  Acceptance criteria:
  - Add HTML document inspection using a scraper utility when a `text/html` Content-Type is received.
  - Extract candidate asset links (e.g., `<a href="...">` containing matching download file extensions).
  - Implement a recursive resolution heuristic (depth limit = 2) to follow the best asset candidate.
  - Log landing-page resolution events to the orchestrator to update the task source target URI.

  Resolution: Implemented in `worker/http.rs` — detects `text/html` content-type, regex-scrapes `<a href>` links, matches against asset extensions (.zip, .tar.gz, .dmg, .exe, .pkg, .iso, .rar, .7z, .bin, .msi, .pdf, .mp4, .mkv, .tar). Includes wiremock integration tests.
---
