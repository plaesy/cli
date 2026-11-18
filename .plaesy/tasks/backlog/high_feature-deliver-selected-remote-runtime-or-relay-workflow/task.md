---
spec: plaesy-agent-platform
topic: feature-deliver-selected-remote-runtime-or-relay-workflow
title: Deliver selected remote runtime or relay workflow
phase: implement
status: backlog
---

# Deliver selected remote runtime or relay workflow

## Goal

Deliver remote runtime/relay only if an owner-selected workflow requires it.

## Scope

- Authenticated transport, project-scoped identity, delivery/liveness semantics, retry and recovery.

## Non-goals

- No unauthenticated endpoint, caller-ID-as-auth, or false claim of realtime/push/wakeup/read receipt.

## Deliverables

- Rust service/protocol, durable messaging where required, threat tests, deployment and operations artifacts.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Owner-approved topology/trust boundary and delivery semantics are documented.

- [ ] Authentication and authorization resist forged identity and cross-project access.
- [ ] Queued/accepted/acknowledged/read/liveness states are distinct and evidence-backed.

- [ ] Duplicate, lost acknowledgement, offline peer, timeout, revocation and restart tests pass.
- [ ] No network-exposed SQLite; deployment profile and recovery runbook validated.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
