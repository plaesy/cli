---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Implement durable task run lifecycle and recovery
phase: implement
status: backlog
---

# Implement durable task run lifecycle and recovery

## Goal

Persist task/run identities and truthful state, and recover or reconcile interrupted execution for the selected workflow.

## Scope

- Implement task/run state transitions, stable correlation IDs, idempotent boundaries, persisted results/artifacts, timeout/cancel and restart reconciliation according to child spec. Use fake adapter first; do not enable production side effects before failure tests pass.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Persisted run lifecycle and recovery implementation with integration tests.

## Dependencies

- Adapter contract/fake suite; identity/policy boundary; child spec.

## Acceptance criteria

[ ] Success, failure, timeout, cancel, restart and ambiguous external outcome have explicit persisted states.

- [ ] Repeated request does not duplicate protected side effects or is explicitly surfaced for reconciliation.

- [ ] Restart preserves task/run IDs and linked evidence.
- [ ] A worker/child persistence facade covers every session-store operation needed by its runtime consumers; parity is checked against the canonical store, and unsupported operations fail closed.
- [ ] Session close, execution completion, and deferred-work settlement are distinct persisted lifecycle outcomes; restart reconciliation covers interrupted and ambiguous states.
- [ ] Completion requires acceptance evidence, not agent prose alone.

- [ ] Focused integration tests pass on disposable storage.
