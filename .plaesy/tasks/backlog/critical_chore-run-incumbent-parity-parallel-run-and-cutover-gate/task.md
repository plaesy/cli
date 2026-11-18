---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Run incumbent parity parallel-run and cutover gate
phase: implement
status: backlog
---

# Run incumbent parity parallel-run and cutover gate

## Goal

Demonstrate first-workflow parity, migration integrity, operational recovery and rollback before any incumbent cutover.

## Scope

- Execute same reproducible scenario against incumbent and Plaesy using pinned versions/config. Compare output, permissions, failures, artifacts, state and recovery; run parallel pilot; reconcile data; define accountable cutover and rollback triggers. Keep incumbent available unless gate is approved.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Evidence-backed parity report, migration reconciliation, parallel-run results and signed/authorized cutover decision or explicit blocker.

## Dependencies

- Incumbent evidence; task/run lifecycle; operator controls; backup/restore/rollback; all security gates.

## Acceptance criteria

[ ] Same workflow input and acceptance rubric run on both pinned versions.

- [ ] Material differences and unknowns documented; no unsupported parity claim.

- [ ] Migration counts/checksums reconcile and restore drill passes.
- [ ] Rollback trigger, owner and procedure exercised.

- [ ] No incumbent disabled before owner-approved gate passes.
