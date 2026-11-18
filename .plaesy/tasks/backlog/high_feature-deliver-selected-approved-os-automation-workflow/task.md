---
spec: plaesy-agent-platform
topic: feature-deliver-selected-approved-os-automation-workflow
title: Deliver selected approved OS automation workflow
phase: implement
status: backlog
---

# Deliver selected approved OS automation workflow

## Goal

Deliver a permission-gated OS automation workflow only for an explicitly approved platform.

## Scope

- Selected OS, actions, permissions, approval UX, isolation, audit and recovery.

## Non-goals

- Do not infer safe actions or platform support from docs alone; no privileged execution without explicit grants.

## Deliverables

- Rust broker/policy boundary, platform adapter, disposable test harness and audited controls.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Supported OS/platform and allowed actions are explicitly approved and pinned.

- [ ] Every action is least-privilege, scoped, auditable and denied without required grant/approval.
- [ ] Synthetic secrets and prohibited actions produce no side effects; cancellation/timeout is honored.

- [ ] Isolation, failure recovery and operator-visible status pass on declared platform.
- [ ] Rust gates and platform-specific E2E evidence pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
