---
name: "Feature request: Automated Multi-Source Mirror Failover"
about: Track mirror health and dynamically failover to backup URLs.
title: "Feat: Automated Multi-Source Mirror Failover and Health Tracking"
labels: ["type:enhancement", "priority:critical", "area:orchestrator"]
assignees: ""
---

### Problem Description
Currently, if a protocol subtask (mirror source) fails, the range is recycled back into the pending pool. However, the system does not dynamically track mirror source health (e.g. tracking persistent 503/429 status codes, EWMA timeouts, or connection failures) to select or transition to hot-standby mirror URLs (such as those provided via Metalink manifests).

We need to implement a robust source health state machine in the orchestrator.

### Proposed Solution
- Add `MirrorState` tracking (e.g. `Active`, `Degraded`, `Blacklisted`) inside `SubTask`.
- Dynamically degrade or blacklist mirrors that hit specific error thresholds (e.g., 5 consecutive errors).
- Automatically query alternative URL mirrors from task manifests (e.g. Metalink URLs) and launch failover subtasks.
- Implement a fallback sequence that prevents retrying blacklisted sources.

### Acceptance Criteria
- [ ] Add `MirrorState` tracking in `SubTask` inside `aura-core`.
- [ ] Dynamically degrade or blacklist mirrors after 5 consecutive failures.
- [ ] Implement query loop for alternative Metalink URL mirrors.
- [ ] Prevent enqueuing/dispatching ranges to blacklisted sources.
