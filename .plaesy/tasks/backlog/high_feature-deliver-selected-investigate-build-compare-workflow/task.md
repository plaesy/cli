---
spec: plaesy-agent-platform
topic: feature-deliver-selected-investigate-build-compare-workflow
title: Deliver selected investigate build compare workflow
phase: implement
status: backlog
---

# Deliver selected investigate build compare workflow

## Goal

Deliver approved clean-room investigate/build/compare workflow with reproducible evidence.

## Scope

- Selected orchestration steps, tools, artifact provenance, compare criteria and safety boundaries from audit.

## Non-goals

- No claiming source research, builds or test execution unless artifacts/commands prove them.

## Deliverables

- Rust task orchestration, bounded tool execution, artifacts and acceptance fixtures.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Each workflow stage has inputs, outputs, permissions, timeout and failure semantics.

- [ ] Evidence artifacts bind source revision, command/environment and result; unsupported claims remain UNKNOWN.
- [ ] Untrusted inputs cannot override authorization or cause undeclared external effects.

- [ ] Retry/recovery does not duplicate non-idempotent side effects.
- [ ] Disposable E2E and Rust quality gates pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
