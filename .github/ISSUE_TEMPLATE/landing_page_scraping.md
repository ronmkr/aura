---
name: "Feature request: HTML Landing Page Scraping"
about: Scrape intermediate HTML pages for direct asset links when text/html is received.
title: "Feat: Scrape HTML Landing Pages for Direct Asset Link Resolution"
labels: ["type:enhancement", "priority:moderate", "area:worker-http"]
assignees: ""
---

### Problem Description
When users provide download URLs that point to intermediate HTML landing pages rather than direct binaries, the HTTP worker currently aborts with a Protocol error.

We need to implement inline HTML scraping to search for direct links matching standard patterns, asset suffixes, or anchor tags, and automatically resolve the direct link.

### Proposed Solution
- Add HTML document inspection using a scraper utility when a `text/html` Content-Type is received.
- Extract candidate asset links (e.g., `<a href="...">` containing matching download file extensions).
- Implement a recursive resolution heuristic (depth limit = 2) to follow the best asset candidate.
- Log landing-page resolution events to the orchestrator to update the task source target URI.

### Acceptance Criteria
- [ ] Implement HTML response body scanning up to 512KB.
- [ ] Use regex pattern matching to extract `href` links from anchor tags.
- [ ] Resolve relative candidate URLs using the `url` crate.
- [ ] Automatically resume metadata resolution loop with the extracted direct link.
