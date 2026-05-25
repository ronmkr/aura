---
title      : "Bug: Metalink parser has debug eprintln and hardcoded priority"
labels     : [type:bug, priority:low, area:metalink]
description: |
  Two issues found in `metalink/logic.rs` during the deep-dive audit:
  1. Line 83 has `eprintln!("DEBUG: ...")` left in production code.
  2. Mirror priority is always set to 0 — the `priority` attribute from `<url priority="...">` is never parsed from the XML.

  Acceptance criteria:
  - Remove the debug `eprintln!` statement.
  - Parse the `priority` attribute from `<url>` elements and populate `MetalinkResource.priority`.
  - Add test with multi-priority mirrors verifying correct ordering.
---
