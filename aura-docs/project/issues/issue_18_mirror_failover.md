---
title      : "Feat: Automated Multi-Source Mirror Failover and Health Tracking"
labels     : [type:enhancement, priority:critical, area:orchestrator]
status     : RESOLVED
resolved   : 2026-05-17
description: |
  Currently, if a protocol subtask (mirror source) fails, the range is recycled back into the pending pool. However, the system does not dynamically track mirror source health (e.g. tracking persistent 503/429 status codes, EWMA timeouts, or connection failures) to select or transition to hot-standby mirror URLs (such as those provided via Metalink manifests).

  We need to implement a robust source health state machine in the orchestrator.

  Acceptance criteria:
  - Add `MirrorState` tracking (e.g. `Active`, `Degraded`, `Blacklisted`) inside `SubTask`.
  - Dynamically degrade or blacklist mirrors that hit specific error thresholds (e.g., 5 consecutive errors).
  - Automatically query alternative URL mirrors from task manifests (e.g. Metalink URLs) and launch failover subtasks.
  - Implement a fallback sequence that prevents retrying blacklisted sources.

  Resolution: Implemented in `orchestrator/events.rs` — retry logic with URI blacklisting, exponential backoff, and Metalink failover dispatch in `commands.rs::handle_add_task()`.
---
