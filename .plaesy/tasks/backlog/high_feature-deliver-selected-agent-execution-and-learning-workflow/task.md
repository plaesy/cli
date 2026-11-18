---
spec: plaesy-agent-platform
topic: feature-deliver-selected-agent-execution-and-learning-workflow
title: Deliver selected agent execution and learning workflow
phase: implement
status: backlog
---

# Deliver selected agent execution and learning workflow

## Goal

Deliver an owner-selected agent execution/learning workflow, integrating approved backends only through tested contracts.

## Scope

- Selected workflow scenarios, agent configuration, tools/skills, sessions, memory behavior, failure recovery and operator controls.

## Non-goals

- Do not modify Hermes/upstream; do not claim all provider or harness behavior is native Plaesy.

## Deliverables

- Rust core/runtime integration and UI only under approved ADR

- runnable acceptance fixtures and support matrix.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Workflow and product boundary are approved and mapped to audited capability IDs.
- [ ] If scheduling is selected, distinguish heartbeat-style same-session idle prompts from cron-style isolated jobs; test pause/resume/clear, coalescing/restart recovery, stop-in-flight, receipts, uncertain delivery, and retry policy. Script-only jobs are in scope only if explicitly selected.
- [ ] If Hermes-style delegation is selected, test joined success/failure/timeout/interruption outcomes separately from durable run restart; document that transcripts, reconnect, and worktree isolation are UNKNOWN until accepted and verified.
- [ ] If memory is selected, acceptance covers session-start snapshot and next-session visibility, capacity/write failure, pending approval vs committed write, profile isolation, session-search boundary, and provider discovery/activation precedence.
- [ ] Candidate evidence from Hermes docs at pinned snapshot `40d1e95f37e409f9f618f32c170661c66b86a4f4`: heartbeat/cron pages, delegation and subagent lifecycle pages, memory and memory-provider pages. These are documentation-derived only; map to exact lines in the evidence ledger before claiming parity.
- [ ] Owner decisions for workflow inclusion remain explicit; unselected jobs, providers, OS confinement, catch-up semantics and platform-specific spawn behavior stay UNKNOWN/deferred.

- [ ] Backend capability matrix distinguishes backend-provided and Plaesy-owned behavior.
- [ ] Success, denial, timeout, cancellation, restart and provider failure outcomes are tested and truthful.

- [ ] Memory/skill provenance and secret/policy boundaries are verified.
- [ ] Rust checks and end-to-end disposable-profile acceptance pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
