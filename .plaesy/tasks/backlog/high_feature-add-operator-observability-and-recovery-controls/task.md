---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Add operator observability and recovery controls
phase: implement
status: backlog
---

# Add operator observability and recovery controls

## Goal

Expose truthful run/task/queue state, failure reason, correlation and authorized recovery for the chosen workflow.

## Scope

- Use persisted lifecycle as source of truth. Add only required operator surface; avoid a UI implementation until the selected surface is decided. Include structured diagnostics and redact secrets.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- CLI/API/Studio operator journey and recovery tests.

## Dependencies

- Persisted run lifecycle; operator-surface decision.

## Acceptance criteria

[ ] Operator can find one run by stable ID and see task, state, timestamps, reason and evidence.

- [ ] Retry/cancel/recover actions are permission-checked and audited.

- [ ] Queue/adapter unavailable state is distinguishable from success.
- [ ] Synthetic secret is absent from support output.

- [ ] Focused operator acceptance test passes.
